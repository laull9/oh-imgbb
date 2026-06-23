use std::collections::HashMap;

use crate::engine::task::{Task, TaskKind};

/// ActionKind 表示从页面中推导出的下一步行为类型。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionKind {
    ExtractData,
    FollowLink,
    DownloadFile,
    CallApi,
    SpawnTask,
    TriggerPlugin,
}

/// Action 是 Page 到新 Task 之间的声明式行为意图。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    pub kind: ActionKind,
    pub target: Option<String>,
    pub priority_delta: i32,
    pub next_task_kind: Option<TaskKind>,
    pub metadata: HashMap<String, String>,
}

impl Action {
    /// 创建默认抓取行为。
    pub fn fetch(target: impl Into<String>) -> Self {
        Self::follow_link(target)
    }

    /// 创建跟踪链接行为。
    pub fn follow_link(target: impl Into<String>) -> Self {
        Self::new(ActionKind::FollowLink).with_target(target)
    }

    /// 创建下载文件行为。
    pub fn download_file(target: impl Into<String>) -> Self {
        Self::new(ActionKind::DownloadFile).with_target(target)
    }

    /// 创建下载行为的短别名。
    pub fn download(target: impl Into<String>) -> Self {
        Self::download_file(target)
    }

    /// 创建 API 调用行为。
    pub fn call_api(target: impl Into<String>) -> Self {
        Self::new(ActionKind::CallApi).with_target(target)
    }

    /// 创建 API 调用行为的短别名。
    pub fn api(target: impl Into<String>) -> Self {
        Self::call_api(target)
    }

    /// 创建自定义任务生成行为。
    pub fn spawn(target: impl Into<String>, next_task_kind: TaskKind) -> Self {
        Self::new(ActionKind::SpawnTask)
            .with_target(target)
            .with_next_task_kind(next_task_kind)
    }

    /// 创建通用行为。
    pub fn new(kind: ActionKind) -> Self {
        Self {
            kind,
            target: None,
            priority_delta: 0,
            next_task_kind: None,
            metadata: HashMap::new(),
        }
    }

    /// 设置行为目标。
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// 设置生成任务的优先级偏移。
    pub fn with_priority_delta(mut self, priority_delta: i32) -> Self {
        self.priority_delta = priority_delta;
        self
    }

    /// 设置行为生成的新任务类型。
    pub fn with_next_task_kind(mut self, next_task_kind: TaskKind) -> Self {
        self.next_task_kind = Some(next_task_kind);
        self
    }

    /// 设置行为元数据。
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 将可执行行为转换为新任务。
    pub fn to_task(&self, parent: &Task) -> Option<Task> {
        let target = self.target.clone()?;
        let kind = match &self.kind {
            ActionKind::FollowLink => TaskKind::Fetch,
            ActionKind::DownloadFile => TaskKind::Download,
            ActionKind::CallApi => TaskKind::Api,
            ActionKind::SpawnTask => self.next_task_kind.clone().unwrap_or(TaskKind::Fetch),
            ActionKind::ExtractData | ActionKind::TriggerPlugin => return None,
        };

        let mut task = Task::new(kind, target)
            .with_parent_id(parent.id)
            .with_depth(parent.depth.saturating_add(1))
            .with_priority(parent.priority.saturating_add(self.priority_delta));
        task.metadata = self.metadata.clone();

        Some(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::task::TaskId;

    /// 验证链接行为可以生成子抓取任务。
    #[test]
    fn follow_link_action_generates_child_task() {
        let parent = Task::fetch("https://example.com")
            .with_id(TaskId(1))
            .with_priority(3);
        let action = Action::follow_link("https://example.com/docs").with_priority_delta(2);
        let task = action.to_task(&parent).unwrap();

        assert_eq!(task.kind, TaskKind::Fetch);
        assert_eq!(task.depth, 1);
        assert_eq!(task.parent_id, Some(TaskId(1)));
        assert_eq!(task.priority, 5);
    }

    /// 验证短别名可以创建常用行为。
    #[test]
    fn action_aliases_create_common_actions() {
        assert_eq!(
            Action::fetch("https://example.com").kind,
            ActionKind::FollowLink
        );
        assert_eq!(
            Action::api("https://example.com/api").kind,
            ActionKind::CallApi
        );
        assert_eq!(
            Action::download("https://example.com/file.pdf").kind,
            ActionKind::DownloadFile
        );
    }
}
