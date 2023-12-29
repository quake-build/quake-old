use std::sync::Arc;

use nu_protocol::{BlockId, Spanned, Value, VarId};
use serde::Serialize;

use quake_core::prelude::*;

pub type TaskId = usize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Metadata {
    tasks: Vec<Arc<Task>>,
}

impl Metadata {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn tasks(&self) -> &Vec<Arc<Task>> {
        &self.tasks
    }

    pub fn get_task(&self, id: TaskId) -> Option<&Arc<Task>> {
        self.tasks.get(id)
    }

    pub fn register_task(&mut self, task: Task) -> TaskId {
        self.tasks.push(Arc::new(task));
        self.tasks.len() - 1
    }

    pub fn global_tasks(&self) -> impl Iterator<Item = &Arc<Task>> {
        self.tasks.iter().filter(|t| t.kind == TaskKind::Global)
    }

    pub fn get_global_task(&self, name: &str) -> Result<&Arc<Task>> {
        Ok(&self.tasks[self.get_global_task_id(name)?])
    }

    pub fn get_global_task_id(&self, name: &str) -> Result<TaskId> {
        self.global_tasks()
            .filter_map(|t| t.name.as_ref())
            .position(|n| n.item == name)
            .ok_or_else(|| {
                errors::TaskNotFound {
                    name: name.to_owned(),
                }
                .into()
            })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Task {
    pub name: Option<Spanned<String>>,
    pub kind: TaskKind,
    pub dependencies: Vec<TaskId>,
    pub sources: Vec<Spanned<String>>,
    pub artifacts: Vec<Spanned<String>>,
    pub concurrent: bool,
    #[serde(skip)]
    pub(crate) run_block: Option<BlockId>,
    pub argument: Option<(VarId, Value)>,
}

impl Task {
    pub fn new(
        name: Spanned<String>,
        kind: TaskKind,
        run_block: Option<BlockId>,
        concurrent: bool,
    ) -> Self {
        Task {
            name: Some(name),
            kind,
            dependencies: Vec::new(),
            sources: Vec::new(),
            artifacts: Vec::new(),
            concurrent,
            run_block,
            argument: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum TaskKind {
    Global,
    Subtask,
}
