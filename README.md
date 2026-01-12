# Nest Sync - Rust Implementation

A Rust application for downloading and syncing Google Nest camera events with proper metadata and timestamps.

## Architecture

This application mirrors the Python `google-nest-telegram-sync` implementation with the following modular structure:

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
- ✅ **Video Download**: Download MP4 clips for each event
- ✅ **Metadata Embedding**: Use ffmpeg to embed creation timestamps
- ✅ **File Timestamps**: Set filesystem timestamps to match event times
- ✅ **Timezone Support**: Proper timezone handling (configurable, defaults to America/Vancouver)

## Dependencies

- **tokio**: Async runtime
- **reqwest**: HTTP client for REST APIs
- **tonic**: gRPC client framework
- **prost**: Protocol Buffers implementation
- **chrono**: Date and time handling
- **quick-xml**: XML parsing for event manifests
- **anyhow**: Error handling

## Configuration

Create a `.env` file with:

```env
GOOGLE_MASTER_TOKEN=your_master_token_here
GOOGLE_USERNAME=your_google_email@gmail.com
```

## Usage

```bash
# Download events to current directory
cargo run

# Download events to specific directory
cargo run /path/to/output
```

## Event Processing Flow

1. Load environment variables from `.env`
2. Authenticate with Google using master token
3. Query HomeGraph API via gRPC to discover Nest camera devices
4. For each camera:
   - Fetch events from last 12 hours
   - Download MP4 video for each event
   - Embed creation timestamp in video metadata using ffmpeg
   - Set file modification time to match event time
   - Clean up temporary files

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

- Rust 1.70+
- ffmpeg installed and available in PATH
- Valid Google master token

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
