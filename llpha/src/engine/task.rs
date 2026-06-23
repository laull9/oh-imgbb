use std::collections::HashMap;

use crate::downloader::request::{HttpMethod, RequestOptions};

/// UNASSIGNED_TASK_ID 表示尚未进入任务图的任务 ID。
pub const UNASSIGNED_TASK_ID: TaskId = TaskId(0);

/// TaskId 表示任务图中的唯一任务编号。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(pub u64);

impl TaskId {
    /// 判断任务编号是否尚未分配。
    pub fn is_unassigned(self) -> bool {
        self == UNASSIGNED_TASK_ID
    }
}

/// TaskKind 表示任务执行意图的类型。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TaskKind {
    Fetch,
    Extract,
    Download,
    Api,
    Follow,
    Custom(String),
}

/// TaskStatus 表示任务在调度生命周期中的状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

/// Task 是任务引擎中最小的可执行意图。
#[derive(Clone, Debug)]
pub struct Task {
    pub id: TaskId,
    pub kind: TaskKind,
    pub target: String,
    pub method: HttpMethod,
    pub body: Option<String>,
    pub depth: usize,
    pub priority: i32,
    pub parent_id: Option<TaskId>,
    pub status: TaskStatus,
    pub options: RequestOptions,
    pub metadata: HashMap<String, String>,
}

impl Task {
    /// 创建 GET 抓取任务。
    pub fn get(target: impl Into<String>) -> Self {
        Self::fetch(target)
    }

    /// 创建 POST 抓取任务。
    pub fn post(target: impl Into<String>, body: impl Into<String>) -> Self {
        Self::fetch(target)
            .with_method(HttpMethod::Post)
            .with_body(body)
    }

    /// 创建抓取任务。
    pub fn fetch(target: impl Into<String>) -> Self {
        Self::new(TaskKind::Fetch, target)
    }

    /// 创建 API 调用任务。
    pub fn api(target: impl Into<String>) -> Self {
        Self::new(TaskKind::Api, target)
    }

    /// 创建下载任务。
    pub fn download(target: impl Into<String>) -> Self {
        Self::new(TaskKind::Download, target)
    }

    /// 创建指定类型的任务。
    pub fn new(kind: TaskKind, target: impl Into<String>) -> Self {
        Self {
            id: UNASSIGNED_TASK_ID,
            kind,
            target: target.into(),
            method: HttpMethod::Get,
            body: None,
            depth: 0,
            priority: 0,
            parent_id: None,
            status: TaskStatus::Pending,
            options: RequestOptions::default(),
            metadata: HashMap::new(),
        }
    }

    /// 设置任务 ID。
    pub fn with_id(mut self, id: TaskId) -> Self {
        self.id = id;
        self
    }

    /// 设置父任务 ID。
    pub fn with_parent_id(mut self, parent_id: TaskId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// 设置任务深度。
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// 设置任务优先级。
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// 设置 HTTP 方法。
    pub fn with_method(mut self, method: HttpMethod) -> Self {
        self.method = method;
        self
    }

    /// 设置请求正文。
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// 设置请求配置。
    pub fn with_options(mut self, options: RequestOptions) -> Self {
        self.options = options;
        self
    }

    /// 设置任务元数据。
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 返回用于去重的稳定指纹。
    pub fn fingerprint(&self) -> String {
        format!("{:?}:{}:{}", self.kind, self.method_key(), self.target)
    }

    /// 返回 HTTP 方法的稳定文本表示。
    fn method_key(&self) -> &'static str {
        match self.method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
        }
    }
}

impl From<&str> for Task {
    /// 将 URL 字符串转换为默认抓取任务。
    fn from(target: &str) -> Self {
        Self::fetch(target)
    }
}

impl From<String> for Task {
    /// 将 URL 字符串转换为默认抓取任务。
    fn from(target: String) -> Self {
        Self::fetch(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证抓取任务默认状态。
    #[test]
    fn task_fetch_uses_pending_defaults() {
        let task = Task::fetch("https://example.com");

        assert_eq!(task.kind, TaskKind::Fetch);
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.id.is_unassigned());
    }

    /// 验证任务指纹包含类型、方法和目标。
    #[test]
    fn task_fingerprint_is_stable() {
        let task = Task::fetch("https://example.com");

        assert_eq!(task.fingerprint(), "Fetch:GET:https://example.com");
    }

    /// 验证字符串可以直接转换为抓取任务。
    #[test]
    fn string_converts_to_fetch_task() {
        let task: Task = "https://example.com".into();

        assert_eq!(task.kind, TaskKind::Fetch);
        assert_eq!(task.target, "https://example.com");
    }

    /// 验证 POST 任务会写入方法和正文。
    #[test]
    fn task_post_sets_method_and_body() {
        let task = Task::post("https://example.com/api", "{}");

        assert_eq!(task.method, HttpMethod::Post);
        assert_eq!(task.body.as_deref(), Some("{}"));
    }
}
