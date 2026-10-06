use askama::Template;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use tower_sessions::Session;

use crate::csrf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("session error: {0}")]
    Session(#[from] tower_sessions::session::Error),
    #[error("template error: {0}")]
    Render(#[from] askama::Error),
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Message that is safe to show to the user; internals are only logged.
    fn public_message(&self) -> &'static str {
        match self {
            AppError::Forbidden(_) => "Request rejected.",
            _ => "Something went wrong.",
        }
    }
}

/// Carries the error to `render_errors`, which knows whether the request came from htmx.
#[derive(Clone)]
struct ErrorInfo {
    status: StatusCode,
    message: &'static str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!(error = %self, "request failed");
        let info = ErrorInfo { status: self.status(), message: self.public_message() };
        let mut res = info.status.into_response();
        res.extensions_mut().insert(info);
        res
    }
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage {
    csrf_token: String,
    status: u16,
    message: &'static str,
}

#[derive(Template)]
#[template(path = "partials/error.html")]
struct ErrorFragment {
    message: &'static str,
}

/// Turns an `AppError` response into a full page, or a small fragment for htmx requests.
pub async fn render_errors(session: Session, req: Request, next: Next) -> Response {
    let is_htmx = req.headers().contains_key("hx-request");
    let res = next.run(req).await;
    let Some(info) = res.extensions().get::<ErrorInfo>().cloned() else {
        return res;
    };

    let body = if is_htmx {
        ErrorFragment { message: info.message }.render()
    } else {
        let csrf_token = csrf::token(&session).await.unwrap_or_default();
        ErrorPage { csrf_token, status: info.status.as_u16(), message: info.message }.render()
    };
    match body {
        Ok(html) => {
            let mut res = (info.status, Html(html)).into_response();
            if is_htmx {
                // htmx ignores 4xx/5xx bodies by default; the htmx:responseError handler
                // or hx-swap config decides what to do. Retarget so the fragment is visible.
                res.headers_mut().insert("HX-Retarget", "#errors".parse().unwrap());
                res.headers_mut().insert("HX-Reswap", "innerHTML".parse().unwrap());
            }
            res
        }
        Err(_) => info.status.into_response(),
    }
}
