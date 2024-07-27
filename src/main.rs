use rusqlite::{params, Connection, Result};

#[derive(Debug)]
struct NodeDB {
    key: i32,
    data: Option<Vec<u8>>,
}

fn main() -> Result<()> {
    //let conn =  Connection::open_in_memory()?;

    let conn = Connection::open("trie.db")?;

    _ = conn.execute(
        "CREATE TABLE trie (
            key   INTEGER PRIMARY KEY,
            data BLOB
        )",
        (), // empty list of parameters.
    );

    let me = NodeDB {
        key: 3,
        data: None,
    };
    _ = conn.execute(
        "INSERT INTO trie (key, data) VALUES (?1, ?2)",
        (&me.key, &me.data),
    )?;
    let me = NodeDB {
        key: 4,
        data: Some(vec![1,2,3,4]),
    };
    _ = conn.execute(
        "INSERT INTO trie (key, data) VALUES (?1, ?2)",
        (&me.key, &me.data),
    )?;

    let mut stmt = conn.prepare("SELECT key, data FROM trie")?;
    let node_iter = stmt.query_map([], |row| {
        Ok(NodeDB {
            key: row.get(0)?,
            data: row.get(1)?,
        })
    })?;

    for node in node_iter {
        println!("Found node {:?}", node.unwrap());
    }
    println!("Finished");
    Ok(())
}