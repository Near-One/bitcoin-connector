use crate::bitcoin_connector_types::Script::OpReturn;
use btc_types::hash::H256;
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::AccountId;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, BorshSerialize, BorshDeserialize)]
pub struct UTXO {
    pub txid: H256,
    pub vout: u32,
    pub value: u64,
    pub script_pubkey: Script,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, BorshSerialize, BorshDeserialize)]
pub struct NewTransferToBitcoin {
    pub sender_id: AccountId,
    pub recipient_on_bitcoin: String,
    pub value: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub enum Script {
    OpReturn(String),
    V0P2wpkh(String),
}

impl Script {
    pub fn from_bytes(script_raw: Vec<u8>) -> Result<Script, &'static str> {
        const OP_RETURN: u8 = 0x6a;

        if script_raw[0] == OP_RETURN {
            return Ok(OpReturn(
                String::from_utf8(script_raw[2..].to_vec()).unwrap(),
            ));
        }

        if script_raw[0] == 0x00 && script_raw[1] == 0x14 {
            return Ok(Script::V0P2wpkh(hex::encode(&script_raw[2..])));
        }

        return Err("Incorrect script");
    }
}
