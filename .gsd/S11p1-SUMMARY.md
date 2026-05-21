---
slice: S11.1
title: ABI args 디코딩 + root_cause.input 디코드 (M004 깊이 가산)
status: done
edge: weak-spot — 진단 정밀도
milestone: M004 (잔여 깊이 가산 — 별 단위 PR, 마일스톤 분기 X)
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace --all-targets -D warnings 0 · -p decoder 27/27 (기존 18 + 신규 9) · -p api 단위 13 + 통합 7 = 20/20 무회귀 · -p indexer 36/36 무회귀 · -p db --lib 17/17 + db --ignored 27/27 무회귀 · verify-failed-tx.sh ALL PASS (root_cause_decoded = exactInputSingle 실제 매칭) · verify-alerts + verify-failed-tx-by-label 무회귀 자동 · tsc/py_compile clean · web typecheck/test 41/41 (37 기존 + 4 신규)/build OK
decisions: [D025, D026, D027]
artifacts:
  - crates/decoder/Cargo.toml                       # serde.workspace = true 가산 (DecodedArg Serialize)
  - crates/decoder/src/abi.rs                       # 신설 — decode_args + DecodedArg + AbiDecodeError + 단위테스트 9
  - crates/decoder/src/lib.rs                       # pub mod abi 가산
  - crates/db/Cargo.toml                            # decoder path dep 가산 (DecodedArg 호스팅)
  - crates/db/src/lib.rs                            # pub use decoder::abi re-export
  - crates/db/src/models.rs                         # DecodedFunction.args 가산 + FailedTxDetail.root_cause_decoded 가산
  - crates/api/src/routes/failed_tx.rs              # 핸들러 args 디코드 + root_cause_decoded 합성 + extract_selector helper
  - scripts/verify-failed-tx.sh                     # args + root_cause_decoded 의미 단언 추가
  - docs/api-failed-tx.md                           # args 절 + root_cause_decoded 절 신설 + 라이브러리 선택 framing
  - docs/cookbook.md                                # 시나리오 1 응답 예시에 args + root_cause_decoded 추가
  - web/src/api/types.ts                            # DecodedArg + DecodedFunction.args + FailedTxDetail.root_cause_decoded
  - web/src/api/contract.ts                         # parseDecodedArg + DecodedFunction args 파싱 + root_cause_decoded missing-key 거부
  - web/src/api/contract.test.ts                    # 신규 4 케이스(args 정상 / root_cause_decoded selector 일관 / root_cause_decoded missing throw / args missing throw)
  - web/src/pages/FailedTx.tsx                      # DecodedArgsList + DecodedFunctionLabel helper + KPI 카드 아래 args 박스 + root_cause 블록에 root_cause_decoded 패널
  - examples/typescript-client/client.ts            # DecodedArg interface + DecodedFunction.args + FailedTxDetail.root_cause_decoded
  - examples/python-client/client.py                # DecodedArg dataclass + DecodedFunction.args + FailedTxDetail.root_cause_decoded
  - .gsd/DECISIONS.md                               # D025 (alloy dyn_abi) · D026 (Vec<DecodedArg> 모델) · D027 (args=null silent fallback)
verification_constraint: "라이브 메인넷 자동 회귀 부재 — docker compose 시드 데이터의 GOOD tx가 실제로 ABI 디코드를 트리거하는지는 시드에 달림. 실측: verify에서 root_cause_decoded = `exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))`로 *실제 디코드 성공* — 시드된 Uniswap V3 selector가 매칭됨. failing_function_decoded는 GOOD tx의 selector가 17건 시드와 미매칭이라 null (S11과 일관)."
---

# S11.1 — 무엇이 실제로 일어났나

M004 잔여 깊이 가산. BACKLOG #1 (S11.1 sketch). S11에서 selector → name/signature
까지 박혔던 디코딩을 *typed value 추출*까지 확장. `/v1/failed-tx/{tx_hash}`
응답이 한 단계 더 똑똑해짐 — dApp 개발자가 `transfer(0xabc…, 1000000)` 같은
타입된 인자값을 *한 호출*에서 받음. 별 단위 PR, 마일스톤 분기 X (REQUIREMENTS#M004
출하 정의는 이미 충족).

## 응답·표면 — 깊이 가산

| 기존 (S11) | 신규 (S11.1) |
|------------|--------------|
| `failing_function_decoded: { selector, name, signature, source }` | `failing_function_decoded: { ..., args: DecodedArg[] | null }` |
| — (root_cause만 raw TraceLog) | `root_cause_decoded: DecodedFunction | null` — root_cause.input의 selector + args |

응답이 *additive*: 기존 필드 무변경, args + root_cause_decoded 두 신규 필드만
가산 (D004/D014 일관 — 클라이언트는 무회귀).

## 수용 기준 (PLAN S11.1) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| `crates/decoder/src/abi.rs` 신설 + `decode_args(signature, input_hex)` + DecodedArg/AbiDecodeError | ✅ | 통과 27/27 (decoder 단위 9 신규: transfer/approve/exactInputSingle tuple + invalid_hex/invalid_signature/input_too_short/mismatched/split helper/extract helper) |
| alloy `DynSolType::parse` + tuple wrap 패턴 (신규 의존 0) | ✅ | D025 — `decoder/Cargo.toml`은 alloy `workspace = true` 이미 보유; serde만 가산 |
| `DecodedFunction.args: Option<Vec<DecodedArg>>` 가산 + `FailedTxDetail.root_cause_decoded` 가산 | ✅ | `crates/db/src/models.rs` + serde Serialize derive + 호환 `From<FunctionSignature>` `args: None` 기본 |
| 핸들러: failing_function_decoded.args(root frame input에서) + root_cause_decoded(selector 4바이트 + args) | ✅ | `crates/api/src/routes/failed_tx.rs` + `extract_selector` helper + tracing::debug 실패 로깅 |
| 디코드 실패 = args=null, 객체 자체는 살림 (D027) | ✅ | `Ok(args) => decoded.args = Some(args), Err(e) => tracing::debug!` 패턴 |
| verify HTTP 의미 단언 (args + root_cause_decoded selector 자기일관성) | ✅ | `scripts/verify-failed-tx.sh` 실행 → `ROOT_DECODED OK (exactInputSingle :: exactInputSingle((address,address,uint24,...)))` 실제 매칭 |
| web parser missing-key 거부 + 단위테스트 | ✅ | `parseDecodedArg` + `parseDecodedFunction` args missing throw + parseFailedTxDetail root_cause_decoded missing throw, web 신규 4 case |
| FailedTx 페이지 UI 가산 (`<DecodedArgsList>` + `<DecodedFunctionLabel>`) | ✅ | KPI 카드 아래 args 박스 + root_cause 블록에 root_cause_decoded 패널 |
| examples (TS/Python) wire types 가산 | ✅ | TS `DecodedArg` + `DecodedFunction.args` + `FailedTxDetail.root_cause_decoded`; Python @dataclass + from_dict |
| docs/cookbook 갱신 | ✅ | docs/api-failed-tx.md 상단 `failing_function_decoded` 절 확장 + `root_cause_decoded` 신규 절 + cookbook 시나리오 1 응답 예시 |
| 비기능: prod unwrap 0 / `///` doc / D004 silent default 거부 / 시드 무변경 | ✅ | 전체 워크스페이스 clippy/fmt clean, 마이그레이션 없음 (디코더는 시드 lookup만) |

## 최종 게이트 (2026-05-21, 단일 호흡 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — 0
- `cargo test -p decoder` — **27/27** (기존 18 + 신규 9: transfer/approve/
  exactInputSingle tuple + invalid_hex/invalid_signature/input_too_short/
  mismatched_args + split_top_level/extract_param_list helper)
- `cargo test -p api` — 단위 13 + 통합 7 = **20/20** 무회귀
- `cargo test -p indexer` — **36/36** 무회귀
- `cargo test -p db --lib` — **17/17** 무회귀
- `cargo test -p db -- --ignored` — **27/27** 무회귀 (docker postgres)
- `bash scripts/verify-failed-tx.sh` — **ALL PASS** + `ROOT_DECODED OK
  (exactInputSingle :: exactInputSingle((address,address,uint24,address,
  uint256,uint256,uint256,uint160)))` *실제 디코드 성공* (docker 시드의
  Uniswap V3 SwapRouter selector 매칭)
- `bash scripts/verify-alerts.sh` / `verify-failed-tx-by-label.sh` — 무회귀
  자동 (서버/스크립트 0 변경)
- `tsc --noEmit -p examples/typescript-client/tsconfig.json` — clean
- `python3 -m py_compile examples/python-client/{client,examples}.py` — clean
- `cd web && npm run typecheck` — clean
- `cd web && npm run test` — **41/41** (37 기존 + 4 신규 S11.1)
- `cd web && npm run build` — OK

## 태스크

- **T01** decoder 모듈 — `crates/decoder/src/abi.rs` 신설 (`decode_args` +
  `DecodedArg` + `AbiDecodeError` + helper `extract_param_list` /
  `split_top_level` / `dynsol_to_json`). 단위테스트 9 case.
- **T02** 응답 모델 + 핸들러 — `DecodedFunction.args` 가산 + `FailedTxDetail.
  root_cause_decoded` 가산. `db/Cargo.toml`에 decoder dep 추가 + `db/lib.rs`
  `pub use decoder::abi` re-export. 핸들러에 args 디코드 + root_cause_decoded
  합성 + `extract_selector` helper. api 무회귀 (20/20).
- **T03** verify + docs + web + examples + cookbook + DECISIONS + SUMMARY +
  ROADMAP + BACKLOG + 게이트 + 커밋 + PR. *별 단위 PR* (마일스톤 분기 X).

## 핵심 교훈 (KNOWLEDGE 후보)

- **이미 들어와 있는 의존 활용 정신 (D025)** — alloy는 인덱서 RPC용으로 워크스페이스
  의존이지만 *dyn_abi 모듈*은 본 슬라이스 전 미사용. 신규 의존 추가 0으로
  ABI 디코더 도입 — D008/D015 "외부 의존 미도입" 정신 위반 X (*동일 의존의
  다른 모듈*은 신규 의존 아님). 자체 minimal 디코더 부담(tuple/nested array
  코너 케이스) 회피.
- **JSON precision-safe representation (D026)** — `uint*`/`int*`를 *decimal
  string*으로 lowering. JSON number는 2^53 초과 시 silent 정밀도 손실 →
  `uint256`를 number로 보내면 JS/TS 클라이언트가 *조용히 깨짐*. string은
  type 필드와 함께 정직 + 안전. 모든 정수형 통일 (uint8도 string) — 일관성.
- **selector lookup과 args decode는 독립 단계 (D027)** — name/signature는
  *항상* 알 수 있음 (selector 시드 매칭만 필요). args는 input + signature
  + decode 모두 성공해야. *부분 실패*가 `args=null`로 안전하게 흡수 →
  객체 자체는 살림 + 사용자에게 *최대 정보* 보존. 두 케이스 (시드 미매칭 vs
  args 디코드 실패)가 *시각적으로 구분* (전자: DecodedFunction null, 후자:
  DecodedFunction 객체 + args null).
- **db crate가 wire schema 단일 출처** — DecodedArg는 *decoder*에 정의되지만
  `db::abi`로 re-export → api 핸들러는 `db::abi::decode_args` 호출. 의존
  그래프 단순 (api는 db만 의존하면 됨). decoder는 *순수 라이브러리*로 유지,
  serde Serialize derive는 decoder에 박힘 (wire schema 일관성).
- **컴파일 시점 회귀 차단 (D014/D015 정신 연장)** — `DecodedFunction.args`와
  `FailedTxDetail.root_cause_decoded`를 *명시 필드*로 가산 → 새 응답 캐스트
  추가 시 *반드시* 채워야 컴파일됨. web parser는 missing key를 *throw* —
  silent default 거부 정신 일관 (S10/S11/S12 패턴 누적).
- **alloy DynSolType API quirk** — `parse_seq`가 alloy 1.x에서 노출되지 않아
  *단일 `parse` + tuple wrap*으로 우회. signature paren-list를 `(types)`로
  감싸 parse → `DynSolType::Tuple(...)` → `abi_decode_params` → `DynSolValue
  ::Tuple(values)` → flat unwrap. 의도된 API라 보이지만 *문서 검색 시간*이
  PLAN 단계에 잡혀야 — PLAN의 "parse_seq" 가정은 잘못된 추정. 정직히 정정.

## 정직한 한계 / 잔여

- **시드 매칭 의존** (D015 일관) — 디코드는 우리 자기소유 시드 17건 selector만.
  운영자가 `INSERT INTO function_signature ... ON CONFLICT DO NOTHING`로 확장
  가능. 4byte.directory 등 외부 의존 미도입.
- **DecodedArg.value의 다형성** (D026 트레이드오프) — JSON `unknown`이라
  클라이언트가 `type` 필드를 보고 분기 필요. 정보 보존 우선.
- **param name 미포함** (D026) — 시드 시그니처가 익명 형태. 미래 named form
  으로 시드 확장 시 자연 가산 가능. 본 슬라이스는 *type + value*까지.
- **디코드 실패 = silent fallback** (D027) — args=null이 *시도 실패*와 *시도
  하지 않음* 모두 매핑. `args_decode_error: string | null` 가산 가능하지만
  *응답 표면 가산* + *서버 내부 노출* 위험. 디버깅은 *서버 로그* (tracing::
  debug)로.
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터.
  실측: docker GOOD tx에서 `root_cause_decoded`는 *실제로 디코드 성공*
  (exactInputSingle 튜플), `failing_function_decoded`는 GOOD selector가 17건
  시드에 미매칭이라 null. 두 경로 모두 자동 단언으로 통과.
- **alloy 부분 사용** — alloy `[full]` features가 인덱서용으로 이미 들어와
  있으나 본 슬라이스가 사용하는 건 `dyn_abi` 모듈만. 신규 cargo dep 0.
  serde는 decoder crate에 workspace = true 가산 (1줄).

## Reassess

ROADMAP 완료 백로그 표에 `S11.1 ABI args 디코딩 + root_cause.input 디코드 —
DONE → S11p1-SUMMARY.md` 추가. M004는 이미 ✅ SHIPPED — 본 슬라이스는 *잔여
깊이 가산 PR*. **세 페르소나 완결 상태 유지** (M001~M006 + 본 깊이 가산).
BACKLOG.md S11.1 항목 제거 + 우선순위 표 재정렬 (#1 = S12.1 enum 세분화).

다음 호흡은 사용자 결정 — S12.1 / S13.1 / 별 단위 hardening / M007 분기 모두
GSD-2 정신 일관.
