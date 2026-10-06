use askama::Template;
use axum::response::{Html, IntoResponse, Response};

use crate::error::AppError;

/// Wrap a template so a handler can return it directly. Escaping stays on (askama default).
pub struct Page<T>(pub T);

impl<T: Template> IntoResponse for Page<T> {
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(body) => Html(body).into_response(),
            Err(e) => AppError::from(e).into_response(),
        }
    }
}
