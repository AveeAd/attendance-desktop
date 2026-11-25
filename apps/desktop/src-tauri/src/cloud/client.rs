use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::cloud::config::Credentials;
use crate::error::{AppError, AppResult};

pub struct CloudClient {
    http: Client,
    base_url: String,
    api_key: String,
    device_id: String,
}

impl CloudClient {
    pub fn new(credentials: Credentials) -> AppResult<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| AppError::Cloud(format!("Cannot build HTTP client: {error}")))?;
        Ok(Self {
            http,
            base_url: credentials.api_url,
            api_key: credentials.api_key,
            device_id: credentials.device_id,
        })
    }

    fn build(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-Device-Id", &self.device_id)
    }

    pub fn request<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&B>,
        base_version: Option<&str>,
    ) -> AppResult<(u16, Option<Value>)> {
        let mut request = self.build(method, path);
        for (key, value) in query {
            request = request.query(&[(key, value)]);
        }
        if let Some(version) = base_version {
            if !version.is_empty() {
                request = request.header("X-Base-Version", version);
            }
        }
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request
            .send()
            .map_err(|error| AppError::Cloud(format!("Cannot reach server: {error}")))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .map_err(|error| AppError::Cloud(format!("Cannot read server response: {error}")))?;
        let value = if text.trim().is_empty() {
            None
        } else {
            serde_json::from_str::<Value>(&text).ok()
        };

        if status >= 400 {
            let message = value
                .as_ref()
                .and_then(|value| value.get("error"))
                .and_then(Value::as_str)
                .unwrap_or(text.trim())
                .to_string();
            if status == 409 {
                let detail = value
                    .as_ref()
                    .and_then(|value| value.get("current"))
                    .map(|current| current.to_string());
                let message = match detail {
                    Some(detail) if !detail.is_empty() => {
                        format!("{message} · server has {detail}")
                    }
                    _ => message,
                };
                return Err(AppError::CloudConflict { message });
            }
            return Err(AppError::CloudHttp { status, message });
        }

        Ok((status, value))
    }

    pub fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> AppResult<T> {
        let (_, value) = self.request::<()>(Method::GET, path, query, None, None)?;
        serde_json::from_value(value.unwrap_or(Value::Null))
            .map_err(|error| AppError::Cloud(format!("Unexpected server response: {error}")))
    }

    pub fn send_json<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
        base_version: Option<&str>,
    ) -> AppResult<T> {
        let (_, value) = self.request(method, path, &[], Some(body), base_version)?;
        serde_json::from_value(value.unwrap_or(Value::Null))
            .map_err(|error| AppError::Cloud(format!("Unexpected server response: {error}")))
    }
}
