//! Deterministic reference host for the planned data-only UI effect protocol.
//!
//! This is intentionally not wired into Jisp source syntax yet. It gives host
//! adapters one testable ownership/reconciliation contract before `update`
//! gains commands or subscriptions.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    App,
    Component { template: String, key: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub id: String,
    pub owner: Owner,
    pub capability: Capability,
    pub request: Value,
    pub replace: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Capability {
    pub name: String,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trace {
    Start {
        owner: Owner,
        id: String,
        generation: u64,
    },
    Cancel {
        owner: Owner,
        id: String,
        generation: u64,
    },
    Deliver {
        owner: Owner,
        id: String,
        generation: u64,
    },
    IgnoreLate {
        owner: Owner,
        id: String,
        generation: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Duplicate { owner: Owner, id: String },
    UnsupportedCapability(Capability),
    ReplacementForbidden { owner: Owner, id: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate { owner, id } => write!(f, "duplicate command `{id}` for {owner:?}"),
            Self::UnsupportedCapability(capability) => write!(
                f,
                "host does not support capability {}@{}",
                capability.name, capability.version
            ),
            Self::ReplacementForbidden { owner, id } => {
                write!(f, "command `{id}` for {owner:?} forbids replacement")
            }
        }
    }
}

impl std::error::Error for Error {}

#[derive(Default)]
pub struct FakeHost {
    capabilities: BTreeSet<Capability>,
    active: BTreeMap<(Owner, String), Active>,
    next_generation: u64,
    pub trace: Vec<Trace>,
}

#[derive(Clone)]
struct Active {
    command: Command,
    generation: u64,
}

impl FakeHost {
    pub fn with_capabilities(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            capabilities: capabilities.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Reconcile desired commands by `(owner, id)`. Equal commands are
    /// retained; removed commands cancel; changed commands replace only with
    /// explicit permission. Returned errors leave the current host untouched.
    pub fn reconcile(&mut self, desired: Vec<Command>) -> Result<(), Error> {
        let mut next = BTreeMap::new();
        for command in desired {
            let key = (command.owner.clone(), command.id.clone());
            if next.insert(key.clone(), command).is_some() {
                return Err(Error::Duplicate {
                    owner: key.0,
                    id: key.1,
                });
            }
        }
        for command in next.values() {
            if !self.capabilities.contains(&command.capability) {
                return Err(Error::UnsupportedCapability(command.capability.clone()));
            }
        }
        for (key, command) in &next {
            if let Some(active) = self.active.get(key) {
                if active.command != *command && !command.replace {
                    return Err(Error::ReplacementForbidden {
                        owner: key.0.clone(),
                        id: key.1.clone(),
                    });
                }
            }
        }
        let removed = self
            .active
            .keys()
            .filter(|key| !next.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in removed {
            self.cancel(&key);
        }
        for (key, command) in next {
            if self
                .active
                .get(&key)
                .is_some_and(|active| active.command == command)
            {
                continue;
            }
            if self.active.contains_key(&key) {
                self.cancel(&key);
            }
            self.next_generation += 1;
            let generation = self.next_generation;
            self.trace.push(Trace::Start {
                owner: key.0.clone(),
                id: key.1.clone(),
                generation,
            });
            self.active.insert(
                key,
                Active {
                    command,
                    generation,
                },
            );
        }
        Ok(())
    }

    pub fn deliver(&mut self, owner: Owner, id: impl Into<String>, generation: u64) -> bool {
        let id = id.into();
        let key = (owner.clone(), id.clone());
        if self
            .active
            .get(&key)
            .is_some_and(|active| active.generation == generation)
        {
            self.trace.push(Trace::Deliver {
                owner,
                id,
                generation,
            });
            true
        } else {
            self.trace.push(Trace::IgnoreLate {
                owner,
                id,
                generation,
            });
            false
        }
    }

    pub fn dispose(&mut self, owner: &Owner) {
        let keys = self
            .active
            .keys()
            .filter(|(active, _)| active == owner)
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            self.cancel(&key);
        }
    }

    fn cancel(&mut self, key: &(Owner, String)) {
        if let Some(active) = self.active.remove(key) {
            self.trace.push(Trace::Cancel {
                owner: key.0.clone(),
                id: key.1.clone(),
                generation: active.generation,
            });
        }
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::{Capability, Command, Error, FakeHost, Owner, Trace};

    fn storage() -> Capability {
        Capability {
            name: "storage.write".to_owned(),
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
        }
    }

    #[test]
    fn reconciles_replacement_cancellation_and_late_completion() {
        let mut host = FakeHost::with_capabilities([storage()]);
        host.reconcile(vec![command("one", true)]).unwrap();
        host.reconcile(vec![command("two", true)]).unwrap();

        assert_eq!(
            host.trace[0],
            Trace::Start {
                owner: Owner::App,
                id: "save:1".to_owned(),
                generation: 1
            }
        );
        assert_eq!(
            host.trace[1],
            Trace::Cancel {
                owner: Owner::App,
                id: "save:1".to_owned(),
                generation: 1
            }
        );
        assert!(!host.deliver(Owner::App, "save:1", 1));
        assert!(host.deliver(Owner::App, "save:1", 2));
    }

    #[test]
    fn rejects_duplicates_unsupported_capabilities_and_unapproved_replacement() {
        let mut host = FakeHost::with_capabilities([storage()]);
        assert!(matches!(
            host.reconcile(vec![command("one", true), command("two", true)]),
            Err(Error::Duplicate { .. })
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
            Err(Error::ReplacementForbidden { .. })
        ));
    }
}
