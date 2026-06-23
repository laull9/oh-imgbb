# Llpha - 一个轻量级 Rust 爬虫与数据分析框架

## 项目结构

- `src/engine/` 核心任务引擎
- `src/downloader/` 请求、重试、代理和下载客户端
- `src/analysis/` HTML 提取与后续分析能力
- `src/fake/` 请求伪装策略
- `src/config/` 可缺省 TOML 配置系统
- `src/logging/` 彩色终端、JSON 和文件日志
- `src/plugin.rs` 插件系统
- `docs/` 框架文档

ImgBB 等站点级实现已经移入 workspace 的 `../imgbb/` crate，`llpha` 只保留可复用框架代码。

## 核心文档

- [任务引擎文档](./docs/engine.md)

## 当前能力

- Task -> Page -> Action -> Task 的任务图闭环
- 内存调度器支持优先级、深度、去重和并发额度控制
- 下载器默认启用请求头伪装，支持 reqwest、代理池、重试策略、插件钩子和文件下载落盘
- HTML 提取器支持 title、正文和链接提取
- 伪装器支持固定和随机浏览器请求头策略
- 配置系统支持缺少 config.toml 时使用默认值启动
- 日志系统默认彩色终端输出，支持文件目标和 JSON 格式

## 简化用法

默认约定已经覆盖常见爬取场景：客户端自动启用浏览器请求头伪装，引擎默认使用 HTTP 抓取、HTML 解析、链接扩展、内存调度和任务图记录。

```rust
use anyhow::Result;
use llpha::{Action, WorkflowEngine};

#[tokio::main]
async fn main() -> Result<()> {
    let mut engine = WorkflowEngine::builder()
        .max_depth(1)
        .on_page(|_task, page| {
            println!("抓到页面: {}", page.url);
            Ok(vec![Action::fetch("https://example.com/next")])
        })
        .build()?;

    let report = engine.run(["https://example.com"]).await?;
    println!("处理任务数: {}", report.processed_tasks);

    Ok(())
}
```

只需要一次普通请求时：

```rust
let html = llpha::LlphaClient::new()?
    .get("https://github.com/trending")
    .await?
    .body;
```

## 配置示例

默认读取当前目录下的 `config.toml`，文件不存在时会使用内置默认值。

```toml
[logging]
level = "info,llpha=debug"
target = "console" # console / file
format = "console" # console / json
file_path = "logs/llpha.log"
color = true
```


## 底层依赖库

- reqwest
- scraper
- serde
- tokio

## 设计风格

- 简洁易用
- 高内聚接口、模块化设计
- 高性能
- 可扩展性强
