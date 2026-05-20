---
slice: S11
title: selector → 함수명 + signature 디코딩 (M004 둘째 슬라이스)
status: done
edge: weak-spot
milestone: M004
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 19/19 (function_signature 4 신규 + alerts 3 + failed_tx 10 + labels 1 + rollback 1) · verify 3종 ALL PASS · web typecheck/test/build OK
decision: D015
artifacts:
  - migrations/20240106000001_add_function_signature.sql  # 멱등 + ERC20/Uniswap V3/WETH9 ABI 시드 17건
  - crates/db/src/models.rs                               # FunctionSignature + DecodedFunction + FailedTxDetail.failing_function_decoded
  - crates/db/src/queries.rs                              # get_function_signature (LOWER($1) lookup)
  - crates/db/tests/function_signature.rs                 # 통합테스트 4 (seeded/uniswap/unknown/case)
  - crates/api/src/routes/failed_tx.rs                    # 핸들러 가산 (selector lookup → decoded)
  - scripts/verify-failed-tx.sh                           # DECODED semantics 단언 (selector 자기일관성)
  - docs/api-failed-tx.md                                 # failing_function_decoded 절 + 자기시드 framing
  - web/src/api/{types,contract}.ts                       # DecodedFunction + parser (missing-key 거부)
  - web/src/api/contract.test.ts                          # 신규 케이스 3 (object/missing/malformed) + 기존 2 수정
  - web/src/pages/FailedTx.tsx                            # "Failing function" KPI 갱신 (name/sig/source)
  - .gsd/DECISIONS.md                                     # D015 (args 디코딩 분리, 자기시드 정책)
verification_constraint: "M004는 *진행 중* — S11 출하만으로 미마감. S11.1/S12/S13 분해는 다음 지시에서. GOOD seed tx의 failing_function이 시드된 selector list와 매칭되는지는 시드 데이터에 달림 — 자기일관성 단언은 자동, 의미 검증(실제 decoded object 응답)은 매칭 시드 도입 시 수동 시연."
---

# S11 — 무엇이 실제로 일어났나

REQUIREMENTS#M004 2차 가산 — `/v1/failed-tx/{tx_hash}` 응답에
`failing_function_decoded: DecodedFunction | null` 추가. 4바이트 selector
(`0xa9059cbb`)를 자기소유 ABI 시드와 lookup해 사람이 읽는 함수명·시그니처로
매핑. 기존 `failing_function`(selector 그대로) 계약 불변(D004); 새 필드만 가산.

- **T01 (스키마 + 시드 + 모델 + 쿼리 + 통합테스트 + D015)**: 멱등 마이그레이션
  `20240106000001_add_function_signature.sql` — `function_signature(selector PK,
  name, signature, source?, created_at)` + 시드 17건(ERC20 5 + Uniswap V3
  SwapRouter 6 + Factory 1 + Pool 3 + WETH9 2). `ON CONFLICT (selector) DO
  NOTHING`으로 멱등 보장(재실행 안전). `FunctionSignature`(FromRow) +
  `DecodedFunction`(응답 합성) 모델 + `impl From<FunctionSignature>` 변환.
  쿼리 `get_function_signature`는 `LOWER($1)` lookup(대소문자 무관, SQL
  주입 안전). 통합테스트 4건:
  - `get_function_signature_seeded_lookup_ok` (ERC20 transfer 매칭)
  - `get_function_signature_uniswap_router_seeded` (튜플 시그니처 확인)
  - `get_function_signature_unknown_selector_is_none` (silent default 금지)
  - `get_function_signature_case_insensitive_lookup` (LOWER lookup 불변식)

  D015 결정 기록(DECISIONS): args 디코딩 분리, 자기시드 정책, 외부 의존 미도입.

- **T02 (API 응답 가산 + verify + docs)**: `FailedTxDetail`에
  `failing_function_decoded: Option<DecodedFunction>` 가산(`skip_serializing_if`
  금지 — silent default 차단, S10 일관). 핸들러에서 `failing_function`이
  `Some(selector)`이면 `get_function_signature` 호출 → `DecodedFunction::from`,
  `None`이거나 미매칭이면 명시 `null`. `verify-failed-tx.sh`에 node 의미
  단언 추가(S01 리뷰 Lesson: shape ≠ semantics):
  - `failing_function_decoded` 필드 존재 (`hasOwnProperty`)
  - null 또는 object
  - object면 `selector === data.failed.failing_function.toLowerCase()` (자기
    일관성) + lowercased + `name`/`signature` 비-빈
  실측: GOOD tx에서 `DECODED OK (null)` — GOOD 시드의 selector가 우리
  17건 시드에 없거나 `failing_function`이 null. 두 경로 모두 정상 처리됨이
  자동 단언. `docs/api-failed-tx.md`에 `failing_function_decoded` 절 +
  "자기소유 ABI 시드 vs 4byte.directory" 두 이유(런타임 의존 0 + 큐레이트
  품질) framing.

- **T03 (web + S11 출하)**: `types.ts`에 `DecodedFunction` + `FailedTxDetail.
  failing_function_decoded`. `parseFailedTxDetail`이 `"failing_function_decoded"
  in obj`로 missing-key 거부(S10/D014 일관). `contract.test.ts`에 신규 3
  케이스(object 정상/missing key throw/malformed name throw) + 기존 정상
  2케이스에 `failing_function_decoded: null` 추가, S10의 malformed root_cause
  케이스도 필드 추가(검증은 root_cause에서 throw하므로 무관). FailedTx
  페이지의 Tx inspection 카드 "Failing function" KPI 갱신: decoded면
  name(굵게) + signature(mono 작게) + source 배지, decoded null이면
  selector mono 유지(무회귀). 기존 Root cause / call_tree / by-label /
  list 카드 무회귀.

**해자(D002·D014·D015)의 *셋째* 깊이가 코드로 박힘**
- selector(`0xa9059cbb`) → `transfer(address,uint256)` 매핑은 그 자체로
  사람이 읽는 진단 정보. dApp 개발자는 "내 트랜잭션의 `transfer`가 실패
  했다"를 한 호출에서 받음 — 별도 ABI lookup 불요.
- 자기소유 시드 정책(D015/D008 일관): 외부 4byte.directory 의존 0. 운영자가
  필요 시 `INSERT INTO function_signature ... ON CONFLICT DO NOTHING`으로
  추가 — 큐레이트 가능. 공개 selector DB의 typo/충돌 garbage 미도입.
- Dune이 못 함: ABI 디코딩은 *consumer-specific* 인프라(우리 시드/우리 라벨).
  S09 컨트랙트 라벨 조인과 같은 패턴 — *비공개 보조 데이터*가 차별 가치.

**정직한 한계**
- docker 기본 시드의 GOOD tx `failing_function`이 우리 17건 시드 selector
  와 매칭되는지는 시드 데이터에 달림. 실측: 현재 null. verify는 두 경로
  (null/object) 모두 자동 단언으로 통과 — 의미적 시연(실제 decoded 객체
  표시)은 매칭 시드 도입 시점에 수동 스모크.
- args 디코딩 미포함(D015) — 응답엔 name/signature까지만. ABI typed value
  추출은 별 슬라이스(S11.1 sketch).
- `root_cause.input`의 selector도 같은 디코드 대상이지만 본 슬라이스는
  `failing_function_decoded` 1필드만 — root_cause 가산은 후속(S11.1).
- 인증 미연결(D008 일관) — `function_signature` 관리용 HTTP 표면 없음
  (시드는 마이그레이션·운영 SQL 직접).

**M004 진행**
- S10 ✅ root_cause attribution
- S11 ✅ selector decoding (본 슬라이스)
- S11.1 / S12 / S13 `[sketch]` 유지 — 다음 지시에서 분해. 후보:
  - S11.1 ABI args 디코딩 + root_cause.input 디코드 (typed value)
  - S12 카테고리 세분화 v2 + 진단 메시지/추천액션
  - S13 개발자 SDK/문서 (TS/Python 미니멈 클라이언트)

**Reassess**: ROADMAP M004 S11 `[x]`, M004는 `🚧 IN PROGRESS` 유지.
KNOWLEDGE 추가 없음 — D015가 결정 자체에 기록(자기소유 시드 정책이 핵심
패턴). 백로그(DNS-rebinding SSRF / 임계율 집계 / Pools·Traders 매핑)는
단독 단위 유지.
