use super::{
    exports::jisp::ui_capabilities::capabilities::{
        ErrorCode, Guest, NavigationRequest, StorageWriteRequest, TimerSleepRequest,
    },
    CapabilityHost,
};

#[test]
fn fixture_host_advertises_only_the_capabilities_it_implements() {
    let capabilities = <CapabilityHost as Guest>::supported();
    assert_eq!(
        capabilities
            .iter()
            .map(|capability| (capability.name.as_str(), capability.version))
            .collect::<Vec<_>>(),
        [("storage.write", 1), ("timer.sleep", 1)]
    );
}

#[test]
fn fixture_host_validates_requests_and_diagnoses_navigation() {
    assert!(
        <CapabilityHost as Guest>::storage_write(StorageWriteRequest {
            key: "draft:1".to_owned(),
            value_json: r#"{"title":"Plan"}"#.to_owned(),
        })
        .is_ok()
    );
    assert_eq!(
        <CapabilityHost as Guest>::storage_write(StorageWriteRequest {
            key: String::new(),
            value_json: "null".to_owned(),
        })
        .unwrap_err()
        .code,
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        <CapabilityHost as Guest>::timer_sleep(TimerSleepRequest { milliseconds: 0 })
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        <CapabilityHost as Guest>::navigate(NavigationRequest {
            target: "/settings".to_owned(),
            replace: false,
        })
        .unwrap_err()
        .code,
        ErrorCode::UnsupportedCapability
    );
}
