use anyhow::{Context, Result};
use reqwest::Client;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tonic::Request;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, ClientTlsConfig};

pub mod foyer {
    tonic::include_proto!("google.internal.home.foyer.v1");
}

use foyer::structures_service_client::StructuresServiceClient;
use foyer::{GetHomeGraphRequest, GetHomeGraphResponse};

const ACCESS_TOKEN_DURATION: Duration = Duration::from_secs(3600);
const GOOGLE_HOME_FOYER_API: &str = "https://googlehomefoyer-pa.googleapis.com";
const AUTH_URL: &str = "https://android.clients.google.com/auth";
const USER_AGENT: &str = "GoogleAuth/1.4";
const ACCESS_TOKEN_APP_NAME: &str = "com.google.android.apps.chromecast.app";
const ACCESS_TOKEN_CLIENT_SIGNATURE: &str = "24bb24c05e47e0aefa68a58a766179d9b613a600";
const ACCESS_TOKEN_SERVICE: &str = "oauth2:https://www.google.com/accounts/OAuthLogin";
const NEST_SCOPE: &str = "oauth2:https://www.googleapis.com/auth/nest-account";

pub struct GoogleConnection {
    client: Client,
    master_token: String,
    username: String,
    android_id: String,
    access_token: Option<String>,
    access_token_date: Option<SystemTime>,
    nest_access_token: Option<String>,
    nest_access_token_date: Option<SystemTime>,
    homegraph: Option<GetHomeGraphResponse>,
    homegraph_date: Option<SystemTime>,
}

impl GoogleConnection {
    pub fn new(master_token: String, username: String) -> Self {
        // Generate a random 16-character Android ID
        let android_id = format!("{:016x}", rand::random::<u64>());

        Self {
            client: Client::new(),
            master_token,
            username,
            android_id,
            access_token: None,
            access_token_date: None,
            nest_access_token: None,
            nest_access_token_date: None,
            homegraph: None,
            homegraph_date: None,
        }
    }

    async fn perform_oauth(&self, service: &str) -> Result<String> {
        let mut params = HashMap::new();
        params.insert("accountType", "HOSTED_OR_GOOGLE");
        params.insert("Email", self.username.as_str());
        params.insert("has_permission", "1");
        params.insert("EncryptedPasswd", self.master_token.as_str());
        params.insert("service", service);
        params.insert("source", "android");
        params.insert("androidId", self.android_id.as_str());
        params.insert("app", ACCESS_TOKEN_APP_NAME);
        params.insert("client_sig", ACCESS_TOKEN_CLIENT_SIGNATURE);
        params.insert("device_country", "us");
        params.insert("operatorCountry", "us");
        params.insert("lang", "en");
        params.insert("sdk_version", "17");
        params.insert("google_play_services_version", "240913000");

        let response = self
            .client
            .post(AUTH_URL)
            .header("Accept-Encoding", "identity")
            .header("Content-type", "application/x-www-form-urlencoded")
            .header("User-Agent", USER_AGENT)
            .form(&params)
            .send()
            .await
            .context("Failed to send OAuth request")?;

        let text = response
            .text()
            .await
            .context("Failed to read OAuth response")?;

        // Parse the response (format: key=value\nkey=value)
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("Auth=") {
                return Ok(value.to_string());
            }
        }

        Err(anyhow::anyhow!("No Auth token in OAuth response: {}", text))
    }

    async fn get_access_token(&mut self) -> Result<String> {
        let needs_refresh = match (self.access_token.as_ref(), self.access_token_date) {
            (Some(_), Some(date)) => {
                SystemTime::now()
                    .duration_since(date)
                    .unwrap_or(Duration::from_secs(0))
                    > ACCESS_TOKEN_DURATION
            }
            _ => true,
        };

        if needs_refresh {
            let token = self.perform_oauth(ACCESS_TOKEN_SERVICE).await?;
            self.access_token = Some(token.clone());
            self.access_token_date = Some(SystemTime::now());
            Ok(token)
        } else {
            Ok(self.access_token.as_ref().unwrap().clone())
        }
    }

    async fn get_nest_access_token(&mut self) -> Result<String> {
        let needs_refresh = match (self.nest_access_token.as_ref(), self.nest_access_token_date) {
            (Some(_), Some(date)) => {
                SystemTime::now()
                    .duration_since(date)
                    .unwrap_or(Duration::from_secs(0))
                    > ACCESS_TOKEN_DURATION
            }
            _ => true,
        };

        if needs_refresh {
            let token = self.perform_oauth(NEST_SCOPE).await?;
            self.nest_access_token = Some(token.clone());
            self.nest_access_token_date = Some(SystemTime::now());
            Ok(token)
        } else {
            Ok(self.nest_access_token.as_ref().unwrap().clone())
        }
    }

    async fn get_homegraph(&mut self) -> Result<GetHomeGraphResponse> {
        const HOMEGRAPH_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

        let needs_refresh = match (self.homegraph.as_ref(), self.homegraph_date) {
            (Some(_), Some(date)) => {
                SystemTime::now()
                    .duration_since(date)
                    .unwrap_or(Duration::from_secs(0))
                    > HOMEGRAPH_DURATION
            }
            _ => true,
        };

        if needs_refresh {
            let access_token = self.get_access_token().await?;

            let tls_config = ClientTlsConfig::new().with_native_roots();

            let channel = Channel::from_static(GOOGLE_HOME_FOYER_API)
                .tls_config(tls_config)?
                .connect()
                .await
                .context("Failed to connect to Google Home Foyer API")?;

            let token: MetadataValue<_> = format!("Bearer {}", access_token)
                .parse()
                .context("Failed to parse access token")?;

            let mut client =
                StructuresServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
                    req.metadata_mut().insert("authorization", token.clone());
                    Ok(req)
                });

            let request = Request::new(GetHomeGraphRequest {
                string1: String::new(),
                num2: String::new(),
            });

            let response = client
                .get_home_graph(request)
                .await
                .context("Failed to get home graph")?;

            self.homegraph = Some(response.into_inner());
            self.homegraph_date = Some(SystemTime::now());
        }

        Ok(self.homegraph.as_ref().unwrap().clone())
    }

    pub async fn make_nest_get_request(
        &mut self,
        device_id: &str,
        url: &str,
        params: &[(&str, String)],
    ) -> Result<Vec<u8>> {
        let url = url.replace("{device_id}", device_id);
        let access_token = self.get_nest_access_token().await?;

        let response = self
            .client
            .get(&url)
            .query(params)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("Failed to send request")?;

        let bytes = response
            .error_for_status()
            .context("Request returned error status")?
            .bytes()
            .await
            .context("Failed to read response body")?;

        Ok(bytes.to_vec())
    }

    pub async fn get_nest_camera_devices(&mut self) -> Result<Vec<(String, String)>> {
        let homegraph = self.get_homegraph().await?;

        let mut devices = Vec::new();

        if let Some(home) = homegraph.home {
            for device in home.devices {
                let has_camera_stream = device
                    .traits
                    .iter()
                    .any(|t| t == "action.devices.traits.CameraStream");

                let is_nest_device = device
                    .hardware
                    .as_ref()
                    .map(|h| h.model.contains("Nest"))
                    .unwrap_or(false);

                if has_camera_stream && is_nest_device {
                    let device_id = device
                        .device_info
                        .and_then(|di| di.agent_info)
                        .map(|ai| ai.unique_id)
                        .unwrap_or_default();

                    let device_name = device.device_name;

                    if !device_id.is_empty() {
                        devices.push((device_id, device_name));
                    }
                }
            }
        }

        Ok(devices)
    }
}
