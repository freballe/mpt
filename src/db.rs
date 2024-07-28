use std::error::Error;
use rusqlite::{params, Connection, Result};
use crate::errors::MemDBError;

/// "DB" defines the "trait" of trie and database interaction.
/// You should first write the data to the cache and write the data
/// to the database in bulk after the end of a set of operations.
pub trait DB: Send + Sync {
    type Error: Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, bool>;

    /// Insert data into the cache.
    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), bool>;

    /// Remove data with given key.
    fn remove(&self, key: &[u8]) -> Result<(), bool>;

    /// Insert a batch of data into the cache.
    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), bool> {
        for i in 0..keys.len() {
            let key = &keys[i];
            let value = values[i].clone();
            self.insert(key, value)?;
        }
        Ok(())
    }

    /// Remove a batch of data into the cache.
    fn remove_batch(&self, keys: &[Vec<u8>]) -> Result<(), bool> {
        for key in keys {
            self.remove(key)?;
        }
        Ok(())
    }

    /// Flush data to the DB from the cache.
    fn flush(&self) -> Result<(), bool>;

    // #[cfg(test)]
    // fn len(&self) -> Result<usize, bool>;
    // #[cfg(test)]
    // fn is_empty(&self) -> Result<bool, bool>;
}

#[derive(Default, Debug)]
pub struct SqliteDB {
    db_name: String,
}

#[derive(Debug)]
struct NodeDB {
    key: Vec<u8>,
    data: Option<Vec<u8>>,
}

impl SqliteDB {
    pub fn new() -> Self {
        return SqliteDB {
            db_name: String::from("trie.db")
        }
    }
}

// TODO catch all errors
impl DB for SqliteDB {
    type Error = MemDBError;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, bool> {
        let conn = Connection::open(self.db_name.clone()).unwrap();

        _ = conn.execute(
            "CREATE TABLE trie (
                key BLOB PRIMARY KEY,
                data BLOB
            )",
            (), // empty list of parameters.
        );

        // let mut stmt = conn.prepare("SELECT key, data FROM trie").unwrap();
        // let node_iter = stmt.query_map([], |row| {
        //     Ok(NodeDB {
        //         key: row.get(0)?,
        //         data: row.get(1)?,
        //     })
        // }).unwrap();

        let mut stmt = conn.prepare("SELECT key, data FROM trie WHERE key=?1").unwrap();
        let node_iter = stmt.query_map([key], |row| {
            Ok(NodeDB {
                key: row.get(0)?,
                data: row.get(1)?,
            })
        }).unwrap();
        
        for node in node_iter {
            return Ok(node.unwrap().data.clone());
        }

        Ok(None)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), bool> {
        let conn = Connection::open(self.db_name.clone()).unwrap();

        _ = conn.execute(
            "CREATE TABLE trie (
                key BLOB PRIMARY KEY,
                data BLOB
            )",
            (), // empty list of parameters.
        );
        let node_to_add = NodeDB {
            key: key.to_vec(),
            data: Some(value),
        };
        _ = conn.execute(
            "INSERT INTO trie (key, data) VALUES (?1, ?2)",
            (&node_to_add.key, &node_to_add.data),
        );
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), bool> {
        let conn = Connection::open(self.db_name.clone()).unwrap();

        let mut stmt = conn.prepare("DELETE FROM trie WHERE key=?1").unwrap();
        stmt.execute([key.clone()]);
    
        Ok(())
    }

    fn flush(&self) -> Result<(),  bool> {
        Ok(())
    }
}