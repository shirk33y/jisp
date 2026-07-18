#[allow(dead_code)] // The shared runner also serves the existing per-syntax integration test crate.
mod portable_support;

include!(concat!(env!("OUT_DIR"), "/portable_syntax_parity_tests.rs"));
