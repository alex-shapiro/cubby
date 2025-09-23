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
