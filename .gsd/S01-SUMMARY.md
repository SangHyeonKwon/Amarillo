---
slice: S01
title: 단건 실패 진단 엔드포인트
status: done
edge: untapped
tasks: [T01, T02, T03]
gate: pass            # clippy 0 warnings, fmt ok, verify script green, no prod unwrap
migrations: none      # D004 — data already present
artifacts:
  - crates/db/src/models.rs         # + FailedTxDetail
  - crates/db/src/queries.rs        # + get_failed_transaction, list_trace_logs_by_tx
  - crates/api/src/routes/failed_tx.rs
  - crates/api/src/routes/mod.rs    # + module + route
  - scripts/verify-failed-tx.sh
  - docs/api-failed-tx.md
fixtures:
  good_hash: "0xdead000000000000000000000000000000000000000000000000000000000001"
  bad_hash:  "0x0000000000000000000000000000000000000000000000000000000000000000"
---

# S01 — 무엇이 실제로 일어났나 (계획 대비)

계획대로 3태스크 순서(DB → API → 검증) 진행. 마이그레이션·프론트 없음(D004, 범위 분리).

**계획대로 된 것**
- `failed_transaction`(1) ↔ `trace_log`(N)을 JOIN 대신 쿼리 2개로 뽑아 `FailedTxDetail`로
  Rust에서 조립. 기존 `get_pool_by_address`의 `fetch_optional → ok_or_else(NotFound)` 패턴 재사용.
- `DbError::NotFound → ApiError::NotFound(404)` 기존 `From` 임플로 404가 공짜로 떨어짐 —
  핸들러에서 상태코드 미터치.
- 라이브 검증: GOOD 200(failed+call_tree 정렬), BAD 404 `{error}` 단언 통과.

**계획과 달랐던/배운 것 (→ KNOWLEDGE.md 반영)**
- 시드 `failed_transaction`은 미분류(`Unknown`/`revert_reason=null`)인데 `trace_log.error`에
  `"Too little received"`가 존재 → **단건 진단이 집계보다 정보량 우위**. D002 제품가설 첫 실증.
- `cargo fmt`가 `list_trace_logs_by_tx` 시그니처를 1줄로 강제(99자, max_width 100 이내).
  자동교정으로 해결. (다인자 시그니처는 100자 경계 의식)
- docker `api` 컨테이너는 구 이미지 → 코드 변경 검증은 로컬 바이너리(:3001)로.
  `verify-failed-tx.sh`가 절차 캡슐화. 신규 슬라이스 재사용.

**다음 (Reassess 결과)**: S02 `[sketch]` 해제, S02-PLAN.md로 태스크 분해 완료.
M002/M003은 M001 출하 전까지 분해 금지(GSD-2).

## 리뷰 후 교정 (post-review)

슬라이스 리뷰에서 결함 발견 → 즉시 교정 (S02 진입 전).

- **H1 (High, 수정완료)**: `list_trace_logs_by_tx`가 `ORDER BY call_depth ASC, trace_id ASC`
  → 형제 서브트리가 섞여 콜트리 복원 불가. `ORDER BY trace_id ASC`(pre-order DFS)로 수정.
  doc 주석 + `docs/api-failed-tx.md` 정정. `verify-failed-tx.sh`에 node 순서 단언 추가 →
  재검증 `ORDER OK`. 근본원인: 검증이 shape만 봄 → KNOWLEDGE Lesson 등록.
- **M1 (Medium, 이월)**: 계획된 cargo 통합테스트 미구축. D007로 정식 결정·이월,
  ROADMAP 백로그 `TEST-HARNESS`로 추적. 회귀가드는 스크립트 의미단언으로 임시 확보.
- **L1–L4**: L4(스크립트가 빌드에러 은폐) 같이 수정. L1(`SELECT *`)·L2(잘못된 해시 400)·
  L3(콜트리 크기 무제한)은 S04 하드닝 백로그로 기록, S01 범위 외(의도적).

게이트 재확인: clippy 0 · fmt OK · `verify-failed-tx.sh`(단건+순서+404) green.
