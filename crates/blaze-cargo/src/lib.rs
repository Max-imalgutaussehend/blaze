//! A faster front-end for the everyday cargo commands.
//!
//! * [`fast`]: `build`, `check` and `run` skip cargo entirely when nothing relevant changed,
//!   using the same inputs cargo tracks (rustc dep-info, build-script `rerun-if-*` lines).
//! * [`add`]: `add` and `remove` edit `Cargo.toml` directly, resolving versions from the
//!   crates.io sparse index with a single request per crate.
//!
//! Anything not understood is forwarded to cargo unchanged.

pub mod add;
pub mod fast;
