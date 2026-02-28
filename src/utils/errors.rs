use std::collections::VecDeque;

use gpui::{App, AsyncApp, Entity};
use tracing::error;

/// Push an error message to the global error queue.
/// Works from synchronous contexts with `&mut App`.
pub fn push_error(errors: &Entity<VecDeque<String>>, cx: &mut App, message: impl Into<String>) {
    let message = message.into();
    error!("{}", message);
    errors.update(cx, |errors, cx| {
        errors.push_back(message);
        cx.notify();
    });
}

/// Push an error message to the global error queue from an async context.
/// Silently drops if the app context is no longer available.
pub fn push_error_async(errors: &Entity<VecDeque<String>>, cx: &mut AsyncApp, message: impl Into<String>) {
    let message = message.into();
    error!("{}", message);
    let _ = errors.update(cx, |errors, cx| {
        errors.push_back(message);
        cx.notify();
    });
}
