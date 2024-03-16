use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use nu_protocol::engine::{EngineState, Stack};
use nu_protocol::{Span, Value};
use parking_lot::lock_api::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock};
use parking_lot::RwLock;
use serde::Serialize;

use quake_core::metadata::{Metadata, TaskCallId, TaskCallMetadata};
use quake_core::prelude::*;

use crate::nu::{QUAKE_SCOPE_VARIABLE_ID, QUAKE_VARIABLE_ID};

type ScopeId = usize;

/// Internal state for use by the [`Engine`](super::Engine) and commands.
///
/// Its primary purpose is to keep track of tasks and task calls (stored in the
/// [`Metadata`]) and their associated signatures and blocks, which are
/// evaluated lazily by the engine.
///
/// This is stored inside the nushell engine with the
/// [`VarId`](::nu_protocol::VarId) of
/// [`QUAKE_VARIABLE_ID`](crate::QUAKE_VARIABLE_ID) so that it can be fetched by
/// commands while they are evaluating.
#[derive(Debug, Default)]
pub struct State {
    pub metadata: Metadata,
    scopes: BTreeMap<ScopeId, Scope>,
}

impl State {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_engine_state(
        engine_state: &EngineState,
    ) -> ArcRwLockReadGuard<impl RawRwLock, Self> {
        get_state(engine_state).read_arc()
    }

    pub fn from_engine_state_mut(
        engine_state: &EngineState,
    ) -> ArcRwLockWriteGuard<impl RawRwLock, Self> {
        get_state(engine_state).write_arc()
    }

    pub fn check_in_scope(&self, stack: &Stack, span: Span) -> ShellResult<()> {
        get_scope_id(stack, span)?;
        Ok(())
    }

    pub fn scope_call_id(&self, stack: &Stack, span: Span) -> ShellResult<TaskCallId> {
        let id = get_scope_id(stack, span)?;
        Ok(self
            .scopes
            .get(&id)
            .unwrap_or_else(|| panic_bug!("invalid task call ID in scope"))
            .0)
    }

    pub fn scope_metadata(
        &self,
        stack: &Stack,
        span: Span,
    ) -> ShellResult<impl Deref<Target = TaskCallMetadata> + '_> {
        let call_id = self.scope_call_id(stack, span)?;
        Ok(self.metadata.task_call_metadata_mut(call_id).unwrap())
    }

    pub fn scope_metadata_mut(
        &self,
        stack: &Stack,
        span: Span,
    ) -> ShellResult<impl DerefMut<Target = TaskCallMetadata> + '_> {
        let call_id = self.scope_call_id(stack, span)?;
        Ok(self.metadata.task_call_metadata_mut(call_id).unwrap())
    }

    pub fn push_scope(
        &mut self,
        call_id: TaskCallId,
        stack: &mut Stack,
        span: Span,
    ) -> ShellResult<()> {
        // error if nested scopes (i.e. if scope variable is non-negative)
        if let Ok(Value::Int { val, .. }) = stack.get_var(QUAKE_SCOPE_VARIABLE_ID, span)
            && !val.is_negative()
        {
            return Err(errors::NestedScopes { span })
                .into_diagnostic()
                .into_shell_result();
        }

        let scope_id = self.scopes.len();
        self.scopes.insert(scope_id, Scope::new(call_id));

        stack.add_var(
            QUAKE_SCOPE_VARIABLE_ID,
            Value::int(scope_id.try_into().unwrap(), span),
        );

        Ok(())
    }

    pub fn pop_scope(&mut self, stack: &mut Stack, span: Span) -> ShellResult<()> {
        let scope_id = get_scope_id(stack, span)?;
        self.scopes
            .remove(&scope_id)
            .expect("scope ID does not exist");
        stack.add_var(QUAKE_SCOPE_VARIABLE_ID, Value::int(-1, Span::unknown()));
        Ok(())
    }

    pub fn peek_scope(&self, stack: &Stack, span: Span) -> ShellResult<TaskCallId> {
        let scope_id = get_scope_id(stack, span)?;
        Ok(self
            .scopes
            .get(&scope_id)
            .expect("scope ID does not exist")
            .0)
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

#[inline]
fn get_state(engine_state: &EngineState) -> Arc<RwLock<State>> {
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

    panic_bug!("could not fetch internal state")
}

#[inline]
fn get_scope_id(stack: &Stack, span: Span) -> ShellResult<ScopeId> {
    if let Ok(Value::Int { val, .. }) = stack.get_var(QUAKE_SCOPE_VARIABLE_ID, span) {
        if let Ok(val) = val.try_into() {
            return Ok(val);
        }
    }

    Err(errors::UnknownScope { span })
        .into_diagnostic()
        .into_shell_result()
}

#[derive(Debug, Clone)]
struct Scope(TaskCallId);

impl Scope {
    const fn new(call_id: TaskCallId) -> Self {
        Self(call_id)
    }
}
