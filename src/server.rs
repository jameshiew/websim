use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::any;
use config::Config;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::WebSimConfig;
use crate::db::Database;
use crate::handler::handle;
use crate::openrouter::OpenRouterClient;
use crate::state::AppState;

pub async fn run_server(db_path: Option<PathBuf>, config_path: PathBuf) -> Result<()> {
    // Load configuration
    let config_str = config_path.display().to_string();
    let config = Config::builder()
        .add_source(config::File::with_name(&config_str))
        .build()
        .with_context(|| format!("Failed to load config from: {}", config_str))?;

    let websim_config: WebSimConfig = config
        .try_deserialize()
        .with_context(|| format!("Failed to parse config from: {}", config_str))?;

    info!(
        "Loaded config from {} with {} content types",
        config_str,
        websim_config.content_types.len()
    );

    // Log configured content types
    for (mime_type, ct_config) in &websim_config.content_types {
        info!(
            "  {} -> {} (model: {}, extensions: {})",
            mime_type,
            ct_config.content_type_header,
            ct_config.model,
            ct_config.extensions.join(", ")
        );
    }

    // Initialize database
    let db = Database::new(db_path)?;

    // Initialize OpenRouter client
    let api_key = std::env::var("WEBSIM_API_KEY")
        .with_context(|| "WEBSIM_API_KEY environment variable must be set")?;
    let openrouter_client = OpenRouterClient::new(api_key.into());

    let state = Arc::new(AppState {
        db,
        config: websim_config,
        openrouter_client,
        in_flight: RwLock::new(HashSet::new()),
    });

    let app = Router::new().fallback(any(handle)).with_state(state);

    let listener = tokio::net::TcpListener::bind("localhost:3000").await?;
    info!("Server running on http://localhost:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
