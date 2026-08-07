-- Loop #21: persisted outbound Telegram notification queue.
-- Replaces fire-and-forget direct sends for referral lifecycle events,
-- giving idempotency, retry, and observability.
CREATE TABLE IF NOT EXISTS notification_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    telegram_id BIGINT NOT NULL,
    kind VARCHAR(50) NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    scheduled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    attempts INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notification_queue_pending
    ON notification_queue (scheduled_at, attempts)
    WHERE processed_at IS NULL;
