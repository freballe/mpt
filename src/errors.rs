use std::error::Error;
use std::fmt;

use ethereum_types::H256;
use rlp::DecoderError;

use crate::nibbles::Nibbles;

#[derive(Debug, PartialEq, Eq)]
pub enum TrieError {
    SqliteDB(String),
    Decoder(DecoderError),
    InvalidData,
    InvalidProof,
    MissingTrieNode {
        node_hash: H256,
        traversed: Option<Nibbles>,
        root_hash: Option<H256>,
        err_key: Option<Vec<u8>>,
    },
}

impl Error for TrieError {}

impl fmt::Display for TrieError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            TrieError::SqliteDB(ref err) => format!("trie error: {:?}", err),
            TrieError::Decoder(ref err) => format!("trie error: {:?}", err),
            TrieError::InvalidData => "trie error: invalid data".to_owned(),
            TrieError::InvalidProof => "trie error: invalid proof".to_owned(),
            TrieError::MissingTrieNode { .. } => "trie error: missing node".to_owned(),
        };
        write!(f, "{}", printable)
    }
}

impl From<DecoderError> for TrieError {
    fn from(error: DecoderError) -> Self {
        TrieError::Decoder(error)
    }
}