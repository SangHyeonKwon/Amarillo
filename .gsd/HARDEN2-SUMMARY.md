---
slice: HARDEN2
title: 알림 운영 위생 (URL 마스킹 + 시크릿 회전)
status: done
edge: weak-spot
milestone: cross-cutting (S08/HARDEN follow-up)
tasks: [T01, T02]
gate: pass             # fmt clean · clippy --workspace -D warnings 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 12/12 · verify-alerts ALL PASS
migrations: none
decision: none new
artifacts:
  - crates/indexer/src/alerts.rs              # redact_urls + dispatch_item 결선 + 6 단위테스트
  - crates/db/src/queries.rs                  # rotate_alert_subscription_secret
  - crates/db/tests/alerts.rs                 # alert_secret_rotation_happy_404_inactive
  - crates/api/src/routes/alerts.rs           # rotate_alert_subscription_secret 핸들러
  - crates/api/src/routes/mod.rs              # POST .../rotate-secret 라우트 등록
  - scripts/verify-alerts.sh                  # rotate happy + 999999999 + 비활성 → 404 추가
  - docs/api-alerts.md                        # rotate 엔드포인트 + L1 URL 마스킹 REALIZED
verification_constraint: "T01은 순수 단위 + dispatcher fail-path 결선. T02는 통합테스트(4 시나리오) + HTTP 수용으로 자동 가드. 라이브 webhook 전송은 여전히 docs 수동 스모크(D009~D012 일관)."
---

# HARDEN2 — 무엇이 실제로 일어났나

S08/HARDEN의 잔여 hygiene 항목 2개. 새 마이그레이션 0, 신규 의존성 0, 새 결정 0.

- **T01 (`last_error` URL 마스킹)** — 순수 `redact_urls(&str)`가 `http(s)://…` 부터
  첫 공백까지를 `<redacted-url>`로 치환. dispatcher 실패 경로에서 redact 먼저 →
  500자 캡 순서로 처리(잘림 직전에 URL 꼬리 노출되던 갭 차단). 단위 6 케이스
  (없음/단일/스킴 변형/다중/포트·경로 포함/false positive 방어). 성공 경로 무회귀.
  → 리뷰 L1 후속 REALIZED.

- **T02 (`signing_secret` 회전)** — 활성 구독에 한해 시크릿 회전:
  - DB: `rotate_alert_subscription_secret(pool, id, new_secret) -> Result<Option<AlertSubscription>>`.
    `WHERE active = TRUE` 필터 + `RETURNING` 명시 컬럼. 미존재/비활성 → `None`.
  - API: `POST /v1/alert-subscriptions/{id}/rotate-secret` — `getrandom` 32B
    hex → 회전 → 새 시크릿을 `AlertSubscriptionCreated`로 **1회 노출**(생성과
    동일 계약). 200(성공) / 404(미존재 또는 비활성). 비활성 구독에 회전을
    허용하지 않는 이유: 소프트-삭제된 구독을 재활성화시키지 않기 위해(운영 안전).
  - 통합 4 시나리오: happy/idempotent/missing/inactive. HTTP 수용 4 케이스:
    rotate 200 + 신규 시크릿이 초기와 다름 + 회전 후 GET 응답에 시크릿 미노출
    + nonexistent 404 + 비활성 후 rotate → 404.

**리뷰 매핑**
- HARDEN 리뷰 L1(URL 누설 후속) → T01 REALIZED
- HARDEN 백로그 *signing_secret 회전* → T02 REALIZED

**잔여(HARDEN 백로그)**
- DNS-time IP 재바인딩 SSRF — 별도 단독 PR 가치(custom DNS resolver).
- 임계/율 집계(D012 MVP 제외분) — 제품 슬라이스 수준, M003 분해 시점에 같이.

**Reassess**: ROADMAP "HARDEN 잔여" 항목에서 위 2건 제거. 남은 항목은 SSRF 잔여와
임계율, 그리고 큰 줄기로는 M003 출하(S09) + FE-WIRE. PR로 묶어 머지 대기.
