//! logging 模块提供终端和文件日志初始化能力。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use tracing_subscriber::EnvFilter;

use crate::config::{LoggingConfig, LoggingFormat, LoggingTarget};

/// LoggingGuard 持有异步文件日志写入守卫。
pub enum LoggingGuard {
    Console,
    File(tracing_appender::non_blocking::WorkerGuard),
}

/// init_logging 按配置初始化全局日志订阅器。
pub fn init_logging(config: &LoggingConfig) -> Result<LoggingGuard> {
    let filter = EnvFilter::try_new(&config.level)
        .with_context(|| format!("日志级别配置无效: {}", config.level))?;

    match config.target {
        LoggingTarget::Console => init_console_logging(config, filter),
        LoggingTarget::File => init_file_logging(config, filter),
    }
}

/// init_console_logging 初始化终端日志输出。
fn init_console_logging(config: &LoggingConfig, filter: EnvFilter) -> Result<LoggingGuard> {
    match config.format {
        LoggingFormat::Console => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(config.color)
                .try_init()
                .map_err(|err| anyhow!("初始化终端日志失败: {err}"))?;
        }
        LoggingFormat::Json => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .with_ansi(false)
                .try_init()
                .map_err(|err| anyhow!("初始化 JSON 终端日志失败: {err}"))?;
        }
    }

    Ok(LoggingGuard::Console)
}

/// init_file_logging 初始化标准日志文件输出。
fn init_file_logging(config: &LoggingConfig, filter: EnvFilter) -> Result<LoggingGuard> {
    let (dir, file_name) = split_log_path(&config.file_path);
    fs::create_dir_all(&dir).with_context(|| format!("创建日志目录失败: {}", dir.display()))?;

    let appender = tracing_appender::rolling::never(&dir, file_name);
    let (writer, guard) = tracing_appender::non_blocking(appender);

    match config.format {
        LoggingFormat::Console => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(writer)
                .try_init()
                .map_err(|err| anyhow!("初始化文件日志失败: {err}"))?;
        }
        LoggingFormat::Json => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(writer)
                .try_init()
                .map_err(|err| anyhow!("初始化 JSON 文件日志失败: {err}"))?;
        }
    }

    Ok(LoggingGuard::File(guard))
}

/// split_log_path 将日志路径拆成目录和文件名。
fn split_log_path(path: &str) -> (PathBuf, &str) {
    let path = Path::new(path);
    let dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("llpha.log");

    (dir, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证日志文件路径可以拆成目录和文件名。
    #[test]
    fn split_log_path_handles_nested_path() {
        let (dir, file_name) = split_log_path("target/logs/llpha.log");

        assert_eq!(dir, PathBuf::from("target/logs"));
        assert_eq!(file_name, "llpha.log");
    }

    /// 验证裸文件名会使用当前目录。
    #[test]
    fn split_log_path_handles_plain_file_name() {
        let (dir, file_name) = split_log_path("llpha.log");

        assert_eq!(dir, PathBuf::from("."));
        assert_eq!(file_name, "llpha.log");
    }
}
