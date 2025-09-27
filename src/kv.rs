use std::{collections::HashMap, io::Cursor, path::Path};

use bytes::Bytes;
use rand::distr::{Alphanumeric, SampleString};
use roaring::RoaringTreemap;
use rusqlite::{Connection, OptionalExtension};

use crate::hlc::Hlc;

static SCHEMA_SQL: &str = include_str!("schema.sql");

/// Persisted key value store backed by SQLite
pub struct KVStore {
    local: Peer,
    sqlite: Connection,
}

pub struct KVStoreTxn<'a> {
    sqlite: rusqlite::Transaction<'a>,
    local_id: i64,
    bookmark: &'a mut Hlc,
    inserts: RoaringTreemap,
    deletes: HashMap<i64, RoaringTreemap>,
}

struct Peer {
    id: i64,
    public_id: Bytes,
    bookmark: Hlc,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("mismatched local ID")]
    MismatchedLocalId,
    #[error("cannot deserialize bitmap")]
    CannotDeserializeBitmap,
}

impl KVStore {
    /// Opens a KVStore at the path, with a provided local ID
    pub fn open_with_local_id<P: AsRef<Path>>(path: &P, local_id: &[u8]) -> Result<Self, Error> {
        let sqlite = Connection::open(path)?;
        let local = setup(&sqlite, Some(local_id))?;
        Ok(KVStore { local, sqlite })
    }

    /// Opens a KVStore at the path.
    /// If the store is new, a random local ID will be assigned.
    pub fn open<P: AsRef<Path>>(path: &P) -> Result<Self, Error> {
        let sqlite = Connection::open(path)?;
        let local = setup(&sqlite, None)?;
        Ok(KVStore { local, sqlite })
    }

    /// Begins a transaction
    pub fn begin(&mut self) -> Result<KVStoreTxn<'_>, Error> {
        self.local.bookmark = self.local.bookmark.next();
        Ok(KVStoreTxn {
            sqlite: self.sqlite.transaction()?,
            local_id: self.local.id,
            bookmark: &mut self.local.bookmark,
            inserts: RoaringTreemap::new(),
            deletes: HashMap::default(),
        })
    }
}

impl KVStoreTxn<'_> {
    /// Get the value for a key
    pub fn get(&self, key: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(self
            .sqlite
            .query_row("SELECT value FROM entries WHERE key = ?", [key], |row| {
                row.get(0)
            })?)
    }

    /// Insert a key value pair into the store
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        // delete the old value at the key, if it exists
        self.delete(key)?;

        // generate an incremented HLC
        let peer_id = self.local_id;
        let hlc = self.bookmark.inc();
        *self.bookmark = hlc;
        self.inserts.insert(hlc.to_u64());

        // insert the new value
        self.sqlite.execute(
            "INSERT INTO entries (key, value, peer_id, hlc) VALUES (?1, ?2, ?3, ?4)",
            (key, value, peer_id, hlc.to_u64()),
        )?;

        Ok(())
    }

    /// Delete a key from the store
    pub fn delete(&mut self, key: &[u8]) -> Result<(), Error> {
        // remove the deleted entry if it exists
        let deleted_entry = self
            .sqlite
            .query_row(
                "DELETE FROM entries WHERE key = ? RETURNING peer_id, hlc",
                [key],
                |row| {
                    let old_peer_id: i64 = row.get(0)?;
                    let old_hlc: i64 = row.get(1)?;
                    Ok((old_peer_id, old_hlc))
                },
            )
            .optional()?;

        // mark the old value for `key` for deletion from peer state
        if let Some((peer_id, hlc)) = deleted_entry {
            let deletes = self.deletes.entry(peer_id).or_default();
            deletes.insert(hlc as u64);
        }
        Ok(())
    }

    /// Commit a series of inserts and deletes
    pub fn commit(mut self) -> Result<(), Error> {
        let sqlite: &Connection = &self.sqlite;

        // persist updated bookmark
        update_bookmark(sqlite, self.local_id, *self.bookmark)?;

        // update local bitmap
        if !self.inserts.is_empty() || self.deletes.contains_key(&self.local_id) {
            let mut local_bitmap = fetch_bitmap(&self.sqlite, self.local_id)?;
            local_bitmap |= self.inserts;
            if let Some(local_deletes) = self.deletes.remove(&self.local_id) {
                local_bitmap -= local_deletes;
            }
            if local_bitmap.is_empty() {
                delete_bitmap(sqlite, self.local_id)?;
            } else {
                upsert_bitmap(sqlite, self.local_id, &local_bitmap)?;
            }
        }

        // update or delete bitmaps from other peers
        for (peer_id, deletes) in self.deletes {
            let mut bitmap = fetch_bitmap(sqlite, peer_id)?;
            bitmap -= deletes;
            if bitmap.is_empty() {
                delete_bitmap(sqlite, peer_id)?;
            } else {
                upsert_bitmap(sqlite, peer_id, &bitmap)?;
            }
        }

        // commit changes in SQLite
        self.sqlite.commit()?;

        Ok(())
    }
}

fn random_public_id() -> Bytes {
    Alphanumeric
        .sample_string(&mut rand::rng(), 8)
        .into_bytes()
        .into()
}

/// Sets up the schema and local peer if necessary, returning the local peer
fn setup(sqlite: &Connection, public_id: Option<&[u8]>) -> Result<Peer, Error> {
    if schema_exists(sqlite)? {
        let local_peer_id = fetch_local_id(sqlite)?;
        let local_peer = fetch_peer(sqlite, local_peer_id)?;
        if let Some(public_id) = public_id
            && public_id != local_peer.public_id
        {
            return Err(Error::MismatchedLocalId);
        }
        Ok(local_peer)
    } else {
        let public_id = public_id
            .map(Bytes::copy_from_slice)
            .unwrap_or_else(random_public_id);
        let public_id_slice: &[u8] = &public_id;

        sqlite.execute_batch(SCHEMA_SQL)?;
        let id = sqlite.query_one(
            "INSERT INTO peers (public_id, bookmark) VALUES (?, 0) RETURNING id",
            [public_id_slice],
            |row| row.get(0),
        )?;
        Ok(Peer {
            id,
            public_id,
            bookmark: Hlc::from_u64(0),
        })
    }
}

/// Checks whether a schema exists
fn schema_exists(sqlite: &Connection) -> Result<bool, Error> {
    Ok(sqlite.query_row(
        "SELECT count(1) FROM sqlite_master WHERE name = 'metadata'",
        [],
        |r| r.get(0),
    )?)
}

/// Fetch the local ID
fn fetch_local_id(sqlite: &Connection) -> Result<i64, Error> {
    Ok(sqlite.query_one("SELECT local_id FROM metadata", [], |row| row.get(0))?)
}

/// Fetch a peer
fn fetch_peer(sqlite: &Connection, id: i64) -> Result<Peer, Error> {
    Ok(sqlite.query_one(
        "SELECT public_id, bookmark FROM peers WHERE id = ?",
        [id],
        |row| {
            let raw_public_id = row.get_ref(0)?.as_blob()?;
            let raw_hlc: i64 = row.get(1)?;
            Ok(Peer {
                id,
                public_id: Bytes::copy_from_slice(raw_public_id),
                bookmark: Hlc::from_u64(raw_hlc as u64),
            })
        },
    )?)
}

/// Fetch a peer bitmap
fn fetch_bitmap(sqlite: &Connection, peer_id: i64) -> Result<RoaringTreemap, Error> {
    sqlite
        .query_row(
            "SELECT state FROM bitmap_state WHERE peer_id = ?",
            [peer_id],
            |row| {
                let bytes = row.get_ref(0)?.as_blob()?;
                let cursor = Cursor::new(bytes);
                Ok(RoaringTreemap::deserialize_from(cursor)
                    .map_err(|_| Error::CannotDeserializeBitmap))
            },
        )
        .optional()?
        .unwrap_or_else(|| Ok(RoaringTreemap::default()))
}

/// Upsert a peer bitmap
fn upsert_bitmap(sqlite: &Connection, peer_id: i64, bitmap: &RoaringTreemap) -> Result<(), Error> {
    let mut bitmap_bytes = vec![];
    bitmap.serialize_into(&mut bitmap_bytes)?;
    sqlite.execute("INSERT INTO bitmap_state (peer_id, state) VALUES (?1, ?2) ON CONFLICT (peer_id) DO UPDATE SET state = ?2", (peer_id, &bitmap_bytes))?;
    Ok(())
}

/// Delete a peer bitmap
fn delete_bitmap(sqlite: &Connection, peer_id: i64) -> Result<(), Error> {
    sqlite.execute("DELETE FROM bitmap_state WHERE peer_id = ?", (peer_id,))?;
    Ok(())
}

/// Update a peer bookmark
fn update_bookmark(sqlite: &Connection, peer_id: i64, bookmark: Hlc) -> Result<(), Error> {
    let bookmark = bookmark.to_u64() as i64;
    sqlite.execute(
        "UPDATE peers SET bookmark = ?2 WHERE id = ?1",
        (peer_id, bookmark),
    )?;
    Ok(())
}
