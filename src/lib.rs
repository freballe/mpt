mod nibbles;
mod node;
mod tests;

mod db;
mod errors;
mod trie;

pub use db::{SqliteDB, DB};
pub use errors::{TrieError};
pub use trie::{EthTrie, ITrie};

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
