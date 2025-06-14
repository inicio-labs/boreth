use std::sync::Arc;

use alloy_consensus::{BlockHeader, TxReceipt};
use alloy_primitives::{keccak256, BlockHash, Sealable};
use bor::params::BorParams;
use reth::{
    api::{FullNodeTypes, NodeTypes},
    builder::{components::ConsensusBuilder, rpc::EngineValidatorBuilder, BuilderContext},
    consensus::{Consensus, ConsensusError, FullConsensus, HeaderValidator},
};
use reth_chainspec::{ChainSpec, EthChainSpec, EthereumHardforks};
use reth_primitives::{
    gas_spent_by_transactions, EthPrimitives, GotExpected, NodePrimitives, RecoveredBlock,
    SealedBlock, SealedHeader,
};
use reth_primitives_traits::{Block, BlockBody};
use reth_provider::BlockExecutionResult;

use crate::consensus::constants::{
    EXTRA_SEAL_LENGTH, EXTRA_VANITY_LENGTH, VALIDATOR_HEADER_BYTES_LENGTH,
};

/// A basic ethereum consensus builder.
#[derive(Debug, Clone)]
pub struct BorConsensusBuilder {
    pub bor_params: Arc<BorParams>,
    // TODO add closure to modify consensus
}

impl<Node> ConsensusBuilder<Node> for BorConsensusBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
{
    type Consensus = Arc<dyn FullConsensus<EthPrimitives, Error = ConsensusError>>;

    async fn build_consensus(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Consensus> {
        Ok(Arc::new(BorConsensus::new(
            ctx.chain_spec(),
            self.bor_params,
        )))
    }
}

/// BSC consensus implementation.
///
/// Provides basic checks as outlined in the execution specs.
#[derive(Debug, Clone)]
pub struct BorConsensus<ChainSpec> {
    chain_spec: Arc<ChainSpec>,
    bor_params: Arc<BorParams>,
}

impl<ChainSpec> BorConsensus<ChainSpec> {
    /// Create a new instance of [`BorConsensus`]
    pub const fn new(chain_spec: Arc<ChainSpec>, bor_params: Arc<BorParams>) -> Self {
        Self {
            chain_spec,
            bor_params,
        }
    }

    fn validate_header_extra_field(&self, extra_bytes: &[u8]) -> Result<(), String> {
        if extra_bytes.len() < EXTRA_VANITY_LENGTH {
            return Err("missing vanity in extra data".to_string());
        }

        if extra_bytes.len() < EXTRA_VANITY_LENGTH + EXTRA_SEAL_LENGTH {
            return Err("missing signature in extra data".to_string());
        }

        Ok(())
    }

    fn get_validator_bytes(&self, header: &impl BlockHeader) -> &[u8] {
        // Extract validator bytes from header's extra data
        // This is a placeholder - implement according to your specific format
        todo!()
    }

    fn verify_cascading_fields(&self, header: &impl BlockHeader) -> Result<(), ConsensusError> {
        // TODO: Implement
        Ok(())
    }
}

impl<ChainSpec: EthChainSpec + EthereumHardforks, N: NodePrimitives> FullConsensus<N>
    for BorConsensus<ChainSpec>
{
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<N::Block>,
        result: &BlockExecutionResult<N::Receipt>,
    ) -> Result<(), ConsensusError> {
        // TODO: Add more checks if required, for now just added one check.
        // Check if gas used matches the value set in header.
        let cumulative_gas_used = result
            .receipts
            .last()
            .map(|receipt| receipt.cumulative_gas_used())
            .unwrap_or(0);
        if block.header().gas_used() != cumulative_gas_used {
            return Err(ConsensusError::BlockGasUsed {
                gas: GotExpected {
                    got: cumulative_gas_used,
                    expected: block.header().gas_used(),
                },
                gas_spent_by_tx: gas_spent_by_transactions(&result.receipts),
            });
        }

        Ok(())
    }
}

impl<ChainSpec: EthChainSpec + EthereumHardforks, B: Block> Consensus<B>
    for BorConsensus<ChainSpec>
{
    type Error = ConsensusError;

    fn validate_body_against_header(
        &self,
        body: &B::Body,
        header: &SealedHeader<B::Header>,
    ) -> Result<(), ConsensusError> {
        // validate_body_against_header(body, header.header())

        // TODO: Confirm it is correct to not allow ommers in bor blocks
        if body.ommers().is_some() {
            return Err(ConsensusError::Other(
                "ommers are not allowed in bor blocks".to_string(),
            ));
        }

        let tx_root = body.calculate_tx_root();
        if header.transactions_root() != tx_root {
            return Err(ConsensusError::BodyTransactionRootDiff(
                GotExpected {
                    got: tx_root,
                    expected: header.transactions_root(),
                }
                .into(),
            ));
        }
        Ok(())
    }

    fn validate_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), ConsensusError> {
        let header = block.header();
        let number = header.number();

        // Check if block is from the future
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if header.timestamp() > current_time {
            return Err(ConsensusError::Other(
                "block is from the future".to_string(),
            ));
        }

        // Validate extra data
        let extra_data = header.extra_data();
        if let Err(err) = self.validate_header_extra_field(extra_data) {
            return Err(ConsensusError::Other(err));
        }

        // Check sprint end
        let is_sprint_end = self.bor_params.bor_config.is_sprint_start(number + 1);

        // Validate signers in extra data
        let signers_bytes = self.get_validator_bytes(header).len();

        if !is_sprint_end && signers_bytes != 0 {
            return Err(ConsensusError::Other(
                "extra validators not allowed at non sprint end".to_string(),
            ));
        }

        if is_sprint_end && signers_bytes % VALIDATOR_HEADER_BYTES_LENGTH != 0 {
            return Err(ConsensusError::Other("invalid span validators".to_string()));
        }

        // Ensure mix digest is zero
        if header.mix_hash().is_some() {
            return Err(ConsensusError::Other("non zero mix digest".to_string()));
        }

        // TODO: confirm if this is correct
        // Ensure uncle hash is correct (empty in PoA)
        if header.ommers_hash() != alloy_primitives::keccak256(&[]) {
            return Err(ConsensusError::Other("invalid uncle hash".to_string()));
        }

        // Validate difficulty for non-genesis blocks
        if number > 0 && header.difficulty().is_zero() {
            return Err(ConsensusError::Other("invalid difficulty".to_string()));
        }

        // Verify gas limit
        let gas_cap = u64::MAX >> 1; // 2^63-1
        if header.gas_limit() > gas_cap {
            return Err(ConsensusError::Other(format!(
                "invalid gasLimit: have {}, max {}",
                header.gas_limit(),
                gas_cap
            )));
        }

        // Check withdrawals hash
        if header.withdrawals_root().is_some() {
            return Err(ConsensusError::Other("unexpected withdrawals".to_string()));
        }

        self.verify_cascading_fields(header)?;

        Ok(())
    }
}

impl<ChainSpec: EthChainSpec + EthereumHardforks, H: BlockHeader> HeaderValidator<H>
    for BorConsensus<ChainSpec>
{
    fn validate_header(&self, _header: &SealedHeader<H>) -> Result<(), ConsensusError> {
        // validate_header_gas(header.header())?;
        // validate_header_base_fee(header.header(), &self.chain_spec)
        Ok(())
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        // Parent number is consistent.
        if parent.number() + 1 != header.number() {
            return Err(ConsensusError::ParentBlockNumberMismatch {
                parent_block_number: parent.number(),
                block_number: header.number(),
            });
        }

        //TODO: look into this and correct it
        // if parent.hash() != header.parent_hash() {
        //     return Err(ConsensusError::ParentHashMismatch(
        //         GotExpected {
        //             got: header.parent_hash(),
        //             expected: parent.hash(),
        //         }
        //         .into(),
        //     ));
        // }

        Ok(())
    }
}
