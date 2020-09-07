//! # The protocol for NetworkTables v4 (NT over WebSockets)
//!
//! This crate contains Rust representations of the messages described in the NTv4 specification.
//! These types are serialized with the relevant `serde` crates, and can be used to create a client or server compliant with the specification
//!
//! This crate does not prescribe a runtime that must be used, text messages are decoded as normal using serde_json,
//! and binary messages can be decoded from a normal byte slice. Implementation details of the runtime used are left to consumers of the crate.

mod bin;
mod text;

pub mod prelude {
    pub use crate::bin::*;
    pub use crate::text::*;
}
