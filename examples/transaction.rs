//! 1 million transactional inserts with state sync
//! This should only run in --release mode (debug mode is too slow)

use cubby::memory::MemStore;

fn main() {
    let mut a = MemStore::new("alice");
    let mut b = MemStore::new("bob");

    let mut a_txn = a.begin();

    for _ in 0..1_000_000 {
        let mut key = [0u8; 16];
        let mut value = [0u8; 128];
        rand::fill(&mut key);
        rand::fill(&mut value);
        a_txn.insert(key, value);
    }

    a_txn.commit();

    // Full state sync from A => B
    // The sync request is only ~8 bytes, less than the 2KB seen in the `basic` example.
    // The size is smaller because transactional inserts are highly compressed in the bitmap index.
    let request = b.request_diff();
    assert_eq!(request.index_size(), 8, "{}", request.index_size());
    let diff = a.build_diff(request);
    b.integrate_diff(diff);

    assert_eq!(a.entries(), b.entries());
}
