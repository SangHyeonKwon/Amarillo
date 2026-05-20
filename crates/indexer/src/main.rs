//! DeFi Analytics 인덱서 — Uniswap V3 이벤트 수집기.
//!
//! 이더리움 블록체인에서 Uniswap V3 이벤트를 수집·디코딩·저장한다.
//!
//! ## 사용법
//! ```bash
//! cargo run -p indexer -- --from-block 18000000 --to-block 18001000
//! cargo run -p indexer -- --follow            # 체인 헤드 추종 (S05)
//! ```

mod alerts;
mod config;
mod worker;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::worker::{resolve_trigger_mode, WorkerPool};

/// Uniswap V3 DeFi transaction indexer.
///
/// Collects, decodes, and stores Uniswap V3 events from the Ethereum
/// blockchain into PostgreSQL.
#[derive(Parser)]
#[command(name = "indexer", version, about)]
struct Cli {
    /// Start block number (inclusive). Required unless --follow.
    #[arg(long)]
    from_block: Option<u64>,

    /// End block number (inclusive). Ignored with --follow.
    #[arg(long)]
    to_block: Option<u64>,

    /// Continuously follow the chain head instead of a fixed range.
    #[arg(long)]
    follow: bool,

    /// Poll interval in seconds while following (default 12).
    #[arg(long, default_value_t = 12)]
    poll_interval_secs: u64,

    /// Confirmation lag: index only up to head - N (default 12).
    #[arg(long, default_value_t = 12)]
    confirmations: u64,

    /// Drive follow cycles by a newHeads subscription instead of polling.
    /// Requires WS_URL; falls back to polling if unavailable (D011).
    #[arg(long)]
    subscribe: bool,

    /// Run the alerts dispatcher only (S08/M003, D012). Mutually exclusive
    /// with `--follow` and fixed-range. Needs DATABASE_URL only (no RPC).
    #[arg(long)]
    dispatch_alerts: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("starting DeFi Analytics indexer");

    // CLI + 환경변수 설정 로드
    let cli = Cli::parse();
    // 리뷰 M3: 사전 동작 `--follow + --from-block`(from-block 무시)을 보존하기 위해
    // 상호배타는 `--dispatch-alerts`에만 적용한다.
    if cli.dispatch_alerts && (cli.follow || cli.from_block.is_some()) {
        anyhow::bail!("--dispatch-alerts is mutually exclusive with --follow / --from-block");
    }
    if !cli.dispatch_alerts && !cli.follow && cli.from_block.is_none() {
        anyhow::bail!("specify one of: --from-block, --follow, --dispatch-alerts");
    }

    // Dispatcher 모드는 RPC 불필요 — Config::from_env(RPC_URL 강제) 우회.
    if cli.dispatch_alerts {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?;
        let max_db_connections = std::env::var("MAX_DB_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let db_pool = db::create_pool(&database_url, max_db_connections).await?;
        db::run_migrations(&db_pool).await?;
        tracing::info!("database connected and migrations applied");
        alerts::dispatch_loop(
            db_pool,
            std::time::Duration::from_secs(cli.poll_interval_secs),
        )
        .await?;
        tracing::info!("indexer (dispatch-alerts) stopped");
        return Ok(());
    }

    let config = Config::from_env()?
        .with_block_range(cli.from_block.unwrap_or(0), cli.to_block)
        .with_follow_opts(
            cli.follow,
            cli.poll_interval_secs,
            cli.confirmations,
            cli.subscribe,
        );
    tracing::info!(?config, "configuration loaded");

    // DB 연결 + 마이그레이션
    let db_pool = db::create_pool(&config.database_url, config.max_db_connections).await?;
    db::run_migrations(&db_pool).await?;
    tracing::info!("database connected and migrations applied");

    if config.follow {
        let worker_pool = WorkerPool::new(
            db_pool,
            config.rpc_url.clone(),
            config.max_concurrent_blocks,
            config.batch_size,
        );
        let trigger = resolve_trigger_mode(config.subscribe, config.ws_url.as_deref());
        worker_pool
            .follow(
                config.confirmations,
                std::time::Duration::from_secs(config.poll_interval_secs),
                trigger,
            )
            .await?;
        tracing::info!("indexer (follow) stopped");
        return Ok(());
    }

    // 체크포인트에서 재개 지점 결정
    let from_block = match db::queries::get_last_checkpoint(&db_pool, 1).await? {
        Some(last) if last >= config.from_block as i64 => {
            let resume = (last + 1) as u64;
            tracing::info!(
                checkpoint = last,
                resume_from = resume,
                "resuming from checkpoint"
            );
            resume
        }
        _ => config.from_block,
    };

    let to_block = config.to_block.unwrap_or(from_block);
    if from_block > to_block {
        tracing::info!("all blocks already indexed up to checkpoint");
        return Ok(());
    }

    // 워커 풀 생성 및 인덱싱 시작
    let worker_pool = WorkerPool::new(
        db_pool,
        config.rpc_url.clone(),
        config.max_concurrent_blocks,
        config.batch_size,
    );

    // 고정-범위 모드는 외부 cancellation 없이 끝까지 진행 — 영구 false 플래그.
    let no_cancel = std::sync::atomic::AtomicBool::new(false);
    worker_pool
        .index_range(from_block, to_block, &no_cancel)
        .await?;

    tracing::info!("indexer finished successfully");
    Ok(())
}
