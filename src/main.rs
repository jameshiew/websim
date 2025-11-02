use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "websim")]
#[command(about = "AI-powered web simulator", long_about = None)]
struct Args {
    /// Path to SQLite database for caching (if not provided, uses in-memory database)
    #[arg(long)]
    db: Option<PathBuf>,

    /// Path to configuration file
    #[arg(short, long, default_value = "websim.config.yml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing subscriber
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_ids(true)
        .with_target(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

    // Initialize tokio-console if TOKIO_CONSOLE environment variable is set
    if std::env::var("TOKIO_CONSOLE").is_ok() {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        tracing_subscriber::registry()
            .with(console_subscriber::spawn())
            .with(fmt_layer)
            .with(env_filter)
            .init();

        info!("tokio-console enabled on http://127.0.0.1:6669");
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_thread_ids(true)
            .with_target(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
            .init();
    }

    websim::run_server(args.db, args.config).await
}
