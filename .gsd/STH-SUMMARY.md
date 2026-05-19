---
slice: STH
title: DB 통합 테스트 하네스
status: done
origin: ROADMAP backlog (D007 이월) — M1 종결
tasks: [T01, T02]
gate: pass            # cargo test (0 ignored, green) · --ignored 4 passed · clippy 0 · fmt ok
artifacts:
  - crates/db/Cargo.toml            # + [dev-dependencies] tokio
  - crates/db/tests/failed_tx.rs    # 4 integration tests (#[ignore])
  - docs/api-failed-tx.md           # Verify: 2-layer (HTTP + db)
  - .gsd/DECISIONS.md               # D007 RESOLVED
  - .gsd/M001-ROADMAP.md            # backlog TEST-HARNESS [x]
  - .gsd/KNOWLEDGE.md               # 통합테스트 관례 Lesson
run: "cargo test -p db -- --ignored   # docker PG 기동 전제"
---

# STH — 무엇이 실제로 일어났나 (계획 대비)

D007로 S01에서 이월했던 정식 테스트 갭(M1)을 해소. 계획(STH-PLAN) T01→T02 그대로.

**계획대로 된 것**
- `crates/db/tests/failed_tx.rs` 통합테스트 4건, 전부 `#[ignore]`:
  `get_failed_transaction` Ok / NotFound, `list_trace_logs_by_tx` pre-order 불변식 /
  미존재 빈 Vec. `cargo test -p db`(기본) = 0 실행·green(CI 무 PG 유지),
  `--ignored`(docker PG) = **4 passed**.
- 통합테스트 크레이트가 `db` 일반 deps를 미상속하는 점을 반영해
  `[dev-dependencies] tokio`만 추가, `db::create_pool` 인라인으로 sqlx 타입명 회피
  (sqlx dev-dep 불요).

**핵심 가치 (왜 이걸 먼저 했나)**
- S01 H1(트레이스 정렬)이 이제 **실행 가능한 회귀 테스트**(`trace_logs_preserve_pre_order_invariant`).
  옛 `ORDER BY call_depth` 구현에서 반드시 실패하는 형태로 박음 → 조용한 재발 차단.
- 검증 2계층 확립: 스크립트=HTTP end-to-end, cargo test=db 쿼리/불변식. 상호보완.

**계획과 달랐던 것**
- 없음(설계대로). fmt가 `assert_eq!`를 멀티라인으로 강제 → 자동교정(공백만, 동작 불변,
  재실행 4 passed 재확인).

**Reassess**: D007 RESOLVED, ROADMAP 백로그 TEST-HARNESS 종결. M1 닫힘.
다음: S02(실패 목록/검색 API) — S02-PLAN.md 분해 완료 상태 그대로. M002/M003은
M001 출하 전 분해 금지(GSD-2) 유지.
