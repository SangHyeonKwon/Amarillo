use serde::Serialize;

/// 단일 리소스 API 응답 래퍼.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    /// 응답 데이터
    pub data: T,
}

/// 페이지네이션된 목록 API 응답 래퍼.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    /// 응답 데이터 목록
    pub data: Vec<T>,
    /// 페이지네이션 정보
    pub pagination: PaginationInfo,
}

/// 페이지네이션 메타데이터.
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    /// 요청된 최대 아이템 수
    pub limit: i64,
    /// 건너뛴 아이템 수
    pub offset: i64,
    /// 반환된 아이템 수
    pub count: i64,
}

/// `total`을 포함한 페이지네이션 메타데이터 (임베드형 소비자용, D005).
#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    /// 요청된 최대 아이템 수
    pub limit: i64,
    /// 건너뛴 아이템 수
    pub offset: i64,
    /// 이 페이지에 반환된 아이템 수
    pub count: i64,
    /// 필터 적용 후 전체 건수 (`LIMIT`/`OFFSET` 무관)
    pub total: i64,
}

/// `total` 포함 페이지네이션 목록 응답 래퍼.
///
/// 기존 `PaginatedResponse`는 계약 호환을 위해 불변 — 신규 엔드포인트만 사용한다(D005).
#[derive(Debug, Serialize)]
pub struct TotalPaginatedResponse<T: Serialize> {
    /// 응답 데이터 목록
    pub data: Vec<T>,
    /// 페이지네이션 정보 (`total` 포함)
    pub pagination: PaginationMeta,
}
