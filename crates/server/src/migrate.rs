use sqlx::PgPool;

/// Ordered list of embedded migrations.
///
/// Each entry is `(name, sql)`. The name is used as the idempotency key in
/// `schema_migrations`. SQL is split on `;` for execution — this is fine for
/// DDL-only files; switch to a proper tokenizer (or `sqlx migrate`) when a
/// migration includes PL/pgSQL blocks that contain semicolons.
/// TODO: replace `;`-split with sqlx migrate when a 3rd migration lands.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_analytics.sql",
        include_str!("../migrations/001_analytics.sql"),
    ),
    (
        "20260417_partner_tokens.sql",
        include_str!("../migrations/20260417_partner_tokens.sql"),
    ),
];

pub async fn run(pool: &PgPool) {
    // 1. Ensure the tracker table exists.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations ( \
            name TEXT PRIMARY KEY, \
            applied_at TIMESTAMPTZ DEFAULT NOW() \
        )",
    )
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("failed to create schema_migrations: {e}"));

    // 2. Seed existing migrations so they are never re-applied after this
    //    change is first deployed onto a DB that already ran them.
    sqlx::query(
        "INSERT INTO schema_migrations (name) \
         VALUES ('001_analytics.sql'), ('20260417_partner_tokens.sql') \
         ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("failed to seed schema_migrations: {e}"));

    // 3. Apply each migration that hasn't been recorded yet.
    let mut applied = 0usize;
    for (name, sql) in MIGRATIONS {
        let already_applied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE name = $1)")
                .bind(name)
                .fetch_one(pool)
                .await
                .unwrap_or_else(|e| panic!("schema_migrations check for {name} failed: {e}"));

        if already_applied {
            tracing::debug!(migration = %name, "migration already applied, skipping");
            continue;
        }

        for statement in sql.split(';') {
            let stmt = statement.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt)
                .execute(pool)
                .await
                .unwrap_or_else(|e| panic!("migration {name} failed on statement: {stmt}\n{e}"));
        }

        sqlx::query("INSERT INTO schema_migrations (name) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(name)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("failed to record migration {name}: {e}"));

        tracing::debug!(migration = %name, "migration applied");
        applied += 1;
    }

    tracing::info!(applied, total = MIGRATIONS.len(), "migrations complete");
}
