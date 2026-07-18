#[allow(dead_code)] // The shared runner also serves the separate parity integration test crate.
mod portable_support;

include!(concat!(env!("OUT_DIR"), "/portable_language_tests.rs"));
