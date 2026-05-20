# HARDEN2 — 알림 운영 위생 (HARDEN slice 후속) · PLAN

Slice 목표: HARDEN slice가 마감한 운영 안전(R3·R4·M1·M2·L3) 위에 남은 *작은*
운영 위생 2가지 — `alert_delivery.last_error`의 URL 누설 방지 + per-구독
시크릿 회전 API. M003 출하나 제품 기능이 아니라 *알림 라이프사이클의 후속
hygiene*.

엣지: `[edge: weak-spot]` (이미 잡힌 갭의 후속 정리). risk: low. deps: HARDEN
완료. 새 마이그레이션 0, 신규 의존성 0, 새 결정 0.
검증 제약: T01 순수 단위 + dispatcher 통합. T02는 verify-alerts.sh HTTP 수용
+ DB 통합으로 자동 가드. 라이브 전송은 여전히 환경 의존.
태스크: T01 → T02.

---

## T01 — `last_error` URL 마스킹

**Must-haves**
- *Truths*
  - 순수 `redact_urls(&str) -> String`: `http://`/`https://` 부터 첫 공백까지를
    `<redacted-url>`로 치환. 여러 URL 동시 처리. 단위테스트(없음/한 개/여러 개/
    스킴 변형/공백 경계).
  - `dispatch_item` 실패 경로에서 `e.to_string()` → redact → 500자 캡 → DB 기록.
    순서: redact 먼저(잘림 전), 그 다음 캡. 무회귀(성공 경로엔 영향 없음).
- *Artifacts*: `crates/indexer/src/alerts.rs`(`redact_urls` + 결선 + 단위테스트)
- *Key Links*: 리뷰 L1 follow-up (HARDEN-T03이 500자 캡으로 처리, 본 항목이
  URL 자체 마스킹 완료)

## T02 — `signing_secret` 회전 API

**Must-haves**
- *Truths*
  - 새 쿼리 `rotate_alert_subscription_secret(pool, id, new_secret) -> Result<Option<AlertSubscription>>`:
    `active = TRUE` 인 행만 UPDATE … RETURNING 명시 컬럼. 미존재/비활성 → None.
    멱등(같은 시크릿 재호출도 동일 결과).
  - 새 API 라우트 `POST /v1/alert-subscriptions/{id}/rotate-secret`:
    `getrandom` 32B → hex → 회전 → `AlertSubscriptionCreated` 응답으로 **새
    시크릿 1회 노출** (생성 시 계약 동일). 미존재/비활성 → 404. 기존 봉투 재사용.
  - `verify-alerts.sh` 확장: rotate happy path(200 + 새 secret) + 404(미존재)
    + 회전 후 GET 응답에 secret 노출 없음.
- *Artifacts*: `crates/db/src/queries.rs`(쿼리), `crates/db/tests/alerts.rs`
  (통합 1: 회전·미존재·active=false 패턴), `crates/api/src/routes/alerts.rs`
  (라우트), `crates/api/src/routes/mod.rs`(등록), `scripts/verify-alerts.sh`,
  `docs/api-alerts.md`(rotate 엔드포인트 + 수신자 회전 절차)
- *Note*: `rotated_at` 컬럼 추가는 스코프 밖 — 마이그레이션이면 다른 단위.
  현재 회전 추적은 `created_at` 갱신 안 하고 시크릿만 바꿔(audit 흔적은 운영
  로그에 위임).

---

## Slice 수용 (Complete = HARDEN2 PR 머지)
- [ ] T01–T02 must-haves, 기존 `/v1/*` 무회귀
- [ ] `cargo test -p indexer` (redact_urls 단위) green · `-p db --ignored`
      (rotate 통합 포함) green · clippy --workspace -D warnings 0 · fmt clean
- [ ] `verify-alerts.sh` 확장된 ALL PASS (rotate 케이스 추가)
- [ ] 신규 의존성 0, 마이그레이션 0
- [ ] docs/api-alerts.md에 rotate 절 + L1 후속 메모 ("URL 마스킹 REALIZED")
