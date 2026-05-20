//! `/v1/alert-subscriptions` — 실패 패턴 구독 CRUD (S08-T03).
//!
//! - POST: `webhook_url`을 [`db::validators::webhook_url_is_safe`]로 검증 (unsafe → 400),
//!   `error_category`를 [`ErrorCategory::FromStr`] 단일 출처로 파싱 (invalid → 400),
//!   `to_addr`는 소문자 정규화, `signing_secret`은 CSPRNG로 32바이트 생성해 hex.
//!   **생성 응답에서 signing_secret을 1회만** 반환한다 — 모델의 `#[serde(skip_serializing)]`
//!   덕에 이후 조회·로그에선 노출되지 않음.
//! - GET: 활성·비활성 모두 최신순(`subscription_id DESC`), `limit` 캡(1..=500).
//! - DELETE: soft 비활성화 (`active = FALSE`). 행 미존재/이미 비활성이면 404.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::response::ApiResponse;
use db::models::{AlertSubscription, AlertSubscriptionCreated, ErrorCategory};
use db::validators::webhook_url_is_safe;

/// `webhook_url` 길이 상한 (DoS 방어용 휴리스틱; RFC 8615 권고와 일치).
const MAX_WEBHOOK_URL_LEN: usize = 2048;
/// 기본 GET 페이지 크기.
const DEFAULT_LIST_LIMIT: i64 = 100;
/// GET 최대 페이지 크기.
const MAX_LIST_LIMIT: i64 = 500;
/// signing_secret 바이트 수(hex 인코딩 후 64자).
const SECRET_BYTES: usize = 32;
/// 0x + 40 hex 형태의 EVM 주소.
const ADDR_HEX_LEN: usize = 40;

/// `POST /v1/alert-subscriptions` 요청 본문.
#[derive(Debug, Deserialize)]
pub struct CreateBody {
    /// 알림을 받을 HTTPS URL (사설/loopback/메타데이터/로컬 호스트 거부)
    pub webhook_url: String,
    /// 매칭할 에러 카테고리 (SCREAMING_SNAKE_CASE; 생략 시 모든 카테고리)
    pub error_category: Option<String>,
    /// 매칭할 컨트랙트 주소 (`0x` + 40 hex; 생략 시 모든 주소). 자동 소문자.
    pub to_addr: Option<String>,
}

/// `GET /v1/alert-subscriptions` 쿼리 파라미터.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// 페이지 크기 (기본 100, 최대 500)
    pub limit: Option<i64>,
}

/// CSPRNG로 hex signing_secret 생성. OS RNG 실패는 500.
fn generate_signing_secret() -> Result<String, ApiError> {
    let mut buf = [0u8; SECRET_BYTES];
    getrandom::getrandom(&mut buf).map_err(|e| {
        tracing::error!(error = %e, "OS RNG failed for signing_secret");
        ApiError::Internal("rng unavailable".into())
    })?;
    Ok(hex::encode(buf))
}

/// `0x` + 40 hex 형식 검사 (lowercased 입력에 대해).
fn is_evm_address(lower: &str) -> bool {
    match lower.strip_prefix("0x") {
        Some(hex) => hex.len() == ADDR_HEX_LEN && hex.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// 새 알림 구독을 생성한다. **`signing_secret`은 이 응답에서 1회만 노출**.
///
/// 거부 케이스(전부 400): `webhook_url`이 [`webhook_url_is_safe`]에서 reject /
/// `webhook_url` 길이가 [`MAX_WEBHOOK_URL_LEN`] 초과 / `error_category` 파싱 실패
/// / `to_addr` 형식 오류.
pub async fn create_alert_subscription(
    State(pool): State<PgPool>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<ApiResponse<AlertSubscriptionCreated>>), ApiError> {
    if body.webhook_url.len() > MAX_WEBHOOK_URL_LEN {
        return Err(ApiError::BadRequest(format!(
            "webhook_url too long (max {MAX_WEBHOOK_URL_LEN})"
        )));
    }
    if let Err(reason) = webhook_url_is_safe(&body.webhook_url) {
        return Err(ApiError::BadRequest(format!(
            "unsafe webhook_url: {reason:?}"
        )));
    }

    let category = match body.error_category.as_deref() {
        None | Some("") => None,
        Some(s) => Some(
            s.parse::<ErrorCategory>()
                .map_err(|_| ApiError::BadRequest(format!("invalid `error_category`: {s}")))?,
        ),
    };

    let to_addr_norm: Option<String> = match body.to_addr.as_deref() {
        None | Some("") => None,
        Some(s) => {
            let lower = s.to_ascii_lowercase();
            if !is_evm_address(&lower) {
                return Err(ApiError::BadRequest(format!(
                    "invalid `to_addr` (expected 0x + 40 hex): {s}"
                )));
            }
            Some(lower)
        }
    };

    let signing_secret = generate_signing_secret()?;
    let row = db::queries::insert_alert_subscription(
        &pool,
        category.as_ref(),
        to_addr_norm.as_deref(),
        &body.webhook_url,
        &signing_secret,
    )
    .await?;

    // 보안: signing_secret을 응답에 1회 노출(이후엔 어디서도 조회 불가).
    let created = AlertSubscriptionCreated {
        subscription_id: row.subscription_id,
        error_category: row.error_category,
        to_addr: row.to_addr,
        webhook_url: row.webhook_url,
        signing_secret: row.signing_secret,
        active: row.active,
        created_at: row.created_at,
    };
    Ok((StatusCode::CREATED, Json(ApiResponse { data: created })))
}

/// 알림 구독 목록을 최신순으로 조회한다. **`signing_secret`은 노출 안 됨**(모델
/// 자체가 `#[serde(skip_serializing)]`).
pub async fn list_alert_subscriptions(
    State(pool): State<PgPool>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<AlertSubscription>>>, ApiError> {
    let limit = q
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let rows = db::queries::list_alert_subscriptions(&pool, limit).await?;
    Ok(Json(ApiResponse { data: rows }))
}

/// 알림 구독을 비활성화한다(soft delete). 미존재/이미 비활성이면 404.
///
/// 영구 삭제는 호출자가 별도 절차로(예: 어드민 도구) 처리 — 본 엔드포인트는
/// **운영 안전을 위해 soft만** 제공(`alert_delivery` 멱등 이력을 보존).
pub async fn deactivate_alert_subscription(
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let affected = db::queries::deactivate_alert_subscription(&pool, id).await?;
    if affected == 0 {
        return Err(ApiError::NotFound(format!(
            "alert subscription {id} (not found or already inactive)"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}
