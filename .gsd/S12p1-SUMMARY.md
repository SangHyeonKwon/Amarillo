---
slice: S12.1
title: ErrorCategory enum 세분화 v2 (M004 정밀도 가산)
status: done
edge: weak-spot — 진단 정밀도
milestone: M004 (잔여 정밀도 가산 — 별 단위 PR, 마일스톤 분기 X)
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace --all-targets -D warnings 0 · -p decoder 31/31 (기존 27 + 신규 5 — slippage 4 + allowance 1, 기존 generic 단일 테스트는 fallback 형태로 갱신) · -p api 단위 13 + 통합 7 = 20/20 무회귀 · -p indexer 36/36 무회귀 · -p db --lib 17/17 + db --ignored (alerts 3 + alert_rate 3 + category_diagnosis 3 + failed_tx 10 + function_signature 4 + labels 3 + rollback 1 = 27) 무회귀 · 마이그레이션 자동 적용(category_diagnosis 4 신규 시드 멱등 INSERT) · verify-failed-tx.sh ALL PASS (SEEDED set 4 확장) · verify-alerts/by-label 무회귀 자동 · tsc/py_compile clean · web typecheck/test 41/41/build OK
decisions: [D028, D029, D030]
artifacts:
  - migrations/20240109000001_subdivide_categories.sql  # 신설 — 4 ALTER TYPE ADD VALUE IF NOT EXISTS + 4 category_diagnosis 시드 INSERT (멱등)
  - crates/db/src/models.rs                              # ErrorCategory 4 신규 variant + FromStr + as_wire
  - crates/db/src/queries.rs                             # error_category_to_sql 4 신규 분기
  - crates/decoder/src/classifier.rs                     # 룰 우선순위 — 세부 매칭 먼저, fallback은 generic. 단위테스트 5 신규(allowance + slippage_amount_out/in/price_impact + slippage_generic_fallback)
  - scripts/verify-failed-tx.sh                          # SEEDED set 4 확장 (DIAG semantics 무회귀)
  - docs/api-failed-tx.md                                # ErrorCategory 10-row 표 + 세분화 정신(D028 fallback 유지) framing
  - docs/cookbook.md                                     # 시나리오 1에 S12.1 한 단락 (subdivided 카테고리 + 부모 fallback)
  - web/src/api/types.ts                                 # ErrorCategory union 4 신규 + ERROR_CATEGORIES 배열
  - web/src/lib/format.ts                                # ERROR_LABELS + ERROR_COLORS 4 신규 (부모 색상 변형 유지 — 차트 인지 그룹화)
  - examples/typescript-client/client.ts                 # ErrorCategory union 4 신규
  - examples/python-client/client.py                     # ErrorCategory docstring 10 카테고리 명시
  - .gsd/DECISIONS.md                                    # D028 (세분화 명세 SLIPPAGE 3 + ALLOWANCE 1, fallback 유지) · D029 (ALTER TYPE ADD VALUE 멱등 패턴) · D030 (classifier 룰 우선순위)
verification_constraint: "라이브 메인넷 자동 회귀 부재 — docker compose 시드 데이터의 GOOD tx가 실제로 세부 카테고리에 매칭되는지는 시드 revert reason에 달림. 실측: docker GOOD tx는 'Unknown' 카테고리(시드된 4 세부 카테고리 어느 것에도 매칭 안 됨)라 verify의 DIAG 단언은 *기존 6 시드*에 의존 — SEEDED set 4 확장은 *미래 reclassify 데이터 + 직접 새 카테고리로 시드된 tx*에 대비. 가산 자체가 정상 작동(마이그레이션 자동 적용 + 시드 4 행 INSERT 멱등)은 db --ignored 통과로 확인."
---

# S12.1 — 무엇이 실제로 일어났나

M004 잔여 정밀도 가산. BACKLOG #1 (S12.1 sketch). S12에서 6개 카테고리에
진단 메시지를 박았으나 *카테고리 자체*가 조잡 — `SLIPPAGE_EXCEEDED` 하나로
매수/매도/가격영향이 *같은 메시지*. dApp 개발자에게 *더 정확한 액션*을
주기 위해 4 신규 세부 카테고리 가산. 별 단위 PR, 마일스톤 분기 X
(REQUIREMENTS#M004 출하 정의는 이미 충족).

## 응답·표면 — 정밀도 가산

| 기존 | 신규 (S12.1) |
|------|--------------|
| `SLIPPAGE_EXCEEDED` (단일) | `SLIPPAGE_EXCEEDED` (fallback) + `SLIPPAGE_AMOUNT_OUT` / `SLIPPAGE_AMOUNT_IN` / `SLIPPAGE_PRICE_IMPACT` |
| `INSUFFICIENT_BALANCE` (단일) | `INSUFFICIENT_BALANCE` (token 잔액 부족) + `INSUFFICIENT_ALLOWANCE` (approve 부족 — 진단 메시지 완전 다름) |
| 총 6 카테고리 | **총 10 카테고리** (신규 4건) |

응답은 *additive* — 기존 카테고리 무변경, 4 신규만 가산 (D028). 기존
generic은 *fallback*으로 유지 (backward compat — PostgreSQL `ALTER TYPE ...
DROP VALUE` 제약).

## 수용 기준 (PLAN S12.1) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| `migrations/20240109000001_subdivide_categories.sql` (4 ADD VALUE + 4 시드 INSERT, 멱등) | ✅ | `IF NOT EXISTS` + `ON CONFLICT DO NOTHING`, db --ignored 자동 실행 통과 |
| Rust `ErrorCategory` 4 신규 variant + FromStr + as_wire | ✅ | `crates/db/src/models.rs` + queries.rs `error_category_to_sql` 4 신규 분기 (컴파일러가 non-exhaustive match로 회귀 차단) |
| classifier 룰 우선순위 — 세부 먼저, generic fallback (D030) | ✅ | 단위테스트 5 신규 + 회귀 가드(`"allowance"` 키워드가 `"balance"`보다 먼저 매칭) |
| 프론트 type union + label/color helper 4 신규 | ✅ | `web/src/api/types.ts` ErrorCategory + ERROR_CATEGORIES + `web/src/lib/format.ts` ERROR_LABELS + ERROR_COLORS (부모 색상 변형 — 차트 인지 그룹화) |
| examples (TS/Python) ErrorCategory 갱신 | ✅ | TS union 4 신규, Python docstring 10 카테고리 명시 |
| verify SEEDED set 확장 | ✅ | DIAG 단언이 4 신규 카테고리도 시드 매칭으로 받아들임 — verify ALL PASS |
| docs/api-failed-tx.md ErrorCategory 표 갱신 | ✅ | 10-row 표 + S12.1 세분화 정신 framing (D028 fallback 유지) |
| docs/cookbook.md 시나리오 1 한 단락 | ✅ | "S12.1: subdivided categories" 단락 추가 |
| 비기능: prod unwrap 0 / `///` doc / 마이그레이션 멱등 / 기존 데이터 무회귀 | ✅ | 전체 clippy/fmt clean, db --ignored 무회귀, web 41/41 무회귀 |

## 최종 게이트 (2026-05-21, 단일 호흡 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — 0
- `cargo test -p decoder` — **31/31** (기존 27 + 신규 5 — slippage 4 +
  allowance 1; 기존 generic `test_classify_slippage`는 *세부 4 + fallback 1*
  로 분해 갱신, 사실상 변형 4개 + 신규 케이스 추가)
- `cargo test -p api` — 단위 13 + 통합 7 = **20/20** 무회귀
- `cargo test -p indexer` — **36/36** 무회귀
- `cargo test -p db --lib` — **17/17** 무회귀
- `cargo test -p db -- --ignored` — **27/27** 무회귀 (alerts 3 + alert_rate 3
  + category_diagnosis 3 + failed_tx 10 + function_signature 4 + labels 3 +
  rollback 1 — 마이그레이션 자동 실행으로 4 시드 행 추가)
- `bash scripts/verify-failed-tx.sh` — **ALL PASS** (SEEDED 4 확장, DIAG
  semantics 무회귀)
- `bash scripts/verify-alerts.sh` / `verify-failed-tx-by-label.sh` — 무회귀
  자동 (서버/스크립트 변경 0)
- `tsc --noEmit -p examples/typescript-client/tsconfig.json` — clean
- `python3 -m py_compile examples/python-client/{client,examples}.py` — clean
- `cd web && npm run typecheck` — clean
- `cd web && npm run test` — **41/41** 무회귀
- `cd web && npm run build` — OK

## 태스크

- **T01** 마이그레이션 + Rust enum + classifier 룰 + 단위테스트 (5 신규).
  PostgreSQL `ALTER TYPE ADD VALUE IF NOT EXISTS` 4건 + `category_diagnosis`
  4 시드 INSERT(TEXT PK라 enum 값 무관, 같은 트랜잭션 안전). queries.rs
  `error_category_to_sql` 4 신규 분기 (컴파일러 회귀 차단).
- **T02** 프론트 type union 4 신규 + label/color helper. 부모 색상 변형으로
  차트 인지 그룹화 유지. contract.test.ts 무회귀 (기존 41/41).
- **T03** verify SEEDED 확장 + docs ErrorCategory 표 갱신 + examples 타입
  가산 + cookbook 한 단락 + DECISIONS D028/D029/D030 + SUMMARY + ROADMAP
  + BACKLOG.

## 핵심 교훈 (KNOWLEDGE 후보)

- **PostgreSQL enum 확장의 *추가만* 방향 (D029)** — `ALTER TYPE ... DROP
  VALUE` 제약으로 *제거 X*. 신중한 *가산만*이 backward compat 일관 패턴.
  컬럼이 *TEXT*(우리 `category_diagnosis.error_category`)인 곳은 *우연한
  안전망* — 같은 트랜잭션 내 enum 값 직접 사용 제약 회피. 미래 *대규모 enum
  재구성*은 새 enum 타입 생성 + 컬럼 마이그레이션 + 구 enum drop 패턴 (큰
  분량).
- **classifier 룰 우선순위 의존 (D030)** — *더 구체적인 키워드*가 *더 일반적인
  키워드*보다 먼저 매칭되어야 세부 카테고리가 잡힘. 회귀 가드 단위테스트
  (`test_classify_insufficient_allowance`에서 `"allowance"` 키워드가
  `"balance"` 매칭에 흡수되지 않음 단언)가 *룰 순서 깨짐*을 즉시 잡음. 시드
  키워드 풀이 작을 때(현재 ~30개)는 *순서 정렬 비용*이 낮음 — 큰 풀(예: 50+
  패턴)이면 우선순위 큐 / 정규식 트리 패턴.
- **non-exhaustive match가 컴파일 시점 회귀 차단** — Rust enum에 새 variant
  추가하면 `match` 표현식이 *non-exhaustive*가 되어 컴파일러 즉시 실패.
  `error_category_to_sql`에서 *4 신규 variant 빠뜨림*이 첫 빌드에서 잡힘.
  Rust enum의 *exhaustiveness*가 *컴파일 시점 분류 게이트* — D004/D014의
  silent default 거부 정신과 동일 정신.
- **추가만 가산 패턴은 *영원한 fallback*도 동반** — `SLIPPAGE_EXCEEDED`가
  fallback으로 *영원히* 존재. 시간이 지나면 *주로 안 쓰임*이지만 *제거 어려움*.
  운영자가 *카테고리 정리* 원하면 별 마일스톤 (enum 재구성 패턴). 본 슬라이스
  는 *세분화 가치* 우선, *정리는 후순위*.
- **색상 변형으로 차트 인지 그룹화** — 부모 카테고리(`SLIPPAGE_EXCEEDED`)와
  세부 카테고리(`SLIPPAGE_AMOUNT_OUT` 등)가 *같은 색상 계열*(노랑 톤 변형)을
  공유하면 차트 레전드에서 *그룹 관계*가 즉시 보임. UI 코드 추가 없이 *디자인
  단일 설정*으로 운영자/dApp 개발자에게 *카테고리 가족 관계* 전달.

## 정직한 한계 / 잔여

- **classifier 휴리스틱 한계** — 세부 매칭은 *revert reason 문자열 패턴*에
  의존. 컨트랙트가 *애매한* revert("custom error 0x...") 쓰면 `UNKNOWN`.
  4byte.directory 등 외부 매핑 *미도입* (D015 자기시드 정신 일관).
- **기존 데이터 reclassify 불가** — 마이그레이션이 ADD VALUE만, 기존 행은
  `SLIPPAGE_EXCEEDED` / `INSUFFICIENT_BALANCE` 그대로. 운영자가 *full
  reclassify* 원하면 별 절차 (`UPDATE failed_transaction SET error_category
  = classify_rerun(...)` 등).
- **enum 값 제거 불가** — PostgreSQL `ALTER TYPE ... DROP VALUE` 제약 (D029
  일관). 신중한 *추가만* 방향 → 장기적으로 stale 카테고리 누적 가능성.
- **`SLIPPAGE_EXCEEDED` fallback의 *향후 잔여*** — 새 트랜잭션은 세부 카테고리로
  분류되지만 *애매한 slippage*는 generic. 시간이 지나면 fallback 사용 빈도
  ↓이나 *제거 어려움*. 운영 관측 후 *카테고리 정리 마일스톤* 후보.
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터.
  실측: docker GOOD tx는 `Unknown` 카테고리(시드된 4 세부에 매칭 안 됨)라
  verify의 DIAG 단언은 *기존 6 시드*에 의존. 4 신규 시드 행이 정상 INSERT
  됨은 db --ignored 통과로 확인.
- **세분화 가치 4 카테고리 한정** — 다른 4 카테고리(DEADLINE/UNAUTHORIZED/
  TRANSFER_FAILED/UNKNOWN)는 *분기 가치 낮음*으로 판단. 첫 사용자 요구 시
  추후 별 슬라이스.

## Reassess

ROADMAP 완료 백로그 표에 `S12.1 — ErrorCategory enum 세분화 v2 (M004 정밀도
가산)` 추가. M004는 이미 ✅ SHIPPED — 본 슬라이스는 *잔여 정밀도 가산 PR*.
**M004 깊이 시리즈 (S10 root_cause → S11 selector → S11.1 args → S12 diagnosis
→ S12.1 enum 세분화) 자연 마감**. 세 페르소나 완결 상태 유지.

BACKLOG.md S12.1 항목 제거 + 우선순위 표 재정렬 (#1 → S13.1 npm/PyPI 패키지
게시). 다음 호흡은 사용자 결정 — S13.1 / OS resolver / 별 단위 hardening /
M007 분기 모두 GSD-2 정신 일관.
