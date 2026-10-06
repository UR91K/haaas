use std::convert::Infallible;

use askama::Template;
use axum::{
    Router,
    extract::State,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use sqlx::SqlitePool;
use tokio_stream::{
    StreamExt,
    wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
};
use tower_sessions::Session;

use crate::{app::AppState, csrf, error::AppError, render::Page};

const SESSION_CLICKS: &str = "clicks";

#[derive(Template)]
#[template(path = "index.html")]
struct Index {
    csrf_token: String,
    count: i64,
    session_count: i64,
}

#[derive(Template)]
#[template(path = "partials/count.html")]
struct Count {
    count: i64,
    oob: bool,
}

#[derive(Template)]
#[template(path = "partials/session_count.html")]
struct SessionCount {
    session_count: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/counter/increment", post(increment))
        .route("/counter/events", get(events))
}

async fn current(db: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT value FROM counter WHERE id = 1").fetch_one(db).await
}

async fn index(session: Session, State(state): State<AppState>) -> Result<Page<Index>, AppError> {
    Ok(Page(Index {
        csrf_token: csrf::token(&session).await?,
        count: current(&state.db).await?,
        session_count: session.get::<i64>(SESSION_CLICKS).await?.unwrap_or(0),
    }))
}

async fn increment(session: Session, State(state): State<AppState>) -> Result<Response, AppError> {
    let count: i64 =
        sqlx::query_scalar("UPDATE counter SET value = value + 1 WHERE id = 1 RETURNING value")
            .fetch_one(&state.db)
            .await?;
    let session_count = session.get::<i64>(SESSION_CLICKS).await?.unwrap_or(0) + 1;
    session.insert(SESSION_CLICKS, session_count).await?;

    // No receivers just means nobody is listening; not an error.
    let _ = state.events.send(count);

    let mut body = SessionCount { session_count }.render()?;
    body.push_str(&Count { count, oob: true }.render()?);
    Ok(axum::response::Html(body).into_response())
}

/// SSE messages must be a single data line, so newlines are stripped from the fragment.
fn count_event(count: i64) -> Result<Event, askama::Error> {
    let html = Count { count, oob: false }.render()?.replace(['\n', '\r'], "");
    Ok(Event::default().event("count").data(html))
}

async fn events(
    State(state): State<AppState>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, AppError> {
    // Subscribe before reading, so an update landing in between is not lost.
    let rx = state.events.subscribe();
    let initial = current(&state.db).await?;

    let db = state.db.clone();
    let updates = BroadcastStream::new(rx).then(move |msg| {
        let db = db.clone();
        async move {
            match msg {
                Ok(count) => count,
                // Missed some updates: resync from the source of truth.
                Err(BroadcastStreamRecvError::Lagged(_)) => current(&db).await.unwrap_or(0),
            }
        }
    });

    let stream = tokio_stream::once(initial)
        .chain(updates)
        .filter_map(|count| match count_event(count) {
            Ok(ev) => Some(Ok(ev)),
            Err(e) => {
                tracing::error!(error = %e, "render count event");
                None
            }
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}