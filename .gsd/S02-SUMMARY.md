---
slice: S02
title: 실패 목록/검색 API (정확한 total)
status: done
edge: weak-spot
tasks: [T01, T02, T03]
gate: pass            # clippy 0 (db+api+tests), fmt ok, verify green, cargo test -p db --ignored 6/6
migrations: none
artifacts:
  - crates/db/src/queries.rs        # + list_failed_transactions, count_failed_transactions
  - crates/db/src/models.rs         # + impl FromStr for ErrorCategory
  - crates/db/Cargo.toml            # + chrono dev-dep (시간 필터 테스트용)
  - crates/db/tests/failed_tx.rs    # + 2 integration tests (필터/total, 미래구간)
  - crates/api/src/response.rs      # + TotalPaginatedResponse, PaginationMeta
  - crates/api/src/routes/failed_tx.rs   # + list_failed_tx, parse_ts, FailedTxQuery
  - crates/api/src/routes/mod.rs    # + /failed-tx route
  - scripts/verify-failed-tx.sh     # + list/total/400 assertions
  - docs/api-failed-tx.md           # + GET /v1/failed-tx 섹션
---

# S02 — 무엇이 실제로 일어났나 (계획 대비)

S02-PLAN T01→T02→T03 그대로. 마이그레이션 없음(기존 테이블만).

**계획대로 된 것**
- DB: `list_failed_transactions`/`count_failed_transactions` — 옵션 필터를
  `($1::TEXT IS NULL OR ...)` 관용구로 단일 prepared statement(동적 SQL 금지, CLAUDE 규칙).
  list/count 분리로 `LIMIT` 독립 `total` 확보.
- API: `TotalPaginatedResponse` **신규 봉투**(기존 `PaginatedResponse` 불변, D005).
  입력 검증을 엣지로: 잘못된 `category`/RFC3339 → **400**(404 아님).
  `ErrorCategory: FromStr`을 str→enum 단일 출처로 추가.
- 검증: 통합테스트 2건 추가(필터/total 불변식, 미래구간 0건) → `cargo test -p db --ignored`
  6/6. 스크립트 라이브: `?category=UNKNOWN&limit=2` → `total=3, returned=2`,
  `?category=BOGUS`/`?from=not-a-date` → 400. S01 단건+순서+404 무회귀.

**중간에 막혔다 푼 것 (Lesson → KNOWLEDGE)**
- 작업 중 Docker 데몬이 꺼져 통합테스트 6건이 30s 타임아웃으로 전부 실패 → **코드 결함이
  아니라 환경**. `cargo test --no-run`으로 코드 무결성 분리 확인 후, Docker 복구·`pgdata`
  볼륨 보존 확인(failed=3) → 재실행 6/6. 교훈: 실패 시 환경/코드부터 분리 진단.
- 통합테스트 크레이트가 일반 deps 미상속 → `chrono`도 dev-dep로 추가(시간필터 테스트).
- rustfmt가 다인자 호출/`assert!`를 멀티라인 강제 → 자동교정(공백, 동작 불변).

**Reassess**: ROADMAP S02 `[x]`, S03 `[sketch]` 해제·S03-PLAN.md 분해 완료.
D005 REALIZED. M002/M003은 M001 출하 전 분해 금지 유지.
