CREATE TABLE IF NOT EXISTS call_events (
    id UUID PRIMARY KEY,
    device_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    room_id TEXT,
    data JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_events_device ON call_events (device_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_type ON call_events (event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_room ON call_events (room_id, created_at DESC) WHERE room_id IS NOT NULL;
