//! download_tasks 模块管理运行期下载任务状态。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tokio::sync::Mutex;

/// DownloadTaskStatus 表示下载任务当前状态。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadTaskStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// DownloadTaskRecord 保存前端可展示的下载任务行。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DownloadTaskRecord {
    pub id: u64,
    pub title: String,
    pub target_kind: String,
    pub target_url: String,
    pub status: DownloadTaskStatus,
    pub total_items: usize,
    pub finished_items: usize,
    pub downloaded_files: usize,
    pub bytes_written: usize,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// DownloadTaskStore 保存任务记录、取消标记和后台句柄。
#[derive(Default)]
pub struct DownloadTaskStore {
    inner: Mutex<DownloadTaskStoreInner>,
}

/// DownloadTaskStoreInner 保存可变任务集合。
#[derive(Default)]
struct DownloadTaskStoreInner {
    next_id: u64,
    tasks: Vec<DownloadTaskRecord>,
    cancel_flags: HashMap<u64, Arc<AtomicBool>>,
    handles: HashMap<u64, JoinHandle<()>>,
}

impl DownloadTaskStore {
    /// 新建下载任务仓库。
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建一条等待执行的下载任务。
    pub async fn create_task(
        &self,
        title: String,
        target_kind: String,
        target_url: String,
        total_items: usize,
    ) -> (DownloadTaskRecord, Arc<AtomicBool>) {
        let mut inner = self.inner.lock().await;
        inner.next_id = inner.next_id.saturating_add(1);
        let id = inner.next_id;
        let now = now_string();
        let task = DownloadTaskRecord {
            id,
            title,
            target_kind,
            target_url,
            status: DownloadTaskStatus::Pending,
            total_items,
            finished_items: 0,
            downloaded_files: 0,
            bytes_written: 0,
            error_message: None,
            created_at: now.clone(),
            updated_at: now,
        };
        let cancel_flag = Arc::new(AtomicBool::new(false));

        inner.cancel_flags.insert(id, cancel_flag.clone());
        inner.tasks.insert(0, task.clone());

        (task, cancel_flag)
    }

    /// 记录后台任务句柄用于取消。
    pub async fn attach_handle(&self, id: u64, handle: JoinHandle<()>) {
        let mut inner = self.inner.lock().await;
        inner.handles.insert(id, handle);
    }

    /// 读取全部下载任务。
    pub async fn list_tasks(&self) -> Vec<DownloadTaskRecord> {
        self.inner.lock().await.tasks.clone()
    }

    /// 设置任务为运行中。
    pub async fn mark_running(&self, id: u64) -> Option<DownloadTaskRecord> {
        self.update_task(id, |task| {
            task.status = DownloadTaskStatus::Running;
            task.error_message = None;
        })
        .await
    }

    /// 更新任务标题并增加已知下载条目总数。
    pub async fn update_title_and_add_total_items(
        &self,
        id: u64,
        title: String,
        total_items: usize,
    ) -> Option<DownloadTaskRecord> {
        self.update_task(id, |task| {
            task.title = title;
            task.total_items = task.total_items.saturating_add(total_items);
        })
        .await
    }

    /// 增加任务已完成下载条目统计。
    pub async fn add_finished_item(
        &self,
        id: u64,
        bytes_written: usize,
    ) -> Option<DownloadTaskRecord> {
        self.update_task(id, |task| {
            task.finished_items = task.finished_items.saturating_add(1);
            task.downloaded_files = task.downloaded_files.saturating_add(1);
            task.bytes_written = task.bytes_written.saturating_add(bytes_written);
        })
        .await
    }

    /// 设置任务为完成状态。
    pub async fn mark_completed(&self, id: u64) -> Option<DownloadTaskRecord> {
        self.remove_runtime_state(id).await;
        self.update_task(id, |task| {
            task.status = DownloadTaskStatus::Completed;
            task.finished_items = task.total_items;
            task.error_message = None;
        })
        .await
    }

    /// 设置任务为失败状态。
    pub async fn mark_failed(&self, id: u64, error: String) -> Option<DownloadTaskRecord> {
        self.remove_runtime_state(id).await;
        self.update_task(id, |task| {
            task.status = DownloadTaskStatus::Failed;
            task.error_message = Some(error);
        })
        .await
    }

    /// 设置任务为已取消状态但不操作后台句柄。
    pub async fn mark_cancelled(&self, id: u64) -> Option<DownloadTaskRecord> {
        self.remove_runtime_state(id).await;
        self.update_task(id, |task| {
            task.status = DownloadTaskStatus::Cancelled;
        })
        .await
    }

    /// 请求取消任务并中断后台句柄。
    pub async fn cancel_task(&self, id: u64) -> Option<DownloadTaskRecord> {
        let mut inner = self.inner.lock().await;
        if let Some(flag) = inner.cancel_flags.get(&id) {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(handle) = inner.handles.remove(&id) {
            handle.abort();
        }
        inner.cancel_flags.remove(&id);

        let now = now_string();
        inner
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .map(|task| {
                task.status = DownloadTaskStatus::Cancelled;
                task.updated_at = now;
                task.clone()
            })
    }

    /// 检查任务是否已经被请求取消。
    pub fn is_cancelled(flag: &AtomicBool) -> bool {
        flag.load(Ordering::SeqCst)
    }

    /// 更新指定任务并返回新快照。
    async fn update_task<F>(&self, id: u64, update: F) -> Option<DownloadTaskRecord>
    where
        F: FnOnce(&mut DownloadTaskRecord),
    {
        let mut inner = self.inner.lock().await;
        let now = now_string();
        inner
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .map(|task| {
                update(task);
                task.updated_at = now;
                task.clone()
            })
    }

    /// 删除任务运行期取消标记和后台句柄。
    async fn remove_runtime_state(&self, id: u64) {
        let mut inner = self.inner.lock().await;
        inner.cancel_flags.remove(&id);
        inner.handles.remove(&id);
    }
}

/// 返回当前 UTC 时间字符串。
fn now_string() -> String {
    Utc::now().to_rfc3339()
}
