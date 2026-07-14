//! Build-time conformance gate for the portable UI capability contract.
//!
//! The build script generates independent Rust and C guest bindings from the
//! same WIT world. The generated sources stay in `OUT_DIR`: they are evidence
//! of generator compatibility, not a hand-maintained second ABI.

#[cfg(test)]
mod tests {
    const RUST_BINDING: &str = include_str!(concat!(env!("OUT_DIR"), "/rust/jisp_ui_host.rs"));
    const C_BINDING: &str = include_str!(concat!(env!("OUT_DIR"), "/c/jisp_ui_host.c"));
    const C_HEADER: &str = include_str!(concat!(env!("OUT_DIR"), "/c/jisp_ui_host.h"));

    #[test]
    fn two_generated_host_bindings_preserve_the_capability_contract() {
        assert!(RUST_BINDING.contains("pub trait Guest"));
        assert!(RUST_BINDING.contains("storage_write"));
        assert!(RUST_BINDING.contains("timer_sleep"));
        assert!(RUST_BINDING.contains("navigate"));
        assert!(RUST_BINDING.contains("UnsupportedCapability"));
        assert!(RUST_BINDING.contains("PermissionDenied"));
        assert!(C_BINDING.contains("storage-write"));
        assert!(C_BINDING.contains("timer-sleep"));
        assert!(C_BINDING.contains("navigate"));
        assert!(C_HEADER.contains("exports_jisp_ui_capabilities_capabilities_supported"));
        assert!(C_HEADER.contains("storage_write"));
        assert!(C_HEADER.contains("timer_sleep"));
        assert!(C_HEADER.contains("navigate"));
        assert!(C_HEADER.contains("UNSUPPORTED_CAPABILITY"));
        assert!(C_HEADER.contains("PERMISSION_DENIED"));
    }
}
