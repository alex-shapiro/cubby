# Cubby

Synced KV store backed by an experimental [roaring bitmap](https://github.com/RoaringBitmap/roaring-rs)-based CRDT. Properties:

* transactions
* pull-based state sync
* push-based op sync
* no tombstones
* no garbage collection

## Basic Example

Basic state sync example:

```rs
let mut a = MemStore::new("alice");
let mut b = MemStore::new("bob");

// add 1000 random entries each to A and B
for _ in 0..1000 {
    let mut key = [0u8; 16];
    let mut value = [0u8; 128];
    rand::fill(&mut key);
    rand::fill(&mut value);
    a.insert(key, value);

        let mut key = [0u8; 16];
        let mut value = [0u8; 128];
        rand::fill(&mut key);
        rand::fill(&mut value);
        b.insert(key, value);
}

// full state sync from B => A
let request = a.request_diff();
let diff = b.build_diff(request);
a.integrate_diff(diff);

// full state sync from A => B
let request = b.request_diff();
let diff = a.build_diff(request);
b.integrate_diff(diff);

assert_eq!(a.entries(), b.entries())
```

You can find other examples in the `examples/` directory.

### Roadmap

- [x] In-Mememory Store
- [x] Transactions
- [x] State Sync
- [ ] Op Sync
- [ ] Persisted KV Store
- [ ] Persisted SQL Store
- [ ] Fuzzing
- [ ] Formal verification

### Why roaring bitmaps?

**TL;DR** Roaring bitmaps provide an efficient way to compress and compute over number sets, and logical clock accounting is mostly computing over number sets.

CRDT state sync can take on a few forms. Cubby uses a request-response form: peer A requests a state sync from peer B and receives all updates (inserts and deletes) as a response. The goal is to minimize the compute requirements and size of request & response while avoiding tombstones or garbage collection.

In the most naive approach, peer B transfers its entire state to A on each sync. This approach is simple but has the obvious downside of sending lots of data unnecessarily. Ideally, we want to send the minimum set of inserts and deletes that A needs from B.

How do we calculate this minimum set? The minimum set can be calculated from the set of all current logical clocks in A and B:

- Inserts: `e ⊂ (B - A) and e > A.max`
- Deletes: `e ⊂ (A - B) and e ≤ B.max`

If `A` can send its logical clocks to  `B`, then `B` can compute the minimum set of inserts and delete it must send to `A`.

Here, Cubby introduces roaring bitmaps. Each insert into the store is tagged with the author's peer ID and a hybrid logical clock (HLC). The set of all HLCs inserted by a particular peer is stored as a roaring bitmap. On each state sync, we send `Map<PeerId, RoaringBitmap>` from `A` to `B`.

Technically, the size of the roaring bitmap grows with the size of the elements in the data structure. In practice, it can be extremely small, often on the order of hundreds of bytes per sync. The small size makes it possible to run fast diffs over millions of entries, especially if entries are inserted in contiguous batches. Even in the worst case, the overhead is significantly more compressed than naive set arithmetic.

### Tradeoffs

This crate makes an explicit tradeoff: stateful sync in exchange for no tombstones or garbage collection. It can be a good choice if your write pattern involves frequent updates to a few million KV pairs, since this pattern can potentially generate far more tombstones than fresh entries.

While roaring bitmaps compress CRDT state, they do not eliminate it. If you need to sync billions of rows, you should test against your use case's write pattern. Concretely, if your use case is mostly inserts and few deletes, you may be better off with a tombstoned + GC'ed CRDT.
