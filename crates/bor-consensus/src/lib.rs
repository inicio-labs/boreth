//! Bor consensus engine implementation.

pub mod difficulty;
pub use difficulty::{calculate_difficulty, is_inturn};

pub mod extra_data;
pub use extra_data::ExtraData;

pub mod proposer;

pub mod recents;
pub use recents::Recents;

pub mod snapshot;
pub use snapshot::BorSnapshot;

pub mod seal;
pub use seal::{compute_seal_hash, ecrecover_seal, SealError};

pub mod block_validation;
pub use block_validation::{validate_block_pre_execution, validate_block_post_execution};

pub mod validation;
pub use validation::{
    HeaderValidationParams, ParentValidationParams, ValidationError,
    validate_header, validate_header_against_parent,
};

pub mod reth_consensus;
pub use reth_consensus::BorConsensus;
