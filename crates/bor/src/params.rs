use std::sync::Arc;

use crate::{
    config::BorConfig,
    heimdall::{client::HeimdallClient, genesis_contract_client::GenesisContractClient},
};

#[derive(Debug, Clone)]
pub struct BorParams {
    pub bor_config: Arc<BorConfig>,
    pub genesis_contract_client: GenesisContractClient,
    pub heimdall_client: HeimdallClient,
}

impl BorParams {
    pub fn new(
        bor_config: Arc<BorConfig>,
        genesis_contract_client: GenesisContractClient,
        heimdall_client: HeimdallClient,
    ) -> Self {
        Self {
            bor_config,
            genesis_contract_client,
            heimdall_client,
        }
    }
}
