# FE-WIRE2 — 알림 구독 UI (S08/HARDEN2 결선) · PLAN

Slice 목표: FE-WIRE가 *실패 데이터*까지 클릭 가능하게 만든 위에, S08·HARDEN2가
출하한 *알림 구독 라이프사이클*까지 대시보드에서 사용 가능하게 한다. 백엔드는
이미 완성(create/list/delete/rotate 4개), 프론트엔드만 추가.

엣지: `[edge: weak-spot]` (출하한 API의 *클릭 가능 표면*). risk: low.
deps: FE-WIRE 머지(c50c48d). 새 결정 0, 마이그레이션 0, 백엔드 변경 0,
신규 의존성 0(`navigator.clipboard`는 표준 Web API).

**핵심 보안 UX**: 시크릿은 생성·회전 응답에서 **딱 1회만** 노출되고 이후 어디서도
조회 불가. 모달이 이 계약을 *눈에 보이게* 강제해야 한다 — 복사 버튼 + 경고문구 +
모달 닫으면 메모리에서 즉시 폐기 + URL/쿼리캐시에 절대 안 남기기.

검증 제약: `npm run typecheck` + `npm run test`(vitest) + `npm run build`로 자동.
실제 HTTP 흐름은 `verify-alerts.sh`가 백엔드 측에서 별도 가드. 시각 회귀(모달
열림·복사 버튼 등)는 로컬 dev 서버 수동 스모크.

태스크: T01 → T02.

---

## T01 — contract + hooks + client (POST/DELETE 능력)

**Must-haves**
- *Truths*
  - `client.ts`: `apiPost<T>(path, body, signal?, parser?)` + `apiDelete(path, signal?)`
    추가 (현재 `apiGet`만 있음). axum의 204 No Content + JSON 모두 처리. ApiError는
    기존 패턴 재사용.
  - `types.ts`: `AlertSubscription`(보안: secret 필드 *프론트 타입에는 미포함* — 백엔드의
    `#[serde(skip_serializing)]`과 정합), `AlertSubscriptionCreated`(POST·rotate 응답 — secret
    포함), `CreateAlertSubscriptionBody`(폼 입력).
  - `contract.ts`: 파서·envelope 5개 (parseAlertSubscription, parseAlertSubscriptionCreated,
    parseAlertSubscriptionListEnvelope, parseAlertSubscriptionCreatedEnvelope의 후자 2).
    secret이 응답에 없으면 throw(불변식).
  - `hooks.ts`: `useAlertSubscriptions(limit?)` query + `useCreateAlertSubscription`,
    `useRotateAlertSubscription`, `useDeactivateAlertSubscription` mutations.
    onSuccess에 query invalidate. 시크릿은 mutation `data`에만 잠시 머물고 cache 미저장.
  - `contract.test.ts`: 4 케이스 (list round-trip + secret-skip 가드 + created 정상 +
    malformed throw).
- *Artifacts*: `web/src/api/{client,types,contract,hooks}.ts` + `contract.test.ts`
- *Key Links*: 기존 `apiGet`/`parseTotalPaginatedResponse` 패턴, S08-T03 API 계약,
  HARDEN2-T02 회전 계약, `docs/api-alerts.md`의 수신자 검증 절(참고용)

## T02 — `/alerts` 페이지 (목록 + 생성 + 회전 + 비활성화)

**Must-haves**
- *Truths*
  - 신규 `web/src/pages/Alerts.tsx` + App router에 `/alerts` 라우트 등록 + nav link
    (기존 네비 구조 따름).
  - **구독 목록 카드**: `useAlertSubscriptions`. 활성·비활성 모두 보임, 비활성은 시각
    구분(opacity·라벨). 각 행에 Rotate / Deactivate 버튼 (비활성 행에선 Rotate 비활성).
    `webhook_url`은 *전체* 표시(시크릿 아님)지만 mono + truncate-with-tooltip.
  - **구독 생성 폼**: webhook_url (`https://…`) + category select(ALL=any/없음) +
    to_addr (`0x` + 40hex, optional). 클라이언트 사이드 검증(https 시작·to_addr 길이)
    *최소한*만(서버가 권위 — 400 → 사용자 친화 메시지). submit → mutation →
    **시크릿 모달 자동 오픈**.
  - **시크릿 1회 노출 모달** (보안 척추): 생성·회전 응답 수신 직후 모달 표시.
    내용: `subscription_id`, `signing_secret`(mono, 큼), 큼지막한 경고문구
    ("이 시크릿은 지금만 표시됩니다 — 즉시 복사하세요"), Copy(navigator.clipboard) +
    Close. 닫으면 React state에서 clear → 다시 못 봄. URL/쿼리캐시에 절대 안 남김
    (mutation 결과는 즉시 메모리에서 가비지 가능 형태로 변환). 다음 수신자 검증
    절차는 `docs/api-alerts.md` 링크 한 줄.
  - **Rotate**: 행 버튼 → 확인 prompt → POST `/rotate-secret` → 시크릿 모달 재사용
    (새 시크릿). list invalidate.
  - **Deactivate**: 행 버튼 → 확인 prompt → DELETE → list invalidate. 200/204 모두 처리.
  - **에러 표시**: 400(SSRF/카테고리/주소) → 응답 `error` 메시지 그대로 폼 위에 표시.
    404(미존재/이미 비활성) → 토스트 또는 인라인 표시.
- *Artifacts*: `web/src/pages/Alerts.tsx` (단일 페이지, 필요 시 컴포넌트 인라인),
  `web/src/App.tsx`(라우트 + nav), `App.smoke.test.tsx` 확장(/alerts 라우트 렌더).
  `web/src/lib/*`에 helpers 1-2개 추가 가능. `docs/`에는 별도 문서 없음 — README의
  대시보드 절에 한 줄 추가.
- *Note*: rotate-on-inactive(404), deactivate-on-already-inactive(404)는 백엔드 계약 —
  UI는 "구독을 못 찾았어요 / 이미 비활성입니다"로 통합 표시(404 의미 차이는 운영
  관점 비공개).

---

## Slice 수용 (Complete = FE-WIRE2 PR 머지)
- [ ] T01–T02 must-haves, 기존 페이지(Overview/FailedTx/Pools/Traders) 무회귀
- [ ] `cd web && npm run typecheck` clean · `npm run test` green(신규 contract +
      App smoke `/alerts` 렌더 포함) · `npm run build` 성공
- [ ] 시크릿은 모달에서만, 1회만 노출 — `webkit-textfill-color` 류 기교 없이 평문
      복사 가능 + 닫으면 즉시 폐기 (단위테스트로 강제하기 어렵지만 코드 리뷰 가드)
- [ ] 신규 의존성 0, 백엔드 변경 0, 마이그레이션 0
- [ ] `docker compose up -d` 환경에서 dev 서버로 1회 수동 스모크: 생성 → 시크릿 표시
      → 복사 → 모달 닫기 → 목록에서 secret 없음 확인 → 회전 → 새 시크릿 표시 →
      비활성화 → 비활성 행 표시 확인
