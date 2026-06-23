use anyhow::Result;

use crate::downloader::request::{FetchRequest, FetchResponse};
use crate::engine::{Action, Page, Task};

/// PluginContext 保存插件执行时可以共享的上下文信息。
#[derive(Clone, Debug, Default)]
pub struct PluginContext {
    pub name: String,
}

/// Plugin 定义请求生命周期中的可扩展插件接口。
///
/// 插件可以在请求发出前改写请求，也可以在响应返回后记录指标、清洗内容或触发
/// 外部回调。默认实现保持无副作用，便于调用方只覆盖自己关心的阶段。
pub trait Plugin: Send + Sync {
    /// 返回插件名称。
    fn name(&self) -> &str;

    /// 请求发送前的钩子。
    fn before_request(
        &self,
        _context: &PluginContext,
        request: FetchRequest,
    ) -> Result<FetchRequest> {
        Ok(request)
    }

    /// 响应返回后的钩子。
    fn after_response(
        &self,
        _context: &PluginContext,
        response: FetchResponse,
    ) -> Result<FetchResponse> {
        Ok(response)
    }

    /// 任务进入调度器前的钩子。
    fn before_task(&self, _context: &PluginContext, task: Task) -> Result<Task> {
        Ok(task)
    }

    /// 页面解析完成后的钩子。
    fn after_page(&self, _context: &PluginContext, _task: &Task, page: Page) -> Result<Page> {
        Ok(page)
    }

    /// 行为生成完成后的钩子。
    fn after_actions(
        &self,
        _context: &PluginContext,
        _task: &Task,
        _page: &Page,
        actions: Vec<Action>,
    ) -> Result<Vec<Action>> {
        Ok(actions)
    }
}

/// PluginRegistry 管理请求生命周期中启用的插件集合。
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    /// 创建空插件注册表。
    pub fn new() -> Self {
        Self { plugins: vec![] }
    }

    /// 注册一个新插件。
    pub fn register<P>(&mut self, plugin: P)
    where
        P: Plugin + 'static,
    {
        self.plugins.push(Box::new(plugin));
    }

    /// 返回已注册插件数量。
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// 判断插件注册表是否为空。
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// 依次执行请求前插件钩子。
    pub fn apply_before_request(
        &self,
        context: &PluginContext,
        mut request: FetchRequest,
    ) -> Result<FetchRequest> {
        for plugin in &self.plugins {
            request = plugin.before_request(context, request)?;
        }

        Ok(request)
    }

    /// 依次执行响应后插件钩子。
    pub fn apply_after_response(
        &self,
        context: &PluginContext,
        mut response: FetchResponse,
    ) -> Result<FetchResponse> {
        for plugin in &self.plugins {
            response = plugin.after_response(context, response)?;
        }

        Ok(response)
    }

    /// 依次执行任务入队前插件钩子。
    pub fn apply_before_task(&self, context: &PluginContext, mut task: Task) -> Result<Task> {
        for plugin in &self.plugins {
            task = plugin.before_task(context, task)?;
        }

        Ok(task)
    }

    /// 依次执行页面解析后插件钩子。
    pub fn apply_after_page(
        &self,
        context: &PluginContext,
        task: &Task,
        mut page: Page,
    ) -> Result<Page> {
        for plugin in &self.plugins {
            page = plugin.after_page(context, task, page)?;
        }

        Ok(page)
    }

    /// 依次执行行为生成后插件钩子。
    pub fn apply_after_actions(
        &self,
        context: &PluginContext,
        task: &Task,
        page: &Page,
        mut actions: Vec<Action>,
    ) -> Result<Vec<Action>> {
        for plugin in &self.plugins {
            actions = plugin.after_actions(context, task, page, actions)?;
        }

        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloader::request::FetchRequest;

    /// RenamePlugin 用于测试请求改写钩子。
    struct RenamePlugin;

    impl Plugin for RenamePlugin {
        /// 返回测试插件名称。
        fn name(&self) -> &str {
            "rename"
        }

        /// 在测试中改写请求 URL。
        fn before_request(
            &self,
            _context: &PluginContext,
            mut request: FetchRequest,
        ) -> Result<FetchRequest> {
            request.url = "https://example.com/changed".to_string();
            Ok(request)
        }
    }

    #[test]
    /// 验证插件注册表可以执行请求前钩子。
    fn plugin_registry_applies_before_request_hooks() {
        let mut registry = PluginRegistry::new();
        registry.register(RenamePlugin);

        let request = registry
            .apply_before_request(
                &PluginContext::default(),
                FetchRequest::get("https://example.com"),
            )
            .unwrap();

        assert_eq!(request.url, "https://example.com/changed");
    }
}
