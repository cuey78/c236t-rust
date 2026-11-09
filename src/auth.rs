/*
 * Inkbird Smart Center API Authentication
 * 
 * Reverse-engineered from INKBIRD APK (v2.1.18):
 *   - Base URL: https://api-inkbird.com/api/ (or https://api-smartcenter.ink-bird.net/)
 *   - Login endpoint: POST smartAgent/user/login
 *   - API Headers: API-KEY: 8V073Jrc4H, API-SECRET-KEY: 9HbzOuNhQlkESVW7PqxQ
 *   - LoginBean: username, password, countryCode, deviceType=2, registerType=1
 *
 * Source: com/inkbird/inkbirdapp/login/api/LoginApi.java
 *         com/inkbird/inkbirdapp/login/bean/LoginBean.java
 *         com/inkbird/base/http/utils/WifiUtils.java
 */

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

/// Inkbird API base URLs by region
pub const BASE_URL_GLOBAL: &str = "https://api-inkbird.com/api/";
pub const BASE_URL_EU: &str = "https://eu.api-inkbird.com/api/";
pub const BASE_URL_US: &str = "https://us.api-inkbird.com/api/";
pub const BASE_URL_SMARTCENTER: &str = "https://api-smartcenter.ink-bird.net/";
pub const BASE_URL_GLOBAL_ALT: &str = "https://global.api-inkbird.com/api/";

/// API keys (from LoginApi.java headers)
pub const API_KEY: &str = "8V073Jrc4H";
pub const API_SECRET_KEY: &str = "9HbzOuNhQlkESVW7PqxQ";

/// Login request body (matches LoginBean)
#[derive(Debug, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(rename = "countryCode")]
    pub country_code: String,
    #[serde(rename = "deviceType")]
    pub device_type: i32,
    #[serde(rename = "registerType")]
    pub register_type: i32,
    #[serde(rename = "thirdUid")]
    pub third_uid: Option<String>,
    #[serde(rename = "originalThirdUid")]
    pub original_third_uid: Option<String>,
}

/// Login response — matches actual Inkbird API: {"code": 200, "message": "...", "data": {...}}
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    #[serde(deserialize_with = "crate::auth::deserialize_optional_string_or_int")]
    pub code: Option<String>,
    pub message: Option<String>,
    pub data: Option<LoginResult>,
}

#[derive(Debug, Deserialize)]
pub struct LoginResult {
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
    #[serde(rename = "expireDate")]
    pub expire_date: String,
    pub user: LoginUser,
}

#[derive(Debug, Deserialize)]
pub struct LoginUser {
    pub uid: String,
    pub username: Option<String>,
    pub country_code: Option<String>,
}

/// Device info from device list
#[derive(Debug, Deserialize, Clone)]
pub struct DeviceInfo {
    #[serde(default, deserialize_with = "crate::auth::deserialize_string_or_int")]
    pub dev_id: String,
    #[serde(default, deserialize_with = "crate::auth::deserialize_string_or_int")]
    pub product_id: String,
    pub name: String,
    #[serde(default, deserialize_with = "crate::auth::deserialize_bool_or_int")]
    pub online: bool,
    pub local_key: Option<String>,
}

/// Device list response — matches Inkbird API pattern: {"code": 200, "message": "...", "data": [...]}
#[derive(Debug, Deserialize)]
pub struct DeviceListResponse {
    #[serde(deserialize_with = "crate::auth::deserialize_optional_string_or_int")]
    pub code: Option<String>,
    pub message: Option<String>,
    pub data: Option<Vec<DeviceInfo>>,
}

/// MQTT config response from ThingCloud
#[derive(Debug, Deserialize, Clone)]
pub struct MqttConfigResponse {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub client_id: String,
}

/// Inkbird authenticated API client
pub struct InkbirdApiClient {
    client: Client,
    base_url: String,
    access_token: Option<String>,
}

impl InkbirdApiClient {
    /// Create a new Inkbird API client for a specific region
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            access_token: None,
        }
    }

    /// Log in with username/password
    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
        country_code: &str,
    ) -> Result<LoginResponse, Box<dyn std::error::Error>> {
        let url = format!("{}smartAgent/user/login", self.base_url);

        let request = LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
            country_code: country_code.to_string(),
            device_type: 2,
            register_type: 1,
            third_uid: None,
            original_third_uid: None,
        };

        info!("Logging in to Inkbird API...");
        info!("  URL: {}", url);
        info!("  Username: {}", username);

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("API-KEY", API_KEY)
            .header("API-SECRET-KEY", API_SECRET_KEY)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body_text = response.text().await?;

        info!("Login response status: {}", status);
        info!("Login response body: {}", &body_text[..1000.min(body_text.len())]);

        let login_resp: LoginResponse = match serde_json::from_str(&body_text) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to parse login response: {}", e);
                error!("Raw response: {}", body_text);
                return Err(e.into());
            }
        };

        let is_success = login_resp.code.as_deref() == Some("200");
        if is_success {
            if let Some(ref result) = login_resp.data {
                self.access_token = Some(result.access_token.clone());
                info!("✓ Login successful! UID: {}", result.user.uid);
                info!(
                    "✓ Access token: {}... ({} chars)",
                    &result.access_token[..12.min(result.access_token.len())],
                    result.access_token.len()
                );
            } else {
                error!("✗ Login succeeded but no data returned");
                std::process::exit(1);
            }
        } else {
            error!("✗ Login failed: {:?} (code: {:?})", login_resp.message, login_resp.code);
            error!("  Try checking your username, password, or country code");
            std::process::exit(1);
        }

        Ok(login_resp)
    }

    /// Get device list for the logged-in user
    pub async fn get_device_list(&self) -> Result<DeviceListResponse, Box<dyn std::error::Error>> {
        let token = self.access_token.as_deref().ok_or("Not logged in")?;
        let url = format!("{}smartAgent/user/deviceList", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("API-KEY", API_KEY)
            .header("API-SECRET-KEY", API_SECRET_KEY)
            .header("TOKEN", token)
            .json(&serde_json::json!({}))
            .send()
            .await?;

        let body_text = response.text().await?;
        info!("Device list response: {}", &body_text[..1000.min(body_text.len())]);

        let device_list: DeviceListResponse = match serde_json::from_str(&body_text) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to parse device list: {}", e);
                error!("Raw: {}", body_text);
                return Err(e.into());
            }
        };

        if let Some(ref devices) = device_list.data {
            info!("Found {} devices", devices.len());
            for dev in devices {
                info!(
                    "  - {} ({}) product={} online={}",
                    dev.name, dev.dev_id, dev.product_id, dev.online
                );
            }
        }

        Ok(device_list)
    }

    /// Get the current access token
    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }
}

/// Find your C236T device from the list
pub fn find_c236t_device<'a>(
    devices: &'a [DeviceInfo],
    target_dev_id: Option<&'a str>,
) -> Option<&'a DeviceInfo> {
    if let Some(dev_id) = target_dev_id {
        return devices.iter().find(|d| d.dev_id == dev_id);
    }

    devices.iter().find(|d| {
        d.product_id == "ae6rmrnkqz7rjh66"
            || d.product_id == "fhxzbdcssszg1pc5"
            || d.product_id == "giwst7t3cnb7q7xz"
            || d.product_id == "6mbrvmttev3t0dei"
    })
}

/// Deserialize a field that might be a string or an integer
fn deserialize_string_or_int<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let val: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    Ok(match val {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        _ => val.to_string(),
    })
}

/// Deserialize an optional field that might be a string or an integer
fn deserialize_optional_string_or_int<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    Ok(val.map(|v| match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        _ => v.to_string(),
    }))
}

/// Deserialize a field that might be a bool or an integer (0/1)
fn deserialize_bool_or_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let val: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match val {
        serde_json::Value::Bool(b) => Ok(b),
        serde_json::Value::Number(n) => Ok(n.as_i64().unwrap_or(0) != 0),
        serde_json::Value::String(s) => Ok(s == "1" || s.eq_ignore_ascii_case("true")),
        _ => Err(Error::custom("expected bool or int")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_c236t_device() {
        let devices = vec![
            DeviceInfo {
                dev_id: "other_device".to_string(),
                product_id: "some_other_product".to_string(),
                name: "Other".to_string(),
                online: true,
                local_key: None,
            },
            DeviceInfo {
                dev_id: "example-device-id".to_string(),
                product_id: "ae6rmrnkqz7rjh66".to_string(),
                name: "C236T Thermostat".to_string(),
                online: true,
                local_key: None,
            },
        ];

        let found = find_c236t_device(&devices, Some("example-device-id"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "C236T Thermostat");
    }
}
