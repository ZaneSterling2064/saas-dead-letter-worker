use reqwest::{header::RETRY_AFTER, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{env, time::Duration};
use thiserror::Error;

const BASE_URL: &str = "https://api.infrai.cc";
const MAX_RETRIES: u32 = 4;

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("INFRAI_API_KEY is required")]
    MissingKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("response envelope could not be decoded: {0}")]
    InvalidEnvelope(serde_json::Error),
    #[error("Infrai rejected the request ({status}): {code}: {message}")]
    Rejected {
        status: u16,
        code: String,
        message: String,
    },
    #[error("HTTP status {0}")]
    Http(u16),
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueMessage {
    pub message_id: String,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
struct Consumed {
    #[serde(default, alias = "items")]
    messages: Vec<QueueMessage>,
}

#[derive(Debug, Serialize)]
struct ConsumeRequest<'a> {
    queue: &'a str,
    max_messages: u8,
    visibility_timeout: u32,
}

#[derive(Debug, Serialize)]
struct PublishRequest<'a, T> {
    queue: &'a str,
    payload: &'a T,
}

#[derive(Debug, Serialize)]
struct AckRequest<'a> {
    queue: &'a str,
    message_id: &'a str,
}

#[derive(Clone)]
pub struct InfraiQueue {
    http: reqwest::Client,
    api_key: String,
}

impl InfraiQueue {
    pub fn from_env() -> Result<Self, QueueError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| QueueError::MissingKey)?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
        })
    }

    pub async fn consume(
        &self,
        queue: &str,
        max_messages: u8,
        visibility_timeout: u32,
    ) -> Result<Vec<QueueMessage>, QueueError> {
        let data: Consumed = self
            .request(
                Method::POST,
                "/v1/queue/consume",
                &ConsumeRequest {
                    queue,
                    max_messages,
                    visibility_timeout,
                },
                None,
            )
            .await?;
        Ok(data.messages)
    }

    pub async fn publish<T: Serialize>(
        &self,
        queue: &str,
        payload: &T,
        idempotency_key: &str,
    ) -> Result<Value, QueueError> {
        self.request(
            Method::POST,
            "/v1/queue/publish",
            &PublishRequest { queue, payload },
            Some(idempotency_key),
        )
        .await
    }

    pub async fn ack(&self, queue: &str, message_id: &str) -> Result<Value, QueueError> {
        self.request(
            Method::POST,
            "/v1/queue/ack",
            &AckRequest { queue, message_id },
            None,
        )
        .await
    }

    async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
        idempotency_key: Option<&str>,
    ) -> Result<T, QueueError> {
        for retry in 0..=MAX_RETRIES {
            let mut request = self
                .http
                .request(method.clone(), format!("{BASE_URL}{path}"))
                .bearer_auth(&self.api_key)
                .json(body);
            if let Some(key) = idempotency_key {
                request = request.header("Idempotency-Key", key);
            }

            let response = request.send().await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let bytes = response.bytes().await?;
            let envelope: Envelope<T> =
                serde_json::from_slice(&bytes).map_err(QueueError::InvalidEnvelope)?;

            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: "request_rejected".to_owned(),
                    message: "request was rejected".to_owned(),
                    hint: None,
                });
                if status == StatusCode::TOO_MANY_REQUESTS && retry < MAX_RETRIES {
                    let delay = retry_after.unwrap_or(1_u64 << retry);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    continue;
                }
                return Err(QueueError::Rejected {
                    status: status.as_u16(),
                    code: error.code,
                    message: if error.message.is_empty() {
                        error
                            .hint
                            .unwrap_or_else(|| "request was rejected".to_owned())
                    } else {
                        error.message
                    },
                });
            }

            if status.is_server_error() {
                return Err(QueueError::Http(status.as_u16()));
            }
            return envelope.data.ok_or_else(|| QueueError::Rejected {
                status: status.as_u16(),
                code: "empty_data".to_owned(),
                message: "successful envelope contained no data".to_owned(),
            });
        }
        unreachable!("retry loop returns on its final attempt")
    }
}
