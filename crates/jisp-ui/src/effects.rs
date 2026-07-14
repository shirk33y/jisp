//! Deterministic reference host for the planned data-only UI effect protocol.
//!
//! It deliberately performs no real side effects. Hosts can use it to prove
//! reconciliation, cancellation, completion, and disposal semantics for the
//! declarative resource values returned by a reducer.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

#[path = "effects_decode.rs"]
mod effects_decode;
pub use effects_decode::{decode_resources, DecodeError, ReconcileError};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    App,
    /// An identified component instance. This is a lifecycle identity only;
    /// component-local Jisp state is not implemented yet.
    Component {
        template: String,
        key: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub id: String,
    pub owner: Owner,
    pub capability: Capability,
    pub request: Value,
    pub replace: bool,
    pub on_ok: Option<ActionTemplate>,
    pub on_error: Option<ActionTemplate>,
}

/// A desired long-lived source of actions. It is reconciled independently from
/// a one-shot command, so the same `(owner, id)` may appear once in each kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Subscription {
    pub id: String,
    pub owner: Owner,
    pub capability: Capability,
    pub request: Value,
    pub replace: bool,
    pub on_ok: Option<ActionTemplate>,
    pub on_error: Option<ActionTemplate>,
}

/// The complete desired effect set produced by one future reducer turn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DesiredResources {
    pub commands: Vec<Command>,
    pub subscriptions: Vec<Subscription>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Capability {
    pub name: String,
    pub version: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Command,
    Subscription,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostErrorCode {
    UnsupportedCapability,
    PermissionDenied,
    InvalidRequest,
    Cancelled,
    HostFailure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostError {
    pub code: HostErrorCode,
    pub message: String,
}

/// A serializable completion passed back into the future reducer boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum Delivery {
    Ok(Value),
    Err(HostError),
}

/// Portable action data attached to a resource completion. It is not a host
/// callback: it creates a regular Jisp variant only after a live resource
/// delivers a value.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionTemplate {
    pub tag: String,
    pub fields: Vec<ActionTemplateField>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActionTemplateField {
    Literal(Value),
    Result,
    Error,
}

impl ActionTemplate {
    pub fn instantiate(&self, delivery: &Delivery) -> jisp_eval::Value {
        let fields = self
            .fields
            .iter()
            .map(|field| match field {
                ActionTemplateField::Literal(value) => jisp_value(value),
                ActionTemplateField::Result => match delivery {
                    Delivery::Ok(value) => jisp_value(value),
                    Delivery::Err(error) => jisp_value(&error_value(error)),
                },
                ActionTemplateField::Error => match delivery {
                    Delivery::Ok(value) => jisp_value(value),
                    Delivery::Err(error) => jisp_value(&error_value(error)),
                },
            })
            .collect();
        jisp_eval::Value::Variant {
            tag: self.tag.clone(),
            fields,
        }
    }
}

fn error_value(error: &HostError) -> Value {
    serde_json::json!({ "code": error_code(error.code.clone()), "message": error.message })
}

fn error_code(code: HostErrorCode) -> &'static str {
    match code {
        HostErrorCode::UnsupportedCapability => "unsupported-capability",
        HostErrorCode::PermissionDenied => "permission-denied",
        HostErrorCode::InvalidRequest => "invalid-request",
        HostErrorCode::Cancelled => "cancelled",
        HostErrorCode::HostFailure => "host-failure",
    }
}

fn jisp_value(value: &Value) -> jisp_eval::Value {
    match value {
        Value::Null => jisp_eval::Value::Null,
        Value::Bool(value) => jisp_eval::Value::Bool(*value),
        Value::Number(value) => value
            .as_i64()
            .map(jisp_eval::Value::Int)
            .or_else(|| value.as_f64().map(jisp_eval::Value::Float))
            .expect("effect JSON numbers are finite i64/f64"),
        Value::String(value) => jisp_eval::Value::string(value.as_str()),
        Value::Array(values) => jisp_eval::Value::List(values.iter().map(jisp_value).collect()),
        Value::Object(fields) => jisp_eval::Value::Obj(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), jisp_value(value)))
                .collect(),
        ),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trace {
    Start {
        kind: ResourceKind,
        owner: Owner,
        id: String,
        generation: u64,
    },
    Cancel {
        kind: ResourceKind,
        owner: Owner,
        id: String,
        generation: u64,
    },
    Deliver {
        kind: ResourceKind,
        owner: Owner,
        id: String,
        generation: u64,
        result: Delivery,
    },
    IgnoreLate {
        kind: ResourceKind,
        owner: Owner,
        id: String,
        generation: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Duplicate {
        kind: ResourceKind,
        owner: Owner,
        id: String,
    },
    UnsupportedCapability(Capability),
    ReplacementForbidden {
        kind: ResourceKind,
        owner: Owner,
        id: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate { kind, owner, id } => {
                write!(f, "duplicate {kind:?} `{id}` for {owner:?}")
            }
            Self::UnsupportedCapability(capability) => write!(
                f,
                "host does not support capability {}@{}",
                capability.name, capability.version
            ),
            Self::ReplacementForbidden { kind, owner, id } => {
                write!(f, "{kind:?} `{id}` for {owner:?} forbids replacement")
            }
        }
    }
}

impl std::error::Error for Error {}

type ResourceKey = (Owner, String);

#[derive(Default)]
pub struct FakeHost {
    capabilities: BTreeSet<Capability>,
    commands: BTreeMap<ResourceKey, Active<Command>>,
    subscriptions: BTreeMap<ResourceKey, Active<Subscription>>,
    next_generation: u64,
    pub trace: Vec<Trace>,
}

#[derive(Clone)]
struct Active<T> {
    resource: T,
    generation: u64,
}

trait Resource: Clone + PartialEq {
    const KIND: ResourceKind;

    fn id(&self) -> &str;
    fn owner(&self) -> &Owner;
    fn capability(&self) -> &Capability;
    fn replace(&self) -> bool;
}

impl Resource for Command {
    const KIND: ResourceKind = ResourceKind::Command;

    fn id(&self) -> &str {
        &self.id
    }

    fn owner(&self) -> &Owner {
        &self.owner
    }

    fn capability(&self) -> &Capability {
        &self.capability
    }

    fn replace(&self) -> bool {
        self.replace
    }
}

impl Resource for Subscription {
    const KIND: ResourceKind = ResourceKind::Subscription;

    fn id(&self) -> &str {
        &self.id
    }

    fn owner(&self) -> &Owner {
        &self.owner
    }

    fn capability(&self) -> &Capability {
        &self.capability
    }

    fn replace(&self) -> bool {
        self.replace
    }
}

impl FakeHost {
    pub fn with_capabilities(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            capabilities: capabilities.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Backwards-compatible command-only reconciliation. Existing
    /// subscriptions are retained; use [`Self::reconcile_resources`] for a
    /// reducer's whole desired set.
    pub fn reconcile(&mut self, desired: Vec<Command>) -> Result<(), Error> {
        self.reconcile_commands(desired)
    }

    pub fn reconcile_commands(&mut self, desired: Vec<Command>) -> Result<(), Error> {
        let next = self.validate(desired, &self.commands)?;
        Self::reconcile_map(
            &mut self.commands,
            next,
            &mut self.next_generation,
            &mut self.trace,
        );
        Ok(())
    }

    pub fn reconcile_subscriptions(&mut self, desired: Vec<Subscription>) -> Result<(), Error> {
        let next = self.validate(desired, &self.subscriptions)?;
        Self::reconcile_map(
            &mut self.subscriptions,
            next,
            &mut self.next_generation,
            &mut self.trace,
        );
        Ok(())
    }

    /// Reconciles both resource kinds atomically. A duplicate, unsupported
    /// capability, or forbidden replacement in either list leaves every active
    /// command/subscription and the trace unchanged.
    pub fn reconcile_resources(&mut self, desired: DesiredResources) -> Result<(), Error> {
        let commands = self.validate(desired.commands, &self.commands)?;
        let subscriptions = self.validate(desired.subscriptions, &self.subscriptions)?;
        Self::reconcile_map(
            &mut self.commands,
            commands,
            &mut self.next_generation,
            &mut self.trace,
        );
        Self::reconcile_map(
            &mut self.subscriptions,
            subscriptions,
            &mut self.next_generation,
            &mut self.trace,
        );
        Ok(())
    }

    /// Decode canonical Jisp resource descriptors and reconcile them as the
    /// app owner. This is the deterministic bridge used by host tests; it does
    /// not call a real capability provider.
    pub fn reconcile_declared_resources(
        &mut self,
        commands: &[jisp_eval::Value],
        subscriptions: &[jisp_eval::Value],
    ) -> Result<(), ReconcileError> {
        let desired = decode_resources(commands, subscriptions)?;
        self.reconcile_resources(desired)?;
        Ok(())
    }

    /// Delivers a command result if that exact generation remains current.
    /// A successful delivery does not implicitly remove the desired command:
    /// the next reducer result owns that decision.
    pub fn deliver_command(
        &mut self,
        owner: Owner,
        id: impl Into<String>,
        generation: u64,
        result: Delivery,
    ) -> bool {
        Self::record_delivery(
            &self.commands,
            &mut self.trace,
            ResourceKind::Command,
            owner,
            id.into(),
            generation,
            result,
        )
    }

    /// Deliver a command and, when it is still current, materialize its
    /// declared success/error template as the next reducer action.
    pub fn deliver_command_action(
        &mut self,
        owner: Owner,
        id: impl Into<String>,
        generation: u64,
        result: Delivery,
    ) -> Option<jisp_eval::Value> {
        let id = id.into();
        let template = self
            .commands
            .get(&(owner.clone(), id.clone()))
            .and_then(|active| match &result {
                Delivery::Ok(_) => active.resource.on_ok.clone(),
                Delivery::Err(_) => active.resource.on_error.clone(),
            });
        self.deliver_command(owner, id, generation, result.clone())
            .then(|| template.map(|template| template.instantiate(&result)))
            .flatten()
    }

    /// Compatibility helper for an empty successful command completion.
    pub fn deliver(&mut self, owner: Owner, id: impl Into<String>, generation: u64) -> bool {
        self.deliver_command(owner, id, generation, Delivery::Ok(Value::Null))
    }

    /// Subscriptions stay active after delivery and may produce many results.
    pub fn deliver_subscription(
        &mut self,
        owner: Owner,
        id: impl Into<String>,
        generation: u64,
        result: Delivery,
    ) -> bool {
        Self::record_delivery(
            &self.subscriptions,
            &mut self.trace,
            ResourceKind::Subscription,
            owner,
            id.into(),
            generation,
            result,
        )
    }

    /// Cancels every resource owned by precisely this app/component instance.
    /// It is idempotent, so unmount paths can call it exactly once without
    /// needing host-specific bookkeeping.
    pub fn dispose(&mut self, owner: &Owner) {
        Self::dispose_map(&mut self.commands, owner, &mut self.trace);
        Self::dispose_map(&mut self.subscriptions, owner, &mut self.trace);
    }

    fn validate<T: Resource>(
        &self,
        desired: Vec<T>,
        active: &BTreeMap<ResourceKey, Active<T>>,
    ) -> Result<BTreeMap<ResourceKey, T>, Error> {
        let mut next = BTreeMap::new();
        for resource in desired {
            let key = (resource.owner().clone(), resource.id().to_owned());
            if next.insert(key.clone(), resource).is_some() {
                return Err(Error::Duplicate {
                    kind: T::KIND,
                    owner: key.0,
                    id: key.1,
                });
            }
        }
        for resource in next.values() {
            if !self.capabilities.contains(resource.capability()) {
                return Err(Error::UnsupportedCapability(resource.capability().clone()));
            }
        }
        for (key, resource) in &next {
            if active
                .get(key)
                .is_some_and(|current| current.resource != *resource && !resource.replace())
            {
                return Err(Error::ReplacementForbidden {
                    kind: T::KIND,
                    owner: key.0.clone(),
                    id: key.1.clone(),
                });
            }
        }
        Ok(next)
    }

    fn reconcile_map<T: Resource>(
        active: &mut BTreeMap<ResourceKey, Active<T>>,
        next: BTreeMap<ResourceKey, T>,
        next_generation: &mut u64,
        trace: &mut Vec<Trace>,
    ) {
        let removed = active
            .keys()
            .filter(|key| !next.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in removed {
            Self::cancel(active, &key, trace);
        }
        for (key, resource) in next {
            if active
                .get(&key)
                .is_some_and(|current| current.resource == resource)
            {
                continue;
            }
            if active.contains_key(&key) {
                Self::cancel(active, &key, trace);
            }
            *next_generation += 1;
            let generation = *next_generation;
            trace.push(Trace::Start {
                kind: T::KIND,
                owner: key.0.clone(),
                id: key.1.clone(),
                generation,
            });
            active.insert(
                key,
                Active {
                    resource,
                    generation,
                },
            );
        }
    }

    fn record_delivery<T: Resource>(
        active: &BTreeMap<ResourceKey, Active<T>>,
        trace: &mut Vec<Trace>,
        kind: ResourceKind,
        owner: Owner,
        id: String,
        generation: u64,
        result: Delivery,
    ) -> bool {
        let key = (owner.clone(), id.clone());
        if active
            .get(&key)
            .is_some_and(|current| current.generation == generation)
        {
            trace.push(Trace::Deliver {
                kind,
                owner,
                id,
                generation,
                result,
            });
            true
        } else {
            trace.push(Trace::IgnoreLate {
                kind,
                owner,
                id,
                generation,
            });
            false
        }
    }

    fn dispose_map<T: Resource>(
        active: &mut BTreeMap<ResourceKey, Active<T>>,
        owner: &Owner,
        trace: &mut Vec<Trace>,
    ) {
        let keys = active
            .keys()
            .filter(|(active_owner, _)| active_owner == owner)
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            Self::cancel(active, &key, trace);
        }
    }

    fn cancel<T: Resource>(
        active: &mut BTreeMap<ResourceKey, Active<T>>,
        key: &ResourceKey,
        trace: &mut Vec<Trace>,
    ) {
        if let Some(current) = active.remove(key) {
            trace.push(Trace::Cancel {
                kind: T::KIND,
                owner: key.0.clone(),
                id: key.1.clone(),
                generation: current.generation,
            });
        }
    }
}

#[cfg(test)]
#[path = "effects_test.rs"]
mod effects_test;
