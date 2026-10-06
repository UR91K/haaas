use std::{env, str::FromStr, time::Duration};

use haaas_counter::{MIGRATOR, app};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data.db?mode=rwc".into());
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let secure_cookies = env::var("COOKIE_SECURE").is_ok_and(|v| v == "1" || v == "true");

    let opts = SqliteConnectOptions::from_str(&database_url)
        .expect("DATABASE_URL")
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new().connect_with(opts).await.expect("connect db");
    MIGRATOR.run(&pool).await.expect("migrations");

    let state = app::AppState::new(pool);
    let app = app::router(state, secure_cookies).await.expect("router");

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.expect("bind");
    tracing::info!(%bind_addr, "listening");
    axum::serve(listener, app).await.expect("serve");
}
