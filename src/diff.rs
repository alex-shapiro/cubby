use std::collections::HashMap;

use roaring::RoaringTreemap;
use serde::{Deserialize, Serialize};

use crate::{hlc::Hlc, peer_id::PeerId};

#[derive(Serialize, Deserialize)]
pub struct DiffRequest(pub(crate) HashMap<PeerId, DiffRequestPeerState>);

#[derive(Serialize, Deserialize)]
pub struct DiffRequestPeerState {
    #[serde(skip_serializing_if = "RoaringTreemap::is_empty")]
    pub index: RoaringTreemap,
    pub bookmark: Hlc,
}

#[derive(Serialize, Deserialize)]
pub struct Diff<K, V>(pub(crate) HashMap<PeerId, DiffPeerState<K, V>>);

#[derive(Serialize, Deserialize)]
pub struct DiffPeerState<K, V> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inserts: Vec<Insert<K, V>>,
    #[serde(skip_serializing_if = "RoaringTreemap::is_empty")]
    pub deletes: RoaringTreemap,
    pub bookmark: Hlc,
}

#[derive(Serialize, Deserialize)]
pub struct Insert<K, V> {
    pub key: K,
    pub value: V,
    pub hlc: Hlc,
}

impl DiffRequest {
    /// Returns the index size, in bytes
    pub fn index_size(&self) -> usize {
        self.0.iter().map(|(_, state)| state.index_size()).sum()
    }
}

impl DiffRequestPeerState {
    /// Returns the index size, in bytes
    pub fn index_size(&self) -> usize {
        self.index.serialized_size()
    }
}
