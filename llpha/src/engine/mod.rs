//! 任务引擎模块提供 Task -> Page -> Action -> Task 的闭环能力。

pub mod action;
pub mod fetcher;
pub mod generator;
pub mod graph;
pub mod page;
pub mod parser;
pub mod scheduler;
pub mod task;
pub mod task_pool;
pub mod workflow;

pub use action::{Action, ActionKind};
pub use fetcher::{Fetcher, FnFetcher, LlphaFetcher};
pub use generator::{ActionGenerator, LinkActionGenerator};
pub use graph::{TaskEdge, TaskGraph, TaskNode};
pub use page::{Page, PageBody};
pub use parser::{HtmlParser, Parser};
pub use scheduler::{InMemoryScheduler, Scheduler};
pub use task::{Task, TaskId, TaskKind, TaskStatus, UNASSIGNED_TASK_ID};
pub use task_pool::{TaskPool, TaskPoolReport};
pub use workflow::{EngineReport, WorkflowEngine, WorkflowEngineBuilder};
