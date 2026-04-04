use sqlx::PgPool;

const MIGRATION: &str = include_str!("../migrations/001_analytics.sql");

pub async fn run(pool: &PgPool) {
    for statement in MIGRATION.split(';') {
        let stmt = statement.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt)
            .execute(pool)
            .await
            .expect("migration failed");
    }
    tracing::info!("migrations applied");
}
