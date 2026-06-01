//! TUI 설정 — 환경변수에서 로드.
//!
//! `crates/api/src/config.rs` 패턴을 미러: `from_env()`가 `from_env_with(get)`의
//! thin wrapper(테스트 주입형), 시크릿은 [`Debug`]에서 마스킹. API 클라이언트는
//! DB 자격증명이 필요 없고 `AMARILLO_API_URL`만 알면 동작한다.

use std::env;
use std::fmt;

/// 기본 API 베이스 URL (로컬 개발).
const DEFAULT_API_URL: &str = "http://127.0.0.1:3000";
/// 기본 자동 폴링 간격(초).
const DEFAULT_REFRESH_SECS: u64 = 10;
/// 기본 HTTP 요청 타임아웃(초).
const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// TUI 런타임 설정.
///
/// `admin_api_key`는 읽기 전용 MVP에선 미사용이나, 향후 write 기능(alert
/// subscriptions, contract labels)을 위해 배선해 둔다. [`Debug`]에서 마스킹된다.
#[derive(Clone)]
pub struct TuiConfig {
    /// API 베이스 URL (끝 `/` 제거됨), 예: `http://127.0.0.1:3000`.
    pub api_url: String,
    /// Admin/write API key (Bearer). 미설정 시 `None` — 읽기 전용 MVP에선 무방.
    pub admin_api_key: Option<String>,
    /// 자동 폴링 간격(초).
    pub refresh_interval_secs: u64,
    /// HTTP 요청 타임아웃(초) — hung 태스크 방지.
    pub request_timeout_secs: u64,
    /// 로그 파일을 둘 디렉토리 (TUI는 stdout에 로그 금지).
    pub log_dir: String,
    /// `RUST_LOG` 스타일 필터 문자열 (기본 `info`).
    pub log_filter: String,
}

impl TuiConfig {
    /// 프로세스 환경변수에서 설정을 로드한다.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_env_with(|k| env::var(k).ok())
    }

    /// 환경변수 getter를 주입받아 설정을 빌드한다 — 단위테스트용.
    ///
    /// 모든 필드가 합리적 기본값을 가지므로 실패하지 않지만, 시그니처는 향후
    /// 필수 항목 추가를 위해 `Result`를 유지한다(`api`와 동일).
    #[doc(hidden)]
    pub fn from_env_with<F>(get: F) -> anyhow::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let api_url = get("AMARILLO_API_URL")
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_API_URL.to_string());

        let admin_api_key = get("AMARILLO_ADMIN_API_KEY")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let refresh_interval_secs = get("AMARILLO_TUI_REFRESH_SECS")
            .and_then(|v| v.trim().parse().ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_REFRESH_SECS);

        let request_timeout_secs = get("AMARILLO_TUI_TIMEOUT_SECS")
            .and_then(|v| v.trim().parse().ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        let log_dir = get("AMARILLO_TUI_LOG_DIR")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ".".to_string());

        let log_filter = get("RUST_LOG")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "info".to_string());

        Ok(Self {
            api_url,
            admin_api_key,
            refresh_interval_secs,
            request_timeout_secs,
            log_dir,
            log_filter,
        })
    }
}

/// 시크릿 노출 방지 — `admin_api_key`는 `Some("***")`/`None`으로 마스킹.
impl fmt::Debug for TuiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TuiConfig")
            .field("api_url", &self.api_url)
            .field("admin_api_key", &self.admin_api_key.as_ref().map(|_| "***"))
            .field("refresh_interval_secs", &self.refresh_interval_secs)
            .field("request_timeout_secs", &self.request_timeout_secs)
            .field("log_dir", &self.log_dir)
            .field("log_filter", &self.log_filter)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn fake_env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn defaults_when_unset() {
        let cfg = TuiConfig::from_env_with(fake_env(&[])).expect("defaults ok");
        assert_eq!(cfg.api_url, DEFAULT_API_URL);
        assert_eq!(cfg.admin_api_key, None);
        assert_eq!(cfg.refresh_interval_secs, DEFAULT_REFRESH_SECS);
        assert_eq!(cfg.request_timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert_eq!(cfg.log_dir, ".");
        assert_eq!(cfg.log_filter, "info");
    }

    #[test]
    fn overrides_parse_and_strip_trailing_slash() {
        let cfg = TuiConfig::from_env_with(fake_env(&[
            ("AMARILLO_API_URL", "https://amarillo.example.com/"),
            ("AMARILLO_ADMIN_API_KEY", "  secret-key  "),
            ("AMARILLO_TUI_REFRESH_SECS", "5"),
            ("AMARILLO_TUI_TIMEOUT_SECS", "30"),
            ("RUST_LOG", "tui=debug"),
        ]))
        .expect("overrides ok");
        assert_eq!(cfg.api_url, "https://amarillo.example.com");
        assert_eq!(cfg.admin_api_key.as_deref(), Some("secret-key"));
        assert_eq!(cfg.refresh_interval_secs, 5);
        assert_eq!(cfg.request_timeout_secs, 30);
        assert_eq!(cfg.log_filter, "tui=debug");
    }

    #[test]
    fn zero_refresh_falls_back_to_default() {
        let cfg =
            TuiConfig::from_env_with(fake_env(&[("AMARILLO_TUI_REFRESH_SECS", "0")])).expect("ok");
        assert_eq!(cfg.refresh_interval_secs, DEFAULT_REFRESH_SECS);
    }

    #[test]
    fn debug_masks_admin_api_key() {
        let cfg =
            TuiConfig::from_env_with(fake_env(&[("AMARILLO_ADMIN_API_KEY", "super-secret-key")]))
                .expect("ok");
        let s = format!("{cfg:?}");
        assert!(!s.contains("super-secret-key"), "key leaked: {s}");
        assert!(s.contains("***"), "expected mask marker: {s}");
    }
}
