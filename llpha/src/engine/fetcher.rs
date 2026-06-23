use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::downloader::client::LlphaClient;
use crate::downloader::request::{FetchRequest, FetchResponse};
use crate::engine::task::Task;

/// Fetcher 定义任务到原始响应的异步执行接口。
#[async_trait]
pub trait Fetcher: Send + Sync {
    /// 执行任务并返回网络响应。
    async fn fetch(&self, task: &Task) -> Result<FetchResponse>;
}

/// FnFetcher 让异步闭包可以直接作为任务执行器。
pub struct FnFetcher<F> {
    handler: F,
}

impl<F> FnFetcher<F> {
    /// 创建闭包执行器。
    pub fn new(handler: F) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<F, Fut> Fetcher for FnFetcher<F>
where
    F: Fn(Task) -> Fut + Send + Sync,
    Fut: Future<Output = Result<FetchResponse>> + Send,
{
    /// 克隆任务后交给异步闭包执行。
    async fn fetch(&self, task: &Task) -> Result<FetchResponse> {
        (self.handler)(task.clone()).await
    }
}

/// LlphaFetcher 使用 LlphaClient 执行任务请求。
pub struct LlphaFetcher {
    client: Arc<LlphaClient>,
}

impl LlphaFetcher {
    /// 创建默认执行器。
    pub fn new(client: impl Into<Arc<LlphaClient>>) -> Self {
        Self {
            client: client.into(),
        }
    }
}

#[async_trait]
impl Fetcher for LlphaFetcher {
    /// 将 Task 转换为 FetchRequest 并执行。
    async fn fetch(&self, task: &Task) -> Result<FetchResponse> {
        let request = FetchRequest {
            method: task.method.clone(),
            url: task.target.clone(),
            body: task.body.clone(),
            options: task.options.clone(),
        };

        self.client.fetch(request).await
    }
}
