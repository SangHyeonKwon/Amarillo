//! Shared router state — `ApiState` (S16/M006).
//!
//! 모든 핸들러는 [`ApiState`]를 통해 DB pool과 admin API key를 접근한다.
//! 기존 `State<PgPool>` 사용 핸들러는 [`FromRef`] 구현 덕에 *변경 없이* 그대로
//! 컴파일된다(D022 — extractor 게이트는 S16-T02에서 핸들러 시그니처에 박음).

use std::sync::Arc;

use axum::extract::FromRef;
use sqlx::PgPool;

/// API 라우터 공유 상태.
///
/// - `db_pool`: PostgreSQL 연결 풀.
/// - `admin_api_key`: write/admin 라우트 보호용 키 (S16/M006/D021). `Arc<str>`로
///   wrap해 router clone 시 cheap copy + 메모리 한 부 (key는 process 수명 동안 불변).
#[derive(Clone)]
pub struct ApiState {
    /// PostgreSQL 연결 풀.
    pub db_pool: PgPool,
    /// Admin/write 라우트 보호용 API key (S16/M006).
    pub admin_api_key: Arc<str>,
}

/// 기존 `State<PgPool>` 핸들러가 변경 없이 동작하도록 하는 axum 표준 패턴.
impl FromRef<ApiState> for PgPool {
    fn from_ref(state: &ApiState) -> Self {
        state.db_pool.clone()
    }
}
