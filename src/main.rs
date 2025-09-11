use axum::{
    extract::{Path, Request},
    http::StatusCode,
    middleware::{self, Next},
    response::{Json, Response},
    routing::get,
    Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
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
    subdivision: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
    continent: Option<String>,
    continent_code: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    timezone: Option<String>,
    accuracy_radius: Option<u16>,
}

type SharedDatabase = Arc<tokio::sync::RwLock<Option<GeoDatabase>>>;

fn get_allowed_hosts() -> Vec<String> {
    let default_hosts = "localhost,127.0.0.1";
    let allowed_hosts = env::var("ALLOWED_HOSTS").unwrap_or_else(|_| default_hosts.to_string());
    allowed_hosts
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect()
}

fn get_api_key() -> Option<String> {
    env::var("API_KEY").ok()
}

fn is_host_allowed(host: &str, allowed_hosts: &[String]) -> bool {
    let host = host.to_lowercase();

    for allowed in allowed_hosts {
        if allowed.starts_with('*') {
            let suffix = &allowed[1..];
            if host.ends_with(suffix) {
                return true;
            }
        } else if host == *allowed {
            return true;
        }
    }
    false
}

async fn validate_host(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let allowed_hosts = get_allowed_hosts();

    if let Some(host_header) = request.headers().get("host") {
        if let Ok(host_str) = host_header.to_str() {
            let host = host_str.split(':').next().unwrap_or(host_str);

            if is_host_allowed(host, &allowed_hosts) {
                return Ok(next.run(request).await);
            }
        }
    }

    Err(StatusCode::FORBIDDEN)
}

async fn validate_api_key(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let api_key = match get_api_key() {
        Some(key) => key,
        None => return Ok(next.run(request).await),
    };

    let provided_key = extract_api_key_from_request(&request);

    match provided_key {
        Some(key) if key == api_key => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn extract_api_key_from_request(request: &Request) -> Option<String> {
    if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    if let Some(api_key_header) = request.headers().get("x-api-key") {
        if let Ok(key) = api_key_header.to_str() {
            return Some(key.to_string());
        }
    }

    if let Some(query) = request.uri().query() {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "api_key" {
                    return Some(value.to_string());
                }
            }
        }
    }

    None
}

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
        .merge(
            Router::new()
                .route("/{ip}", get(lookup_ip))
                .with_state(database.clone())
                .layer(middleware::from_fn(validate_api_key))
                .layer(middleware::from_fn(validate_host))
        )
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