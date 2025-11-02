use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{Connection, params};
use tracing::info;

/// Database wrapper for storing content
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Create a new database (in-memory or file-based)
    pub fn new(db_path: Option<PathBuf>) -> Result<Self> {
        let conn = if let Some(path) = db_path {
            info!("Opening SQLite database at: {}", path.display());
            Connection::open(path)?
        } else {
            info!("Using in-memory SQLite database");
            Connection::open_in_memory()?
        };

        // Create the resources table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS resources (
                path TEXT NOT NULL,
                query TEXT NOT NULL,
                content TEXT NOT NULL,
                PRIMARY KEY (path, query)
            )",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Look up content by path and query
    pub async fn get(&self, path: &str, query: &str) -> Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        let path = path.to_string();
        let query = query.to_string();

        tokio::task::Builder::new()
            .name("db-get")
            .spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                let result: Result<String, rusqlite::Error> = conn.query_row(
                    "SELECT content FROM resources WHERE path = ?1 AND query = ?2",
                    params![path, query],
                    |row| row.get(0),
                );

                match result {
                    Ok(content) => Ok(Some(content)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e.into()),
                }
            })?
            .await?
    }

    /// Store content in database
    pub async fn set(&self, path: &str, query: &str, content: &str) -> Result<()> {
        let conn = Arc::clone(&self.conn);
        let path = path.to_string();
        let query = query.to_string();
        let content = content.to_string();

        tokio::task::Builder::new()
            .name("db-set")
            .spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                conn.execute(
                    "INSERT OR REPLACE INTO resources (path, query, content) VALUES (?1, ?2, ?3)",
                    params![path, query, content],
                )?;
                Ok(())
            })?
            .await?
    }
}
