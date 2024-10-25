use btc_types::hash::H256;
use crate::transaction::Script::OpReturn;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Transaction {
    pub version: i32,
    pub lock_time: u32,
    pub input: Vec<TxIn>,
    pub output: Vec<TxOut>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct TxIn {
    pub previous_output: OutPoint,
    pub script_sig: Vec<u8>,
    pub sequence: u32,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct TxOut {
    pub value: u64,
    pub script_pubkey: Script,
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Script {
    OpReturn(String),
    V0P2wpkh(String),
}

impl ConsensusDecoder for Script {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let script_raw = Vec::<u8>::from_bytes(bytes, offset)?;

        const OP_RETURN: u8 = 0x6a;

        if script_raw[0] == OP_RETURN {
            return Ok(OpReturn(String::from_utf8(script_raw[2..].to_vec()).unwrap()));
        }

        if script_raw[0] == 0x00 && script_raw[1] == 0x14 {
            return Ok(Script::V0P2wpkh(hex::encode(&script_raw[2..])));
        }

        return Err("Incorrect script");
    }
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct OutPoint {
    pub txid: H256,
    pub vout: u32,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct VarInt(pub u64);

pub trait ConsensusDecoder: Sized {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str>;
}

impl ConsensusDecoder for Transaction {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let mut tx = Transaction{
            version: 0,
            lock_time: 0,
            input: vec![],
            output: vec![]
        };

        tx.version = i32::from_bytes(bytes, offset)?;
        tx.input = Vec::<TxIn>::from_bytes(bytes, offset)?;
        tx.output = Vec::<TxOut>::from_bytes(bytes, offset)?;
        tx.lock_time = u32::from_bytes(bytes, offset)?;

        Ok(tx)
    }
}

impl ConsensusDecoder for i32 {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        if *offset + 4 > bytes.len() {
            return Err("Not enough bytes for parsing i32");
        }
        let value = i32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        Ok(value)
    }
}

impl ConsensusDecoder for u8 {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        if *offset + 1 > bytes.len() {
            return Err("Not enough bytes for parsing u8");
        }
        let value = u8::from_le_bytes(bytes[*offset..*offset + 1].try_into().unwrap());
        *offset += 1;
        Ok(value)
    }
}

impl ConsensusDecoder for u16 {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        if *offset + 2 > bytes.len() {
            return Err("Not enough bytes for parsing u16");
        }
        let value = u16::from_le_bytes(bytes[*offset..*offset + 2].try_into().unwrap());
        *offset += 2;
        Ok(value)
    }
}

impl ConsensusDecoder for u32 {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        if *offset + 4 > bytes.len() {
            return Err("Not enough bytes for parsing u32");
        }
        let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        Ok(value)
    }
}

impl ConsensusDecoder for u64 {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        if *offset + 8 > bytes.len() {
            return Err("Not enough bytes for parsing u64");
        }
        let value = u64::from_le_bytes(bytes[*offset..*offset + 8].try_into().unwrap());
        *offset += 8;
        Ok(value)
    }
}

impl ConsensusDecoder for TxIn {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let mut txinput = TxIn{
            previous_output: OutPoint { txid: H256::default(), vout: 0 },
            script_sig: vec![],
            sequence: 0,
        };
        txinput.previous_output = OutPoint::from_bytes(bytes, offset)?;
        txinput.script_sig = Vec::<u8>::from_bytes(bytes, offset)?;
        txinput.sequence = u32::from_bytes(bytes, offset)?;

        Ok(txinput)
    }
}

impl ConsensusDecoder for TxOut {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let mut txoutput = TxOut {
            value: u64::from_bytes(bytes, offset)?,
            script_pubkey: Script::from_bytes(bytes, offset)?
        };

        Ok(txoutput)
    }
}

impl ConsensusDecoder for Vec<TxIn> {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let length = VarInt::from_bytes(bytes, offset)?.0 as usize;
        let mut value = vec![];
        for i in 0..length {
            value.push(TxIn::from_bytes(bytes, offset)?);
        }
        Ok(value)
    }
}

impl ConsensusDecoder for Vec<TxOut> {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let length = VarInt::from_bytes(bytes, offset)?.0 as usize;
        let mut value = vec![];
        for i in 0..length {
            value.push(TxOut::from_bytes(bytes, offset)?);
        }
        Ok(value)
    }
}


impl ConsensusDecoder for OutPoint {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let mut value = OutPoint{ txid: H256::from([0u8; 32]), vout: 0 };
        value.txid = H256::from_bytes(bytes, offset)?;
        value.vout = u32::from_bytes(bytes, offset)?;

        Ok(value)
    }
}

impl ConsensusDecoder for H256 {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        if *offset + 32 > bytes.len() {
            return Err("Not enough bytes for parsing H256");
        }
        let value = H256::try_from(bytes[*offset..*offset + 32].to_vec()).unwrap();
        *offset += 32;
        Ok(value)
    }
}

impl ConsensusDecoder for Vec<u8> {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let length = VarInt::from_bytes(bytes, offset)?.0 as usize;

        if *offset + length > bytes.len() {
            return Err("Not enough bytes for decode Vec<u8>");
        }

        let vec = bytes[*offset..*offset + length].to_vec();
        *offset += length;
        Ok(vec)
    }
}

impl ConsensusDecoder for VarInt {
    fn from_bytes(bytes: &[u8], offset: &mut usize) -> Result<Self, &'static str> {
        let n = u8::from_bytes(bytes, offset)?;
        match n {
            0xFF => {
                let x = u64::from_bytes(bytes, offset)?;
                if x < 0x100000000 {
                    Err("Incorrect VarInt")
                } else {
                    Ok(VarInt(x.into()))
                }
            }
            0xFE => {
                let x = u32::from_bytes(bytes, offset)?;
                if x < 0x10000 {
                    Err("Incorrect VarInt")
                } else {
                    Ok(VarInt(x.into()))
                }
            }
            0xFD => {
                let x = u16::from_bytes(bytes, offset)?;
                if x < 0xFD {
                    Err("Incorrect VarInt")
                } else {
                    Ok(VarInt(x.into()))
                }
            }
            n => Ok(VarInt(n.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_tx() {
        let raw_tx = vec![2, 0, 0, 0, 1, 146, 97, 87, 240, 48, 14, 73, 34, 141, 7, 70, 93, 114, 66, 33, 225, 162, 61, 65, 121, 144, 125, 23, 135, 76, 73, 173, 138, 39, 187, 4, 2, 1, 0, 0, 0, 0, 255, 255, 255, 255, 3, 44, 1, 0, 0, 0, 0, 0, 0, 22, 0, 20, 57, 110, 118, 95, 63, 217, 155, 137, 76, 174, 167, 233, 46, 187, 109, 135, 100, 174, 92, 221, 220, 5, 0, 0, 0, 0, 0, 0, 22, 0, 20, 171, 25, 243, 146, 206, 8, 220, 194, 181, 209, 37, 38, 57, 134, 222, 74, 165, 156, 95, 221, 0, 0, 0, 0, 0, 0, 0, 0, 17, 106, 15, 72, 101, 108, 108, 111, 44, 32, 66, 105, 116, 99, 111, 105, 110, 33, 0, 0, 0, 0];
        let tx = Transaction::from_bytes(&raw_tx,&mut 0).unwrap();

        println!("{:?}", tx);
    }
}
