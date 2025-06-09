use std::u64;

use alloy_consensus::{
    Transaction,
    transaction::{Recovered, TxLegacy},
};
use alloy_evm::{IntoTxEnv, revm::context::TxEnv};
use alloy_primitives::{Address, Bytes, TxKind, U256};

use crate::system_addres::SYSTEM_ADDRESS;

pub fn get_system_msg(to_address: Address, data: Bytes) -> TxLegacy {
    TxLegacy {
        chain_id: None,
        nonce: 0,
        to: TxKind::Call(to_address),
        value: U256::ZERO,
        gas_limit: u64::MAX / 2,
        gas_price: 0,
        input: data,
    }
}
