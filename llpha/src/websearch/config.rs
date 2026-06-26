use std::time::Duration;

use anyhow::{Result, anyhow};

/// DEFAULT_SEARCH_LIMIT 表示默认搜索结果条数。
pub const DEFAULT_SEARCH_LIMIT: usize = 10;

/// DEFAULT_SEARCH_TIMEOUT 表示默认单次请求超时时间。
pub const DEFAULT_SEARCH_TIMEOUT: Duration = Duration::from_secs(10);

/// DEFAULT_SEARCH_PING_TIMEOUT 表示搜索可用性探测默认超时时间。
pub const DEFAULT_SEARCH_PING_TIMEOUT: Duration = Duration::from_secs(3);

/// DEFAULT_MAX_SEARCH_PAGES 表示默认最大翻页次数。
pub const DEFAULT_MAX_SEARCH_PAGES: usize = 5;

/// SearchConfig 保存搜索引擎通用配置。
#[derive(Clone, Debug)]
pub struct SearchConfig {
    pub base_urls: Vec<String>,
    pub limit: usize,
    pub timeout: Duration,
    pub max_pages: usize,
}

impl SearchConfig {
    /// 创建指定默认地址的搜索配置。
    pub fn new(default_base_url: impl Into<String>) -> Self {
        Self {
            base_urls: vec![default_base_url.into()],
            limit: DEFAULT_SEARCH_LIMIT,
            timeout: DEFAULT_SEARCH_TIMEOUT,
            max_pages: DEFAULT_MAX_SEARCH_PAGES,
        }
    }

    /// 校验搜索配置是否可用。
    pub fn validate(&self) -> Result<()> {
        if self.base_urls.is_empty() {
            return Err(anyhow!("搜索 base_url 不能为空"));
        }

        if self.limit == 0 {
            return Err(anyhow!("搜索条数必须大于 0"));
        }

        if self.max_pages == 0 {
            return Err(anyhow!("最大翻页次数必须大于 0"));
        }

        Ok(())
    }
}

/// SearchBuilder 保存搜索引擎构造过程中的通用配置。
#[derive(Clone, Debug)]
pub struct SearchBuilder {
    config: SearchConfig,
}

impl SearchBuilder {
    /// 创建搜索配置构建器。
    pub fn new(default_base_url: impl Into<String>) -> Self {
        Self {
            config: SearchConfig::new(default_base_url),
        }
    }

    /// 设置主搜索地址。
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.config.base_urls = vec![base_url.into()];
        self
    }

    /// 增加搜索地址回退项。
    pub fn fallback_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.config.base_urls.push(base_url.into());
        self
    }

    /// 设置需要返回的搜索条数。
    pub fn limit(mut self, limit: usize) -> Self {
        self.config.limit = limit.max(1);
        self
    }

    /// 设置单次请求超时时间。
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// 设置最大翻页次数。
    pub fn max_pages(mut self, max_pages: usize) -> Self {
        self.config.max_pages = max_pages.max(1);
        self
    }

    /// 构建搜索配置。
    pub fn build(self) -> Result<SearchConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}
