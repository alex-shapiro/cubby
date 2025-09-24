use std::collections::{BTreeMap, BTreeSet, HashMap, btree_map};

use roaring::RoaringTreemap;

use crate::{
    diff::{Diff, DiffPeerState, DiffRequest, DiffRequestPeerState, Insert},
    hlc::Hlc,
    peer_id::PeerId,
};

pub struct MemStore<K, V> {
    local_id: PeerId,
    entries: BTreeMap<K, Entry<V>>,
    peers: HashMap<PeerId, PeerState<K>>,
}

pub struct MemStoreTxn<'a, K, V> {
    store: &'a mut MemStore<K, V>,
    inserts: BTreeMap<K, V>,
    deletes: BTreeSet<K>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Entries<'a, K, V>(&'a BTreeMap<K, Entry<V>>);

#[derive(Debug, PartialEq, Eq)]
struct Entry<V> {
    value: V,
    author: PeerId,
    hlc: Hlc,
}

struct PeerState<K> {
    index: RoaringTreemap,
    keys: HashMap<Hlc, K>,
    bookmark: Hlc,
}

impl<K> Default for PeerState<K> {
    fn default() -> Self {
        Self {
            index: Default::default(),
            keys: Default::default(),
            bookmark: Default::default(),
        }
    }
}

impl<K: Clone + Ord, V: Clone> MemStore<K, V> {
    /// Creates a new, empty CRDT
    pub fn new(id: &str) -> Self {
        let local_id = PeerId::from_str(id);
        let mut peers = HashMap::default();
        peers.insert(local_id.clone(), PeerState::default());
        MemStore {
            local_id,
            entries: BTreeMap::default(),
            peers,
        }
    }

    /// Returns the local peer ID
    pub fn id(&self) -> &str {
        let slice = self.local_id.as_slice();
        std::str::from_utf8(slice).expect("invalid local peer ID")
    }

    /// Returns the number of elements in the CRDT
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the CRDT contains no entries
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries<'a>(&'a self) -> Entries<'a, K, V> {
        Entries(&self.entries)
    }

    pub fn begin<'a>(&'a mut self) -> MemStoreTxn<'a, K, V> {
        MemStoreTxn {
            store: self,
            inserts: BTreeMap::default(),
            deletes: BTreeSet::default(),
        }
    }

    /// Inserts a key-value pair into the CRDT
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert_with_hlc(key, value, None)
    }

    // Private insert method
    // - if called inside of a transaction, expect a `txn_hlc`
    // - if called outside of a transaction, generate the HLC from the current bookmark
    fn insert_with_hlc(&mut self, key: K, value: V, txn_hlc: Option<Hlc>) -> Option<V> {
        // update peer state
        let peer_state = self.mut_local_peer_state();
        let hlc = if let Some(hlc) = txn_hlc {
            hlc
        } else {
            peer_state.bookmark.next()
        };
        peer_state.index.insert(hlc.to_u64());
        peer_state.keys.insert(hlc, key.clone());
        peer_state.bookmark = hlc;

        // update kv entries
        let entry = Entry {
            value,
            author: self.local_id.clone(),
            hlc,
        };

        let Some(old_entry) = self.entries.insert(key, entry) else {
            return None;
        };

        // update peer state for overwritten entry
        let peer_state = self
            .peers
            .get_mut(&old_entry.author)
            .expect("invalid peer state accounting");
        peer_state.index.remove(old_entry.hlc.to_u64());
        if old_entry.author != self.local_id {
            peer_state.keys.remove(&old_entry.hlc);
        }

        Some(old_entry.value)
    }

    fn mut_local_peer_state(&mut self) -> &mut PeerState<K> {
        self.peers
            .get_mut(&self.local_id)
            .expect("local peer state must always exist")
    }

    /// Removes a key from the CRDT, returning the value at the key if the key was previously in the CRDT.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let Some(old_entry) = self.entries.remove(key) else {
            return None;
        };

        let peer_state = self
            .peers
            .get_mut(&old_entry.author)
            .expect("invalid peer state accounting");

        peer_state.index.remove(old_entry.hlc.to_u64());
        peer_state.keys.remove(&old_entry.hlc);
        Some(old_entry.value)
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    /// Returns a diff request object
    pub fn request_diff(&self) -> DiffRequest {
        DiffRequest(
            self.peers
                .iter()
                .map(|(peer_id, state)| (peer_id.to_owned(), state.diff_request()))
                .collect(),
        )
    }

    /// Returns a diff from the request
    pub fn build_diff(&self, request: DiffRequest) -> Diff<K, V> {
        let mut diff_peer_states = HashMap::with_capacity(self.peers.len());

        for (peer_id, peer_state) in &self.peers {
            let mut diff_peer_state = DiffPeerState {
                inserts: Vec::default(),
                deletes: RoaringTreemap::new(),
                bookmark: peer_state.bookmark,
            };

            if let Some(request) = request.0.get(peer_id) {
                // inserts: all e ⊂ (local - remote) AND e > remote.max
                let mut insert_hlcs = &peer_state.index - &request.index;
                insert_hlcs.remove_range(0..=request.bookmark.to_u64());
                diff_peer_state.inserts = insert_hlcs
                    .iter()
                    .map(|hlc| -> _ {
                        let hlc = Hlc::from_u64(hlc);
                        let key = peer_state.keys.get(&hlc).expect("missing key for HLC");
                        let value = self.get(key).expect("missing value for key");
                        Insert {
                            key: key.to_owned(),
                            value: value.to_owned(),
                            hlc,
                        }
                    })
                    .collect();

                // deletes: all e ⊂ (remote - local) AND e ≤ local.max
                diff_peer_state.deletes = &request.index - &peer_state.index;
                diff_peer_state
                    .deletes
                    .remove_range(diff_peer_state.bookmark.to_u64()..);
            } else {
                // inserts: all e ⊂ local
                diff_peer_state.inserts = peer_state
                    .index
                    .iter()
                    .map(|hlc| {
                        let hlc = Hlc::from_u64(hlc);
                        let key = peer_state.keys.get(&hlc).expect("missing key for HLC");
                        let value = self.get(key).expect("missing value for key");
                        Insert {
                            key: key.to_owned(),
                            value: value.to_owned(),
                            hlc,
                        }
                    })
                    .collect();
            }

            if !diff_peer_state.inserts.is_empty() || diff_peer_state.deletes.is_empty() {
                diff_peer_states.insert(peer_id.clone(), diff_peer_state);
            }
        }

        Diff(diff_peer_states)
    }

    /// Integrates a diff into the local CRDT
    pub fn integrate_diff(&mut self, diff: Diff<K, V>) {
        let mut overwritten: HashMap<PeerId, Vec<Hlc>> = HashMap::default();

        // integrate deletes
        for (peer_id, diff_peer) in &diff.0 {
            if let Some(peer) = self.peers.get_mut(peer_id) {
                peer.index -= &diff_peer.deletes;
                for delete in &diff_peer.deletes {
                    if let Some(key) = peer.keys.remove(&Hlc::from_u64(delete)) {
                        self.entries.remove(&key);
                    }
                }
            }
        }

        // integrate inserts
        for (peer_id, diff_peer) in diff.0 {
            self.integrate_peer_inserts(peer_id, diff_peer, &mut overwritten);
        }
    }

    fn integrate_peer_inserts(
        &mut self,
        peer_id: PeerId,
        diff_peer: DiffPeerState<K, V>,
        overwritten: &mut HashMap<PeerId, Vec<Hlc>>,
    ) -> Hlc {
        let peer = self.peers.entry(peer_id.to_owned()).or_default();

        for insert in diff_peer.inserts {
            let did_insert = match self.entries.entry(insert.key.clone()) {
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(Entry {
                        value: insert.value,
                        author: peer_id.clone(),
                        hlc: insert.hlc,
                    });
                    true
                }
                btree_map::Entry::Occupied(mut entry) => {
                    // replace the old entry iff the new insert follows causally
                    let old = entry.get_mut();
                    let id = peer_id.clone();
                    if old.hlc < insert.hlc || old.hlc == insert.hlc && old.author < id {
                        let old = entry.insert(Entry {
                            value: insert.value,
                            author: id,
                            hlc: insert.hlc,
                        });
                        overwritten.entry(old.author).or_default().push(old.hlc);
                        true
                    } else {
                        false
                    }
                }
            };

            if did_insert {
                peer.index.insert(insert.hlc.to_u64());
                peer.keys.insert(insert.hlc, insert.key);
            }
        }

        peer.bookmark = peer.bookmark.max(diff_peer.bookmark);
        peer.bookmark
    }
}

impl<K> PeerState<K> {
    fn diff_request(&self) -> DiffRequestPeerState {
        DiffRequestPeerState {
            index: self.index.clone(),
            bookmark: self.bookmark,
        }
    }
}

impl<'a, K: Ord + Clone, V: Clone> MemStoreTxn<'a, K, V> {
    /// Inserts a key-value pair into the CRDT
    pub fn insert(&mut self, key: K, value: V) {
        self.inserts.insert(key, value);
    }

    /// Removes a key from the CRDT
    pub fn remove(&mut self, key: &K) {
        self.inserts.remove(key);
        self.deletes.insert(key.to_owned());
    }

    /// Aborts the transaction
    pub fn abort(self) {}

    /// Commits the transaction
    pub fn commit(self) {
        let mut hlc = self.store.mut_local_peer_state().bookmark.next();
        for (key, value) in self.inserts {
            self.store.insert_with_hlc(key, value, Some(hlc));
            hlc = hlc.inc();
        }
        for key in self.deletes {
            self.store.remove(&key);
        }
    }
}

impl<'a, K: Ord, V> Entries<'a, K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key).map(|entry| &entry.value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(key, entry)| (key, &entry.value))
    }
}
