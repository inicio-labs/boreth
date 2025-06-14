//! System contract call functions.

use std::sync::Arc;

use alloy_evm::{Evm, IntoTxEnv};
use alloy_hardforks::EthereumHardforks;
use alloy_primitives::{Bytes, TxKind, U256};
use bor::{
    heimdall::{error::HeimdallError, span::Span},
    params::BorParams,
};
use reth_chainspec::EthChainSpec;
use revm::DatabaseCommit;
use revm_context::{result::ExecutionResult, TransactionType, TxEnv};

/// An ephemeral helper type for executing system calls.
///
/// This can be used to chain system transaction calls.
///
#[derive(Debug)]
pub struct SystemCaller<Spec: EthChainSpec> {
    spec: Spec,
    bor_params: Arc<BorParams>,
}

impl<Spec: EthChainSpec> SystemCaller<Spec> {
    /// Create a new system caller with the given chain spec.
    pub const fn new(spec: Spec, bor_params: Arc<BorParams>) -> Self {
        Self { spec, bor_params }
    }
}

impl<Spec: EthChainSpec> SystemCaller<Spec>
where
    Spec: EthereumHardforks,
{
    /// Apply state sync contract call.>
    pub fn apply_state_sync_contract_call<E>(&mut self, evm: &mut E) -> Result<(), HeimdallError>
    where
        E: Evm<DB: DatabaseCommit>,
        TxEnv: IntoTxEnv<E::Tx>,
    {
        let last_state_id = self.last_state_sync_event_id(evm)?;
        let from_id = last_state_id;

        // calculating the to time
        let to_time = if self
            .spec
            .is_spurious_dragon_active_at_block(evm.block().number)
        //Need to change the hardfork logic, it is for dummy purpose
        {
            evm.block().timestamp - calculate_state_delay(evm.block().number)
        } else {
            // TODO: Need to rewrite this logic, it is not correct
            // need to create the bor config
            evm.block().timestamp
        };

        // fetching the state sync events from heimdall
        let state_sync_events = self
            .bor_params
            .heimdall_client
            .fetch_state_sync_events(from_id, to_time)?;

        for event in state_sync_events {
            let data = self
                .bor_params
                .genesis_contract_client
                .encode_state_sync_data(event)?;
            let tx = self.get_state_sync_tx(data.into());

            let result = evm
                .transact_commit(tx)
                .map_err(|_| HeimdallError::InvalidStateSyncData)?;

            match result {
                ExecutionResult::Success {
                    reason: _,
                    gas_used: _,
                    gas_refunded: _,
                    logs: _,
                    output: _,
                } => {}

                _ => {
                    return Err(HeimdallError::EVMError);
                }
            }
        }

        Ok(())
    }

    /// Get the last state sync event id.
    pub fn last_state_sync_event_id(
        &mut self,
        evm: &mut impl Evm<DB: DatabaseCommit>,
    ) -> Result<u64, HeimdallError> {
        let data = self.bor_params.genesis_contract_client.last_state_id()?;
        let data = Bytes::from(data);

        let state_receiver_contract = self
            .bor_params
            .genesis_contract_client
            .get_state_receiver_contract_address();
        let system_address = self.bor_params.genesis_contract_client.get_system_address();

        let result_and_state = evm
            .transact_system_call(system_address, state_receiver_contract, data)
            .map_err(|e| HeimdallError::EVMError)?;

        match result_and_state.result {
            ExecutionResult::Success {
                reason: _,
                gas_used: _,
                gas_refunded: _,
                logs: _,
                output,
            } => {
                let output = output.data();
                let last_state_id = self
                    .bor_params
                    .genesis_contract_client
                    .decode_last_state_id(output)?;
                Ok(last_state_id)
            }

            _ => {
                return Err(HeimdallError::EVMError);
            }
        }
    }

    /// Creates a deposit tx to pay block reward to a validator.
    pub fn get_state_sync_tx(&self, input: Bytes) -> TxEnv {
        let state_receiver_contract = self
            .bor_params
            .genesis_contract_client
            .get_state_receiver_contract_address();

        let system_address = self.bor_params.genesis_contract_client.get_system_address();

        TxEnv {
            tx_type: TransactionType::Legacy as u8,
            caller: system_address,
            gas_limit: u64::MAX / 2,
            gas_price: 0,
            kind: TxKind::Call(state_receiver_contract),
            value: U256::ZERO,
            data: input,
            nonce: 0,
            chain_id: Some(self.spec.chain_id()),
            access_list: Default::default(),
            gas_priority_fee: None,
            blob_hashes: Default::default(),
            max_fee_per_blob_gas: 0,
            authorization_list: Default::default(),
        }
    }

    //-----------------------------------Span Functions----------------------------------------

    //TODO: Club all the following function in another file
    /// Apply state sync contract call.>
    pub fn check_and_apply_commit_span<E>(&mut self, evm: &mut E) -> Result<(), HeimdallError>
    where
        E: Evm<DB: DatabaseCommit>,
        TxEnv: IntoTxEnv<E::Tx>,
    {
        todo!()
    }

    /// Apply state sync contract call.>
    pub fn apply_commit_span<E>(&mut self, evm: &mut E) -> Result<(), HeimdallError>
    where
        E: Evm<DB: DatabaseCommit>,
        TxEnv: IntoTxEnv<E::Tx>,
    {
        todo!()
    }

    /// Get the last state sync event id.
    pub fn get_current_span(
        &mut self,
        evm: &mut impl Evm<DB: DatabaseCommit>,
    ) -> Result<Span, HeimdallError> {
        todo!()
    }

    pub fn get_current_validators_by_hash(
        &mut self,
        evm: &mut impl Evm<DB: DatabaseCommit>,
    ) -> Result<u64, HeimdallError> {
        todo!()
    }

    pub fn get_current_validators_by_block_nr_or_hash(
        &mut self,
        evm: &mut impl Evm<DB: DatabaseCommit>,
    ) -> Result<u64, HeimdallError> {
        todo!()
    }

    pub fn commit_span(
        &mut self,
        evm: &mut impl Evm<DB: DatabaseCommit>,
    ) -> Result<(), HeimdallError> {
        todo!()
    }
}

fn calculate_state_delay(block_number: u64) -> u64 {
    todo!()
}
