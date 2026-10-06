use axum::Router;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, SessionManagerLayer, cookie::SameSite};
use tower_sessions_sqlx_store::SqliteStore;
use axum::middleware;

use crate::{csrf, error, routes};

/// Shared by all handlers. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    /// Live events (the new global count); SSE handlers subscribe.
    pub events: broadcast::Sender<i64>,
}

impl AppState {
    pub fn new(db: SqlitePool) -> Self {
        let (events, _) = broadcast::channel(64);
        Self { db, events }
    }
}


/// Builds the router. `secure_cookies` should be false on plain-http localhost.
pub async fn router(state: AppState, secure_cookies: bool) -> Result<Router, sqlx::Error> {
    let store = SqliteStore::new(state.db.clone());
    store.migrate().await?;

    let sessions = SessionManagerLayer::new(store)
        .with_secure(secure_cookies)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnSessionEnd);

    Ok(Router::new()
        .merge(routes::counter::router())
        .nest_service("/static", ServeDir::new("static"))
        // Layer order: last added is outermost. Sessions wrap everything that reads them.
        .layer(middleware::from_fn(csrf::protect))
        .layer(middleware::from_fn(error::render_errors))
        .layer(sessions)
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}
