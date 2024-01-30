use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use nu_protocol::engine::{EngineState, Stack};
use nu_protocol::{Span, Value};
use parking_lot::Mutex;
use serde::Serialize;

use quake_core::prelude::*;

use crate::metadata::{Metadata, TaskCallMetadata};
use crate::{Result, QUAKE_SCOPE_VARIABLE_ID, QUAKE_VARIABLE_ID};

pub mod metadata;

pub type ScopeId = usize;

/// Internal state for use by the [`Engine`](super::Engine) and commands.
///
/// Its primary purpose is to keep track of [`Task`]s and their associated signatures and blocks,
/// which are evaluated lazily by the engine.
///
/// This is stored as an `Arc<Mutex<State>>` inside the quake engine with the
/// [`VarId`](::nu_protocol::VarId) of [`QUAKE_VARIABLE_ID`](crate::QUAKE_VARIABLE_ID) so that it
/// can be fetched by commands while they are evaluating.
#[derive(Debug, Default)]
pub struct State {
    pub metadata: Metadata,
    scopes: BTreeMap<ScopeId, Scope>,
}

impl State {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_engine_state(engine_state: &EngineState) -> Arc<Mutex<Self>> {
        get_state(engine_state)
    }

    pub fn get_scope(&self, stack: &Stack, span: Span) -> Result<&Scope> {
        let id = get_scope_id(stack, span)?;
        self.scopes
            .get(&id)
            .ok_or_else(|| panic!("no scope registered with ID {id}"))
    }

    pub fn get_scope_mut(&mut self, stack: &Stack, span: Span) -> Result<&mut Scope> {
        let id = get_scope_id(stack, span)?;
        self.scopes
            .get_mut(&id)
            .ok_or_else(|| panic!("no scope registered with ID {id}"))
    }

    pub fn push_scope(&mut self, scope: Scope, stack: &mut Stack, span: Span) -> Result<ScopeId> {
        // error if nested scopes (i.e. if scope variable is non-negative)
        if let Ok(Value::Int { val, .. }) = stack.get_var(QUAKE_SCOPE_VARIABLE_ID, span)
            && !val.is_negative()
        {
            return Err(errors::NestedScopes { span }.into());
        }

        let scope_id = self.scopes.len();
        self.scopes.insert(scope_id, scope);

        stack.add_var(
            QUAKE_SCOPE_VARIABLE_ID,
            Value::int(scope_id.try_into().unwrap(), span),
        );

        Ok(scope_id)
    }

    pub fn pop_scope(&mut self, stack: &mut Stack, span: Span) -> Result<Scope> {
        let scope_id = get_scope_id(stack, span)?;
        let scope = self
            .scopes
            .remove(&scope_id)
            .expect("scope ID does not exist");
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

#[derive(Debug, Clone, Serialize)]
pub struct Scope {
    pub task: Arc<TaskCallMetadata>,
}

impl Scope {
    pub fn new(task: Arc<TaskCallMetadata>) -> Self {
        Self { task }
    }
}

fn get_state(engine_state: &EngineState) -> Arc<Mutex<State>> {
    if let Some(Value::CustomValue { val, .. }) = &engine_state.get_var(QUAKE_VARIABLE_ID).const_val
    {
        if let Some(state) = val
            .as_any()
            .downcast_ref::<crate::nu::types::State>()
            .cloned()
        {
            return state.0;
        }
    }

    panic!("could not fetch internal state")
}

fn get_scope_id(stack: &Stack, span: Span) -> Result<ScopeId> {
    if let Value::Int { val, .. } = stack.get_var(QUAKE_SCOPE_VARIABLE_ID, span)? {
        if let Ok(val) = val.try_into() {
            return Ok(val);
        }
    }

    Err(errors::UnknownScope { span }.into())
}
