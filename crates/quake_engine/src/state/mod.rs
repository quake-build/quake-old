use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use miette::miette;
use nu_protocol::engine::Stack;
use nu_protocol::{CustomValue, ShellError, Span, Value};
use quake_core::prelude::*;
use serde::Serialize;

use crate::metadata::{BuildMetadata, Task};
use crate::{Result, QUAKE_SCOPE_VARIABLE_ID, QUAKE_VARIABLE_ID};

pub type ScopeId = usize;

#[derive(Clone, Debug)]
pub(crate) struct State {
    pub metadata: BuildMetadata,
    scopes: HashMap<ScopeId, Scope>,
    next_scope_id: ScopeId,
}

impl State {
    pub fn new() -> Self {
        State {
            metadata: BuildMetadata::new(),
            scopes: HashMap::new(),
            next_scope_id: 0,
        }
    }

    pub fn from_stack(stack: &Stack, span: Span) -> Result<Arc<Mutex<Self>>> {
        get_state(stack, span)
    }

    #[allow(dead_code)]
    pub fn get_scope(&self, stack: &Stack, span: Span) -> Result<&Scope> {
        self.scopes
            .get(&get_scope_id(stack, span)?)
            .ok_or_else(|| errors::UndefinedScope { span }.into())
    }

    pub fn get_scope_mut(&mut self, stack: &Stack, span: Span) -> Result<&mut Scope> {
        self.scopes
            .get_mut(&get_scope_id(stack, span)?)
            .ok_or_else(|| errors::UndefinedScope { span }.into())
    }

    pub fn push_scope(&mut self, scope: Scope, stack: &mut Stack, span: Span) -> ScopeId {
        // TODO error if nested scopes
        let scope_id = self.next_scope_id;
        self.scopes.insert(scope_id, scope);
        stack.add_var(
            QUAKE_SCOPE_VARIABLE_ID,
            Value::int(scope_id.try_into().unwrap(), span),
        );
        self.next_scope_id += 1;
        scope_id
    }

    pub fn pop_scope(&mut self, stack: &mut Stack, span: Span) -> Result<Scope> {
        let scope = self
            .scopes
            .remove(&get_scope_id(stack, span)?)
            .expect("Invalid scope ID");
        stack.add_var(QUAKE_SCOPE_VARIABLE_ID, Value::int(-1, Span::unknown()));
        Ok(scope)
    }
}

impl Serialize for State {
    fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        unimplemented!("serialize")
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) enum Scope {
    TaskDecl(Task),
}

#[derive(Clone, Debug)]
pub(crate) struct StateVariable(pub Arc<Mutex<State>>);

impl Serialize for StateVariable {
    fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        unimplemented!("serialize")
    }
}

impl CustomValue for StateVariable {
    fn clone_value(&self, span: Span) -> Value {
        Value::custom_value(Box::new(self.clone()), span)
    }

    fn value_string(&self) -> String {
        self.typetag_name().to_string()
    }

    fn to_base_value(&self, span: Span) -> std::result::Result<Value, ShellError> {
        // TODO implement this?
        Err(ShellError::GenericError(
            "`$quake` cannot be represented as a nushell value".to_owned(),
            String::new(),
            Some(span),
            None,
            Vec::new(),
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn typetag_name(&self) -> &'static str {
        "StateVariable" // TODO name?
    }

    fn typetag_deserialize(&self) {
        unimplemented!("typetag_deserialize")
    }
}

fn get_state(stack: &Stack, span: Span) -> Result<Arc<Mutex<State>>> {
    if let Value::CustomValue { val, .. } = stack.get_var(QUAKE_VARIABLE_ID, span)? {
        if let Some(state) = val.as_any().downcast_ref::<StateVariable>().cloned() {
            return Ok(state.0);
        }
    }

    Err(miette!("Could not fetch internal state"))
}

fn get_scope_id(stack: &Stack, span: Span) -> Result<ScopeId> {
    if let Value::Int { val, .. } = stack.get_var(QUAKE_SCOPE_VARIABLE_ID, span)? {
        val.try_into()
            .map_err(|_| miette!("Invalid scope ID: {val}"))
    } else {
        Err(errors::UndefinedScope { span }.into())
    }
}