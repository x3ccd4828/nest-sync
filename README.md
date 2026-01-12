# Nest Sync - Rust Implementation

A Rust application for downloading and syncing Google Nest camera events with proper metadata and timestamps.

## Architecture

This application mirrors the Python [`google-nest-telegram-sync`](https://github.com/TamirMa/google-nest-telegram-sync) implementation with the following modular structure. For more details on the Google Nest Camera internal API, see [this Medium article](https://medium.com/@tamirmayer/google-nest-camera-internal-api-fdf9dc3ce167).

### Modules

1. **`google_auth.rs`** - Google authentication and API client
   - OAuth token management with automatic refresh
   - gRPC client for Google Home Foyer API
   - Device discovery via HomeGraph API
   - Nest API request wrapper

2. **`models.rs`** - Data models
   - `CameraEvent` - Represents camera event with start time and duration
   - XML attribute parsing from Nest API responses

3. **`nest_api.rs`** - Nest device API client
   - `NestDevice` - Device representation with ID and name
   - Event retrieval with timezone support
   - Video download functionality
   - XML parsing for event manifests

4. **`main.rs`** - Application entry point
   - Environment configuration loading
   - Device discovery orchestration
   - Event downloading and processing
   - ffmpeg integration for metadata embedding
   - File timestamp synchronization

## Features

- ✅ **Authentication**: Google OAuth token management with gRPC
- ✅ **Device Discovery**: Automatic Nest camera detection via HomeGraph API
- ✅ **Event Retrieval**: Fetch camera events with customizable time range
- ✅ **Video Download**: Download MP4 clips for each event with concurrent download support
- ✅ **File Timestamps**: Set filesystem timestamps to match event times
- ✅ **Timezone Support**: Proper timezone handling (defaults to America/Vancouver)
- ✅ **Continuous Sync**: Run continuously with configurable check intervals
- ✅ **Video Retention**: Automatic pruning of old videos based on retention policy
- ✅ **Structured Logging**: Using `tracing` for observability with configurable log levels

## Dependencies

- **tokio**: Async runtime
- **reqwest**: HTTP client for REST APIs
- **tonic**: gRPC client framework
- **prost**: Protocol Buffers implementation
- **chrono**: Date and time handling with timezone support
- **quick-xml**: XML parsing for event manifests
- **anyhow**: Error handling
- **tracing**: Structured logging and diagnostics
- **clap**: Command-line argument parsing
- **walkdir**: Directory traversal for video pruning

## Configuration

Create a `.env` file with:

```env
GOOGLE_MASTER_TOKEN=your_master_token_here
GOOGLE_USERNAME=your_google_email@gmail.com
```

## Usage

```bash
# Run continuously with default settings (check every 5 minutes)
cargo run

# Run once and exit
cargo run -- --once

# Custom output directory and concurrency
cargo run -- --output ~/nest-videos --concurrency 20

# Custom check interval (in minutes)
cargo run -- --check-interval 10

# Configure video retention (in days, 0 = keep forever)
cargo run -- --retention-days 30

# Enable debug logging
RUST_LOG=debug cargo run

# Show all available options
cargo run -- --help
```

### Command-line Options

- `--output, -o <PATH>`: Output directory for downloaded videos (default: current directory)
- `--concurrency, -c <NUM>`: Number of concurrent downloads (default: 10)
- `--check-interval, -i <MIN>`: Minutes between event checks (default: 5)
- `--once`: Run once and exit instead of continuous mode
- `--retention-days <DAYS>`: Days to keep videos, 0 = keep forever (default: 60)
- `--retention-hours`: Use hours instead of days for retention (testing only)
- `--prune-interval <MIN>`: Minutes between pruning checks (default: 10)

### Logging

The application uses structured logging via `tracing`. Control log levels with the `RUST_LOG` environment variable:

```bash
# Info level (default)
RUST_LOG=info cargo run

# Debug level for verbose output
RUST_LOG=debug cargo run

# Only errors
RUST_LOG=error cargo run

# Module-specific logging
RUST_LOG=nest_sync=debug,tonic=info cargo run
```

## Event Processing Flow

1. Load environment variables from `.env`
2. Initialize tracing subscriber for structured logging
3. Authenticate with Google using master token
4. Query HomeGraph API via gRPC to discover Nest camera devices
5. Enter main loop (or run once):
   - **Event Check**: At configured intervals
     - Fetch events from last 12 hours for each camera
     - Download MP4 videos concurrently (respecting concurrency limit)
     - Organize files in YYYY/MM/DD directory structure
     - Set file modification time to match event time
     - Skip already downloaded files
   - **Video Pruning**: At configured intervals
     - Walk directory tree to find all MP4 files
     - Delete videos older than retention period
     - Log pruning statistics

## Implementation Notes

### OAuth Token Management
The implementation uses the `gpsoauth` protocol to exchange the master token for service-specific access tokens. The OAuth flow:
1. Uses Android client credentials to authenticate
2. Exchanges master token for access tokens (standard and Nest-specific)
3. Caches tokens with automatic refresh after 1 hour
4. Generates a random Android ID for each session

### gRPC Communication
The app uses tonic to communicate with Google's Home Foyer API (`googlehomefoyer-pa.googleapis.com:443`) using the Protocol Buffers definitions from `api.proto`. TLS is configured with native system roots for certificate validation.

### Device Filtering
Devices are filtered by:
- Trait: `action.devices.traits.CameraStream`
- Hardware model contains: "Nest"

## Requirements

- Rust 1.70+ (edition 2024)
- Valid Google master token
- Sufficient disk space for video storage

## Comparison with Python Version

| Feature | Python | Rust |
|---------|--------|------|
| Async/Await | ✅ asyncio | ✅ tokio |
| gRPC | ✅ grpcio | ✅ tonic |
| XML Parsing | ✅ ElementTree | ✅ quick-xml |
| HTTP Client | ✅ requests | ✅ reqwest |
| Error Handling | Exceptions | Result<T, E> |
| Type Safety | Runtime | Compile-time |

## License

Same as original project
