use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use alloy::consensus::Transaction as ConsensusTx;
use alloy::eips::BlockNumberOrTag;
use alloy::network::TransactionResponse;
use alloy::providers::ext::DebugApi;
use alloy::providers::{Provider, ProviderBuilder};
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
    /// 블록 범위를 `batch_size` 단위 청크로 분할하고,
    /// `tokio::JoinSet`으로 병렬 수집한다.
    #[tracing::instrument(skip(self))]
    pub async fn index_range(&self, from_block: u64, to_block: u64) -> anyhow::Result<()> {
        tracing::info!(from_block, to_block, "starting block range indexing");

        let total = to_block.saturating_sub(from_block) + 1;
        let mut processed = 0u64;

        for chunk_start in (from_block..=to_block).step_by(self.batch_size) {
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
    pub async fn follow(&self, confirmations: u64, poll: Duration) -> anyhow::Result<()> {
        let provider = ProviderBuilder::new().connect_http(
            self.rpc_url
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid RPC URL: {e}"))?,
        );
        tracing::info!(
            confirmations,
            poll_secs = poll.as_secs(),
            "follow mode started (Ctrl-C to stop)"
        );

        let scan_depth = (confirmations as i64).max(64);

        loop {
            // reorg 체크가 우선 — fork면 롤백 후 재인덱싱(되감긴 checkpoint).
            // 불확실(RPC 실패/블록 부재)이면 detect_fork가 None → 무작동(안전).
            if let Some(fork) = self.detect_fork(&provider, scan_depth).await? {
                tracing::warn!(fork, "reorg detected — rolling back and re-indexing");
                db::queries::rollback_from_block(&self.db_pool, fork as i64).await?;
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
            match next_target(head, confirmations, checkpoint) {
                Some((from, to)) => {
                    tracing::info!(head, from, to, "following: indexing new range");
                    self.index_range(from, to).await?;
                }
                None => tracing::debug!(head, "following: no new confirmed blocks"),
            }

            tokio::select! {
                _ = tokio::time::sleep(poll) => {}
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Ctrl-C received — stopping follow loop");
                    break;
                }
            }
        }
        Ok(())
    }

    /// 최근 로컬 블록 해시를 체인과 대조해 reorg fork point를 찾는다.
    ///
    /// RPC 조회 불가/블록 부재(불확실)면 `None`을 반환한다 — 파괴적 롤백의
    /// false positive 방지(안전 규칙). 비동기 사전조회로 체인 해시 맵을 만든 뒤
    /// 순수 [`find_fork_point`]에 주입한다(테스트성·안전규칙 보존).
    async fn detect_fork(
        &self,
        provider: &impl Provider,
        scan_depth: i64,
    ) -> anyhow::Result<Option<u64>> {
        let local: Vec<(u64, String)> = db::queries::recent_block_hashes(&self.db_pool, scan_depth)
            .await?
            .into_iter()
            .map(|(n, h)| (n as u64, h))
            .collect();
        if local.is_empty() {
            return Ok(None);
        }

        let mut chain: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
        for (height, _) in &local {
            let h = *height;
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
                // 블록 부재(헤드 밖) 또는 RPC 실패 → 불확실: 이번 사이클 판단 보류
                Ok(None) | Err(_) => return Ok(None),
            }
        }

        Ok(find_fork_point(&local, |h| chain.get(&h).cloned()))
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

/// reorg fork point를 찾는 순수 함수.
///
/// `local`은 (블록번호, 로컬해시)를 **번호 내림차순(tip 먼저)** 으로, 연속 구간으로
/// 받는다. `chain_hash_at`는 현재 체인의 해당 높이 해시(`None`이면 조회 불가).
///
/// 반환 `Some(f)` = 높이 `f`부터(포함) 무효 → 롤백 floor. `None` = reorg 없음
/// **또는 판단 불가**. 핵심 안전 규칙: 체인 해시가 `None`이면(RPC 불확실) 절대
/// fork로 단정하지 않는다 — 파괴적 롤백의 false positive 방지.
pub fn find_fork_point(
    local: &[(u64, String)],
    chain_hash_at: impl Fn(u64) -> Option<String>,
) -> Option<u64> {
    let mut fork: Option<u64> = None;
    for (height, local_hash) in local {
        match chain_hash_at(*height) {
            // 불확실 → 파괴적 롤백을 거는 것보다 이번 사이클 스킵이 안전
            None => return None,
            Some(chain_hash) if &chain_hash == local_hash => {
                // 일치: 이 높이 이하는 정상(체인 연결성) → 탐색 종료
                return fork;
            }
            Some(_) => fork = Some(*height), // 불일치: 롤백 floor 후보(계속 하강)
        }
    }
    // 윈도우를 다 봐도 불일치 → reorg가 최소 이 깊이 이상(호출자가 윈도우 확대 가능)
    fork
}

#[cfg(test)]
mod tests {
    use super::{find_fork_point, next_target};

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
    fn fork_none_when_tip_matches() {
        let local = [(100, h("a100")), (99, h("a99"))];
        let chain = |n: u64| Some(format!("a{n}"));
        assert_eq!(find_fork_point(&local, chain), None);
    }

    #[test]
    fn fork_tip_only() {
        let local = [(100, h("old100")), (99, h("a99"))];
        let chain = |n: u64| {
            Some(if n == 100 {
                h("new100")
            } else {
                format!("a{n}")
            })
        };
        assert_eq!(find_fork_point(&local, chain), Some(100));
    }

    #[test]
    fn fork_deeper_returns_lowest_invalid() {
        // 100,99 reorged; 98 still matches → fork = 99
        let local = [(100, h("o100")), (99, h("o99")), (98, h("a98"))];
        let chain = |n: u64| {
            Some(if n >= 99 {
                format!("new{n}")
            } else {
                format!("a{n}")
            })
        };
        assert_eq!(find_fork_point(&local, chain), Some(99));
    }

    #[test]
    fn fork_whole_window_mismatch_returns_floor() {
        let local = [(100, h("o100")), (99, h("o99"))];
        let chain = |n: u64| Some(format!("new{n}"));
        assert_eq!(find_fork_point(&local, chain), Some(99));
    }

    #[test]
    fn fork_inconclusive_rpc_is_safe_none() {
        let local = [(100, h("o100")), (99, h("o99"))];
        // 체인 조회 불가 → 절대 롤백 단정 금지
        assert_eq!(find_fork_point(&local, |_| None), None);
    }

    #[test]
    fn fork_empty_local_is_none() {
        assert_eq!(find_fork_point(&[], |_| Some(h("x"))), None);
    }
}
