---
slice: S08
title: 실패 패턴 구독 + 웹훅 전송
status: done
edge: untapped
milestone: M003
tasks: [T01, T02, T03]
gate: pass            # fmt clean · clippy --workspace -D warnings 0 · cargo test -p indexer 22/22 · -p db --lib 9/9 (validators) · -p db --ignored 10/10 · verify-alerts.sh ALL PASS
migrations: 20240104000001_add_alert_tables.sql (idempotent)
decision: D012
artifacts:
  - migrations/20240104000001_add_alert_tables.sql                  # alert_subscription + alert_delivery
  - crates/db/src/{models.rs,queries.rs,validators.rs}              # 모델·매칭/멱등 쿼리·SSRF 가드
  - crates/db/tests/alerts.rs                                       # 매칭·anti-join·재시도·active 통합테스트
  - crates/indexer/src/alerts.rs                                    # HMAC 서명 + 디스패처 (얇은 드라이버)
  - crates/indexer/src/{config.rs,main.rs}                          # --dispatch-alerts 서브모드
  - crates/api/src/routes/alerts.rs                                 # POST/GET/DELETE /v1/alert-subscriptions
  - scripts/verify-alerts.sh                                        # HTTP 수용 + signing_secret 누설 검증
  - docs/api-alerts.md                                              # 엔드포인트·디스패처·보안 자세
verification_constraint: "라이브 webhook 전송은 수신 엔드포인트 필요(CI 미보장) → 순수 SSRF 9 + HMAC 3 + payload 1 단위 + 매칭 anti-join 통합(`-p db --ignored`)이 1차 증빙, 실제 POST는 수동 스모크."
---

# S08 — 무엇이 실제로 일어났나 (계획 대비)

S08-PLAN T01→T03 그대로. 마이그레이션 1개(`20240104000001_add_alert_tables.sql`).
**이 슬라이스는 M003 출하는 아니다 — M003 = S08 ∧ S09. S09는 sketch 유지.**

- **T01 (스키마/모델/매칭)**: 멱등 마이그레이션으로 `alert_subscription`
  (`error_category enum NULL=any`, `to_addr TEXT NULL=any`, `webhook_url`,
  `signing_secret`, `active`, `created_at`) + `alert_delivery`
  (PK=`(subscription_id, tx_hash)`=멱등 키, FK ON DELETE CASCADE) +
  `idx_alert_subscription_active`(partial index). 기존 `error_category` PG enum 재사용.
  쿼리: insert/list/deactivate/**delete**(hard, admin)/`find_pending_alert_matches`
  (anti-join on `alert_delivery WHERE status='delivered'`) /`record_alert_delivery`
  (멱등 upsert: 성공→`delivered_at=NOW()`, 실패→`attempts+=1, last_error`).
  통합테스트가 매칭·카테고리 필터·anti-join 멱등·재시도(`status='failed'`는
  제외 안 됨)·active 필터·teardown(cascade)을 전부 검증.
- **T02 (디스패처)**: 순수 SSRF 가드 (`webhook_url_is_safe` — T03에서 `db::validators`
  로 이동) + 순수 HMAC-SHA256 서명(`sign_payload`, RFC-4231 벡터 고정) +
  안정 본문 빌더(`build_payload`, 키 순서 고정으로 서명 결정성). 얇은 비동기
  드라이버 `indexer --dispatch-alerts`: 새 의존성 4개(`reqwest` rustls-only +
  `hmac` + `sha2` + `getrandom` T03), `Policy::none()` + 10초 타임아웃 + UA.
  `Ctrl-C` graceful. follow 루프엔 **인라인 금지**(D012). RPC 불요라
  Config::from_env 우회(DATABASE_URL만).
- **T03 (API + 검증/문서)**: `POST /v1/alert-subscriptions` (SSRF 가드+카테고리
  파싱+to_addr 정규화 → 모두 400; `webhook_url ≤ 2048`; `signing_secret`은 CSPRNG
  32바이트 hex, **응답 1회 노출**). `GET` (최신순, limit 1..=500, **signing_secret
  미직렬화**). `DELETE /{id}` (soft 비활성화, 미존재/이미 비활성 → 404).
  `verify-alerts.sh`로 6개 케이스 자동 검증(POST 201/SSRF×5·카테고리·to_addr×400/
  GET no-secret-leak/DELETE 204·재요청 404/nonexistent 404).
  **리팩토링**: SSRF 가드 + `UnsafeUrlReason`을 `crates/db/src/validators.rs`로
  이동 — indexer 디스패처와 api POST가 **동일한 규칙**으로 검증해야 보안 갭이
  생기지 않음(DRY). url 크레이트는 db에 직접 의존.

**리뷰 이월/실현**
- **D012 deviation (정직)**: 계획은 "신규 의존성 1개(reqwest)"였으나 실제 5개
  (`reqwest`+`hmac`+`sha2`+`url`+`getrandom`). HMAC-SHA256/SHA-256/CSPRNG/URL 파서를
  손수 구현하는 건 crypto·security anti-pattern이라 도입 1개로는 실현 불가. 모두
  작고 audited, 시스템 의존 없음. DECISIONS D012에 "REALIZED & DEVIATION" 기록.
- **KNOWLEDGE 추가 (S08-T01 통합테스트에서 실측)**:
  1. `trg_transaction_check_failed` 트리거가 status=0 tx INSERT 시 UNKNOWN
     failed_transaction을 자동 채우고 `ON CONFLICT DO NOTHING` → 호출자 카테고리가
     "선점"당함. 인덱서 실제 경로는 무해(호출자 INSERT가 먼저).
  2. 이전 KNOWLEDGE의 "FK 완화"는 부분적 — `failed_transaction.tx_hash → transaction`
     FK는 보존(error 23503으로 실측). 이벤트 쪽 FK만 완화돼 있었음. KNOWLEDGE 수정 기록.

**검증 한계 (정직)**: 라이브 webhook 전송은 수신 엔드포인트 필요(이 환경 미보장).
순수 단위(SSRF 9 + HMAC 3 + payload 1 + RFC-4231 벡터) + 매칭 anti-join 통합 +
`verify-alerts.sh` HTTP 수용이 1차 증빙. 실제 POST·재시도 사이클은 docs 수동
스모크 절차(`webhook.site` 등). 잔여 SSRF 리스크: DNS 시점 IP 재바인딩 — 명시 표기,
완전 해소(연결 시점 IP 검사)는 백로그.

**Reassess**: ROADMAP S08 `[x]`. **M003 미출하 — S09 분해 필요**. S09는
`[sketch]`로 남았고, "온체인×비공개 데이터 조인 예시 1건"의 소비 유스케이스 확정이
다음 Reassess에서 정제할 항목. M003 출하 = S08 ∧ S09.
