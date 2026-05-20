---
slice: S10
title: 콜트리 루트코즈 어트리뷰션 (M004 첫 슬라이스)
status: done
edge: untapped
milestone: M004
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 15/15 (failed_tx 10 — 신규 root_cause 2 포함) · verify 3종 ALL PASS · web typecheck/test/build OK
decision: D014
artifacts:
  - crates/db/src/queries.rs                    # get_first_error_frame (trace_id ASC LIMIT 1)
  - crates/db/src/models.rs                     # FailedTxDetail.root_cause: Option<TraceLog>
  - crates/db/tests/failed_tx.rs                # 신규 통합테스트 2 (자기일관성 + unknown=None)
  - crates/api/src/routes/failed_tx.rs          # 핸들러 가산 1줄 (root_cause 조립)
  - scripts/verify-failed-tx.sh                 # node 의미 단언 (null/object + trace_id 일치)
  - docs/api-failed-tx.md                       # root_cause 절 + example 갱신 + Dune-can't 단락
  - web/src/api/{types,contract}.ts             # root_cause 타입 + 파서 (missing key 거부)
  - web/src/api/contract.test.ts                # 신규 케이스 3 (object/missing/malformed) + 기존 1 수정
  - web/src/pages/FailedTx.tsx                  # "Root cause" 강조 블록 (Tx inspection 안)
  - .gsd/DECISIONS.md                           # D014 (M004 방향 + dApp 개발자 페르소나)
verification_constraint: "M004는 *완료 아님* — S10 출하만으로 마감되지 않음. S11/S12/S13 분해는 다음 지시에서. 라이브 메인넷 tx 자동 회귀는 불가능 — 시드 GOOD tx의 trace.error frame이 의미 검증의 기반(KNOWLEDGE S01 Lesson)."
---

# S10 — 무엇이 실제로 일어났나

REQUIREMENTS#M004 1차 가산 — `/v1/failed-tx/{tx_hash}` 응답에 *어디서 revert가
실제로 발생했는지*를 정확히 짚는 `root_cause` 필드를 추가했다. 기존
`call_tree`/`call_tree_truncated` 계약은 불변(D004); 새 필드만 가산.

- **T01 (DB 쿼리 + 모델 + 통합테스트)**: `get_first_error_frame(pool, tx_hash)
  -> Option<TraceLog>`(`error IS NOT NULL ORDER BY trace_id ASC LIMIT 1`).
  Lesson S01의 trace_id ASC 불변식(인덱서 pre-order DFS) 그대로 활용 —
  `trace_id` 단조성이 "맨 처음 발생한 revert frame"의 정의가 된다.
  `FailedTxDetail`에 `root_cause: Option<TraceLog>` 추가, `skip_serializing_if`
  **금지**(명시 `null` — silent default 차단). 핸들러는 1줄(`get_first_error_frame`
  호출) 가산. 통합테스트 2건:
  - `first_error_frame_matches_call_tree_first_error`: `list_trace_logs_by_tx`로
    받은 frames에서 첫 `error.is_some()` frame을 직접 찾아 비교 → 자기일관성 단언.
  - `first_error_frame_unknown_hash_is_none`: BAD tx → None(에러 아님).

- **T02 (API 응답 + verify + docs)**: 모델 변경만으로 `FailedTxDetail` Serialize가
  새 필드를 자동 직렬화 — 핸들러는 채움 1줄만. `scripts/verify-failed-tx.sh`에
  node 의미 단언 추가(S01 리뷰 Lesson "shape 아닌 semantics"):
  - `root_cause` 필드 존재 (`hasOwnProperty`)
  - 값이 null이거나 object
  - object면 `error` 비-null + `call_tree`의 첫 error frame `trace_id`와 일치
  실측: GOOD tx에서 `ROOT OK (trace_id=16 matches first error frame in call_tree)`.
  `docs/api-failed-tx.md`에 `root_cause` 절(정의·예시 응답 갱신·"Dune이 못
  하는" 단락) 추가.

- **T03 (web + S10 출하)**: `FailedTxDetail.root_cause: TraceLog | null` 추가.
  `parseFailedTxDetail`가 `"root_cause" in obj`로 키 존재를 명시 검증 → silent
  default 차단. `contract.test.ts`에 3 신규 케이스 + 기존 정상 케이스에
  `root_cause: null` 가산:
  - root_cause object 정상 → trace_id 4 일치, error 비-null
  - root_cause key 누락 → throw `/root_cause/`
  - root_cause.trace_id 비-숫자 → throw `/trace_id/`
  FailedTx 페이지의 Tx inspection 카드 안에 "Root cause" 강조 블록 추가 —
  객체면 frame 강조(depth, type, from→to, selector, error), null이면 짧은
  안내(silent default 금지 명시). 기존 call_tree 리스트·페이지네이션·by-label
  카드 무회귀.

**해자(D002·D014)의 *다음* 깊이가 코드로 박힘**
- 단건 진단 응답이 한 호출 안에서 점점 똑똑해지는 누적의 1단계. 클라이언트는
  call_tree 배열을 스캔하지 않고 `root_cause` 1필드로 "어디서 났나"를 받음.
- per-frame trace 분석은 Dune 모델 밖(공개 인덱싱 데이터에 trace.error가
  없음) → 차별 자산이 응답에 직접 노출됨.

**정직한 한계**
- 시드 GOOD tx에 error frame이 박혀 있다는 KNOWLEDGE Lesson(S01 실측)에 의존.
  시드가 바뀌어 *양쪽 다* None이 되면 통합테스트가 자동 통과 → 자기일관성은
  유지되지만 의미 검증이 약해진다. 라이브 메인넷 회귀는 환경 부재로 불가능.
- root_cause는 *첫 error frame*일 뿐 — "왜 그 frame이 실패했나"(decoded args /
  분류 정확도)는 S11/S12 영역. depth 0의 root revert와 depth N의 internal
  revert를 동등하게 첫 발생점으로 노출 — dApp 디버깅엔 충분, 자동 카테고리
  재분류엔 부족.
- 인증 미연결(D008 일관). `/v1/failed-tx/{tx_hash}`는 여전히 인증 없음.

**M004 진행**
- S10 ✅ 본 슬라이스
- S11 / S12 / S13 `[sketch]` 유지 — 다음 지시에서 분해 (GSD-2: 슬라이스 단위 호흡).
  후보:
  - S11 함수 selector → 함수명 + decoded args (failing_function 가독화)
  - S12 카테고리 세분화 v2 + 진단 메시지/추천액션
  - S13 개발자 SDK/문서 (TS/Python 미니멈 클라이언트, 프로덕트화)

**Reassess**: ROADMAP M004 S10 `[x]`, M004는 `🚧 IN PROGRESS` 유지. 새 Lesson
은 누적 시점에 KNOWLEDGE. 백로그(DNS-rebinding SSRF / 임계율 집계 / Pools·
Traders 매핑)는 단독 단위 유지 — 진단 깊이 누적과 직교.
