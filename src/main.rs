use axum::{extract::Path, http::StatusCode, response::Json, routing::get, Router};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tracing::info;

mod database;
mod downloader;

use database::GeoDatabase;
use downloader::DatabaseDownloader;

#[derive(Parser)]
#[command(name = "geoip-api")]
#[command(about = "A lightweight GeoIP API service")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:80")]
    bind: SocketAddr,
    
    #[arg(long, default_value = "/data")]
    data_dir: String,
}

#[derive(Serialize, Deserialize)]
struct GeoLocation {
    ip: String,
    city: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    timezone: Option<String>,
    accuracy_radius: Option<u16>,
}

type SharedDatabase = Arc<tokio::sync::RwLock<Option<GeoDatabase>>>;

async fn lookup_ip(
    Path(ip): Path<String>,
    database: axum::extract::State<SharedDatabase>,
) -> Result<Json<GeoLocation>, StatusCode> {
    let db_guard = database.read().await;
    let db = match db_guard.as_ref() {
        Some(db) => db,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    match db.lookup(&ip).await {
        Ok(location) => Ok(Json(location)),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "healthy"}))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("GeoIP API starting...");
    eprintln!("Args: {:?}", std::env::args().collect::<Vec<_>>());
    
    // Initialize tracing with environment filter
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();
    
    let args = Args::parse();
    info!("Starting GeoIP API server on {}", args.bind);
    
    let database = Arc::new(tokio::sync::RwLock::new(None::<GeoDatabase>));
    
    let db_clone = Arc::clone(&database);
    let data_dir = args.data_dir.clone();
    tokio::spawn(async move {
        let mut downloader = DatabaseDownloader::new(&data_dir);
        downloader.start_background_updates(db_clone).await;
    });
    
    let app = Router::new()
        .route("/health", get(health))
        .route("/{ip}", get(lookup_ip))
        .with_state(database);

    let listener = match tokio::net::TcpListener::bind(&args.bind).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to {}: {}", args.bind, e);
            std::process::exit(1);
        }
    };
    info!("Server listening on {}", args.bind);
    
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
    
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received");
}