# Roaring Bitmap CRDT

Synced KV store backed by [roaring bitmap](https://github.com/RoaringBitmap/roaring-rs) state CRDTs. Properties:

* pull-based state sync
* push-based op sync
* no tombstones
* no garbage collection

## Basic Example

The following

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

Incremental Op Sync:

```rs
let mut a = MemStore::new();
let mut b = MemStore::new();
let mut rng = rand::rng();

for _ in 0..1000 {
    let key: [u8; 16] = rng.generate();
    let val: [u8; 128] = rng.generate();
    let op = a.insert_with_op(key, val);
    b.integrate_op(op);
}

// A has sent B all of its entries
assert_eq!(a.entries(), b.entries());
```

Transactions:

```rs
let mut a = MemStore::new();
let mut b = MemStore::new():

let a_txn = a.begin();

for _ in 0..10000 {
    let key: [u8; 16] = rng.generate();
    let val: [u8; 128] = rng.generate();
    a.insert(key, val);
}

let ops = a.commit_with_ops();
b.integrate_ops(ops);
assert_eq!(a.entries(), b.entries());
```

The expected pattern is to pass ops incrementally in real time, occasionally running a state sync to ensure that offline or lost changes sync appropriately.

### Why roaring bitmaps?

Roaring bitmaps provide an efficient way to compress number sets, and logical clocks are mostly number sets. A single logical clock effectively consists of a tuple `(PeerId, Clock)`. A set of logical clocks can be represented as `(PeerId, ClockSet)`. The roaring bitmap serves as the `ClockSet` implementation.

Each roaring bitmap has the following structure for storing a set of `u64`s:

```rs
Map<u32, Map<u16, Container>>
```

- The first 32 bits are represented by the top-level key
- The next 16 bits are represented by the middle key
- The `Container` contains between 1 and `2^16` elements in the range prefixed by the 48 bits.

The `Container` is always *AT MOST* 8196 bytes. How do you store 64k integers in 8196 bytes? There are three possible strategies:

* For large sets of contiguous numbers, store ranges. Each range is 4 bytes: `(start: u16, end: u16)`.
* For small sets of sparse numbers, store a vector of ordered numbers. The vector can store at most 4096 elements.
* For large sets of sparse numbers, store a bitset. The container is guaranteed to be `8196` bytes, and each index represents the presence of a single number in the set.

With a traditional, fully contiguous logical clock per peer, one peer authoring 1 million edits will generate CRDT diff-checking overhead of roughly (8 * 1M / 16K) = 122 bytes. That overhead grows if their write pattern includes random deletes, but even in the worst case the overhead is significantly more compressed than a naive set of `u64`s.







- Range containers store contiguous ranges efficiently ()

contains many containers, and each container contains numbers in a `2^13`-sized range.



logical clocks because it efficiently compress number sets. A logical clock has two parts, the peer ID and the peer clock. Instead of

Each insert range is stored in an 8-byte (start, end) index. Thus it is possible to store a large database with no need for garbage collection and minimal state CRDT overhead.

This crate foregoes pure logical clocks for hybrid logical clocks (HLCs). HLC counters contain both a logical clock and a wall clock. Within a single transaction, all inserts are a dense range because the wall clock time is identical. Between transactions, each implementor can determine their own tradeoff of timestep granularity vs CRDT efficiency.

### Tradeoffs

This crate makes an explicit tradeoff: stateful sync in exchange for no tombstones or garbage collection. It can be a good choice if your write pattern involves frequent updates to <1 million KV pairs, since this pattern can potentially generate far more tombstones than fresh entries.

While roaring bitmaps compress CRDT state, they do not eliminate it. If you need to sync millions or billions of rows, you should test against your use case's write pattern. Concretely, if your use case is mostly inserts and few deletes, you may be better off with a tombstoned + GC'ed CRDT.

```rs
let mut map: Map<usize, String> = Map::new();

let mut kv_store: KVStore<usize, String> = KVStore::new("alex");


store.insert("Alice".to_string(), "{}")



```
