# S12.1 — ErrorCategory enum 세분화 v2 · PLAN

Slice 목표: BACKLOG #1 — **M004 잔여 정밀도 가산**. S12에서 6개 카테고리에
진단 메시지·추천 액션을 박았으나 *카테고리 자체*가 조잡 — `SLIPPAGE_EXCEEDED`
하나로 매수/매도/가격영향이 *같은 메시지*. dApp 개발자에게 *더 정확한 진단*을
주기 위해 4개 신규 카테고리를 가산. 별 단위 PR, 마일스톤 분기 X (REQUIREMENTS
#M004 출하 정의는 이미 충족).

엣지: `[edge: weak-spot — 진단 정밀도]`. risk: med. deps: M001~M006 + S11.1 +
S12. **M004 잔여 정밀도 가산** — 본 슬라이스 출하 후 M004 깊이 시리즈(S10
→ S11 → S11.1 → S12 → S12.1)의 자연 마감 호흡.

핵심 결정 (착수 시 기록):
- **D028** — 세분화 명세 (옵션 2 — fallback 유지):
  - `SLIPPAGE_EXCEEDED` 유지 + **3 신규** 변형:
    - `SLIPPAGE_AMOUNT_OUT` — "too little received" (매수 슬리피지)
    - `SLIPPAGE_AMOUNT_IN` — "too much requested" (매도 슬리피지)
    - `SLIPPAGE_PRICE_IMPACT` — "price slipped" / "amount out" (가격 영향)
  - `INSUFFICIENT_BALANCE` 유지 + **1 신규** 변형:
    - `INSUFFICIENT_ALLOWANCE` — "allowance" / "exceeds allowance" (approve 부족,
      진단 메시지 완전 다름)
  - 나머지 4 카테고리 (DEADLINE_EXPIRED / UNAUTHORIZED / TRANSFER_FAILED /
    UNKNOWN) **불변** — 가치 낮음, 마이그레이션 부담만 가산.
  - **총 6 → 10 카테고리** (신규 4건).
- **D029** — PostgreSQL enum 처리: `ALTER TYPE error_category ADD VALUE IF
  NOT EXISTS 'XXX'` 4회. PostgreSQL 12+에서 트랜잭션 내 ADD VALUE OK, 같은
  트랜잭션 내 즉시 사용 가능. `category_diagnosis.error_category`는 *TEXT*
  컬럼이라 enum 값 사용 X — 같은 마이그레이션에서 시드 INSERT 안전. 기존
  enum 변형 *제거 X* (`ALTER TYPE ... DROP VALUE`는 PostgreSQL에서 불가/제한)
  — backward compat. 기존 데이터(`SLIPPAGE_EXCEEDED` 등으로 분류된 행) 그대로
  유지.
- **D030** — classifier 룰 우선순위: 세부 카테고리 *먼저 매칭* → fallback이
  기존 generic. 예: `"too little received"`는 *우선* `SLIPPAGE_AMOUNT_OUT`,
  매칭 실패 시 `SLIPPAGE_EXCEEDED` fallback. 클래시파이어가 *세부 패턴 미매칭*
  이지만 *generic 패턴 매칭*하는 경우 generic 카테고리로. 신규 트랜잭션은
  *세부 카테고리* 우선 분류.

검증 제약(D009~D027 일관): T01 단위테스트(decoder classifier 세부 매칭 +
generic fallback + db 통합 시드 lookup) + cargo workspace 게이트, T02 프론트
typecheck/test, T03 verify HTTP + tsc/py_compile + cookbook 갱신.

태스크: T01 → T02 → T03.

---

## T01 — 마이그레이션 + Rust ErrorCategory + classifier + 단위테스트

**Must-haves**
- *Truths*
  - `migrations/20240109000001_subdivide_categories.sql` **신설**:
    ```sql
    BEGIN;
    -- ALTER TYPE는 PostgreSQL 12+에서 트랜잭션 안전 + IF NOT EXISTS 멱등
    ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'SLIPPAGE_AMOUNT_OUT';
    ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'SLIPPAGE_AMOUNT_IN';
    ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'SLIPPAGE_PRICE_IMPACT';
    ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'INSUFFICIENT_ALLOWANCE';
    -- category_diagnosis는 TEXT PK라 enum 값 무관, 같은 트랜잭션 INSERT 안전
    INSERT INTO category_diagnosis (error_category, message, recommended_action, source) VALUES
      ('SLIPPAGE_AMOUNT_OUT',
       'Trade output fell below the minimum amount you specified (buy-side slippage).',
       'Increase amountOutMin tolerance, or split the trade to lower price impact.',
       'builtin'),
      ('SLIPPAGE_AMOUNT_IN',
       'Trade required more input than the maximum you specified (sell-side slippage).',
       'Increase amountInMax tolerance, or split the trade to lower price impact.',
       'builtin'),
      ('SLIPPAGE_PRICE_IMPACT',
       'Pool price moved past the allowed limit during execution.',
       'Widen sqrtPriceLimitX96 (or remove the limit) and consider splitting the trade.',
       'builtin'),
      ('INSUFFICIENT_ALLOWANCE',
       'The spender lacks ERC-20 allowance for this token transfer.',
       'Call approve(spender, amount) on the token before the trade or rerun.',
       'builtin')
    ON CONFLICT (error_category) DO NOTHING;
    COMMIT;
    ```
    - 멱등 (`IF NOT EXISTS` + `ON CONFLICT DO NOTHING`)
    - 기존 6 시드 무영향 (`ON CONFLICT DO NOTHING`)
  - `crates/db/src/models.rs` `ErrorCategory` 갱신:
    - 4 신규 variant 추가 (`SlippageAmountOut`, `SlippageAmountIn`,
      `SlippagePriceImpact`, `InsufficientAllowance`)
    - `impl FromStr` 4 신규 분기
    - `as_wire()` 4 신규 분기
    - sqlx::Type derive는 SCREAMING_SNAKE rename으로 자동 매핑 — Rust enum
      variants 가산만으로 PostgreSQL enum과 일치 (마이그레이션이 enum 값
      추가, Rust enum이 그 값 인식)
  - `crates/decoder/src/classifier.rs` 룰 우선순위 확장:
    - **SLIPPAGE 세부 매칭 우선** — `"too little received"` → AMOUNT_OUT,
      `"too much requested"` → AMOUNT_IN, `"price slipped"` / `"amount out"`
      → PRICE_IMPACT. 매칭 실패 시 기존 `SLIPPAGE_EXCEEDED` fallback (다른
      slippage 키워드 — 단순 `"slippage"`).
    - **ALLOWANCE 우선 매칭** — `"allowance"` (`"insufficient allowance"`,
      `"exceeds allowance"` 등) → `INSUFFICIENT_ALLOWANCE`. fallback은 기존
      `INSUFFICIENT_BALANCE`.
    - 패턴 순서 중요: 더 *구체적인 패턴*을 먼저, 더 *일반적인 패턴*은 나중.
  - 단위테스트 6+ 신규 (`test_classify_slippage_amount_out` /
    `_amount_in` / `_price_impact` / `_slippage_generic_fallback` /
    `_insufficient_allowance` / `_insufficient_balance_fallback`).
  - 기존 18 단위테스트 무회귀 — `test_classify_slippage`의 기존 단언이 *세부
    카테고리*로 변경됨에 따라 갱신 (예: `"Too little received"` → 새 기댓값
    `SLIPPAGE_AMOUNT_OUT`).
  - prod `unwrap()` 0 / `///` doc / 모든 신규 변형에 PostgreSQL ENUM과 wire
    form 정확히 매칭.
- *Artifacts*: `migrations/20240109000001_subdivide_categories.sql`(신설),
  `crates/db/src/models.rs`, `crates/decoder/src/classifier.rs`
- *Key Links*:
  - S12 `migrations/20240107000001_add_category_diagnosis.sql` (시드 패턴 동일)
  - 기존 `ErrorCategory` PascalCase + SCREAMING_SNAKE rename pattern

## T02 — 프론트 type union + 색상/라벨 + 무회귀

**Must-haves**
- *Truths*
  - `web/src/api/types.ts` `ErrorCategory` union + `ERROR_CATEGORIES` list:
    - 4 신규 변형 추가 (`SLIPPAGE_AMOUNT_OUT`, `SLIPPAGE_AMOUNT_IN`,
      `SLIPPAGE_PRICE_IMPACT`, `INSUFFICIENT_ALLOWANCE`)
    - `ERROR_CATEGORIES` 배열 확장 (UI dropdown / filter)
  - `web/src/lib/format.ts` (또는 동등 위치) `errorCategoryLabel` +
    `errorCategoryColor` helper에 4 신규 매핑:
    - `SLIPPAGE_AMOUNT_OUT` label: "Slippage (amount out)" — 색: 슬리피지 색 유지
    - `SLIPPAGE_AMOUNT_IN` label: "Slippage (amount in)"
    - `SLIPPAGE_PRICE_IMPACT` label: "Slippage (price impact)"
    - `INSUFFICIENT_ALLOWANCE` label: "Insufficient allowance"
  - `web/src/api/contract.test.ts` 무회귀 — 기존 mock의 카테고리 사용은 변경
    없음 (신규 카테고리는 *옵션*). 단, 신규 카테고리 정상 파싱 단언 1-2건
    추가 가능 (선택).
  - PascalCase serde wire form 동일 패턴 — `SlippageAmountOut` (PascalCase) ↔
    `SLIPPAGE_AMOUNT_OUT` (SCREAMING_SNAKE) 자동 매핑.
- *Artifacts*: `web/src/api/types.ts`, `web/src/lib/format.ts` (또는 동등),
  `web/src/api/contract.test.ts`
- *Key Links*: 기존 `ERROR_CATEGORIES` 배열 + `errorCategoryLabel` 패턴

## T03 — verify + docs + examples + cookbook + SUMMARY + 게이트 + PR

**Must-haves**
- *Truths*
  - `scripts/verify-failed-tx.sh` SEEDED set 확장:
    - 4 신규 카테고리 (`SLIPPAGE_AMOUNT_OUT` / `SLIPPAGE_AMOUNT_IN` /
      `SLIPPAGE_PRICE_IMPACT` / `INSUFFICIENT_ALLOWANCE`)를 SEEDED set에
      추가 → `category_diagnosis` 시드 lookup이 *non-null diagnosis*를
      보장하는 단언 (기존 6와 동일 패턴)
  - `docs/api-failed-tx.md` `ErrorCategory` 표 갱신:
    - 신규 4 카테고리 추가 (각각 description 한 줄)
    - 세분화 정신 메모 (D028): fallback 유지 + backward compat
  - `examples/typescript-client/client.ts` `ErrorCategory` union 확장 (4 신규)
  - `examples/python-client/client.py` `ErrorCategory = str` 변경 없음 (str
    alias). 단 docstring에 4 신규 카테고리 명시.
  - `docs/cookbook.md` 시나리오 1 한 단락 추가 — "S12.1: 세분화된 카테고리"
    한 줄 + diagnosis 예시 (SLIPPAGE_AMOUNT_OUT 등).
  - `.gsd/DECISIONS.md` **D028/D029/D030** 기록.
  - `.gsd/S12p1-SUMMARY.md`: T01/T02/T03 산출, 게이트 evidence, 정직한 한계
    (시드 매칭 의존 / 기존 generic 데이터 유지 / 마이그레이션 PostgreSQL 12+
    의존).
  - `.gsd/M001-ROADMAP.md`:
    - 완료 백로그 표에 "S12.1 — ErrorCategory 세분화 v2" 추가
  - `.gsd/BACKLOG.md`:
    - S12.1 항목 제거 + 우선순위 표 재정렬 (#1 → S13.1)
  - 최종 게이트 재실행 (KNOWLEDGE S04 Rule):
    - `cargo fmt --check` (workspace)
    - `cargo clippy --workspace --all-targets -- -D warnings`
    - `cargo test -p decoder` (신규 classifier 단위테스트 포함)
    - `cargo test -p api / -p indexer / -p db --lib / --ignored` 무회귀
    - `bash scripts/verify-failed-tx.sh` ALL PASS (SEEDED 확장)
    - `verify-alerts.sh` / `verify-failed-tx-by-label.sh` 무회귀
    - `tsc --noEmit` clean / `py_compile` clean
    - `cd web && npm run typecheck && npm run test && npm run build`
- *Reassess*: S12.1 ✅ DONE → ROADMAP 완료 표 + BACKLOG 정리. M004 깊이 시리즈
  (S10 → S11 → S11.1 → S12 → S12.1) 자연 마감. 다음 호흡은 사용자 결정.
- *Artifacts*: `scripts/verify-failed-tx.sh`, `docs/api-failed-tx.md`,
  `docs/cookbook.md`, `examples/*/client.{ts,py}`,
  `.gsd/{DECISIONS,S12p1-SUMMARY,M001-ROADMAP,BACKLOG}.md`

---

## Slice 수용 (Complete = S12.1 SHIPPED)
- [ ] T01–T03 must-haves, 기존 모든 표면 무회귀
- [ ] `cargo test -p decoder` 신규 6+ 케이스 + 기존 27 모두 green (기존 slippage
      테스트는 세부 카테고리로 단언 갱신)
- [ ] `cargo test -p api` 단위 13 + 통합 7 = 20/20 무회귀
- [ ] `cargo test -p indexer` 36 / `-p db --lib` 17 / `-p db --ignored` 27+
      (category_diagnosis 시드 통과 — 신규 행 4건이 멱등 추가)
- [ ] `verify-failed-tx.sh` ALL PASS (SEEDED 4 신규 포함 — DIAG semantics
      신규 매칭 단언 통과)
- [ ] `verify-alerts.sh` / `verify-failed-tx-by-label.sh` 무회귀
- [ ] `tsc --noEmit` clean / `py_compile` clean
- [ ] `web` typecheck + test + build 모두 green
- [ ] S12.1-SUMMARY + ROADMAP 완료 표 + BACKLOG 정리

## 정직한 한계 (S12.1 출하 시점)
- **classifier 룰의 휴리스틱 한계** — 세부 카테고리 매칭은 *revert reason
  문자열 패턴*에 의존. 컨트랙트가 *애매한* revert reason("custom error 0x...")
  쓰면 `UNKNOWN`. 4byte.directory 등 외부 매핑 *미도입* (D015 일관).
- **기존 데이터 reclassify 불가** — 마이그레이션이 ADD VALUE만, 기존 행은
  `SLIPPAGE_EXCEEDED` / `INSUFFICIENT_BALANCE` 그대로. 운영자가 *full
  reclassify*를 원하면 `UPDATE failed_transaction SET error_category =
  classify_rerun(...)` 같은 별 절차 — 본 슬라이스 스코프 밖.
- **enum 값 제거 불가** — PostgreSQL `ALTER TYPE ... DROP VALUE`는 9.4+
  에서 *제한* (또는 16에서도 지원 미흡). 카테고리 *추가만* 가능. 향후 정리
  시 `enum 마이그레이션 → 새 enum 이름 → 기존 enum 제거` 패턴 필요 — 큰 분량.
- **`SLIPPAGE_EXCEEDED` fallback의 *향후 잔여*** — 새 트랜잭션은 세부 카테고리
  로 분류되지만 *애매한 slippage*는 여전히 generic. 시간이 지나면 fallback이
  *주로 안 쓰임* — 그래도 stale 카테고리 정리는 별 슬라이스.
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터
  (M001~M006 일관 한계).
