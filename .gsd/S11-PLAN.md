# S11 — selector → 함수명 + signature 디코딩 (M004 둘째 슬라이스) · PLAN

Slice 목표: REQUIREMENTS#M004의 2차 가산 — `/v1/failed-tx/{tx_hash}` 응답에
`failing_function_decoded: { selector, name, signature, source? } | null`을
추가한다. 4바이트 selector(`0xa9059cbb`)를 사람이 읽는 함수명/시그니처로 매핑.
기존 `failing_function`(selector 그대로) 계약 불변(D004); 새 필드만 가산.

엣지: `[edge: weak-spot]`. risk: med. deps: M001~M003 + S10 (이미 머지됨).
**M004 둘째 슬라이스** — 누적의 2단계. S10이 *어디서*, S11이 *어떤 함수가*.

핵심 결정: **D015** (착수 시 기록) — args 디코딩은 별 슬라이스(`S11.1`
sketch). 자기소유 ABI 시드만, 외부 4byte.directory 미도입.

검증 제약(D009~D014 일관): T01 통합 PG + db lib clippy/fmt, T02 verify HTTP
+ api 통합, T03 web typecheck+vitest+build. 라이브 메인넷 회귀는 환경 부재로
불가능 — 자기 시드 selector로 의미 단언.

태스크: T01 → T02 → T03.

---

## T01 — 마이그레이션 + 시드 + 모델 + DB 쿼리 + 통합테스트 + D015

**Must-haves**
- *Truths*
  - 멱등 마이그레이션 `migrations/20240106000001_add_function_signature.sql`
    (`BEGIN/COMMIT` + `CREATE TABLE IF NOT EXISTS` + `COMMENT ON`):
    ```sql
    CREATE TABLE IF NOT EXISTS function_signature (
      selector   TEXT PRIMARY KEY,         -- '0x' + 8 hex, lowercased
      name       TEXT NOT NULL,            -- 'transfer'
      signature  TEXT NOT NULL,            -- 'transfer(address,uint256)'
      source     TEXT,                     -- 'erc20' | 'uniswap-v3-router' | ...
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );
    ```
    시드(`INSERT ... ON CONFLICT (selector) DO NOTHING` 동봉): ERC20 5
    (transfer / approve / transferFrom / balanceOf / allowance), Uniswap V3
    SwapRouter 4 (exactInputSingle / exactOutputSingle / exactInput /
    exactOutput) + multicall, Factory createPool, Pool mint/burn/collect,
    WETH9 deposit/withdraw — 합 ~13건. selector 값은 SQL 내 주석으로 출처
    문구 명시(자기 검증).
  - 모델 `FunctionSignature` (FromRow + Serialize) — 테이블 1행. 응답용 합성
    `DecodedFunction { selector, name, signature, source: Option<String> }`
    (Serialize only).
  - 쿼리 `get_function_signature(pool, selector: &str) -> Option<FunctionSignature>`:
    파라미터화 SQL, lowercased lookup (`SELECT … WHERE selector = LOWER($1)`).
    매칭 없으면 None. 호출자는 결과를 DecodedFunction으로 변환(name/sig 채움).
  - 통합테스트 `crates/db/tests/function_signature.rs`:
    - 시드된 selector(`0xa9059cbb`) → name='transfer', signature 비-빈
    - 미시드 selector(`0xdeadbeef`) → None
    - 대문자 입력(`0xA9059CBB`)도 매칭(lowercased lookup invariant)
  - **D015 기록** — DECISIONS에 결정 명시(args 보류, 자기 시드 정책, 검증 제약).
  - prod `unwrap()` 0 / 파라미터화 SQL 100% / 신규 public `///` doc.
- *Artifacts*: `migrations/20240106000001_add_function_signature.sql`,
  `crates/db/src/{models,queries}.rs`, `crates/db/tests/function_signature.rs`,
  `.gsd/DECISIONS.md`
- *Key Links*: S09 마이그레이션 멱등 + 시드 동봉 패턴, S10 모델 가산 + skip_*
  금지 패턴, STH 통합테스트 하니스, KNOWLEDGE 마이그레이션 BEGIN/COMMIT 중첩 OK

## T02 — API 응답 가산 + verify + docs

**Must-haves**
- *Truths*
  - `FailedTxDetail`에 `failing_function_decoded: Option<DecodedFunction>` 추가
    (`skip_serializing_if` **금지** — silent default 차단, S10/D014 일관).
  - 핸들러: `failed.failing_function`이 `Some(selector)`이면 `get_function_
    signature` 호출 → `Some(DecodedFunction{ selector, name, signature, source })`,
    `None` 또는 미매칭 → `None` 가산. 기존 `failed.failing_function`(selector
    문자열) 채움 로직 변경 없음.
  - `scripts/verify-failed-tx.sh`에 의미 단언 추가(S01 리뷰 Lesson: shape ≠
    semantics):
    - `failing_function_decoded` 필드 존재 (`hasOwnProperty`)
    - null이거나 object
    - object면: `selector === data.failed.failing_function` (자기 일관성) +
      `name` 비-빈 + `signature` 비-빈 + `selector === selector.toLowerCase()`
  - `docs/api-failed-tx.md`에 `failing_function_decoded` 절: 정의·예시 응답
    갱신·"자기소유 ABI 시드 vs 4byte.directory" framing(어떻게 다른가) 한 단락.
- *Artifacts*: `crates/api/src/routes/failed_tx.rs`, `scripts/verify-failed-tx.sh`,
  `docs/api-failed-tx.md`
- *Key Links*: S10-PLAN T02 가산성 패턴, S04 마일스톤 검증 Rule

## T03 — 프론트 "Failing function" 갱신 + S11-SUMMARY

**Must-haves**
- *Truths*
  - `types.ts`: `DecodedFunction { selector, name, signature, source }` +
    `FailedTxDetail.failing_function_decoded: DecodedFunction | null`.
  - `contract.ts`: `parseDecodedFunction`(shape 단언) + `parseFailedTxDetail`
    가 `"failing_function_decoded" in obj`로 missing-key 거부(silent default
    금지, S10 일관).
  - `contract.test.ts`: 기존 정상 케이스에 `failing_function_decoded: null`
    추가 + 신규 케이스 3:
    - object 정상 (selector/name/signature 일관성)
    - missing key → throw `/failing_function_decoded/`
    - malformed (예: name이 숫자) → throw `/name/`
  - `FailedTx.tsx`의 Tx inspection 카드 안 "Failing function" KPI 갱신:
    - decoded가 있으면: `name`을 KPI value로(큰 글자), `signature`를 작은
      mono로 두 줄. `source`가 있으면 작은 배지(예: 'erc20').
    - decoded가 null이면 기존 selector mono 표시 유지(무회귀).
  - `.gsd/S11-SUMMARY.md` + ROADMAP M004 S11 `[x]`. M004는 `🚧 IN PROGRESS`
    유지 — S11.1 / S12 / S13 남음.
- *Reassess*: S11 출하 후 — S11.1(args 디코딩 + root_cause input 디코드) 또는
  S12로 분해 결정 사용자 지시 대기. 새 Lesson은 KNOWLEDGE에 기록. M004는
  여전히 누적 중.
- *Artifacts*: `web/src/api/{types,contract}.ts`, `web/src/api/contract.test.ts`,
  `web/src/pages/FailedTx.tsx`, `.gsd/{S11-SUMMARY,M001-ROADMAP}.md`

---

## Slice 수용 (Complete)
- [ ] T01–T03 must-haves, 기존 `/v1/*`·`/alerts`·페이지 무회귀
- [ ] DB 통합(`-p db --ignored`) + indexer + db lib + clippy --workspace + fmt 모두 green
- [ ] `verify-failed-tx.sh` ALL PASS (신규 `failing_function_decoded` semantics 단언 포함),
      `verify-alerts.sh` + `verify-failed-tx-by-label.sh` 무회귀
- [ ] `web` typecheck + test(decoded 신규 케이스 포함) + build 통과
- [ ] REQUIREMENTS#M004 S11 항목 ✅ + S11-SUMMARY + ROADMAP S11 `[x]`
- [ ] M004는 *진행 중* 유지 — S11.1/S12/S13 분해는 다음 지시에서만
