use rusqlite::{params, Connection, Result};
mod nibbles;
mod node;

mod db;
mod errors;
mod trie;
pub use db::{SqliteDB, DB};
pub use errors::{TrieError};
pub use trie::{EthTrie, ITrie};

use std::sync::Arc;
use hex::FromHex;
use rand::Rng;
use std::fs;


#[derive(Debug)]
struct NodeDB {
    key: Vec<u8>,
    data: Option<Vec<u8>>,
}

fn insert_full_branch() {
    delete_file(String::from("test1.db"));
    let memdb = Arc::new(SqliteDB::new(String::from("test1.db")));
    let mut trie = EthTrie::new(memdb);

    trie.put(b"test", b"test");
    trie.put(b"test1", b"test");
    trie.put(b"test2", b"test");
    trie.put(b"test23", b"test");
    trie.put(b"test33", b"test");
    trie.put(b"test44", b"test");
    trie.commit();

    let v = trie.get(b"test").unwrap();
    assert_eq!(Some(b"test".to_vec()), v);
}

fn test_trie_remove() {
    delete_file(String::from("test1.db"));
    let memdb = Arc::new(SqliteDB::new(String::from("test1.db")));
    let mut trie = EthTrie::new(memdb);
    trie.put(b"test", b"test");
    trie.commit();

    trie.del(b"test");
    trie.commit();
    let found = trie.get(b"test");
    assert!(found.is_err())
}

fn delete_file(path:String) -> std::io::Result<()> {
    fs::remove_file(path)?;
    Ok(())
}
fn test_small_trie_at_root() {
    delete_file(String::from("test1.db"));
    let memdb = Arc::new(SqliteDB::new(String::from("test1.db")));
    let mut trie = EthTrie::new(memdb.clone());
    trie.put(b"key", b"val");
    let new_root_hash = trie.commit();

    let empty_trie = EthTrie::new(memdb.clone());
    // Can't find key in new trie at empty root
    assert!(empty_trie.get(b"key").is_err());

    let trie_view = empty_trie.at_root(new_root_hash);
    assert_eq!(&trie_view.get(b"key").unwrap().unwrap(), b"val");

    // Previous trie was not modified
    assert!(empty_trie.get(b"key").is_err());
}

fn main() -> Result<()> {
    test_trie_remove();
    insert_full_branch();
    test_small_trie_at_root();
    Ok(())
}