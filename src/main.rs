mod google_auth;
mod models;
mod nest_api;

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use chrono_tz::America::Vancouver;
use clap::Parser;
use filetime::FileTime;
use google_auth::GoogleConnection;
use nest_api::NestDevice;
use tokio::{sync::Semaphore, task::JoinSet, time};
use tracing::{debug, error, info};

const EVENT_HISTORY_DURATION_MINUTES: i64 = 12 * 60;

struct AppState {
    google_connection: GoogleConnection,
    nest_camera_devices: Vec<(String, String)>,
    google_master_token: String,
    google_username: String,
    output_path: PathBuf,
}

async fn initialize(args: &Args) -> Option<AppState> {
    let google_master_token = match std::env::var("GOOGLE_MASTER_TOKEN") {
        Ok(token) => token,
        Err(e) => {
            error!(error = %e, "GOOGLE_MASTER_TOKEN environment variable not set");
            return None;
        }
    };
    let google_username = match std::env::var("GOOGLE_USERNAME") {
        Ok(username) => username,
        Err(e) => {
            error!(error = %e, "GOOGLE_USERNAME environment variable not set");
            return None;
        }
    };

    let output_path = shellexpand::tilde(&args.output.to_string_lossy()).to_string();
    let output_path = PathBuf::from(output_path);
    if let Err(e) = fs::create_dir_all(&output_path) {
        error!(error = %e, "Failed to create output directory");
        return None;
    }

    let mut google_connection =
        GoogleConnection::new(google_master_token.clone(), google_username.clone());

    let nest_camera_devices = match google_connection.get_nest_camera_devices().await {
        Ok(devices) => {
            let device_count = devices.len();
            info!(device_count, "Found camera devices");
            devices
        }
        Err(e) => {
            error!(error = %e, "Failed to get camera devices");
            return None;
        }
    };

    Some(AppState {
        google_connection,
        nest_camera_devices,
        google_master_token,
        google_username,
        output_path,
    })
}

async fn prune_old_videos(
    output_path: &Path,
    retention_period: u64,
    use_hours: bool,
) -> Result<()> {
    if retention_period == 0 {
        // No pruning
        return Ok(());
    }

    let unit = if use_hours { "hours" } else { "days" };
    info!(
        retention_period,
        unit, "Pruning videos older than specified period"
    );

    let retention_seconds = if use_hours {
        retention_period * 60 * 60
    } else {
        retention_period * 24 * 60 * 60
    };
    let cutoff_time = SystemTime::now() - Duration::from_secs(retention_seconds);
    let mut deleted_count = 0;
    let mut kept_count = 0;

    // Walk through all .mp4 files in the directory tree
    for entry in walkdir::WalkDir::new(output_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mp4"))
    {
        let path = entry.path();

        match fs::metadata(path) {
            Ok(metadata) => {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff_time {
                        match fs::remove_file(path) {
                            Ok(_) => {
                                info!(path = %path.display(), "Deleted old video");
                                deleted_count += 1;
                            }
                            Err(e) => {
                                error!(path = %path.display(), error = %e, "Failed to delete video");
                            }
                        }
                    } else {
                        kept_count += 1;
                    }
                }
            }
            Err(e) => {
                error!(path = %path.display(), error = %e, "Failed to get metadata");
            }
        }
    }

    info!(deleted_count, kept_count, "Pruning complete");

    Ok(())
}

async fn check_and_download_events(
    google_connection: &mut GoogleConnection,
    nest_camera_devices: &[(String, String)],
    output_path: &Path,
    semaphore: &Arc<Semaphore>,
    google_master_token: &str,
    google_username: &str,
    interval_minutes: u64,
) -> Result<()> {
    info!("Checking for new events");
    let mut join_set = JoinSet::new();
    let mut completed_count = 0;
    let mut total_count = 0;

    for (device_id, device_name) in nest_camera_devices {
        let nest_device = NestDevice::new(device_id.clone(), device_name.clone());

        let end_time: DateTime<Utc> = Utc::now();
        let events = nest_device
            .get_events(google_connection, end_time, EVENT_HISTORY_DURATION_MINUTES)
            .await?;
        info!(count = events.len(), device_name, "Received camera events");

        for event in events {
            let event_local_time = event.start_time.with_timezone(&Vancouver);

            // Create folder structure: YEAR/MONTH/DAY
            let year = event_local_time.format("%Y").to_string();
            let month = event_local_time.format("%m").to_string();
            let day = event_local_time.format("%d").to_string();
            let date_folder = output_path.join(&year).join(&month).join(&day);

            fs::create_dir_all(&date_folder).context("Failed to create date folder structure")?;

            let filename = event_local_time.format("%Y-%m-%dT%H-%M-%S.mp4").to_string();
            let filepath = date_folder.join(&filename);

            if filepath.exists() {
                debug!(
                    event_id = %event.event_id(),
                    path = %filepath.display(),
                    "Skipping camera event, file already exists"
                );
                continue;
            }

            info!(
                event_id = %event.event_id(),
                path = %filepath.display(),
                "Downloading camera event"
            );

            // Acquire permit before spawning to prevent unbounded task creation
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(e) => {
                    error!(error = %e, "Failed to acquire semaphore permit");
                    continue;
                }
            };

            let nest_device_clone = nest_device.clone();
            let google_master_token_clone = google_master_token.to_string();
            let google_username_clone = google_username.to_string();
            let event_clone = event.clone();
            let filepath_clone = filepath.clone();

            total_count += 1;

            join_set.spawn(async move {
                let _permit = permit;

                // Create a new GoogleConnection for this task
                let mut task_google_connection =
                    GoogleConnection::new(google_master_token_clone, google_username_clone);

                let video_data = nest_device_clone
                    .download_camera_event(&mut task_google_connection, &event_clone)
                    .await?;

                let mut file =
                    fs::File::create(&filepath_clone).context("Failed to create file")?;
                file.write_all(&video_data)
                    .context("Failed to write video data")?;

                let event_local_time = event_clone.start_time.with_timezone(&Vancouver);
                let timestamp = event_local_time.timestamp();
                let filetime = FileTime::from_unix_time(timestamp, 0);
                filetime::set_file_times(&filepath_clone, filetime, filetime)
                    .context("Failed to set file times")?;

                Ok::<(), anyhow::Error>(())
            });

            // Drain completed tasks to avoid accumulating all tasks in memory
            while let Some(result) = join_set.try_join_next() {
                match result {
                    Ok(Ok(())) => {
                        completed_count += 1;
                        info!(completed_count, total_count, "Download progress");
                    }
                    Ok(Err(e)) => error!(error = %e, "Download error"),
                    Err(e) => error!(error = %e, "Task join error"),
                }
            }
        }
    }

    // Wait for all remaining downloads to complete
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {
                completed_count += 1;
                info!(completed_count, total_count, "Download progress");
            }
            Ok(Err(e)) => error!(error = %e, "Download error"),
            Err(e) => error!(error = %e, "Task join error"),
        }
    }

    info!(completed_count, total_count, "All downloads complete");
    info!(interval_minutes, "Waiting before next check");

    Ok(())
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output directory for downloaded videos
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// Number of concurrent downloads
    #[arg(short, long, default_value = "10")]
    concurrency: usize,

    /// Interval in minutes to check for new events
    #[arg(short = 'i', long, default_value = "5")]
    check_interval: u64,

    /// Run once and exit instead of running continuously
    #[arg(long)]
    once: bool,

    /// Number of days to keep videos (0 = keep forever, no pruning)
    #[arg(long, default_value = "60")]
    retention_days: u64,

    /// Use hours instead of days for retention period (for testing)
    #[arg(long)]
    retention_hours: bool,

    /// Interval in minutes to prune old videos
    #[arg(long, default_value = "10")]
    prune_interval: u64,
}

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!(
        "Application: {}, Version: {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    dotenvy::dotenv().ok();

    let args = Args::parse();

    let mut app_state = None;

    let semaphore = Arc::new(Semaphore::new(args.concurrency));
    let mut check_events_interval = time::interval(Duration::from_secs(args.check_interval * 60));
    check_events_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    let mut prune_interval = time::interval(Duration::from_secs(args.prune_interval * 60));
    prune_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    if !args.once {
        info!(
            check_interval = args.check_interval,
            "Checking for events at regular intervals"
        );
        if args.retention_days > 0 {
            let unit = if args.retention_hours {
                "hours"
            } else {
                "days"
            };
            info!(
                retention_days = args.retention_days,
                unit,
                prune_interval = args.prune_interval,
                "Video pruning enabled"
            );
        } else {
            info!("Video pruning disabled (retention_days = 0)");
        }
    }

    loop {
        tokio::select! {
            _ = check_events_interval.tick() => {
                if app_state.is_none() {
                    app_state = initialize(&args).await;
                }

                if let Some(ref mut state) = app_state
                    && let Err(e) = check_and_download_events(
                        &mut state.google_connection,
                        &state.nest_camera_devices,
                        &state.output_path,
                        &semaphore,
                        &state.google_master_token,
                        &state.google_username,
                        args.check_interval,
                    ).await {
                        error!(error = %e, "Error checking events");
                    }
            }
            _ = prune_interval.tick() => {
                if let Some(ref state) = app_state
                    && let Err(e) = prune_old_videos(&state.output_path, args.retention_days, args.retention_hours).await {
                        error!(error = %e, "Error pruning videos");
                    }
            }
            // Add more branches here as needed
            // _ = some_signal => { ... }
        }

        if args.once {
            break;
        }
    }
}
