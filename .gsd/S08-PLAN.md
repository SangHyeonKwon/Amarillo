# S08 — 실패 패턴 구독 + 웹훅 전송 (M003 첫 슬라이스) · PLAN

Slice 목표: 봇 운영자가 실패 패턴(category/to_addr)을 구독하면, M002가 실시간
적재한 매칭 실패 tx에 대해 **HMAC 서명된 웹훅이 정확히 1회** 전송된다 — Dune이
구조적으로 못 하는 "실패 → 액션".

엣지: `[edge: untapped]`. risk: med. deps: M002(S05~S07 출하).
핵심 결정: **D012**(outbox 디스패처 / indexer 서브모드 / SSRF·HMAC / `reqwest`
1개 수용 / MVP 건별 정확매칭). 착수 시 DECISIONS 기록 완료.
검증 제약(D009~D011 동일): 라이브 전송은 수신 엔드포인트 필요(CI 미보장) → 순수
SSRF 가드·HMAC 서명·매칭 술어 단위 + 매칭 쿼리 통합(PG), 실제 POST는 수동 스모크.
태스크: T01 → T02 → T03.

---

## T01 — alert 스키마 + 모델 + 매칭 쿼리

**Must-haves**
- *Truths*
  - 멱등 마이그레이션으로 `alert_subscription`(id, error_category?, to_addr?,
    webhook_url, signing_secret, active, created_at)·`alert_delivery`
    (subscription_id, tx_hash, status, attempts, last_error?, delivered_at;
    PK(subscription_id,tx_hash)=멱등 키) 추가. `BEGIN/COMMIT`,`IF NOT EXISTS`
  - 매칭 쿼리: 활성 구독 × `failed_transaction`(+`transaction.to_addr` 조인)에서
    **아직 `alert_delivery`에 없는**(anti-join) 매칭쌍을 한정 반환. 파라미터화 SQL,
    `($1 IS NULL OR ...)` 옵션 필터 패턴 재사용. LIMIT로 상한
  - 기존 `/v1/*` 무회귀, FromRow `SELECT` 명시 컬럼(S04 L1)
- *Artifacts*: `migrations/<ts>_add_alert_tables.sql`, `crates/db/src/models.rs`
  (AlertSubscription/AlertDelivery + serde), `crates/db/src/queries.rs`
  (`insert_alert_subscription`/`list_alert_subscriptions`/`deactivate_*`/
  `find_pending_alert_matches`), `crates/db/tests/alerts.rs`(매칭·anti-join 멱등 통합)
- *Key Links*: S06 마이그레이션 멱등 패턴, `list_failed_transactions` 옵션필터,
  STH 통합테스트 하니스(`#[ignore]`, `-p db --ignored`)

## T02 — 디스패처 (순수 매칭/SSRF/서명 + 얇은 전송 드라이버)

**Must-haves**
- *Truths*
  - 순수·단위테스트: ① `webhook_url_is_safe` (https-only, loopback/RFC1918/
    link-local/메타데이터 169.254.169.254 거부) ② `sign_payload`(HMAC-SHA256 hex,
    안정 직렬화) ③ 매칭 술어(SQL 외 추가 판정 있으면)
  - 얇은 비동기 드라이버: `indexer --dispatch-alerts` 서브모드 — `find_pending_
    alert_matches` → 서명 POST(`reqwest`, 리다이렉트 비추적, 타임아웃) → 결과를
    `alert_delivery`에 **멱등 upsert**(성공/실패·attempts·last_error). 재시도는
    follow의 backoff 패턴 재사용. graceful ctrl_c. follow 루프엔 **인라인 금지**(D012)
  - **시크릿 안전**: signing_secret·webhook_url 본문은 로그 미출력(모드/카운트만).
    드라이버는 컴파일+clippy, 라이브는 수동 스모크
- *Artifacts*: `crates/indexer/src/{config.rs,main.rs}`(`--dispatch-alerts`),
  `crates/indexer/src/alerts.rs`(순수 가드/서명/매칭 + 드라이버), Cargo: `reqwest`
  (rustls, 최소 피처) 추가 — D012 근거 주석
- *Decision*: D012 (기록 완료) — outbox/서브모드/SSRF·HMAC/`reqwest`/건별 MVP

## T03 — 구독 관리 API + 검증/문서 + S09 Reassess

**Must-haves**
- *Truths*
  - `POST /v1/alert-subscriptions`(생성; `webhook_url`을 **순수 SSRF 가드**로
    검증 → unsafe면 400 `{error}`), `GET /v1/alert-subscriptions`(목록),
    `DELETE /v1/alert-subscriptions/{id}`(비활성화). 기존 `ApiResponse`/에러
    매핑·봉투 재사용(변형 금지, 가산만). signing_secret은 생성 응답 1회만 노출
  - `scripts/verify-alerts.sh`(생성→unsafe 400→목록→삭제, 의미 단언),
    `docs/api-alerts.md` + realtime-follow에 디스패처 절
  - REQUIREMENTS#M003 S08 수용 항목별 ✅ + 전체 게이트 green
- *Artifacts*: `crates/api/src/routes/alerts.rs`+`routes/mod.rs`, `response.rs`
  재사용, `scripts/verify-alerts.sh`, `docs/api-alerts.md`, `.gsd/S08-SUMMARY.md`,
  ROADMAP S08 `[x]`
- *Reassess*: S08 출하 후에만 **S09(`[sketch]`) 분해**(온체인×비공개 조인 예시,
  소비 유스케이스 확정). M003 출하 = S08 ∧ S09 — S08만으론 M003 미출하(정직).

---

## Slice 수용 (Complete = S08 출하, M003은 아직)
- [ ] T01–T03 must-haves, 기존 `/v1/*`·follow 무회귀
- [ ] `cargo test -p db --ignored`(매칭/멱등 포함) green · `-p indexer`(순수 가드/
      서명) green · clippy `--workspace -D warnings` 0 · fmt clean
- [ ] REQUIREMENTS#M003 **S08** 수용 기준 전체 ✅, S08-SUMMARY, ROADMAP S08 마감
- [ ] SSRF 가드·HMAC 서명 단위 + 매칭 anti-join 통합으로 1차 증빙, 라이브 전송은
      수동 스모크 절차 문서화(수신 엔드포인트 부재 시)
- [ ] M003 미출하 명시(S09 잔존) — Reassess에서 S09 분해 착수
