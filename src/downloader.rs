use chrono::{DateTime, Datelike, Utc};
use flate2::read::GzDecoder;
use reqwest::Client;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{database::GeoDatabase, SharedDatabase};

const MIN_FILE_SIZE: u64 = 1024 * 1024; // 1MB minimum
const UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

pub struct DatabaseDownloader {
    data_dir: PathBuf,
    client: Client,
}

impl DatabaseDownloader {
    pub fn new(data_dir: &str) -> Self {
        let client = Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            data_dir: PathBuf::from(data_dir),
            client,
        }
    }

    pub async fn start_background_updates(&mut self, database: SharedDatabase) {
        info!("Starting database background update service");

        // Initial setup
        if let Err(e) = self.ensure_database_exists().await {
            error!("Failed to ensure database exists: {}", e);
        }

        // Load initial database if available
        if let Err(e) = self.load_latest_database(&database).await {
            error!("Failed to load initial database: {}", e);
        }

        // Start periodic update loop
        loop {
            sleep(UPDATE_INTERVAL).await;

            match self.check_for_updates().await {
                Ok(updated) => {
                    if updated {
                        info!("Database updated, reloading...");
                        if let Err(e) = self.load_latest_database(&database).await {
                            error!("Failed to reload database after update: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to check for database updates: {}", e);
                }
            }

            // Cleanup old files
            if let Err(e) = self.cleanup_old_databases().await {
                error!("Failed to cleanup old databases: {}", e);
            }
        }
    }

    async fn ensure_database_exists(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        fs::create_dir_all(&self.data_dir)?;

        let latest_path = self.data_dir.join("latest.mmdb");
        if !latest_path.exists() {
            info!("No database found, downloading initial database...");
            self.download_current_month().await?;
        }

        Ok(())
    }

    async fn check_for_updates(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let current_filename = self.get_current_month_filename(&now);
        let current_path = self.data_dir.join(&current_filename);

        if !current_path.exists() {
            info!("Current month database not found, downloading: {}", current_filename);
            self.download_current_month().await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn download_current_month(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let filename = self.get_current_month_filename(&now);
        let download_url = self.get_download_url(&now);

        info!("Downloading database from: {}", download_url);

        let response = self.client.get(&download_url).send().await?;
        if !response.status().is_success() {
            return Err(format!("Download failed with status: {}", response.status()).into());
        }

        let compressed_data = response.bytes().await?;
        info!("Downloaded {} bytes (compressed)", compressed_data.len());

        // Decompress the gzipped data
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut decompressed_data = Vec::new();
        decoder.read_to_end(&mut decompressed_data)?;

        if decompressed_data.len() < MIN_FILE_SIZE as usize {
            return Err(format!(
                "Downloaded file too small: {} bytes (minimum: {} bytes)",
                decompressed_data.len(),
                MIN_FILE_SIZE
            ).into());
        }

        // Write to temporary file first
        let temp_path = self.data_dir.join(format!("{}.tmp", filename));
        let final_path = self.data_dir.join(&filename);

        {
            let mut temp_file = File::create(&temp_path)?;
            temp_file.write_all(&decompressed_data)?;
            temp_file.sync_all()?;
        }

        // Move temp file to final location
        fs::rename(&temp_path, &final_path)?;

        // Update symlink atomically
        self.update_latest_symlink(&filename)?;

        info!("Successfully downloaded and installed: {}", filename);
        Ok(())
    }

    fn get_current_month_filename(&self, date: &DateTime<Utc>) -> String {
        format!("dbip-city-lite-{}-{:02}.mmdb", date.year(), date.month())
    }

    fn get_download_url(&self, date: &DateTime<Utc>) -> String {
        format!(
            "https://download.db-ip.com/free/dbip-city-lite-{}-{:02}.mmdb.gz",
            date.year(),
            date.month()
        )
    }

    fn update_latest_symlink(&self, filename: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let latest_path = self.data_dir.join("latest.mmdb");
        let temp_symlink = self.data_dir.join("latest.mmdb.tmp");

        // Remove existing temp symlink if it exists
        let _ = fs::remove_file(&temp_symlink);

        // Create new symlink to temp location
        #[cfg(unix)]
        std::os::unix::fs::symlink(filename, &temp_symlink)?;
        
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(filename, &temp_symlink)?;

        // Atomically replace the symlink
        fs::rename(&temp_symlink, &latest_path)?;

        Ok(())
    }

    async fn load_latest_database(&self, database: &SharedDatabase) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let latest_path = self.data_dir.join("latest.mmdb");
        if !latest_path.exists() {
            warn!("No latest database symlink found");
            return Ok(());
        }

        info!("Loading database from: {:?}", latest_path);
        let new_db = GeoDatabase::new(&latest_path)?;
        
        let mut db_guard = database.write().await;
        *db_guard = Some(new_db);
        info!("Database loaded successfully");

        Ok(())
    }

    async fn cleanup_old_databases(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut db_files = Vec::new();

        let entries = fs::read_dir(&self.data_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("dbip-city-lite-") && filename.ends_with(".mmdb") {
                    let metadata = entry.metadata()?;
                    if let Ok(modified) = metadata.modified() {
                        db_files.push((path, modified));
                    }
                }
            }
        }

        // Sort by modification time, newest first
        db_files.sort_by(|a, b| b.1.cmp(&a.1));

        // Keep only the 3 most recent files
        for (path, _) in db_files.iter().skip(3) {
            info!("Removing old database file: {:?}", path);
            if let Err(e) = fs::remove_file(path) {
                warn!("Failed to remove old database file {:?}: {}", path, e);
            }
        }

        Ok(())
    }
}