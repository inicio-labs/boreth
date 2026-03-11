//! Bor RPC extensions.

pub mod api;
pub mod methods;
pub mod types;

pub use api::BorApi;
pub use methods::{BorRpcError, compute_root_hash, get_author};
pub use types::{BorReceiptResponse, BorSnapshotResponse, CurrentValidatorsResponse, ValidatorInfo};
