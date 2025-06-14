use crate::heimdall::error::HeimdallError;
use alloy_json_abi::JsonAbi;
use alloy_primitives::Address;
use alloy_sol_types::{
    SolCall,
    private::{Bytes, Uint},
    sol,
};

pub mod state_receiver;

#[derive(Debug, Clone, Default)]
pub struct GenesisContractClient {
    validator_contract: Address,
    state_receiver_contract: Address,
    system_address: Address,
}

impl GenesisContractClient {
    pub fn new(
        validator_contract: Address,
        state_receiver_contract: Address,
        system_address: Address,
    ) -> Self {
        Self {
            validator_contract,
            state_receiver_contract,
            system_address,
        }
    }

    pub fn get_validator_contract_address(&self) -> Address {
        self.validator_contract
    }

    pub fn get_state_receiver_contract_address(&self) -> Address {
        self.state_receiver_contract
    }

    pub fn get_system_address(&self) -> Address {
        self.system_address
    }

    pub fn validator_set_abi(&self) -> &JsonAbi {
        todo!()
    }
}
