//! OpSet sync
//! This should only run in --release mode (debug mode is too slow)

use cubby::memory::MemStore;

fn main() {
    let mut a = MemStore::new("alice").with_opset();
    let mut b = MemStore::new("bob");

    for _ in 0..1_000_000 {
        // Add 1000 random entries to A
        let mut key = [0u8; 16];
        let mut value = [0u8; 128];
        rand::fill(&mut key);
        rand::fill(&mut value);
        a.insert(key, value);
    }

    // OpSet sync from A => B
    let opset = a.take_opset();
    b.integrate_opset(opset);
    assert_eq!(a.entries(), b.entries())
}
