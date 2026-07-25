//! Test-only support code.
//!
//! The per-service test modules themselves are attached to the code they
//! exercise with `#[path = "…"] mod tests;` (see `src/client/*.rs`,
//! `src/session.rs`, `src/types/mod.rs`); only the shared helpers are declared
//! here so every one of them can `use crate::tests::common::*`.

pub(crate) mod common;
