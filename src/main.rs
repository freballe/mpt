use rusqlite::{params, Connection, Result};
mod nibbles;
mod node;
mod tests;

mod db;
mod errors;
mod trie;
pub use db::{SqliteDB, DB};
pub use errors::{MemDBError, TrieError};
pub use trie::{EthTrie, ITrie};

use std::sync::Arc;


#[derive(Debug)]
struct NodeDB {
    key: Vec<u8>,
    data: Option<Vec<u8>>,
}

fn insert_full_branch() {
    let memdb = Arc::new(SqliteDB::new());
    let mut trie = EthTrie::new(memdb);

    trie.put(b"test", b"test").unwrap();
    trie.put(b"test1", b"test").unwrap();
    trie.put(b"test2", b"test").unwrap();
    trie.put(b"test23", b"test").unwrap();
    trie.put(b"test33", b"test").unwrap();
    trie.put(b"test44", b"test").unwrap();
    trie.commit().unwrap();

    let v = trie.get(b"test").unwrap();
    assert_eq!(Some(b"test".to_vec()), v);
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
    insert_full_branch();
    Ok(())
}