use std::sync::{Arc, RwLock};

use ethereum_types::H256;
use hashbrown::{HashMap, HashSet};
use keccak_hash::{keccak, KECCAK_EMPTY, KECCAK_NULL_RLP};
use log::warn;
use rlp::{Prototype, Rlp, RlpStream};

use crate::db::{SqliteDB, DB};
use crate::errors::TrieError;
use crate::nibbles::Nibbles;
use crate::node::{empty_children, BranchNode, Node};

pub type TrieResult<T> = Result<T, TrieError>;
const HASHED_LENGTH: usize = 32;

use std::fs;
fn delete_file(path:String) -> std::io::Result<()> {
    fs::remove_file(path)?;
    Ok(())
}

pub trait ITrie<D: DB> {
    /// Returns the value for key stored in the trie.
    fn get(&self, key: &[u8]) -> TrieResult<Option<Vec<u8>>>;

    /// Inserts value into trie and modifies it if it exists
    fn put(&mut self, key: &[u8], value: &[u8]) -> ();

    /// Removes any existing value for key from the trie.
    fn del(&mut self, key: &[u8]) -> TrieResult<()>;

    /// Saves all the nodes in the db, clears the cache data, recalculates the root.
    /// Returns the root hash of the trie.
    fn commit(&mut self) -> H256;

    /// Prove constructs a merkle proof for key. The result contains all encoded nodes
    /// on the path to the value at key. The value itself is also included in the last
    /// node and can be retrieved by verifying the proof.
    ///
    /// If the trie does not contain a value for key, the returned proof contains all
    /// nodes of the longest existing prefix of the key (at least the root node), ending
    /// with the node that proves the absence of the key.
    // TODO refactor encode_raw() so that it doesn't need a &mut self
    fn proof(&mut self, key: &[u8]) -> TrieResult<Vec<Vec<u8>>>;    
}

#[derive(Debug)]
pub struct EthTrie<D>
where
    D: DB,
{
    root: Node,
    root_hash: H256,

    db: Arc<D>,

    // The batch of pending new nodes to write
    cache: HashMap<Vec<u8>, Vec<u8>>,
    passing_keys: HashSet<Vec<u8>>,
    gen_keys: HashSet<Vec<u8>>,
}

enum EncodedNode {
    Hash(H256),
    Inline(Vec<u8>),
}

#[derive(Clone, Debug)]
enum TraceStatus {
    Start,
    Doing,
    Child(u8),
    End,
}

#[derive(Clone, Debug)]
struct TraceNode {
    node: Node,
    status: TraceStatus,
}

impl TraceNode {
    fn advance(&mut self) {
        self.status = match &self.status {
            TraceStatus::Start => TraceStatus::Doing,
            TraceStatus::Doing => match self.node {
                Node::Branch(_) => TraceStatus::Child(0),
                _ => TraceStatus::End,
            },
            TraceStatus::Child(i) if *i < 15 => TraceStatus::Child(i + 1),
            _ => TraceStatus::End,
        }
    }
}

impl From<Node> for TraceNode {
    fn from(node: Node) -> TraceNode {
        TraceNode {
            node,
            status: TraceStatus::Start,
        }
    }
}

pub struct TrieIterator<'a, D>
where
    D: DB,
{
    trie: &'a EthTrie<D>,
    nibble: Nibbles,
    nodes: Vec<TraceNode>,
}

impl<'a, D> Iterator for TrieIterator<'a, D>
where
    D: DB,
{
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut now = self.nodes.last().cloned();
            if let Some(ref mut now) = now {
                self.nodes.last_mut().unwrap().advance();

                match (now.status.clone(), &now.node) {
                    (TraceStatus::End, node) => {
                        match *node {
                            Node::Leaf(ref leaf) => {
                                let cur_len = self.nibble.len();
                                self.nibble.truncate(cur_len - leaf.key.len());
                            }

                            Node::Extension(ref ext) => {
                                let cur_len = self.nibble.len();
                                self.nibble
                                    .truncate(cur_len - ext.read().unwrap().prefix.len());
                            }

                            Node::Branch(_) => {
                                self.nibble.pop();
                            }
                            _ => {}
                        }
                        self.nodes.pop();
                    }

                    (TraceStatus::Doing, Node::Extension(ref ext)) => {
                        self.nibble.extend(&ext.read().unwrap().prefix);
                        self.nodes.push((ext.read().unwrap().node.clone()).into());
                    }

                    (TraceStatus::Doing, Node::Leaf(ref leaf)) => {
                        self.nibble.extend(&leaf.key);
                        return Some((self.nibble.encode_raw().0, leaf.value.clone()));
                    }

                    (TraceStatus::Doing, Node::Branch(ref branch)) => {
                        let value_option = branch.read().unwrap().value.clone();
                        if let Some(value) = value_option {
                            return Some((self.nibble.encode_raw().0, value));
                        } else {
                            continue;
                        }
                    }

                    (TraceStatus::Doing, Node::Hash(ref hash_node)) => {
                        let node_hash = hash_node.hash;
                        if let Ok(n) = self.trie.recover_from_db(node_hash) {
                            self.nodes.pop();
                            match n {
                                Some(node) => self.nodes.push(node.into()),
                                None => {
                                    warn!("Trie node with hash {:?} is missing from the database. Skipping...", &node_hash);
                                    continue;
                                }
                            }
                        } else {
                            //error!();
                            return None;
                        }
                    }

                    (TraceStatus::Child(i), Node::Branch(ref branch)) => {
                        if i == 0 {
                            self.nibble.push(0);
                        } else {
                            self.nibble.pop();
                            self.nibble.push(i);
                        }
                        self.nodes
                            .push((branch.read().unwrap().children[i as usize].clone()).into());
                    }

                    (_, Node::Empty) => {
                        self.nodes.pop();
                    }
                    _ => {}
                }
            } else {
                return None;
            }
        }
    }
}

impl<D> EthTrie<D>
where
    D: DB,
{
    pub fn iter(&self) -> TrieIterator<D> {
        let nodes: Vec<TraceNode> = vec![(self.root.clone()).into()];
        TrieIterator {
            trie: self,
            nibble: Nibbles::from_raw(&[], false),
            nodes,
        }
    }
    pub fn new(db: Arc<D>) -> Self {
        Self {
            root: Node::Empty,
            root_hash: KECCAK_NULL_RLP.as_fixed_bytes().into(),

            cache: HashMap::new(),
            passing_keys: HashSet::new(),
            gen_keys: HashSet::new(),

            db,
        }
    }

    pub fn at_root(&self, root_hash: H256) -> Self {
        Self {
            root: Node::from_hash(root_hash),
            root_hash,

            cache: HashMap::new(),
            passing_keys: HashSet::new(),
            gen_keys: HashSet::new(),

            db: self.db.clone(),
        }
    }
}

impl<D> ITrie<D> for EthTrie<D>
where
    D: DB,
{
    /// Returns the value for key stored in the trie.
    fn get(&self, key: &[u8]) -> TrieResult<Option<Vec<u8>>> {
        let path = &Nibbles::from_raw(key, true);
        let result: Result<Option<Vec<u8>>, TrieError> = self.get_at(&self.root, path, 0);
        
        if let Err(TrieError::MissingTrieNode {
            node_hash,
            traversed,
            root_hash,
            err_key: _,
        }) = result
        {
            Err(TrieError::MissingTrieNode {
                node_hash,
                traversed,
                root_hash,
                err_key: Some(key.to_vec()),
            })
        } else {
            result
        }
    }

    /// Inserts value into trie and modifies it if it exists
    fn put(&mut self, key: &[u8], value: &[u8]) -> () {
        if value.is_empty() {
            self.del(key);
            return ();
        }
        let root = self.root.clone();
        let path = &Nibbles::from_raw(key, true);
        let result = self.insert_at(root, path, 0, value.to_vec());
        self.root = result.unwrap();       
    }

    /// Removes any existing value for key from the trie.
    fn del(&mut self, key: &[u8]) -> TrieResult<()> {
        let path = &Nibbles::from_raw(key, true);
        let result = self.delete_at(&self.root.clone(), path, 0);

        if let Err(TrieError::MissingTrieNode {
            node_hash,
            traversed,
            root_hash,
            err_key: _,
        }) = result
        {
            Err(TrieError::MissingTrieNode {
                node_hash,
                traversed,
                root_hash,
                err_key: Some(key.to_vec()),
            })
        } else {
            let (n, removed) = result.unwrap();
            self.root = n;
            Ok(())
        }
    }

    /// Saves all the nodes in the db, clears the cache data, recalculates the root.
    /// Returns the root hash of the trie.
    fn commit(&mut self) -> H256 {
        self.commit()
    }

    /// Prove constructs a merkle proof for key. The result contains all encoded nodes
    /// on the path to the value at key. The value itself is also included in the last
    /// node and can be retrieved by verifying the proof.
    ///
    /// If the trie does not contain a value for key, the returned proof contains all
    /// nodes of the longest existing prefix of the key (at least the root node), ending
    /// with the node that proves the absence of the key.
    fn proof(&mut self, key: &[u8]) -> TrieResult<Vec<Vec<u8>>> {
        let key_path = &Nibbles::from_raw(key, true);
        let result = self.get_path_at(&self.root, key_path, 0);

        if let Err(TrieError::MissingTrieNode {
            node_hash,
            traversed,
            root_hash,
            err_key: _,
        }) = result
        {
            Err(TrieError::MissingTrieNode {
                node_hash,
                traversed,
                root_hash,
                err_key: Some(key.to_vec()),
            })
        } else {
            let mut path = result?;
            match self.root {
                Node::Empty => {}
                _ => path.push(self.root.clone()),
            }
            Ok(path
                .into_iter()
                .rev()
                .map(|n| self.encode_raw(&n))
                .collect())
        }
    }
}

impl<D> EthTrie<D>
where
    D: DB,
{
    fn get_at(
        &self,
        source_node: &Node,
        path: &Nibbles,
        path_index: usize,
    ) -> TrieResult<Option<Vec<u8>>> {
        let partial = &path.offset(path_index);
        //println!("{:?} AAAA {:?}", partial, source_node);
        match source_node {
            Node::Empty => {
                Err(TrieError::MissingTrieNode {
                    node_hash: KECCAK_EMPTY,
                    traversed: Some(path.slice(0, path_index)),
                    root_hash: Some(self.root_hash),
                    err_key: None,
                })
                //Ok(None)
            }, //Ok(None),
            Node::Leaf(leaf) => {
                if &leaf.key == partial {
                    Ok(Some(leaf.value.clone()))
                } else {
                    Err(TrieError::MissingTrieNode {
                        node_hash: KECCAK_EMPTY,
                        traversed: Some(path.slice(0, path_index)),
                        root_hash: Some(self.root_hash),
                        err_key: None,
                    })
                    //Ok(None)
                }
            }
            Node::Branch(branch) => {
                let borrow_branch = branch.read().unwrap();

                if partial.is_empty() || partial.at(0) == 16 {
                    Ok(borrow_branch.value.clone())
                } else {
                    let index = partial.at(0);
                    self.get_at(&borrow_branch.children[index], path, path_index + 1)
                }
            }
            Node::Extension(extension) => {
                let extension = extension.read().unwrap();

                let prefix = &extension.prefix;
                let match_len = partial.common_prefix(prefix);
                if match_len == prefix.len() {
                    self.get_at(&extension.node, path, path_index + match_len)
                } else {
                    Err(TrieError::MissingTrieNode {
                        node_hash: KECCAK_EMPTY,
                        traversed: Some(path.slice(0, path_index)),
                        root_hash: Some(self.root_hash),
                        err_key: None,
                    })
                    //Ok(None)
                }
            }
            Node::Hash(hash_node) => {
                let node_hash = hash_node.hash;
                let node =
                    self.recover_from_db(node_hash)?
                        .ok_or_else(|| TrieError::MissingTrieNode {
                            node_hash,
                            traversed: Some(path.slice(0, path_index)),
                            root_hash: Some(self.root_hash),
                            err_key: None,
                        })?;
                self.get_at(&node, path, path_index)
            }
        }
    }

    fn insert_at(
        &mut self,
        n: Node,
        path: &Nibbles,
        path_index: usize,
        value: Vec<u8>,
    ) -> TrieResult<Node> {
        let partial = path.offset(path_index);
        match n {
            Node::Empty => Ok(Node::from_leaf(partial, value)),
            Node::Leaf(leaf) => {
                let old_partial = &leaf.key;
                let match_index = partial.common_prefix(old_partial);
                if match_index == old_partial.len() {
                    return Ok(Node::from_leaf(leaf.key.clone(), value));
                }

                let mut branch = BranchNode {
                    children: empty_children(),
                    value: None,
                };

                let n = Node::from_leaf(old_partial.offset(match_index + 1), leaf.value.clone());
                branch.insert(old_partial.at(match_index), n);

                let n = Node::from_leaf(partial.offset(match_index + 1), value);
                branch.insert(partial.at(match_index), n);

                if match_index == 0 {
                    return Ok(Node::Branch(Arc::new(RwLock::new(branch))));
                }

                // if include a common prefix
                Ok(Node::from_extension(
                    partial.slice(0, match_index),
                    Node::Branch(Arc::new(RwLock::new(branch))),
                ))
            }
            Node::Branch(branch) => {
                let mut borrow_branch = branch.write().unwrap();

                if partial.at(0) == 0x10 {
                    borrow_branch.value = Some(value);
                    return Ok(Node::Branch(branch.clone()));
                }

                let child = borrow_branch.children[partial.at(0)].clone();
                let new_child = self.insert_at(child, path, path_index + 1, value)?;
                borrow_branch.children[partial.at(0)] = new_child;
                Ok(Node::Branch(branch.clone()))
            }
            Node::Extension(ext) => {
                let mut borrow_ext = ext.write().unwrap();

                let prefix = &borrow_ext.prefix;
                let sub_node = borrow_ext.node.clone();
                let match_index = partial.common_prefix(prefix);

                if match_index == 0 {
                    let mut branch = BranchNode {
                        children: empty_children(),
                        value: None,
                    };
                    branch.insert(
                        prefix.at(0),
                        if prefix.len() == 1 {
                            sub_node
                        } else {
                            Node::from_extension(prefix.offset(1), sub_node)
                        },
                    );
                    let node = Node::Branch(Arc::new(RwLock::new(branch)));

                    return self.insert_at(node, path, path_index, value);
                }

                if match_index == prefix.len() {
                    let new_node =
                        self.insert_at(sub_node, path, path_index + match_index, value)?;
                    return Ok(Node::from_extension(prefix.clone(), new_node));
                }

                let new_ext = Node::from_extension(prefix.offset(match_index), sub_node);
                let new_node = self.insert_at(new_ext, path, path_index + match_index, value)?;
                borrow_ext.prefix = prefix.slice(0, match_index);
                borrow_ext.node = new_node;
                Ok(Node::Extension(ext.clone()))
            }
            Node::Hash(hash_node) => {
                let node_hash = hash_node.hash;
                self.passing_keys.insert(node_hash.as_bytes().to_vec());
                let node =
                    self.recover_from_db(node_hash)?
                        .ok_or_else(|| TrieError::MissingTrieNode {
                            node_hash,
                            traversed: Some(path.slice(0, path_index)),
                            root_hash: Some(self.root_hash),
                            err_key: None,
                        })?;
                self.insert_at(node, path, path_index, value)
            }
        }
    }

    fn delete_at(
        &mut self,
        old_node: &Node,
        path: &Nibbles,
        path_index: usize,
    ) -> TrieResult<(Node, bool)> {
        let partial = &path.offset(path_index);
        let (new_node, deleted) = match old_node {
            Node::Empty => Ok((Node::Empty, false)),
            Node::Leaf(leaf) => {
                if &leaf.key == partial {
                    return Ok((Node::Empty, true));
                }
                Ok((Node::Leaf(leaf.clone()), false))
            }
            Node::Branch(branch) => {
                let mut borrow_branch = branch.write().unwrap();

                if partial.at(0) == 0x10 {
                    borrow_branch.value = None;
                    return Ok((Node::Branch(branch.clone()), true));
                }

                let index = partial.at(0);
                let child = &borrow_branch.children[index];

                let (new_child, deleted) = self.delete_at(child, path, path_index + 1)?;
                if deleted {
                    borrow_branch.children[index] = new_child;
                }

                Ok((Node::Branch(branch.clone()), deleted))
            }
            Node::Extension(ext) => {
                let mut borrow_ext = ext.write().unwrap();

                let prefix = &borrow_ext.prefix;
                let match_len = partial.common_prefix(prefix);

                if match_len == prefix.len() {
                    let (new_node, deleted) =
                        self.delete_at(&borrow_ext.node, path, path_index + match_len)?;

                    if deleted {
                        borrow_ext.node = new_node;
                    }

                    Ok((Node::Extension(ext.clone()), deleted))
                } else {
                    Ok((Node::Extension(ext.clone()), false))
                }
            }
            Node::Hash(hash_node) => {
                let hash = hash_node.hash;
                self.passing_keys.insert(hash.as_bytes().to_vec());

                let node =
                    self.recover_from_db(hash)?
                        .ok_or_else(|| TrieError::MissingTrieNode {
                            node_hash: hash,
                            traversed: Some(path.slice(0, path_index)),
                            root_hash: Some(self.root_hash),
                            err_key: None,
                        })?;
                self.delete_at(&node, path, path_index)
            }
        }?;

        if deleted {
            Ok((self.degenerate(new_node)?, deleted))
        } else {
            Ok((new_node, deleted))
        }
    }

    // This refactors the trie after a node deletion, as necessary.
    // For example, if a deletion removes a child of a branch node, leaving only one child left, it
    // needs to be modified into an extension and maybe combined with its parent and/or child node.
    fn degenerate(&mut self, n: Node) -> TrieResult<Node> {
        match n {
            Node::Branch(branch) => {
                let borrow_branch = branch.read().unwrap();

                let mut used_indexs = vec![];
                for (index, node) in borrow_branch.children.iter().enumerate() {
                    match node {
                        Node::Empty => continue,
                        _ => used_indexs.push(index),
                    }
                }

                // if only a value node, transmute to leaf.
                if used_indexs.is_empty() && borrow_branch.value.is_some() {
                    let key = Nibbles::from_raw(&[], true);
                    let value = borrow_branch.value.clone().unwrap();
                    Ok(Node::from_leaf(key, value))
                // if only one node. make an extension.
                } else if used_indexs.len() == 1 && borrow_branch.value.is_none() {
                    let used_index = used_indexs[0];
                    let n = borrow_branch.children[used_index].clone();

                    let new_node = Node::from_extension(Nibbles::from_hex(&[used_index as u8]), n);
                    self.degenerate(new_node)
                } else {
                    Ok(Node::Branch(branch.clone()))
                }
            }
            Node::Extension(ext) => {
                let borrow_ext = ext.read().unwrap();

                let prefix = &borrow_ext.prefix;
                match borrow_ext.node.clone() {
                    Node::Extension(sub_ext) => {
                        let borrow_sub_ext = sub_ext.read().unwrap();

                        let new_prefix = prefix.join(&borrow_sub_ext.prefix);
                        let new_n = Node::from_extension(new_prefix, borrow_sub_ext.node.clone());
                        self.degenerate(new_n)
                    }
                    Node::Leaf(leaf) => {
                        let new_prefix = prefix.join(&leaf.key);
                        Ok(Node::from_leaf(new_prefix, leaf.value.clone()))
                    }
                    // try again after recovering node from the db.
                    Node::Hash(hash_node) => {
                        let node_hash = hash_node.hash;
                        self.passing_keys.insert(node_hash.as_bytes().to_vec());

                        let new_node =
                            self.recover_from_db(node_hash)?
                                .ok_or(TrieError::MissingTrieNode {
                                    node_hash,
                                    traversed: None,
                                    root_hash: Some(self.root_hash),
                                    err_key: None,
                                })?;

                        let n = Node::from_extension(borrow_ext.prefix.clone(), new_node);
                        self.degenerate(n)
                    }
                    _ => Ok(Node::Extension(ext.clone())),
                }
            }
            _ => Ok(n),
        }
    }

    // Get nodes path along the key, only the nodes whose encode length is greater than
    // hash length are added.
    // For embedded nodes whose data are already contained in their parent node, we don't need to
    // add them in the path.
    // In the code below, we only add the nodes get by `get_node_from_hash`, because they contains
    // all data stored in db, including nodes whose encoded data is less than hash length.
    fn get_path_at(
        &self,
        source_node: &Node,
        path: &Nibbles,
        path_index: usize,
    ) -> TrieResult<Vec<Node>> {
        let partial = &path.offset(path_index);
        match source_node {
            Node::Empty | Node::Leaf(_) => Ok(vec![]),
            Node::Branch(branch) => {
                let borrow_branch = branch.read().unwrap();

                if partial.is_empty() || partial.at(0) == 16 {
                    Ok(vec![])
                } else {
                    let node = &borrow_branch.children[partial.at(0)];
                    self.get_path_at(node, path, path_index + 1)
                }
            }
            Node::Extension(ext) => {
                let borrow_ext = ext.read().unwrap();

                let prefix = &borrow_ext.prefix;
                let match_len = partial.common_prefix(prefix);

                if match_len == prefix.len() {
                    self.get_path_at(&borrow_ext.node, path, path_index + match_len)
                } else {
                    Ok(vec![])
                }
            }
            Node::Hash(hash_node) => {
                let node_hash = hash_node.hash;
                let n = self
                    .recover_from_db(node_hash)?
                    .ok_or(TrieError::MissingTrieNode {
                        node_hash,
                        traversed: None,
                        root_hash: Some(self.root_hash),
                        err_key: None,
                    })?;
                let mut rest = self.get_path_at(&n, path, path_index)?;
                rest.push(n);
                Ok(rest)
            }
        }
    }

    fn commit(&mut self) -> H256 {
        let root_hash = match self.write_node(&self.root.clone()) {
            EncodedNode::Hash(hash) => hash,
            EncodedNode::Inline(encoded) => {
                let hash: H256 = keccak(&encoded).as_fixed_bytes().into();
                self.cache.insert(hash.as_bytes().to_vec(), encoded);
                hash
            }
        };

        let mut keys = Vec::with_capacity(self.cache.len());
        let mut values = Vec::with_capacity(self.cache.len());
        for (k, v) in self.cache.drain() {
            keys.push(k.to_vec());
            values.push(v);
        }

        self.db.insert_batch(keys, values);

        let removed_keys: Vec<Vec<u8>> = self
            .passing_keys
            .iter()
            .filter(|h| !self.gen_keys.contains(&h.to_vec()))
            .map(|h| h.to_vec())
            .collect();

        self.db.remove_batch(&removed_keys);

        self.root_hash = root_hash;
        self.gen_keys.clear();
        self.passing_keys.clear();
        self.root = self.recover_from_db(root_hash).unwrap().unwrap();
        root_hash
    }

    fn write_node(&mut self, to_encode: &Node) -> EncodedNode {
        // Returns the hash value directly to avoid double counting.
        if let Node::Hash(hash_node) = to_encode {
            return EncodedNode::Hash(hash_node.hash);
        }

        let data = self.encode_raw(to_encode);
        // Nodes smaller than 32 bytes are stored inside their parent,
        // Nodes equal to 32 bytes are returned directly
        if data.len() < HASHED_LENGTH {
            EncodedNode::Inline(data)
        } else {
            let hash: H256 = keccak(&data).as_fixed_bytes().into();
            self.cache.insert(hash.as_bytes().to_vec(), data);

            self.gen_keys.insert(hash.as_bytes().to_vec());
            EncodedNode::Hash(hash)
        }
    }

    fn encode_raw(&mut self, node: &Node) -> Vec<u8> {
        match node {
            Node::Empty => rlp::NULL_RLP.to_vec(),
            Node::Leaf(leaf) => {
                let mut stream = RlpStream::new_list(2);
                stream.append(&leaf.key.encode_compact());
                stream.append(&leaf.value);
                stream.out().to_vec()
            }
            Node::Branch(branch) => {
                let borrow_branch = branch.read().unwrap();

                let mut stream = RlpStream::new_list(17);
                for i in 0..16 {
                    let n = &borrow_branch.children[i];
                    match self.write_node(n) {
                        EncodedNode::Hash(hash) => stream.append(&hash.as_bytes()),
                        EncodedNode::Inline(data) => stream.append_raw(&data, 1),
                    };
                }

                match &borrow_branch.value {
                    Some(v) => stream.append(v),
                    None => stream.append_empty_data(),
                };
                stream.out().to_vec()
            }
            Node::Extension(ext) => {
                let borrow_ext = ext.read().unwrap();

                let mut stream = RlpStream::new_list(2);
                stream.append(&borrow_ext.prefix.encode_compact());
                match self.write_node(&borrow_ext.node) {
                    EncodedNode::Hash(hash) => stream.append(&hash.as_bytes()),
                    EncodedNode::Inline(data) => stream.append_raw(&data, 1),
                };
                stream.out().to_vec()
            }
            Node::Hash(_hash) => unreachable!(),
        }
    }

    fn decode_node(data: &[u8]) -> TrieResult<Node> {
        let r = Rlp::new(data);

        match r.prototype()? {
            Prototype::Data(0) => Ok(Node::Empty),
            Prototype::List(2) => {
                let key = r.at(0)?.data()?;
                let key = Nibbles::from_compact(key);

                if key.is_leaf() {
                    Ok(Node::from_leaf(key, r.at(1)?.data()?.to_vec()))
                } else {
                    let n = Self::decode_node(r.at(1)?.as_raw())?;

                    Ok(Node::from_extension(key, n))
                }
            }
            Prototype::List(17) => {
                let mut nodes = empty_children();
                #[allow(clippy::needless_range_loop)]
                for i in 0..nodes.len() {
                    let rlp_data = r.at(i)?;
                    let n = Self::decode_node(rlp_data.as_raw())?;
                    nodes[i] = n;
                }

                // The last element is a value node.
                let value_rlp = r.at(16)?;
                let value = if value_rlp.is_empty() {
                    None
                } else {
                    Some(value_rlp.data()?.to_vec())
                };

                Ok(Node::from_branch(nodes, value))
            }
            _ => {
                if r.is_data() && r.size() == HASHED_LENGTH {
                    let hash = H256::from_slice(r.data()?);
                    Ok(Node::from_hash(hash))
                } else {
                    Err(TrieError::InvalidData)
                }
            }
        }
    }

    fn recover_from_db(&self, key: H256) -> TrieResult<Option<Node>> {
        let node = match self
            .db
            .get(key.as_bytes())
            .map_err(|e| TrieError::SqliteDB(e.to_string()))?
        {
            Some(value) => Some(Self::decode_node(&value)?),
            None => None,
        };
        Ok(node)
    }
}
