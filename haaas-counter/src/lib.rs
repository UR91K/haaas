pub mod app;
pub mod csrf;
pub mod error;
pub mod render;
pub mod routes;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();