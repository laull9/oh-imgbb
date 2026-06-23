//! config 模块提供可缺省的 TOML 配置读取能力。

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::downloader::{DEFAULT_MAX_CONCURRENT_REQUESTS, DEFAULT_MAX_RETRIES};

/// DEFAULT_CONFIG_PATH 表示默认配置文件路径。
pub const DEFAULT_CONFIG_PATH: &str = "config.toml";

/// AppConfig 保存 Llpha 启动时使用的全局配置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub logging: LoggingConfig,
    pub request: RequestConfig,
}

impl Default for AppConfig {
    /// 创建默认全局配置。
    fn default() -> Self {
        Self {
            logging: LoggingConfig::default(),
            request: RequestConfig::default(),
        }
    }
}

impl AppConfig {
    /// 从指定 TOML 文件读取配置，文件不存在时返回默认配置。
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {}", path.display()))?;
        toml::from_str(&content).with_context(|| format!("解析配置文件失败: {}", path.display()))
    }
}

/// LoggingConfig 保存日志输出配置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
    pub target: LoggingTarget,
    pub format: LoggingFormat,
    pub file_path: String,
    pub color: bool,
}

impl Default for LoggingConfig {
    /// 创建默认日志配置。
    fn default() -> Self {
        Self {
            level: "info,llpha=debug".to_string(),
            target: LoggingTarget::Console,
            format: LoggingFormat::Console,
            file_path: "logs/llpha.log".to_string(),
            color: true,
        }
    }
}

/// LoggingTarget 表示日志输出目标。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoggingTarget {
    #[default]
    Console,
    File,
}

/// LoggingFormat 表示日志内容格式。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoggingFormat {
    #[default]
    Console,
    Json,
}

/// RequestConfig 保存请求限流和重试配置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct RequestConfig {
    pub max_concurrent_requests: usize,
    pub max_retries: usize,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_jitter: bool,
}

impl Default for RequestConfig {
    /// 创建默认请求配置。
    fn default() -> Self {
        Self {
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_base_delay_ms: 300,
            retry_max_delay_ms: 5_000,
            retry_jitter: true,
        }
    }
}

/// load_config 从默认路径读取配置。
pub fn load_config() -> Result<AppConfig> {
    AppConfig::from_path(DEFAULT_CONFIG_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证缺失配置文件时可以返回默认配置。
    #[test]
    fn config_uses_defaults_when_file_missing() {
        let config = AppConfig::from_path("target/missing-llpha-config.toml").unwrap();

        assert_eq!(config, AppConfig::default());
    }

    /// 验证部分 TOML 配置会保留未填写项的默认值。
    #[test]
    fn config_merges_partial_toml_with_defaults() {
        let content = r#"
            [logging]
            target = "file"
            format = "json"
        "#;

        let config: AppConfig = toml::from_str(content).unwrap();

        assert_eq!(config.logging.target, LoggingTarget::File);
        assert_eq!(config.logging.format, LoggingFormat::Json);
        assert_eq!(config.logging.level, "info,llpha=debug");
        assert_eq!(config.logging.file_path, "logs/llpha.log");
        assert!(config.logging.color);
        assert_eq!(
            config.request.max_concurrent_requests,
            DEFAULT_MAX_CONCURRENT_REQUESTS
        );
        assert_eq!(config.request.max_retries, DEFAULT_MAX_RETRIES);
    }

    /// 验证请求配置可以从 TOML 覆盖。
    #[test]
    fn config_accepts_request_overrides() {
        let content = r#"
            [request]
            max_concurrent_requests = 4
            max_retries = 2
            retry_base_delay_ms = 100
            retry_max_delay_ms = 1000
            retry_jitter = false
        "#;

        let config: AppConfig = toml::from_str(content).unwrap();

        assert_eq!(config.request.max_concurrent_requests, 4);
        assert_eq!(config.request.max_retries, 2);
        assert_eq!(config.request.retry_base_delay_ms, 100);
        assert_eq!(config.request.retry_max_delay_ms, 1000);
        assert!(!config.request.retry_jitter);
    }
}
