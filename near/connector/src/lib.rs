use btc_types::contract_args::ProofArgs;
use bitcoin_types::connector_args::FinTransferArgs;
use near_plugins::{access_control, pause, AccessControlRole, AccessControllable, Pausable, Upgradable};
use near_sdk::borsh::{BorshSerialize, BorshDeserialize};
use near_sdk::{AccountId, Gas, near, Promise, require, BorshStorageKey, env, PromiseError, PromiseOrValue};
use near_sdk::collections::{LookupMap, LookupSet, Vector};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::PanicOnDefault;
use near_sdk::ext_contract;
use bitcoin_types::transaction::{ConsensusDecoder, NewTransferToBitcoin, OutPoint, Script, Transaction, TxIn, TxOut, UTXO};
use btc_types::hash::H256;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use bitcoin_types::bitcoin_connector_events::BitcoinConnectorEvent;

const MINT_BTC_GAS: Gas = Gas::from_tgas(10);
const BURN_BTC_GAS: Gas = Gas::from_tgas(10);
const VERIFY_TX_GAS: Gas = Gas::from_tgas(100);
const FT_TRANSFER_CALLBACK_GAS: Gas = Gas::from_tgas(50);
const MPC_SIGNING_GAS: Gas = Gas::from_tgas(250);

const SIGN_PATH: &str = "bitcoin-connector-1";

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    FinalisedTransfers,
    UTXOs,
    NewTransfers,
}

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
    pub omni_btc: AccountId,
    pub finalised_transfers: LookupSet<H256>,
    pub confirmations: u64,
    pub btc_light_client: AccountId,
    pub mpc_signer: AccountId,
    pub utxos: Vector<UTXO>,
    pub new_transfers: LookupMap<u64, NewTransferToBitcoin>,
    pub min_nonce: u64,
    pub last_nonce: u64,
}

#[ext_contract(ext_omni_bitcoin)]
pub trait ExtOmniBitcoin {
    fn mint(&mut self,
            receiver_id: AccountId,
            amount: U128);

    fn burn(&mut self, amount: U128);
}

#[ext_contract(ext_btc_light_client)]
pub trait ExtBtcLightClient {
    fn verify_transaction_inclusion(&self,
                                    #[serializer(borsh)] args: ProofArgs) -> bool;
}


#[near]
impl BitcoinConnector {
    #[init]
    pub fn new(omni_btc: AccountId,
               confirmations: u64,
               btc_light_client: AccountId,
               mpc_signer: AccountId) -> Self {
        Self {
            bitcoin_pk: "396e765f3fd99b894caea7e92ebb6d8764ae5cdd".to_string(),
            omni_btc,
            finalised_transfers: LookupSet::new(StorageKey::FinalisedTransfers),
            confirmations,
            btc_light_client,
            mpc_signer,
            utxos: Vector::new(StorageKey::UTXOs),
            new_transfers: LookupMap::new(StorageKey::NewTransfers),
            min_nonce: 0,
            last_nonce: 0,
        }
    }

    pub fn fin_transfer(&mut self, #[serializer(borsh)] args: FinTransferArgs) -> Promise {
        let tx = Transaction::from_bytes(&args.tx_raw, &mut 0).unwrap();

        let proof_args = ProofArgs {
            tx_id: tx.tx_hash.clone(),
            tx_block_blockhash: args.tx_block_blockhash,
            tx_index: args.tx_index,
            merkle_proof: args.merkle_proof,
            confirmations: self.confirmations.clone()
        };

        ext_btc_light_client::ext(self.btc_light_client.clone())
            .with_static_gas(VERIFY_TX_GAS)
            .verify_transaction_inclusion(proof_args)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(FT_TRANSFER_CALLBACK_GAS)
                    .fin_transfer_callback(tx),
            )
    }

    #[private]
    pub fn fin_transfer_callback(
        &mut self,
        #[callback_result] call_result: Result<bool, PromiseError>,
        #[serializer(borsh)] tx: Transaction
    ) {
        require!(call_result.unwrap(), "Failed to verify proof");

        let mut value = 0;
        let mut recipient = None;
        for (i, tx_output) in tx.output.into_iter().enumerate() {
            match tx_output.script_pubkey.clone() {
                Script::V0P2wpkh(pk) => {
                    if pk == self.bitcoin_pk {
                        value += tx_output.value;
                        self.utxos.push(
                            &UTXO {
                                txid: tx.tx_hash.clone(),
                                vout: i as u32,
                                value: tx_output.value.clone(),
                                script_pubkey: tx_output.script_pubkey
                            }
                        );
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

        require!(self.finalised_transfers.insert(&tx.tx_hash),
            "The transfer is already finalised");

        if let Some(recipient) = recipient {
            ext_omni_bitcoin::ext(self.omni_btc.clone())
                .with_static_gas(MINT_BTC_GAS)
                .mint(recipient.parse().unwrap(), U128::from(value as u128));
        }
    }

    pub fn sign(&mut self) {
        let (unsigned_tx, utxos) = self.get_unsigned_tx();

    }
}

impl BitcoinConnector {
    fn get_unsigned_tx(&mut self) -> (Transaction, Vec<UTXO>) {
        let utxos = self.get_utxos();
        let new_transfer_data = self.new_transfers.get(&self.min_nonce).unwrap();
        self.new_transfers.remove(&self.min_nonce);
        self.min_nonce += 1;

        let txin = TxIn {
            previous_output: OutPoint {
                txid: utxos[0].txid.clone(),
                vout: utxos[0].vout.clone()
            },
            script_sig: vec![],
            sequence: 0
        };

        let txout = TxOut {
            value: new_transfer_data.value,
            script_pubkey: Script::V0P2wpkh(new_transfer_data.recipient_on_bitcoin)
        };

        let unsigned_tx = Transaction {
            version: 2,
            lock_time: 0,
            input: vec![txin],
            output: vec![txout],
            tx_hash: Default::default()
        };

        (unsigned_tx, utxos)
    }

    fn get_utxos(&mut self) -> Vec<UTXO> {
        vec![self.utxos.pop().unwrap()]
    }
}


#[near]
impl FungibleTokenReceiver for BitcoinConnector {
    #[pause(except(roles(Role::DAO)))]
    fn ft_on_transfer(&mut self,
                      sender_id: AccountId,
                      amount: U128,
                      msg: String) -> PromiseOrValue<U128> {
        self.new_transfers.insert(&self.last_nonce, &NewTransferToBitcoin {
            sender_id: sender_id.clone(),
            recipient_on_bitcoin: msg.clone(),
            value: amount.0.clone() as u64
        });

        ext_omni_bitcoin::ext(self.omni_btc.clone())
                .with_static_gas(BURN_BTC_GAS)
                .burn(amount);

        env::log_str(&BitcoinConnectorEvent::InitTransferEvent{
            sender_id,
            recipient_on_bitcoin: msg,
            value: amount.0.clone() as u64
        }.to_log_string());

        PromiseOrValue::Value(U128(0))
    }
}
