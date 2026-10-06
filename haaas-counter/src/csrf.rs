use axum::{
    extract::Request,
    http::{Method, header},
    middleware::Next,
    response::Response,
};
use rand::RngExt;
use tower_sessions::Session;

use crate::error::AppError;

const KEY: &str = "csrf_token";
const HEADER: &str = "x-csrf-token";

/// The session's CSRF token, generated on first use.
pub async fn token(session: &Session) -> Result<String, AppError> {
    if let Some(t) = session.get::<String>(KEY).await? {
        return Ok(t);
    }
    let bytes: [u8; 32] = rand::rng().random();
    let t: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    session.insert(KEY, &t).await?;
    Ok(t)
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Origin (if sent) must match the Host header.
fn origin_ok(req: &Request) -> bool {
    let Some(origin) = req.headers().get(header::ORIGIN) else { return true };
    let Some(host) = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    let Some(origin) = origin.to_str().ok() else { return false };
    let origin_host = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    origin_host == host
}

/// Synchronizer-token CSRF check for every method except GET/HEAD.
pub async fn protect(session: Session, req: Request, next: Next) -> Result<Response, AppError> {
    let expected = token(&session).await?; // also makes sure a session exists on first GET
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        if !origin_ok(&req) {
            return Err(AppError::Forbidden("origin mismatch"));
        }
        let sent = req.headers().get(HEADER).map(|v| v.as_bytes()).unwrap_or_default();
        if !ct_eq(sent, expected.as_bytes()) {
            return Err(AppError::Forbidden("bad csrf token"));
        }
    }
    Ok(next.run(req).await)
}
