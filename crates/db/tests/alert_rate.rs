//! S14 (M005) — rate_threshold alert 통합 테스트 (실제 PostgreSQL 필요).
//!
//! `#[ignore]`. 실행: docker PG 기동 후 `cargo test -p db -- --ignored`.
//! 픽스처는 시드(~18M)와 분리된 높은 블록을 쓰고 끝에 원복(파괴적 rollback 회피).

use db::models::{Block, Transaction};

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";
const BASE_BLOCK: i64 = 97_000_001; // alerts.rs와 분리(98M), rollback과도 분리(99M)
const TX_PREFIX: &str = "0xa1e7c071";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn tx_hash(i: usize) -> String {
    // 12자 prefix + 0-padded counter → 0x + 64 hex
    let counter = format!("{:054x}", i);
    format!("{TX_PREFIX}{counter}")
}

/// 시간 윈도우 내 카운트가 threshold 이상이면 매칭에 잡힌다.
/// 그 후 dispatch 기록을 INSERT 하면 디바운스 안에서는 매칭에서 빠진다.
/// 디바운스 만료 후엔 다시 잡힌다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn rate_match_then_debounce_then_match_again() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    let now = chrono::Utc::now();
    let prior = db::queries::get_last_checkpoint(&pool, 1)
        .await
        .expect("checkpoint");

    // ── 픽스처: 분리된 높은 블록에 status=0 tx 3개 ──
    // `trg_transaction_check_failed` 트리거가 failed_transaction을 자동으로 만든다
    // (UNKNOWN 카테고리, ON CONFLICT DO NOTHING) — KNOWLEDGE Lesson [S08-T01].
    db::queries::insert_blocks(
        &pool,
        &[Block {
            block_number: BASE_BLOCK,
            timestamp: now,
            gas_used: 1,
            block_hash: Some("0xb14e".to_string()),
            parent_hash: Some("0xb13e".to_string()),
        }],
    )
    .await
    .expect("insert block");

    let txs: Vec<Transaction> = (0..3)
        .map(|i| Transaction {
            tx_hash: tx_hash(i),
            from_addr: "0xfeed".to_string(),
            to_addr: Some("0x00000000000000000000000000000000000000aa".to_string()),
            block_number: BASE_BLOCK,
            gas_used: 1,
            gas_price: bigdecimal::BigDecimal::from(0),
            value: bigdecimal::BigDecimal::from(0),
            status: 0, // 트리거가 failed_transaction을 자동 생성
            input_data: None,
        })
        .collect();
    db::queries::insert_transactions(&pool, &txs)
        .await
        .expect("insert tx");

    // ── 구독: 임계 2, 윈도우 60s, 디바운스 60s. category None = 모두 매칭. ──
    let sub = db::queries::insert_alert_subscription_rate(
        &pool,
        None,                                               // any category
        Some("0x00000000000000000000000000000000000000aa"), // 시드된 to_addr
        "https://example.com/hook-rate",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        2,  // threshold_count
        60, // threshold_window_secs
        60, // debounce_secs
    )
    .await
    .expect("insert rate sub");
    assert_eq!(sub.sub_type, "rate_threshold");
    assert_eq!(sub.threshold_count, Some(2));

    // ── 1차 매칭: count=3 >= threshold=2 ──
    let matches = db::queries::find_pending_rate_alert_matches(&pool, 10)
        .await
        .expect("find rate matches");
    let mine = matches
        .iter()
        .find(|m| m.subscription_id == sub.subscription_id)
        .expect("sub must match (count=3 >= threshold=2 within window)");
    assert!(
        mine.match_count >= 2,
        "match_count must be >= threshold; got {}",
        mine.match_count
    );
    assert_eq!(mine.threshold_count, 2);
    assert_eq!(mine.threshold_window_secs, 60);

    // ── 발송 기록 → 디바운스 시작 ──
    db::queries::record_rate_alert_dispatch(
        &pool,
        sub.subscription_id,
        mine.match_count as i32,
        true,
        None,
    )
    .await
    .expect("record dispatch");

    // ── 2차 매칭: 디바운스 안이므로 sub이 빠져야 함 ──
    let matches2 = db::queries::find_pending_rate_alert_matches(&pool, 10)
        .await
        .expect("find rate matches (debounced)");
    assert!(
        matches2
            .iter()
            .all(|m| m.subscription_id != sub.subscription_id),
        "sub must be filtered out within debounce window"
    );

    // ── teardown: 구독 삭제 → CASCADE로 dispatch 행도 정리, tx/block 원복 ──
    let deleted = db::queries::delete_alert_subscription(&pool, sub.subscription_id)
        .await
        .expect("delete sub");
    assert_eq!(deleted, 1);

    // 픽스처 tx + block 정리 (failed_transaction은 FK CASCADE 없음 — 명시 삭제)
    for i in 0..3 {
        let h = tx_hash(i);
        sqlx::query("DELETE FROM failed_transaction WHERE tx_hash = $1")
            .bind(&h)
            .execute(&pool)
            .await
            .expect("cleanup failed_tx");
        sqlx::query("DELETE FROM transaction WHERE tx_hash = $1")
            .bind(&h)
            .execute(&pool)
            .await
            .expect("cleanup tx");
    }
    sqlx::query("DELETE FROM block WHERE block_number = $1")
        .bind(BASE_BLOCK)
        .execute(&pool)
        .await
        .expect("cleanup block");

    // 체크포인트 원복 (트리거가 만진 게 없지만 안전망)
    if let Some(prev) = prior {
        db::queries::update_checkpoint(&pool, 1, prev)
            .await
            .expect("restore checkpoint");
    }
}

/// per_event sub은 rate matcher에서 빠져야 한다 (분리 검증).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn rate_matcher_ignores_per_event_subscriptions() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let sub = db::queries::insert_alert_subscription(
        &pool,
        None,
        None,
        "https://example.com/hook-per-event",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("insert per-event sub");
    assert_eq!(sub.sub_type, "per_event");
    assert!(sub.threshold_count.is_none());

    let matches = db::queries::find_pending_rate_alert_matches(&pool, 100)
        .await
        .expect("find rate matches");
    assert!(
        matches
            .iter()
            .all(|m| m.subscription_id != sub.subscription_id),
        "per_event sub must NOT appear in rate matcher results"
    );

    db::queries::delete_alert_subscription(&pool, sub.subscription_id)
        .await
        .expect("cleanup");
}

/// CHECK 제약: per_event sub은 rate 필드 없이 만들어지고 모두 NULL.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn per_event_sub_has_null_rate_fields_by_default() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let sub = db::queries::insert_alert_subscription(
        &pool,
        None,
        None,
        "https://example.com/hook-default",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("insert");
    assert_eq!(sub.sub_type, "per_event");
    assert!(sub.threshold_count.is_none());
    assert!(sub.threshold_window_secs.is_none());
    assert!(sub.debounce_secs.is_none());

    db::queries::delete_alert_subscription(&pool, sub.subscription_id)
        .await
        .expect("cleanup");
}
