# S10 — 콜트리 루트코즈 어트리뷰션 (M004 첫 슬라이스) · PLAN

Slice 목표: REQUIREMENTS#M004의 1차 가산 — `/v1/failed-tx/{tx_hash}` 응답에
"실제로 어디서 revert가 났는가"를 정확하게 짚는 `root_cause` 필드를 추가한다.
기존 `call_tree` 배열 + `call_tree_truncated` 계약은 불변(D004); 새 필드만 가산.

엣지: `[edge: untapped]`. risk: med. deps: M001~M003 (이미 머지됨).
**M004 첫 슬라이스** — 본 슬라이스 출하는 M004 마감이 아니라 *누적의 시작*.

핵심 결정: **D014** (이미 기록) — M004 방향과 dApp 개발자 페르소나, S11/S12/S13
탈락-후보 비교, 스코프 D003 유지.

검증 제약(D009~D013 일관): T01은 통합 PG + db lib clippy/fmt, T02는 verify HTTP
+ api 통합, T03은 web typecheck+vitest+build. 라이브 메인넷 회귀는 불가능 —
docker 시드 + S01 절차로 의미 단언, 메인넷은 수동.

태스크: T01 → T02 → T03.

---

## T01 — DB 쿼리 + 모델 확장 + 통합테스트

**Must-haves**
- *Truths*
  - **Lesson 활용**: S01의 *trace_id ASC = pre-order DFS* 불변식(인덱서가 콜트리를
    flatten하며 삽입한 순서를 BIGSERIAL에 보존). `trace_id ASC LIMIT 1`이 정확히
    "맨 처음 발생한 revert frame"을 잡는다. 다른 정렬(call_depth) 금지.
  - 신규 DB 함수 `get_first_error_frame(pool, tx_hash) -> Option<TraceLog>`:
    ```sql
    SELECT * FROM trace_log
    WHERE tx_hash = $1 AND error IS NOT NULL
    ORDER BY trace_id ASC
    LIMIT 1
    ```
    파라미터화. error IS NOT NULL이 없으면 None (시드 데이터/인덱서 미경유 케이스).
  - `FailedTxDetail` 모델에 `root_cause: Option<TraceLog>` 필드 추가
    (`crates/db/src/models.rs`). `#[serde(skip_serializing_if)]` **금지** —
    미존재도 명시 `null` 노출(silent failure 차단; D004 일관).
  - `get_failed_transaction` (또는 호출처)에서 새 쿼리 추가 호출 → `FailedTxDetail`
    조립. 기존 `call_tree`/`call_tree_truncated` 채움 로직 변경 없음.
  - 통합테스트 (`crates/db/tests/failed_tx.rs`에 추가 또는 신규 `root_cause.rs`):
    - 시드 tx의 첫 error frame `trace_id` = `root_cause.trace_id` (불변식)
    - error 없는 시드 tx → `root_cause = None`
    - call_tree의 첫 error frame과 root_cause가 동일 (자기 일관성)
  - prod `unwrap()` 0 / 파라미터화 SQL 100% / 신규 public `///` doc.
- *Artifacts*: `crates/db/src/queries.rs`, `crates/db/src/models.rs`,
  `crates/db/tests/{failed_tx,root_cause}.rs`
- *Key Links*: S01-PLAN(`list_trace_logs_by_tx`), KNOWLEDGE "trace_id ASC 불변식
  (Lesson S01)", STH 통합테스트 하니스

## T02 — API 응답 + verify 스크립트 + docs

**Must-haves**
- *Truths*
  - `/v1/failed-tx/{tx_hash}` 핸들러는 모델만 손대면 자동 통과 — `FailedTxDetail`
    Serialize가 새 필드를 그대로 직렬화. 단 응답 envelope `ApiResponse<FailedTxDetail>`
    + 기존 404 계약 불변. 추가 핸들러 코드 0이 목표(가산성 검증).
  - `scripts/verify-failed-tx.sh`에 의미 단언 추가 (S04 Rule + S01 리뷰 Lesson):
    - 필드 존재(`hasOwnProperty('root_cause')`)
    - 값이 null이거나 객체
    - 객체이면 `call_tree`에서 첫 `error !== null` frame의 `trace_id`와 일치
    - 객체이면 `error` 필드가 non-null (정의상 보장)
  - `docs/api-failed-tx.md`에 `root_cause` 필드 절 추가: 정의, 예시 응답 갱신,
    "call_tree와의 관계" 한 단락(전체 평탄 트리 vs 첫 error frame), Dune이
    구조적으로 못 하는 영역(per-frame trace 분석) 한 줄 framing.
- *Artifacts*: `crates/api/src/routes/failed_tx.rs`(필요 시 doc만),
  `scripts/verify-failed-tx.sh`, `docs/api-failed-tx.md`
- *Key Links*: S04 "마일스톤 검증 = 게이트 전체 재실행" Rule, S01 리뷰 "shape
  단언 아니라 semantics 단언" Lesson

## T03 — 프론트 "Root cause" 카드 + S10-SUMMARY

**Must-haves**
- *Truths*
  - `web/src/api/types.ts`: `FailedTxDetail`에 `root_cause: TraceLog | null` 추가.
  - `web/src/api/contract.ts` `parseFailedTxDetailEnvelope`: 신규 필드 파싱 +
    null/객체 허용, 객체이면 TraceLog shape 단언(`trace_id`/`call_depth`/`error`
    등). 누락 시 throw(silent default 금지).
  - `web/src/api/contract.test.ts`: 신규 케이스 — root_cause 객체와 null 둘 다,
    악성 shape(`trace_id` 문자열 등)이면 throw.
  - `FailedTx.tsx` 단건 화면에 "Root cause" 카드 추가: root_cause 객체일 때
    frame 1개를 크게 강조(call_depth 배지, call_type, from→to mid-trunc,
    selector/input 일부, error 메시지). null이면 짧은 안내("Indexer did not
    record a per-frame error for this transaction"). 기존 call_tree 카드 무회귀.
  - `.gsd/S10-SUMMARY.md` + ROADMAP M004 S10 `[x]` 표시 (M004는 `🚧 IN PROGRESS`
    유지 — S11/S12 남음).
- *Reassess*: S10 출하 후 — S11 `[sketch]` 해제(분해)할지, S12로 갈지 사용자
  지시 대기. 새 Lesson은 KNOWLEDGE에 기록. M004 마감 *아님* — 누적의 1단계.
- *Artifacts*: `web/src/api/{types,contract,hooks}.ts`,
  `web/src/api/contract.test.ts`, `web/src/pages/FailedTx.tsx`,
  `.gsd/S10-SUMMARY.md`, `.gsd/M001-ROADMAP.md`
- *Key Links*: FE-WIRE-PLAN(파서·hook 추가 패턴), S04 마일스톤 검증 절차

---

## Slice 수용 (Complete)
- [ ] T01–T03 must-haves, 기존 `/v1/*`·`/alerts`·페이지 무회귀
- [ ] DB 통합(`-p db --ignored`) + indexer + db lib + clippy --workspace + fmt 모두 green
- [ ] `verify-failed-tx.sh` ALL PASS (신규 root_cause semantics 단언 포함),
      `verify-alerts.sh` + `verify-failed-tx-by-label.sh` 무회귀
- [ ] `web` typecheck + test(root_cause 신규 케이스 포함) + build 통과
- [ ] REQUIREMENTS#M004 S10 항목 ✅ + S10-SUMMARY + ROADMAP S10 `[x]`
- [ ] M004는 *진행 중* 유지 — S11/S12 분해는 다음 지시에만
