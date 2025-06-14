use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Configuration for the Bor consensus engine
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BorConfig {
    /// Number of seconds between blocks to enforce
    #[serde(rename = "period")]
    pub period: BTreeMap<u64, u64>,

    /// Number of seconds delay between two producer interval
    #[serde(rename = "producerDelay")]
    pub producer_delay: BTreeMap<u64, u64>,

    /// Epoch length to proposer
    #[serde(rename = "sprint")]
    pub sprint: BTreeMap<u64, u64>,

    /// Backup multiplier to determine the wiggle time
    #[serde(rename = "backupMultiplier")]
    pub backup_multiplier: BTreeMap<u64, u64>,

    /// Validator set contract
    #[serde(rename = "validatorContract")]
    pub validator_contract: Address,

    /// State receiver contract
    #[serde(rename = "stateReceiverContract")]
    pub state_receiver_contract: Address,

    /// Override state records count
    #[serde(rename = "overrideStateSyncRecords")]
    pub override_state_sync_records: BTreeMap<u64, i32>,

    // TODO: Handle it in more better way later on.
    /// Block allocation configuration
    #[serde(rename = "blockAlloc")]
    pub block_alloc: BTreeMap<String, serde_json::Value>,

    /// Governance contract where the token will be sent to and burnt in london fork
    #[serde(rename = "burntContract")]
    pub burnt_contract: BTreeMap<u64, Address>,

    /// Jaipur switch block (None = no fork, Some(0) = already on jaipur)
    #[serde(rename = "jaipurBlock")]
    pub jaipur_block: Option<u64>,

    /// Delhi switch block (None = no fork, Some(0) = already on delhi)
    #[serde(rename = "delhiBlock")]
    pub delhi_block: Option<u64>,

    /// Indore switch block (None = no fork, Some(0) = already on indore)
    #[serde(rename = "indoreBlock")]
    pub indore_block: Option<u64>,

    /// StateSync Confirmation Delay, in seconds, to calculate `to`
    #[serde(rename = "stateSyncConfirmationDelay")]
    pub state_sync_confirmation_delay: BTreeMap<u64, u64>,

    /// Ahmedabad switch block (None = no fork, Some(0) = already on ahmedabad)
    #[serde(rename = "ahmedabadBlock")]
    pub ahmedabad_block: Option<u64>,
}

impl BorConfig {
    pub fn is_sprint_start(&self, block_number: u64) -> bool {
        let sprint_number = self.sprint_number(block_number);

        match sprint_number {
            Ok(sprint_number) => {
                if block_number % sprint_number == 0 {
                    return true;
                } else {
                    return false;
                }
            }

            Err(e) => {
                panic!("Sprint not found for block: {}", e);
            }
        }
    }

    pub fn sprint_number(&self, block_number: u64) -> Result<u64, String> {
        let mut sprint_number: u64 = 0;

        for (key, value) in self.sprint.iter() {
            if block_number >= *key {
                sprint_number = *value;
            }
        }

        if sprint_number == 0 {
            return Err("Sprint not found for block".to_string());
        }

        Ok(sprint_number)
    }

    pub fn is_indore_fork_enabled(&self, block_number: u64) -> bool {
        self.indore_block.is_some() && self.indore_block.unwrap() <= block_number
    }

    pub fn is_ahmedabad_fork_enabled(&self, block_number: u64) -> bool {
        self.ahmedabad_block.is_some() && self.ahmedabad_block.unwrap() <= block_number
    }

    pub fn is_delhi_fork_enabled(&self, block_number: u64) -> bool {
        self.delhi_block.is_some() && self.delhi_block.unwrap() <= block_number
    }

    pub fn is_jaipur_fork_enabled(&self, block_number: u64) -> bool {
        self.jaipur_block.is_some() && self.jaipur_block.unwrap() <= block_number
    }

    pub fn validator_contract(&self) -> Address {
        self.validator_contract.clone()
    }

    pub fn state_receiver_contract(&self) -> Address {
        self.state_receiver_contract.clone()
    }

    // TODO: Handle it in more better way later on.
    pub fn burnt_contract(&self, block_number: u64) -> Result<Address, String> {
        todo!()
    }
}
