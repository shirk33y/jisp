use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{RuntimeError, Value};

#[derive(Clone)]
pub struct Env(Rc<Frame>);

struct Frame {
    parent: Option<Env>,
    values: RefCell<HashMap<String, Rc<RefCell<Value>>>>,
}

impl Env {
    pub fn root() -> Self {
        Self(Rc::new(Frame {
            parent: None,
            values: RefCell::new(HashMap::new()),
        }))
    }

    pub fn child(&self) -> Self {
        Self(Rc::new(Frame {
            parent: Some(self.clone()),
            values: RefCell::new(HashMap::new()),
        }))
    }

    pub fn define(&self, name: impl Into<String>, value: Value) {
        self.0
            .values
            .borrow_mut()
            .insert(name.into(), Rc::new(RefCell::new(value)));
    }

    pub fn define_placeholder(&self, name: impl Into<String>) {
        let name = name.into();
        self.define(name.clone(), Value::Uninitialized(name));
    }

    pub fn assign(&self, name: &str, value: Value) -> Result<(), RuntimeError> {
        let cell = self
            .lookup_cell(name)
            .ok_or_else(|| RuntimeError::message(format!("unknown binding `{name}`")))?;
        *cell.borrow_mut() = value;
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Result<Value, RuntimeError> {
        let cell = self
            .lookup_cell(name)
            .ok_or_else(|| RuntimeError::message(format!("unknown name `{name}`")))?;
        let value = cell.borrow().clone();
        match value {
            Value::Uninitialized(name) => Err(RuntimeError::message(format!(
                "recursive binding `{name}` was used before initialization"
            ))),
            other => Ok(other),
        }
    }

    fn lookup_cell(&self, name: &str) -> Option<Rc<RefCell<Value>>> {
        if let Some(value) = self.0.values.borrow().get(name) {
            return Some(value.clone());
        }
        self.0.parent.as_ref()?.lookup_cell(name)
    }
}
