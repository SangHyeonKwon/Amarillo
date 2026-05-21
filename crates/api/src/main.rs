//! DeFi Analytics REST API 서버 (binary entrypoint).
//!
//! 인덱싱된 Uniswap V3 데이터를 JSON REST API로 제공한다. 모듈은 `api` lib crate에
//! 있고, 본 binary는 진입점만 담당한다(통합테스트가 lib에서 router를 빌드할 수
//! 있도록 — S16/M006).
//!
//! ## 사용법
//! ```bash
//! # 환경변수 설정 후 (DATABASE_URL, AMARILLO_ADMIN_API_KEY 필수)
//! cargo run -p api
//! ```

use tracing_subscriber::EnvFilter;

use api::config::ApiConfig;
use api::routes::{self, ApiState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = ApiConfig::from_env()?;
    // 시크릿 필드는 ApiConfig::Debug에서 마스킹됨 (S16/M006 — HARDEN2 정신).
    tracing::info!(?config, "starting DeFi Analytics API");

    let db_pool = db::create_pool(&config.database_url, config.max_db_connections).await?;
    db::run_migrations(&db_pool).await?;
    tracing::info!("database connected and migrations applied");

    let state = ApiState {
        db_pool,
        admin_api_key: config.admin_api_key.clone().into(),
    };
    let app = routes::build_router(state);
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr, "API server listening");

    axum::serve(listener, app).await?;
    Ok(())
}
