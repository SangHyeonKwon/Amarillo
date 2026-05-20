---
slice: FE-WIRE2
title: 알림 구독 UI (S08/HARDEN2 결선)
status: done
edge: weak-spot
milestone: M001/M003 후속 (alerting frontend consumption)
tasks: [T01, T02]
gate: pass             # typecheck clean · vitest 15/15 · build OK
migrations: none
decision: none new
artifacts:
  - web/src/api/client.ts                # apiPost / apiDelete 추가 (204 처리 + body=undefined 분기)
  - web/src/api/types.ts                 # AlertSubscription(secret 미포함) / AlertSubscriptionCreated(secret 포함) / CreateAlertSubscriptionBody
  - web/src/api/contract.ts              # parseAlertSubscription/Created + envelope 2개 + readBoolean
  - web/src/api/hooks.ts                 # useAlertSubscriptions / useCreate / useRotate / useDeactivate (mutations + invalidate)
  - web/src/api/contract.test.ts         # 신규 4 케이스 (list secret-skip + created reveal + missing secret throw + non-boolean throw)
  - web/src/pages/Alerts.tsx             # 새 페이지 — 목록·생성·회전·비활성 + 시크릿 1회 모달
  - web/src/App.tsx                      # /alerts 라우트 등록
  - web/src/components/Layout.tsx        # nav 링크 추가
  - web/src/App.smoke.test.tsx           # /alerts 라우트 렌더 케이스 추가
verification_constraint: "시크릿 1회 노출 contract는 단위(throw on missing) + 모달 자동 reset()으로 정적 가드. 실제 클립보드·HTTP 흐름은 docker postgres+api 대상 로컬 dev 서버 수동 스모크."
---

# FE-WIRE2 — 무엇이 실제로 일어났나

FE-WIRE가 *실패 데이터*까지 클릭 가능하게 한 위에, **알림 구독 라이프사이클**
(create/list/rotate/deactivate)을 대시보드에서 사용 가능하게 한다. 백엔드 무변경,
마이그레이션 0, 새 결정 0, 신규 의존성 0.

- **T01 (contract + hooks + client POST/DELETE)** — `client.ts`가 apiGet만
  지원했음 → `apiPost`(body 옵셔널: `/rotate-secret`는 body 없이 호출, 일반 생성은
  JSON body) + `apiDelete`(204 No Content 정상 처리) 추가. types.ts에 두 시크릿
  계약을 *타입 단계*에서 분리: `AlertSubscription`은 `signing_secret` 필드 **자체가
  없음**(백엔드 `#[serde(skip_serializing)]`와 정합), `AlertSubscriptionCreated`만
  보유 (POST + rotate 응답). contract.ts에 파서 + envelope 2개 + 신규 `readBoolean`
  헬퍼. hooks.ts에 1 query + 3 mutations — mutations는 모두 onSuccess에 list
  invalidate. 단위테스트 4 케이스: list가 secret을 절대 안 가짐 (`"signing_secret"
  in row === false`), created에서 secret 미포함이면 throw, active 비-boolean이면 throw.

- **T02 (`/alerts` 페이지)** — 새 페이지 + 라우트 + nav 링크 + smoke 케이스.
  3 섹션 + 모달:
  - **Create subscription 폼**: `webhook_url` / category select(ALL/none + 6개) /
    optional `to_addr` (`0x` + 40hex). 최소 클라이언트 검증(https://·hex 길이),
    서버 400 메시지는 그대로 폼 위 alert에 표시.
  - **Subscriptions 목록**: ID / Status / Category / To address / Webhook(mid-trunc) /
    Created / Actions (Rotate / Deactivate). 비활성 행은 회색 badge, 액션 disabled.
  - **시크릿 1회 모달 (보안 척추)**: create 또는 rotate 성공 즉시 자동 오픈. 큼지막한
    `<code className="mono">`로 64-hex 표시 + 경고 문구 + `navigator.clipboard` Copy +
    Close. ESC·backdrop 클릭 모두 닫힘. 닫으면 `setRevealed(null)` + `create.reset()` +
    `rotate.reset()`로 **mutation 캐시까지 폐기** → DevTools/메모리 어디에도 안 남음.

**보안 UX 구현 디테일**
- 시크릿은 React state `revealed`에만 살고 모달 닫을 때 폐기. URL/쿼리 캐시/
  localStorage 어디에도 안 저장.
- `mutation.reset()` 호출로 react-query의 mutation observer cache까지 정리 — 다른
  컴포넌트에서 mutation 결과 재조회로도 시크릿 못 얻음.
- list 모델 자체에 secret 필드 *없음* → 어떤 페이지든 list 행에서 secret 출력
  자체가 컴파일 불가.
- Copy 실패 시(예: jsdom·구형 브라우저) "Copy failed (select & copy manually)"
  메시지 + 평문 노출은 유지(사용자가 수동 선택 가능).

**리뷰 매핑**
- FE-WIRE의 *intentional exclusion*(알림 구독 UI) → 본 슬라이스가 닫음.
- S08/HARDEN2의 API 표면이 *클릭 가능한 표면*으로 완성.

**의도적 제외 (잔여 백로그)**
- 알림 *수신 검증* (수신자 측 HMAC 재계산 도구) — receiver 책임 영역. docs/api-alerts.md
  에 Node/Python 예제만 제공.
- Pools/Traders 신규 API 매핑 — 우선순위 낮음.

**잔여 한계 (정직)**
- 실제 webhook 발사·복사 흐름은 docker+api 대상 dev 서버 수동 클릭으로만 검증.
- `window.confirm`/`window.alert` 사용 — 더 좋은 UX는 in-app 토스트지만 dependency
  무도입 우선. 의도적 trade-off.
- modal focus trap 미구현 — ESC + backdrop 클릭만 제공(MVP).

**Reassess**: ROADMAP 백로그에서 *알림 UI(FE-WIRE2)* 제거. 남은 백로그:
DNS-rebinding SSRF / 임계율 집계 / S09(M003 출하 게이트, 유스케이스 미확정).
