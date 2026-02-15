use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

const MAX_EVENT_DURATION_SECS: i64 = 10 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraEvent {
    pub device_id: String,
    pub start_time: DateTime<Utc>,
    pub duration: Duration,
}

impl CameraEvent {
    pub fn new(device_id: String, start_time: DateTime<Utc>, duration: Duration) -> Self {
        Self {
            device_id,
            start_time,
            duration,
        }
    }

    pub fn end_time(&self) -> DateTime<Utc> {
        self.start_time + self.duration
    }

    pub fn event_id(&self) -> String {
        format!(
            "{}->{}|{}",
            self.start_time.to_rfc3339(),
            self.end_time().to_rfc3339(),
            self.device_id
        )
    }

    pub fn from_xml_attributes(
        device_id: String,
        program_date_time: &str,
        duration_str: &str,
    ) -> anyhow::Result<Self> {
        let start_time = DateTime::parse_from_rfc3339(program_date_time)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                chrono::DateTime::parse_from_str(program_date_time, "%Y-%m-%dT%H:%M:%S%.fZ")
                    .map(|dt| dt.with_timezone(&Utc))
            })?;

        let duration_parsed = iso8601_duration::Duration::parse(duration_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse duration: {:?}", e))?;
        let duration_secs = duration_parsed.num_seconds().unwrap_or(0.0) as i64;
        let capped_duration_secs = duration_secs.min(MAX_EVENT_DURATION_SECS);

        if duration_secs > MAX_EVENT_DURATION_SECS {
            warn!(
                %device_id,
                %program_date_time,
                duration_secs,
                capped_duration_secs,
                "Event duration exceeded cap; clipping download window"
            );
        }

        let duration = Duration::seconds(capped_duration_secs);

        Ok(Self::new(device_id, start_time, duration))
    }
}
