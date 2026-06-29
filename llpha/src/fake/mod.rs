//! fake 模块提供请求伪装策略能力。

pub mod user_agents;
pub use user_agents::{
    get_user_agent, has_user_agent, user_agent_count, user_agent_is_empty, user_agent_names,
    user_agent_values,
};

use anyhow::{Context, Result, anyhow};
use rand::Rng;
use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, HeaderMap, HeaderName, HeaderValue,
    USER_AGENT,
};

use crate::downloader::request::FetchRequest;

/// FakeProfile 表示一组可复用的请求伪装头。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeProfile {
    pub user_agent: String,
    pub accept: String,
    pub accept_language: String,
    pub accept_encoding: String,
    pub connection: String,
    pub extra_headers: Vec<(String, String)>,
}

impl Default for FakeProfile {
    /// 创建默认浏览器伪装配置。
    fn default() -> Self {
        Self {
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36".to_string(),
            accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
            accept_language: "zh-CN,zh;q=0.9,en;q=0.8".to_string(),
            accept_encoding: "gzip, deflate, br, zstd".to_string(),
            connection: "keep-alive".to_string(),
            extra_headers: vec![
                ("upgrade-insecure-requests".to_string(), "1".to_string()),
                ("sec-fetch-dest".to_string(), "document".to_string()),
                ("sec-fetch-mode".to_string(), "navigate".to_string()),
                ("sec-fetch-site".to_string(), "none".to_string()),
                ("sec-fetch-user".to_string(), "?1".to_string()),
            ],
        }
    }
}

impl FakeProfile {
    /// 创建自定义 User-Agent 的伪装配置。
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// 设置 Accept 请求头。
    pub fn with_accept(mut self, accept: impl Into<String>) -> Self {
        self.accept = accept.into();
        self
    }

    /// 设置 Accept-Language 请求头。
    pub fn with_accept_language(mut self, accept_language: impl Into<String>) -> Self {
        self.accept_language = accept_language.into();
        self
    }

    /// 设置 Accept-Encoding 请求头。
    pub fn with_accept_encoding(mut self, accept_encoding: impl Into<String>) -> Self {
        self.accept_encoding = accept_encoding.into();
        self
    }

    /// 添加一个额外请求头。
    pub fn with_extra_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    /// 转换为 HTTP 请求头集合。
    pub fn to_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        insert_header(&mut headers, USER_AGENT, &self.user_agent)?;
        insert_header(&mut headers, ACCEPT, &self.accept)?;
        insert_header(&mut headers, ACCEPT_LANGUAGE, &self.accept_language)?;
        insert_header(&mut headers, ACCEPT_ENCODING, &self.accept_encoding)?;
        insert_header(&mut headers, CONNECTION, &self.connection)?;
        for (name, value) in &self.extra_headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .with_context(|| format!("解析伪装请求头名称失败: {name}"))?;
            insert_header(&mut headers, name, value)?;
        }
        Ok(headers)
    }
}

/// browser_profiles 返回内置的常见浏览器伪装配置集合。
pub fn browser_profiles() -> Vec<FakeProfile> {
    user_agent_values()
        .filter(|user_agent| is_browser_user_agent(user_agent))
        .map(|user_agent| FakeProfile::default().with_user_agent(user_agent))
        .collect()
}

/// HeaderFakeStrategy 定义请求头伪装策略接口。
pub trait HeaderFakeStrategy: Send + Sync {
    /// 将伪装头应用到请求上。
    fn apply(&self, request: FetchRequest) -> Result<FetchRequest>;
}

/// StaticHeaderFakeStrategy 提供固定请求头伪装策略。
#[derive(Clone, Debug)]
pub struct StaticHeaderFakeStrategy {
    profile: FakeProfile,
}

impl Default for StaticHeaderFakeStrategy {
    /// 创建默认固定伪装策略。
    fn default() -> Self {
        Self::new(FakeProfile::default())
    }
}

impl StaticHeaderFakeStrategy {
    /// 创建固定伪装策略。
    pub fn new(profile: FakeProfile) -> Self {
        Self { profile }
    }
}

impl HeaderFakeStrategy for StaticHeaderFakeStrategy {
    /// 将固定伪装头合并到请求头中。
    fn apply(&self, mut request: FetchRequest) -> Result<FetchRequest> {
        merge_headers(&mut request, &self.profile)?;
        Ok(request)
    }
}

/// RandomHeaderFakeStrategy 提供随机浏览器请求头伪装策略。
#[derive(Clone, Debug)]
pub struct RandomHeaderFakeStrategy {
    profiles: Vec<FakeProfile>,
}

impl Default for RandomHeaderFakeStrategy {
    /// 创建使用内置浏览器集合的随机伪装策略。
    fn default() -> Self {
        Self {
            profiles: browser_profiles(),
        }
    }
}

impl RandomHeaderFakeStrategy {
    /// 创建自定义候选集合的随机伪装策略。
    pub fn new(profiles: Vec<FakeProfile>) -> Result<Self> {
        if profiles.is_empty() {
            return Err(anyhow!("随机伪装策略至少需要一个候选 profile"));
        }

        Ok(Self { profiles })
    }

    /// 返回候选 profile 数量。
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// 判断候选集合是否为空。
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// 随机选择一个 profile。
    fn choose_profile(&self) -> Result<&FakeProfile> {
        if self.profiles.is_empty() {
            return Err(anyhow!("随机伪装策略没有可用 profile"));
        }

        let mut rng = rand::rng();
        let index = rng.random_range(0..self.profiles.len());
        Ok(&self.profiles[index])
    }
}

impl HeaderFakeStrategy for RandomHeaderFakeStrategy {
    /// 将随机伪装头合并到请求头中。
    fn apply(&self, mut request: FetchRequest) -> Result<FetchRequest> {
        let profile = self.choose_profile()?;
        merge_headers(&mut request, profile)?;
        Ok(request)
    }
}

/// 合并 profile 中的请求头。
fn merge_headers(request: &mut FetchRequest, profile: &FakeProfile) -> Result<()> {
    let headers = profile.to_headers()?;
    for (name, value) in headers.iter() {
        request.options.headers.insert(name, value.clone());
    }
    Ok(())
}

/// 判断 User-Agent 是否适合作为浏览器伪装候选。
fn is_browser_user_agent(user_agent: &str) -> bool {
    user_agent.contains("Mozilla/") && HeaderValue::from_str(user_agent).is_ok()
}

/// 插入字符串请求头并转换错误类型。
fn insert_header(headers: &mut HeaderMap, name: HeaderName, value: &str) -> Result<()> {
    headers.insert(name, HeaderValue::from_str(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证伪装策略可以写入请求头。
    #[test]
    fn static_header_fake_strategy_adds_headers() {
        let strategy = StaticHeaderFakeStrategy::default();
        let request = strategy
            .apply(FetchRequest::get("https://example.com"))
            .unwrap();

        assert!(request.options.headers.contains_key(USER_AGENT));
        assert!(request.options.headers.contains_key(ACCEPT_LANGUAGE));
    }

    /// 验证内置浏览器集合提供多个候选 profile。
    #[test]
    fn browser_profiles_have_multiple_choices() {
        let profiles = browser_profiles();

        assert!(profiles.len() > 8_000);
        assert!(
            profiles
                .iter()
                .any(|profile| profile.user_agent.contains("Firefox"))
        );
        assert!(
            profiles
                .iter()
                .any(|profile| profile.user_agent.contains("Mobile"))
        );
    }

    /// 验证内置浏览器集合来自 User-Agent 数据文件。
    #[test]
    fn browser_profiles_use_embedded_user_agents() {
        let profiles = browser_profiles();
        let mozilla = user_agents::get("mozilla").unwrap();

        assert!(profiles.iter().any(|profile| profile.user_agent == mozilla));
    }

    /// 验证随机伪装策略可以写入增强请求头。
    #[test]
    fn random_header_fake_strategy_adds_browser_headers() {
        let strategy = RandomHeaderFakeStrategy::default();
        let request = strategy
            .apply(FetchRequest::get("https://example.com"))
            .unwrap();

        assert!(request.options.headers.contains_key(USER_AGENT));
        assert!(request.options.headers.contains_key(ACCEPT_ENCODING));
        assert!(request.options.headers.contains_key(CONNECTION));
    }

    /// 验证随机伪装策略拒绝空候选集合。
    #[test]
    fn random_header_fake_strategy_rejects_empty_profiles() {
        let strategy = RandomHeaderFakeStrategy::new(vec![]);

        assert!(strategy.is_err());
    }
}
