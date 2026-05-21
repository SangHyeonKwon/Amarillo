//! `/v1/contract-labels` — admin endpoints for the `contract_label` table (S15 / M005).
//!
//! `POST`은 *create-or-update* (UPSERT) 시맨틱 — 같은 address면 label/owner_id를
//! 새 값으로 덮어쓴다. `DELETE`는 영구 삭제(soft 아님; CASCADE는 contract_label
//! 에 없음 — 단순 row 제거).
//!
//! **인증 (S16/M006/D021/D022 적용 완료)**: 두 핸들러는 `_: AdminAuth` extractor를
//! 첫 파라미터로 받아 `Authorization: Bearer <AMARILLO_ADMIN_API_KEY>` 헤더가
//! 일치해야 통과. 누락/형식 오류/키 불일치 모두 401(info-leak 방지로 단일 응답).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth::AdminAuth;
use crate::error::ApiError;
use crate::response::ApiResponse;
use db::models::ContractLabel;

const ADDR_HEX_LEN: usize = 40;
const MAX_LABEL_LEN: usize = 100;
const MAX_OWNER_ID_LEN: usize = 100;

/// `POST /v1/contract-labels` 본문.
#[derive(Debug, Deserialize)]
pub struct CreateLabelBody {
    /// `0x` + 40 hex (대소문자 무관 — 핸들러가 lowercase 정규화).
    pub address: String,
    /// 사람이 읽는 라벨 (비-빈, 최대 100자).
    pub label: String,
    /// 테넌시 힌트 (선택, 최대 100자). `None`/빈 문자열 = 공개 라벨.
    pub owner_id: Option<String>,
}

/// `0x` + 40 hex 형식 검사 (lowercased 입력에 대해).
fn is_evm_address(lower: &str) -> bool {
    match lower.strip_prefix("0x") {
        Some(hex) => hex.len() == ADDR_HEX_LEN && hex.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// 컨트랙트 라벨을 생성·갱신한다 (UPSERT).
///
/// 잘못된 주소/빈 label/길이 초과 모두 400. UPSERT라 같은 address 재호출은
/// label/owner_id를 덮어쓴 새 행을 201로 반환 — 호출자는 결과를 확인하기 위해
/// 별도 GET 불필요. 인증 필수 (S16/M006/D021 — `AdminAuth` extractor).
pub async fn create_contract_label(
    _: AdminAuth,
    State(pool): State<PgPool>,
    Json(body): Json<CreateLabelBody>,
) -> Result<(StatusCode, Json<ApiResponse<ContractLabel>>), ApiError> {
    let address = body.address.trim().to_ascii_lowercase();
    if !is_evm_address(&address) {
        return Err(ApiError::BadRequest(format!(
            "invalid `address` (expected 0x + 40 hex): {}",
            body.address
        )));
    }
    let label = body.label.trim();
    if label.is_empty() {
        return Err(ApiError::BadRequest("`label` must be non-empty".into()));
    }
    if label.len() > MAX_LABEL_LEN {
        return Err(ApiError::BadRequest(format!(
            "`label` too long (max {MAX_LABEL_LEN} bytes)"
        )));
    }
    let owner_id_norm: Option<&str> = body.owner_id.as_deref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });
    if let Some(o) = owner_id_norm {
        if o.len() > MAX_OWNER_ID_LEN {
            return Err(ApiError::BadRequest(format!(
                "`owner_id` too long (max {MAX_OWNER_ID_LEN} bytes)"
            )));
        }
    }

    let row = db::queries::upsert_contract_label(&pool, &address, label, owner_id_norm).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse { data: row })))
}

/// 컨트랙트 라벨을 영구 삭제한다. 미존재 → 404, 잘못된 주소 → 400, 성공 → 204.
///
/// 멱등의 *의미*: 같은 주소 두 번째 DELETE는 404 (이미 없음) — 운영자가 멱등
/// retry 시 404를 *no-op 신호*로 해석 가능. 인증 필수 (S16/M006/D021).
pub async fn delete_contract_label(
    _: AdminAuth,
    State(pool): State<PgPool>,
    Path(address): Path<String>,
) -> Result<StatusCode, ApiError> {
    let lower = address.trim().to_ascii_lowercase();
    if !is_evm_address(&lower) {
        return Err(ApiError::BadRequest(format!(
            "invalid `address` (expected 0x + 40 hex): {address}"
        )));
    }
    let affected = db::queries::delete_contract_label(&pool, &lower).await?;
    if affected == 0 {
        return Err(ApiError::NotFound(format!(
            "contract_label {lower} (not found)"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}
