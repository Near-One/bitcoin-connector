use bitcoin_types::connector_args::FinTransferArgs;
use near_plugins::{access_control, AccessControlRole, AccessControllable, Pausable, Upgradable};
use near_sdk::borsh::BorshDeserialize;
use near_sdk::near;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::PanicOnDefault;

#[derive(AccessControlRole, Deserialize, Serialize, Copy, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum Role {
    DAO,
    PauseManager,
    UpgradableCodeStager,
    UpgradableCodeDeployer,
}

#[near(contract_state)]
#[derive(Pausable, Upgradable, PanicOnDefault)]
#[access_control(role_type(Role))]
#[pausable(manager_roles(Role::PauseManager))]
#[upgradable(access_control_roles(
    code_stagers(Role::UpgradableCodeStager, Role::DAO),
    code_deployers(Role::UpgradableCodeDeployer, Role::DAO),
    duration_initializers(Role::DAO),
    duration_update_stagers(Role::DAO),
    duration_update_appliers(Role::DAO),
))]
pub struct BitcoinConnector {}

#[near]
impl BitcoinConnector {
    #[init]
    pub fn new() -> Self {
        Self {}
    }

    #[payable]
    pub fn fin_transfer(&mut self, #[serializer(borsh)] args: FinTransferArgs) {

    }
}
