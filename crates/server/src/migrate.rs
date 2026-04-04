use sqlx::PgPool;

const MIGRATION: &str = include_str!("../migrations/001_analytics.sql");

pub async fn run(pool: &PgPool) {
    sqlx::query(MIGRATION)
        .execute(pool)
        .await
        .expect("migration failed");
    tracing::info!("migrations applied");
}
