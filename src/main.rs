use rusqlite::{params, Connection, Result};
mod nibbles;
mod node;
mod tests;

mod db;
mod errors;
mod trie;
pub use db::{SqliteDB, DB};
pub use errors::{SqliteDBError, TrieError};
pub use trie::{EthTrie, ITrie};

use std::sync::Arc;
use hex::FromHex;
use rand::Rng;


#[derive(Debug)]
struct NodeDB {
    key: Vec<u8>,
    data: Option<Vec<u8>>,
}

fn insert_full_branch() {
    let memdb = Arc::new(SqliteDB::new(String::from("test1.db")));
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

fn test_proof_basic() {
    let db_name = String::from("test2.db");
    let memdb = Arc::new(SqliteDB::new(db_name.clone()));
    let mut trie = EthTrie::new(Arc::clone(&memdb));
    trie.put(b"doe", b"reindeer").unwrap();
    trie.put(b"dog", b"puppy").unwrap();
    trie.put(b"dogglesworth", b"cat").unwrap();
    let root = trie.commit().unwrap();
    let r = format!("0x{}", hex::encode(trie.commit().unwrap()));
    assert_eq!(
        r.as_str(),
        "0x8aad789dff2f538bca5d8ea56e8abe10f4c7ba3a5dea95fea4cd6e7c3a1168d3"
    );

    // proof of key exists
    let proof = trie.proof(b"doe").unwrap();
    let expected = vec![
        "e5831646f6a0db6ae1fda66890f6693f36560d36b4dca68b4d838f17016b151efe1d4c95c453",
        "f83b8080808080ca20887265696e6465657280a037efd11993cb04a54048c25320e9f29c50a432d28afdf01598b2978ce1ca3068808080808080808080",
    ];
    assert_eq!(
        proof
            .clone()
            .into_iter()
            .map(hex::encode)
            .collect::<Vec<_>>(),
        expected
    );
    let value = trie.verify_proof(root, b"doe", proof, db_name.clone()).unwrap();
    assert_eq!(value, Some(b"reindeer".to_vec()));

    // proof of key not exist
    let proof = trie.proof(b"dogg").unwrap();
    let expected = vec![
        "e5831646f6a0db6ae1fda66890f6693f36560d36b4dca68b4d838f17016b151efe1d4c95c453",
        "f83b8080808080ca20887265696e6465657280a037efd11993cb04a54048c25320e9f29c50a432d28afdf01598b2978ce1ca3068808080808080808080",
        "e4808080808080ce89376c6573776f72746883636174808080808080808080857075707079",
    ];
    assert_eq!(
        proof
            .clone()
            .into_iter()
            .map(hex::encode)
            .collect::<Vec<_>>(),
        expected
    );
    let value = trie.verify_proof(root, b"dogg", proof, db_name.clone()).unwrap();
    assert_eq!(value, None);

    // empty proof
    let proof = vec![];
    let value = trie.verify_proof(root, b"doe", proof, db_name.clone());
    assert!(value.is_err());

    // bad proof
    let proof = vec![b"aaa".to_vec(), b"ccc".to_vec()];
    let value = trie.verify_proof(root, b"doe", proof, db_name.clone());
    assert!(value.is_err());
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
    //test_proof_basic();
    Ok(())
}