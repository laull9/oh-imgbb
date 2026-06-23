use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, ensure};
use tokio::task::JoinSet;
use tokio::time::sleep;

/// TaskPool 提供轻量级异步任务并发和启动节流控制。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskPool {
    max_concurrency: usize,
    start_interval: Option<Duration>,
}

impl Default for TaskPool {
    /// 创建默认任务池。
    fn default() -> Self {
        Self::new(1)
    }
}

impl TaskPool {
    /// 创建指定最大并发数的任务池。
    pub fn new(max_concurrency: usize) -> Self {
        Self {
            max_concurrency: max_concurrency.max(1),
            start_interval: None,
        }
    }

    /// 设置相邻任务启动的最小间隔。
    pub fn with_start_interval(mut self, start_interval: Duration) -> Self {
        self.start_interval = Some(start_interval);
        self
    }

    /// 返回最大并发任务数。
    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    /// 批量执行任务并汇总成功和失败结果。
    pub async fn run_all<I, T, F, Fut, O>(&self, items: I, handler: F) -> TaskPoolReport<O>
    where
        I: IntoIterator<Item = T>,
        T: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O>> + Send + 'static,
        O: Send + 'static,
    {
        let handler = Arc::new(handler);
        let mut iterator = items.into_iter();
        let mut tasks = JoinSet::new();
        let mut successes = Vec::new();
        let mut failures = Vec::new();
        let mut running = 0usize;

        while running < self.max_concurrency {
            if !self.spawn_next(&mut iterator, &handler, &mut tasks).await {
                break;
            }
            running = running.saturating_add(1);
        }

        while let Some(result) = tasks.join_next().await {
            running = running.saturating_sub(1);
            match result {
                Ok(Ok(value)) => successes.push(value),
                Ok(Err(err)) => failures.push(err.to_string()),
                Err(err) => failures.push(format!("任务执行失败: {err}")),
            }

            if self.spawn_next(&mut iterator, &handler, &mut tasks).await {
                running = running.saturating_add(1);
            }
        }

        TaskPoolReport {
            successes,
            failures,
        }
    }

    /// 向任务集合补充下一个任务。
    async fn spawn_next<I, T, F, Fut, O>(
        &self,
        iterator: &mut I,
        handler: &Arc<F>,
        tasks: &mut JoinSet<Result<O>>,
    ) -> bool
    where
        I: Iterator<Item = T>,
        T: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O>> + Send + 'static,
        O: Send + 'static,
    {
        let Some(item) = iterator.next() else {
            return false;
        };

        if let Some(start_interval) = self.start_interval {
            sleep(start_interval).await;
        }

        let handler = handler.clone();
        tasks.spawn(async move { handler(item).await });

        true
    }
}

/// TaskPoolReport 保存任务池执行后的结果和错误。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskPoolReport<T> {
    pub successes: Vec<T>,
    pub failures: Vec<String>,
}

impl<T> TaskPoolReport<T> {
    /// 判断是否所有任务都成功完成。
    pub fn is_success(&self) -> bool {
        self.failures.is_empty()
    }

    /// 消费报告并返回全部成功结果。
    pub fn into_successes(self) -> Result<Vec<T>> {
        ensure!(
            self.failures.is_empty(),
            "部分任务执行失败:\n{}",
            self.failures.join("\n")
        );

        Ok(self.successes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::Duration;

    /// 验证任务池会限制同时运行的任务数量。
    #[tokio::test]
    async fn task_pool_limits_concurrency() {
        let running = Arc::new(AtomicUsize::new(0));
        let observed = Arc::new(AtomicUsize::new(0));
        let pool = TaskPool::new(2);
        let running_for_task = running.clone();
        let observed_for_task = observed.clone();

        let report = pool
            .run_all(0..6, move |value| {
                let running = running_for_task.clone();
                let observed = observed_for_task.clone();
                async move {
                    let current = running.fetch_add(1, Ordering::SeqCst) + 1;
                    observed.fetch_max(current, Ordering::SeqCst);
                    sleep(Duration::from_millis(5)).await;
                    running.fetch_sub(1, Ordering::SeqCst);

                    Ok(value)
                }
            })
            .await;

        assert!(report.is_success());
        assert_eq!(report.successes.len(), 6);
        assert_eq!(observed.load(Ordering::SeqCst), 2);
    }

    /// 验证任务池会收集任务错误。
    #[tokio::test]
    async fn task_pool_collects_failures() {
        let report = TaskPool::new(2)
            .run_all(0..3, |value| async move {
                if value == 1 {
                    anyhow::bail!("任务失败: {value}");
                }

                Ok(value)
            })
            .await;

        assert_eq!(report.successes.len(), 2);
        assert_eq!(report.failures, vec!["任务失败: 1".to_string()]);
    }
}
