//! Parsing helpers for fixture-only portable UI scenario setup.

use jisp_core::{Node, NodeKind};

use crate::effects::Capability;

use super::{UiTestActual, UiTestError, UiTestStep};

pub(super) fn parse_supports(name: &str, items: &[Node]) -> Result<UiTestStep, UiTestError> {
    if items.len() != 3 {
        return Err(UiTestError(format!(
            "{name}: supports expects a capability name and positive version"
        )));
    }
    let capability = items[1]
        .as_string()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| UiTestError(format!("{name}: supports capability name must be nonempty")))?;
    let NodeKind::Int(version) = &items[2].kind else {
        return Err(UiTestError(format!(
            "{name}: supports capability version must be a positive u32"
        )));
    };
    let version = u32::try_from(*version)
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(|| {
            UiTestError(format!(
                "{name}: supports capability version must be a positive u32"
            ))
        })?;
    Ok(UiTestStep::Supports {
        capability: Capability {
            name: capability.to_owned(),
            version,
        },
    })
}

pub(super) fn actual_accessor(node: &Node) -> Option<UiTestActual> {
    let items = node.as_form()?;
    if items.len() != 1 {
        return None;
    }
    match items[0].as_symbol() {
        Some("ui.test.state") => Some(UiTestActual::State),
        Some("ui.test.html") => Some(UiTestActual::Html),
        Some("ui.test.tree") => Some(UiTestActual::Tree),
        Some("ui.test.commands") => Some(UiTestActual::Commands),
        Some("ui.test.subscriptions") => Some(UiTestActual::Subscriptions),
        _ => None,
    }
}
