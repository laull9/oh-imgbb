use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use anyhow::Result;

use crate::engine::task::{Task, TaskId, TaskStatus};

/// Scheduler 定义任务生命周期和执行顺序控制接口。
pub trait Scheduler {
    /// 将任务放入调度器。
    fn push(&mut self, task: Task) -> Result<bool>;

    /// 取出下一个可执行任务。
    fn next(&mut self) -> Option<Task>;

    /// 将任务标记为成功。
    fn mark_succeeded(&mut self, task_id: TaskId);

    /// 将任务标记为失败。
    fn mark_failed(&mut self, task_id: TaskId);

    /// 返回等待执行的任务数量。
    fn pending_len(&self) -> usize;

    /// 返回运行中的任务数量。
    fn running_len(&self) -> usize;
}

/// QueueEntry 保存优先队列中的任务排序信息。
#[derive(Clone, Debug, Eq, PartialEq)]
struct QueueEntry {
    priority: i32,
    sequence: u64,
    task_id: TaskId,
}

impl Ord for QueueEntry {
    /// 比较任务优先级和进入队列顺序。
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for QueueEntry {
    /// 返回任务排序结果。
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// InMemoryScheduler 提供优先级、深度、去重和并发额度控制的内存调度器。
#[derive(Clone, Debug)]
pub struct InMemoryScheduler {
    queue: BinaryHeap<QueueEntry>,
    tasks: HashMap<TaskId, Task>,
    seen: HashSet<String>,
    running: HashSet<TaskId>,
    sequence: u64,
    max_depth: usize,
    max_concurrency: usize,
}

impl Default for InMemoryScheduler {
    /// 创建默认内存调度器。
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryScheduler {
    /// 创建新的内存调度器。
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            tasks: HashMap::new(),
            seen: HashSet::new(),
            running: HashSet::new(),
            sequence: 0,
            max_depth: 3,
            max_concurrency: 1,
        }
    }

    /// 设置最大任务深度。
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// 设置最大运行中任务数量。
    pub fn with_max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = max_concurrency.max(1);
        self
    }

    /// 返回最大运行中任务数量。
    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    /// 返回当前剩余并发额度。
    pub fn available_slots(&self) -> usize {
        self.max_concurrency.saturating_sub(self.running.len())
    }

    /// 返回任务状态。
    pub fn status(&self, task_id: TaskId) -> Option<TaskStatus> {
        self.tasks.get(&task_id).map(|task| task.status.clone())
    }
}

impl Scheduler for InMemoryScheduler {
    /// 将任务放入优先队列。
    fn push(&mut self, mut task: Task) -> Result<bool> {
        if task.depth > self.max_depth {
            task.status = TaskStatus::Skipped;
            self.tasks.insert(task.id, task);
            return Ok(false);
        }

        let fingerprint = task.fingerprint();
        if !self.seen.insert(fingerprint) {
            task.status = TaskStatus::Skipped;
            self.tasks.insert(task.id, task);
            return Ok(false);
        }

        task.status = TaskStatus::Pending;
        self.sequence = self.sequence.saturating_add(1);
        self.queue.push(QueueEntry {
            priority: task.priority,
            sequence: self.sequence,
            task_id: task.id,
        });
        self.tasks.insert(task.id, task);

        Ok(true)
    }

    /// 取出下一个可执行任务。
    fn next(&mut self) -> Option<Task> {
        if self.available_slots() == 0 {
            return None;
        }

        while let Some(entry) = self.queue.pop() {
            let Some(task) = self.tasks.get_mut(&entry.task_id) else {
                continue;
            };

            if task.status != TaskStatus::Pending {
                continue;
            }

            task.status = TaskStatus::Running;
            self.running.insert(task.id);
            return Some(task.clone());
        }

        None
    }

    /// 将任务标记为成功。
    fn mark_succeeded(&mut self, task_id: TaskId) {
        self.running.remove(&task_id);
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.status = TaskStatus::Succeeded;
        }
    }

    /// 将任务标记为失败。
    fn mark_failed(&mut self, task_id: TaskId) {
        self.running.remove(&task_id);
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.status = TaskStatus::Failed;
        }
    }

    /// 返回队列中的等待任务数量。
    fn pending_len(&self) -> usize {
        self.tasks
            .values()
            .filter(|task| task.status == TaskStatus::Pending)
            .count()
    }

    /// 返回运行中的任务数量。
    fn running_len(&self) -> usize {
        self.running.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证调度器按优先级取任务。
    #[test]
    fn scheduler_pops_high_priority_first() {
        let mut scheduler = InMemoryScheduler::new();
        scheduler
            .push(Task::fetch("https://example.com/a").with_id(TaskId(1)))
            .unwrap();
        scheduler
            .push(
                Task::fetch("https://example.com/b")
                    .with_id(TaskId(2))
                    .with_priority(10),
            )
            .unwrap();

        let next = scheduler.next().unwrap();

        assert_eq!(next.id, TaskId(2));
    }

    /// 验证调度器会跳过重复任务。
    #[test]
    fn scheduler_dedupes_by_fingerprint() {
        let mut scheduler = InMemoryScheduler::new();

        assert!(
            scheduler
                .push(Task::fetch("https://example.com").with_id(TaskId(1)))
                .unwrap()
        );
        assert!(
            !scheduler
                .push(Task::fetch("https://example.com").with_id(TaskId(2)))
                .unwrap()
        );
    }

    /// 验证调度器会限制运行中任务数量。
    #[test]
    fn scheduler_respects_max_concurrency() {
        let mut scheduler = InMemoryScheduler::new().with_max_concurrency(1);
        scheduler
            .push(Task::fetch("https://example.com/a").with_id(TaskId(1)))
            .unwrap();
        scheduler
            .push(Task::fetch("https://example.com/b").with_id(TaskId(2)))
            .unwrap();

        let first = scheduler.next().unwrap();

        assert_eq!(scheduler.running_len(), 1);
        assert_eq!(scheduler.available_slots(), 0);
        assert!(scheduler.next().is_none());

        scheduler.mark_succeeded(first.id);

        assert_eq!(scheduler.running_len(), 0);
        assert!(scheduler.next().is_some());
    }
}
