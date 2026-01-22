pub trait AsyncOption<T> {
    fn map_async<B, F, Fut>(self, f: F) -> impl Future<Output = Option<B>>
    where
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = B>;

    fn or_else_async<F, Fut>(self, f: F) -> impl Future<Output = Option<T>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<T>>;
}

impl<T> AsyncOption<T> for Option<T> {
    async fn map_async<B, F, Fut>(self, f: F) -> Option<B>
    where
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = B>,
    {
        if let Some(this) = self {
            Some(f(this).await)
        } else {
            None
        }
    }

    async fn or_else_async<F, Fut>(self, f: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<T>>,
    {
        match self {
            Some(this) => Some(this),
            None => f().await,
        }
    }
}
