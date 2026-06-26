# ImgBB 爬虫

`imgbb` 是基于 `llpha` 框架的站点级实现，包含：

- ImgBB 相册解析、图片列表输出和文件下载
- ImgBB 用户主页子专辑遍历和批量下载
- 可选登录态解析私有/账号相关内容
- 已登录账号下的相册创建、图片上传、图片/相册删除、背景图管理和图片编辑
- 独立 CLI 和配置示例

## 快速使用

```bash
imgbb album https://ibb.co/album/ABC123
imgbb album -o downloads https://ibb.co/album/ABC123
imgbb profile https://beautif11.imgbb.com/albums?list=albums
```

旧版子命令 `ibb-album` 和 `ibb-profile` 仍作为别名保留。

## CLI 命令

解析相册，输出可用于选中下载的图片 ID：

```bash
imgbb parse-album https://ibb.co/album/ABC123
imgbb parse-album --json https://ibb.co/album/ABC123
```

下载整本相册或指定图片：

```bash
imgbb album -o downloads --format "{album}_{count}_{name}" https://ibb.co/album/ABC123
imgbb images https://ibb.co/album/ABC123 --image-id https://i.ibb.co/demo/a.jpg
```

遍历个人空间，或下载个人空间中的全部/指定相册：

```bash
imgbb profile https://beautif11.imgbb.com/albums?list=albums
imgbb profile --json https://beautif11.imgbb.com/albums?list=albums
imgbb profile-download -o downloads https://beautif11.imgbb.com/albums?list=albums
imgbb profile-download https://beautif11.imgbb.com/albums?list=albums --album-url https://ibb.co/album/ABC123/
```

需要登录态的命令可以直接传参，也可以使用环境变量：

```bash
export IMGBB_LOGIN_SUBJECT="you@example.com"
export IMGBB_PASSWORD="your-password"

imgbb login
imgbb status
imgbb mine
imgbb mine --json
imgbb profile
imgbb create-album "Demo Album" --privacy private
imgbb upload-image ABC123 ./photo.jpg
imgbb edit-image IMAGE123 --title "New title" --description "New description"
imgbb delete-image IMAGE123
imgbb delete-album ABC123
imgbb upload-profile-background ./background.jpg
imgbb delete-profile-background
imgbb logout
```

也可以在单条命令中传入登录凭据：

```bash
imgbb login --login-subject you@example.com --password your-password
```

`login` 会把 Cookie 会话保存到当前目录的 `.imgbb-session.json`，不会保存密码。后续 `profile` 可以省略 URL，管理类命令也会自动复用这个登录态。可以通过全局参数 `--session <PATH>` 指定其他会话文件。

`mine` 专门列出当前已登录账号的相册，`my-albums` 和 `mine-albums` 是等价别名。CLI 模式会把日志级别固定为 `warn`，避免普通信息日志混入列表和 JSON 输出。

## 配置

默认读取运行目录下的 `config.toml`。可以从示例复制：

```bash
cp imgbb/config.example.toml config.toml
```

也可以显式指定配置：

```bash
imgbb --config imgbb/config.example.toml album https://ibb.co/album/ABC123
```
