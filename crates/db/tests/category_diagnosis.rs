//! S12 — `category_diagnosis` lookup 통합테스트.
//!
//! 실제 PostgreSQL이 필요하다 (`#[ignore]`). 실행:
//! docker PG 기동 후 `cargo test -p db -- --ignored`.
//! 마이그레이션은 `db::run_migrations`가 멱등 적용(이미 적용분은 sqlx가 스킵).

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

/// 6 카테고리 모두 시드되어 있고, message가 비-빈, source=`builtin`이 박힘.
///
/// 본 테스트가 깨지면: 마이그레이션 시드가 누락되었거나 카테고리 wire form이
/// `ErrorCategory` enum과 어긋난 것 — 후자라면 D016 결정대로 enum 세분화는
/// 별 슬라이스에서 처리하고, 본 시드 컬럼은 이름만 유지하면 됨.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn all_six_categories_seeded() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    for cat in [
        "UNKNOWN",
        "INSUFFICIENT_BALANCE",
        "SLIPPAGE_EXCEEDED",
        "DEADLINE_EXPIRED",
        "UNAUTHORIZED",
        "TRANSFER_FAILED",
    ] {
        let d = db::queries::get_category_diagnosis(&pool, cat)
            .await
            .expect("query ok")
            .unwrap_or_else(|| panic!("{cat} must be seeded"));
        assert_eq!(d.error_category, cat);
        assert!(!d.message.is_empty(), "{cat} message must be non-empty");
        assert_eq!(
            d.source.as_deref(),
            Some("builtin"),
            "{cat} source should be 'builtin'"
        );
    }
}

/// SLIPPAGE_EXCEEDED는 명시적으로 recommended_action을 가져야 한다 (시드 정합성).
///
/// 6 카테고리 중 모두 recommended_action을 가지지만, 본 케이스로 *적어도 하나*
/// 시연. 시드가 단순 message만 박은 회귀를 잡는다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn slippage_diagnosis_has_recommended_action() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let d = db::queries::get_category_diagnosis(&pool, "SLIPPAGE_EXCEEDED")
        .await
        .expect("query ok")
        .expect("SLIPPAGE_EXCEEDED must be seeded");
    let action = d
        .recommended_action
        .as_deref()
        .expect("slippage seed must carry a recommended_action");
    assert!(!action.is_empty(), "recommended_action must be non-empty");
}

/// 미시드 카테고리 → `None` (silent default not allowed).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn nonexistent_category_is_none() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let none = db::queries::get_category_diagnosis(&pool, "NONEXISTENT_CATEGORY")
        .await
        .expect("query ok");
    assert!(
        none.is_none(),
        "unknown category must be None (caller surfaces explicit null)"
    );
}
