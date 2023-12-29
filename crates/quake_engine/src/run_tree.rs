use std::collections::HashSet;

use crate::metadata::{Metadata, TaskId};

#[derive(Debug, Clone, PartialEq)]
pub struct RunNode {
    pub task_id: TaskId,
    pub children: Vec<RunNode>,
}

impl RunNode {
    pub fn new(id: TaskId) -> Self {
        Self {
            task_id: id,
            children: Vec::new(),
        }
    }

    /// Flatten the run tree in order of execution.
    pub fn flatten(&self) -> Vec<&RunNode> {
        let mut nodes = Vec::with_capacity(32);
        for child in &self.children {
            nodes.extend(child.flatten());
        }
        nodes.push(self);
        nodes
    }

    /// Locate a subtree within this tree.
    #[allow(dead_code)]
    pub fn locate(&self, task_id: TaskId) -> Option<&RunNode> {
        if self.task_id == task_id {
            return Some(self);
        }

        for child in &self.children {
            if let Some(node) = child.locate(task_id) {
                return Some(node);
            }
        }

        None
    }
}

pub fn generate_run_tree(task: TaskId, metadata: &Metadata) -> RunNode {
    let mut included = HashSet::new();
    generate_run_tree_inner(task, metadata, &mut included)
}

fn generate_run_tree_inner(
    task_id: TaskId,
    metadata: &Metadata,
    included: &mut HashSet<TaskId>,
) -> RunNode {
    included.insert(task_id);

    let mut node = RunNode::new(task_id);

    let task = metadata.get_task(task_id).expect("invalid task ID");
    for dep in &task.dependencies {
        if included.contains(dep) {
            continue;
        }

        node.children
            .push(generate_run_tree_inner(*dep, metadata, included));
    }

    node
}
