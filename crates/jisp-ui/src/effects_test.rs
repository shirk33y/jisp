use serde_json::json;

use super::{
    ActionTemplate, ActionTemplateField, Capability, Command, Delivery, DesiredResources, Error,
    FakeHost, HostError, HostErrorCode, Owner, ResourceKind, Subscription, Trace,
};

fn storage() -> Capability {
    Capability {
        name: "storage.write".to_owned(),
        version: 1,
    }
}

fn timer() -> Capability {
    Capability {
        name: "timer.sleep".to_owned(),
        version: 1,
    }
}

fn command(value: &str, replace: bool) -> Command {
    Command {
        id: "save:1".to_owned(),
        owner: Owner::App,
        capability: storage(),
        request: json!({ "value": value }),
        replace,
        on_ok: None,
        on_error: None,
    }
}

fn subscription(owner: Owner, milliseconds: u64, replace: bool) -> Subscription {
    Subscription {
        id: "refresh".to_owned(),
        owner,
        capability: timer(),
        request: json!({ "milliseconds": milliseconds }),
        replace,
        on_ok: None,
        on_error: None,
    }
}

#[test]
fn current_command_delivery_materializes_a_portable_action_template() {
    let mut host = FakeHost::with_capabilities([storage()]);
    let mut save = command("draft", true);
    save.on_ok = Some(ActionTemplate {
        tag: "Saved".to_owned(),
        fields: vec![
            ActionTemplateField::Literal(json!(42)),
            ActionTemplateField::Result,
        ],
    });
    save.on_error = Some(ActionTemplate {
        tag: "SaveFailed".to_owned(),
        fields: vec![ActionTemplateField::Error],
    });
    host.reconcile(vec![save]).unwrap();

    let ok = host
        .deliver_command_action(
            Owner::App,
            "save:1",
            1,
            Delivery::Ok(json!({ "etag": "v1" })),
        )
        .unwrap();
    assert!(matches!(
        ok,
        jisp_eval::Value::Variant { tag, fields }
            if tag == "Saved"
                && matches!(fields.as_slice(), [jisp_eval::Value::Int(42), jisp_eval::Value::Obj(value)] if matches!(value.get("etag"), Some(jisp_eval::Value::Str(text)) if text.as_ref() == "v1"))
    ));

    let error = host
        .deliver_command_action(
            Owner::App,
            "save:1",
            1,
            Delivery::Err(HostError {
                code: HostErrorCode::PermissionDenied,
                message: "readonly".to_owned(),
            }),
        )
        .unwrap();
    assert!(matches!(
        error,
        jisp_eval::Value::Variant { tag, fields }
            if tag == "SaveFailed"
                && matches!(fields.as_slice(), [jisp_eval::Value::Obj(value)] if matches!(value.get("code"), Some(jisp_eval::Value::Str(text)) if text.as_ref() == "permission-denied"))
    ));
}

#[test]
fn active_subscription_materializes_each_delivery_until_disposed() {
    let mut host = FakeHost::with_capabilities([timer()]);
    let mut refresh = subscription(Owner::App, 1000, true);
    refresh.on_ok = Some(ActionTemplate {
        tag: "Tick".to_owned(),
        fields: vec![ActionTemplateField::Result],
    });
    host.reconcile_subscriptions(vec![refresh]).unwrap();

    for tick in [1, 2] {
        let action = host
            .deliver_subscription_action(Owner::App, "refresh", 1, Delivery::Ok(json!(tick)))
            .unwrap();
        assert!(matches!(
            action,
            jisp_eval::Value::Variant { tag, fields }
                if tag == "Tick" && matches!(fields.as_slice(), [jisp_eval::Value::Int(value)] if *value == tick)
        ));
    }

    host.reconcile_subscriptions(vec![]).unwrap();
    assert!(host
        .deliver_subscription_action(Owner::App, "refresh", 1, Delivery::Ok(json!(3)))
        .is_none());
}

#[test]
fn reconciles_replacement_cancellation_and_late_completion() {
    let mut host = FakeHost::with_capabilities([storage()]);
    host.reconcile(vec![command("one", true)]).unwrap();
    host.reconcile(vec![command("two", true)]).unwrap();

    assert_eq!(
        host.trace[0],
        Trace::Start {
            kind: ResourceKind::Command,
            owner: Owner::App,
            id: "save:1".to_owned(),
            generation: 1
        }
    );
    assert_eq!(
        host.trace[1],
        Trace::Cancel {
            kind: ResourceKind::Command,
            owner: Owner::App,
            id: "save:1".to_owned(),
            generation: 1
        }
    );
    assert!(!host.deliver(Owner::App, "save:1", 1));
    assert!(host.deliver_command(
        Owner::App,
        "save:1",
        2,
        Delivery::Err(HostError {
            code: HostErrorCode::PermissionDenied,
            message: "storage disabled".to_owned(),
        })
    ));
    assert!(matches!(
        host.trace.last(),
        Some(Trace::Deliver {
            kind: ResourceKind::Command,
            result: Delivery::Err(HostError {
                code: HostErrorCode::PermissionDenied,
                ..
            }),
            ..
        })
    ));
}

#[test]
fn subscriptions_deliver_multiple_results_and_cancel_when_removed() {
    let mut host = FakeHost::with_capabilities([timer()]);
    host.reconcile_subscriptions(vec![subscription(Owner::App, 1000, true)])
        .unwrap();

    assert!(host.deliver_subscription(
        Owner::App,
        "refresh",
        1,
        Delivery::Ok(json!({ "tick": 1 }))
    ));
    assert!(host.deliver_subscription(
        Owner::App,
        "refresh",
        1,
        Delivery::Ok(json!({ "tick": 2 }))
    ));
    host.reconcile_subscriptions(vec![]).unwrap();
    assert!(!host.deliver_subscription(
        Owner::App,
        "refresh",
        1,
        Delivery::Ok(json!({ "tick": 3 }))
    ));

    assert_eq!(
        host.trace
            .iter()
            .filter(|entry| matches!(
                entry,
                Trace::Deliver {
                    kind: ResourceKind::Subscription,
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(matches!(
        host.trace[3],
        Trace::Cancel {
            kind: ResourceKind::Subscription,
            generation: 1,
            ..
        }
    ));
}

#[test]
fn resource_reconciliation_is_atomic_across_commands_and_subscriptions() {
    let mut host = FakeHost::with_capabilities([storage(), timer()]);
    host.reconcile_resources(DesiredResources {
        commands: vec![command("one", true)],
        subscriptions: vec![subscription(Owner::App, 1000, true)],
    })
    .unwrap();
    let trace_before = host.trace.clone();

    let unsupported = Subscription {
        capability: Capability {
            name: "network.fetch".to_owned(),
            version: 1,
        },
        ..subscription(Owner::App, 500, true)
    };
    assert!(matches!(
        host.reconcile_resources(DesiredResources {
            commands: vec![command("two", true)],
            subscriptions: vec![unsupported],
        }),
        Err(Error::UnsupportedCapability(_))
    ));
    assert_eq!(host.trace, trace_before);
    assert!(host.deliver(Owner::App, "save:1", 1));
    assert!(host.deliver_subscription(Owner::App, "refresh", 2, Delivery::Ok(json!(null))));
}

#[test]
fn disposing_a_component_cancels_each_owned_resource_once() {
    let owner = Owner::Component {
        template: "todo-row".to_owned(),
        key: "42".to_owned(),
    };
    let mut host = FakeHost::with_capabilities([storage(), timer()]);
    host.reconcile_resources(DesiredResources {
        commands: vec![Command {
            owner: owner.clone(),
            ..command("row", true)
        }],
        subscriptions: vec![subscription(owner.clone(), 1000, true)],
    })
    .unwrap();

    host.dispose(&owner);
    host.dispose(&owner);

    let cancelled = host
        .trace
        .iter()
        .filter(|entry| {
            matches!(entry, Trace::Cancel { owner: cancelled_owner, .. } if cancelled_owner == &owner)
        })
        .count();
    assert_eq!(cancelled, 2);
    assert!(!host.deliver(Owner::App, "save:1", 1));
    assert!(!host.deliver_subscription(owner, "refresh", 2, Delivery::Ok(json!(null))));
}

#[test]
fn rejects_duplicates_unsupported_capabilities_and_unapproved_replacement() {
    let mut host = FakeHost::with_capabilities([storage()]);
    assert!(matches!(
        host.reconcile(vec![command("one", true), command("two", true)]),
        Err(Error::Duplicate {
            kind: ResourceKind::Command,
            ..
        })
    ));
    let unsupported = Command {
        capability: Capability {
            name: "network.fetch".to_owned(),
            version: 1,
        },
        ..command("one", true)
    };
    assert!(matches!(
        host.reconcile(vec![unsupported]),
        Err(Error::UnsupportedCapability(_))
    ));
    host.reconcile(vec![command("one", true)]).unwrap();
    assert!(matches!(
        host.reconcile(vec![command("two", false)]),
        Err(Error::ReplacementForbidden {
            kind: ResourceKind::Command,
            ..
        })
    ));
}
