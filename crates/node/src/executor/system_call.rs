//! System contract call functions.

use alloy_consensus::{Transaction, TxLegacy};
use alloy_eips::Encodable2718;
use alloy_evm::{Evm, FromRecoveredTx, FromTxWithEncoded, IntoTxEnv};
use alloy_hardforks::EthereumHardforks;
use alloy_primitives::{Bytes, B256};
use bor::{
    call_message::get_system_msg,
    heimdall::{
        client::HeimdallClient, error::HeimdallError,
        genesis_contract_client::GenesisContractClient,
    },
};
use revm::DatabaseCommit;
use revm_context::{result::ExecutionResult, TxEnv};

/// An ephemeral helper type for executing system calls.
///
/// This can be used to chain system transaction calls.
///
#[derive(Debug)]
pub struct SystemCaller<Spec> {
    spec: Spec,
    heimdall_client: HeimdallClient,
    genesis_contract_client: GenesisContractClient,
}

impl<Spec> SystemCaller<Spec> {
    /// Create a new system caller with the given chain spec.
    pub const fn new(
        spec: Spec,
        heimdall_client: HeimdallClient,
        genesis_contract_client: GenesisContractClient,
    ) -> Self {
        Self {
            spec,
            heimdall_client,
            genesis_contract_client,
        }
    }
}

impl<Spec> SystemCaller<Spec>
where
    Spec: EthereumHardforks,
{
    /// Apply state sync contract call.>
    pub fn apply_state_sync_contract_call<E, T>(&mut self, evm: &mut E) -> Result<(), HeimdallError>
    where
        E: Evm<DB: DatabaseCommit, Tx: IntoTxEnv<T>>,
        T: Transaction,
    {
        let last_state_id = self.last_state_sync_event_id(evm)?;
        let from_id = last_state_id;

        // calculating the to time
        let to_time = evm.block().timestamp - calculate_state_delay(evm.block().number);
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
            .heimdall_client
            .fetch_state_sync_events(from_id, to_time)?;

        let system_address = self.genesis_contract_client.get_system_address();
        let state_receiver_contract = self
            .genesis_contract_client
            .get_state_receiver_contract_address();

        for event in state_sync_events {
            let data = self.genesis_contract_client.encode_state_sync_data(event)?;
            let tx = get_system_msg(state_receiver_contract, data.into());

            let result = evm
                .transact_commit(tx)
                .map_err(|e| HeimdallError::InvalidStateSyncData)?;

            match result {
                ExecutionResult::Success {
                    reason,
                    gas_used,
                    gas_refunded,
                    logs,
                    output,
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
        let data = self.genesis_contract_client.last_state_id()?;
        let data = Bytes::from(data);

        let state_receiver_contract = self
            .genesis_contract_client
            .get_state_receiver_contract_address();
        let system_address = self.genesis_contract_client.get_system_address();

        let result_and_state = evm
            .transact_system_call(system_address, state_receiver_contract, data)
            .map_err(|e| HeimdallError::EVMError)?;

        match result_and_state.result {
            ExecutionResult::Success {
                reason,
                gas_used,
                gas_refunded,
                logs,
                output,
            } => {
                let output = output.data();
                let last_state_id = self.genesis_contract_client.decode_last_state_id(output)?;
                Ok(last_state_id)
            }

            _ => {
                return Err(HeimdallError::EVMError);
            }
        }
    }
}

fn calculate_state_delay(block_number: u64) -> u64 {
    todo!()
}
