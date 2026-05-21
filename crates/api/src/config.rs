use std::env;
use std::fmt;

/// 권장 API key 최소 바이트 (S16/M006/D023 — 거부는 아님, WARN 트리거).
const RECOMMENDED_KEY_BYTES: usize = 32;

/// API 서버 설정.
///
/// 환경변수에서 로드한다.
///
/// **시크릿 필드는 [`Debug`]에서 마스킹**(`***`): `database_url`(credentials 포함) +
/// `admin_api_key`(S16/M006/D023). HARDEN2 정신 일관 (webhook_url/signing_secret).
#[derive(Clone)]
pub struct ApiConfig {
    /// PostgreSQL 연결 문자열 (credentials 포함, Debug 마스킹)
    pub database_url: String,
    /// 서버 바인딩 호스트
    pub host: String,
    /// 서버 바인딩 포트
    pub port: u16,
    /// DB 연결 풀 최대 크기
    pub max_db_connections: u32,
    /// Admin/write API 보호용 API key (S16/M006/D021/D023; Debug 마스킹).
    ///
    /// 빈 문자열/미설정은 [`from_env`](Self::from_env)에서 거부됨 (silent default
    /// 금지, D004 정신).
    pub admin_api_key: String,
}

impl ApiConfig {
    /// 환경변수에서 설정을 로드한다.
    ///
    /// 필수:
    /// - `DATABASE_URL`
    /// - `AMARILLO_ADMIN_API_KEY` (S16/M006/D023 — 빈 문자열 거부, 짧으면 WARN)
    ///
    /// 선택: `API_HOST` (기본 `0.0.0.0`), `API_PORT` (기본 `3000`),
    /// `MAX_DB_CONNECTIONS` (기본 `10`).
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_env_with(|k| env::var(k).ok())
    }

    /// 환경변수 getter를 주입받아 설정을 빌드한다 — 단위테스트용.
    ///
    /// `from_env`는 본 함수에 `env::var`를 주입한 thin wrapper. 글로벌 env 상태
    /// mutation 없이 모든 분기(미설정 / 빈 / 짧음 / 정상)를 테스트할 수 있게 함.
    #[doc(hidden)]
    pub fn from_env_with<F>(get: F) -> anyhow::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let database_url = get("DATABASE_URL")
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("DATABASE_URL environment variable is required"))?;

        let admin_api_key = match get("AMARILLO_ADMIN_API_KEY") {
            Some(v) if !v.trim().is_empty() => v.trim().to_string(),
            _ => anyhow::bail!(
                "AMARILLO_ADMIN_API_KEY environment variable is required (S16/M006/D023, no silent default)"
            ),
        };

        if admin_api_key.len() < RECOMMENDED_KEY_BYTES {
            tracing::warn!(
                actual = admin_api_key.len(),
                recommended = RECOMMENDED_KEY_BYTES,
                "AMARILLO_ADMIN_API_KEY shorter than recommended; rotate to a longer key for production"
            );
        }

        Ok(Self {
            database_url,
            host: get("API_HOST").unwrap_or_else(|| "0.0.0.0".to_string()),
            port: get("API_PORT").and_then(|v| v.parse().ok()).unwrap_or(3000),
            max_db_connections: get("MAX_DB_CONNECTIONS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            admin_api_key,
        })
    }
}

/// 시크릿 노출 방지 — `database_url`(credentials 포함)과 `admin_api_key`는 마스킹.
/// HARDEN2 정신 일관 (webhook_url/signing_secret 마스킹 패턴).
impl fmt::Debug for ApiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiConfig")
            .field("database_url", &"***")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("max_db_connections", &self.max_db_connections)
            .field("admin_api_key", &"***")
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
    fn from_env_with_missing_admin_key_fails() {
        let r = ApiConfig::from_env_with(fake_env(&[("DATABASE_URL", "postgres://test")]));
        assert!(r.is_err());
        let err = r.unwrap_err().to_string();
        assert!(
            err.contains("AMARILLO_ADMIN_API_KEY"),
            "expected admin key error, got: {err}"
        );
    }

    #[test]
    fn from_env_with_empty_admin_key_fails() {
        let r = ApiConfig::from_env_with(fake_env(&[
            ("DATABASE_URL", "postgres://test"),
            ("AMARILLO_ADMIN_API_KEY", "   "),
        ]));
        assert!(r.is_err());
        let err = r.unwrap_err().to_string();
        assert!(err.contains("AMARILLO_ADMIN_API_KEY"));
    }

    #[test]
    fn from_env_with_short_admin_key_succeeds() {
        let r = ApiConfig::from_env_with(fake_env(&[
            ("DATABASE_URL", "postgres://test"),
            ("AMARILLO_ADMIN_API_KEY", "short-key"),
        ]));
        assert!(r.is_ok());
        let cfg = r.unwrap();
        assert_eq!(cfg.admin_api_key, "short-key");
    }

    #[test]
    fn from_env_with_normal_admin_key_succeeds() {
        let key = "a".repeat(64);
        let r = ApiConfig::from_env_with(fake_env(&[
            ("DATABASE_URL", "postgres://test"),
            ("AMARILLO_ADMIN_API_KEY", key.as_str()),
        ]));
        assert!(r.is_ok());
        let cfg = r.unwrap();
        assert_eq!(cfg.admin_api_key, key);
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 3000);
        assert_eq!(cfg.max_db_connections, 10);
    }

    #[test]
    fn from_env_with_missing_database_url_fails() {
        let r = ApiConfig::from_env_with(fake_env(&[("AMARILLO_ADMIN_API_KEY", "k")]));
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn debug_masks_admin_api_key_and_database_url() {
        let config = ApiConfig {
            database_url: "postgres://user:secret123@host/db".into(),
            host: "0.0.0.0".into(),
            port: 3000,
            max_db_connections: 10,
            admin_api_key: "super-secret-key".into(),
        };
        let s = format!("{:?}", config);
        assert!(!s.contains("super-secret-key"), "admin_api_key leaked: {s}");
        assert!(
            !s.contains("secret123"),
            "database_url password leaked: {s}"
        );
        assert!(s.contains("***"), "expected mask marker `***`: {s}");
        assert!(s.contains("admin_api_key"), "field name should appear: {s}");
    }
}
