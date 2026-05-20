//! S09 / M003: contract_label × failed_tx aggregate 통합 테스트.
//!
//! `#[ignore]`. 실행: docker PG 기동 후 `cargo test -p db -- --ignored`.
//! 픽스처는 시드(~18M)와 alerts(98M)/rollback(99M) 테스트와도 분리된 높은
//! 블록(97M)을 쓰고, 끝에 라벨·블록·체크포인트를 원복한다.

use db::models::{Block, Transaction};

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";
const BLOCK: i64 = 97_000_001; // 시드 + alerts(98M) + rollback(99M) 분리
const TXH_A: &str = "0xc40bec0000000000000000000000000000000000000000000000000000000001";
const TXH_B: &str = "0xc40bec0000000000000000000000000000000000000000000000000000000002";
const LABEL_PUBLIC_ADDR: &str = "0xaabb000000000000000000000000000000000000";
const LABEL_ALICE_ADDR: &str = "0xccdd000000000000000000000000000000000000";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn tx_fixture(hash: &str, to: &str) -> Transaction {
    Transaction {
        tx_hash: hash.to_string(),
        from_addr: "0x01".to_string(),
        to_addr: Some(to.to_string()),
        block_number: BLOCK,
        gas_used: 1,
        gas_price: bigdecimal::BigDecimal::from(0),
        value: bigdecimal::BigDecimal::from(0),
        status: 0, // trigger creates failed_transaction(UNKNOWN)
        input_data: None,
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn failed_tx_by_label_pivots_categories_and_filters_by_owner() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    let prior = db::queries::get_last_checkpoint(&pool, 1)
        .await
        .expect("read checkpoint");
    let now = chrono::Utc::now();

    // ── labels: one public + one tenant-owned ──
    db::queries::insert_contract_label(&pool, LABEL_PUBLIC_ADDR, "Public test", None)
        .await
        .expect("insert public label");
    db::queries::insert_contract_label(&pool, LABEL_ALICE_ADDR, "Alice test", Some("alice"))
        .await
        .expect("insert alice label");

    // ── fixture block + two failed txs targeting each label ──
    db::queries::insert_blocks(
        &pool,
        &[Block {
            block_number: BLOCK,
            timestamp: now,
            gas_used: 1,
            block_hash: Some("0xc40b".to_string()),
            parent_hash: Some("0xc40a".to_string()),
        }],
    )
    .await
    .expect("insert block");
    db::queries::insert_transactions(
        &pool,
        &[
            tx_fixture(TXH_A, LABEL_PUBLIC_ADDR),
            tx_fixture(TXH_B, LABEL_ALICE_ADDR),
        ],
    )
    .await
    .expect("insert tx");

    // (1) owner=None: both labels appear (plus any pre-existing seed labels —
    //     don't assert exact count; only that ours are present).
    let all = db::queries::failed_tx_by_label_aggregate(&pool, None, None, None, 1000)
        .await
        .expect("aggregate all");
    let public_row = all
        .iter()
        .find(|p| p.address == LABEL_PUBLIC_ADDR)
        .expect("public label in result");
    assert_eq!(public_row.total_failures, 1);
    assert_eq!(public_row.label, "Public test");
    assert_eq!(public_row.by_category.get("UNKNOWN").copied(), Some(1));
    let alice_in_all = all
        .iter()
        .find(|p| p.address == LABEL_ALICE_ADDR)
        .expect("alice label in result (owner=None matches everything)");
    assert_eq!(alice_in_all.total_failures, 1);

    // (2) owner=Some("alice"): only alice's label shows; public is excluded.
    let alice_only =
        db::queries::failed_tx_by_label_aggregate(&pool, Some("alice"), None, None, 1000)
            .await
            .expect("aggregate alice");
    assert!(
        alice_only.iter().all(|p| p.address == LABEL_ALICE_ADDR),
        "owner=alice must only return alice-owned labels"
    );
    assert_eq!(alice_only.len(), 1);
    assert_eq!(alice_only[0].total_failures, 1);

    // (3) owner=Some("nobody"): no matches → empty.
    let nobody = db::queries::failed_tx_by_label_aggregate(&pool, Some("nobody"), None, None, 1000)
        .await
        .expect("aggregate nobody");
    assert_eq!(nobody.len(), 0);

    // (4) future window: from > now → empty regardless of owner.
    let future_from = now + chrono::Duration::days(365);
    let future =
        db::queries::failed_tx_by_label_aggregate(&pool, None, Some(future_from), None, 1000)
            .await
            .expect("aggregate future");
    assert!(
        future
            .iter()
            .all(|p| { p.address != LABEL_PUBLIC_ADDR && p.address != LABEL_ALICE_ADDR }),
        "future window must exclude our just-inserted fixtures"
    );

    // ── teardown ──
    db::queries::delete_contract_label(&pool, LABEL_PUBLIC_ADDR)
        .await
        .expect("delete public label");
    db::queries::delete_contract_label(&pool, LABEL_ALICE_ADDR)
        .await
        .expect("delete alice label");
    db::queries::rollback_from_block(&pool, BLOCK)
        .await
        .expect("rollback fixtures");
    if let Some(p) = prior {
        db::queries::update_checkpoint(&pool, 1, p)
            .await
            .expect("restore checkpoint");
    }
}
