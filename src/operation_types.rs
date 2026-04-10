//! Shared wire types for the async ops facility and [`crate::event::AppEvent`].

/// Monotonic identifier for an operation request and its completion events.
pub type OperationId = u64;
