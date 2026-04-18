use sqlx::PgPool;

/// Ordered list of embedded migrations. Each is applied at boot via simple
/// `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` idempotency
/// — no version table yet because every statement is self-guarded.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_analytics",
        include_str!("../migrations/001_analytics.sql"),
    ),
    (
        "20260417_partner_tokens",
        include_str!("../migrations/20260417_partner_tokens.sql"),
    ),
];

pub async fn run(pool: &PgPool) {
    for (name, sql) in MIGRATIONS {
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
        tracing::debug!(migration = %name, "migration applied");
    }
    tracing::info!(count = MIGRATIONS.len(), "migrations applied");
}
