# STH — DB 통합 테스트 하네스 · PLAN

ROADMAP 백로그 `TEST-HARNESS` 승격. D007(S01에서 이월)을 해소한다.

목표: `crates/db`에 PostgreSQL 통합 테스트 하네스를 구축하고, **S01 H1 불변식
(트레이스 pre-order: root-first + trace_id 오름차순)을 실행 가능한 회귀 테스트**로 박는다.
쉘 스크립트(HTTP 레벨)는 유지하고, 그 아래 DB 레벨을 cargo 테스트로 보강.

관례(CLAUDE.md): PG 필요한 테스트는 `#[ignore]` + `cargo test -p db -- --ignored`.
`cargo test`(기본)는 PG 없이도 green 유지(테스트 0개 실행).

태스크: T01 → T02.

---

## T01 — Cargo dev-deps + 통합 테스트 작성

**Must-haves**
- *Truths*
  - `cargo test -p db` (기본) → 컴파일·통과 (ignored 테스트는 미실행)
  - `cargo test -p db -- --ignored` (docker PG 기동 시) → 아래 4개 통과:
    1. `get_failed_transaction` 시드 해시 → `Ok`, `tx_hash` 일치·`gas_used>0`
    2. `get_failed_transaction` 미존재 해시 → `Err(DbError::NotFound(_))`
    3. `list_trace_logs_by_tx` 시드 해시 → 비어있지 않고, `frames[0].call_depth==0`,
       `trace_id` **strictly ascending** (H1 불변식)
    4. `list_trace_logs_by_tx` 미존재 해시 → `Ok(빈 Vec)`
- *Artifacts*
  - `crates/db/Cargo.toml`: `[dev-dependencies] tokio = { workspace = true }`
  - `crates/db/tests/failed_tx.rs`: `#[tokio::test] #[ignore]` 4건 +
    `DATABASE_URL`(기본 docker URL) 헬퍼. 테스트라 `expect()` 허용(CLAUDE.md)
- *Key Links*
  - `db::create_pool`, `db::queries::{get_failed_transaction,list_trace_logs_by_tx}`,
    `db::error::DbError`, `db::models::FailedTransaction/TraceLog` (전부 pub)
  - 픽스처: GOOD `0xdead…0001`(failed+trace 2프레임), BAD `0x0000…0000`

## T02 — 검증 + GSD/문서 정합

**Must-haves**
- *Truths*
  - `cargo test -p db` green / `cargo test -p db -- --ignored` (docker PG) green
  - `cargo clippy -p db -- -D warnings` / `cargo fmt --check` 통과
- *Artifacts*
  - `docs/api-failed-tx.md` "Verify"에 `cargo test -p db -- --ignored` 추가
  - GSD 정합: `DECISIONS.md` D007 → RESOLVED, `M001-ROADMAP.md` 백로그 체크,
    `KNOWLEDGE.md` Lesson(통합테스트 관례 확립·M1 종결), `STH-SUMMARY.md`
- *Key Links*: 기존 `verify-failed-tx.sh`는 HTTP 레벨로 유지(상호보완) — 명시

---

## Slice 수용 (Complete 게이트)
- [ ] T01·T02 must-haves 충족
- [ ] `cargo test -p db` + `--ignored`(docker PG) green, clippy/fmt 통과
- [ ] D007 RESOLVED·ROADMAP 백로그 해소, Reassess 후 S02 진입 가능
