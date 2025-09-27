#![doc = include_str!("../README.md")]

pub mod diff;
mod hlc;
#[cfg(feature = "kv")]
pub mod kv;
#[cfg(feature = "memory")]
pub mod memory;
pub mod opset;
mod peer_id;
