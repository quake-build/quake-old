use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

use nu_protocol::ast::Argument;
use nu_protocol::{BlockId, DeclId, Span, Spanned, Value, VarId};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::prelude::*;

pub type TaskId = usize;

pub type TaskCallId = usize;

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Metadata {
    tasks: Vec<Arc<Task>>,
    task_calls: Vec<Arc<RwLock<TaskCall>>>,
}

impl Metadata {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn task(&self) -> impl Iterator<Item = &Arc<Task>> {
        self.tasks.iter()
    }

    pub fn get_task(&self, task_id: TaskId) -> Option<&Arc<Task>> {
        self.tasks.get(task_id)
    }

    pub fn find_task(&self, name: &str, span: Option<Span>) -> Result<&Arc<Task>> {
        self.tasks
            .iter()
            .find(|t| t.name.item == name)
            .ok_or_else(|| {
                errors::TaskNotFound {
                    name: name.to_owned(),
                    span,
                }
                .into()
            })
    }

    pub fn find_task_id(&self, name: &str, span: Option<Span>) -> Result<TaskId> {
        self.tasks
            .iter()
            .position(|t| t.name.item == name)
            .ok_or_else(|| {
                errors::TaskNotFound {
                    name: name.to_owned(),
                    span,
                }
                .into()
            })
    }

    pub fn register_task(&mut self, name: String, task: impl Into<Arc<Task>>) -> Result<TaskId> {
        if let Ok(existing) = self.find_task(&name, None) {
            return Err(errors::TaskDuplicateDefinition {
                name,
                existing_span: existing.name.span,
            }
            .into());
        }

        let task_id = self.next_task_id();
        self.tasks.push(task.into());
        Ok(task_id)
    }

    pub fn next_task_id(&self) -> TaskId {
        self.tasks.len()
    }

    pub fn get_task_call(
        &self,
        call_id: TaskCallId,
    ) -> Option<impl Deref<Target = TaskCall> + 'static + Send + Sync> {
        self.task_calls.get(call_id).map(RwLock::read_arc)
    }

    /// Insert a task call with basic information provided.
    ///
    /// This will always result in a new task call ID, even if an otherwise
    /// identical one already exists, so that individual invocations are
    /// tracked.
    ///
    /// Returns `None` when there is no task for `task_id`.
    pub fn register_task_call(
        &mut self,
        task_id: TaskId,
        span: Span,
        arguments: Vec<Argument>,
        constants: Vec<(VarId, Value)>,
    ) -> Option<TaskCallId> {
        let _task = self.get_task(task_id)?;

        let entry = Arc::new(RwLock::new(TaskCall {
            task_id,
            span,
            arguments,
            constants,
            metadata: TaskCallMetadata::default(),
        }));

        self.task_calls.push(entry);
        Some(self.task_calls.len() - 1)
    }

    pub fn task_call_metadata(
        &self,
        call_id: TaskCallId,
    ) -> Option<impl Deref<Target = TaskCallMetadata> + '_ + Send + Sync> {
        Some(RwLockReadGuard::map(
            self.task_calls.get(call_id)?.read(),
            |c: &TaskCall| &c.metadata,
        ))
    }

    pub fn task_call_metadata_mut(
        &self,
        call_id: TaskCallId,
    ) -> Option<impl DerefMut<Target = TaskCallMetadata> + '_ + Send + Sync> {
        Some(RwLockWriteGuard::map(
            self.task_calls.get(call_id)?.write(),
            |c: &mut TaskCall| &mut c.metadata,
        ))
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Task {
    pub name: Spanned<String>,
    pub flags: TaskFlags,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub depends_decl_id: Option<DeclId>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub decl_body: Option<BlockId>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub run_body: Option<BlockId>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskFlags {
    pub concurrent: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskCall {
    pub task_id: TaskId,
    pub span: Span,
    pub arguments: Vec<Argument>,
    pub constants: Vec<(VarId, Value)>,
    pub metadata: TaskCallMetadata, // TODO box this as well
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskCallMetadata {
    pub dependencies: Vec<TaskCallId>,
    pub sources: Vec<PathBuf>,
    pub artifacts: Vec<PathBuf>,
}
