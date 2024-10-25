use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct FinTransferArgs {
    pub tx_raw: Vec<u8>,
}
