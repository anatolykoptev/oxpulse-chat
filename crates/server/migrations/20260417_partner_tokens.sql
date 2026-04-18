-- Partner edge-node registration tokens.
--
-- One row per issued bootstrap token. A token becomes a registered node
-- when an edge-node posts to /api/partner/register with the matching raw
-- value — at that point used_at / used_from_ip / node_id are filled.
--
-- IP column uses TEXT rather than INET to avoid pulling the ipnetwork
-- feature into sqlx -- no query uses CIDR semantics. Migrate to INET
-- when audit tooling wants network queries.
CREATE TABLE IF NOT EXISTS partner_tokens (
    token_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id  TEXT NOT NULL,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    used_from_ip TEXT,
    node_id     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at  TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_partner_tokens_hash
    ON partner_tokens (token_hash)
    WHERE revoked_at IS NULL AND used_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_partner_tokens_partner
    ON partner_tokens (partner_id, created_at DESC);
