# S18 — 프론트 `/alerts` 인증 + M006 마감 · PLAN

Slice 목표: **M006 마감 슬라이스**. S16(인증 게이트) + S17(verify/examples/docs)에
이어 *프론트가 마지막으로 깨진 표면*. `/alerts` 페이지에 세션 메모리 기반
키 입력 UI + 401 처리 + write 버튼 비활성을 박아 *세 페르소나 모두* 안전하게
사용 가능한 상태로 마감. M006 = S16 ∧ S17 ∧ S18; 본 슬라이스 출하 시 M006
✅ SHIPPED.

엣지: `[edge: weak-spot — 운영 안전 표면, 프론트]`. risk: low. deps: M001~M005 +
S16 + S17. **M006 마감 슬라이스** — 본 출하 시 M006 ✅ SHIPPED 선언.

핵심 결정: **D024** (착수 시 기록) — 키 저장 *세션 React state만*, localStorage/
sessionStorage 미사용 (XSS 표면 최소화). `NEXT_PUBLIC_*` 빌드 타임 주입도 X
(번들에 박힘 = 소스맵 노출). 사용자가 *런타임에 직접 입력*. 새로고침하면 사라짐
— 의도된 *운영 안전 시그널*. 호출 측 구조는 **A: module-level mutable + Context
sync** — Context Provider state가 모듈 helper의 mutable slot을 sync. 기존
`apiPost`/`apiDelete` 호출처 *변경 0*. (대안 B: helper signature 변경은 hooks.ts
모든 mutation hook 갱신 부담.)

검증 제약(D009~D023 일관): T01 web typecheck/test + 컴포넌트 smoke, T02 401
재현은 *런타임*에서 — 자동 테스트는 helper 단위(자동 헤더 부착 단언), T03
verify 3종 무회귀 자동(코드 0 변경), cargo workspace 무회귀 자동.

태스크: T01 → T02 → T03 → T04 (게이트 + 커밋 + PR).

---

## T01 — ApiKey Context + ApiKeyInput 컴포넌트 + write 버튼 비활성 로직

**Must-haves**
- *Truths*
  - `web/src/state/apiKey.tsx` **신설**:
    - `ApiKeyContext` (React.Context, default `{ apiKey: null, setApiKey: noop }`)
    - `ApiKeyProvider` — `useState<string | null>(null)` + `useEffect`로 *module-level
      mutable*과 sync (D024 A안). state는 메모리만 — localStorage/sessionStorage X.
    - `useApiKey(): { apiKey: string | null; setApiKey: (k: string | null) => void }`
      hook.
  - `web/src/api/client.ts` (T02에서 함께 갱신) — 모듈 레벨 `let _apiKey: string | null = null`
    + `export function setAdminApiKey(k: string | null) { _apiKey = k; }`. `apiPost`/`apiDelete`가
    `_apiKey` 있으면 `Authorization: Bearer ${_apiKey}` 자동 부착.
  - `web/src/components/ApiKeyInput.tsx` **신설**:
    - state: `value: string`, `applied: boolean`, error message (간단).
    - `<input type="password">` (마스킹) + "Apply" 버튼.
    - Apply 클릭 → trim → 빈 문자열 거부 → `setApiKey(value)` 호출 → `applied=true`,
      입력 비활성 + "Clear" 버튼 표시.
    - Clear → `setApiKey(null)` → state 리셋. *재입력 강제*.
    - 안내 문구: "Session memory only — clears on refresh (XSS 표면 최소화 — D024).
      Recommended: 32+ bytes (hex 64 chars)."
  - `web/src/pages/Alerts.tsx` 상단에 `<ApiKeyInput />` 배치 + write 버튼/액션
    비활성 — `useApiKey()`로 `apiKey === null`이면 disabled + 안내 텍스트:
    - "Create subscription" 버튼 disabled
    - "Rotate secret" / "Deactivate" 행 액션 disabled
    - 모든 disabled 위치에 `<title>` 또는 sibling text: "API key required (S16/M006)"
  - `web/src/App.tsx` 또는 entry — `<ApiKeyProvider>`로 라우터 트리 감싸기.
  - **테스트** — `Alerts.smoke.test.tsx` 무회귀 + 신규 단순 어서션:
    - 키 미입력 시 "Create" 버튼 disabled
    - 키 입력 + Apply 시 enabled
    - (`react-testing-library` 패턴 — 기존 smoke 테스트 패턴 활용)
- *Artifacts*: `web/src/state/apiKey.tsx`(신설), `web/src/components/ApiKeyInput.tsx`(신설),
  `web/src/api/client.ts` (T02), `web/src/pages/Alerts.tsx`, `web/src/App.tsx`
- *Key Links*:
  - 기존 Alerts.tsx mutation 패턴 (`useCreateAlertSubscription` etc)
  - 기존 `_request` 인증 헤더 부착 패턴(TS examples client, S17)

## T02 — `client.ts` 자동 Authorization 헤더 + 401 명확 메시지

**Must-haves**
- *Truths*
  - `web/src/api/client.ts`:
    - 모듈 레벨 `let _apiKey: string | null = null;` + `export function setAdminApiKey(k: string | null)`.
    - `apiPost`: `_apiKey != null`이면 `headers["Authorization"] = "Bearer " + _apiKey`.
    - `apiDelete`: 동일.
    - `apiGet`: **무변경** (GET은 공개 라우트 — 임베드성 보존, S17/D021/X 정책).
    - 401 응답 처리 — 기존 ApiError throw 로직 그대로(메시지가 `"unauthorized"`이므로
      자연스러움). 추가: ApiError가 status === 401일 때 사용자 친화 메시지로
      변환할 수 있도록 *helper 또는 컴포넌트 측 핸들링*에서 처리. (helper 자체는
      *server 메시지* 보존 — 정직성.)
  - Alerts 페이지의 mutation 실패 시 — `error.status === 401`이면 *키 입력 폼
    쪽으로 시선 유도*하는 명확한 배너 텍스트 ("Unauthorized — check or re-enter
    your API key. The key is in session memory only and clears on refresh.").
  - 단위테스트: `web/src/api/client.test.ts` 또는 새 파일:
    - `setAdminApiKey("foo")` 후 `apiPost` 호출 시 Authorization 헤더 부착 단언
      (fetch mock 사용)
    - `setAdminApiKey(null)` 후 `apiPost` 호출 시 Authorization 미부착
    - `apiGet`은 키 설정과 무관 (헤더 X)
  - 기존 `web/src/api/contract.test.ts` 무회귀.
- *Artifacts*: `web/src/api/client.ts`, `web/src/api/client.test.ts`(신설 또는 확장),
  `web/src/pages/Alerts.tsx` (401 에러 UX)
- *Key Links*:
  - S17 examples TS client `_request` 자동 헤더 부착 패턴
  - 기존 `apiPost`/`apiDelete` 시그니처 — *변경 없음*이 핵심 (호출처 0 갱신)

## T03 — cookbook 신설 절 + S18-SUMMARY + M006-SUMMARY + ROADMAP M006 ✅

**Must-haves**
- *Truths*
  - `docs/cookbook.md`에 신규 절 추가 — "From the `/alerts` page (S18)":
    - 키 입력 → "Apply" → 활성화된 write 버튼들 사용 흐름 step-by-step
    - 새로고침 시 키 사라짐 = 세션 안전 시그널 (D024)
    - 401 응답이 페이지 배너로 표시 (시나리오 + 복구 방법: Clear → 재입력)
  - `.gsd/S18-SUMMARY.md`: T01/T02/T03 산출, 게이트 evidence, 정직한 한계
    (라이브 메인넷 자동 회귀 부재 / multi-key runtime 회전 별 슬라이스).
  - `.gsd/M006-SUMMARY.md` **신설** (M005-SUMMARY 패턴):
    - 출하 정의 (REQUIREMENTS#M006)
    - 응답·표면 — *운영자* 페르소나 완결 흐름 (env 키 설정 → 서버 부팅 → verify
      / examples / 프론트 모두 인증된 호출 → 401 단일 응답으로 회귀 차단)
    - 수용 기준 (M006 전체) 항목별 ✅
    - 슬라이스 (S16 / S17 / S18) 한 줄 요약 + SUMMARY 링크
    - 최종 게이트 재실행 (KNOWLEDGE S04 Rule)
    - 핵심 교훈 — 세 페르소나(dApp 개발자 / 봇 운영자 / 운영자) 완결, env 단일
      키의 운영 트레이드오프, info-leak 방지 단일 401의 사용 측 영향
    - 정직한 한계 — 무중단 회전 / multi-key / rate limiting / audit log 모두
      별 단위, *M006 출하 후 후속 마일스톤 또는 BACKLOG*
    - Reassess — M001~M006 ✅ SHIPPED 선언, 다음 호흡 후보 (S11.1 dApp 깊이 /
      Pools-Traders FE / 별 단위 hardening 등)
  - `.gsd/M001-ROADMAP.md`:
    - M006 섹션 헤더 → `✅ SHIPPED → M006-SUMMARY.md`
    - S18 → `[x] DONE → S18-SUMMARY.md`
  - `.gsd/BACKLOG.md`:
    - M006 진행 항목 → 완료 표 한 줄로 압축
    - 우선순위 표 재정렬 — M004 잔여 / FE / hardening / 새 마일스톤 후보
    - **별 단위 hardening 후보 추가** — toolchain 회귀 lint 2건 (decoder/events.rs,
      indexer/worker.rs) 정리 (S16에서 인라인 fix, 별 단위로 깔끔하게 갱신할
      후보)
  - 최종 게이트 재실행 (KNOWLEDGE S04 Rule):
    - `cargo fmt --check` (workspace) — 무회귀 자동 (서버 코드 0 변경)
    - `cargo clippy --workspace --all-targets -- -D warnings` — 무회귀 자동
    - `cargo test -p api / -p indexer / -p db --lib / --ignored / -p decoder` — 무회귀 자동
    - `bash scripts/verify-*` 3종 — 무회귀 자동 (스크립트/서버 0 변경)
    - `tsc --noEmit -p examples/typescript-client/tsconfig.json` — 무회귀 자동
    - `python3 -m py_compile examples/python-client/*.py` — 무회귀 자동
    - **`cd web && npm run typecheck && npm run test && npm run build`** — 신규
      컴포넌트/state + 단위테스트 + 빌드 확장. typecheck clean / test 신규 통과
      포함 / build 무회귀.
- *Reassess*: S18 ✅ DONE → **M006 ✅ SHIPPED**. 세 페르소나 완결. 다음 호흡은
  사용자 결정 — BACKLOG.md 우선순위 표 활용. M007 분기 또는 단독 슬라이스.
- *Artifacts*: `docs/cookbook.md`, `.gsd/{S18-SUMMARY,M006-SUMMARY,M001-ROADMAP,BACKLOG}.md`

## T04 — 최종 게이트 + 커밋 + PR

**Must-haves**
- 단일 호흡 게이트 재실행 (위 T03 게이트 목록)
- 변경 파일 staging — *서버 코드는 0 변경*. 변경 면적:
  - web: client.ts / Alerts.tsx / App.tsx + 신설 state/apiKey.tsx, components/ApiKeyInput.tsx, api/client.test.ts (또는 확장)
  - docs: cookbook.md
  - .gsd: S18-PLAN / S18-SUMMARY / M006-SUMMARY / M001-ROADMAP / BACKLOG (+ DECISIONS D024)
- 커밋 메시지: `S18 + M006 SHIPPED: frontend auth — /alerts API key UI (M006 마감)`
- PR 생성 `feat/s18-frontend-auth` → `main`

---

## Slice 수용 (Complete = S18 SHIPPED = M006 SHIPPED)
- [ ] T01–T03 must-haves, 기존 모든 표면 무회귀
- [ ] web typecheck + test (기존 + 신규) + build 모두 green
- [ ] cargo workspace + verify 3종 + tsc + py_compile 모두 무회귀 자동 (서버/스크립트/examples 코드 0 변경)
- [ ] REQUIREMENTS#M006 S18 항목 ✅ + S18-SUMMARY + M006-SUMMARY + ROADMAP M006 `[x] SHIPPED`
- [ ] BACKLOG.md M006 후속 항목 압축 + 우선순위 표 재정렬

## 정직한 한계 (S18 출하 시점)

- **multi-key runtime 회전 미부착** — 키 회전 = env 갱신 + 서버 재시작. 무중단
  회전(DB 키 테이블 + 활성 키 다중 보유)은 *별 슬라이스 / 별 마일스톤*.
- **빌드 타임 키 주입 미지원 (D024)** — `NEXT_PUBLIC_*` 등 빌드 시 환경 변수
  주입 가능하지만 *번들에 박혀* 소스맵·DevTools에 노출 → 운영 위험. 본 슬라이스는
  *런타임 입력*만.
- **세션 메모리 = 새로고침 시 사라짐 (D024 의도)** — 운영자 UX 트레이드오프:
  편의보다 안전. 잦은 새로고침이 부담이면 *비밀번호 매니저 자동 입력* 사용을
  cookbook에서 권고.
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터
  기반 (M004/M005/M006 일관 한계).
- **rate limiting / audit log 미부착** — write 라우트 brute-force 방어는 *별
  단위*. 401 응답이 attacker에게 key 정보 X (S16/D021).
