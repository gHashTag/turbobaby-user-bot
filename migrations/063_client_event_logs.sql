-- Loop #13: lightweight conversion/event telemetry sink.
-- Unlike client_error_logs, this table stores intentional frontend events
-- (e.g. cart_deep_link_opened) for funnel attribution, not crashes.
CREATE TABLE IF NOT EXISTS client_event_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event TEXT NOT NULL,
    detail TEXT,
    url_path TEXT,
    telegram_id BIGINT,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_client_event_logs_event
    ON client_event_logs(event, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_client_event_logs_telegram_id
    ON client_event_logs(telegram_id, occurred_at DESC);
