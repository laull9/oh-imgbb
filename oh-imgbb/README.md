# oh-imgbb

oh-imgbb 是一个基于 Tauri + React + TypeScript 的极简 ImgBB 爬虫 GUI。它优先解决「粘贴链接、解析预览、选择内容、稳定下载」这条主流程，并把收藏、缓存、设置持久化、登录和自己的个人空间管理作为内置能力。

## 技术栈

- 桌面端：Tauri 2
- 前端：React、TypeScript、Ant Design、Ant Design Icons
- 后端：Rust、anyhow、async/await
- 数据库：SQLite，由 sqlx 管理迁移和查询
- 爬虫能力：优先复用当前 workspace 中 `imgbb` 和 `llpha` 的解析、请求、下载能力

## 产品目标

- 入口简单：用户只需要粘贴 ImgBB 相册或用户空间地址。
- 预览明确：解析后先展示按比例缩放的缩略图、标题、数量、来源等信息，再决定下载。
- 详情清晰：点击缩略图后打开遮罩详情页，详情图通过临时文件多线程下载，不写入缓存。
- 下载可控：支持一键下载全部、勾选图片下载、从个人空间勾选相册批量加入下载任务，并可在下载页查看真实文件进度和取消。
- 本地友好：解析结构、缩略图、收藏、设置都保存在本地，重复打开尽量少请求网络。
- 可管理：登录后可进入“我的”管理账号相册，支持创建/删除相册、上传图片、拖拽上传和删除图片。

## 首版功能范围

### 1. 解析相册地址

- 支持输入 `https://ibb.co/album/{id}` 或省略协议的 `ibb.co/album/{id}`。
- 后端解析相册信息，返回相册名称、作者空间地址、图片列表。
- 前端以缩略图网格展示图片。
- 缩略图在固定区域内按图片自身比例显示，不拉伸变形。
- 点击缩略图打开详情遮罩页，支持放大、缩小、上一张、下一张和关闭。
- 详情页先展示缩略图并覆盖加载状态，详情图下载完成后自动替换为本地临时详情图。
- 详情图使用后端临时文件下载能力，符合条件时自动走分片并发下载，关闭或切换后不写入缓存。
- 相册图片支持按文件名即时搜索，输入变化后立即过滤显示结果。
- 相册图片支持分页展示，默认 20 张/页，可在设置页关闭分页或调整每页数量。
- 支持全选、反选、单张选择。
- 支持下载全部图片，点击后创建后台下载任务。
- 支持只下载选中的图片，点击后创建后台下载任务。
- 解析结果写入本地缓存，后续打开同一相册时优先展示缓存，再允许手动刷新。
- 相册结果标签页写入本地数据库，应用页面切换或重启后可恢复。

### 2. 解析用户个人空间

- 支持输入 `https://{name}.imgbb.com` 或 `https://{name}.imgbb.com/albums?list=albums`。
- 后端遍历个人空间中的相册列表。
- 前端展示相册封面、名称、地址、缓存状态。
- 个人空间相册支持按标题即时搜索，输入变化后立即过滤显示结果。
- 个人空间相册支持分页展示，默认 10 个/页，可在设置页关闭分页或调整每页数量。
- 支持选择一个或多个相册下载，批量任务按相册单位展示进度。
- 支持收藏个人空间，也支持直接收藏个人空间中选中的相册。
- 点击相册可以在解析页打开或复用相册详情标签页，复用相册缩略图浏览和图片选择下载能力。
- 个人空间结果标签页写入本地数据库，应用页面切换或重启后可恢复。

### 3. 收藏与缓存

- 收藏相册：保存相册名称、URL、作者、封面、图片数量、最近解析时间。
- 收藏个人空间：保存空间名称、URL、相册数量、最近解析时间。
- 缓存相册结构：保存图片 URL、缩略图 URL、原始文件名、排序。
- 缓存缩略图：下载到应用缓存目录，数据库保存本地路径和远程 URL 映射。
- 收藏页支持打开收藏的相册或个人空间，自动跳转到解析页对应标签。
- 收藏页以卡片展示收藏项，包含封面、标题、类型、地址和常用操作。
- 支持清理缩略图缓存、清理解析缓存、删除单条收藏。

### 4. 我的账号管理

- “我的”页复用解析页的浏览器式标签体验，固定保留“登录”和“个人空间”两个标签。
- 登录成功后会把用户名和密码保存到设置项，应用启动后立即尝试恢复登录。
- 登录适配 ImgBB 返回 `200 OK` 的情况，并继续通过 Cookie 和个人空间访问校验登录是否有效。
- “个人空间”标签按解析页个人空间逻辑访问自己的 `https://{name}.imgbb.com` 相册列表，并复用解析页头部、搜索、列表和分页样式。
- 当前登录账号自己的个人空间会使用登录 Cookie 解析首屏和后续 JSON 分页，避免退化成访客视角而漏掉私密相册。
- 支持创建相册、删除相册，所有删除操作都会弹窗确认。
- 点击账号相册后打开或复用管理版相册标签，支持搜索、分页、上传图片和拖动图片到当前相册标签上传。
- 已登录状态下解析相册详情会携带 Cookie 访问相册页、embeds 页和 JSON 内容接口，支持打开私密相册。
- 管理版相册详情支持删除单张图片，删除按钮位于每张图片右下角，删除前弹窗确认。
- 上传请求会按当前相册来源推断 `Origin`，避免用户子域和 `ibb.co` 相册页之间的上传跨域头不一致。
- 上传完成后会根据返回的图片 ID 再执行一次移动到目标相册的编辑请求，避免接口返回成功但图片落在账号根目录。

### 5. 页面导航

主导航包含：

- 解析：固定解析输入标签，相册/个人空间结果以浏览器式标签页打开，标签标题过长时省略并悬停显示完整名。
- 收藏：相册收藏、个人空间收藏、快速重新打开、删除收藏。
- 下载：查看后台下载任务、文件进度、完成/取消/失败状态，并支持取消任务。
- 我的：登录账号、查看自己的个人空间、以标签页打开子相册、创建/删除相册、上传/删除图片。
- 设置：下载目录、并发数、文件命名、缓存策略、解析分页。
- 启动时跟随系统亮暗模式，并监听系统变化同步 Ant Design 主题和自定义页面样式。

### 6. 设置与配置

- 默认下载目录，首次启动默认使用系统 Downloads 目录。
- 单文件命名模式：原文件名、计数命名、自定义模板。
- 最大并发下载数。
- 请求重试次数。
- 是否启用缩略图缓存。
- 缩略图缓存上限。
- 是否启动时恢复上次页面。
- 是否启用解析结果分页。
- 个人空间相册每页数量，默认 10。
- 相册图片每页数量，默认 20。
- 预留代理配置字段。
- 登录账号和密码保存在设置项中，不加密，用于启动时自动恢复登录。

## 推荐界面结构

```text
src/
  App.tsx
  main.tsx
  api/
    tauri_client.ts
    types.ts
  components/
    app_layout.tsx
    thumbnail_grid.tsx
    download_bar.tsx
    empty_state.tsx
  pages/
    parse_page.tsx
    favorites_page.tsx
    settings_page.tsx
    album_detail_page.tsx
    profile_detail_page.tsx
  state/
    app_store.ts
  styles/
    global.css
src-tauri/src/
  lib.rs
  commands/
    mod.rs
    parse.rs
    download.rs
    image_detail.rs
    favorite.rs
    settings.rs
  app_state.rs
  db/
    mod.rs
    models.rs
    repository.rs
  spider/
    mod.rs
    album.rs
    profile.rs
  download/
    mod.rs
    task.rs
  cache/
    mod.rs
    thumbnail.rs
  settings/
    mod.rs
```

前端采用 Ant Design 的 `Layout`、`Menu`、`Tabs`、`Card`、`Image`、`List`、`Table`、`Form`、`Progress`、`App` 消息系统。按钮图标优先使用 `@ant-design/icons`，例如下载、刷新、收藏、设置、删除、文件夹选择。

## 后端模块规划

### commands

Tauri command 是前后端唯一公开边界，所有返回值使用可序列化 DTO。

- `parse_album(url, refresh)`：解析相册并返回相册详情。
- `parse_profile(url, refresh)`：解析个人空间并返回相册列表。
- `download_album(album_url)`：创建整本相册后台下载任务。
- `download_album_images(album, image_ids)`：创建选中图片后台下载任务。
- `download_profile_albums(album_urls)`：创建个人空间选中相册批量下载任务。
- `list_download_tasks()`：读取运行期下载任务列表。
- `cancel_download_task(id)`：取消指定下载任务。
- `list_favorites(kind)`：读取收藏。
- `save_favorite(target)`：保存收藏。
- `remove_favorite(id)`：删除收藏。
- `get_settings()`：读取设置。
- `update_settings(settings)`：更新设置。
- `clear_cache(kind)`：清理缓存。

### spider

爬虫层负责把 ImgBB 页面和 JSON 接口转换成稳定结构。

- 相册解析需要从现有 `download_album` 拆出 `parse_album`，避免预览阶段直接下载。
- 个人空间解析可以复用现有 `list_profile_albums`。
- 下载时复用 `llpha` 的请求、重试、并发和文件保存能力。
- 解析错误用 anyhow 补充上下文，再转换成前端可读错误。

### db

SQLite 只保存本地状态，不保存敏感凭据。

建议迁移文件：

```text
src-tauri/migrations/
  0001_init.sql
```

建议表结构：

```sql
CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE favorites (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  url TEXT NOT NULL UNIQUE,
  cover_url TEXT,
  local_cover_path TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE album_cache (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  album_url TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  author_url TEXT,
  cover_url TEXT,
  image_count INTEGER NOT NULL,
  raw_json TEXT NOT NULL,
  parsed_at TEXT NOT NULL
);

CREATE TABLE image_cache (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  album_url TEXT NOT NULL,
  image_url TEXT NOT NULL,
  thumbnail_url TEXT,
  local_thumbnail_path TEXT,
  filename TEXT NOT NULL,
  sort_index INTEGER NOT NULL,
  selected INTEGER NOT NULL DEFAULT 0,
  UNIQUE(album_url, image_url)
);

CREATE TABLE profile_cache (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_url TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  album_count INTEGER NOT NULL,
  raw_json TEXT NOT NULL,
  parsed_at TEXT NOT NULL
);

CREATE TABLE download_tasks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  target_kind TEXT NOT NULL,
  target_url TEXT NOT NULL,
  status TEXT NOT NULL,
  total_items INTEGER NOT NULL DEFAULT 0,
  finished_items INTEGER NOT NULL DEFAULT 0,
  error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

## 前后端数据类型

前端和后端保持相同字段命名，使用 snake_case 作为 Tauri 边界字段，前端类型也跟随后端 DTO，避免转换层过厚。

```ts
export interface AlbumDetail {
  url: string;
  title: string;
  author_url?: string;
  images: AlbumImage[];
  cached: boolean;
  parsed_at: string;
}

export interface AlbumImage {
  id: string;
  filename: string;
  image_url: string;
  thumbnail_url?: string;
  local_thumbnail_path?: string;
  sort_index: number;
}

export interface ProfileDetail {
  url: string;
  title: string;
  albums: ProfileAlbum[];
  cached: boolean;
  parsed_at: string;
}

export interface ProfileAlbum {
  title: string;
  url: string;
  cover_url?: string;
  local_cover_path?: string;
}
```

## 关键流程

### 相册解析流程

```text
用户输入 URL
  -> 前端判断为空和基本格式
  -> invoke parse_album
  -> 后端规整 URL
  -> 命中缓存且 refresh=false 时返回缓存
  -> 请求 ImgBB 页面和 JSON
  -> 解析图片列表
  -> 写入 album_cache 和 image_cache
  -> 异步补充缩略图缓存
  -> 前端展示缩略图网格
```

### 相册下载流程

```text
用户点击下载全部或下载选中
  -> 前端传 album_url 和 image_ids
  -> 后端读取缓存中的图片列表
  -> 生成下载计划
  -> 创建 download_tasks
  -> 按设置并发下载
  -> 通过 Tauri event 推送进度
  -> 前端显示进度、完成数、失败信息
```

### 个人空间下载流程

```text
用户解析个人空间
  -> 前端展示相册列表
  -> 用户选择相册
  -> 后端逐个解析相册详情
  -> 复用相册下载流程
  -> 汇总整体进度
```

## 事件与进度

下载进度用 Tauri event 推送，避免前端轮询。

- `download://task_started`
- `download://item_started`
- `download://item_finished`
- `download://item_failed`
- `download://task_finished`

事件内容包含 `task_id`、`target_url`、`finished_items`、`total_items`、`current_filename`、`error_message`。

## 迭代计划

### Milestone 1：项目基础

- 安装 Ant Design、Ant Design Icons。
- 后端加入 anyhow、tokio、sqlx、serde、tracing 相关依赖。
- 配置 SQLite 数据库初始化和迁移。
- 搭建三层导航：解析、收藏、设置。
- 移除 Tauri 默认示例页面。

### Milestone 2：相册解析预览

- 从现有 `imgbb` 逻辑拆出只解析不下载的相册接口。
- 实现 `parse_album` command。
- 实现相册缩略图网格、选择栏、刷新按钮。
- 写入并读取相册缓存。

### Milestone 3：相册下载

- 实现下载设置。
- 实现全部下载和选中下载。
- 实现下载任务表和进度事件。
- 前端显示下载进度、失败提示和完成状态。

### Milestone 4：个人空间解析

- 实现 `parse_profile` command。
- 展示个人空间相册列表。
- 支持相册收藏、打开相册详情。
- 支持选择多个相册批量下载。

### Milestone 5：收藏、缓存、设置完善

- 完成收藏页。
- 完成缓存清理。
- 完成设置持久化。
- 增加最近解析记录和启动恢复。

### Milestone 6：登录、上传和账号空间管理

- 新增“我的”导航页，固定包含登录和个人空间标签，子相册以管理版标签打开。
- 实现登录 Cookie 保存、`200 OK` 登录响应适配和启动自动登录。
- 实现自己的个人空间相册创建、删除、图片上传、拖拽上传和图片删除。

## 需要优先确认的问题

- 是否将 `imgbb` crate 改成 library + binary 双入口，供 `oh-imgbb` 直接依赖。
- 下载目录是否默认使用系统下载目录，还是应用数据目录下的 `downloads`。
- 文件命名模板首版是否沿用当前 `{album}_{count}` 风格。
- 缩略图缓存是否需要大小上限，默认建议 64 MB。
- 个人空间批量下载时，是否默认每个相册单独建目录。

## 开发约定

- Rust 函数、变量使用 snake_case。
- Rust 结构体、枚举使用 PascalCase。
- 常量使用 UPPER_SNAKE_CASE。
- 每个函数和结构体前保留一行简短中文注释。
- 核心函数和 trait 接口补充更详细的中文文档。
- 错误处理使用 anyhow，并返回 Result。
- 异步逻辑使用 async/await。
- 测试直接写在实现文件后面。
- 单个非测试代码文件尽量不超过 500 行，按解析、下载、缓存、设置等职责拆分。
