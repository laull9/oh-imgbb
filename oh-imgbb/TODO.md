# oh-imgbb TODO

## 当前阶段

- [x] 改造 `imgbb` 为 library + binary 双入口，供 Tauri 后端直接依赖。
- [x] 为 `imgbb` 增加流式个人空间解析接口，让前端可以边解析边显示相册。
- [x] 在 `oh-imgbb` 中安装并接入 Ant Design、Ant Design Icons。
- [x] 搭建 `oh-imgbb` 前端三层导航：解析、收藏、设置。
- [x] 搭建 `oh-imgbb` 后端命令、数据库、设置和缓存基础结构。

## Milestone 1：基础设施

- [x] `imgbb/src/lib.rs` 导出爬虫模块。
- [x] `imgbb` CLI 改为使用 library 导出，不再只靠 `main.rs` 内部模块。
- [x] `oh-imgbb/src-tauri/Cargo.toml` 依赖 `imgbb`、`llpha`、`anyhow`、`tokio`、`sqlx`、`tracing`。
- [x] 初始化 SQLite 连接池和首版表结构。
- [x] 建立 Tauri `AppState`。

## Milestone 2：实时解析

- [x] 相册解析拆出只解析不下载接口。
- [x] 个人空间解析提供 page/batch 事件回调。
- [x] Tauri 后端通过 event 推送 `profile://album_found`。
- [x] 前端解析页支持实时追加个人空间相册。

## Milestone 3：前端骨架

- [x] 使用 Ant Design Layout + Menu 完成主导航。
- [x] 解析页支持 URL 输入、刷新开关、解析按钮。
- [x] 相册结果使用缩略图网格展示。
- [x] 个人空间结果使用列表展示。
- [x] 收藏页和设置页完成基础列表与表单。

## Milestone 4：下载与缓存

- [x] 下载全部相册。
- [ ] 下载选中图片。
- [ ] 批量下载个人空间选中相册。
- [x] 保存相册缓存、个人空间缓存和收藏。
- [ ] 缩略图缓存落盘和清理。
- [ ] 缓存命中时补充准确的 `parsed_at`。
- [ ] 相册缓存命中前先规整 URL，避免 `ibb.co/...` 和 `https://ibb.co/...` 重复。

## 已确认约束

- Rust 代码使用 snake_case、PascalCase、UPPER_SNAKE_CASE 命名约定。
- 每个函数和结构体前保留简短中文注释。
- 错误处理使用 anyhow，并返回 Result。
- 异步操作使用 async/await。
- 测试直接放在实现文件后面。
- 单个非测试 Rust 文件尽量不超过 500 行。
