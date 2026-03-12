//! Block assembler for Bor.
//!
//! Assembles a complete block from execution results. Similar to Ethereum's
//! `EthBlockAssembler` but operates on [`BorBlockExecutionCtx`] and omits
//! features not present in Bor (blob gas, beacon roots, Prague requests, etc.).

use crate::block_executor::BorBlockExecutionCtx;
use alloy_consensus::{
    proofs::{self, calculate_receipt_root},
    Block, BlockBody, Header, TxReceipt, EMPTY_OMMER_ROOT_HASH,
};
use alloy_eips::merge::BEACON_NONCE;
use reth_chainspec::{EthChainSpec, EthereumHardforks};
use reth_evm::{
    block::{BlockExecutionResult, BlockExecutorFactory},
    execute::{BlockAssembler, BlockAssemblerInput, BlockExecutionError},
};
use reth_primitives_traits::{logs_bloom, Receipt, SignedTransaction};
use revm::context::Block as _;
use std::sync::Arc;

/// Block assembler for Bor.
#[derive(Debug, Clone)]
pub struct BorBlockAssembler<ChainSpec = reth_chainspec::ChainSpec> {
    /// Chain spec.
    pub chain_spec: Arc<ChainSpec>,
}

impl<ChainSpec> BorBlockAssembler<ChainSpec> {
    /// Creates a new [`BorBlockAssembler`].
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self { chain_spec }
    }
}

impl<F, ChainSpec> BlockAssembler<F> for BorBlockAssembler<ChainSpec>
where
    F: for<'a> BlockExecutorFactory<
        ExecutionCtx<'a> = BorBlockExecutionCtx<'a>,
        Transaction: SignedTransaction,
        Receipt: Receipt,
    >,
    ChainSpec: EthChainSpec + EthereumHardforks,
{
    type Block = Block<F::Transaction>;

    fn assemble_block(
        &self,
        input: BlockAssemblerInput<'_, '_, F>,
    ) -> Result<Self::Block, BlockExecutionError> {
        let BlockAssemblerInput {
            evm_env,
            execution_ctx,
            parent: _,
            transactions,
            output: BlockExecutionResult { receipts, gas_used, .. },
            state_root,
            ..
        } = input;

        // Extract the Ethereum context from the Bor context
        let ctx = &execution_ctx.eth;

        let timestamp = evm_env.block_env.timestamp().saturating_to();

        let transactions_root = proofs::calculate_transaction_root(&transactions);
        let receipts_root = calculate_receipt_root(
            &receipts.iter().map(|r| r.with_bloom_ref()).collect::<Vec<_>>(),
        );
        let logs_bloom = logs_bloom(receipts.iter().flat_map(|r| r.logs()));

        // Bor: no withdrawals
        let withdrawals = None;
        let withdrawals_root = None;

        // Bor: no Prague requests
        let requests_hash = None;

        // Bor: no blob gas (no EIP-4844)
        let excess_blob_gas = None;
        let blob_gas_used = None;

        let header = Header {
            parent_hash: ctx.parent_hash,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            beneficiary: evm_env.block_env.beneficiary(),
            state_root,
            transactions_root,
            receipts_root,
            withdrawals_root,
            logs_bloom,
            timestamp,
            mix_hash: evm_env.block_env.prevrandao().unwrap_or_default(),
            nonce: BEACON_NONCE.into(),
            base_fee_per_gas: Some(evm_env.block_env.basefee()),
            number: evm_env.block_env.number().saturating_to(),
            gas_limit: evm_env.block_env.gas_limit(),
            difficulty: evm_env.block_env.difficulty(),
            gas_used: *gas_used,
            extra_data: Default::default(),
            parent_beacon_block_root: None,
            blob_gas_used,
            excess_blob_gas,
            requests_hash,
        };

        Ok(Block {
            header,
            body: BlockBody {
                transactions,
                ommers: Default::default(),
                withdrawals,
            },
        })
    }
}
