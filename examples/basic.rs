//! Basic, non-transactional state sync

use bitmap_crdt::memory::MemStore;

fn main() {
    let mut a = MemStore::new("alice");
    let mut b = MemStore::new("bob");

    for _ in 0..1000 {
        // Add 1000 random entries to A
        let mut key = [0u8; 16];
        let mut value = [0u8; 128];
        rand::fill(&mut key);
        rand::fill(&mut value);
        a.insert(key, value);

        // Add 1000 random entries to B
        let mut key = [0u8; 16];
        let mut value = [0u8; 128];
        rand::fill(&mut key);
        rand::fill(&mut value);
        b.insert(key, value);
    }

    // Full state sync from B => A
    // The sync request size is ~2KB
    let request = a.request_diff();
    assert!(request.index_size() <= 2200);
    let diff = b.build_diff(request);
    a.integrate_diff(diff);

    // Full state sync from A => B
    let request = b.request_diff();
    assert!(request.index_size() <= 2200);
    let diff = a.build_diff(request);
    b.integrate_diff(diff);

    assert_eq!(a.entries(), b.entries())
}
