---
slice: S13
title: 개발자 예시 클라이언트(TS+Python) + cookbook (M004 마무리 슬라이스)
status: done
edge: weak-spot
milestone: M004
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace 0 · web typecheck/test 26/build 900 modules OK · TS tsc --noEmit (examples) clean · Python py_compile clean (python3.13 + python3.6 모두) · cargo 코드 무변경 → 회귀 없음
decision: D017
artifacts:
  - examples/typescript-client/client.ts          # AmarilloClient + 모든 wire types + verifyAlertSignature
  - examples/typescript-client/examples.ts        # 3 시나리오 main()
  - examples/typescript-client/tsconfig.json      # types=[] + lib=ES2020,DOM + allowImportingTsExtensions
  - examples/typescript-client/ambient.d.ts       # node:crypto + Buffer + process 최소 declare (npm 의존 0)
  - examples/typescript-client/README.md          # 사용법 + Express receiver outline
  - examples/python-client/client.py              # AmarilloClient + dataclasses + verify_alert_signature
  - examples/python-client/examples.py            # 3 시나리오 main()
  - examples/python-client/README.md              # 사용법 + Flask receiver outline
  - docs/cookbook.md                              # 3 시나리오 curl + TS + Python 3중 walkthrough + M004 한 단락
  - README.md                                     # 새 "Client examples & cookbook" 섹션 + Failure Intelligence API 표 갱신
  - .gsd/DECISIONS.md                             # D017 (예시 = SDK 동일, 게시는 S13.1)
verification_constraint: "예시 클라이언트는 typecheck/syntax까지만 자동 검증 — 라이브 호출은 사용자가 docker compose 환경에서 수동 스모크(README/cookbook 명시). M004는 본 슬라이스 출하로 acceptance 완성됐으나 SHIPPED 선언은 사용자 결정 대기(잔여 S11.1 / S12.1 / S13.1는 M005 후보)."
---

# S13 — 무엇이 실제로 일어났나

REQUIREMENTS#M004의 *프로덕트화 단계*. M001~M003 (data / real-time /
alerts) + S10~S12 (depth) 가산이 *작동하는 카피 가능한 코드*로 결정 가능
하게 됨. dApp 개발자는 `examples/typescript-client/client.ts` 또는
`examples/python-client/client.py`를 자기 프로젝트에 *그대로 붙여* 사용
가능 — `npm install` / `pip install` 절차 없음.

- **T01 (TypeScript 예시 클라이언트 + D017)**: `examples/typescript-client/`
  4 파일:
  - `client.ts` — `AmarilloClient` 전 엔드포인트(failed-tx 단건/목록/timeseries/
    by-label + alert 구독 4종) + 모든 wire types(`FailedTxDetail` = S10/S11/S12
    가산 포함) + `verifyAlertSignature(rawBody, header, secret)` 함수
    (`node:crypto` HMAC-SHA256, hex-decoded 32B key, `timingSafeEqual`)
  - `examples.ts` — 3 시나리오 main (단건 진단 / 알림 구독 + HMAC 검증 데모
    / by-label 분포)
  - `tsconfig.json` — `types: []` + `lib: ["ES2020", "DOM"]` + `allowImportingTsExtensions`
    (외부 의존 0 보장)
  - `ambient.d.ts` — `node:crypto` / `Buffer` / `process` 최소 ambient
    declare (npm 의존 0이지만 typecheck 통과)
  - `README.md` — 사용법 + Express receiver outline
  검증: `./web/node_modules/.bin/tsc --noEmit -p examples/typescript-client/tsconfig.json`
  통과. D017 결정 기록(예시 = SDK 동일, 게시는 S13.1).

- **T02 (Python 예시 클라이언트)**: `examples/python-client/` 3 파일:
  - `client.py` — `AmarilloClient` 전 엔드포인트 + `@dataclass(frozen=True)`
    wire types + `from_dict` 분리(silent default 금지 — `KeyError`) +
    `verify_alert_signature` (`hmac.compare_digest` 상수시간 비교)
  - `examples.py` — TS와 동일 3 시나리오
  - `README.md` — 사용법 + Flask receiver outline
  외부 의존 0 (stdlib만 — `urllib.request`/`json`/`hmac`/`hashlib`/`dataclasses`/
  `typing`). 검증: `python3 -m py_compile` 통과 (Python 3.13 + 3.6 모두 — 후자는
  `from __future__ import annotations` 제거로 호환). Python 3.7+ 호환 명시.

- **T03 (cookbook + README + 전체 게이트 + S13-SUMMARY)**: `docs/cookbook.md`
  신규 — 3 시나리오 각각 `curl` + TypeScript + Python 3중 예시:
  1. 단건 진단 (`failing_function_decoded` / `diagnosis` / `root_cause` 활용)
  2. 알림 구독 + Express/Flask receiver에서 HMAC 검증
  3. by-label 라벨된 컨트랙트 실패 분포
  + "왜 npm/pip install이 없는가" 단락(D017 인용) + "M004 in one paragraph"
  단락(M004 핵심 framing 재진술).

  README.md 갱신:
  - Failure Intelligence API 표에 `root_cause`/`failing_function_decoded`/
    `diagnosis` 추가 + `by-label` + `alert-subscriptions` 행 추가
  - 새 섹션 "Client examples & cookbook" — `examples/typescript-client/`,
    `examples/python-client/`, `docs/cookbook.md` 링크

  게이트 ALL GREEN:
  - `cargo fmt --check` clean · `cargo clippy --workspace -- -D warnings` 0
    (코드 무변경 — 회귀 가드)
  - web `npm run typecheck/test/build` clean / 26/26 / 900 modules
  - TS examples `tsc --noEmit` clean · Python examples `py_compile` clean

**해자(D002~D017)의 *프로덕트 표면*이 코드로 박힘**
- dApp 개발자가 5분 안에 `client.ts` 또는 `client.py` 카피 → `getFailedTx`
  호출 → root_cause/decoded/diagnosis 받음. 우리 API의 *결정적 차별 자산*이
  사용자 코드에서 *즉시 작동*.
- 외부 의존 0 정책(D017)은 *프로덕트 표면의 첫 번째 신호*: "이 API를 쓰는데
  뭐 받을 게 없다" — 그 자체로 가치 신호.
- cookbook의 3중 예시는 *언어 선택의 자유* + *복사-수정 가능한 출발점*.
  curl/TS/Python 어느 쪽이든 동일 시나리오 → 클라가 자기 스택에 맞춰 픽업.

**정직한 한계**
- 예시 클라이언트는 *typecheck/syntax* 자동 검증까지만 — 라이브 호출은 docker
  compose + verify 스크립트와 동일 환경 요구, 본 슬라이스에서는 README/cookbook
  의 수동 스모크 절차로 위임.
- npm / PyPI 게시 미도입(D017) — semver / 게시 토큰 / CI / 종속 그래프 관리는
  별 슬라이스(`S13.1` sketch). 첫 사용자가 실제 요청하기 전엔 인프라 비용 큼.
- ABI args 디코딩(S11.1) + enum 세분화(S12.1) 잔여 — *진단 깊이*의 추가
  정밀도. 본 슬라이스는 그것들 없이도 *프로덕트로 사용 가능*함을 시연.
- 인증 미연결(D008 일관) — `alert-subscriptions` 등 *쓰기* 엔드포인트도 인증
  없음. 운영 시 별 단위.

**M004 진행 / Reassess**
- S10 ✅ root_cause attribution (*어디서*)
- S11 ✅ selector decoding (*어떤 함수가*)
- S12 ✅ category diagnosis + action (*왜 + 어떻게*)
- S13 ✅ TS + Python 예시 클라이언트 + cookbook (*어떻게 쓰나*) ← 본 슬라이스
- **M004 acceptance 완성** — 응답 4축(failed / root_cause / decoded /
  diagnosis) + 사용자 도달 가능성(examples + cookbook)이 모두 박힘.

**M004 ship 결정 (사용자 대기)**:
- 옵션 A: **M004 ✅ SHIPPED 선언**. 잔여(S11.1 ABI args / S12.1 enum 세분화 /
  S13.1 패키지 게시)는 M005+ 후보로 이월. *프로덕트로 출하 가능*한 표면이 박혔다는
  사실의 공식화.
- 옵션 B: **IN PROGRESS 유지**. 잔여 슬라이스를 M004 내에서 마무리. 정밀도/
  운영성 ↑ 이지만 *마감 vs 추가*의 trade-off.
- 기본: **IN PROGRESS 유지**(M004-SUMMARY 미작성). 사용자가 SHIPPED 선언 시
  M004-SUMMARY.md 작성 + ROADMAP M004 `[x] SHIPPED` 표기.

KNOWLEDGE 추가 후보: "예시 코드 = SDK = 동일 (외부 의존 0 정책의 직접
결과)" — D017 정신이 KNOWLEDGE Pattern으로 일반화 가치 있음. 본 슬라이스에서는
SUMMARY로 충분, 추후 슬라이스에서 패턴 재사용 시 KNOWLEDGE 등재 검토.

ROADMAP M004 S13 `[x]` 표기. M004 마감 여부 사용자 결정 대기. 백로그(DNS-rebinding
SSRF / 임계율 집계 / Pools·Traders 매핑 / 인증)는 단독 단위 유지.
