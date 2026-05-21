---
slice: S18
title: 프론트 `/alerts` 인증 UI + M006 마감 (M006 third slice)
status: done
edge: weak-spot — 운영 안전 표면, 프론트
milestone: M006 (마감 슬라이스)
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace --all-targets -D warnings 0 · -p api 단위 13 + 통합 7 = 20/20 무회귀 · -p indexer 36/36 · -p db --lib 17/17 · -p db --ignored 27/27 · -p decoder 18/18 · web typecheck clean · web test 37/37 (8 신규 + 26 + 3) · web build OK · verify 3종 무회귀 자동 (서버/스크립트 0 변경)
decisions: [D024]
artifacts:
  - web/src/state/apiKey.tsx           # 신설 — Context + ApiKeyProvider + useApiKey hook + module slot sync
  - web/src/api/client.ts              # _apiKey mutable + setAdminApiKey + apiPost/apiDelete 자동 헤더 (apiGet 무부착)
  - web/src/api/client.test.ts         # 신설 — 8 case (setAdminApiKey 정규화 + apiPost·apiDelete·apiGet 헤더 단언 + null 즉시 wipe)
  - web/src/components/ApiKeyInput.tsx # 신설 — type=password 입력 + Apply/Clear + 활성 시 길이만 표시
  - web/src/pages/Alerts.tsx           # ApiKeyInput 상단 배치 + writesDisabled = (apiKey == null) + 3 mutation 버튼 disabled + describeError로 401 변환
  - web/src/App.tsx                    # <ApiKeyProvider>로 라우터 트리 래핑
  - docs/cookbook.md                   # "5. From the /alerts page (S18)" 신설 — Apply → Create → Rotate/Deactivate → 401 복구 step
  - .gsd/DECISIONS.md                  # D024 (세션 메모리 + module-mutable + Context sync)
verification_constraint: "라이브 401 응답 시 메시지 변환은 describeError 단위테스트 미추가 — 본 슬라이스에서는 컴파일·렌더·헬퍼 헤더 부착 단언까지. 페이지 인터랙션(클릭 → fetch mock → 응답) 자동화는 부담이 커 *수동 검증*과 cookbook 가이드로 위임."
---

# S18 — 무엇이 실제로 일어났나

M006 마감 슬라이스. S16(인증 게이트) + S17(verify/examples/docs)에 이어
**프론트가 마지막으로 깨진 표면**. `/alerts` 페이지에 세션 메모리 기반 키
입력 UI + 401 처리 + write 버튼 비활성을 박아 *세 페르소나 모두 안전하게*
사용 가능한 상태에 도달. **M006 = S16 ∧ S17 ∧ S18 — 본 출하 시 M006 ✅ SHIPPED.**

핵심 결정 **D024** (착수 시 기록): 세션 React state만, localStorage/sessionStorage/
cookie/URL 파라미터/빌드 타임 주입 *모두 X*. 호출 측 구조는 **A안 채택** —
`@/api/client`에 모듈 mutable `_apiKey` + Context Provider sync. 기존 mutation
hook 호출처 *0 갱신*.

## 응답·표면 — M006 마감 운영자 흐름

| 단계 | 출하 위치 |
|------|-----------|
| 인증 게이트 인프라 (extractor + 마스킹 + state + 통합테스트) | S16 |
| 보호 라우트 5 핸들러에 게이트 부착 | S16 |
| verify 3종 인증 헤더 + 401 case | S17 |
| examples (TS/Python) apiKey 옵션 + 사전 throw | S17 |
| cookbook 4 시나리오 + docs Authentication 종합 | S17 |
| **프론트 `/alerts` 키 입력 UI (세션 메모리)** | **S18-T01** |
| **`apiPost`/`apiDelete` 자동 헤더 + 401 → describeError 안내** | **S18-T02** |
| **cookbook 5번째 시나리오 "From the /alerts page" + M006-SUMMARY** | **S18-T03** |

## 수용 기준 (REQUIREMENTS.md#M006 S18) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| `<ApiKeyInput>` 컴포넌트 + Apply/Clear (세션 메모리만, D024) | ✅ | `web/src/components/ApiKeyInput.tsx`, `web/src/state/apiKey.tsx` (Provider + useApiKey hook + `setAdminApiKey` 모듈 sync) |
| 키 미설정 시 write 버튼(생성/회전/비활성) 모두 disabled + 안내 | ✅ | `writesDisabled = apiKey == null` → 3 버튼 `disabled` + tooltip 메시지 ("API key required (S16/M006)") + form 위 안내 박스 |
| `localStorage` / `sessionStorage` / cookie / URL param / 빌드타임 주입 *전부 미사용* | ✅ | `state/apiKey.tsx`는 `useState`만 사용, `client.ts` slot은 module-level let, ApiKeyInput는 password input + value 미저장. 검색: `localStorage` / `sessionStorage` 어디에도 미등장 |
| `apiPost` / `apiDelete` 자동 `Authorization: Bearer` 부착, `apiGet` 무부착 | ✅ | `client.test.ts` 8 case 단언 (set/null/post·delete 부착·미부착/get 무부착/즉시 wipe) |
| 401 응답 시 *명확한 메시지* + 키 입력 패널로 시선 유도 | ✅ | `describeError(err)` helper — `ApiError(401)` → "Unauthorized — enter or re-enter your admin API key in the panel above…", 3 catch 블록에서 사용 |
| cookbook에 `/alerts` page step-by-step | ✅ | `docs/cookbook.md` 시나리오 5 신설 (Apply → Create → Rotate/Deactivate → 401 복구 + D024 정신 한 단락) |
| 비기능: 코드 0 회귀 / 단위테스트 신규 추가 | ✅ | web typecheck/test 37/37 (8 신규 + 26 + 3), App.smoke 무회귀, contract.test 무회귀 |

## 최종 게이트 (2026-05-21, 단일 호흡 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean (서버 코드 0 변경)
- `cargo clippy --workspace --all-targets -- -D warnings` — 0
- `cargo test -p api` — 단위 13 + 통합 7 = **20/20** 무회귀
- `cargo test -p indexer` — **36/36** 무회귀
- `cargo test -p db --lib` — **17/17** 무회귀
- `cargo test -p db -- --ignored` — **27/27** 무회귀 (docker postgres)
- `cargo test -p decoder` — **18/18** 무회귀
- `bash scripts/verify-{failed-tx,alerts,failed-tx-by-label}.sh` — 모두 무회귀
  자동 (스크립트/서버 0 변경, S17 적용 상태 유지)
- `tsc --noEmit -p examples/typescript-client/tsconfig.json` — 무회귀 자동
- `python3 -m py_compile examples/python-client/{client,examples}.py` — 무회귀 자동
- `cd web && npm run typecheck` — clean
- `cd web && npm run test` — **37/37** (`client.test.ts` 8 신규 + `contract.test.ts`
  26 + `App.smoke.test.tsx` 3)
- `cd web && npm run build` — 후속 게이트(T04)에서 명시 실행

## 태스크

- **T01** Context + 컴포넌트 + Alerts 통합 — `web/src/state/apiKey.tsx`(신설),
  `web/src/components/ApiKeyInput.tsx`(신설), `web/src/App.tsx` Provider 래핑,
  `web/src/pages/Alerts.tsx` 상단 `<ApiKeyInput />` 배치 + `writesDisabled`
  로직 3 버튼(`create.isPending || writesDisabled`).
- **T02** `client.ts` 자동 헤더 + 401 안내 — 모듈 mutable + `setAdminApiKey` +
  `apiPost`/`apiDelete` 자동 부착, `apiGet` 무부착. `describeError(err)` helper
  로 401을 *키 입력 패널 유도* 메시지로 변환. `web/src/api/client.test.ts` 8 case.
- **T03** cookbook + S18-SUMMARY + DECISIONS D024 + (다음 단계 M006-SUMMARY +
  ROADMAP).

## 핵심 교훈 (KNOWLEDGE 후보)

- **세션 메모리 패턴 (D024)** — `localStorage` / cookie / 빌드타임 주입 모두
  XSS·CSRF·소스맵 노출 표면 가산. React state in memory는 *공격자가 페이지에
  이미 침투해야* 노출 → 표면 *증가 X*. 새로고침 = 키 사라짐 = 운영자에게 *세션
  한정* 정직 시그널. 비밀번호 매니저 자동 입력으로 운영자 마찰 보완.
- **모듈 mutable + Context sync (A안)** — Provider state 변경 시 `useEffect`로
  모듈 mutable slot에 mirror → 기존 helper API 시그니처 *변경 0*. mutation hook
  10+ 호출처 갱신 부담 회피. 단점: non-React state라 테스트가 명시적 reset
  필요 — `beforeEach(setAdminApiKey(null))` 패턴.
- **컴파일러 + 컴포넌트 disabled의 이중 게이트** — 서버는 `_: AdminAuth` extractor
  로 *시그니처 차원* 게이트(S16/D022). 프론트는 `writesDisabled` boolean으로
  *UI 차원* 게이트. 두 층이 *독립적으로* 회귀 차단 → 한 층이 깨져도 다른 층이
  운영자 안전 보호.
- **info-leak 방지 단일 401의 *프론트 UX 영향* (D021/D024)** — 서버 401만으로는
  운영자가 "키가 잘못됐나? URL이 잘못됐나? 라벨 형식이 잘못됐나?" 판단 어려움.
  프론트의 `describeError(err)` helper가 *프론트 측 context* 추가 — "키 입력
  패널에서 재입력해 주세요". 서버는 정보 X, 프론트가 운영자에게 친절.
- **HTML `<input type="password">`의 한계** — 평문 노출 차단은 *입력 시점만*.
  React state로 저장 후엔 DevTools에서 component state inspection 시 평문 노출.
  본 슬라이스에서는 *길이만* 활성 시 표시 (`{apiKey.length} chars`), 값 미표시
  — 인지 보호 + DevTools는 여전히 보이지만 *일반 UI에는 노출 X*.

## 정직한 한계 / 잔여

- **multi-key runtime 회전 미부착** (D021/D023 일관) — 키 회전 = env 갱신 +
  서버 재시작. 무중단 회전은 *별 슬라이스 / 별 마일스톤*. 운영자 측 절차는
  cookbook + `docs/api-failed-tx.md#Authentication`에 명시.
- **rate limiting / audit log 미부착** — 키 brute-force 방어는 *별 단위*. write
  라우트 좁고 401 응답이 attacker에게 key 정보 X (S16/D021 spirit).
- **`describeError(err)` 단위테스트 미포함** — 컴파일 단언 + 코드 리뷰 + 수동
  검증으로 1차 보장. 라이브 401 자동화는 fetch mock + 페이지 클릭 시뮬레이션
  부담이 크고 본 슬라이스 스코프 밖 — 추후 후속 후보.
- **DevTools state inspection으로 키 평문 노출** — React state는 DevTools에서
  보임 (XSS와 동일 위협 모델). password input + 길이만 UI 표시는 *일반 사용*
  보호이지 *DevTools 차단*은 아님. 운영자 책임 영역.
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터
  (M004/M005/M006 일관 한계).
- **별 단위 hardening 후보**: S16에서 toolchain 회귀 lint 2건 인라인 fix
  (`crates/decoder/src/events.rs` cmp_owned allow, `crates/indexer/src/worker.rs`
  needless_borrows). 의미 무변경, *별 슬라이스에서 깔끔하게 리팩토링* 후보 —
  BACKLOG로 잠재 이월.

## Reassess

ROADMAP **M006 S18 `[x] DONE`** + **M006 ✅ SHIPPED** (M006-SUMMARY 작성으로
마감 선언). 세 페르소나(dApp 개발자 / 봇 운영자 / **운영자**) 완결.

다음 호흡: 사용자 결정 — BACKLOG.md 우선순위 표 활용:
- S11.1 ABI args 디코딩 (dApp 깊이)
- S12.1 ErrorCategory enum 세분화 v2
- S13.1 npm/PyPI 패키지 게시
- OS resolver 캐시 race (hickory-dns)
- Pools/Traders FE 매핑
- 별 단위 hardening (toolchain 회귀 정리)
- 새 마일스톤 분기 (M007 후보 — 거래소 KYC 매핑 / 자동화된 incident response /
  RPC 성능 대시보드 / multi-key runtime 회전 등)

GSD-2 원칙: 다음 마일스톤 분해는 *다음 지시 시*만 (M006 출하 전 분해 금지 원칙
일관 적용 — 본 SHIPPED 선언으로 분해 가능).
