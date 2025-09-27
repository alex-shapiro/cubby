# Cubby

Synced KV store backed by an experimental [roaring bitmap](https://github.com/RoaringBitmap/roaring-rs)-based CRDT. Properties:

* transactions
* pull-based state sync
* push-based op sync
* no tombstones
* no garbage collection

## Basic Example

Basic state sync example:

```rust
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
- [x] Mem Transactions
- [x] Mem State Sync
- [x] Mem Op Sync
- [ ] Formal verification
- [ ] Peer State LRU cache
- [ ] Persisted KV Store
- [ ] Persisted State sync
- [ ] Persisted Op sync
- [ ] Containerized SQL storage
- [ ] Persisted SQL Store

### Why roaring bitmaps?

**TL;DR** *Roaring bitmaps provide an efficient way to compress and compute over number sets, and logical clock accounting is mostly computing over number sets.*

CRDT state sync can take on a few forms. Cubby uses a request-response form: peer A requests a state sync from peer B and receives all updates (inserts and deletes) as a response. Cubby's goal is to minimize compute the requirements and size of request + response pair, while avoiding the complexity of tombstones and garbage collection.

The most naive approach is for peer B to transfer its entire state to A on each sync. However, to scale to large data stores we must sync the minimum set of inserts and deletes that A needs from B.

The minimum set can be found with the following logic:

- Inserts: `e ⊂ (B - A) and e > A.max`
- Deletes: `e ⊂ (A - B) and e ≤ B.max`

If `A` can send its logical clocks to  `B`, then `B` can compute the minimum set of inserts and deletes it must send to `A`.

Here, Cubby introduces roaring bitmaps. Each insert is tagged with the author's peer ID and a hybrid logical clock (HLC). The set of all HLCs inserted by a particular peer is stored as a roaring bitmap. On each state sync, we send `Map<PeerId, RoaringBitmap>` from `A` to `B` as a *diff request*.

Technically, the size of the roaring bitmap grows with the size of the elements in the data structure. In practice, it can be extremely small, often on the order of a few KB to sync millions of entries. The small size makes it possible to build diffs efficiently, especially if inserts are performed in contiguous batches. Even in the worst case, overhead is significantly more compressed than naive set arithmetic.

### Tradeoffs

This crate makes an explicit tradeoff: stateful sync in exchange for no tombstones or garbage collection. It can be a good choice if your write pattern involves frequent updates to a few million KV pairs, since this pattern can potentially generate far more tombstones than fresh entries.

While roaring bitmaps compress CRDT state, they do not eliminate it. If you need to sync billions of rows, you should test against your use case's write pattern. Concretely, if your use case is mostly inserts and few deletes, you may be better off with a tombstoned + GC'ed CRDT.

### TODO: Peer State LRU Cache

It may be possible to compress state sync further by LRU-caching peers' past sync states.

- A requests a diff from B
- B sends the diff
- A integrates the diff and caches B's state
- Next time A sends a diff request, it subtracts B's cached state from the request and adds a tuple: `(B_cached.max, B_cached.sha256)`
- B checks the sha256 against its own hash of `B[..=cached_max]`.
- If the hash is identical, it ignores `B[..=cached_max]` when building the diff
- If the hash is different, B requests a full sync request from A

Caching can significantly compress state sync in cases where peer state is large. However, any delete in `B[..=cached_max]` will bust the cache. It may more robust to hash individual bitmap containers instead of hashing the full bitmap once. TBD.

## License

Cubby is licensed under either of

* [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
* [MIT license](https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Cubby by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
