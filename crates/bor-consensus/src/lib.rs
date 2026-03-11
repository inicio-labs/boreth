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
pub use seal::{ecrecover_seal, SealError};
