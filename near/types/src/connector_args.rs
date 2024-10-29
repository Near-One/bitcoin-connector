use btc_types::hash::H256;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct FinTransferArgs {
    pub tx_raw: Vec<u8>,
    pub tx_block_blockhash: H256,
    pub tx_index: u64,
    pub merkle_proof: Vec<H256>,
}
