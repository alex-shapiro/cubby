use std::collections::{HashMap, hash_map::Entry};

use roaring::RoaringTreemap;
use serde::{Deserialize, Serialize};

use crate::{diff::Insert, hlc::Hlc, peer_id::PeerId};

/// Op set for incremental diffs during a live connection
#[derive(Serialize, Deserialize)]
pub struct OpSet<K, V> {
    pub(crate) peer_id: PeerId,
    pub(crate) inserts: Vec<Insert<K, V>>,
    pub(crate) deletes: HashMap<PeerId, RoaringTreemap>,
}

impl<K, V> OpSet<K, V> {
    pub(crate) fn new(peer_id: PeerId) -> Self {
        OpSet {
            peer_id,
            inserts: Vec::default(),
            deletes: HashMap::default(),
        }
    }

    /// Adds an insert to the op set
    pub(crate) fn add_insert(&mut self, item: Insert<K, V>) {
        self.inserts.push(item);
    }

    /// Adds a delete to the op set
    pub(crate) fn add_delete(&mut self, peer_id: PeerId, hlc: Hlc) {
        self.deletes
            .entry(peer_id)
            .or_default()
            .insert(hlc.to_u64());
    }

    /// Merge one op set into another
    pub fn merge(&mut self, mut other: OpSet<K, V>) {
        self.inserts.append(&mut other.inserts);
        for (peer_id, other_treemap) in other.deletes {
            match self.deletes.entry(peer_id) {
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() = entry.get() | other_treemap;
                }
                Entry::Vacant(entry) => {
                    entry.insert(other_treemap);
                }
            }
        }
    }
}
