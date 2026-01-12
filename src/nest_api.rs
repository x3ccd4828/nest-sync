use crate::google_auth::GoogleConnection;
use crate::models::CameraEvent;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use quick_xml::Reader;
use quick_xml::events::Event;

const EVENTS_URI: &str = "https://nest-camera-frontend.googleapis.com/dashmanifest/namespace/nest-phoenix-prod/device/{device_id}";
const DOWNLOAD_VIDEO_URI: &str = "https://nest-camera-frontend.googleapis.com/mp4clip/namespace/nest-phoenix-prod/device/{device_id}";

pub struct NestDevice {
    pub device_id: String,
    #[allow(dead_code)]
    pub device_name: String,
}

impl Clone for NestDevice {
    fn clone(&self) -> Self {
        Self {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
        }
    }
}

impl NestDevice {
    pub fn new(device_id: String, device_name: String) -> Self {
        Self {
            device_id,
            device_name,
        }
    }

    pub async fn get_events(
        &self,
        connection: &mut GoogleConnection,
        end_time: DateTime<Utc>,
        duration_minutes: i64,
    ) -> Result<Vec<CameraEvent>> {
        let start_time = end_time - Duration::minutes(duration_minutes);

        let start_str = format_datetime_for_api(&start_time);
        let end_str = format_datetime_for_api(&end_time);

        let params = [
            ("start_time", start_str),
            ("end_time", end_str),
            ("types", "4".to_string()),
            ("variant", "2".to_string()),
        ];

        let xml_data = connection
            .make_nest_get_request(&self.device_id, EVENTS_URI, &params)
            .await?;

        self.parse_events(&xml_data)
    }

    fn parse_events(&self, xml_data: &[u8]) -> Result<Vec<CameraEvent>> {
        let mut reader = Reader::from_reader(xml_data);
        reader.config_mut().trim_text(true);
        let mut events = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    if e.name().as_ref() == b"Period" {
                        let mut program_date_time = None;
                        let mut duration = None;

                        for attr in e.attributes().flatten() {
                            let key = attr.key.as_ref();
                            let value = String::from_utf8_lossy(&attr.value).to_string();

                            if key == b"programDateTime" {
                                program_date_time = Some(value);
                            } else if key == b"duration" {
                                duration = Some(value);
                            }
                        }

                        if let (Some(pdt), Some(dur)) = (program_date_time, duration)
                            && let Ok(event) =
                                CameraEvent::from_xml_attributes(self.device_id.clone(), &pdt, &dur)
                        {
                            events.push(event);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(anyhow::anyhow!("XML parsing error: {}", e)),
                _ => {}
            }
            buf.clear();
        }

        Ok(events)
    }

    pub async fn download_camera_event(
        &self,
        connection: &mut GoogleConnection,
        event: &CameraEvent,
    ) -> Result<Vec<u8>> {
        let start_ms = event.start_time.timestamp_millis();
        let end_ms = event.end_time().timestamp_millis();

        let params = [
            ("start_time", start_ms.to_string()),
            ("end_time", end_ms.to_string()),
        ];

        connection
            .make_nest_get_request(&self.device_id, DOWNLOAD_VIDEO_URI, &params)
            .await
    }
}

fn format_datetime_for_api(dt: &DateTime<Utc>) -> String {
    let formatted = dt.format("%Y-%m-%dT%H:%M:%S").to_string();
    format!("{}.000Z", formatted)
}
