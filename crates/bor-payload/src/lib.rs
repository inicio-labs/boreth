//! Bor payload builder.
//!
//! Constructs block payloads for Bor, including user transactions and
//! system transactions (commitSpan, onStateReceive) at appropriate boundaries.

pub mod builder;
pub use builder::{BorPayloadBuilder, PayloadConfig, BuiltPayload};
