# Llpha Workspace

这个仓库按职责拆成两个独立 crate：

- `llpha/`：轻量级 Rust 爬虫与数据分析框架，只保留通用能力。
- `imgbb/`：基于 `llpha` 的 ImgBB 爬虫实现，包含 CLI、相册下载和用户主页专辑遍历。

## 目录结构

```text
.
├── Cargo.toml
├── llpha/
│   ├── Cargo.toml
│   ├── README.md
│   ├── docs/
│   └── src/
└── imgbb/
    ├── Cargo.toml
    ├── README.md
    ├── config.example.toml
    └── src/
```

## 常用命令

```bash
cargo test --workspace
cargo run -p imgbb -- album https://ibb.co/album/ABC123
cargo run -p imgbb -- profile https://beautif11.imgbb.com/albums?list=albums
```

## 文档入口

- [Llpha 框架说明](./llpha/README.md)
- [Llpha 任务引擎文档](./llpha/docs/engine.md)
- [ImgBB 爬虫说明](./imgbb/README.md)
