use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::error::DbError;
use crate::models::{
    Block, DailySwapVolume, ErrorCategory, FailedTransaction, FailedTxAnalysis, FailedTxTrendPoint,
    LiquidityEvent, LiquidityEventType, Pool, PoolStats, PriceSnapshot, SnapshotInterval,
    SwapEvent, TimeBucket, Token, TokenTransfer, TopTrader, TraceLog, Transaction, UserProfile,
};

/// PostgreSQL enum 값으로 변환하는 헬퍼.
fn liquidity_type_to_sql(t: &LiquidityEventType) -> &'static str {
    match t {
        LiquidityEventType::Mint => "MINT",
        LiquidityEventType::Burn => "BURN",
    }
}

/// PostgreSQL enum 값으로 변환하는 헬퍼.
fn error_category_to_sql(c: &ErrorCategory) -> &'static str {
    match c {
        ErrorCategory::InsufficientBalance => "INSUFFICIENT_BALANCE",
        ErrorCategory::SlippageExceeded => "SLIPPAGE_EXCEEDED",
        ErrorCategory::DeadlineExpired => "DEADLINE_EXPIRED",
        ErrorCategory::Unauthorized => "UNAUTHORIZED",
        ErrorCategory::TransferFailed => "TRANSFER_FAILED",
        ErrorCategory::Unknown => "UNKNOWN",
    }
}

/// PostgreSQL enum 값으로 변환하는 헬퍼.
fn snapshot_interval_to_sql(i: &SnapshotInterval) -> &'static str {
    match i {
        SnapshotInterval::OneMinute => "1m",
        SnapshotInterval::FiveMinutes => "5m",
        SnapshotInterval::FifteenMinutes => "15m",
        SnapshotInterval::OneHour => "1h",
        SnapshotInterval::FourHours => "4h",
        SnapshotInterval::OneDay => "1d",
    }
}

/// 블록을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, blocks))]
pub async fn insert_blocks(pool: &PgPool, blocks: &[Block]) -> Result<u64, DbError> {
    if blocks.is_empty() {
        return Ok(0);
    }

    let block_numbers: Vec<i64> = blocks.iter().map(|b| b.block_number).collect();
    let timestamps: Vec<DateTime<Utc>> = blocks.iter().map(|b| b.timestamp).collect();
    let gas_useds: Vec<i64> = blocks.iter().map(|b| b.gas_used).collect();
    let block_hashes: Vec<Option<&str>> = blocks.iter().map(|b| b.block_hash.as_deref()).collect();
    let parent_hashes: Vec<Option<&str>> =
        blocks.iter().map(|b| b.parent_hash.as_deref()).collect();

    let result = sqlx::query(
        "INSERT INTO block (block_number, timestamp, gas_used, block_hash, parent_hash)
         SELECT * FROM UNNEST($1::BIGINT[], $2::TIMESTAMPTZ[], $3::BIGINT[], $4::TEXT[], $5::TEXT[])
         ON CONFLICT (block_number) DO NOTHING",
    )
    .bind(&block_numbers)
    .bind(&timestamps)
    .bind(&gas_useds)
    .bind(&block_hashes)
    .bind(&parent_hashes)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 트랜잭션을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, transactions))]
pub async fn insert_transactions(
    pool: &PgPool,
    transactions: &[Transaction],
) -> Result<u64, DbError> {
    if transactions.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = transactions.iter().map(|t| t.tx_hash.as_str()).collect();
    let from_addrs: Vec<&str> = transactions.iter().map(|t| t.from_addr.as_str()).collect();
    let to_addrs: Vec<Option<&str>> = transactions.iter().map(|t| t.to_addr.as_deref()).collect();
    let block_numbers: Vec<i64> = transactions.iter().map(|t| t.block_number).collect();
    let gas_useds: Vec<i64> = transactions.iter().map(|t| t.gas_used).collect();
    let gas_prices: Vec<&bigdecimal::BigDecimal> =
        transactions.iter().map(|t| &t.gas_price).collect();
    let values: Vec<&bigdecimal::BigDecimal> = transactions.iter().map(|t| &t.value).collect();
    let statuses: Vec<i16> = transactions.iter().map(|t| t.status).collect();
    let input_datas: Vec<Option<&str>> = transactions
        .iter()
        .map(|t| t.input_data.as_deref())
        .collect();

    let result = sqlx::query(
        "INSERT INTO transaction (tx_hash, from_addr, to_addr, block_number, gas_used, gas_price, value, status, input_data)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::BIGINT[], $5::BIGINT[], $6::NUMERIC[], $7::NUMERIC[], $8::SMALLINT[], $9::TEXT[])
         ON CONFLICT (tx_hash) DO NOTHING",
    )
    .bind(&tx_hashes)
    .bind(&from_addrs)
    .bind(&to_addrs)
    .bind(&block_numbers)
    .bind(&gas_useds)
    .bind(&gas_prices)
    .bind(&values)
    .bind(&statuses)
    .bind(&input_datas)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 토큰 메타데이터를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, tokens))]
pub async fn insert_tokens(pool: &PgPool, tokens: &[Token]) -> Result<u64, DbError> {
    if tokens.is_empty() {
        return Ok(0);
    }

    let addresses: Vec<&str> = tokens.iter().map(|t| t.token_address.as_str()).collect();
    let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
    let names: Vec<&str> = tokens.iter().map(|t| t.name.as_str()).collect();
    let decimals: Vec<i16> = tokens.iter().map(|t| t.decimals).collect();

    let result = sqlx::query(
        "INSERT INTO token (token_address, symbol, name, decimals)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::SMALLINT[])
         ON CONFLICT (token_address) DO NOTHING",
    )
    .bind(&addresses)
    .bind(&symbols)
    .bind(&names)
    .bind(&decimals)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 풀 정보를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool_conn, pools))]
pub async fn insert_pools(pool_conn: &PgPool, pools: &[Pool]) -> Result<u64, DbError> {
    if pools.is_empty() {
        return Ok(0);
    }

    let addresses: Vec<&str> = pools.iter().map(|p| p.pool_address.as_str()).collect();
    let pair_names: Vec<&str> = pools.iter().map(|p| p.pair_name.as_str()).collect();
    let token0s: Vec<&str> = pools.iter().map(|p| p.token0_address.as_str()).collect();
    let token1s: Vec<&str> = pools.iter().map(|p| p.token1_address.as_str()).collect();
    let fee_tiers: Vec<i32> = pools.iter().map(|p| p.fee_tier).collect();
    let created_ats: Vec<DateTime<Utc>> = pools.iter().map(|p| p.created_at).collect();

    let result = sqlx::query(
        "INSERT INTO pool (pool_address, pair_name, token0_address, token1_address, fee_tier, created_at)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::INT[], $6::TIMESTAMPTZ[])
         ON CONFLICT (pool_address) DO NOTHING",
    )
    .bind(&addresses)
    .bind(&pair_names)
    .bind(&token0s)
    .bind(&token1s)
    .bind(&fee_tiers)
    .bind(&created_ats)
    .execute(pool_conn)
    .await?;

    Ok(result.rows_affected())
}

/// 스왑 이벤트를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, events))]
pub async fn insert_swap_events(pool: &PgPool, events: &[SwapEvent]) -> Result<u64, DbError> {
    if events.is_empty() {
        return Ok(0);
    }

    let pool_addresses: Vec<&str> = events.iter().map(|e| e.pool_address.as_str()).collect();
    let tx_hashes: Vec<&str> = events.iter().map(|e| e.tx_hash.as_str()).collect();
    let senders: Vec<&str> = events.iter().map(|e| e.sender.as_str()).collect();
    let recipients: Vec<&str> = events.iter().map(|e| e.recipient.as_str()).collect();
    let amount0s: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount0).collect();
    let amount1s: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount1).collect();
    let amount_ins: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount_in).collect();
    let amount_outs: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount_out).collect();
    let sqrt_prices: Vec<&bigdecimal::BigDecimal> =
        events.iter().map(|e| &e.sqrt_price_x96).collect();
    let liquidities: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.liquidity).collect();
    let ticks: Vec<i32> = events.iter().map(|e| e.tick).collect();
    let log_indices: Vec<i32> = events.iter().map(|e| e.log_index).collect();
    let timestamps: Vec<DateTime<Utc>> = events.iter().map(|e| e.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO swap_event (pool_address, tx_hash, sender, recipient, amount0, amount1, amount_in, amount_out, sqrt_price_x96, liquidity, tick, log_index, timestamp)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::NUMERIC[], $6::NUMERIC[], $7::NUMERIC[], $8::NUMERIC[], $9::NUMERIC[], $10::NUMERIC[], $11::INT[], $12::INT[], $13::TIMESTAMPTZ[])
         ON CONFLICT (tx_hash, log_index) DO NOTHING",
    )
    .bind(&pool_addresses)
    .bind(&tx_hashes)
    .bind(&senders)
    .bind(&recipients)
    .bind(&amount0s)
    .bind(&amount1s)
    .bind(&amount_ins)
    .bind(&amount_outs)
    .bind(&sqrt_prices)
    .bind(&liquidities)
    .bind(&ticks)
    .bind(&log_indices)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 유동성 이벤트를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, events))]
pub async fn insert_liquidity_events(
    pool: &PgPool,
    events: &[LiquidityEvent],
) -> Result<u64, DbError> {
    if events.is_empty() {
        return Ok(0);
    }

    let event_types: Vec<&str> = events
        .iter()
        .map(|e| liquidity_type_to_sql(&e.event_type))
        .collect();
    let pool_addresses: Vec<&str> = events.iter().map(|e| e.pool_address.as_str()).collect();
    let tx_hashes: Vec<&str> = events.iter().map(|e| e.tx_hash.as_str()).collect();
    let providers: Vec<&str> = events.iter().map(|e| e.provider.as_str()).collect();
    let token0_amounts: Vec<&bigdecimal::BigDecimal> =
        events.iter().map(|e| &e.token0_amount).collect();
    let token1_amounts: Vec<&bigdecimal::BigDecimal> =
        events.iter().map(|e| &e.token1_amount).collect();
    let tick_lowers: Vec<i32> = events.iter().map(|e| e.tick_lower).collect();
    let tick_uppers: Vec<i32> = events.iter().map(|e| e.tick_upper).collect();
    let liquidities: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.liquidity).collect();
    let log_indices: Vec<i32> = events.iter().map(|e| e.log_index).collect();
    let timestamps: Vec<DateTime<Utc>> = events.iter().map(|e| e.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO liquidity_event (event_type, pool_address, tx_hash, provider, token0_amount, token1_amount, tick_lower, tick_upper, liquidity, log_index, timestamp)
         SELECT t.event_type::liquidity_event_type, t.pool_address, t.tx_hash, t.provider, t.token0_amount, t.token1_amount, t.tick_lower, t.tick_upper, t.liquidity, t.log_index, t.ts
         FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::NUMERIC[], $6::NUMERIC[], $7::INT[], $8::INT[], $9::NUMERIC[], $10::INT[], $11::TIMESTAMPTZ[])
         AS t(event_type, pool_address, tx_hash, provider, token0_amount, token1_amount, tick_lower, tick_upper, liquidity, log_index, ts)
         ON CONFLICT (tx_hash, log_index) DO NOTHING",
    )
    .bind(&event_types)
    .bind(&pool_addresses)
    .bind(&tx_hashes)
    .bind(&providers)
    .bind(&token0_amounts)
    .bind(&token1_amounts)
    .bind(&tick_lowers)
    .bind(&tick_uppers)
    .bind(&liquidities)
    .bind(&log_indices)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 토큰 전송을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, transfers))]
pub async fn insert_token_transfers(
    pool: &PgPool,
    transfers: &[TokenTransfer],
) -> Result<u64, DbError> {
    if transfers.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = transfers.iter().map(|t| t.tx_hash.as_str()).collect();
    let token_addresses: Vec<&str> = transfers.iter().map(|t| t.token_address.as_str()).collect();
    let from_addrs: Vec<&str> = transfers.iter().map(|t| t.from_addr.as_str()).collect();
    let to_addrs: Vec<&str> = transfers.iter().map(|t| t.to_addr.as_str()).collect();
    let amounts: Vec<&bigdecimal::BigDecimal> = transfers.iter().map(|t| &t.amount).collect();
    let log_indices: Vec<i32> = transfers.iter().map(|t| t.log_index).collect();
    let timestamps: Vec<DateTime<Utc>> = transfers.iter().map(|t| t.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO token_transfer (tx_hash, token_address, from_addr, to_addr, amount, log_index, timestamp)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::NUMERIC[], $6::INT[], $7::TIMESTAMPTZ[])
         ON CONFLICT (tx_hash, log_index) DO NOTHING",
    )
    .bind(&tx_hashes)
    .bind(&token_addresses)
    .bind(&from_addrs)
    .bind(&to_addrs)
    .bind(&amounts)
    .bind(&log_indices)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 실패한 트랜잭션을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, failed))]
pub async fn insert_failed_transactions(
    pool: &PgPool,
    failed: &[FailedTransaction],
) -> Result<u64, DbError> {
    if failed.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = failed.iter().map(|f| f.tx_hash.as_str()).collect();
    let categories: Vec<&str> = failed
        .iter()
        .map(|f| error_category_to_sql(&f.error_category))
        .collect();
    let revert_reasons: Vec<Option<&str>> =
        failed.iter().map(|f| f.revert_reason.as_deref()).collect();
    let failing_fns: Vec<Option<&str>> = failed
        .iter()
        .map(|f| f.failing_function.as_deref())
        .collect();
    let gas_useds: Vec<i64> = failed.iter().map(|f| f.gas_used).collect();
    let timestamps: Vec<DateTime<Utc>> = failed.iter().map(|f| f.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO failed_transaction (tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp)
         SELECT t.tx_hash, t.cat::error_category, t.revert_reason, t.failing_fn, t.gas_used, t.ts
         FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::BIGINT[], $6::TIMESTAMPTZ[])
         AS t(tx_hash, cat, revert_reason, failing_fn, gas_used, ts)
         ON CONFLICT (tx_hash) DO NOTHING",
    )
    .bind(&tx_hashes)
    .bind(&categories)
    .bind(&revert_reasons)
    .bind(&failing_fns)
    .bind(&gas_useds)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 가격 스냅샷을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, snapshots))]
pub async fn insert_price_snapshots(
    pool: &PgPool,
    snapshots: &[PriceSnapshot],
) -> Result<u64, DbError> {
    if snapshots.is_empty() {
        return Ok(0);
    }

    let pool_addresses: Vec<&str> = snapshots.iter().map(|s| s.pool_address.as_str()).collect();
    let prices: Vec<&bigdecimal::BigDecimal> = snapshots.iter().map(|s| &s.price).collect();
    let ticks: Vec<i32> = snapshots.iter().map(|s| s.tick).collect();
    let liquidities: Vec<&bigdecimal::BigDecimal> =
        snapshots.iter().map(|s| &s.liquidity).collect();
    let snapshot_tss: Vec<DateTime<Utc>> = snapshots.iter().map(|s| s.snapshot_ts).collect();
    let intervals: Vec<&str> = snapshots
        .iter()
        .map(|s| snapshot_interval_to_sql(&s.interval_type))
        .collect();

    let result = sqlx::query(
        "INSERT INTO price_snapshot (pool_address, price, tick, liquidity, snapshot_ts, interval_type)
         SELECT t.pool_address, t.price, t.tick, t.liquidity, t.snapshot_ts, t.interval::snapshot_interval
         FROM UNNEST($1::TEXT[], $2::NUMERIC[], $3::INT[], $4::NUMERIC[], $5::TIMESTAMPTZ[], $6::TEXT[])
         AS t(pool_address, price, tick, liquidity, snapshot_ts, interval)
         ON CONFLICT (pool_address, snapshot_ts, interval_type) DO NOTHING",
    )
    .bind(&pool_addresses)
    .bind(&prices)
    .bind(&ticks)
    .bind(&liquidities)
    .bind(&snapshot_tss)
    .bind(&intervals)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 유저 프로필을 UNNEST 배치 UPSERT한다.
#[tracing::instrument(skip(pool, profiles))]
pub async fn upsert_user_profiles(pool: &PgPool, profiles: &[UserProfile]) -> Result<u64, DbError> {
    if profiles.is_empty() {
        return Ok(0);
    }

    let addresses: Vec<&str> = profiles.iter().map(|u| u.user_address.as_str()).collect();
    let labels: Vec<Option<&str>> = profiles.iter().map(|u| u.label.as_deref()).collect();
    let first_seens: Vec<DateTime<Utc>> = profiles.iter().map(|u| u.first_seen).collect();
    let last_seens: Vec<DateTime<Utc>> = profiles.iter().map(|u| u.last_seen).collect();
    let total_swaps: Vec<i32> = profiles.iter().map(|u| u.total_swaps).collect();
    let total_volumes: Vec<&bigdecimal::BigDecimal> =
        profiles.iter().map(|u| &u.total_volume_usd).collect();

    let result = sqlx::query(
        "INSERT INTO user_profile (user_address, label, first_seen, last_seen, total_swaps, total_volume_usd)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TIMESTAMPTZ[], $4::TIMESTAMPTZ[], $5::INT[], $6::NUMERIC[])
         ON CONFLICT (user_address) DO UPDATE SET
             last_seen = EXCLUDED.last_seen,
             total_swaps = user_profile.total_swaps + EXCLUDED.total_swaps,
             total_volume_usd = user_profile.total_volume_usd + EXCLUDED.total_volume_usd",
    )
    .bind(&addresses)
    .bind(&labels)
    .bind(&first_seens)
    .bind(&last_seens)
    .bind(&total_swaps)
    .bind(&total_volumes)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 트레이스 로그를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, traces))]
pub async fn insert_trace_logs(pool: &PgPool, traces: &[TraceLog]) -> Result<u64, DbError> {
    if traces.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = traces.iter().map(|t| t.tx_hash.as_str()).collect();
    let call_depths: Vec<i32> = traces.iter().map(|t| t.call_depth).collect();
    let call_types: Vec<&str> = traces.iter().map(|t| t.call_type.as_str()).collect();
    let from_addrs: Vec<&str> = traces.iter().map(|t| t.from_addr.as_str()).collect();
    let to_addrs: Vec<Option<&str>> = traces.iter().map(|t| t.to_addr.as_deref()).collect();
    let values: Vec<&bigdecimal::BigDecimal> = traces.iter().map(|t| &t.value).collect();
    let gas_useds: Vec<i64> = traces.iter().map(|t| t.gas_used).collect();
    let inputs: Vec<Option<&str>> = traces.iter().map(|t| t.input.as_deref()).collect();
    let outputs: Vec<Option<&str>> = traces.iter().map(|t| t.output.as_deref()).collect();
    let errors: Vec<Option<&str>> = traces.iter().map(|t| t.error.as_deref()).collect();

    let result = sqlx::query(
        "INSERT INTO trace_log (tx_hash, call_depth, call_type, from_addr, to_addr, value, gas_used, input, output, error)
         SELECT * FROM UNNEST($1::TEXT[], $2::INT[], $3::TEXT[], $4::TEXT[], $5::TEXT[], $6::NUMERIC[], $7::BIGINT[], $8::TEXT[], $9::TEXT[], $10::TEXT[])",
    )
    .bind(&tx_hashes)
    .bind(&call_depths)
    .bind(&call_types)
    .bind(&from_addrs)
    .bind(&to_addrs)
    .bind(&values)
    .bind(&gas_useds)
    .bind(&inputs)
    .bind(&outputs)
    .bind(&errors)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

// ============================================
// API용 읽기 쿼리
// ============================================

/// 풀 목록을 페이지네이션하여 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_pools(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<Pool>, DbError> {
    let pools =
        sqlx::query_as::<_, Pool>("SELECT * FROM pool ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
    Ok(pools)
}

/// 주소로 단일 풀을 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_pool_by_address(pool: &PgPool, address: &str) -> Result<Pool, DbError> {
    sqlx::query_as::<_, Pool>("SELECT * FROM pool WHERE pool_address = $1")
        .bind(address)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("pool {address}")))
}

/// 토큰 목록을 페이지네이션하여 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_tokens(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<Token>, DbError> {
    let tokens =
        sqlx::query_as::<_, Token>("SELECT * FROM token ORDER BY symbol LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
    Ok(tokens)
}

/// 주소로 단일 토큰을 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_token_by_address(pool: &PgPool, address: &str) -> Result<Token, DbError> {
    sqlx::query_as::<_, Token>("SELECT * FROM token WHERE token_address = $1")
        .bind(address)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("token {address}")))
}

/// 스왑 이벤트를 풀 필터 + 페이지네이션으로 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_swap_events(
    pool: &PgPool,
    pool_address: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SwapEvent>, DbError> {
    let events = sqlx::query_as::<_, SwapEvent>(
        "SELECT * FROM swap_event
         WHERE ($1::TEXT IS NULL OR pool_address = $1)
         ORDER BY timestamp DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(pool_address)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(events)
}

/// 일별 스왑 볼륨을 조회한다 (vw_daily_swap_volume).
#[tracing::instrument(skip(pool))]
pub async fn get_daily_swap_volume(
    pool: &PgPool,
    pool_address: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<DailySwapVolume>, DbError> {
    let rows = sqlx::query_as::<_, DailySwapVolume>(
        "SELECT * FROM vw_daily_swap_volume
         WHERE ($1::TEXT IS NULL OR pool_address = $1)
         ORDER BY swap_date DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(pool_address)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 트레이더 랭킹을 조회한다 (vw_top_traders).
#[tracing::instrument(skip(pool))]
pub async fn get_top_traders(pool: &PgPool, limit: i64) -> Result<Vec<TopTrader>, DbError> {
    let traders = sqlx::query_as::<_, TopTrader>("SELECT * FROM vw_top_traders LIMIT $1")
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(traders)
}

/// 실패 TX 카테고리별 분석을 조회한다 (vw_failed_tx_analysis).
#[tracing::instrument(skip(pool))]
pub async fn get_failed_tx_analysis(pool: &PgPool) -> Result<Vec<FailedTxAnalysis>, DbError> {
    let rows = sqlx::query_as::<_, FailedTxAnalysis>("SELECT * FROM vw_failed_tx_analysis")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// tx 해시로 단건 실패 트랜잭션을 조회한다.
///
/// 해당 해시의 실패 기록이 없으면 [`DbError::NotFound`]를 반환한다.
#[tracing::instrument(skip(pool))]
pub async fn get_failed_transaction(
    pool: &PgPool,
    tx_hash: &str,
) -> Result<FailedTransaction, DbError> {
    sqlx::query_as::<_, FailedTransaction>(
        "SELECT tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp
         FROM failed_transaction WHERE tx_hash = $1",
    )
    .bind(tx_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound(format!("failed transaction {tx_hash}")))
}

/// tx 해시의 평탄화된 콜 트레이스를 호출 순서(pre-order DFS)대로, 최대 `limit`개 조회한다.
///
/// `trace_id`(BIGSERIAL)는 인덱서가 콜트리를 pre-order로 평탄화하며 삽입한
/// 순서를 그대로 보존하므로, `trace_id ASC` = 올바른 트리 선형순서다.
/// `call_depth`로 먼저 정렬하면 형제 서브트리가 섞여 트리 복원이 불가능하다.
/// 호출자는 잘림 감지를 위해 `limit = 상한 + 1`로 조회할 수 있다.
/// 트레이스가 없으면 빈 `Vec`을 반환한다(에러 아님).
#[tracing::instrument(skip(pool))]
pub async fn list_trace_logs_by_tx(
    pool: &PgPool,
    tx_hash: &str,
    limit: i64,
) -> Result<Vec<TraceLog>, DbError> {
    let logs = sqlx::query_as::<_, TraceLog>(
        "SELECT tx_hash, call_depth, call_type, from_addr, to_addr, value,
                gas_used, input, output, error, trace_id
         FROM trace_log WHERE tx_hash = $1
         ORDER BY trace_id ASC
         LIMIT $2",
    )
    .bind(tx_hash)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(logs)
}

/// 실패 트랜잭션을 필터·페이지네이션하여 조회한다.
///
/// `category`/`from`/`to`는 모두 선택 — `None`이면 해당 필터를 적용하지 않는다
/// (단일 prepared statement, 동적 SQL 미사용). `timestamp DESC` 정렬.
/// 전체 건수는 [`count_failed_transactions`]로 별도 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_failed_transactions(
    pool: &PgPool,
    category: Option<&ErrorCategory>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> Result<Vec<FailedTransaction>, DbError> {
    let cat = category.map(error_category_to_sql);
    let rows = sqlx::query_as::<_, FailedTransaction>(
        "SELECT tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp
         FROM failed_transaction
         WHERE ($1::TEXT IS NULL OR error_category = $1::error_category)
           AND ($2::TIMESTAMPTZ IS NULL OR timestamp >= $2)
           AND ($3::TIMESTAMPTZ IS NULL OR timestamp <= $3)
         ORDER BY timestamp DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(cat)
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// [`list_failed_transactions`]와 동일 필터의 전체 건수를 반환한다.
///
/// 페이지네이션 `total` 메타데이터 산출용 — `LIMIT`/`OFFSET`과 무관하다.
#[tracing::instrument(skip(pool))]
pub async fn count_failed_transactions(
    pool: &PgPool,
    category: Option<&ErrorCategory>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<i64, DbError> {
    let cat = category.map(error_category_to_sql);
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM failed_transaction
         WHERE ($1::TEXT IS NULL OR error_category = $1::error_category)
           AND ($2::TIMESTAMPTZ IS NULL OR timestamp >= $2)
           AND ($3::TIMESTAMPTZ IS NULL OR timestamp <= $3)",
    )
    .bind(cat)
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(total)
}

/// 실패 트랜잭션을 시간 버킷 × 카테고리로 집계한다.
///
/// `bucket`은 화이트리스트 [`TimeBucket`]만 받아 `date_trunc($1, timestamp)`에
/// **바인딩 파라미터**로 전달한다 (문자열 보간 금지 — SQL 인젝션 방지).
/// `from`/`to`는 선택. `bucket ASC, error_category ASC` 정렬.
#[tracing::instrument(skip(pool))]
pub async fn failed_tx_timeseries(
    pool: &PgPool,
    bucket: &TimeBucket,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<Vec<FailedTxTrendPoint>, DbError> {
    let rows = sqlx::query_as::<_, FailedTxTrendPoint>(
        "SELECT date_trunc($1, timestamp) AS bucket,
                error_category,
                COUNT(*) AS failure_count
         FROM failed_transaction
         WHERE ($2::TIMESTAMPTZ IS NULL OR timestamp >= $2)
           AND ($3::TIMESTAMPTZ IS NULL OR timestamp <= $3)
         GROUP BY 1, 2
         ORDER BY 1 ASC, 2 ASC",
    )
    .bind(bucket.as_pg())
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 풀 종합 통계를 조회한다 (fn_get_pool_stats).
#[tracing::instrument(skip(pool))]
pub async fn get_pool_stats(
    pool: &PgPool,
    pool_address: &str,
    from_date: DateTime<Utc>,
    to_date: DateTime<Utc>,
) -> Result<PoolStats, DbError> {
    sqlx::query_as::<_, PoolStats>("SELECT * FROM fn_get_pool_stats($1, $2, $3)")
        .bind(pool_address)
        .bind(from_date)
        .bind(to_date)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("pool stats for {pool_address}")))
}

// ============================================
// 인덱서용 체크포인트 쿼리
// ============================================

/// 특정 체인의 마지막 체크포인트를 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_last_checkpoint(pool: &PgPool, chain_id: i32) -> Result<Option<i64>, DbError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT last_processed_block FROM indexer_checkpoint WHERE chain_id = $1")
            .bind(chain_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// 체크포인트를 갱신한다 (없으면 INSERT, 있으면 UPDATE).
#[tracing::instrument(skip(pool))]
pub async fn update_checkpoint(
    pool: &PgPool,
    chain_id: i32,
    last_processed_block: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO indexer_checkpoint (chain_id, last_processed_block, updated_at)
         VALUES ($1, $2, NOW())
         ON CONFLICT (chain_id) DO UPDATE
         SET last_processed_block = EXCLUDED.last_processed_block,
             updated_at = NOW()",
    )
    .bind(chain_id)
    .bind(last_processed_block)
    .execute(pool)
    .await?;
    Ok(())
}

/// 블록 번호로 블록을 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_block_by_number(pool: &PgPool, block_number: i64) -> Result<Block, DbError> {
    sqlx::query_as::<_, Block>("SELECT * FROM block WHERE block_number = $1")
        .bind(block_number)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("block {block_number}")))
}

/// DB에 저장된 가장 최근 블록 번호를 반환한다.
#[tracing::instrument(skip(pool))]
pub async fn get_latest_block_number(pool: &PgPool) -> Result<Option<i64>, DbError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT MAX(block_number) FROM block")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

/// 최근 인덱싱된 블록의 `(번호, 해시)`를 번호 내림차순으로 최대 `limit`개 조회한다.
///
/// 해시가 없는 행(S06 이전 인덱싱)은 비교 불가이므로 제외한다.
/// reorg 감지(`find_fork_point`)의 로컬 입력용.
#[tracing::instrument(skip(pool))]
pub async fn recent_block_hashes(pool: &PgPool, limit: i64) -> Result<Vec<(i64, String)>, DbError> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT block_number, block_hash
         FROM block
         WHERE block_hash IS NOT NULL
         ORDER BY block_number DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// reorg 정정: `from_block`부터(포함) 인덱싱 데이터를 모두 삭제하고 체크포인트를
/// 되감는다. 단일 트랜잭션·멱등.
///
/// tx_hash FK가 `ON DELETE CASCADE`가 아니므로 의존 행 → `transaction` → `block`
/// 순서로 삭제한다. `price_snapshot`/`user_profile`은 블록 스코프가 아니라 제외.
/// 재인덱싱은 하지 않는다(follow가 체크포인트에서 재개). 삭제된 block 수를 반환.
#[tracing::instrument(skip(pool))]
pub async fn rollback_from_block(pool: &PgPool, from_block: i64) -> Result<u64, DbError> {
    let resume = (from_block - 1).max(0);
    let mut tx = pool.begin().await?;

    // 1) tx_hash 의존 행 (transaction 삭제 전 — FK RESTRICT)
    sqlx::query(
        "DELETE FROM swap_event
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM liquidity_event
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM token_transfer
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM trace_log
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM failed_transaction
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;

    // 2) transaction → block
    sqlx::query("DELETE FROM transaction WHERE block_number >= $1")
        .bind(from_block)
        .execute(&mut *tx)
        .await?;
    let blocks = sqlx::query("DELETE FROM block WHERE block_number >= $1")
        .bind(from_block)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    // 3) 체크포인트 되감기 (follow가 from_block부터 재인덱싱)
    sqlx::query(
        "UPDATE indexer_checkpoint
         SET last_processed_block = $1, updated_at = NOW()
         WHERE chain_id = 1",
    )
    .bind(resume)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(blocks)
}

/// 특정 블록의 스왑 이벤트를 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_swap_events_by_block(
    pool: &PgPool,
    block_number: i64,
) -> Result<Vec<SwapEvent>, DbError> {
    let events = sqlx::query_as::<_, SwapEvent>(
        "SELECT se.* FROM swap_event se
         JOIN transaction t ON se.tx_hash = t.tx_hash
         WHERE t.block_number = $1
         ORDER BY se.event_id",
    )
    .bind(block_number)
    .fetch_all(pool)
    .await?;
    Ok(events)
}
