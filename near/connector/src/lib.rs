use bitcoin_types::connector_args::FinTransferArgs;
use near_plugins::{access_control, AccessControlRole, AccessControllable, Pausable, Upgradable};
use near_sdk::borsh::BorshDeserialize;
use near_sdk::{AccountId, near};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::PanicOnDefault;
use bitcoin_types::transaction::{ConsensusDecoder, Script, Transaction};

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
pub struct BitcoinConnector {
    pub bitcoin_pk: String,
    pub omni_btc: AccountId
}

#[near]
impl BitcoinConnector {
    #[init]
    pub fn new(omni_btc: AccountId) -> Self {
        Self {
            bitcoin_pk: "396e765f3fd99b894caea7e92ebb6d8764ae5cdd".to_string(),
            omni_btc
        }
    }

    #[payable]
    pub fn fin_transfer(&mut self, #[serializer(borsh)] args: FinTransferArgs) {
        let tx = Transaction::from_bytes(&args.tx_raw, &mut 0).unwrap();
        let mut value = 0;
        let mut recipient = None;
        for tx_output in tx.output {
            match tx_output.script_pubkey {
                Script::V0P2wpkh(pk) => {
                    if pk == self.bitcoin_pk {
                        value += tx_output.value;
                    }
                }
                Script::OpReturn(account) => {
                    if recipient != None {
                        panic!("Tx should contain exactly one OP_RETURN script");
                    }
                    recipient = Some(account)
                }
            }
        }




    }
}
