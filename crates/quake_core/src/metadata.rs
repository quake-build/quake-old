use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use nu_protocol::ast::Argument;
use nu_protocol::{BlockId, Signature, Span, Spanned};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::prelude::*;

pub type TaskStubId = usize;

pub type TaskCallId = usize;

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Metadata {
    task_calls: Vec<TaskCall>,
    task_stubs: Vec<TaskStub>,
}

impl Metadata {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn task_stubs(&self) -> impl Iterator<Item = &TaskStub> {
        self.task_stubs.iter()
    }

    pub fn get_task_call(&self, call_id: TaskCallId) -> Option<&TaskCall> {
        self.task_calls.get(call_id)
    }

    pub fn find_task_call(
        &self,
        name: &str,
        arguments: &[Argument],
    ) -> Result<(TaskCallId, &TaskCall)> {
        self.task_calls
            .iter()
            .enumerate()
            .find(|(_, call)| {
                self.task_stubs[call.task_id].name.item == name && call.arguments == arguments
            })
            .ok_or_else(|| {
                errors::TaskNotFound {
                    name: name.to_owned(),
                }
                .into()
            })
    }

    pub fn find_task_call_mut(
        &mut self,
        name: &str,
        arguments: &[Argument],
    ) -> Result<(TaskCallId, &mut TaskCall)> {
        self.task_calls
            .iter_mut()
            .enumerate()
            .find(|(_, call)| {
                self.task_stubs[call.task_id].name.item == name && call.arguments == arguments
            })
            .ok_or_else(|| {
                errors::TaskNotFound {
                    name: name.to_owned(),
                }
                .into()
            })
    }

    /// Insert a task call.
    ///
    /// This will always result in a new task call ID, even if an otherwise identical one already
    /// exists, so that individual invocations are tracked.
    ///
    /// For updating the metadata inside of a task call, see [`insert_task_call_metadata`]
    /// and [`clear_all_task_call_metadata`].
    ///
    /// ## Panics
    ///
    /// Panics when passed an invalid task ID.
    pub fn register_task_call(
        &mut self,
        task_id: TaskStubId,
        arguments: Vec<Argument>,
        span: Span,
    ) -> TaskCallId {
        assert!(task_id < self.task_stubs.len(), "invalid task_id");

        let entry = TaskCall {
            task_id,
            arguments,
            span,
            metadata: None,
        };

        self.task_calls.push(entry);
        self.task_calls.len() - 1
    }

    /// Insert (or update) metadata for a task call.
    ///
    /// ## Panics
    ///
    /// Panics if `call_id` is invalid.
    pub fn insert_task_call_metadata(
        &mut self,
        call_id: TaskCallId,
        metadata: Arc<TaskCallMetadata>,
    ) {
        assert!(call_id < self.task_calls.len(), "invalid call_id");
        self.task_calls[call_id].metadata = Some(metadata);
    }

    /// Recursively clear the metadata for a given call and for all of its dependencies.
    ///
    /// ## Panics
    ///
    /// Panics if `call_id` is invalid.
    pub fn clear_all_task_call_metadata(&mut self, call_id: TaskCallId) {
        assert!(call_id < self.task_calls.len(), "invalid call_id");

        let Some(dependencies) = self.task_calls[call_id]
            .metadata
            .clone()
            .map(|m| m.dependencies.clone())
        else {
            return;
        };
        for dep in dependencies {
            self.clear_all_task_call_metadata(dep);
        }

        self.task_calls[call_id].metadata = None;
    }

    pub fn get_task_stub(&self, task_id: TaskStubId) -> Option<&TaskStub> {
        self.task_stubs.get(task_id)
    }

    pub fn find_task_stub(&self, task_name: &str) -> Result<&TaskStub> {
        self.task_stubs
            .iter()
            .find(|t| t.name.item == task_name)
            .ok_or_else(|| {
                errors::TaskNotFound {
                    name: task_name.to_owned(),
                }
                .into()
            })
    }

    pub fn find_task_stub_id(&self, task_name: &str) -> Result<TaskStubId> {
        self.task_stubs
            .iter()
            .position(|t| t.name.item == task_name)
            .ok_or_else(|| {
                errors::TaskNotFound {
                    name: task_name.to_owned(),
                }
                .into()
            })
    }

    pub fn add_task_stub(&mut self, name: String, stub: TaskStub) -> Result<TaskStubId> {
        if let Ok(existing) = self.find_task_stub(&name) {
            return Err(errors::TaskDuplicateDefinition {
                name,
                existing_span: existing.name.span,
            }
            .into());
        }

        let task_id = self.task_stubs.len();
        self.task_stubs.push(stub);

        Ok(task_id)
    }

    pub fn add_task_stubs(&mut self, stubs: HashMap<String, TaskStub>) -> Result<()> {
        for (name, stub) in stubs {
            self.add_task_stub(name, stub)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskCall {
    pub task_id: TaskStubId,
    pub arguments: Vec<Argument>,
    pub span: Span,
    pub metadata: Option<Arc<TaskCallMetadata>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskCallMetadata {
    pub dependencies: Vec<TaskCallId>,
    pub sources: Vec<PathBuf>,
    pub artifacts: Vec<PathBuf>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskFlags {
    pub concurrent: bool,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskStub {
    pub name: Spanned<String>,
    pub flags: TaskFlags,
    pub signature: Box<Signature>,
    pub span: Span,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub decl_body: Option<BlockId>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub run_body: Option<BlockId>,
}
