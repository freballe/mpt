use ethereum_types::H256;
use std::collections::HashMap;
use keccak_hash::{keccak, KECCAK_NULL_RLP};
use rlp::{Prototype, Rlp, RlpStream};

use crate::node::Node;

struct MPT {
    root_hash: H256,
    root: Node,
}

impl MPT {
    fn init() -> Self {
        return Self{root: Node::Empty, root_hash:keccak(vec![1])};
    }

    fn get(&self, key:Vec<u8>) -> Result<Vec<u8>, String>{
        if 1==1 {
            return Ok(key);
        }
        else {
            return Err(String::from("Couldn't retrieve"));
        }
    }

    fn put(&self, key:Vec<u8>, value:Vec<u8>){}

    fn del(&self, key:Vec<u8>){}

    fn commit(&self) -> Vec<u8> {
        let a:Vec<u8> = vec![244];
        return a;
    }

    fn proof(&self, key:Vec<u8>) -> Result<Vec<Vec<u8>>, String>{
        let eu: Vec<Vec<u8>> = vec![
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
    ];

        return Ok(eu);
    }
}

