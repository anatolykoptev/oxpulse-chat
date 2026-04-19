-- Add visitor_id column to call_events.
--
-- Feeds from the `ox_vid` HttpOnly cookie set by the visitor middleware.
-- Companion to the existing `device_id` which comes from SPA localStorage:
--   device_id  = stable within a single browser install until user clears
--                web-storage; unique per tab-chain.
--   visitor_id = stable within a browser profile for up to 400 days
--                (cookie max-age capped by RFC 6265bis); survives
--                localStorage clears, dies on cookie clears.
--
-- Having both lets us correlate sessions where only one of the two
-- survives a privacy reset. NULL on pre-migration rows and on events
-- from clients that rejected the cookie.

ALTER TABLE call_events
    ADD COLUMN IF NOT EXISTS visitor_id TEXT;

CREATE INDEX IF NOT EXISTS idx_events_visitor
    ON call_events (visitor_id, created_at DESC)
    WHERE visitor_id IS NOT NULL;
