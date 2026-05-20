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

    Ok(Json(ApiResponse {
        data: db::models::FailedTxDetail {
            failed,
            call_tree,
            call_tree_truncated,
        },
    }))
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
