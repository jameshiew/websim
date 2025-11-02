use std::collections::HashSet;

use tokio::sync::RwLock;

use crate::config::WebSimConfig;
use crate::db::Database;
use crate::openrouter::OpenRouterClient;

/// Shared application state
pub struct AppState {
    pub db: Database,
    pub config: WebSimConfig,
    pub openrouter_client: OpenRouterClient,
    /// Tracks in-flight requests to prevent duplicate generation for the same path
    pub in_flight: RwLock<HashSet<String>>,
}
