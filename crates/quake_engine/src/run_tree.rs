use std::collections::HashSet;

use quake_core::metadata::{Metadata, TaskCallId};

#[derive(Debug, Clone, PartialEq)]
pub struct RunNode {
    pub call_id: TaskCallId,
    pub children: Vec<RunNode>,
}

impl RunNode {
    pub fn new(call_id: TaskCallId) -> Self {
        Self {
            call_id,
            children: Vec::new(),
        }
    }

    /// Flatten the run tree in order of execution.
    pub fn flatten(&self) -> Vec<&Self> {
        let mut nodes = Vec::with_capacity(32);
        for child in &self.children {
            nodes.extend(child.flatten());
        }
        nodes.push(self);
        nodes
    }

    /// Locate a subtree within this tree.
    pub fn locate(&self, call_id: TaskCallId) -> Option<&Self> {
        if self.call_id == call_id {
            return Some(self);
        }

        for child in &self.children {
            if let Some(node) = child.locate(call_id) {
                return Some(node);
            }
        }

        None
    }
}

pub fn generate_run_tree(call_id: TaskCallId, metadata: &Metadata) -> RunNode {
    let mut included = HashSet::new();
    generate_run_tree_inner(call_id, metadata, &mut included)
}

fn generate_run_tree_inner(
    call_id: TaskCallId,
    metadata: &Metadata,
    included: &mut HashSet<TaskCallId>,
) -> RunNode {
    included.insert(call_id);

    let mut node = RunNode::new(call_id);

    let call = metadata.get_task_call(call_id).unwrap();

    for dep in &call.metadata.dependencies {
        if included.contains(dep) {
            continue;
        }

        node.children
            .push(generate_run_tree_inner(*dep, metadata, included));
    }

    node
}
