//! Deterministic Component Model fixture for Jisp UI capabilities.
//!
//! This is a real `wasm32-wasip2` component implementation of the WIT host
//! world. It validates data at the ABI boundary but deliberately performs no
//! browser/native I/O; production hosts must provide their own permission and
//! persistence policy.

wit_bindgen::generate!({
    path: "../../wit",
    world: "jisp-ui-host",
});

use exports::jisp::ui_capabilities::capabilities::{
    Capability, ErrorCode, Guest, HostError, NavigationRequest, StorageWriteRequest,
    TimerSleepRequest,
};

struct CapabilityHost;

impl Guest for CapabilityHost {
    fn supported() -> Vec<Capability> {
        vec![
            Capability {
                name: "storage.write".to_owned(),
                version: 1,
            },
            Capability {
                name: "timer.sleep".to_owned(),
                version: 1,
            },
        ]
    }

    fn storage_write(request: StorageWriteRequest) -> Result<(), HostError> {
        if request.key.is_empty() {
            return Err(error(
                ErrorCode::InvalidRequest,
                "storage.write@1 requires a nonempty key",
            ));
        }
        serde_json::from_str::<serde_json::Value>(&request.value_json).map_err(|reason| {
            error(
                ErrorCode::InvalidRequest,
                format!("storage.write@1 requires JSON value-json: {reason}"),
            )
        })?;
        Ok(())
    }

    fn timer_sleep(request: TimerSleepRequest) -> Result<(), HostError> {
        if request.milliseconds == 0 {
            return Err(error(
                ErrorCode::InvalidRequest,
                "timer.sleep@1 requires milliseconds greater than zero",
            ));
        }
        Ok(())
    }

    fn navigate(_: NavigationRequest) -> Result<(), HostError> {
        Err(error(
            ErrorCode::UnsupportedCapability,
            "navigate@1 is not enabled by the deterministic fixture host",
        ))
    }
}

fn error(code: ErrorCode, message: impl Into<String>) -> HostError {
    HostError {
        code,
        message: message.into(),
    }
}

export!(CapabilityHost);

#[cfg(test)]
mod lib_test;
