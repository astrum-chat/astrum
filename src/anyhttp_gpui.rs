use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use anyhttp::{HttpClient, HttpResponse, Response};
use bytes::Bytes;
use futures::AsyncReadExt;
use gpui::http_client::AsyncBody;
use http::{Request, StatusCode};
use url::Url;

type GpuiHttpClient = Arc<dyn gpui::http_client::HttpClient>;
type GpuiHttpResponse = gpui::http_client::Response<AsyncBody>;

pub struct GpuiHttpWrapper {
    inner: GpuiHttpClient,
}

impl GpuiHttpWrapper {
    pub fn new(client: GpuiHttpClient) -> GpuiHttpWrapper {
        Self { inner: client }
    }
}

#[async_trait::async_trait]
impl HttpClient for GpuiHttpWrapper {
    async fn execute(
        &self,
        request: Request<Vec<u8>>,
    ) -> std::result::Result<Response, anyhow::Error> {
        let request = request.map(|this| AsyncBody::from_bytes(this.into()));
        let response = self.inner.send(request).await?;

        Ok(Response::new(GpuiHttpResponseWrapper::new(response)))
    }
}

impl Into<GpuiHttpWrapper> for GpuiHttpClient {
    fn into(self) -> GpuiHttpWrapper {
        GpuiHttpWrapper::new(self)
    }
}

impl Deref for GpuiHttpWrapper {
    type Target = GpuiHttpClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for GpuiHttpWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub struct GpuiHttpResponseWrapper {
    inner: gpui::http_client::Response<AsyncBody>,
}

impl GpuiHttpResponseWrapper {
    pub fn new(response: GpuiHttpResponse) -> GpuiHttpResponseWrapper {
        Self { inner: response }
    }
}

#[async_trait::async_trait]
impl HttpResponse for GpuiHttpResponseWrapper {
    async fn bytes(self: Box<Self>) -> anyhow::Result<Bytes> {
        let mut buf = Vec::new();
        self.inner.into_body().read_to_end(&mut buf).await?;
        Ok(Bytes::from(buf))
    }

    //#[cfg(feature = "stream")]
    fn bytes_stream(
        self: Box<Self>,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = anyhow::Result<Bytes>> + Send>> {
        use bytes::Bytes;
        use futures::Stream;
        use http_body::Body;
        use std::{
            pin::Pin,
            task::{Context, Poll},
        };

        struct BodyStream {
            body: AsyncBody,
        }

        impl Stream for BodyStream {
            type Item = anyhow::Result<Bytes>;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let mut pinned = Pin::new(&mut self.body);

                match pinned.as_mut().poll_frame(cx) {
                    Poll::Ready(Some(Ok(frame))) => {
                        if let Ok(data) = frame.into_data() {
                            Poll::Ready(Some(Ok(data)))
                        } else {
                            // A frame with no data is rare but valid
                            Poll::Ready(Some(Ok(Bytes::new())))
                        }
                    }
                    Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(anyhow::Error::new(e)))),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                }
            }
        }

        Box::pin(BodyStream {
            body: self.inner.into_body(),
        })
    }

    fn url(&self) -> &Url {
        todo!()
    }

    fn status(&self) -> StatusCode {
        self.inner.status()
    }
}

impl Into<GpuiHttpResponseWrapper> for GpuiHttpResponse {
    fn into(self) -> GpuiHttpResponseWrapper {
        GpuiHttpResponseWrapper::new(self)
    }
}

impl Deref for GpuiHttpResponseWrapper {
    type Target = GpuiHttpResponse;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for GpuiHttpResponseWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
