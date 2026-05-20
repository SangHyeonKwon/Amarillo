# S12 — 카테고리 진단 메시지 + 추천 액션 (M004 셋째 슬라이스) · PLAN

Slice 목표: REQUIREMENTS#M004의 3차 가산 — `/v1/failed-tx/{tx_hash}` 응답에
`diagnosis: { message, recommended_action?, source? } | null`을 추가한다.
`error_category` 6개(UNKNOWN/INSUFFICIENT_BALANCE/SLIPPAGE_EXCEEDED/DEADLINE_
EXPIRED/UNAUTHORIZED/TRANSFER_FAILED) 각각에 *사람이 읽는 진단 메시지*와
*추천 액션*을 매핑. dApp 개발자가 "왜 실패했나 → 어떻게 고치나"를 한 호출에서.

엣지: `[edge: weak-spot]`. risk: low-med. deps: M001~M003 + S10 + S11 (이미 머지됨).
**M004 셋째 슬라이스** — 누적의 3단계. S10 *어디서*, S11 *어떤 함수가*,
S12 *왜+어떻게*.

핵심 결정: **D016** (착수 시 기록) — enum *세분화*는 별 슬라이스(`S12.1`
sketch). 자기소유 시드 정책(D015/D008 일관). 외부 의존 미도입.

검증 제약(D009~D015 일관): T01 통합 PG + db lib clippy/fmt, T02 verify HTTP
+ api 통합, T03 web typecheck+vitest+build. 라이브 메인넷 회귀 부재 — 자기
시드 6 카테고리로 의미 단언.

태스크: T01 → T02 → T03.

---

## T01 — 마이그레이션 + 시드 + 모델 + DB 쿼리 + 통합테스트 + D016

**Must-haves**
- *Truths*
  - 멱등 마이그레이션 `migrations/20240107000001_add_category_diagnosis.sql`
    (`BEGIN/COMMIT` + `CREATE TABLE IF NOT EXISTS` + `COMMENT ON`):
    ```sql
    CREATE TABLE IF NOT EXISTS category_diagnosis (
      error_category     TEXT PRIMARY KEY,  -- SCREAMING_SNAKE wire form
      message            TEXT NOT NULL,
      recommended_action TEXT,
      source             TEXT,
      created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );
    ```
    + 시드 6행(`INSERT ... ON CONFLICT (error_category) DO NOTHING`):
    - UNKNOWN — "Couldn't classify the failure from the trace alone." /
      "Inspect `root_cause` and the call_tree; raise an issue with the tx hash."
    - INSUFFICIENT_BALANCE — "Sender lacks the balance needed." /
      "Verify the sender holds enough of the input token (or wrap ETH first)."
    - SLIPPAGE_EXCEEDED — "Trade output was below the minimum acceptable amount." /
      "Increase slippage tolerance, or split the trade to reduce price impact."
    - DEADLINE_EXPIRED — "Transaction mined after its deadline." /
      "Resubmit with a later deadline (or tighter gas-price target)."
    - UNAUTHORIZED — "Caller lacks permission (ownership/approval)." /
      "Approve the spender first, or confirm the caller is the owner."
    - TRANSFER_FAILED — "ERC-20 transfer reverted (returned false or threw)." /
      "Check balance, allowance, and whether the token has transfer hooks."
    `source = 'builtin'` 일률.
  - 모델 `CategoryDiagnosis` (FromRow + Serialize) + 응답용 합성 `Diagnosis
    { message, recommended_action, source }` (Serialize only, error_category·
    created_at 제외 — 응답 컨텍스트가 이미 카테고리를 보유).
  - 쿼리 `get_category_diagnosis(pool, error_category_wire: &str) -> Option<CategoryDiagnosis>`:
    파라미터화 SQL, 정확 매칭. 매칭 없으면 None.
  - 통합테스트 `crates/db/tests/category_diagnosis.rs`:
    - 시드 6 카테고리 모두 lookup → `Some` + 비-빈 message
    - 미존재 카테고리 `"NONEXISTENT"` → None
  - **D016 기록** — DECISIONS (스코프: 진단 메시지만, enum 세분화 별 슬라이스,
    자기시드 정책, TEXT PK 채택 이유).
  - prod `unwrap()` 0 / 파라미터화 SQL 100% / 신규 public `///` doc.
- *Artifacts*: `migrations/20240107000001_add_category_diagnosis.sql`,
  `crates/db/src/{models,queries}.rs`, `crates/db/tests/category_diagnosis.rs`,
  `.gsd/DECISIONS.md`
- *Key Links*: S09/S11 마이그레이션 멱등 + 시드 동봉 패턴, S10 모델 가산 +
  skip_* 금지 패턴, STH 통합테스트 하니스

## T02 — API 응답 가산 + verify + docs

**Must-haves**
- *Truths*
  - `FailedTxDetail`에 `diagnosis: Option<Diagnosis>` 가산 (`skip_serializing_if`
    **금지** — silent default 차단, S10/S11/D014 일관).
  - 핸들러: `failed.error_category`를 SCREAMING_SNAKE wire 형태로 변환하여
    `get_category_diagnosis` 호출 → `Some(Diagnosis{ message, recommended_
    action, source })`, 미매칭이면 `None`. ErrorCategory→wire 변환은 기존
    `error_category_to_sql`(crates/db/src/queries.rs) 또는 단순 match 활용.
  - `scripts/verify-failed-tx.sh`에 의미 단언 추가:
    - `diagnosis` 필드 존재 (`hasOwnProperty`)
    - null이거나 object
    - object면: `message`가 비-빈 문자열, `recommended_action`은 string 또는
      null, `source`는 string 또는 null. 시드된 카테고리에 대해서는 항상
      object여야 하므로 `data.failed.error_category`가 SCREAMING_SNAKE
      카테고리 6 중 하나면 not-null 단언.
  - `docs/api-failed-tx.md`에 `diagnosis` 절: 정의·예시 응답 갱신·자기시드
    정책 한 단락(D016).
- *Artifacts*: `crates/api/src/routes/failed_tx.rs`, `scripts/verify-failed-tx.sh`,
  `docs/api-failed-tx.md`
- *Key Links*: S10-PLAN T02 / S11-PLAN T02 가산성 패턴, S04 마일스톤 검증 Rule

## T03 — 프론트 "Diagnosis" 카드 + S12-SUMMARY

**Must-haves**
- *Truths*
  - `types.ts`: `Diagnosis { message, recommended_action: string | null,
    source: string | null }` + `FailedTxDetail.diagnosis: Diagnosis | null`.
  - `contract.ts`: `parseDiagnosis`(shape 단언) + `parseFailedTxDetail`이
    `"diagnosis" in obj`로 missing-key 거부(silent default 금지, S10/S11 일관).
  - `contract.test.ts`: 기존 정상 케이스에 `diagnosis: null` 추가 + 신규 3:
    - object 정상 (message/recommended_action/source)
    - missing key → throw `/diagnosis/`
    - malformed message non-string → throw `/message/`
  - `FailedTx.tsx`의 Tx inspection 카드에 "Diagnosis" 박스 추가 (Root cause
    아래, call_tree 위) — `diagnosis` 객체일 때 message + recommended_action을
    분리해 강조 표시. `null`이면 짧은 안내. 기존 Root cause/call_tree/Failing
    function/by-label/list 무회귀.
  - `.gsd/S12-SUMMARY.md` + ROADMAP M004 S12 `[x]`. M004는 `🚧 IN PROGRESS`
    유지 — S11.1 / S12.1 / S13 남음.
- *Reassess*: S12 출하 후 — S11.1(args 디코딩) / S12.1(enum 세분화) / S13(SDK)
  중 사용자 지시 대기. 새 Lesson은 KNOWLEDGE.
- *Artifacts*: `web/src/api/{types,contract}.ts`, `web/src/api/contract.test.ts`,
  `web/src/pages/FailedTx.tsx`, `.gsd/{S12-SUMMARY,M001-ROADMAP}.md`

---

## Slice 수용 (Complete)
- [ ] T01–T03 must-haves, 기존 `/v1/*`·`/alerts`·페이지 무회귀
- [ ] DB 통합(`-p db --ignored`) + indexer + db lib + clippy --workspace + fmt 모두 green
- [ ] `verify-failed-tx.sh` ALL PASS (신규 `diagnosis` semantics 단언 포함),
      `verify-alerts.sh` + `verify-failed-tx-by-label.sh` 무회귀
- [ ] `web` typecheck + test(diagnosis 신규 케이스 포함) + build 통과
- [ ] REQUIREMENTS#M004 S12 항목 ✅ + S12-SUMMARY + ROADMAP S12 `[x]`
- [ ] M004는 *진행 중* 유지 — S11.1/S12.1/S13 분해는 다음 지시에서만
