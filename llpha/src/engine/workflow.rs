use std::future::Future;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::task::JoinSet;

use crate::downloader::client::LlphaClient;
use crate::downloader::request::FetchResponse;
use crate::engine::action::Action;
use crate::engine::fetcher::{Fetcher, FnFetcher, LlphaFetcher};
use crate::engine::generator::{ActionGenerator, LinkActionGenerator};
use crate::engine::graph::TaskGraph;
use crate::engine::parser::{HtmlParser, Parser};
use crate::engine::scheduler::{InMemoryScheduler, Scheduler};
use crate::engine::task::{Task, TaskStatus};
use crate::plugin::{PluginContext, PluginRegistry};

/// EngineReport 保存任务引擎一次运行后的统计结果。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EngineReport {
    pub processed_tasks: usize,
    pub generated_tasks: usize,
    pub failed_tasks: usize,
}

/// WorkflowEngineBuilder 构建任务驱动的工作流引擎。
pub struct WorkflowEngineBuilder {
    fetcher: Option<Arc<dyn Fetcher>>,
    parser: Arc<dyn Parser>,
    generator: Arc<dyn ActionGenerator>,
    scheduler: InMemoryScheduler,
    plugin_registry: PluginRegistry,
    max_steps: usize,
}

impl Default for WorkflowEngineBuilder {
    /// 创建默认引擎构建器。
    fn default() -> Self {
        Self {
            fetcher: None,
            parser: Arc::new(HtmlParser::new()),
            generator: Arc::new(LinkActionGenerator::new()),
            scheduler: InMemoryScheduler::new(),
            plugin_registry: PluginRegistry::new(),
            max_steps: 128,
        }
    }
}

impl WorkflowEngineBuilder {
    /// 创建新的引擎构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置任务执行器。
    pub fn fetcher<F>(mut self, fetcher: F) -> Self
    where
        F: Fetcher + 'static,
    {
        self.fetcher = Some(Arc::new(fetcher));
        self
    }

    /// 使用异步闭包设置任务执行器。
    pub fn fetcher_fn<F, Fut>(self, fetcher: F) -> Self
    where
        F: Fn(Task) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<FetchResponse>> + Send + 'static,
    {
        self.fetcher(FnFetcher::new(fetcher))
    }

    /// 设置响应解析器。
    pub fn parser<P>(mut self, parser: P) -> Self
    where
        P: Parser + 'static,
    {
        self.parser = Arc::new(parser);
        self
    }

    /// 设置行为生成器。
    pub fn generator<G>(mut self, generator: G) -> Self
    where
        G: ActionGenerator + 'static,
    {
        self.generator = Arc::new(generator);
        self
    }

    /// 使用闭包设置页面行为生成逻辑。
    pub fn on_page<G>(self, generator: G) -> Self
    where
        G: ActionGenerator + 'static,
    {
        self.generator(generator)
    }

    /// 设置内存调度器。
    pub fn scheduler(mut self, scheduler: InMemoryScheduler) -> Self {
        self.scheduler = scheduler;
        self
    }

    /// 设置任务图最大扩展深度。
    pub fn max_depth(mut self, max_depth: usize) -> Self {
        self.scheduler = self.scheduler.with_max_depth(max_depth);
        self
    }

    /// 设置最大运行中任务数量。
    pub fn max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.scheduler = self.scheduler.with_max_concurrency(max_concurrency);
        self
    }

    /// 设置插件注册表。
    pub fn plugin_registry(mut self, plugin_registry: PluginRegistry) -> Self {
        self.plugin_registry = plugin_registry;
        self
    }

    /// 设置最大执行步数。
    pub fn max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// 构建工作流引擎。
    pub fn build(self) -> Result<WorkflowEngine> {
        let fetcher = match self.fetcher {
            Some(fetcher) => fetcher,
            None => Arc::new(LlphaFetcher::new(LlphaClient::global())),
        };

        Ok(WorkflowEngine {
            fetcher,
            parser: self.parser,
            generator: self.generator,
            scheduler: self.scheduler,
            graph: TaskGraph::new(),
            plugin_registry: Arc::new(self.plugin_registry),
            plugin_context: PluginContext {
                name: "llpha-engine".to_string(),
            },
            max_steps: self.max_steps,
        })
    }
}

/// WorkflowEngine 执行事件驱动的任务图扩展闭环。
pub struct WorkflowEngine {
    fetcher: Arc<dyn Fetcher>,
    parser: Arc<dyn Parser>,
    generator: Arc<dyn ActionGenerator>,
    scheduler: InMemoryScheduler,
    graph: TaskGraph,
    plugin_registry: Arc<PluginRegistry>,
    plugin_context: PluginContext,
    max_steps: usize,
}

impl WorkflowEngine {
    /// 创建默认工作流引擎。
    pub fn new() -> Result<Self> {
        WorkflowEngineBuilder::new().build()
    }

    /// 创建工作流引擎构建器。
    pub fn builder() -> WorkflowEngineBuilder {
        WorkflowEngineBuilder::new()
    }

    /// 返回任务图引用。
    pub fn graph(&self) -> &TaskGraph {
        &self.graph
    }

    /// 添加种子任务。
    pub fn add_seed(&mut self, task: Task) -> Result<bool> {
        self.insert_task(task, None)
    }

    /// 运行任务闭环直到队列为空或达到步数上限。
    pub async fn run<I, S>(&mut self, seeds: I) -> Result<EngineReport>
    where
        I: IntoIterator<Item = S>,
        S: Into<Task>,
    {
        for seed in seeds {
            self.add_seed(seed.into())?;
        }

        let mut report = EngineReport::default();
        let mut running_tasks = JoinSet::new();
        while report.processed_tasks < self.max_steps {
            while report.processed_tasks + running_tasks.len() < self.max_steps {
                let Some(task) = self.scheduler.next() else {
                    break;
                };

                self.graph.set_status(task.id, TaskStatus::Running);
                let fetcher = self.fetcher.clone();
                let parser = self.parser.clone();
                let generator = self.generator.clone();
                let plugin_registry = self.plugin_registry.clone();
                let plugin_context = self.plugin_context.clone();

                running_tasks.spawn(async move {
                    let actions = execute_task(
                        fetcher,
                        parser,
                        generator,
                        plugin_registry,
                        plugin_context,
                        task.clone(),
                    )
                    .await;

                    (task, actions)
                });
            }

            let Some(result) = running_tasks.join_next().await else {
                break;
            };

            let (task, result) = result.context("工作流任务执行线程失败")?;
            match result {
                Ok(actions) => {
                    self.scheduler.mark_succeeded(task.id);
                    self.graph.set_status(task.id, TaskStatus::Succeeded);
                    report.processed_tasks = report.processed_tasks.saturating_add(1);
                    report.generated_tasks = report
                        .generated_tasks
                        .saturating_add(self.apply_actions(&task, &actions)?);
                }
                Err(_) => {
                    self.scheduler.mark_failed(task.id);
                    self.graph.set_status(task.id, TaskStatus::Failed);
                    report.processed_tasks = report.processed_tasks.saturating_add(1);
                    report.failed_tasks = report.failed_tasks.saturating_add(1);
                }
            }
        }

        Ok(report)
    }

    /// 使用默认约定抓取一个入口地址。
    pub async fn crawl(seed: impl Into<Task>) -> Result<EngineReport> {
        let mut engine = Self::new()?;
        engine.run([seed.into()]).await
    }

    /// 运行一个入口地址并保留任务图。
    pub async fn crawl_graph(seed: impl Into<Task>) -> Result<(EngineReport, TaskGraph)> {
        let mut engine = Self::new()?;
        let report = engine.run([seed.into()]).await?;

        Ok((report, engine.graph))
    }

    /// 将行为转换为任务并回流调度器。
    fn apply_actions(&mut self, parent: &Task, actions: &[Action]) -> Result<usize> {
        let mut inserted = 0usize;

        for action in actions {
            let Some(task) = action.to_task(parent) else {
                continue;
            };

            if self.insert_task(task, Some((parent.id, action.kind.clone())))? {
                inserted = inserted.saturating_add(1);
            }
        }

        Ok(inserted)
    }

    /// 插入任务到图和调度器。
    fn insert_task(
        &mut self,
        task: Task,
        edge: Option<(
            crate::engine::task::TaskId,
            crate::engine::action::ActionKind,
        )>,
    ) -> Result<bool> {
        let task = self
            .plugin_registry
            .apply_before_task(&self.plugin_context, task)?;
        let task = self.graph.insert_task(task);
        let accepted = self.scheduler.push(task.clone())?;

        if accepted && let Some((from, action_kind)) = edge {
            self.graph.add_edge(from, task.id, action_kind);
        }

        Ok(accepted)
    }
}

/// 执行一个任务并生成行为。
async fn execute_task(
    fetcher: Arc<dyn Fetcher>,
    parser: Arc<dyn Parser>,
    generator: Arc<dyn ActionGenerator>,
    plugin_registry: Arc<PluginRegistry>,
    plugin_context: PluginContext,
    task: Task,
) -> Result<Vec<Action>> {
    let response = fetcher.fetch(&task).await?;
    let mut page = parser.parse(&task, response)?;
    page = plugin_registry.apply_after_page(&plugin_context, &task, page)?;
    let actions = generator.generate(&task, &page)?;

    plugin_registry.apply_after_actions(&plugin_context, &task, &page, actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use reqwest::StatusCode;
    use reqwest::header::HeaderMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{Duration, sleep};

    use crate::downloader::request::FetchResponse;

    /// StaticFetcher 用于测试工作流闭环。
    struct StaticFetcher;

    #[async_trait]
    impl Fetcher for StaticFetcher {
        /// 返回带有链接的固定 HTML 响应。
        async fn fetch(&self, task: &Task) -> Result<FetchResponse> {
            let body = if task.target.ends_with("/docs") {
                "<html><body>docs</body></html>"
            } else {
                r#"<html><body><a href="/docs">Docs</a></body></html>"#
            };

            Ok(FetchResponse {
                url: task.target.clone(),
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: body.to_string(),
            })
        }
    }

    /// 验证工作流引擎可以执行 Task -> Page -> Action -> Task 闭环。
    #[tokio::test]
    async fn workflow_engine_expands_task_graph() {
        let mut engine = WorkflowEngine::builder()
            .fetcher(StaticFetcher)
            .max_steps(2)
            .build()
            .unwrap();

        let report = engine
            .run(vec![Task::fetch("https://example.com")])
            .await
            .unwrap();

        assert_eq!(report.processed_tasks, 2);
        assert_eq!(report.generated_tasks, 1);
        assert_eq!(engine.graph().node_count(), 2);
        assert_eq!(engine.graph().edge_count(), 1);
    }

    /// 验证闭包可以直接定义任务执行和页面扩展逻辑。
    #[tokio::test]
    async fn workflow_engine_accepts_lambda_handlers() {
        let mut engine = WorkflowEngine::builder()
            .fetcher_fn(|task| async move {
                Ok(FetchResponse {
                    url: task.target,
                    status: StatusCode::OK,
                    headers: HeaderMap::new(),
                    body: "<html><body>lambda</body></html>".to_string(),
                })
            })
            .on_page(|_task: &Task, _page: &crate::engine::page::Page| {
                Ok(vec![Action::fetch("https://example.com/next")])
            })
            .max_steps(1)
            .max_depth(1)
            .build()
            .unwrap();

        let report = engine.run(["https://example.com"]).await.unwrap();

        assert_eq!(report.processed_tasks, 1);
        assert_eq!(report.generated_tasks, 1);
        assert_eq!(engine.graph().node_count(), 2);
        assert_eq!(engine.graph().edge_count(), 1);
    }

    /// 验证工作流引擎会按配置并发执行独立任务。
    #[tokio::test]
    async fn workflow_engine_runs_tasks_concurrently() {
        let running = Arc::new(AtomicUsize::new(0));
        let observed = Arc::new(AtomicUsize::new(0));
        let running_for_fetcher = running.clone();
        let observed_for_fetcher = observed.clone();
        let mut engine = WorkflowEngine::builder()
            .fetcher_fn(move |task| {
                let running = running_for_fetcher.clone();
                let observed = observed_for_fetcher.clone();
                async move {
                    let current = running.fetch_add(1, Ordering::SeqCst) + 1;
                    observed.fetch_max(current, Ordering::SeqCst);
                    sleep(Duration::from_millis(10)).await;
                    running.fetch_sub(1, Ordering::SeqCst);

                    Ok(FetchResponse {
                        url: task.target,
                        status: StatusCode::OK,
                        headers: HeaderMap::new(),
                        body: "<html></html>".to_string(),
                    })
                }
            })
            .generator(|_task: &Task, _page: &crate::engine::page::Page| Ok(vec![]))
            .max_concurrency(2)
            .max_steps(4)
            .build()
            .unwrap();

        let report = engine
            .run([
                "https://example.com/a",
                "https://example.com/b",
                "https://example.com/c",
                "https://example.com/d",
            ])
            .await
            .unwrap();

        assert_eq!(report.processed_tasks, 4);
        assert_eq!(observed.load(Ordering::SeqCst), 2);
    }
}
