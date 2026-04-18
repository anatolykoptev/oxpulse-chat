-- Partner edge-node registry.
--
-- One row per registered clone. Populated when a partner edge-node posts to
-- /api/partner/register with a valid bootstrap token; updated on each
-- subsequent heartbeat (last_seen_at).
--
-- public_ip uses TEXT rather than INET to match the project convention
-- established in 20260417_partner_tokens.sql -- sqlx ipnetwork feature is
-- not enabled and no query uses CIDR semantics. Migrate to INET when
-- network-range queries are needed.
CREATE TABLE IF NOT EXISTS partner_nodes (
    node_id          TEXT PRIMARY KEY,
    partner_id       TEXT NOT NULL,
    domain           TEXT NOT NULL,
    turns_subdomain  TEXT NOT NULL,
    public_ip        TEXT NOT NULL,
    registered_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (partner_id, domain)
);

CREATE INDEX IF NOT EXISTS idx_partner_nodes_partner
    ON partner_nodes (partner_id);

CREATE INDEX IF NOT EXISTS idx_partner_nodes_last_seen
    ON partner_nodes (last_seen_at);
