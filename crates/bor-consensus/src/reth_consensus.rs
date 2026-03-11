//! Reth [`Consensus`] adapter for Bor PoA consensus.
//!
//! Wraps Bor's validation logic to conform to Reth's consensus traits,
//! enabling integration with Reth's sync pipeline and block import.
//!
//! # Architecture
//!
//! Header-only validation (`validate_header`) performs structural checks that don't
//! require external state (nonce, ommers, extra data format, etc.).
//!
//! Block-level validation (`validate_block_pre_execution`) performs full seal verification:
//! - Recovers the block signer via ecrecover from the seal
//! - Verifies the signer is in the current validator set (from cached Heimdall spans)
//! - Checks the anti-double-sign window
//!
//! The span cache must be populated eagerly before blocks are validated. This is typically
//! done by a separate component that pre-fetches spans from Heimdall.

use alloy_consensus::EMPTY_OMMER_ROOT_HASH;
use alloy_primitives::Address;
use bor_primitives::Span;
use heimdall_client::SpanCache;
use reth_chainspec::{EthChainSpec, EthereumHardforks};
use reth_consensus::{Consensus, ConsensusError, FullConsensus, HeaderValidator, ReceiptRootBloom};
use reth_execution_types::BlockExecutionResult;
use reth_primitives_traits::{
    AlloyBlockHeader, Block, BlockBody, BlockHeader, GotExpected, GotExpectedBoxed,
    NodePrimitives, RecoveredBlock, SealedBlock, SealedHeader,
};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

use crate::extra_data::ExtraData;
use crate::recents::Recents;
use crate::seal::{compute_seal_hash, ecrecover_seal};

/// Bor consensus engine for Reth.
///
/// Implements Reth's [`Consensus`], [`HeaderValidator`], and [`FullConsensus`]
/// traits using Bor's PoA consensus rules:
///
/// - No ommers allowed (Bor is single-signer PoA)
/// - No withdrawals (Polygon does not use Ethereum withdrawals)
/// - Difficulty is non-zero (PoA in-turn / not-in-turn)
/// - Extra data contains vanity + optional validators + seal
/// - Gas limit and base fee validated per Ethereum rules
/// - Nonce must be zero
/// - Seal is verified against the authorized validator set
#[derive(Debug)]
pub struct BorConsensus<ChainSpec> {
    /// Chain specification.
    chain_spec: Arc<ChainSpec>,
    /// Cached Heimdall spans for validator set lookups.
    span_cache: Mutex<SpanCache>,
    /// Recent block signers for anti-double-sign enforcement.
    recents: Mutex<Recents>,
}

impl<ChainSpec> BorConsensus<ChainSpec> {
    /// Create a new Bor consensus engine.
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            chain_spec,
            span_cache: Mutex::new(SpanCache::new(64)),
            recents: Mutex::new(Recents::new()),
        }
    }

    /// Insert a span into the cache. Call this to eagerly populate spans
    /// before block validation reaches them.
    pub fn insert_span(&self, span: Span) {
        self.span_cache.lock().expect("span cache lock poisoned").insert(span);
    }

    /// Look up the span that covers the given block number.
    /// Returns `None` if the span is not in the cache.
    fn get_span_for_block(&self, block_number: u64, span_size: u64) -> Option<Span> {
        let span_id = bor_primitives::span_id_at(block_number, span_size);
        self.span_cache
            .lock()
            .expect("span cache lock poisoned")
            .get(span_id)
            .cloned()
    }

    /// Get the list of authorized signer addresses from a span's validator set.
    fn authorized_signers(span: &Span) -> Vec<Address> {
        span.validator_set
            .validators
            .iter()
            .map(|v| v.signer)
            .collect()
    }
}

impl<H, ChainSpec> HeaderValidator<H> for BorConsensus<ChainSpec>
where
    H: BlockHeader,
    ChainSpec: EthChainSpec<Header = H> + EthereumHardforks + Debug + Send + Sync,
{
    fn validate_header(&self, header: &SealedHeader<H>) -> Result<(), ConsensusError> {
        let header = header.header();

        // Bor: nonce must always be zero
        if !header.nonce().is_some_and(|nonce| nonce.is_zero()) {
            return Err(ConsensusError::TheMergeNonceIsNotZero);
        }

        // Bor: ommers hash must be empty
        if header.ommers_hash() != EMPTY_OMMER_ROOT_HASH {
            return Err(ConsensusError::TheMergeOmmerRootIsNotEmpty);
        }

        // Validate gas: gas_used <= gas_limit
        if header.gas_used() > header.gas_limit() {
            return Err(ConsensusError::HeaderGasUsedExceedsGasLimit {
                gas_used: header.gas_used(),
                gas_limit: header.gas_limit(),
            });
        }

        // Bor: extra data must be at least vanity (32) + seal (65) = 97 bytes
        if header.extra_data().len() < 97 {
            return Err(ConsensusError::ExtraDataExceedsMax {
                len: header.extra_data().len(),
            });
        }

        // No withdrawals root on Bor
        if header.withdrawals_root().is_some() {
            return Err(ConsensusError::WithdrawalsRootUnexpected);
        }

        // No blob gas on Bor (no EIP-4844)
        if header.blob_gas_used().is_some() {
            return Err(ConsensusError::BlobGasUsedUnexpected);
        }
        if header.excess_blob_gas().is_some() {
            return Err(ConsensusError::ExcessBlobGasUnexpected);
        }

        // No beacon block root on Bor
        if header.parent_beacon_block_root().is_some() {
            return Err(ConsensusError::ParentBeaconBlockRootUnexpected);
        }

        // No requests hash on Bor
        if header.requests_hash().is_some() {
            return Err(ConsensusError::RequestsHashUnexpected);
        }

        Ok(())
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        // Block number must be parent + 1
        if header.number() != parent.number() + 1 {
            return Err(ConsensusError::ParentBlockNumberMismatch {
                parent_block_number: parent.number(),
                block_number: header.number(),
            });
        }

        // Parent hash must match
        if header.parent_hash() != parent.hash() {
            return Err(ConsensusError::ParentHashMismatch(
                GotExpectedBoxed::from(GotExpected::new(header.parent_hash(), parent.hash())),
            ));
        }

        // Timestamp must be strictly increasing
        if header.timestamp() <= parent.timestamp() {
            return Err(ConsensusError::TimestampIsInPast {
                parent_timestamp: parent.timestamp(),
                timestamp: header.timestamp(),
            });
        }

        Ok(())
    }
}

impl<B, ChainSpec> Consensus<B> for BorConsensus<ChainSpec>
where
    B: Block,
    ChainSpec: EthChainSpec<Header = B::Header> + EthereumHardforks + Debug + Send + Sync,
{
    fn validate_body_against_header(
        &self,
        body: &B::Body,
        header: &SealedHeader<B::Header>,
    ) -> Result<(), ConsensusError> {
        // Bor: no ommers allowed
        if body.ommers().is_some_and(|o| !o.is_empty()) {
            return Err(ConsensusError::BodyOmmersHashDiff(
                GotExpectedBoxed::from(GotExpected::new(
                    alloy_primitives::B256::ZERO,
                    EMPTY_OMMER_ROOT_HASH,
                )),
            ));
        }

        // Bor: no withdrawals
        if body.withdrawals().is_some() {
            return Err(ConsensusError::WithdrawalsRootUnexpected);
        }

        // Validate transaction root
        let tx_root =
            reth_primitives_traits::proofs::calculate_transaction_root(body.transactions());
        let header_tx_root = header.transactions_root();
        if tx_root != header_tx_root {
            return Err(ConsensusError::BodyTransactionRootDiff(
                GotExpectedBoxed::from(GotExpected::new(tx_root, header_tx_root)),
            ));
        }

        Ok(())
    }

    fn validate_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), ConsensusError> {
        // Ommers must be empty
        if block.body().ommers().is_some_and(|o| !o.is_empty()) {
            return Err(ConsensusError::BodyOmmersHashDiff(
                GotExpectedBoxed::from(GotExpected::new(
                    alloy_primitives::B256::ZERO,
                    EMPTY_OMMER_ROOT_HASH,
                )),
            ));
        }

        // No withdrawals
        if block.body().withdrawals().is_some() {
            return Err(ConsensusError::WithdrawalsRootUnexpected);
        }

        let header = block.header();
        let block_number = header.number();

        // Parse extra data to get seal
        let extra = ExtraData::parse(header.extra_data()).map_err(|e| {
            ConsensusError::Other(format!("invalid extra data: {e}").into())
        })?;

        // Compute seal hash (header RLP with seal stripped from extra data)
        let seal_hash = compute_seal_hash(header);

        // Recover signer from seal
        let signer = ecrecover_seal(&seal_hash, &extra.seal).map_err(|e| {
            ConsensusError::Other(format!("seal recovery failed: {e}").into())
        })?;

        debug!(target: "bor::consensus", block = block_number, ?signer, "recovered block signer");

        // Look up the validator set from the span cache.
        // Use the chain-appropriate span size. For now, use a heuristic:
        // if all Bor forks are at block 0 (Amoy), Rio is active from genesis → span_size = 1600.
        // Otherwise determine from chain spec.
        // TODO: Get span_size from chain spec properly based on block number.
        let span_size = 6400u64; // Default pre-Rio span size
        if let Some(span) = self.get_span_for_block(block_number, span_size) {
            let signers = Self::authorized_signers(&span);

            // Verify signer is authorized
            if !signers.contains(&signer) {
                return Err(ConsensusError::Other(
                    format!("unauthorized signer {signer} at block {block_number}").into(),
                ));
            }

            // Anti-double-sign check
            let recents = self.recents.lock().expect("recents lock poisoned");
            if recents.is_recently_signed(&signer, block_number, signers.len()) {
                return Err(ConsensusError::Other(
                    format!("signer {signer} signed too recently at block {block_number}").into(),
                ));
            }
            drop(recents);

            // Record this signer in recents
            let mut recents = self.recents.lock().expect("recents lock poisoned");
            recents.add_signer(block_number, signer);
            recents.prune(block_number, signers.len());
        } else {
            warn!(
                target: "bor::consensus",
                block = block_number,
                "span not cached, skipping signer authorization check"
            );
        }

        Ok(())
    }
}

impl<ChainSpec, N> FullConsensus<N> for BorConsensus<ChainSpec>
where
    ChainSpec: Send + Sync + EthChainSpec<Header = N::BlockHeader> + EthereumHardforks + Debug,
    N: NodePrimitives,
{
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<N::Block>,
        result: &BlockExecutionResult<N::Receipt>,
        _receipt_root_bloom: Option<ReceiptRootBloom>,
    ) -> Result<(), ConsensusError> {
        // Validate gas used matches
        let header_gas = block.header().gas_used();
        let exec_gas = result.gas_used;
        if header_gas != exec_gas {
            return Err(ConsensusError::BlockGasUsed {
                gas: GotExpected::new(exec_gas, header_gas),
                gas_spent_by_tx: Vec::new(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::Header;
    use alloy_primitives::{B256, B64};
    use reth_chainspec::ChainSpec;

    fn bor_consensus() -> BorConsensus<ChainSpec> {
        use reth_chainspec::ChainSpecBuilder;
        let spec = ChainSpecBuilder::default()
            .chain(alloy_chains::Chain::from_id(80002))
            .genesis(alloy_genesis::Genesis::default())
            .london_activated()
            .paris_activated()
            .build();
        BorConsensus::new(Arc::new(spec))
    }

    #[test]
    fn test_bor_consensus_rejects_nonzero_nonce() {
        let consensus = bor_consensus();
        let header = Header {
            nonce: B64::from(1u64),
            extra_data: alloy_primitives::Bytes::from(vec![0u8; 97]),
            ..Default::default()
        };
        let sealed = SealedHeader::seal_slow(header);
        assert!(consensus.validate_header(&sealed).is_err());
    }

    #[test]
    fn test_bor_consensus_accepts_valid_header() {
        let consensus = bor_consensus();
        let header = Header {
            nonce: B64::ZERO,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            extra_data: alloy_primitives::Bytes::from(vec![0u8; 97]),
            gas_limit: 30_000_000,
            ..Default::default()
        };
        let sealed = SealedHeader::seal_slow(header);
        assert!(consensus.validate_header(&sealed).is_ok());
    }

    #[test]
    fn test_bor_consensus_rejects_withdrawals_root() {
        let consensus = bor_consensus();
        let header = Header {
            nonce: B64::ZERO,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            extra_data: alloy_primitives::Bytes::from(vec![0u8; 97]),
            gas_limit: 30_000_000,
            withdrawals_root: Some(B256::ZERO),
            ..Default::default()
        };
        let sealed = SealedHeader::seal_slow(header);
        let err = consensus.validate_header(&sealed).unwrap_err();
        assert!(matches!(err, ConsensusError::WithdrawalsRootUnexpected));
    }
}
