//! TUI 에러 타입.
//!
//! 라이브러리 레이어(client/terminal)는 [`TuiError`]를 반환하고, 바이너리
//! 진입점(`main`)은 `anyhow`로 래핑한다 — 워크스페이스 컨벤션과 일관.

/// TUI에서 발생하는 모든 복구 가능한 에러.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    /// HTTP 요청 자체가 실패함 (연결 거부, 타임아웃, DNS 등).
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// 응답 본문을 기대한 DTO로 역직렬화하지 못함.
    #[error("failed to decode response: {0}")]
    Decode(String),

    /// API가 4xx/5xx와 `{\"error\": ...}` 본문을 반환함.
    #[error("api error ({status}): {message}")]
    Api {
        /// HTTP 상태 코드.
        status: u16,
        /// 파싱된 에러 메시지 (파싱 실패 시 raw 본문).
        message: String,
    },

    /// 터미널 I/O 에러 (raw mode 진입/복원 등).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
