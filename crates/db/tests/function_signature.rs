//! S11 — `function_signature` lookup 통합테스트.
//!
//! 실제 PostgreSQL이 필요하다 (`#[ignore]`). 실행:
//! docker PG 기동 후 `cargo test -p db -- --ignored`.
//! 마이그레이션은 `db::run_migrations`가 멱등 적용(이미 적용분은 sqlx가 스킵).

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

/// 시드된 selector(`0xa9059cbb` = ERC20 transfer) → 정확히 매칭.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn get_function_signature_seeded_lookup_ok() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let fs = db::queries::get_function_signature(&pool, "0xa9059cbb")
        .await
        .expect("query ok")
        .expect("ERC20 transfer must be seeded");
    assert_eq!(fs.selector, "0xa9059cbb");
    assert_eq!(fs.name, "transfer");
    assert_eq!(fs.signature, "transfer(address,uint256)");
    assert_eq!(fs.source.as_deref(), Some("erc20"));
}

/// 시드된 selector(`0x414bf389` = Uniswap V3 SwapRouter exactInputSingle).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn get_function_signature_uniswap_router_seeded() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let fs = db::queries::get_function_signature(&pool, "0x414bf389")
        .await
        .expect("query ok")
        .expect("Uniswap V3 exactInputSingle must be seeded");
    assert_eq!(fs.name, "exactInputSingle");
    assert!(
        fs.signature.starts_with("exactInputSingle((") && fs.signature.ends_with("))"),
        "tuple-style ABI signature expected, got {}",
        fs.signature
    );
    assert_eq!(fs.source.as_deref(), Some("uniswap-v3-router"));
}

/// 미시드 selector → `None` (silent default not allowed).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn get_function_signature_unknown_selector_is_none() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let none = db::queries::get_function_signature(&pool, "0xdeadbeef")
        .await
        .expect("query ok");
    assert!(
        none.is_none(),
        "unknown selector must be None (caller surfaces explicit null)"
    );
}

/// 대소문자 무관 — `LOWER($1)` lookup 불변식.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn get_function_signature_case_insensitive_lookup() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let upper = db::queries::get_function_signature(&pool, "0xA9059CBB")
        .await
        .expect("query ok")
        .expect("upper-case selector must match seeded lower-case row");
    assert_eq!(upper.selector, "0xa9059cbb");
    assert_eq!(upper.name, "transfer");
}
