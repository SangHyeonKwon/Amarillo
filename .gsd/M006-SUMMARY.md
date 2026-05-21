---
milestone: M006
title: Operator Auth
status: SHIPPED
date: 2026-05-21
slices: [S16, S17, S18]
gate: pass   # fmt clean · clippy --workspace --all-targets -D warnings 0 · -p api 20/20 · -p indexer 36/36 · -p db --lib 17/17 · -p db --ignored 27/27 · -p decoder 18/18 · verify 3종 ALL PASS · tsc clean · py_compile clean · web typecheck/test 37/37/build OK
decisions: [D021, D022, D023, D024]
ship_definition: "amarillo의 모든 write/admin 엔드포인트가 API key 인증으로 보호되고, 모든 검증·예시·프론트 흐름이 인증된 호출로 동작한다."
---

# M006 — SHIPPED

출하 정의(REQUIREMENTS.md#M006): "amarillo의 모든 write/admin 엔드포인트가
API key 인증으로 보호되고, 모든 검증·예시·프론트 흐름이 인증된 호출로 동작한다."

페르소나 = **운영자**(D021). M001~M005까지의 "데모 스코프 인증 미부착"(D008/
D013/D019) 정직성을 *운영 게이트*로 마감 — 외부 노출 가능 상태에 도달.
**(A) API key Bearer + (X) write/admin만 보호 + (1) env 단일 키 + (D024) 프론트는
세션 메모리** 묶음. 세 페르소나(dApp 개발자 / 봇 운영자 / **운영자**) 모두
*안전*하게 사용 가능.

## 응답·표면 — 운영자 완결 흐름

| 단계 | 슬라이스 | 표면 |
|------|---------|------|
| **부팅** — `AMARILLO_ADMIN_API_KEY` env 강제, 빈/미설정 = 부팅 실패 | S16 | `ApiConfig::from_env_with` 거부, `tracing::warn!` 짧은 키 |
| **게이트** — write/admin 라우트 5개에 `_: AdminAuth` extractor (컴파일러 회귀 차단) | S16 | `crates/api/src/auth.rs` + `subtle::ConstantTimeEq` 상수시간 비교, 단일 401 응답 |
| **검증** — verify 스크립트 3종이 env 강제 + 인증 헤더 + 401 case | S17 | `scripts/verify-{failed-tx,alerts,failed-tx-by-label}.sh` |
| **예시** — TS/Python 클라이언트 `apiKey` 옵션 + write 자동 헤더 + 사전 throw | S17 | `examples/typescript-client/client.ts`, `examples/python-client/client.py` |
| **문서** — cookbook 4 시나리오 + docs Authentication 종합 섹션 | S17 | `docs/cookbook.md`, `docs/api-failed-tx.md` |
| **프론트** — `/alerts` 페이지 키 입력 UI(세션 메모리) + write 버튼 비활성 + 401 안내 | S18 | `web/src/pages/Alerts.tsx`, `web/src/state/apiKey.tsx`, `web/src/components/ApiKeyInput.tsx` |
| **playbook** — cookbook "From the /alerts page" step-by-step | S18 | docs/cookbook.md 시나리오 5 |

## 수용 기준 (REQUIREMENTS.md#M006) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| `AMARILLO_ADMIN_API_KEY` 미설정 또는 빈 → 부팅 실패 | ✅ | S16 `ApiConfig::from_env_with` 단위테스트 6 + docker-compose `${VAR:?required}` |
| 보호 라우트 5개 — `_: AdminAuth` extractor 게이트 | ✅ | S16 `crates/api/src/auth.rs` + 핸들러 5 시그니처 + 통합테스트 7 |
| 401 단일 응답 (헤더 누락/형식/키 불일치 모두 동일) | ✅ | S16 `ApiError::Unauthorized` 고정 메시지 + `subtle::ConstantTimeEq` 상수시간 비교 + `tests/auth.rs` 5 case loop |
| 키 비교 상수시간 + 로그 미노출 | ✅ | S16 `subtle` 사용 + `ApiConfig::fmt::Debug` 마스킹(database_url + admin_api_key) |
| verify 3종 인증 헤더 + 401 case | ✅ | S17 verify-alerts/by-label 401 case 2건씩 (키 누락 + 잘못된 키), verify-failed-tx env 강제 |
| examples 클라이언트(TS/Python) `apiKey` 옵션 가산 + write 자동 헤더 | ✅ | S17 `ClientOptions{apiKey?}` / `api_key=None`, 5 write 메서드 `auth: "admin"`/`auth=True`, 키 없으면 사전 throw/raise |
| cookbook 4 시나리오 인증 헤더 + 401 사례 | ✅ | S17 글로벌 Authentication note + 시나리오 2/4 갱신 + "If you forget the API key" 절 |
| docs Authentication 종합 섹션 | ✅ | S17 `docs/api-failed-tx.md` 상단 `## Authentication` (env 정책 + 보호 표 5 + 401 응답 + curl + 회전 절차 + JWT/OAuth 미선택 이유) |
| 프론트 `/alerts` 키 입력 UI (세션 메모리) + write 버튼 비활성 | ✅ | S18 `<ApiKeyInput>` + `<ApiKeyProvider>` + `writesDisabled` + 3 mutation 버튼 disabled + form 위 안내 박스 |
| 프론트 401 처리 → 키 입력 패널 유도 메시지 | ✅ | S18 `describeError(err)` helper, 3 catch 블록에서 사용 |
| cookbook 프론트 사용법 step-by-step | ✅ | S18 cookbook 시나리오 5 "From the /alerts page" 신설 |
| 비기능: prod unwrap 0 / 파라미터화 SQL / `///` doc / 멱등 마이그레이션 | ✅ | 전체 워크스페이스 clippy/fmt clean, 신규 SQL 없음 (인증은 env-only — D023) |

## 최종 게이트 (2026-05-21, 단일 호흡 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — 0
- `cargo test -p api` — 단위 13 + 통합 7 = **20/20**
- `cargo test -p indexer` — **36/36**
- `cargo test -p db --lib` — **17/17**
- `cargo test -p db -- --ignored` — **27/27** (docker postgres)
- `cargo test -p decoder` — **18/18**
- `bash scripts/verify-failed-tx.sh` — **ALL PASS** (env 강제)
- `bash scripts/verify-alerts.sh` — **ALL PASS** (인증 헤더 + 401 case 2)
- `bash scripts/verify-failed-tx-by-label.sh` — **ALL PASS** (인증 헤더 + 401 case 2)
- `tsc --noEmit -p examples/typescript-client/tsconfig.json` — clean
- `python3 -m py_compile examples/python-client/{client,examples}.py` — clean
- `cd web && npm run typecheck` — clean
- `cd web && npm run test` — **37/37** (8 신규 `client.test.ts` + 26 `contract.test.ts` + 3 `App.smoke`)
- `cd web && npm run build` — OK (S18-T04 게이트에서 명시 실행)

## 슬라이스

- **S16** 인증 미들웨어 + 보호 게이트 `[edge: untapped — 운영 게이트]` — D021/D022/D023 (A+X+1 묶음 / extractor 게이트 / env 단일 키). → S16-SUMMARY.md
- **S17** verify 스크립트 + examples + cookbook 인증 `[weak-spot — 운영 안전 표면]` — 새 결정 없음. 호환 우선 (방향 A) 패턴 일관. → S17-SUMMARY.md
- **S18** 프론트 `/alerts` 인증 UI + M006 마감 `[weak-spot — 운영 안전 표면, 프론트]` — D024 (세션 메모리 + module-mutable + Context sync). → S18-SUMMARY.md

## 핵심 교훈 (KNOWLEDGE 후보)

- **세 페르소나 완결의 *대칭성*** — dApp 개발자(M001~M004) + 봇 운영자(M005) +
  운영자(M006). 각 페르소나는 *다른 표면*에서 가치를 받지만, 운영자는 *모든
  표면을 가로지르는* 보호 정책. M006이 *backbone 마일스톤* — 다른 페르소나의
  안전을 *동시에* 끌어올림.
- **info-leak 방지 단일 401 (D021)** — 헤더 누락/Bearer 형식/키 불일치/길이
  불일치 모두 같은 401. timing/oracle 공격 표면 최소화. 트레이드오프:
  클라이언트 사이드 디버깅 마찰 ↑. 보완: examples 클라이언트가 *사전 throw*
  (TS `AmarilloError(0)`, Python `ValueError`), 프론트가 *키 입력 패널 유도*
  (`describeError`).
- **컴파일러 + 시그니처 + UI 세 층 게이트** — 서버는 `_: AdminAuth` extractor
  (D022 — 컴파일 시점 회귀 차단), examples는 helper `_request(auth: 'admin')`,
  프론트는 `writesDisabled` boolean. 세 층이 *독립적으로* 회귀 차단 → 한 층이
  깨져도 다른 층이 운영자 안전 보호.
- **env 단일 키 + 세션 메모리 = 대칭 패턴 (D023/D024)** — 서버는 env에서
  부팅 시 1회 로드(미설정 = 부팅 실패), 프론트는 사용자 입력으로 세션에
  1회 로드(refresh = 사라짐). 두 층 모두 *re-entry로만 회전 / 갱신*. 회전 =
  운영자 *명시 action*. 무중단 회전은 별 슬라이스 (multi-key runtime).
- **호환 우선 옵션 패턴 (방향 A)** — `new AmarilloClient(baseUrl, { apiKey? })` /
  `AmarilloClient(base_url, api_key=None)` / module mutable + Context sync.
  기존 사용자 갱신 *0건*. options-object 방향(B)이 더 깔끔하지만 마이그레이션
  비용이 가산 가치를 이김. *호환성 > 유연성*.
- **시그니처 회귀 차단의 정신적 효과** — `_: AdminAuth`가 핸들러 시그니처에
  *반드시* 있어야 컴파일됨 → 새 write 라우트 추가 시 *깜빡* 회귀를 컴파일러가
  거부. 보호 표(routes/mod.rs doc + 통합테스트 401 case)와 *3중 신호* —
  관찰자 다양성이 회귀 갭을 메움.

## 정직한 한계 / 잔여 (M007 후보 또는 단독)

- **multi-key runtime 회전 미부착** (D021/D023/D024 일관) — 키 회전 = env 갱신 +
  서버 재시작 + 모든 클라이언트 동시 갱신. 무중단 회전(DB 키 테이블 + 활성
  키 다중 보유)은 *별 슬라이스 / 별 마일스톤* — 첫 사용자 요구 후 (D017 정신).
- **rate limiting / audit log 미부착** — write 라우트 brute-force 방어는 별
  단위. 401 응답이 attacker에게 key 정보 X (S16/D021), 401 표면은 좁음 (write
  5개만). audit log 부재 — 운영자별 작업 추적 X, IP/UA 로그에 의존.
- **JWT / OAuth / scope 미선택** (D021) — 호출 패턴이 *server↔server only*,
  사람 OAuth 흐름 / per-tenant scope / short-lived expiry는 *현재 요구 없음*.
  최소 형태 유지 — 첫 사용자 요구 후 진화 가능.
- **DevTools state inspection으로 키 평문 노출 (프론트)** — React state는
  DevTools에서 보임. password input + 길이만 UI 표시는 *일반 사용* 보호이지
  *DevTools 차단*은 아님 (XSS와 동일 위협 모델 — D024 정신).
- **세션 메모리 = 새로고침 시 사라짐 (D024 의도)** — 운영자 UX 트레이드오프:
  편의보다 안전. 비밀번호 매니저 자동 입력 권고는 cookbook에 명시.
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터
  기반 (M001~M005 일관 한계).
- **별 단위 hardening — toolchain 회귀 lint 2건** (decoder/events.rs cmp_owned
  allow, indexer/worker.rs needless_borrows): S16에서 인라인 fix. 의미 무변경,
  *별 슬라이스에서 깔끔하게 리팩토링* 후보 — BACKLOG로 잠재 이월.

## 백로그 (BACKLOG.md 참조)

M006 마감 — 본 항목은 BACKLOG에서 *완료 표*로 압축. 다음 호흡 후보 (BACKLOG.md
우선순위 표):

- **S11.1 ABI args 디코딩** (M004 dApp 깊이 — 진단 정밀도)
- **S12.1 ErrorCategory enum 세분화 v2** (M004 정밀도)
- **S13.1 npm / PyPI 패키지 게시** (M004 운영성)
- **OS resolver 캐시 race** (HARDEN3 잔여 SSRF 갭, 첫 요구 후)
- **Pools/Traders FE** (D001 정신 — 거의 안 해도 무방)
- **별 단위 hardening** (toolchain 회귀 정리, S16에서 인라인 fix → 별 PR로 깔끔하게)

또는 **M007 분기** (새 페르소나 / 새 마일스톤):
- multi-key runtime 회전 (운영자 무중단 회전)
- 거래소 KYC 매핑 (D013 라벨 패턴 확장)
- 자동화된 incident response (S08/S14 알림 → 자동 action)
- RPC 성능 대시보드 (운영 관측성)

## Reassess

ROADMAP **M001 ~ M006 모두 ✅ SHIPPED**. 세 페르소나 완결 — 제품의 *주요 사용자
그룹* 모두 *안전하게* 진단·구독·관리 가능. 다음 호흡은 *직접 가치 가산*(M004
잔여 / 패키지 게시) 또는 *새 페르소나 확장*(M007 분기) — 사용자 결정 시.

GSD-2 원칙: 다음 마일스톤 분해는 *다음 지시 시*만 (M006 출하 전 분해 금지 원칙
일관 적용 — 본 SHIPPED 선언으로 분해 가능).
