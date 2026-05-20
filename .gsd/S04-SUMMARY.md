---
slice: S04
title: 임베드 가능화 + 하드닝 (M001 마무리)
status: done
edge: untapped
tasks: [T01, T02, T03]
gate: pass            # clippy 0 (db+api+tests), fmt ok, cargo test -p db --ignored 8/8, verify ALL PASS
artifacts:
  - crates/db/src/queries.rs            # L1 명시 컬럼 (get/list/trace)
  - crates/db/src/models.rs             # FailedTxDetail.call_tree_truncated
  - crates/api/src/routes/failed_tx.rs  # L2 is_tx_hash 400, L3 N+1 cap
  - crates/db/tests/failed_tx.rs        # trace_logs_respects_limit
  - scripts/verify-failed-tx.sh         # MALFORMED→400 케이스
  - docs/api-failed-tx.md               # 3 엔드포인트 통합 레퍼런스
  - README.md                           # Failure Intelligence API 포인터
---

# S04 — 무엇이 실제로 일어났나 (계획 대비)

S04-PLAN T01(하드닝)→T02(레퍼런스)→T03(Milestone Validate) 그대로.

- **T01**: L1 `SELECT *`→명시 컬럼(JSON 형태 무회귀 검증), L2 `0x`+64hex 검증으로
  malformed=400 / valid-but-absent=404 분리(regex 의존성 없이), L3 `list_trace_logs_by_tx`
  에 `limit` + N+1 잘림감지 + `call_tree_truncated`. `cargo test -p db --ignored` 8/8.
- **T02**: `docs/api-failed-tx.md`를 3 엔드포인트 단일 레퍼런스로 완결(400/404·cap·
  truncated 반영), README에 포인터 + 원커맨드. OpenAPI 프레임워크 미도입(D008).
- **T03**: M001 수용 기준을 새로 전체 재실행(누적 green 불신 — 환경 드리프트 교훈).
  전 항목 ✅ → M001-SUMMARY.md.

**Reassess**: ROADMAP S04 `[x]`, M001 `[x]` 출하. M002 분해 금지 해제.
