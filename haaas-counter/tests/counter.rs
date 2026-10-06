use askama::Template;
use axum::{
    Router,
    body::{Body, HttpBody},
    http::{Request, Response, StatusCode, header},
};
use haaas_counter::{MIGRATOR, app};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn body_string(body: Body) -> String {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn test_app() -> Router {
    // One connection: each connection to `:memory:` would otherwise be its own database.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    app::router(app::AppState::new(pool), false).await.unwrap()
}

struct Session {
    cookie: String,
    token: String,
    body: String,
}

/// GET / with no cookie: a fresh session. Returns its cookie and CSRF token.
async fn new_session(app: &Router) -> Session {
    visit(app, None).await
}

async fn visit(app: &Router, cookie: Option<&str>) -> Session {
    let mut req = Request::get("/");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let res = app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = match res.headers().get(header::SET_COOKIE) {
        Some(c) => c.to_str().unwrap().split(';').next().unwrap().to_string(),
        None => cookie.expect("session cookie").to_string(),
    };
    let body = body_string(res.into_body()).await;
    let marker = "X-CSRF-Token\": \"";
    let start = body.find(marker).expect("token in page") + marker.len();
    let token = body[start..start + 64].to_string();
    Session { cookie, token, body }
}

async fn post_increment(app: &Router, cookie: &str, token: Option<&str>, origin: Option<&str>) -> Response<Body> {
    let mut req = Request::post("/counter/increment")
        .header(header::COOKIE, cookie)
        .header(header::HOST, "localhost:3000")
        .header("HX-Request", "true");
    if let Some(t) = token {
        req = req.header("X-CSRF-Token", t);
    }
    if let Some(o) = origin {
        req = req.header(header::ORIGIN, o);
    }
    app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap()
}

#[tokio::test]
async fn post_without_token_is_forbidden() {
    let app = test_app().await;
    let s = new_session(&app).await;
    let res = post_increment(&app, &s.cookie, None, None).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_with_wrong_token_is_forbidden() {
    let app = test_app().await;
    let s = new_session(&app).await;
    let wrong = "0".repeat(64);
    let res = post_increment(&app, &s.cookie, Some(&wrong), None).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    // Right length-prefix but shorter must fail too.
    let res = post_increment(&app, &s.cookie, Some(&s.token[..63]), None).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_with_valid_token_increments_and_returns_oob() {
    let app = test_app().await;
    let s = new_session(&app).await;
    let res = post_increment(&app, &s.cookie, Some(&s.token), None).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res.into_body()).await;
    assert!(body.contains(r#"<span id="session-count">1</span>"#), "{body}");
    assert!(body.contains(r#"id="count" hx-swap-oob="true" data-count="1">1</span>"#), "{body}");
}

#[tokio::test]
async fn mismatched_origin_is_forbidden_matching_origin_is_ok() {
    let app = test_app().await;
    let s = new_session(&app).await;
    let res = post_increment(&app, &s.cookie, Some(&s.token), Some("http://evil.example")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = post_increment(&app, &s.cookie, Some(&s.token), Some("http://localhost:3000")).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn htmx_errors_are_fragments_and_page_errors_are_pages() {
    let app = test_app().await;
    let s = new_session(&app).await;
    let res = post_increment(&app, &s.cookie, None, None).await;
    assert_eq!(res.headers().get("HX-Retarget").unwrap(), "#errors");
    let frag = body_string(res.into_body()).await;
    assert!(!frag.contains("<html"), "{frag}");

    let req = Request::post("/counter/increment").header(header::COOKIE, &s.cookie).body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert!(body_string(res.into_body()).await.contains("<html"));
}

#[tokio::test]
async fn global_count_is_shared_session_count_is_not() {
    let app = test_app().await;
    let a = new_session(&app).await;
    let b = new_session(&app).await;
    assert_ne!(a.cookie, b.cookie);

    for _ in 0..2 {
        assert_eq!(post_increment(&app, &a.cookie, Some(&a.token), None).await.status(), StatusCode::OK);
    }
    assert_eq!(post_increment(&app, &b.cookie, Some(&b.token), None).await.status(), StatusCode::OK);

    // Session A: global 3, own 2. Session B: global 3, own 1. Also survives a "refresh".
    let a2 = visit(&app, Some(&a.cookie)).await;
    let b2 = visit(&app, Some(&b.cookie)).await;
    assert!(a2.body.contains(r#"data-count="3""#));
    assert!(b2.body.contains(r#"data-count="3""#));
    assert!(a2.body.contains(r#"<span id="session-count">2</span>"#));
    assert!(b2.body.contains(r#"<span id="session-count">1</span>"#));
    // A brand-new session sees the global count but zero of its own.
    let c = new_session(&app).await;
    assert!(c.body.contains(r#"data-count="3""#));
    assert!(c.body.contains(r#"<span id="session-count">0</span>"#));
}

#[tokio::test]
async fn sse_sends_current_count_on_connect() {
    let app = test_app().await;
    let s = new_session(&app).await;
    post_increment(&app, &s.cookie, Some(&s.token), None).await;

    let req = Request::get("/counter/events").body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.headers().get(header::CONTENT_TYPE).unwrap(), "text/event-stream");
    let mut body = res.into_body();
    // Read only the first frame; the stream never ends.
    let frame = std::future::poll_fn(|cx| {
        std::pin::Pin::new(&mut body).poll_frame(cx)
    })
    .await
    .unwrap()
    .unwrap();
    let text = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(text.starts_with("event: count\n"), "{text}");
    assert!(text.contains(r#"data: <span id="count" data-count="1">1</span>"#), "{text}");
}

#[test]
fn templates_escape_html() {
    #[derive(Template)]
    #[template(source = "<p>{{ v }}</p>", ext = "html")]
    struct T<'a> {
        v: &'a str,
    }
    let out = T { v: "<script>alert(1)</script>" }.render().unwrap();
    assert!(!out.contains("<script>"), "{out}");
    assert!(out.contains("&#60;script&#62;") || out.contains("&lt;script&gt;"), "{out}");
}