use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::response::{ApiResponse, PaginationMeta, TotalPaginatedResponse};

/// 응답 `call_tree` 최대 프레임 수 — 비정상적으로 큰 trace로부터 보호.
const MAX_CALL_TREE_FRAMES: i64 = 2000;

/// `0x` + 64 hex 형태인지 검사한다 (정규식 의존성 없이).
fn is_tx_hash(s: &str) -> bool {
    match s.strip_prefix("0x") {
        Some(h) => h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// tx 해시로 단건 실패 트랜잭션을 진단한다.
///
/// 디코딩된 revert 사유 + 분류된 에러 카테고리 + 평탄화된 콜트리(상한 적용)를 반환한다.
/// 형식이 잘못된 해시는 **400**, 형식은 맞으나 기록이 없으면 **404**다.
pub async fn get_failed_tx(
    State(pool): State<PgPool>,
    Path(tx_hash): Path<String>,
) -> Result<Json<ApiResponse<db::models::FailedTxDetail>>, ApiError> {
    if !is_tx_hash(&tx_hash) {
        return Err(ApiError::BadRequest(format!(
            "invalid tx_hash (expected 0x + 64 hex): {tx_hash}"
        )));
    }

    let failed = db::queries::get_failed_transaction(&pool, &tx_hash).await?;
    let mut call_tree =
        db::queries::list_trace_logs_by_tx(&pool, &tx_hash, MAX_CALL_TREE_FRAMES + 1).await?;
    let call_tree_truncated = call_tree.len() as i64 > MAX_CALL_TREE_FRAMES;
    if call_tree_truncated {
        call_tree.truncate(MAX_CALL_TREE_FRAMES as usize);
    }
    let root_cause = db::queries::get_first_error_frame(&pool, &tx_hash).await?;

    // S11 / M004 + S11.1: failing_function (selector) → 이름/시그니처 lookup +
    // root frame input bytes에서 args 디코드(D027 — 실패 시 args=None, 객체 자체는
    // 살려둠).
    let failing_function_decoded = match failed.failing_function.as_deref() {
        Some(selector) => match db::queries::get_function_signature(&pool, selector).await? {
            Some(fs) => {
                let signature = fs.signature.clone();
                let mut decoded: db::models::DecodedFunction = fs.into();
                // root frame input = `call_depth == 0`인 첫 frame의 input (pre-order
                // DFS이므로 list 첫 원소가 보통 root이지만 명시 lookup으로 안전).
                if let Some(root_input) = call_tree
                    .iter()
                    .find(|f| f.call_depth == 0)
                    .and_then(|f| f.input.as_deref())
                {
                    match db::abi::decode_args(&signature, root_input) {
                        Ok(args) => decoded.args = Some(args),
                        Err(e) => tracing::debug!(
                            tx_hash = %tx_hash, error = %e,
                            "abi decode failed for failing_function args"
                        ),
                    }
                }
                Some(decoded)
            }
            None => None,
        },
        None => None,
    };

    // S11.1: root_cause.input의 첫 4바이트 → selector lookup → DecodedFunction
    // 합성 + args 디코드. `root_cause`, `input`, 시드 매칭 중 하나라도 없으면
    // 명시 `null` (D027/D014 일관).
    let root_cause_decoded = match root_cause.as_ref().and_then(|rc| rc.input.as_deref()) {
        Some(input_hex) => match extract_selector(input_hex) {
            Some(selector) => match db::queries::get_function_signature(&pool, &selector).await? {
                Some(fs) => {
                    let signature = fs.signature.clone();
                    let mut decoded: db::models::DecodedFunction = fs.into();
                    match db::abi::decode_args(&signature, input_hex) {
                        Ok(args) => decoded.args = Some(args),
                        Err(e) => tracing::debug!(
                            tx_hash = %tx_hash, error = %e,
                            "abi decode failed for root_cause args"
                        ),
                    }
                    Some(decoded)
                }
                None => None,
            },
            None => None,
        },
        None => None,
    };

    // S12 / M004: error_category → 사람이 읽는 진단 메시지 + 추천 액션 lookup.
    // `as_wire()`로 SCREAMING_SNAKE form 변환(단일 출처). 시드 미존재 카테고리는
    // 명시 `null` (silent default 금지 — D014/D016 일관). enum 세분화는 S12.1.
    let diagnosis = db::queries::get_category_diagnosis(&pool, failed.error_category.as_wire())
        .await?
        .map(db::models::Diagnosis::from);

    Ok(Json(ApiResponse {
        data: db::models::FailedTxDetail {
            failed,
            call_tree,
            call_tree_truncated,
            root_cause,
            failing_function_decoded,
            root_cause_decoded,
            diagnosis,
        },
    }))
}

/// Pull the 4-byte selector (`0x` + 8 lowercase hex) from a raw input hex
/// string. Returns `None` if the input is too short to host a selector.
fn extract_selector(input_hex: &str) -> Option<String> {
    let stripped = input_hex.strip_prefix("0x").unwrap_or(input_hex);
    if stripped.len() < 8 {
        return None;
    }
    Some(format!("0x{}", stripped[..8].to_lowercase()))
}

/// `GET /v1/failed-tx` 쿼리 파라미터.
#[derive(Deserialize)]
pub struct FailedTxQuery {
    /// 에러 카테고리 필터 (SCREAMING_SNAKE_CASE, 선택)
    pub category: Option<String>,
    /// 시작 시각 필터, RFC3339 (선택)
    pub from: Option<String>,
    /// 종료 시각 필터, RFC3339 (선택)
    pub to: Option<String>,
    /// 페이지 크기 (기본 20, 최대 100)
    pub limit: Option<i64>,
    /// 건너뛸 개수 (기본 0)
    pub offset: Option<i64>,
}

/// 선택 RFC3339 타임스탬프를 파싱한다 — 형식 오류는 400.
fn parse_ts(value: Option<&str>, field: &str) -> Result<Option<DateTime<Utc>>, ApiError> {
    match value {
        None | Some("") => Ok(None),
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map(|dt| Some(dt.with_timezone(&Utc)))
            .map_err(|_| {
                ApiError::BadRequest(format!(
                    "invalid `{field}` timestamp (expected RFC3339): {s}"
                ))
            }),
    }
}

/// 실패 트랜잭션을 필터·페이지네이션하여 조회한다 (`total` 포함).
///
/// 잘못된 `category`/타임스탬프는 404가 아니라 400이다(클라이언트 입력 오류).
pub async fn list_failed_tx(
    State(pool): State<PgPool>,
    Query(q): Query<FailedTxQuery>,
) -> Result<Json<TotalPaginatedResponse<db::models::FailedTransaction>>, ApiError> {
    let category = match q.category.as_deref() {
        None | Some("") => None,
        Some(s) => Some(
            s.parse::<db::models::ErrorCategory>()
                .map_err(|_| ApiError::BadRequest(format!("invalid `category`: {s}")))?,
        ),
    };
    let from = parse_ts(q.from.as_deref(), "from")?;
    let to = parse_ts(q.to.as_deref(), "to")?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let rows =
        db::queries::list_failed_transactions(&pool, category.as_ref(), from, to, limit, offset)
            .await?;
    let total = db::queries::count_failed_transactions(&pool, category.as_ref(), from, to).await?;
    let count = rows.len() as i64;

    Ok(Json(TotalPaginatedResponse {
        data: rows,
        pagination: PaginationMeta {
            limit,
            offset,
            count,
            total,
        },
    }))
}

/// `GET /v1/analytics/failed-tx/timeseries` 쿼리 파라미터.
#[derive(Deserialize)]
pub struct FailedTxTrendQuery {
    /// 버킷 단위: `hour|day|week` (기본 `day`)
    pub interval: Option<String>,
    /// 시작 시각, RFC3339 (선택)
    pub from: Option<String>,
    /// 종료 시각, RFC3339 (선택)
    pub to: Option<String>,
}

/// 실패 트랜잭션 추이를 시간 버킷 × 카테고리로 집계해 반환한다.
///
/// `interval`은 화이트리스트(`hour|day|week`)만 허용 — 그 외는 400.
pub async fn failed_tx_timeseries(
    State(pool): State<PgPool>,
    Query(q): Query<FailedTxTrendQuery>,
) -> Result<Json<ApiResponse<Vec<db::models::FailedTxTrendPoint>>>, ApiError> {
    let bucket = match q.interval.as_deref() {
        None | Some("") => db::models::TimeBucket::Day,
        Some(s) => s.parse::<db::models::TimeBucket>().map_err(|_| {
            ApiError::BadRequest(format!("invalid `interval` (hour|day|week): {s}"))
        })?,
    };
    let from = parse_ts(q.from.as_deref(), "from")?;
    let to = parse_ts(q.to.as_deref(), "to")?;

    let points = db::queries::failed_tx_timeseries(&pool, &bucket, from, to).await?;
    Ok(Json(ApiResponse { data: points }))
}

/// `GET /v1/analytics/failed-tx/by-label` 쿼리 파라미터 (S09 / M003).
#[derive(Deserialize)]
pub struct FailedTxByLabelQuery {
    /// 시작 시각, RFC3339 (선택)
    pub from: Option<String>,
    /// 종료 시각, RFC3339 (선택)
    pub to: Option<String>,
    /// 테넌시 필터 — 빈 문자열/생략은 "모든 라벨"(공개 + 모든 테넌트)
    pub owner: Option<String>,
    /// 결과 행 수 (기본 50, 최대 200)
    pub limit: Option<i64>,
}

/// 라벨된 컨트랙트별 실패 분포를 반환한다 (S09 / M003).
///
/// `contract_label × transaction × failed_transaction` 조인 결과를 (라벨, 주소)
/// 별로 그루핑해 `total_failures` + 카테고리 카운트 맵으로 노출. 잘못된 RFC3339는
/// 400, 빈 결과는 200 + 빈 배열. **Dune이 구조적으로 못 하는** 비공개 라벨 조인의
/// 단일 시연 엔드포인트.
pub async fn failed_tx_by_label(
    State(pool): State<PgPool>,
    Query(q): Query<FailedTxByLabelQuery>,
) -> Result<Json<ApiResponse<Vec<db::models::FailedTxByLabelPoint>>>, ApiError> {
    let from = parse_ts(q.from.as_deref(), "from")?;
    let to = parse_ts(q.to.as_deref(), "to")?;
    let owner = q.owner.as_deref().filter(|s| !s.is_empty());
    let limit = q.limit.unwrap_or(50).clamp(1, 200);

    let rows = db::queries::failed_tx_by_label_aggregate(&pool, owner, from, to, limit).await?;
    Ok(Json(ApiResponse { data: rows }))
}
