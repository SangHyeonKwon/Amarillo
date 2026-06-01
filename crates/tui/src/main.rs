//! amarillo Failure Intelligence TUI (binary entrypoint).
//!
//! 기존 axum REST API를 소비하는 터미널 클라이언트. DB에 직접 붙지 않고
//! `AMARILLO_API_URL`만으로 동작하므로, 로컬·원격 amarillo 인스턴스를 모두
//! 모니터링할 수 있다.
//!
//! ## 사용법
//! ```bash
//! # API 서버가 떠 있어야 함 (cargo run -p api)
//! AMARILLO_API_URL=http://127.0.0.1:3000 cargo run -p tui
//! ```

mod app;
mod client;
mod config;
mod dto;
mod error;
mod event;
mod format;
mod terminal;
mod ui;

use anyhow::Context;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use config::TuiConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = TuiConfig::from_env()?;
    // 로그 가드는 종료까지 살려둬야 버퍼가 flush 된다.
    let _guard = init_logging(&config)?;
    // 시크릿은 TuiConfig::Debug에서 마스킹됨.
    tracing::info!(?config, "starting amarillo TUI");

    terminal::install_panic_hook();
    let mut term = terminal::setup().context("failed to set up terminal")?;
    let result = app::run(&mut term, config).await;
    // 결과와 무관하게 항상 터미널을 복원한다.
    if let Err(e) = terminal::restore(&mut term) {
        tracing::error!(error = %e, "failed to restore terminal");
    }
    result
}

/// 로깅을 **파일로** 초기화한다 — TUI가 터미널을 점유하므로 stdout/stderr 금지.
///
/// `tracing-appender`의 non-blocking writer로 디스크 I/O를 별도 스레드로 보내
/// 렌더 루프를 막지 않는다. 반환된 [`WorkerGuard`]는 drop 시 버퍼를 flush 한다.
fn init_logging(cfg: &TuiConfig) -> anyhow::Result<WorkerGuard> {
    let appender = tracing_appender::rolling::never(&cfg.log_dir, "amarillo-tui.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_env_filter(EnvFilter::new(&cfg.log_filter))
        .init();
    Ok(guard)
}
