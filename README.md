# Llpha Workspace

Llpha 是一个面向轻量级网页抓取和 ImgBB 相册工作流的 Rust workspace。仓库包含可复用爬虫框架、ImgBB 站点实现与 CLI，以及一个基于 Tauri、React、TypeScript 的桌面 GUI。

当前主要面向用户的软件是 `oh-imgbb`：它可以在本地解析 ImgBB 相册或个人空间地址，预览图片，保存收藏，缓存缩略图，并以可见进度下载选中的内容。

## 项目组成

| 路径 | 说明 |
| --- | --- |
| `llpha/` | 可复用的 Rust 爬虫与分析框架。 |
| `imgbb/` | ImgBB 爬虫实现、library 导出和 CLI 命令。 |
| `oh-imgbb/` | 基于 `imgbb` 和 `llpha` 的 Tauri 桌面应用。 |

## 功能

### oh-imgbb 桌面应用

- 解析 `https://ibb.co/album/{id}` 形式的 ImgBB 相册地址。
- 解析 `https://{name}.imgbb.com` 形式的 ImgBB 个人空间地址。
- 解析个人空间时流式追加相册结果。
- 使用缩略图网格预览相册图片。
- 图片详情查看器支持缩放、上一张/下一张，以及临时高清图加载。
- 支持相册图片和个人空间相册的搜索与分页。
- 支持整本相册、选中图片、选中个人空间相册的后台下载任务。
- 展示下载进度、已完成文件数、失败状态和取消状态。
- 支持本地收藏相册和个人空间。
- 本地缓存解析结果和缩略图。
- 支持重启后恢复解析标签页。
- 跟随系统亮暗主题。

### Rust crates

- `llpha` 提供请求与下载、重试、可配置日志、HTML 提取、任务调度和工作流基础能力。
- `imgbb` 基于 `llpha` 实现 ImgBB 相册解析、个人空间相册遍历、下载和已登录账号管理能力，并提供独立 CLI。

## 环境要求

- Rust stable toolchain
- Node.js 和 Yarn
- Tauri 2 对应平台的系统依赖

不同系统的 WebView 和构建工具依赖请参考 Tauri 官方 prerequisite 文档。

## 快速开始

克隆仓库并安装前端依赖：

```bash
git clone <repo-url>
cd llpha
cd oh-imgbb
yarn install
```

以开发模式运行桌面应用：

```bash
yarn tauri dev
```

构建桌面应用：

```bash
yarn tauri build
```

运行所有 Rust 测试：

```bash
cd ..
cargo test --workspace
```

## CLI 用法

`imgbb` CLI 覆盖桌面端的核心站点能力：公开解析/下载、可选登录态解析、选中图片下载、个人空间批量下载，以及需要登录的账号管理操作。桌面端的本地收藏、SQLite 缓存和标签页恢复属于 GUI 本地状态，不在 CLI 中持久化。

解析 ImgBB 相册：

```bash
imgbb parse-album https://ibb.co/album/ABC123
imgbb parse-album --json https://ibb.co/album/ABC123
```

下载 ImgBB 相册：

```bash
imgbb album https://ibb.co/album/ABC123
```

下载相册到指定目录，或只下载解析结果里的指定图片：

```bash
imgbb album -o downloads https://ibb.co/album/ABC123
imgbb images https://ibb.co/album/ABC123 --image-id https://i.ibb.co/demo/a.jpg
```

遍历 ImgBB 个人空间，或批量下载空间中的相册：

```bash
imgbb profile https://example.imgbb.com/albums?list=albums
imgbb profile-download -o downloads https://example.imgbb.com/albums?list=albums
```

需要登录的命令可以使用 `--login-subject`/`--password`，或设置 `IMGBB_LOGIN_SUBJECT` 与 `IMGBB_PASSWORD`：

```bash
imgbb login --login-subject you@example.com --password your-password
imgbb status
imgbb mine
imgbb mine --json
imgbb profile
imgbb create-album "Demo Album" --privacy private
imgbb upload-image ABC123 ./photo.jpg
imgbb edit-image IMAGE123 --title "New title"
imgbb delete-image IMAGE123
imgbb delete-album ABC123
imgbb logout
```

`login` 会保存 Cookie 会话到默认 `.imgbb-session.json`，不会保存密码。可以用全局参数 `--session <PATH>` 指定其他会话文件。

CLI 模式会把日志级别固定为 `warn`，避免列表和 JSON 输出被普通信息日志打断。

旧版 `ibb-album` 和 `ibb-profile` 子命令仍作为别名保留。

## 配置

`imgbb` CLI 默认读取当前运行目录下的 `config.toml`。可以从示例文件开始：

```bash
cp imgbb/config.example.toml config.toml
```

也可以显式指定配置文件：

```bash
imgbb --config imgbb/config.example.toml album https://ibb.co/album/ABC123
```

`oh-imgbb` 桌面应用会通过 Tauri 后端保存自己的本地设置、缓存元数据、收藏和解析标签页。默认下载目录是系统 Downloads 目录。

## 目录结构

```text
.
├── Cargo.toml
├── llpha/
│   ├── Cargo.toml
│   ├── README.md
│   ├── docs/
│   └── src/
├── imgbb/
│   ├── Cargo.toml
│   ├── README.md
│   ├── config.example.toml
│   └── src/
└── oh-imgbb/
    ├── package.json
    ├── src/
    └── src-tauri/
```

## 开发说明

- Rust 代码使用 `anyhow::Result` 处理错误，异步操作使用 async/await。
- 桌面前端使用 React、TypeScript、Ant Design，并通过 Tauri command 作为前后端边界。
- SQLite 用于保存设置、收藏、解析标签页和缓存元数据等本地状态。
- 测试尽量直接放在实现文件附近。

## 负责任使用

请只将本项目用于你有权访问和下载的内容，并遵守 ImgBB 服务条款、创作者权益、版权要求和当地法律法规。

## License

尚未指定许可证。
