# S13 — 개발자 예시 클라이언트(TS+Python) + cookbook (M004 마무리 슬라이스) · PLAN

Slice 목표: REQUIREMENTS#M004의 *프로덕트화 단계*. dApp 개발자가 `/v1/*` API
를 *카피해서 즉시 쓰는* TS + Python 예시 클라이언트 + cookbook 문서.
M001~M003 + S10~S12 누적된 응답 계약(root_cause / failing_function_decoded /
diagnosis / by-label / alert 구독 + HMAC)을 *작동하는 코드*로 시연.

엣지: `[edge: weak-spot]`. risk: low. deps: M001~M003 + S10~S12 (이미 머지됨).
**M004 마무리** — 본 슬라이스로 M004 acceptance 완성(자유 출하 가능). S11.1 /
S12.1 / S13.1은 후속 *심화* 슬라이스.

핵심 결정: **D017** (착수 시 기록) — npm / PyPI 게시는 별 슬라이스(`S13.1`).
TS는 `fetch`, Python은 `urllib.request` — 두 언어 모두 *외부 런타임 의존 0*.

검증 제약(D009~D016 일관): TS는 `tsc --noEmit` typecheck, Python은
`python -m py_compile` syntax 검증. 라이브 호출은 docker compose + verify
스크립트와 동일 환경 요구라 *컴파일·타입 검증*까지 + 수동 스모크 절차 문서화.

태스크: T01 → T02 → T03.

---

## T01 — TypeScript 예시 클라이언트 (`examples/typescript-client/`)

**Must-haves**
- *Truths*
  - 신규 디렉토리 `examples/typescript-client/` (web/과 분리 — 예시는 *카피
    가능한 자기-완결* 단위, web 빌드와 무관).
  - 1 파일 `client.ts` — fetch 기반 minimal client + 응답 타입 정의:
    - `ErrorCategory` / `FailedTransaction` / `TraceLog` / `FailedTxDetail`
      (root_cause + failing_function_decoded + diagnosis 포함) / `Diagnosis`
      / `DecodedFunction` / `AlertSubscription` / `AlertSubscriptionCreated`
      / `FailedTxByLabelPoint` 등 응답 타입
    - `class AmarilloClient { constructor(baseUrl), getFailedTx(hash),
      listFailedTx(filter), getFailedTxTimeseries(filter), getFailedTxByLabel(filter),
      createAlertSubscription(body), listAlertSubscriptions(), deleteAlertSubscription(id),
      rotateAlertSecret(id) }`
    - `verifyAlertSignature(body, signature, signingSecret) -> boolean` (Node
      `crypto` 사용, HMAC-SHA256, hex-decoded 32B key — `docs/api-alerts.md`
      receiver 계약 일관)
  - 1 파일 `examples.ts` — 3 시나리오 main(): (a) 단건 진단 출력,
    (b) 구독 생성 + 시크릿 노출 후 검증 데모, (c) by-label 라벨별 분포 출력.
    `process.argv`로 baseUrl 받고 사용 안내 출력.
  - 1 파일 `README.md` — "복사해서 본인 프로젝트에 붙이세요" 안내 + 의존 0 명시
    + `tsc --noEmit client.ts examples.ts` 검증법.
  - 1 파일 `tsconfig.json` — 최소 (target ES2020, strict, lib `["ES2020", "DOM"]`).
  - 외부 의존 0 — `package.json` 없음(D017). `tsc` 글로벌 또는 web/`npx tsc`로
    typecheck.
  - 신규 public 함수 `///` doc (TS는 JSDoc).
- *Artifacts*: `examples/typescript-client/{client,examples}.ts`, `tsconfig.json`,
  `README.md`
- *Key Links*: `crates/api/src/routes/*.rs`(엔드포인트 계약 단일 출처),
  `web/src/api/contract.ts`(파서 패턴 — 단순화해서 카피, web의 React 의존 제거),
  `docs/api-failed-tx.md` + `docs/api-alerts.md`(receiver HMAC 계약)

## T02 — Python 예시 클라이언트 (`examples/python-client/`)

**Must-haves**
- *Truths*
  - 신규 디렉토리 `examples/python-client/` (자기-완결, stdlib만).
  - 1 파일 `client.py` — `urllib.request` + `json` + `hmac` + `hashlib` 기반:
    - dataclasses(`FailedTransaction`, `TraceLog`, `Diagnosis`, `DecodedFunction`,
      `FailedTxDetail`, `AlertSubscription` 등) + `from_dict` 분리(silent
      default 금지 — `KeyError` 발생).
    - `class AmarilloClient` — `get_failed_tx`/`list_failed_tx`/`get_timeseries`/
      `get_by_label`/`create_alert_subscription`/`list_alert_subscriptions`/
      `delete_alert_subscription`/`rotate_alert_secret`.
    - `verify_alert_signature(body, signature, signing_secret) -> bool` —
      HMAC-SHA256 + hex-decode 32B key, `hmac.compare_digest` 사용.
  - 1 파일 `examples.py` — TS와 같은 3 시나리오 + `sys.argv[1]` baseUrl.
  - 1 파일 `README.md` — 의존 0(stdlib만) + 사용 안내 + `python3 -m py_compile`
    검증법 + `python3 examples.py <baseUrl>` 실행.
  - `pyproject.toml` / `setup.py` 미도입(D017 — 게시는 별 슬라이스).
  - 신규 public 함수 docstring 필수.
- *Artifacts*: `examples/python-client/{client,examples}.py`, `README.md`
- *Key Links*: T01의 TS 클라이언트(타입 일관성 비교 — JSON shape는 양 언어
  동일), `docs/api-alerts.md` receiver 계약

## T03 — Cookbook 문서 + 통합 검증 + S13-SUMMARY

**Must-haves**
- *Truths*
  - 신규 문서 `docs/cookbook.md` — 3 시나리오 각각 curl + TypeScript + Python
    3중 예시:
    1. **단건 진단**: "이 tx 해시가 왜 실패했나"
       - curl 한 줄 + 응답 예시(root_cause / failing_function_decoded /
         diagnosis 강조)
       - TS: `await client.getFailedTx(hash)` + 출력 포매팅
       - Python: `client.get_failed_tx(hash)` + dataclass 활용
    2. **알림 구독 + HMAC 검증**: "특정 카테고리 실패가 발생하면 내 webhook으로"
       - curl POST + signing_secret 1회 노출 응답 강조
       - TS: `createAlertSubscription` + Node Express receiver의
         `verifyAlertSignature(rawBody, signature, secret)` 한 줄
       - Python: 동일 + FastAPI 또는 Flask 한 줄 receiver
    3. **라벨된 컨트랙트 실패 분포**: "내 dApp 컨트랙트별 실패 패턴"
       - curl + 빈 결과 시 메시지 안내
       - TS / Python 호출 + 결과 출력 (라벨 + total + 상위 카테고리)
  - 한 단락: "왜 외부 의존 0인가" — D017 인용 + 사용자가 본인 프로젝트에
    카피해 쓰는 ergonomic.
  - 한 단락: M004 마무리 — 응답 한 호출에 *어디서/뭐가/왜+어떻게* 모두 박힘.
    "Dune이 못 함" framing 재진술 (M001~M004 누적의 결정판).
  - `README.md` 업데이트 — 새 cookbook 링크 + examples 디렉토리 안내(짧게).
  - 검증:
    - `npx tsc --noEmit -p examples/typescript-client/tsconfig.json` 또는 동등 —
      typecheck 통과
    - `python3 -m py_compile examples/python-client/client.py examples/python-client/examples.py` — syntax 통과
    - web 기존 게이트 무회귀 (typecheck/test/build)
    - cargo 게이트(clippy/fmt/cargo test indexer + db lib + db ignored + verify
      스크립트 3종) 무회귀
  - `.gsd/S13-SUMMARY.md` + ROADMAP S13 `[x]`. M004는 *acceptance 완성*이지만
    S11.1 / S12.1 / S13.1 잔여 — `🚧 IN PROGRESS` 유지하거나 `✅ SHIPPED`
    선언? D017 일관: S13으로 *프로덕트화*까지가 M004 ship 정의를 충족 →
    **M004 마감 후보**(사용자 결정 — Reassess에서 검토).
- *Reassess*: S13 출하 후 — M004 ✅ SHIPPED 선언할지(S11.1 / S12.1 / S13.1을
  M005 후보로 이월), 또는 IN PROGRESS 유지하고 잔여 슬라이스 분해할지
  사용자 결정. KNOWLEDGE에 "예시 코드 = SDK = 동일" Lesson 기록 가능.
- *Artifacts*: `docs/cookbook.md`, `README.md`(업데이트), `.gsd/{S13-SUMMARY,M001-ROADMAP}.md`

---

## Slice 수용 (Complete)
- [ ] T01–T03 must-haves, 기존 `/v1/*`·`/alerts`·페이지 무회귀
- [ ] TS `tsc --noEmit` 통과 + Python `py_compile` 통과
- [ ] web 기존 게이트(typecheck + test + build) 무회귀
- [ ] cargo 게이트(clippy/fmt/-p indexer/-p db --lib/-p db --ignored) 무회귀
- [ ] verify-failed-tx / verify-alerts / verify-failed-tx-by-label 3종 무회귀
- [ ] REQUIREMENTS#M004 S13 항목 ✅ + S13-SUMMARY + ROADMAP S13 `[x]`
- [ ] M004 ship 여부 사용자 결정 대기 (default: IN PROGRESS 유지, S11.1/S12.1/S13.1 잔여)
