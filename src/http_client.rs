//! Lightweight reqwest-based HTTP client

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::anyhow;
use futures::TryStreamExt;
use futures::future::BoxFuture;
use gpui::http_client::{AsyncBody, HttpClient, Response, Url};
use reqwest::header::{HeaderMap, HeaderValue};

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// A simple HTTP client implementation using reqwest
///
/// Bridges between GPUI's http client interface and reqwest,
/// handling the runtime differences between GPUI's async model and tokio.
pub struct ReqwestHttpClient {
    client: reqwest::Client,
    handle: tokio::runtime::Handle,
    user_agent: Option<HeaderValue>,
}

impl ReqwestHttpClient {
    /// Create a new HTTP client with default settings
    ///
    /// Uses "ace-desktop" as the default user agent.
    pub fn new() -> anyhow::Result<Self> {
        Self::with_user_agent("ace-desktop")
    }

    /// Create a new HTTP client with a custom user agent
    ///
    /// # Arguments
    ///
    /// * `agent` - The user agent string to use for all requests
    ///
    /// # Example
    ///
    /// ```no_run
    /// use http_client::ReqwestHttpClient;
    /// let client = ReqwestHttpClient::with_user_agent("my-app/1.0.0").unwrap();
    /// ```
    pub fn with_user_agent(agent: &str) -> anyhow::Result<Self> {
        let user_agent = HeaderValue::from_str(agent)?;

        let client = reqwest::Client::builder()
            .user_agent(agent)
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(60))
            .build()?;

        let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            // No tokio runtime found, create a minimal one for reqwest
            let runtime = RUNTIME.get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("Failed to initialize HTTP client runtime")
            });

            runtime.handle().clone()
        });

        Ok(Self {
            client,
            handle,
            user_agent: Some(user_agent),
        })
    }
}

impl HttpClient for ReqwestHttpClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        self.user_agent.as_ref()
    }

    fn send(
        &self,
        req: http::Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let (parts, body) = req.into_parts();

        // Convert the request to reqwest format
        let mut request = self.client.request(parts.method, parts.uri.to_string());

        // Set headers
        let mut headers = HeaderMap::new();
        for (name, value) in parts.headers.iter() {
            headers.insert(name.clone(), value.clone());
        }
        request = request.headers(headers);

        // Handle body based on its inner type
        let body_bytes = match body.0 {
            gpui::http_client::Inner::Empty => bytes::Bytes::new(),
            gpui::http_client::Inner::Bytes(cursor) => cursor.into_inner(),
            gpui::http_client::Inner::AsyncReader(mut reader) => {
                // For simplicity, we'll read the full body into memory
                // In a production implementation, you might want to stream this
                use smol::io::AsyncReadExt;
                let mut buffer = Vec::new();
                let handle = self.handle.clone();

                // We need to handle the async reader synchronously here
                // This is a limitation but acceptable for our use case
                let result = handle.block_on(async { reader.read_to_end(&mut buffer).await });

                match result {
                    Ok(_) => bytes::Bytes::from(buffer),
                    Err(_) => bytes::Bytes::new(),
                }
            }
        };
        request = request.body(body_bytes);

        let handle = self.handle.clone();

        Box::pin(async move {
            // Execute the request on the tokio runtime
            let response = handle.spawn(async move { request.send().await }).await??;

            // Convert response
            let status = response.status();
            let headers = response.headers().clone();

            // Stream the response body
            let stream = response.bytes_stream().map_err(std::io::Error::other);

            // Convert to AsyncRead
            let body_reader = stream.into_async_read();
            let async_body = AsyncBody::from_reader(body_reader);

            // Build the response
            let mut builder = http::Response::builder().status(status.as_u16());

            // Copy headers
            for (name, value) in headers.iter() {
                builder = builder.header(name.as_str(), value.as_bytes());
            }

            builder.body(async_body).map_err(|e| anyhow!(e))
        })
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::http_client::AsyncBody;

    #[test]
    fn test_client_creation() {
        let client = ReqwestHttpClient::new();
        assert!(client.is_ok());

        let _client = client.unwrap();
    }

    #[test]
    fn test_client_with_user_agent() {
        let client = ReqwestHttpClient::with_user_agent("test-agent/1.0");
        assert!(client.is_ok());

        let client = client.unwrap();
        let user_agent = client.user_agent();
        assert!(user_agent.is_some());
        assert_eq!(user_agent.unwrap().to_str().unwrap(), "test-agent/1.0");
    }

    #[test]
    fn test_proxy_returns_none() {
        let client = ReqwestHttpClient::new().unwrap();
        assert!(client.proxy().is_none());
    }

    #[tokio::test]
    async fn test_send_request() {
        // This test requires a mock server or network access
        // For now, we just verify the method exists and returns a future
        let client = ReqwestHttpClient::new().unwrap();

        let request = http::Request::builder()
            .method("GET")
            .uri("https://httpbin.org/get")
            .body(AsyncBody::empty())
            .unwrap();

        // Just verify the send method can be called
        // In a real test, you'd want to use a mock server
        let _future = client.send(request);
    }
}
