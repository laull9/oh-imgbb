use std::collections::HashMap;

use crate::engine::action::ActionKind;
use crate::engine::task::{Task, TaskId, TaskStatus};

/// TaskNode 保存任务图中的单个任务节点。
#[derive(Clone, Debug)]
pub struct TaskNode {
    pub task: Task,
}

/// TaskEdge 保存任务之间由行为产生的扩展关系。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEdge {
    pub from: TaskId,
    pub to: TaskId,
    pub action_kind: ActionKind,
}

/// TaskGraph 保存动态生长的任务执行图。
#[derive(Clone, Debug)]
pub struct TaskGraph {
    nodes: HashMap<TaskId, TaskNode>,
    edges: Vec<TaskEdge>,
    next_id: u64,
}

impl Default for TaskGraph {
    /// 创建默认任务图。
    fn default() -> Self {
        Self::new()
    }
}

impl TaskGraph {
    /// 创建空任务图。
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: vec![],
            next_id: 1,
        }
    }

    /// 插入任务节点并在需要时分配 ID。
    pub fn insert_task(&mut self, mut task: Task) -> Task {
        if task.id.is_unassigned() {
            task.id = TaskId(self.next_id);
            self.next_id = self.next_id.saturating_add(1);
        } else {
            self.next_id = self.next_id.max(task.id.0.saturating_add(1));
        }

        self.nodes.insert(task.id, TaskNode { task: task.clone() });
        task
    }

    /// 添加任务之间的扩展边。
    pub fn add_edge(&mut self, from: TaskId, to: TaskId, action_kind: ActionKind) {
        self.edges.push(TaskEdge {
            from,
            to,
            action_kind,
        });
    }

    /// 更新任务状态。
    pub fn set_status(&mut self, task_id: TaskId, status: TaskStatus) {
        if let Some(node) = self.nodes.get_mut(&task_id) {
            node.task.status = status;
        }
    }

    /// 返回任务节点引用。
    pub fn get(&self, task_id: TaskId) -> Option<&TaskNode> {
        self.nodes.get(&task_id)
    }

    /// 返回任务节点数量。
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 返回任务边数量。
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// 返回全部任务边。
    pub fn edges(&self) -> &[TaskEdge] {
        &self.edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证任务图可以分配任务 ID 并添加边。
    #[test]
    fn graph_assigns_ids_and_tracks_edges() {
        let mut graph = TaskGraph::new();
        let parent = graph.insert_task(Task::fetch("https://example.com"));
        let child = graph.insert_task(Task::fetch("https://example.com/docs"));

        graph.add_edge(parent.id, child.id, ActionKind::FollowLink);

        assert_eq!(parent.id, TaskId(1));
        assert_eq!(child.id, TaskId(2));
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }
}
