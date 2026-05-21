# S11.1 — ABI args 디코딩 + root_cause.input 디코드 · PLAN

Slice 목표: BACKLOG #1 — **M004 깊이 가산** (별 단위 PR, 마일스톤 분기 아님).
S11에서 selector → name/signature까지 박혔던 디코딩을 *typed value 추출*까지
확장. `/v1/failed-tx/{tx_hash}` 응답이 한 단계 더 똑똑해짐:

```jsonc
"failing_function_decoded": {
  "selector": "0xa9059cbb",
  "name": "transfer",
  "signature": "transfer(address,uint256)",
  "source": "erc20",
  "args": [
    { "type": "address", "value": "0xabc...def" },
    { "type": "uint256", "value": "1000000000000000000" }
  ]
},
"root_cause_decoded": {            // 신규 — root_cause.input의 selector + args
  "selector": "0x095ea7b3",
  "name": "approve",
  "signature": "approve(address,uint256)",
  "source": "erc20",
  "args": [...]
}
```

엣지: `[edge: weak-spot — 진단 정밀도]`. risk: med. deps: M001~M006 + S10 +
S11. **M004 잔여 깊이 가산** — 별 단위 PR, 마일스톤 분기 X (REQUIREMENTS#M004
출하 정의는 이미 충족).

핵심 결정 (착수 시 기록 후 진입):
- **D025** — ABI 디코더 라이브러리: **alloy-sol-types** (alloy `[full]` 워크스페이스
  의존에 이미 포함, 신규 의존 0). 자체 minimal 디코더는 *우리 시드 17개 함수의
  tuple 시그니처* 직접 구현 부담 큼, 코너 케이스 유지비. D008/D015 정신 위반 X
  (이미 들어와 있는 의존 활용).
- **D026** — 응답 모델: `DecodedArg { type: string, value: serde_json::Value }`.
  order 보존(`Vec`), type 명시, value는 JSON 호환 변환 (address → "0x..." lowercased,
  uint256 → decimal string으로 precision 보존, bool → boolean, bytes/fixed_bytes →
  "0x..." hex, tuple/array → nested array). param `name`은 시드 시그니처가 *익명*
  이라 미포함(silent null 거부 정신 — null 필드 추가 후 *항상 null*은 정보 X).
- **D027** — 디코드 실패 시맨틱: `DecodedFunction.args: Option<Vec<DecodedArg>>`.
  `None` = 디코드 시도하지 않음(input 누락) 또는 디코드 실패(인자 수 불일치,
  hex 디코딩 실패 등). 명시 `null`로 정직 — name/signature는 표시되지만 args는
  *최선 노력*. *디코드 실패 시 *decoded 객체 전체*를 null로 만들지 않음* —
  selector lookup 성공 정보(name/signature)는 보존.

검증 제약(D009~D024 일관): T01 decoder 단위테스트(시드 17 selector 중 대표
6-7건 + 디코드 실패 케이스), T02 DB 통합테스트 무회귀 + 핸들러 호출 자기일관성,
T03 verify HTTP 의미 단언(args 형식 + root_cause_decoded 자기일관성) + tsc +
py_compile + web typecheck/test/build.

태스크: T01 → T02 → T03.

---

## T01 — `crates/decoder/src/abi.rs` 신설 + 단위테스트

**Must-haves**
- *Truths*
  - `crates/decoder/src/abi.rs` 신설:
    - `pub fn decode_args(signature: &str, input_hex: &str) -> Result<Vec<DecodedArg>, AbiDecodeError>`
      - `signature`: 예 `"transfer(address,uint256)"` 또는 `"exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))"`
      - `input_hex`: `"0x..."` 또는 `"..."` (대소문자 무관, prefix 옵션)
      - 첫 4바이트(8 hex)는 selector — *건너뜀*. 나머지를 args bytes로 디코드.
      - 1) signature 안의 paren-list 추출(`name(types...)`)
      - 2) `DynSolType::parse_seq(types_str)`로 type 시퀀스 파싱
      - 3) `hex::decode(input_hex.strip_prefix("0x").unwrap_or(input_hex))?`로 bytes
      - 4) `seq.abi_decode_params(&bytes[4..])` → `Vec<DynSolValue>`
      - 5) zip(types, values) → `Vec<DecodedArg { type, value }>`
    - `pub struct DecodedArg { type: String, value: serde_json::Value }`
      (serde 직렬화 — `db` 크레이트 모델에서 재export)
    - `pub enum AbiDecodeError`:
      - `InvalidSignature(String)` (paren-list 없거나 형식 오류)
      - `InvalidHex(String)` (input_hex 디코딩 실패)
      - `InputTooShort` (4바이트 selector도 못 채움)
      - `Decode(String)` (alloy abi_decode_params 실패)
    - `fn dynsol_to_json(v: &DynSolValue) -> serde_json::Value`:
      - Address → string `"0x{lowercased}"` (`format!("{a:#x}")` 또는 `format!("0x{}", hex::encode(a.as_slice()))`)
      - Uint(n, _) → string (decimal, precision 보존 — *number 미사용*)
      - Int(n, _) → string
      - Bool → bool
      - Bytes / FixedBytes → string `"0x..."`
      - String → string
      - Array / FixedArray / Tuple → JSON array (recursive)
      - 그 외 (function, enum 등 — 우리 시드에 없음) → `null` + 로깅
    - `fn dynsol_type_name(v: &DynSolValue) -> String` 또는 *signature 파싱 결과의 type string* 그대로 사용 (paren-list에서 분리한 type 토큰을 zip).
      - 추천: signature 파싱 토큰 그대로 — `DecodedArg.type`은 *Solidity 타입
        string 그대로* (예: `"address"`, `"uint256"`, `"(address,uint256,uint256)"`)
  - 단위테스트 6-7 case in `#[cfg(test)] mod tests`:
    - `decode_transfer_address_uint256` — `transfer(address,uint256)` 정상 디코드
    - `decode_approve_address_uint256` — `approve(address,uint256)` 정상
    - `decode_exactInputSingle_tuple` — `exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))` tuple 디코드 → nested array
    - `decode_invalid_hex_returns_err` — `input_hex` 형식 오류 → `InvalidHex`
    - `decode_invalid_signature_returns_err` — `"not a function"` → `InvalidSignature`
    - `decode_input_too_short_returns_err` — 4바이트 미만 → `InputTooShort`
    - `decode_mismatched_args_returns_err` — args bytes 길이 불일치 → `Decode`
  - prod `unwrap()` 0 / `///` doc / `Result<_, AbiDecodeError>` (no panic).
  - `crates/decoder/src/lib.rs`에 `pub mod abi;` 추가 + re-export.
- *Artifacts*: `crates/decoder/src/abi.rs`(신설), `crates/decoder/src/lib.rs`
- *Key Links*: alloy `DynSolType::parse_seq` + `abi_decode_params`, S11
  `function_signature` 시드 17건이 *integration ground truth*

## T02 — 응답 모델 확장 + 핸들러 호출 + DB 통합테스트

**Must-haves**
- *Truths*
  - `crates/db/src/models.rs`:
    - `DecodedFunction.args: Option<Vec<DecodedArg>>` 가산 (line 648 즈음)
    - `FailedTxDetail.root_cause_decoded: Option<DecodedFunction>` 가산 (line 459
      즈음, `failing_function_decoded` 다음). doc 명시 — silent default 거부
      (D004/D014).
    - `DecodedArg`는 `decoder::abi::DecodedArg`를 re-export 또는 *db 크레이트에
      자체 정의* + decoder가 동일 shape 반환. **추천**: decoder가 `Vec<(String,
      serde_json::Value)>` 같은 *raw* 반환, db 모델이 `DecodedArg`로 wrap —
      db가 wire schema의 single source. 더 단순한 대안: decoder 자체에 정의 +
      db는 re-export.
    - **결정**: decoder에 정의, db는 re-export (`pub use decoder::abi::DecodedArg;`)
      — wire schema는 *직렬화 위치 = serde derive 위치*. dependency cycle 없음
      (db는 이미 decoder 의존).
  - `crates/db/Cargo.toml`: decoder 의존 확인 (이미 있음 — `decoder = { path = "../decoder" }`)
  - `crates/api/src/routes/failed_tx.rs` handler 갱신 (line 25-71):
    1) `failing_function_decoded` 처리에 args 가산:
       - `failing_function`이 `Some(selector)`면 lookup 후
       - root frame input(`call_tree.iter().find(|f| f.call_depth == 0).and_then(|f| f.input.as_deref())`)
         + signature(`fs.signature`)로 `decoder::abi::decode_args(...)` 호출
       - 성공이면 `args: Some(Vec<...>)`, 실패면 `args: None` + `tracing::debug!`
       - selector lookup 실패 시 `failing_function_decoded` 자체 `None` (기존 동작)
    2) `root_cause_decoded` 처리:
       - `root_cause.as_ref().and_then(|tl| tl.input.as_deref())` 로 input 가져옴
       - 첫 4바이트(8 hex)를 selector로 분리 → `get_function_signature(&pool, selector)` lookup
       - 성공 + input 디코드 시도 → `DecodedFunction` 합성 (args 포함)
       - selector 시드 미존재 시 `None`
  - DB 통합테스트 무회귀 자동 (db 쿼리 무변경 — args는 *응답 합성 시점*에 디코더 호출).
  - prod `unwrap()` 0 / `///` doc / `Option<Vec<DecodedArg>>` 명시 null.
- *Artifacts*: `crates/decoder/src/abi.rs` (re-export), `crates/db/src/models.rs`,
  `crates/api/src/routes/failed_tx.rs`
- *Key Links*:
  - 기존 S11 핸들러 처리 패턴 (selector → name/signature)
  - S10 root_cause `TraceLog.input` 사용 (이미 응답 필드)

## T03 — verify + docs + web + cookbook + SUMMARY + 게이트 + PR

**Must-haves**
- *Truths*
  - `scripts/verify-failed-tx.sh` DECODED semantics 단언 확장 (line 105-140 즈음):
    - `failing_function_decoded.args` 검증 — null이거나 array
    - array면 각 원소 `{type: string, value: any}` 형식 단언
    - root_cause_decoded 같은 패턴 단언 (`hasOwnProperty` + null|object + selector
      자기일관성 + args 형식)
    - 의미 단언은 *시드 매칭 시*만 — 매칭 없으면 null OK (S11와 일관)
  - `docs/api-failed-tx.md` `failing_function_decoded` 절 확장:
    - args 필드 설명 + DecodedArg shape + value 변환 규칙 (address hex / uint
      decimal string / bytes hex / tuple nested array)
    - `root_cause_decoded` 신규 절 — selector → input 4바이트 + args 디코드
    - 정직한 한계: args=null은 *디코드 시도하지 않음 또는 실패* (D027)
  - `web/src/api/types.ts` + `contract.ts`:
    - `DecodedFunction.args: DecodedArg[] | null`
    - `DecodedArg { type: string; value: unknown }`
    - `FailedTxDetail.root_cause_decoded: DecodedFunction | null`
    - `parseFailedTxDetailEnvelope` — `"root_cause_decoded" in obj` missing-key
      거부 (D014/D015 일관, S10/S11 패턴)
  - `web/src/api/contract.test.ts`:
    - 3-4 신규 케이스 — args 정상 디코드, args null, root_cause_decoded object,
      root_cause_decoded missing key throw
  - `web/src/pages/FailedTx.tsx` "Failing function" KPI 확장:
    - decoded.args 있으면 *작은 표*로 표시 — `<type>: <value>` (truncated, hover
      tooltip로 full value)
    - 디자인 결정: 기존 KPI 카드 옆에 추가 또는 expandable. 너무 무겁지 않게.
  - `examples/typescript-client/client.ts` + `examples/python-client/client.py`:
    - `DecodedFunction.args` + `FailedTxDetail.root_cause_decoded` 타입/dataclass
      가산. wire types만 변경, 메서드 무영향.
  - `docs/cookbook.md` 시나리오 1 단건 진단:
    - 응답 예시 jsonc에 args + root_cause_decoded 추가
    - 한 단락 — "args가 typed value 표현(D027)"
  - `.gsd/DECISIONS.md` D025/D026/D027 기록 (착수 전 1차, T03 최종 확정).
  - `.gsd/S11p1-SUMMARY.md`: T01/T02/T03 산출, 게이트 evidence, 정직한 한계
    (시드 매칭 의존 / 라이브 메인넷 자동 회귀 부재 / alloy 의존 부분 사용).
  - `.gsd/M001-ROADMAP.md`:
    - 완료 백로그 표에 "S11.1 ABI args 디코딩 + root_cause.input 디코드 — DONE → S11p1-SUMMARY.md" 추가
  - `.gsd/BACKLOG.md`:
    - S11.1 항목 제거 (완료) — 우선순위 표에서 S11.1 빼고 #2 S12.1을 #1로 끌어올림
  - 최종 게이트 재실행 (KNOWLEDGE S04 Rule):
    - `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`
    - `cargo test -p decoder` (신규 abi 단위테스트 포함)
    - `cargo test -p api` 무회귀, `cargo test -p indexer`, `cargo test -p db --lib`,
      `cargo test -p db -- --ignored` 무회귀
    - `bash scripts/verify-failed-tx.sh` ALL PASS (args + root_cause_decoded 의미 단언)
    - `bash scripts/verify-alerts.sh` / `verify-failed-tx-by-label.sh` 무회귀
    - `tsc --noEmit -p examples/typescript-client/tsconfig.json` clean
    - `python3 -m py_compile examples/python-client/{client,examples}.py` clean
    - `cd web && npm run typecheck && npm run test && npm run build` clean
- *Reassess*: S11.1 ✅ DONE → ROADMAP 완료 백로그 표 + BACKLOG 정리. M004 깊이 한
  단계 가산, dApp 개발자 페르소나 진단 정밀도 ↑. 다음 호흡은 사용자 결정 —
  S12.1 / S13.1 / hardening / M007 분기.
- *Artifacts*: `scripts/verify-failed-tx.sh`, `docs/api-failed-tx.md`, `docs/cookbook.md`,
  `web/src/api/*.ts`, `web/src/pages/FailedTx.tsx`, `examples/*/client.*`,
  `.gsd/{DECISIONS,S11p1-SUMMARY,M001-ROADMAP,BACKLOG}.md`

---

## Slice 수용 (Complete = S11.1 SHIPPED)
- [ ] T01–T03 must-haves, 기존 모든 표면 무회귀
- [ ] `cargo test -p decoder` 신규 abi 6-7 case + 기존 18 모두 green
- [ ] `cargo test -p api` 단위 13 + 통합 7 = 20/20 무회귀
- [ ] `cargo test -p indexer` 36 / `-p db --lib` 17 / `-p db --ignored` 27 무회귀
- [ ] `verify-failed-tx.sh` ALL PASS (args/root_cause_decoded 의미 단언 포함)
- [ ] `verify-alerts.sh` / `verify-failed-tx-by-label.sh` 무회귀
- [ ] `tsc --noEmit` clean / `py_compile` clean
- [ ] `web` typecheck + test (기존 37 + 신규) + build 모두 green
- [ ] S11.1-SUMMARY + ROADMAP 완료 표 + BACKLOG 정리

## 정직한 한계 (S11.1 출하 시점)
- **시드 매칭 의존** — 디코드는 우리 자기소유 시드 17건 selector만 (D015 일관).
  외부 selector(4byte.directory 등)는 *추가 미도입*. 운영자가 `INSERT INTO
  function_signature ...`로 시드 확장 가능.
- **DecodedArg.value의 표현 자유도** — uint256은 decimal string, address는 hex,
  bool은 boolean. 클라이언트가 *type 필드를 보고 분기* 필요 — JSON Value의
  자연스러운 다형성 비용 (alternative: 모든 타입을 string으로 — 정보 손실 ↑).
- **param name 미포함** (D026) — 시드 시그니처가 익명 형태. 미래에 시그니처를
  named form(`transfer(address recipient, uint256 amount)`)로 바꾸면 자연 확장
  가능. 본 슬라이스는 *type + value*까지.
- **디코드 실패 = silent fallback** (D027) — args=null이 *시도 실패*와 *시도하지
  않음* 모두 매핑. 차이 신호 필요시 별 필드 (`args_decode_error: string | null`)
  추가 가능 — 본 슬라이스 스코프 밖.
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터.
  메인넷 트래픽 자동 회귀는 환경 부재로 불가능 (M001~M006 일관 한계).
- **alloy 의존 부분 사용** — alloy `[full]` features는 인덱서 RPC용으로 이미
  들어와 있음. `alloy-sol-types`(또는 dyn_abi) 모듈만 본 슬라이스가 사용 —
  신규 의존 0 (D025 정신 일관).
