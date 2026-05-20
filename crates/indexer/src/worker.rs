use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use alloy::consensus::Transaction as ConsensusTx;
use alloy::eips::BlockNumberOrTag;
use alloy::network::TransactionResponse;
use alloy::providers::ext::DebugApi;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::trace::geth::{GethDebugTracingOptions, GethTrace};
use backoff::ExponentialBackoffBuilder;
use bigdecimal::BigDecimal;
use chrono::DateTime;
use sqlx::PgPool;
use tokio::sync::Semaphore;

use db::models::{
    Block, ErrorCategory, FailedTransaction, LiquidityEvent, LiquidityEventType, SwapEvent,
    TokenTransfer, TraceLog, Transaction,
};
use decoder::events::{DecodedEvent, DecodedLiquidity, DecodedSwap, DecodedTransfer};

/// 블록 범위를 청크 단위로 분할하여 병렬 수집하는 워커 풀.
pub struct WorkerPool {
    /// DB 연결 풀
    db_pool: PgPool,
    /// RPC 엔드포인트
    rpc_url: String,
    /// 동시 실행 제한 세마포어
    semaphore: Arc<Semaphore>,
    /// 배치 INSERT 크기
    batch_size: usize,
}

impl WorkerPool {
    /// 새 워커 풀을 생성한다.
    pub fn new(db_pool: PgPool, rpc_url: String, max_concurrent: usize, batch_size: usize) -> Self {
        Self {
            db_pool,
            rpc_url,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            batch_size,
        }
    }

    /// 지정된 블록 범위를 인덱싱한다.
    ///
    /// 블록 범위를 `batch_size` 단위 청크로 분할하고, `tokio::JoinSet`으로 병렬
    /// 수집한다. 청크 사이마다 `cancel` 플래그를 확인해 ctrl_c 등 외부 신호에
    /// **청크 경계에서** 정상 종료한다(HARDEN-T01/R4). 체크포인트는 청크 단위로
    /// 박혀 있어 조기 종료해도 부분 진행이 보존된다. 취소가 필요 없는 호출자는
    /// `&AtomicBool::new(false)`를 넘기면 종래 동작과 동일.
    #[tracing::instrument(skip(self, cancel))]
    pub async fn index_range(
        &self,
        from_block: u64,
        to_block: u64,
        cancel: &AtomicBool,
    ) -> anyhow::Result<()> {
        tracing::info!(from_block, to_block, "starting block range indexing");

        let total = to_block.saturating_sub(from_block) + 1;
        let mut processed = 0u64;

        for chunk_start in (from_block..=to_block).step_by(self.batch_size) {
            if cancel.load(Ordering::Relaxed) {
                tracing::info!(
                    from_block,
                    to_block,
                    processed,
                    "cancellation requested — stopping index_range at chunk boundary"
                );
                return Ok(());
            }
            let chunk_end = (chunk_start + self.batch_size as u64 - 1).min(to_block);
            self.process_chunk(chunk_start, chunk_end).await?;

            // 체크포인트 갱신 (chunk 완료 후)
            db::queries::update_checkpoint(&self.db_pool, 1, chunk_end as i64).await?;

            processed += chunk_end - chunk_start + 1;
            tracing::info!(processed, total, "progress");
        }

        tracing::info!(total_blocks = total, "indexing complete");
        Ok(())
    }

    /// 체인 헤드를 따라가며 연속 인덱싱한다.
    ///
    /// `head - confirmations`까지만 인덱싱(얕은 reorg 완화, D009).
    /// `ctrl_c` 수신 시 진행 중 작업을 마치고 graceful 종료한다.
    pub async fn follow(
        &self,
        confirmations: u64,
        poll: Duration,
        trigger: TriggerMode,
    ) -> anyhow::Result<()> {
        let provider = ProviderBuilder::new().connect_http(
            self.rpc_url
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid RPC URL: {e}"))?,
        );
        tracing::info!(
            confirmations,
            poll_secs = poll.as_secs(),
            trigger = trigger.label(),
            "follow mode started (Ctrl-C to stop)"
        );

        let mut m = FollowMetrics::default();

        // Cancellation watcher (HARDEN-T01/R4): ctrl_c가 도착하면 공유 flag를
        // 켠다. `index_range`가 청크 사이에서 이 flag를 확인해 거대한 백필
        // 사이클이라도 빠르게 graceful 종료. wait phase의 `tokio::select!`는
        // 종전대로 사이클 사이의 즉시 종료를 따로 처리한다(중복 안전).
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let cancel = Arc::clone(&cancel);
            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                cancel.store(true, Ordering::Relaxed);
            });
        }

        // 트리거: 구독 모드면 newHeads 틱 채널을 띄운다. 연결/구독 실패나
        // 스트림 종료 시 채널이 닫혀 자동으로 폴링으로 폴백한다(무회귀, D011).
        let mut ws_rx = match &trigger {
            TriggerMode::Subscribe(ws_url) => Some(spawn_head_ticks(ws_url.clone())),
            TriggerMode::Polling => None,
        };

        loop {
            m.cycle += 1;

            // reorg 체크가 우선 — fork면 롤백 후 재인덱싱(되감긴 checkpoint).
            // 불확실(RPC 실패/블록 부재)이면 detect_fork가 None → 무작동(안전).
            if let Some(fork) = self.detect_fork(&provider, REORG_SCAN_CAP).await? {
                tracing::warn!(cycle = m.cycle, fork, "reorg detected — rolling back");
                let rolled = db::queries::rollback_from_block(&self.db_pool, fork as i64).await?;
                m.reorgs += 1;
                m.last_reorg_depth = rolled;
                tracing::warn!(
                    cycle = m.cycle,
                    fork,
                    depth = rolled,
                    reorgs_total = m.reorgs,
                    "rolled back — re-indexing next cycle"
                );
                continue;
            }

            let head = rpc_with_retry(|| async {
                provider
                    .get_block_number()
                    .await
                    .map_err(|e| anyhow::anyhow!(e))
            })
            .await?;

            let checkpoint = db::queries::get_last_checkpoint(&self.db_pool, 1).await?;
            let mut indexed_this_cycle = 0u64;
            match next_target(head, confirmations, checkpoint) {
                Some((from, to)) => {
                    // 사이클당 범위 cap(HARDEN-T01/R3) — 거대한 lag에서도 reorg
                    // 체크가 자주 돌고 ctrl_c 응답성 유지. 잔여는 다음 사이클.
                    let (from_c, to_c) = cap_range_to(from, to, FOLLOW_CYCLE_BLOCK_CAP);
                    let capped = to_c < to;
                    tracing::info!(
                        head,
                        from = from_c,
                        to = to_c,
                        capped,
                        "following: indexing new range"
                    );
                    self.index_range(from_c, to_c, cancel.as_ref()).await?;
                    indexed_this_cycle = (to_c - from_c) + 1;
                    m.blocks_indexed += indexed_this_cycle;
                }
                None => tracing::debug!(head, "following: no new confirmed blocks"),
            }

            // 인덱싱 중 ctrl_c가 들어왔으면 wait phase 없이 즉시 종료(R4).
            if cancel.load(Ordering::Relaxed) {
                tracing::info!("cancellation observed — stopping follow loop");
                break;
            }

            // 사이클당 구조화 관측 1줄: lag·처리량·reorg·마지막 폴 시각.
            // (관측 전용 — 분기/타이밍/IO 불변)
            let now = chrono::Utc::now();
            let lag = checkpoint
                .map(|c| head.saturating_sub(c.max(0) as u64))
                .unwrap_or(0);
            tracing::info!(
                cycle = m.cycle,
                head,
                checkpoint = checkpoint.unwrap_or(-1),
                lag,
                indexed_this_cycle,
                blocks_total = m.blocks_indexed,
                reorgs_total = m.reorgs,
                last_reorg_depth = m.last_reorg_depth,
                last_poll = %now,
                "follow cycle summary"
            );

            // 다음 사이클 트리거: 구독이면 newHeads 틱, 아니면 sleep(poll).
            // ctrl_c는 항상 레이스(graceful 종료). 채널이 닫히면 폴링 폴백.
            let mut fall_back = false;
            match ws_rx.as_mut() {
                Some(rx) => {
                    tokio::select! {
                        tick = rx.recv() => {
                            if tick.is_none() {
                                tracing::warn!(
                                    "newHeads channel closed — falling back to polling"
                                );
                                fall_back = true;
                            }
                        }
                        _ = tokio::signal::ctrl_c() => {
                            tracing::info!("Ctrl-C received — stopping follow loop");
                            break;
                        }
                    }
                }
                None => {
                    tokio::select! {
                        _ = tokio::time::sleep(poll) => {}
                        _ = tokio::signal::ctrl_c() => {
                            tracing::info!("Ctrl-C received — stopping follow loop");
                            break;
                        }
                    }
                }
            }
            if fall_back {
                ws_rx = None;
            }
        }
        Ok(())
    }

    /// 최근 로컬 블록 해시를 체인과 대조해 reorg fork point를 찾는다.
    ///
    /// **lazy + 동적 확대** (R1/R2): tip부터 필요한 만큼만 체인 해시를 조회한다 —
    /// 정상(tip 일치) 시 RPC 1회로 단락(R2). 윈도우 전체 불일치면 cap
    /// (`REORG_SCAN_CAP`)까지 [`next_scan_depth`]로 배수 확대해 **진짜 최소
    /// 공통조상**을 찾는다(R1: under-delete 갭 해소). 순수 판정은
    /// [`classify_fork`]/[`next_scan_depth`]; 여기선 비동기 조회만 담당.
    ///
    /// RPC 조회 불가/블록 부재(불확실)면 `None` — 파괴적 롤백 false positive
    /// 방지(안전 규칙 불변).
    async fn detect_fork(&self, provider: &impl Provider, cap: i64) -> anyhow::Result<Option<u64>> {
        let local: Vec<(u64, String)> = db::queries::recent_block_hashes(&self.db_pool, cap)
            .await?
            .into_iter()
            .map(|(n, h)| (n as u64, h))
            .collect();
        if local.is_empty() {
            return Ok(None);
        }
        let len = local.len();

        let mut chain: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
        let mut depth = 1usize;
        loop {
            // 깊이 [0,depth) 중 미조회분만 lazy fetch (tip 일치면 1회로 끝 — R2)
            for (height, _) in &local[..depth.min(len)] {
                let h = *height;
                if chain.contains_key(&h) {
                    continue;
                }
                let fetched = rpc_with_retry(|| async {
                    provider
                        .get_block_by_number(BlockNumberOrTag::Number(h))
                        .await
                        .map_err(|e| anyhow::anyhow!(e))
                })
                .await;
                match fetched {
                    Ok(Some(block)) => {
                        chain.insert(h, format!("0x{:x}", block.header.hash));
                    }
                    // 블록 부재(헤드 밖) 또는 RPC 실패 → 불확실: 무롤백(안전)
                    Ok(None) | Err(_) => return Ok(None),
                }
            }

            match classify_fork(&local, |h| chain.get(&h).cloned(), depth) {
                ForkScan::NoReorg | ForkScan::Inconclusive => return Ok(None),
                ForkScan::Fork(f) => return Ok(Some(f)),
                ForkScan::NeedDeeper => {
                    let next = next_scan_depth(depth, len);
                    if next == depth {
                        // NeedDeeper면 depth<len이라 논리상 도달 불가 — 안전망
                        return Ok(Some(local[len - 1].0));
                    }
                    depth = next;
                }
            }
        }
    }

    /// 단일 블록 청크를 처리한다.
    #[tracing::instrument(skip(self))]
    async fn process_chunk(&self, from: u64, to: u64) -> anyhow::Result<()> {
        let provider = ProviderBuilder::new().connect_http(
            self.rpc_url
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid RPC URL: {e}"))?,
        );

        let mut join_set = tokio::task::JoinSet::new();

        for block_num in from..=to {
            let permit = Arc::clone(&self.semaphore);
            let provider = provider.clone();
            let db_pool = self.db_pool.clone();

            join_set.spawn(async move {
                let _permit = permit.acquire().await?;
                Self::process_block(&db_pool, &provider, block_num).await
            });
        }

        while let Some(result) = join_set.join_next().await {
            result??;
        }

        Ok(())
    }

    /// 단일 블록을 수집·디코딩·저장한다.
    ///
    /// 1. RPC로 블록 + 영수증 fetch
    /// 2. 각 로그를 decoder로 디코딩
    /// 3. DB에 배치 INSERT
    async fn process_block(
        db_pool: &PgPool,
        provider: &impl Provider,
        block_number: u64,
    ) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        tracing::debug!(block_number, "processing block");

        // 1. 블록 + 영수증 병렬 조회 (with retry)
        let block_num_tag = BlockNumberOrTag::Number(block_number);
        let (block, receipts) = tokio::try_join!(
            rpc_with_retry(|| async {
                provider
                    .get_block_by_number(block_num_tag)
                    .full()
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?
                    .ok_or_else(|| anyhow::anyhow!("block {block_number} not found"))
            }),
            rpc_with_retry(|| async {
                provider
                    .get_block_receipts(block_num_tag.into())
                    .await
                    .map_err(|e| anyhow::anyhow!(e))
                    .map(|r| r.unwrap_or_default())
            })
        )?;

        let timestamp =
            DateTime::from_timestamp(block.header.timestamp as i64, 0).unwrap_or_default();

        // 2. Block 모델 저장
        let block_model = Block {
            block_number: block_number as i64,
            timestamp,
            gas_used: block.header.gas_used as i64,
            block_hash: Some(format!("0x{:x}", block.header.hash)),
            parent_hash: Some(format!("0x{:x}", block.header.parent_hash)),
        };
        db::queries::insert_blocks(db_pool, &[block_model]).await?;

        // 4. 트랜잭션 모델 빌드 + 로그 디코딩
        let block_txs: Vec<_> = block.transactions.into_transactions().collect();
        let mut transactions = Vec::with_capacity(block_txs.len());
        let mut swap_events: Vec<SwapEvent> = Vec::new();
        let mut liquidity_events: Vec<LiquidityEvent> = Vec::new();
        let mut token_transfers: Vec<TokenTransfer> = Vec::new();

        for (idx, receipt) in receipts.iter().enumerate() {
            let tx_hash_str = format!("0x{:x}", receipt.transaction_hash);

            // 트랜잭션 모델 빌드 (블록 TX + 영수증 매칭)
            if let Some(tx) = block_txs.get(idx) {
                let gas_price = tx.effective_gas_price.unwrap_or(0);
                let tx_model = Transaction {
                    tx_hash: tx_hash_str.clone(),
                    from_addr: format!("{}", tx.from()).to_lowercase(),
                    to_addr: ConsensusTx::to(tx).map(|a| format!("{a}").to_lowercase()),
                    block_number: block_number as i64,
                    gas_used: receipt.gas_used as i64,
                    gas_price: BigDecimal::from_str(&gas_price.to_string())
                        .unwrap_or_else(|_| BigDecimal::from(0)),
                    value: BigDecimal::from_str(&ConsensusTx::value(tx).to_string())
                        .unwrap_or_else(|_| BigDecimal::from(0)),
                    status: if receipt.status() { 1 } else { 0 },
                    input_data: if ConsensusTx::input(tx).is_empty() {
                        None
                    } else {
                        Some(format!("{}", ConsensusTx::input(tx)))
                    },
                };
                transactions.push(tx_model);
            }

            // 로그 디코딩
            for log in receipt.inner.logs() {
                let log_data = log.data();
                let topics = log_data.topics().to_vec();
                if topics.is_empty() {
                    continue;
                }

                let data = &log_data.data;
                let log_address = format!("{}", log.address()).to_lowercase();
                let log_idx = log.log_index.unwrap_or(0) as i32;

                match decoder::events::decode_log(
                    &topics,
                    data,
                    &log_address,
                    &tx_hash_str,
                    log_idx,
                    timestamp,
                ) {
                    Ok(DecodedEvent::Swap(s)) => swap_events.push(to_swap_model(s)),
                    Ok(DecodedEvent::Liquidity(l)) => {
                        liquidity_events.push(to_liquidity_model(l));
                    }
                    Ok(DecodedEvent::Transfer(t)) => {
                        token_transfers.push(to_transfer_model(t));
                    }
                    Err(decoder::error::DecodeError::UnknownTopic(_)) => {}
                    Err(e) => {
                        tracing::warn!(block_number, error = %e, "failed to decode log");
                    }
                }
            }
        }

        // 5. 배치 INSERT
        if !transactions.is_empty() {
            db::queries::insert_transactions(db_pool, &transactions).await?;
        }
        if !swap_events.is_empty() {
            db::queries::insert_swap_events(db_pool, &swap_events).await?;
        }
        if !liquidity_events.is_empty() {
            db::queries::insert_liquidity_events(db_pool, &liquidity_events).await?;
        }
        if !token_transfers.is_empty() {
            db::queries::insert_token_transfers(db_pool, &token_transfers).await?;
        }

        // 6. 실패한 트랜잭션 trace 수집
        let failed_tx_hashes: Vec<String> = receipts
            .iter()
            .filter(|r| !r.status())
            .map(|r| format!("0x{:x}", r.transaction_hash))
            .collect();

        if !failed_tx_hashes.is_empty() {
            let (trace_logs, failed_txs) =
                Self::process_failed_txs(provider, &failed_tx_hashes, timestamp).await;

            if !trace_logs.is_empty() {
                db::queries::insert_trace_logs(db_pool, &trace_logs).await?;
            }
            if !failed_txs.is_empty() {
                db::queries::insert_failed_transactions(db_pool, &failed_txs).await?;
            }

            tracing::debug!(
                block_number,
                failed_txs = failed_tx_hashes.len(),
                traces = trace_logs.len(),
                "traces processed"
            );
        }

        tracing::info!(
            block_number,
            txs = transactions.len(),
            swaps = swap_events.len(),
            liq = liquidity_events.len(),
            transfers = token_transfers.len(),
            duration_ms = start.elapsed().as_millis() as u64,
            "block_processed"
        );
        Ok(())
    }

    /// 실패한 트랜잭션들의 trace를 수집·디코딩한다.
    async fn process_failed_txs(
        provider: &impl Provider,
        tx_hashes: &[String],
        timestamp: DateTime<chrono::Utc>,
    ) -> (Vec<TraceLog>, Vec<FailedTransaction>) {
        let mut trace_logs = Vec::new();
        let mut failed_txs = Vec::new();

        let trace_opts = GethDebugTracingOptions::call_tracer(Default::default());

        for tx_hash_str in tx_hashes {
            let tx_hash = match tx_hash_str.parse() {
                Ok(h) => h,
                Err(_) => continue,
            };

            // debug_traceTransaction 호출
            let trace_result = match provider
                .debug_trace_transaction(tx_hash, trace_opts.clone())
                .await
            {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(tx_hash = %tx_hash_str, error = %e, "failed to trace tx");
                    continue;
                }
            };

            // GethTrace → JSON → parse_trace
            let trace_json = match &trace_result {
                GethTrace::CallTracer(frame) => match serde_json::to_value(frame) {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                _ => continue,
            };

            let flattened = match decoder::trace::parse_trace(tx_hash_str, &trace_json) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(tx_hash = %tx_hash_str, error = %e, "failed to parse trace");
                    continue;
                }
            };

            // FlatFrame → TraceLog 모델 변환
            for frame in &flattened.frames {
                trace_logs.push(TraceLog {
                    tx_hash: tx_hash_str.clone(),
                    call_depth: frame.depth,
                    call_type: frame.call_type.clone(),
                    from_addr: frame.from.clone(),
                    to_addr: frame.to.clone(),
                    value: BigDecimal::from_str(&frame.value)
                        .unwrap_or_else(|_| BigDecimal::from(0)),
                    gas_used: frame.gas_used,
                    input: frame.input.clone(),
                    output: frame.output.clone(),
                    error: frame.error.clone(),
                    trace_id: 0,
                });
            }

            // 루트 프레임에서 revert reason 추출
            let (revert_reason, error_category) = if let Some(root) = flattened.frames.first() {
                let reason = root
                    .output
                    .as_ref()
                    .and_then(|o| {
                        let hex = o.strip_prefix("0x").unwrap_or(o);
                        hex::decode(hex).ok()
                    })
                    .and_then(|bytes| decoder::trace::decode_revert_reason(&bytes).ok());

                let category = reason
                    .as_deref()
                    .map(decoder::classifier::classify_error)
                    .unwrap_or("UNKNOWN");

                (reason, category)
            } else {
                (None, "UNKNOWN")
            };

            let category = match error_category {
                "INSUFFICIENT_BALANCE" => ErrorCategory::InsufficientBalance,
                "SLIPPAGE_EXCEEDED" => ErrorCategory::SlippageExceeded,
                "DEADLINE_EXPIRED" => ErrorCategory::DeadlineExpired,
                "UNAUTHORIZED" => ErrorCategory::Unauthorized,
                "TRANSFER_FAILED" => ErrorCategory::TransferFailed,
                _ => ErrorCategory::Unknown,
            };

            // 루트 프레임의 input에서 function selector 추출
            let failing_function = flattened
                .frames
                .first()
                .and_then(|f| f.input.as_ref())
                .and_then(|input| {
                    let hex = input.strip_prefix("0x").unwrap_or(input);
                    if hex.len() >= 8 {
                        Some(format!("0x{}", &hex[..8]))
                    } else {
                        None
                    }
                });

            failed_txs.push(FailedTransaction {
                tx_hash: tx_hash_str.clone(),
                error_category: category,
                revert_reason,
                failing_function,
                gas_used: flattened.frames.first().map(|f| f.gas_used).unwrap_or(0),
                timestamp,
            });
        }

        (trace_logs, failed_txs)
    }
}

/// `DecodedSwap` → DB `SwapEvent` 변환.
fn to_swap_model(s: DecodedSwap) -> SwapEvent {
    SwapEvent {
        pool_address: s.pool_address,
        tx_hash: s.tx_hash,
        sender: s.sender,
        recipient: s.recipient,
        amount0: s.amount0,
        amount1: s.amount1,
        amount_in: s.amount_in,
        amount_out: s.amount_out,
        sqrt_price_x96: s.sqrt_price_x96,
        liquidity: s.liquidity,
        tick: s.tick,
        log_index: s.log_index,
        timestamp: s.timestamp,
        event_id: 0, // DB에서 자동 생성
    }
}

/// `DecodedLiquidity` → DB `LiquidityEvent` 변환.
fn to_liquidity_model(l: DecodedLiquidity) -> LiquidityEvent {
    let event_type = match l.event_type.as_str() {
        "BURN" => LiquidityEventType::Burn,
        _ => LiquidityEventType::Mint,
    };
    LiquidityEvent {
        event_type,
        pool_address: l.pool_address,
        tx_hash: l.tx_hash,
        provider: l.provider,
        token0_amount: l.token0_amount,
        token1_amount: l.token1_amount,
        tick_lower: l.tick_lower,
        tick_upper: l.tick_upper,
        liquidity: l.liquidity,
        log_index: l.log_index,
        timestamp: l.timestamp,
        event_id: 0,
    }
}

/// `DecodedTransfer` → DB `TokenTransfer` 변환.
fn to_transfer_model(t: DecodedTransfer) -> TokenTransfer {
    TokenTransfer {
        tx_hash: t.tx_hash,
        token_address: t.token_address,
        from_addr: t.from_addr,
        to_addr: t.to_addr,
        amount: t.amount,
        log_index: t.log_index,
        timestamp: t.timestamp,
        transfer_id: 0,
    }
}

/// RPC 호출을 지수 백오프로 재시도한다.
///
/// 최대 5회 재시도, 초기 간격 500ms, 최대 간격 10초.
async fn rpc_with_retry<F, Fut, T>(f: F) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let backoff = ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(500))
        .with_max_interval(Duration::from_secs(10))
        .with_max_elapsed_time(Some(Duration::from_secs(60)))
        .build();

    backoff::future::retry(backoff, || async {
        f().await.map_err(backoff::Error::transient)
    })
    .await
}

/// follow 루프의 누적 관측 카운터.
///
/// 경량(프로세스 메모리)·신규 의존성 없음 — 값은 `tracing` 필드로만 방출한다.
/// 동작/타이밍/IO에 영향 없음(관측 전용).
#[derive(Default)]
struct FollowMetrics {
    /// 루프 반복 횟수(1부터)
    cycle: u64,
    /// 누적 인덱싱 블록 수
    blocks_indexed: u64,
    /// 누적 reorg 발생 횟수
    reorgs: u64,
    /// 최근 reorg에서 롤백된 블록 수(깊이). 발생 전이면 0
    last_reorg_depth: u64,
}

/// 한 사이클에 인덱싱할 범위를 cap에 맞춰 자른다(HARDEN-T01/R3, 순수).
///
/// `to - from + 1 ≤ cap_blocks` 이면 원본 그대로, 아니면 `(from, from+cap-1)`.
/// `cap_blocks == 0` 또는 `to < from` 이면 자르지 않음(no-op). `u64` 산술은
/// saturating으로 처리해 tip 부근 오버플로 안전.
pub fn cap_range_to(from: u64, to: u64, cap_blocks: u64) -> (u64, u64) {
    if cap_blocks == 0 || to < from {
        return (from, to);
    }
    let span = to.saturating_sub(from).saturating_add(1);
    if span <= cap_blocks {
        return (from, to);
    }
    let capped_to = from.saturating_add(cap_blocks).saturating_sub(1);
    (from, capped_to)
}

/// follow 루프의 순수 결정 함수 — 다음에 인덱싱할 블록 범위.
///
/// `safe = head - confirmations` (얕은 reorg 완화, D009). 체크포인트가 있으면
/// 그 다음 블록부터, 없으면 tip(`safe`)부터 시작한다(follow는 전체 백필을 하지
/// 않는다). 새로 인덱싱할 확정 블록이 없으면 `None`.
pub fn next_target(
    head: u64,
    confirmations: u64,
    last_checkpoint: Option<i64>,
) -> Option<(u64, u64)> {
    let safe = head.saturating_sub(confirmations);
    let resume = match last_checkpoint {
        Some(c) => (c.max(0) as u64).saturating_add(1),
        None => safe,
    };
    if resume > safe {
        None
    } else {
        Some((resume, safe))
    }
}

/// reorg fork 탐색의 최대 깊이(cap). 윈도우 전체 불일치 시 여기까지 배수 확대해
/// 진짜 최소 공통조상을 찾는다(R1: under-delete 갭 해소). ≈4096블록(메인넷
/// 약 13.6h) — PoS finality(≈64)의 64배 마진. 로컬(자체 DB)은 한 번에 로드해도
/// 저렴하고, 체인 해시는 lazy 조회라 정상 시 RPC 비용은 cap과 무관(R2).
pub const REORG_SCAN_CAP: i64 = 4096;

/// follow 한 사이클이 한번에 인덱싱할 수 있는 블록 수 상한 (HARDEN-T01/R3).
///
/// 큰 lag에서 체크포인트가 한참 뒤처진 상태로 follow를 시작하면 `next_target`이
/// 거대한 범위를 반환해 `index_range`가 길게 도는 동안 ① reorg 체크가 지연되고
/// ② ctrl_c 응답성도 떨어진다. 사이클당 이 cap만큼만 처리하고 다음 사이클로
/// 넘겨 head 재조회·reorg 재검사 + 조기 graceful 종료 여유를 만든다. cap을
/// 넘는 범위는 자연스레 후속 사이클에서 이어 처리된다.
pub const FOLLOW_CYCLE_BLOCK_CAP: u64 = 500;

/// [`classify_fork`]의 판정 결과.
#[derive(Debug, PartialEq, Eq)]
pub enum ForkScan {
    /// tip부터 일치 — reorg 없음
    NoReorg,
    /// 높이 `f`부터(포함) 무효 → `rollback_from_block(f)`
    Fork(u64),
    /// 필요한 체인 해시 부재(불확실) — 안전 규칙: 무롤백
    Inconclusive,
    /// 본 깊이 전부 불일치 & 더 깊은 로컬 존재 → 호출자가 윈도우 확대
    NeedDeeper,
}

/// reorg fork point를 찾는 **순수** 함수 — tip부터 `depth`개만 본다(lazy).
///
/// `local`은 (블록번호, 로컬해시)를 **번호 내림차순(tip 먼저)** 연속 구간으로
/// 받는다. `chain_hash_at`는 해당 높이의 체인 해시(`None`=조회 불가).
///
/// - tip 일치 → [`ForkScan::NoReorg`] (정상 시 체인 조회 1회로 단락 — R2)
/// - 불일치 후 일치 만남 → [`ForkScan::Fork`]`(최저 불일치 높이)` (정확 롤백)
/// - 체인 해시 `None` → [`ForkScan::Inconclusive`] (안전 규칙: 무롤백)
/// - `depth`까지 전부 불일치 & 로컬이 더 있음 → [`ForkScan::NeedDeeper`]
///   (호출자가 [`next_scan_depth`]로 확대); 더 없으면 최저 로컬을 `Fork`(R1)
pub fn classify_fork(
    local: &[(u64, String)],
    chain_hash_at: impl Fn(u64) -> Option<String>,
    depth: usize,
) -> ForkScan {
    let n = depth.min(local.len());
    if n == 0 {
        return ForkScan::NoReorg;
    }
    let mut last_mismatch: Option<u64> = None;
    for (height, local_hash) in &local[..n] {
        match chain_hash_at(*height) {
            // 불확실 → 파괴적 롤백보다 이번 사이클 스킵이 안전
            None => return ForkScan::Inconclusive,
            Some(chain_hash) if &chain_hash == local_hash => {
                // 일치: 이 높이 이하는 정상(체인 연결성)
                return match last_mismatch {
                    Some(f) => ForkScan::Fork(f),
                    None => ForkScan::NoReorg,
                };
            }
            Some(_) => last_mismatch = Some(*height), // 불일치(계속 하강)
        }
    }
    // [0,n) 전부 불일치
    if n < local.len() {
        ForkScan::NeedDeeper
    } else {
        // 더 깊은 로컬 없음 → 최선의 floor(= 최저 로컬 높이). cap이 커서 잔여
        // 갭은 사실상 0이나, 도달 시 정직하게 best-effort 롤백.
        match last_mismatch {
            Some(f) => ForkScan::Fork(f),
            None => ForkScan::NoReorg,
        }
    }
}

/// 윈도우 확대 폭(순수): 현재 깊이를 ×4로 키우되 `max`로 클램프, 항상 전진
/// (`> current`)하도록 보장 → 종료성. `current >= max`면 `max`.
pub fn next_scan_depth(current: usize, max: usize) -> usize {
    if current >= max {
        return max;
    }
    current.saturating_mul(4).clamp(current + 1, max)
}

/// follow 사이클을 무엇이 깨우는지(트리거)를 나타낸다.
///
/// 폴링이 기본, 구독은 옵트인(D011). `Subscribe`는 (트림된) WS URL을 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerMode {
    /// `sleep(poll)`로 사이클을 깨운다(기본, 호환성 안전값).
    Polling,
    /// newHeads 구독으로 사이클을 깨운다. 내부 문자열 = WS 엔드포인트.
    Subscribe(String),
}

impl TriggerMode {
    /// 로그용 모드 라벨. **WS URL(시크릿 가능)은 절대 노출하지 않는다.**
    pub fn label(&self) -> &'static str {
        match self {
            TriggerMode::Polling => "polling",
            TriggerMode::Subscribe(_) => "subscribe",
        }
    }
}

/// `--subscribe`와 `WS_URL`에서 트리거 모드를 결정하는 순수 함수.
///
/// 구독을 요청(`subscribe=true`)했고 `ws_url`이 공백이 아닐 때만
/// `Subscribe(트림된 URL)`. 그 외엔 `Polling`으로 **폴백**한다 — WS 미지정/공백은
/// 회귀가 아니라 정상 기본값(D011). RPC/WS 없이 단위테스트 가능.
pub fn resolve_trigger_mode(subscribe: bool, ws_url: Option<&str>) -> TriggerMode {
    match (subscribe, ws_url.map(str::trim)) {
        (true, Some(u)) if !u.is_empty() => TriggerMode::Subscribe(u.to_string()),
        _ => TriggerMode::Polling,
    }
}

/// newHeads 구독을 백그라운드에서 돌리며 새 헤드마다 사이클 틱을 보낸다.
///
/// WS 연결/구독 실패나 스트림 종료 시 태스크가 끝나 채널이 닫히고, 호출자는
/// 자동으로 폴링으로 폴백한다(무회귀, D011). 연결을 수명 동안 살리기 위해
/// provider를 이 태스크가 소유한다. **WS URL은 로그에 찍지 않는다(시크릿 가능).**
fn spawn_head_ticks(ws_url: String) -> tokio::sync::mpsc::Receiver<()> {
    let (tx, rx) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(async move {
        let provider = match ProviderBuilder::new()
            .connect_ws(WsConnect::new(ws_url))
            .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "WS connect failed — falling back to polling");
                return;
            }
        };
        let mut sub = match provider.subscribe_blocks().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "newHeads subscribe failed — falling back to polling");
                return;
            }
        };
        tracing::info!("subscribe mode: newHeads subscription active");
        loop {
            match sub.recv().await {
                Ok(_) => {
                    // 수신자(follow 루프) 종료 시 send 실패 → 태스크 정리
                    if tx.send(()).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "newHeads stream ended — falling back to polling");
                    break;
                }
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::{
        cap_range_to, classify_fork, next_scan_depth, next_target, resolve_trigger_mode, ForkScan,
        TriggerMode,
    };

    fn h(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn no_checkpoint_starts_at_safe_tip() {
        // 백필 방지: 체크포인트 없으면 tip(head-conf)부터
        assert_eq!(next_target(100, 12, None), Some((88, 88)));
    }

    #[test]
    fn resumes_from_checkpoint_plus_one() {
        assert_eq!(next_target(100, 12, Some(50)), Some((51, 88)));
    }

    #[test]
    fn nothing_new_when_caught_up() {
        // 체크포인트가 이미 safe head에 도달 → None
        assert_eq!(next_target(100, 12, Some(88)), None);
        assert_eq!(next_target(100, 12, Some(200)), None);
    }

    #[test]
    fn boundary_one_block_behind() {
        assert_eq!(next_target(100, 12, Some(87)), Some((88, 88)));
    }

    #[test]
    fn head_below_confirmations_saturates_to_zero() {
        assert_eq!(next_target(5, 12, None), Some((0, 0)));
        assert_eq!(next_target(5, 12, Some(0)), None);
    }

    #[test]
    fn classify_no_reorg_when_tip_matches() {
        let local = [(100u64, h("a100")), (99, h("a99"))];
        let chain = |n: u64| Some(format!("a{n}"));
        // tip 일치 → depth=1로 즉시 NoReorg (R2: 체인 조회 최소)
        assert_eq!(classify_fork(&local, chain, 1), ForkScan::NoReorg);
    }

    #[test]
    fn classify_fork_tip_only() {
        let local = [(100u64, h("old100")), (99, h("a99"))];
        let chain = |n: u64| {
            Some(if n == 100 {
                h("new100")
            } else {
                format!("a{n}")
            })
        };
        assert_eq!(classify_fork(&local, chain, 2), ForkScan::Fork(100));
    }

    #[test]
    fn classify_fork_deeper_returns_lowest_invalid() {
        // 100,99 reorged; 98 matches → fork = 99
        let local = [(100u64, h("o100")), (99, h("o99")), (98, h("a98"))];
        let chain = |n: u64| {
            Some(if n >= 99 {
                format!("new{n}")
            } else {
                format!("a{n}")
            })
        };
        assert_eq!(classify_fork(&local, chain, 3), ForkScan::Fork(99));
    }

    #[test]
    fn classify_need_deeper_then_floor_when_all_mismatch() {
        // 본 깊이 전부 불일치 + 더 깊은 로컬 존재 → 확대 요청 (R1)
        let local = [(100u64, h("o100")), (99, h("o99")), (98, h("o98"))];
        let chain = |n: u64| Some(format!("new{n}"));
        assert_eq!(classify_fork(&local, &chain, 2), ForkScan::NeedDeeper);
        // 전부 봤는데(더 깊은 로컬 없음) 다 불일치 → best-effort 최저 floor
        assert_eq!(classify_fork(&local, &chain, 3), ForkScan::Fork(98));
    }

    #[test]
    fn classify_inconclusive_is_safe() {
        let local = [(100u64, h("o100")), (99, h("o99"))];
        // 체인 조회 불가 → 절대 롤백 단정 금지(안전 규칙 불변)
        assert_eq!(classify_fork(&local, |_| None, 2), ForkScan::Inconclusive);
    }

    #[test]
    fn classify_empty_local_is_no_reorg() {
        assert_eq!(classify_fork(&[], |_| Some(h("x")), 8), ForkScan::NoReorg);
    }

    #[test]
    fn cap_range_to_smaller_than_cap_returns_full() {
        assert_eq!(cap_range_to(100, 200, 500), (100, 200));
    }

    #[test]
    fn cap_range_to_equal_cap_returns_full() {
        // 정확히 cap(500블록)일 때는 자르지 않음
        assert_eq!(cap_range_to(100, 599, 500), (100, 599));
    }

    #[test]
    fn cap_range_to_larger_than_cap_truncates() {
        assert_eq!(cap_range_to(100, 5000, 500), (100, 599));
    }

    #[test]
    fn cap_range_to_single_block() {
        assert_eq!(cap_range_to(100, 100, 500), (100, 100));
    }

    #[test]
    fn cap_range_to_cap_zero_is_noop() {
        // cap_blocks == 0 → 자르지 않음(테스트/특수 호출자용 탈출구)
        assert_eq!(cap_range_to(100, 5000, 0), (100, 5000));
    }

    #[test]
    fn cap_range_to_saturating_near_u64_max() {
        // tip 부근 산술 안전성 — saturating로 오버플로 방지
        let from = u64::MAX - 2;
        let to = u64::MAX;
        assert_eq!(cap_range_to(from, to, 500), (from, to));
    }

    #[test]
    fn next_scan_depth_widens_multiplicatively_and_terminates() {
        // ×4 전진, max 클램프, 항상 증가 → 종료성
        assert_eq!(next_scan_depth(1, 4096), 4);
        assert_eq!(next_scan_depth(4, 4096), 16);
        assert_eq!(next_scan_depth(16, 4096), 64);
        assert_eq!(next_scan_depth(1024, 4096), 4096);
        assert_eq!(next_scan_depth(2000, 4096), 4096); // 8000 → 클램프
        assert_eq!(next_scan_depth(4096, 4096), 4096); // 이미 max
        assert_eq!(next_scan_depth(3, 4), 4); // 경계: 항상 전진
        assert_eq!(next_scan_depth(1, 1), 1);
    }

    #[test]
    fn trigger_polling_when_not_requested() {
        // 구독 미요청이면 WS_URL이 있어도 폴링
        assert_eq!(
            resolve_trigger_mode(false, Some("wss://eth")),
            TriggerMode::Polling
        );
    }

    #[test]
    fn trigger_polling_fallback_when_no_ws() {
        // 구독 요청했으나 WS 없음 → 폴백(무회귀)
        assert_eq!(resolve_trigger_mode(true, None), TriggerMode::Polling);
    }

    #[test]
    fn trigger_polling_when_ws_blank() {
        // 공백/whitespace는 미지정과 동치
        assert_eq!(resolve_trigger_mode(true, Some("")), TriggerMode::Polling);
        assert_eq!(
            resolve_trigger_mode(true, Some("   ")),
            TriggerMode::Polling
        );
    }

    #[test]
    fn trigger_subscribe_when_requested_and_ws_present() {
        assert_eq!(
            resolve_trigger_mode(true, Some("wss://eth")),
            TriggerMode::Subscribe("wss://eth".to_string())
        );
    }

    #[test]
    fn trigger_subscribe_trims_url() {
        assert_eq!(
            resolve_trigger_mode(true, Some("  wss://eth  ")),
            TriggerMode::Subscribe("wss://eth".to_string())
        );
    }

    #[test]
    fn trigger_label_never_leaks_url() {
        // 라벨은 모드만 — WS URL(시크릿 가능) 비노출
        assert_eq!(TriggerMode::Polling.label(), "polling");
        assert_eq!(
            TriggerMode::Subscribe("wss://secret-key@host".to_string()).label(),
            "subscribe"
        );
    }
}
