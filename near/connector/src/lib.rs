use bitcoin::absolute::LockTime;
use bitcoin::blockdata::transaction::Transaction as BitcoinTransaction;
use bitcoin::consensus::{deserialize, serialize};
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::TxIn as BitcoinTxIn;
use bitcoin::TxOut as BitcoinTxOut;
use bitcoin::{sighash, Address, Amount, EcdsaSighashType, PublicKey};
use bitcoin_types::bitcoin_connector_events::BitcoinConnectorEvent;
use bitcoin_types::bitcoin_connector_types::{NewTransferToBitcoin, Script, UTXO};
use bitcoin_types::connector_args::{FinTransferArgs, SignRequest};
use bitcoin_types::mpc_types::SignatureResponse;
use btc_types::contract_args::ProofArgs;
use btc_types::hash::H256;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_plugins::{
    access_control, pause, AccessControlRole, AccessControllable, Pausable, Upgradable,
};
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, LookupSet, Vector};
use near_sdk::ext_contract;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::PanicOnDefault;
use near_sdk::{
    env, near, require, AccountId, BorshStorageKey, Gas, Promise, PromiseError, PromiseOrValue,
};
use std::default::Default;
use std::str::FromStr;

const MINT_BTC_GAS: Gas = Gas::from_tgas(10);
const BURN_BTC_GAS: Gas = Gas::from_tgas(10);
const VERIFY_TX_GAS: Gas = Gas::from_tgas(100);
const FT_TRANSFER_CALLBACK_GAS: Gas = Gas::from_tgas(50);
const MPC_SIGNING_GAS: Gas = Gas::from_tgas(250);
const SIGN_TRANSFER_CALLBACK_GAS: Gas = Gas::from_tgas(5);

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
    fn mint(&mut self, receiver_id: AccountId, amount: U128);

    fn burn(&mut self, amount: U128);
}

#[ext_contract(ext_btc_light_client)]
pub trait ExtBtcLightClient {
    fn verify_transaction_inclusion(&self, #[serializer(borsh)] args: ProofArgs) -> bool;
}

#[ext_contract(ext_signer)]
pub trait ExtSigner {
    fn sign(&mut self, request: SignRequest);
}

#[near]
impl BitcoinConnector {
    #[init]
    pub fn new(
        connector_bitcoin_public_key: String,
        omni_btc: AccountId,
        confirmations: u64,
        btc_light_client: AccountId,
        mpc_signer: AccountId,
    ) -> Self {
        Self {
            bitcoin_pk: connector_bitcoin_public_key,
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
        let tx: BitcoinTransaction = deserialize(&args.tx_raw).unwrap();

        let proof_args = ProofArgs {
            tx_id: Self::get_tx_id(&tx),
            tx_block_blockhash: args.tx_block_blockhash,
            tx_index: args.tx_index,
            merkle_proof: args.merkle_proof,
            confirmations: self.confirmations.clone(),
        };

        ext_btc_light_client::ext(self.btc_light_client.clone())
            .with_static_gas(VERIFY_TX_GAS)
            .verify_transaction_inclusion(proof_args)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(FT_TRANSFER_CALLBACK_GAS)
                    .fin_transfer_callback(args.tx_raw),
            )
    }

    #[private]
    pub fn fin_transfer_callback(
        &mut self,
        #[callback_result] call_result: Result<bool, PromiseError>,
        #[serializer(borsh)] tx_raw: Vec<u8>,
    ) {
        require!(call_result.unwrap(), "Failed to verify proof");
        let tx: BitcoinTransaction = deserialize(&tx_raw).unwrap();
        let tx_id = Self::get_tx_id(&tx);

        let mut value = 0;
        let mut recipient = None;
        for (i, tx_output) in tx.output.into_iter().enumerate() {
            let script: Script =
                Script::from_bytes(tx_output.script_pubkey.as_bytes().to_vec()).unwrap();
            match script.clone() {
                Script::V0P2wpkh(pk) => {
                    if pk == self.bitcoin_pk {
                        value += tx_output.value.to_sat();
                        self.utxos.push(&UTXO {
                            txid: tx_id.clone(),
                            vout: i as u32,
                            value: tx_output.value.clone().to_sat(),
                            script_pubkey: script.clone(),
                        });
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

        require!(
            self.finalised_transfers.insert(&tx_id),
            "The transfer is already finalised"
        );

        if let Some(recipient) = recipient {
            ext_omni_bitcoin::ext(self.omni_btc.clone())
                .with_static_gas(MINT_BTC_GAS)
                .mint(recipient.parse().unwrap(), U128::from(value as u128));
        }
    }

    #[payable]
    pub fn sign(&mut self) -> Promise {
        let (unsigned_tx, utxo) = self.get_unsigned_tx();
        let msg_to_sign: Vec<u8> = self.sign_input(&unsigned_tx, &utxo, 0);
        let ser_tx = serialize(&unsigned_tx);

        ext_signer::ext(self.mpc_signer.clone())
            .with_static_gas(MPC_SIGNING_GAS)
            .with_attached_deposit(env::attached_deposit())
            .sign(SignRequest {
                payload: msg_to_sign.clone().try_into().unwrap(),
                path: SIGN_PATH.to_owned(),
                key_version: 0,
            })
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(SIGN_TRANSFER_CALLBACK_GAS)
                    .sign_callback(ser_tx),
            )
    }

    #[private]
    pub fn sign_callback(
        &mut self,
        #[callback_result] call_result: Result<SignatureResponse, PromiseError>,
        ser_tx: Vec<u8>,
    ) {
        let mut unsigned_tx: BitcoinTransaction = deserialize(&ser_tx).unwrap();

        let signature = call_result.unwrap();
        let sig_raw = signature.to_bytes();
        unsigned_tx.input[0].witness.push(sig_raw);

        let public_key = PublicKey::from_str(&self.bitcoin_pk).unwrap();
        unsigned_tx.input[0].witness.push(public_key.to_bytes());

        let tx_hex_string = hex::encode(serialize(&unsigned_tx));

        env::log_str(
            &BitcoinConnectorEvent::SignTransferEvent {
                bitcoin_tx_hex: tx_hex_string,
            }
            .to_log_string(),
        );
    }
}

impl BitcoinConnector {
    fn get_tx_id(transaction: &BitcoinTransaction) -> H256 {
        let tx_id = transaction.compute_ntxid();
        H256::from(tx_id.to_byte_array())
    }

    fn sign_input(
        &mut self,
        unsigned_tx: &BitcoinTransaction,
        utxo: &UTXO,
        input_index: usize,
    ) -> Vec<u8> {
        let public_key = PublicKey::from_str(&self.bitcoin_pk).unwrap();

        let mut cache = sighash::SighashCache::new(unsigned_tx);
        let sighash = cache
            .p2wpkh_signature_hash(
                input_index,
                &public_key.p2wpkh_script_code().unwrap(),
                Amount::from_sat(utxo.value),
                EcdsaSighashType::All,
            )
            .expect("failed to compute sighash");

        sighash.to_byte_array().to_vec()
    }

    fn get_unsigned_tx(&mut self) -> (BitcoinTransaction, UTXO) {
        let utxo = self.get_utxo();
        let new_transfer_data = self.new_transfers.get(&self.min_nonce).unwrap();
        self.new_transfers.remove(&self.min_nonce);
        self.min_nonce += 1;

        let txin = BitcoinTxIn {
            previous_output: bitcoin::OutPoint {
                txid: bitcoin::Txid::from_byte_array(utxo.txid.clone().0),
                vout: utxo.vout.clone(),
            },
            script_sig: Default::default(),
            sequence: Default::default(),
            witness: Default::default(),
        };

        let recipient_address = Address::from_str(&new_transfer_data.recipient_on_bitcoin).unwrap();
        let recipient_address = recipient_address.assume_checked();

        let txout = BitcoinTxOut {
            value: Amount::from_sat(new_transfer_data.value),
            script_pubkey: recipient_address.script_pubkey(),
        };

        let unsigned_tx = BitcoinTransaction {
            version: Version(2),
            lock_time: LockTime::ZERO,
            input: vec![txin],
            output: vec![txout],
        };

        (unsigned_tx, utxo)
    }

    fn get_utxo(&mut self) -> UTXO {
        let mut max_j = 0;
        for i in 1..self.utxos.len() {
            if self.utxos.get(i).unwrap().value > self.utxos.get(max_j).unwrap().value {
                max_j = i;
            }
        }

        self.utxos.swap_remove(max_j)
    }
}

#[near]
impl FungibleTokenReceiver for BitcoinConnector {
    #[pause(except(roles(Role::DAO)))]
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.new_transfers.insert(
            &self.last_nonce,
            &NewTransferToBitcoin {
                sender_id: sender_id.clone(),
                recipient_on_bitcoin: msg.clone(),
                value: amount.0.clone() as u64,
            },
        );

        ext_omni_bitcoin::ext(self.omni_btc.clone())
            .with_static_gas(BURN_BTC_GAS)
            .burn(amount);

        env::log_str(
            &BitcoinConnectorEvent::InitTransferEvent {
                sender_id,
                recipient_on_bitcoin: msg,
                value: amount.0.clone() as u64,
            }
            .to_log_string(),
        );

        PromiseOrValue::Value(U128(0))
    }
}
