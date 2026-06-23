# ImgBB 爬虫

`imgbb` 是基于 `llpha` 框架的站点级实现，包含：

- ImgBB 相册内容抓取和文件下载
- ImgBB 用户主页子专辑遍历
- 独立 CLI 和配置示例

## 使用方式

```bash
cargo run -p imgbb -- album https://ibb.co/album/ABC123
cargo run -p imgbb -- album -o downloads https://ibb.co/album/ABC123
cargo run -p imgbb -- profile https://beautif11.imgbb.com/albums?list=albums
```

旧版子命令 `ibb-album` 和 `ibb-profile` 仍作为别名保留。

## 配置

默认读取运行目录下的 `config.toml`。可以从示例复制：

```bash
cp imgbb/config.example.toml config.toml
```

也可以显式指定配置：

```bash
cargo run -p imgbb -- --config imgbb/config.example.toml album https://ibb.co/album/ABC123
```
