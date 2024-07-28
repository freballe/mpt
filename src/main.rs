use rusqlite::{params, Connection, Result};
mod nibbles;
mod node;
mod tests;

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
    // let conn = Connection::open("trie.db").unwrap();

    // _ = conn.execute(
    //     "CREATE TABLE trie (
    //         key BLOB PRIMARY KEY,
    //         data BLOB
    //     )",
    //     (), // empty list of parameters.
    // );
    
    // let me = NodeDB {
    //     key: vec![5,5,5,6],
    //     data: Some(vec![1,2,3,4]),
    // };
    // _ = conn.execute(
    //     "INSERT INTO trie (key, data) VALUES (?1, ?2)",
    //     (&me.key, &me.data),
    // )?;
    // let mut stmt = conn.prepare("DELETE FROM trie WHERE key=?1")?;
    // stmt.execute([me.key.clone()]);

    // let mut stmt = conn.prepare("SELECT data FROM trie WHERE key=?1")?;
    // let node_iter = stmt.query_map([me.key.clone()], |row| {
    //     Ok(NodeDB {
    //         key: row.get(0)?,
    //         data: row.get(1)?,
    //     })
    // })?;

    // for node in node_iter {
    //     println!("Found node {:?}", node.unwrap());
    // }
    // println!("Finished");

    test_trie_remove();
    insert_full_branch();
    test_small_trie_at_root();
    Ok(())
}