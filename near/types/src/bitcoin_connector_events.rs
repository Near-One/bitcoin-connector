use near_sdk::AccountId;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json::json;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum BitcoinConnectorEvent {
    InitTransferEvent {
        sender_id: AccountId,
        recipient_on_bitcoin: String,
        value: u64
    },
    SignTransferEvent {
        bitcoin_tx_hex: String,
    }
}


impl BitcoinConnectorEvent {
    pub fn to_log_string(&self) -> String {
        json!(self).to_string()
    }
}
