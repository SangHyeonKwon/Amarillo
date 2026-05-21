-- S14 (M005): alert_subscription에 rate_threshold 모드 가산.
-- 봇 운영자 페르소나 첫 진입 — 건별 알림(per_event)은 노이즈, 임계율 알림은 시그널.
-- 기존 행은 default sub_type='per_event'로 완전 호환(silent default 금지 정신 일관, D018).
-- 디바운스는 시간 기반: 발송 후 debounce_secs 동안 같은 sub 무시.

BEGIN;

-- 1. sub_type — 명시 default (silent default 금지, 기존 행 자동 호환)
ALTER TABLE alert_subscription
  ADD COLUMN IF NOT EXISTS sub_type TEXT NOT NULL DEFAULT 'per_event'
    CHECK (sub_type IN ('per_event', 'rate_threshold'));

-- 2. rate 메타 컬럼 (per_event면 NULL)
ALTER TABLE alert_subscription
  ADD COLUMN IF NOT EXISTS threshold_count INT,
  ADD COLUMN IF NOT EXISTS threshold_window_secs INT,
  ADD COLUMN IF NOT EXISTS debounce_secs INT;

-- 3. rate_threshold 모드 일관성 CHECK (멱등: DROP + ADD, IF NOT EXISTS 없음)
ALTER TABLE alert_subscription DROP CONSTRAINT IF EXISTS alert_subscription_rate_fields_chk;
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
  'per_event(기존 1매칭=1웹훅) | rate_threshold(윈도우 count>=threshold + 디바운스, S14/M005)';
COMMENT ON COLUMN alert_subscription.threshold_count IS
  'rate_threshold: 윈도우 내 매칭 카운트가 이 값 이상이면 발송 (per_event=NULL)';
COMMENT ON COLUMN alert_subscription.threshold_window_secs IS
  'rate_threshold: 카운트 윈도우 (초)';
COMMENT ON COLUMN alert_subscription.debounce_secs IS
  'rate_threshold: 발송 후 이만큼 시간 동안 같은 sub 무시';

-- 4. alert_rate_dispatch — rate 발송 기록 (디바운스 검증; per-event는 alert_delivery 사용)
CREATE TABLE IF NOT EXISTS alert_rate_dispatch (
  dispatch_id     BIGSERIAL PRIMARY KEY,
  subscription_id BIGINT NOT NULL REFERENCES alert_subscription(subscription_id) ON DELETE CASCADE,
  dispatched_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  match_count     INT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('delivered', 'failed')),
  last_error      TEXT
);

CREATE INDEX IF NOT EXISTS idx_alert_rate_dispatch_sub_time
  ON alert_rate_dispatch (subscription_id, dispatched_at DESC);

COMMENT ON TABLE alert_rate_dispatch IS
  'rate_threshold sub 발송 기록 — 디바운스 검증(마지막 발송 + debounce_secs)에 쓰임 (S14/M005).';
COMMENT ON COLUMN alert_rate_dispatch.match_count IS
  '발송 시점 시간 윈도우 내 매칭된 실패 tx 수';
COMMENT ON COLUMN alert_rate_dispatch.status IS
  'delivered | failed (디바운스는 status 무관 — 마지막 발송 시도 시각이 디바운스 시점)';

COMMIT;
