use std::fmt::Debug;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct PeerId(Bytes);

impl PeerId {
    pub fn from_str(id: &str) -> Self {
        PeerId(Bytes::copy_from_slice(id.as_bytes()))
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for PeerId {
    #[inline]
    fn from(eid: Vec<u8>) -> Self {
        PeerId(Bytes::from(eid))
    }
}

impl Debug for PeerId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
