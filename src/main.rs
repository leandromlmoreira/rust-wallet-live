use crate::app::App;

mod app;
pub mod auth;
pub mod error;
pub mod format;
pub mod models;
pub mod portfolio;
pub mod repository;
pub mod routes;
pub mod validation;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    App::start().await
}
