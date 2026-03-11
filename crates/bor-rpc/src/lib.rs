//! Bor RPC extensions.

pub mod api;
pub mod types;

pub use api::BorApi;
pub use types::{BorSnapshotResponse, CurrentValidatorsResponse, ValidatorInfo};
