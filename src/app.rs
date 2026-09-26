use std::sync::Arc;

use axum::Router;
use color_eyre::eyre::eyre;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{
    Layer, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::routes;

/// Segredos lidos do ambiente, nunca fixos no código.
#[derive(Clone)]
pub struct Config {
    pub jwt_secret: Arc<[u8]>,
    pub admin_key: Arc<str>,
}

impl Config {
    fn from_env() -> color_eyre::Result<Self> {
        let jwt_secret = std::env::var("JWT_SECRET")?;
        if jwt_secret.len() < 16 {
            return Err(eyre!("JWT_SECRET precisa ter pelo menos 16 caracteres"));
        }
        let admin_key = std::env::var("ADMIN_KEY")?;

        Ok(Self {
            jwt_secret: jwt_secret.into_bytes().into(),
            admin_key: admin_key.into(),
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
}

impl AppState {
    async fn new() -> color_eyre::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")?;
        let db = PgPool::connect(&database_url).await?;
        sqlx::migrate!().run(&db).await?;

        Ok(Self {
            db,
            config: Config::from_env()?,
        })
    }
}

pub struct App;

impl App {
    pub async fn start() -> color_eyre::Result<()> {
        let layer = tracing_subscriber::fmt::layer()
            .with_span_events(FmtSpan::NEW)
            .boxed();

        tracing_subscriber::registry().with(layer).init();

        dotenvy::dotenv().ok();
        let state = AppState::new().await?;

        let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
        let router = Router::new()
            .nest("/api", routes::api::router())
            .merge(routes::frontend::router())
            .with_state(state);

        info!("Starting service on port {port}");

        axum::serve(listener, router).await?;

        Ok(())
    }
}
