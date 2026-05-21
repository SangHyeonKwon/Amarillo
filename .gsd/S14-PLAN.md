# S14 — 임계율 집계 알림 (M005 첫 슬라이스) · PLAN

Slice 목표: REQUIREMENTS#M005의 1차 가산 — 기존 `alert_subscription` (S08)에
`sub_type` + 임계율 메타 컬럼을 가산해 *rate-threshold* 모드를 지원. 디스패처
는 sub_type 분기로 per-event(기존) / rate-threshold(신규) 처리. 봇 운영자가
*급증 패턴*만 알림 받게 — 건별 노이즈 차단.

엣지: `[edge: untapped]`. risk: med. deps: M001~M004 + S08(alert_subscription 기반).
**M005 첫 슬라이스** — 봇 운영자 페르소나 첫 진입.

핵심 결정: **D018** (착수 시 기록) — 컬럼 가산(`sub_type` 명시 default
`'per_event'` = backwards compat), 디바운스 *시간 기반*만, 비율/추세는 별 슬라이스.

검증 제약(D009~D017 일관): T01 통합 PG, T02 verify HTTP + 통합테스트, T03 web
typecheck/test/build. 라이브 임계율 시뮬은 시드 데이터 시간 분포 기반.

태스크: T01 → T02 → T03.

---

## T01 — 마이그레이션 + 모델 + 매칭/디바운스 쿼리 + 통합테스트 + D018

**Must-haves**
- *Truths*
  - 멱등 마이그레이션 `migrations/20240108000001_add_alert_subscription_rate.sql`:
    ```sql
    BEGIN;

    -- 명시 sub_type — silent default 금지(D014 일관)
    ALTER TABLE alert_subscription
      ADD COLUMN IF NOT EXISTS sub_type TEXT NOT NULL DEFAULT 'per_event'
        CHECK (sub_type IN ('per_event', 'rate_threshold')),
      ADD COLUMN IF NOT EXISTS threshold_count INT,
      ADD COLUMN IF NOT EXISTS threshold_window_secs INT,
      ADD COLUMN IF NOT EXISTS debounce_secs INT;

    -- rate_threshold 모드면 3 컬럼 모두 NOT NULL 필수
    ALTER TABLE alert_subscription
      ADD CONSTRAINT alert_subscription_rate_fields_chk
      CHECK (
        (sub_type = 'per_event'
          AND threshold_count IS NULL
          AND threshold_window_secs IS NULL
          AND debounce_secs IS NULL)
        OR
        (sub_type = 'rate_threshold'
          AND threshold_count IS NOT NULL AND threshold_count > 0
          AND threshold_window_secs IS NOT NULL AND threshold_window_secs > 0
          AND debounce_secs IS NOT NULL AND debounce_secs >= 0)
      );

    COMMENT ON COLUMN alert_subscription.sub_type IS
      'per_event(기존 1매칭=1웹훅) | rate_threshold(윈도우 count>=threshold+디바운스)';
    -- 기타 컬럼 COMMENT

    COMMIT;
    ```
    멱등: `ADD COLUMN IF NOT EXISTS` + CHECK CONSTRAINT는 `IF NOT EXISTS` 미지원 →
    별 마이그레이션 또는 conditional add (PostgreSQL 도구 도움). 단순화: 처음 적용
    시점 가정 + 후속 마이그레이션 시 `DROP CONSTRAINT IF EXISTS` + `ADD`로 idempotent.

  - 모델 `AlertSubscription` 확장:
    ```rust
    pub struct AlertSubscription {
        // ... 기존
        pub sub_type: String,          // "per_event" | "rate_threshold"
        pub threshold_count: Option<i32>,
        pub threshold_window_secs: Option<i32>,
        pub debounce_secs: Option<i32>,
    }
    ```
    `AlertSubscriptionCreated`도 동일 가산.

  - 새 쿼리:
    - `insert_alert_subscription_rate(pool, error_category?, to_addr?, webhook_url,
      signing_secret, threshold_count, threshold_window_secs, debounce_secs)` —
      sub_type='rate_threshold'로 INSERT. 기존 `insert_alert_subscription`은
      per_event 명시 INSERT로 유지(or 통합 — 둘 다 OK, 단순 분리).
    - `find_pending_rate_alert_matches(pool, limit)` — rate sub × 시간 윈도우 내
      failed_tx count 매칭 + last alert sent 이후 debounce_secs 경과 검증.
      예시 (단순화):
      ```sql
      SELECT s.subscription_id, s.webhook_url, s.signing_secret,
             COUNT(f.tx_hash) AS match_count
      FROM alert_subscription s
      JOIN failed_transaction f
        ON (s.error_category IS NULL OR s.error_category = f.error_category)
      LEFT JOIN transaction t ON t.tx_hash = f.tx_hash
      WHERE s.active AND s.sub_type = 'rate_threshold'
        AND (s.to_addr IS NULL OR s.to_addr = t.to_addr)
        AND f.timestamp >= NOW() - (s.threshold_window_secs * INTERVAL '1 second')
        AND NOT EXISTS (
          SELECT 1 FROM alert_delivery d
          WHERE d.subscription_id = s.subscription_id
            AND d.status = 'delivered'
            AND d.delivered_at > NOW() - (s.debounce_secs * INTERVAL '1 second')
        )
      GROUP BY s.subscription_id, s.webhook_url, s.signing_secret, s.threshold_count
      HAVING COUNT(f.tx_hash) >= s.threshold_count
      LIMIT $1
      ```
      반환 행 = "이 sub은 지금 발송 자격 있음 (디바운스 안 걸림)".
    - rate 알림 발송 후 `record_alert_delivery`로 *summary tx_hash*(예: 가장
      최근 tx_hash 또는 별 sentinel)를 기록 — 디바운스 검증에 쓰임. 또는
      별 테이블 `alert_dispatch_log`로 분리(더 깔끔). 단순화: 기존 `alert_delivery`
      재사용 + tx_hash 컬럼에 sentinel(`rate:N` 형식).

      **결정**: 별 테이블 `alert_rate_dispatch(subscription_id, dispatched_at,
      match_count, summary)` 추가가 더 안전 — 기존 outbox claim 패턴과 격리.
      마이그레이션에 함께 포함.

  - 통합테스트 `crates/db/tests/alert_rate.rs`:
    - rate sub 생성 + 시드 (실패 tx N건 시뮬) → `find_pending_rate_alert_matches`가
      threshold 초과 시 매칭 반환
    - debounce 적용 후 같은 sub은 다시 매칭 안 됨 (이전 발송 기록 있음)
    - debounce 만료 후 다시 매칭됨
  - **D018 기록** — DECISIONS에 결정 + 트레이드오프 + 검증 제약.
  - prod `unwrap()` 0 / 파라미터화 SQL 100% / `///` doc.

- *Artifacts*: `migrations/20240108000001_add_alert_subscription_rate.sql`,
  `crates/db/src/{models,queries}.rs`, `crates/db/tests/alert_rate.rs`,
  `.gsd/DECISIONS.md`
- *Key Links*: S08 alert 인프라(매칭/claim/HMAC), S09 마이그레이션 멱등 패턴,
  STH 통합테스트 하니스

## T02 — 디스패처 룰 + API 표면 + verify + docs

**Must-haves**
- *Truths*
  - 디스패처(`crates/indexer/src/alerts.rs`) 확장:
    - 기존 dispatch_loop: per-event 매칭(기존) + rate 매칭(신규) 분리. 각각
      별 batch fetch:
      - `find_pending_alert_matches`(per-event, 기존) — sub_type='per_event' 필터 추가
      - `find_pending_rate_alert_matches`(rate, 신규)
    - rate 매칭은 batch마다 sub_id 단위 1회 발송 + `alert_rate_dispatch` INSERT.
      claim 패턴 불필요 (디바운스 자체가 race-safe — 동시 두 worker가 match
      해도 둘 다 INSERT but 디바운스가 검증, 1초 차이라도 두 번째는 다음
      매칭 시점에 디바운스에 막힘).
    - 발송 payload는 기존 per-event payload와 호환: `{ subscription_id, sub_type:
      "rate_threshold", match_count, window_secs, threshold, since }` 같은 형식.
      기존 receiver(per-event)는 `tx_hash` 기준이므로 sub_type 분기 명시.
  - API:
    - `POST /v1/alert-subscriptions` body: `sub_type?`(default per_event) +
      rate fields(rate_threshold면 필수). 잘못된 조합(예: per_event인데 threshold
      입력) → 400.
    - `GET /v1/alert-subscriptions` 응답: sub_type + rate fields 표시(null이면
      per-event).
    - `POST /v1/alert-subscriptions/{id}/rotate-secret`: sub_type 변경 없음 (기존
      유지).
  - `scripts/verify-alerts.sh`에 rate sub 시나리오 추가:
    - rate sub 생성 → 응답에 sub_type/threshold 필드 단언
    - 잘못된 조합(예: per_event인데 threshold 입력) → 400 단언
    - (선택) docker compose 시드의 실패 tx 분포로 임계 충족하는 케이스 자동 발송
      시뮬 → 발송 기록 단언. 시뮬 어려우면 수동 스모크 절차 명시.
  - `docs/api-alerts.md`에 rate 모드 절 추가: 페이로드 구조 + 디바운스 동작 +
    per-event와의 차이 한 단락.
- *Artifacts*: `crates/indexer/src/alerts.rs`, `crates/api/src/routes/alerts.rs`,
  `crates/api/src/...`, `scripts/verify-alerts.sh`, `docs/api-alerts.md`
- *Key Links*: S08-PLAN (dispatcher 패턴), HARDEN-T02 (outbox claim),
  HARDEN-T03 (bounded parallelism)

## T03 — 프론트 /alerts 페이지 확장 + S14-SUMMARY

**Must-haves**
- *Truths*
  - `web/src/api/types.ts`: `AlertSubscription` + `AlertSubscriptionCreated`에
    sub_type + rate fields 가산.
  - `web/src/api/contract.ts`: parser 갱신 (sub_type/rate fields parsing).
  - `web/src/api/contract.test.ts`: 신규 케이스 — per_event(rate fields null) /
    rate_threshold(fields 채움) / 잘못된 조합 throw.
  - `/alerts` 페이지 폼 확장:
    - sub_type 라디오 (Per event / Rate threshold)
    - rate threshold 선택 시 threshold_count / window_secs / debounce_secs 입력 필드 표시
    - 잘못된 입력 (e.g. threshold=0) 클라이언트 검증
  - 구독 목록 표시:
    - sub_type 배지 (per-event는 회색, rate-threshold는 강조)
    - rate sub은 `count/window/debounce` 한 줄 표시
  - `.gsd/S14-SUMMARY.md` + ROADMAP S14 `[x]`. M005는 `🚧 IN PROGRESS` 유지
    (S15/S16 남음).
- *Reassess*: S14 출하 후 — S15(봇 라벨) / S16(cookbook 봇 시나리오) / M004 잔여
  (S11.1/S12.1/S13.1) 중 사용자 결정.
- *Artifacts*: `web/src/api/{types,contract}.ts`, `web/src/api/contract.test.ts`,
  `web/src/pages/Alerts.tsx`, `.gsd/{S14-SUMMARY,M001-ROADMAP}.md`

---

## Slice 수용 (Complete)
- [ ] T01–T03 must-haves, 기존 `/v1/*`·`/alerts`·페이지 무회귀
- [ ] DB 통합(`-p db --ignored`) + indexer + db lib + clippy --workspace + fmt 모두 green
- [ ] `verify-alerts.sh` ALL PASS (rate sub 신규 단언 포함),
      `verify-failed-tx.sh` + `verify-failed-tx-by-label.sh` 무회귀
- [ ] `web` typecheck + test(rate 신규 케이스) + build 통과
- [ ] REQUIREMENTS#M005 S14 항목 ✅ + S14-SUMMARY + ROADMAP S14 `[x]`
- [ ] M005는 *진행 중* 유지 — S15/S16 분해는 다음 지시에서만
