//! API library — DeFi Analytics REST API.
//!
//! 프로덕션 실행은 binary(`src/main.rs`)지만 *통합테스트가 외부 입장에서*
//! `Router`/`ApiState` 등을 빌드할 수 있도록 모듈을 lib crate로 공개한다
//! (S16/M006 — `crates/api/tests/auth.rs`).

pub mod auth;
pub mod config;
pub mod error;
pub mod pagination;
pub mod response;
pub mod routes;
