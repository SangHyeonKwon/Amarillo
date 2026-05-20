-- Actionable Alerts (M003/S08): failure-pattern subscriptions + delivery log.
-- A subscription matches failed txs by optional error_category and/or to_addr
-- (NULL = "any"). alert_delivery is the idempotency ledger: at most one row
-- per (subscription, tx); the dispatcher upserts it so a webhook is sent
-- exactly once. Idempotent migration (IF NOT EXISTS); reuses the existing
-- `error_category` enum type.

BEGIN;

CREATE TABLE IF NOT EXISTS alert_subscription (
    subscription_id BIGSERIAL PRIMARY KEY,
    error_category  error_category,
    to_addr         TEXT,
    webhook_url     TEXT        NOT NULL,
    signing_secret  TEXT        NOT NULL,
    active          BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE  alert_subscription IS 'Failure-pattern alert subscriptions (S08)';
COMMENT ON COLUMN alert_subscription.error_category IS 'Match this category; NULL = any';
COMMENT ON COLUMN alert_subscription.to_addr        IS 'Match this lowercased contract address; NULL = any';
COMMENT ON COLUMN alert_subscription.signing_secret IS 'Per-subscription HMAC-SHA256 key — never logged/served after creation';

CREATE TABLE IF NOT EXISTS alert_delivery (
    subscription_id BIGINT      NOT NULL,
    tx_hash         TEXT        NOT NULL,
    status          TEXT        NOT NULL,
    attempts        INT         NOT NULL DEFAULT 0,
    last_error      TEXT,
    delivered_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT alert_delivery_pkey PRIMARY KEY (subscription_id, tx_hash),
    CONSTRAINT alert_delivery_subscription_id_fkey
        FOREIGN KEY (subscription_id)
        REFERENCES alert_subscription (subscription_id)
        ON DELETE CASCADE
);

COMMENT ON TABLE  alert_delivery IS 'Webhook delivery ledger — idempotency key (subscription_id, tx_hash) (S08)';
COMMENT ON COLUMN alert_delivery.status     IS 'delivered | failed';
COMMENT ON COLUMN alert_delivery.attempts   IS 'Delivery attempt count';
COMMENT ON COLUMN alert_delivery.last_error IS 'Last delivery error (NULL once delivered)';

CREATE INDEX IF NOT EXISTS idx_alert_subscription_active
    ON alert_subscription (active)
    WHERE active;

COMMIT;
