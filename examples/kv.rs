//! The KV store is WIP. This example is not guaranteed to work.

use cubby::kv::KVStore;

fn main() {
    let mut a = KVStore::open(&":memory:").unwrap(); // todo: generate random tmp store
    let mut b = KVStore::open(&":memory:").unwrap(); // todo: generate random tmp store

    let mut a_txn = a.begin().unwrap();
    let mut b_txn = b.begin().unwrap();

    for _ in 0..1000 {
        // Add 1000 random entries to A
        let mut key = [0u8; 16];
        let mut value = [0u8; 128];
        rand::fill(&mut key);
        rand::fill(&mut value);
        a_txn.insert(&key, &value).unwrap();

        // Add 1000 random entries to B
        let mut key = [0u8; 16];
        let mut value = [0u8; 128];
        rand::fill(&mut key);
        rand::fill(&mut value);
        b_txn.insert(&key, &value).unwrap();
    }

    a_txn.commit().unwrap();
    b_txn.commit().unwrap();
}
