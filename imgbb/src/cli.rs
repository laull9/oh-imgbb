//! cli 模块提供 ImgBB 命令行解析入口。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use llpha::DEFAULT_CONFIG_PATH;

/// DEFAULT_SESSION_PATH 表示 CLI 默认登录态文件。
pub const DEFAULT_SESSION_PATH: &str = ".imgbb-session.json";

/// Cli 保存 ImgBB 命令行全局参数和子命令。
#[derive(Debug, Parser)]
#[command(name = "imgbb", version, about = "基于 Llpha 的 ImgBB 爬虫工具")]
pub struct Cli {
    /// config 指定全局配置文件路径。
    #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
    pub config: PathBuf,

    /// session 指定 CLI 登录态文件路径。
    #[arg(long, default_value = DEFAULT_SESSION_PATH)]
    pub session: PathBuf,

    /// command 指定要执行的业务任务。
    #[command(subcommand)]
    pub command: ImgbbCommand,
}

impl Cli {
    /// 解析当前进程命令行参数。
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

/// ImgbbCommand 保存所有 ImgBB 业务子命令。
#[derive(Debug, Subcommand)]
pub enum ImgbbCommand {
    /// 抓取并下载 ImgBB 相册内容。
    #[command(alias = "ibb-album")]
    Album(IbbAlbumArgs),

    /// 遍历并列出 ImgBB 用户主页的子专辑。
    #[command(alias = "ibb-profile")]
    Profile(IbbProfileArgs),

    /// 列出当前已登录账号的相册。
    #[command(alias = "my-albums", alias = "mine-albums")]
    Mine(IbbMineArgs),

    /// 解析相册信息并输出图片列表。
    ParseAlbum(IbbParseAlbumArgs),

    /// 下载相册中的指定图片。
    Images(IbbImagesArgs),

    /// 下载用户主页中的一批相册。
    ProfileDownload(IbbProfileDownloadArgs),

    /// 校验登录凭据并输出登录态摘要。
    Login(IbbLoginArgs),

    /// 查看已保存的 CLI 登录态。
    Status,

    /// 删除已保存的 CLI 登录态。
    Logout,

    /// 创建已登录账号下的新相册。
    CreateAlbum(IbbCreateAlbumArgs),

    /// 上传图片到指定相册。
    UploadImage(IbbUploadImageArgs),

    /// 删除指定图片。
    DeleteImage(IbbDeleteImageArgs),

    /// 删除指定相册。
    DeleteAlbum(IbbDeleteAlbumArgs),

    /// 上传个人主页背景图。
    UploadProfileBackground(IbbUploadProfileBackgroundArgs),

    /// 删除个人主页背景图。
    DeleteProfileBackground(IbbDeleteProfileBackgroundArgs),

    /// 编辑图片标题、描述或所属相册。
    EditImage(IbbEditImageArgs),

    /// 使用聚合搜索引擎搜索公开网页。
    Search(IbbSearchArgs),
}

/// IbbAuthArgs 保存可选登录凭据参数。
#[derive(Clone, Debug, Default, Args)]
pub struct IbbAuthArgs {
    /// login_subject 指定 ImgBB 登录邮箱或用户名，也可用 IMGBB_LOGIN_SUBJECT。
    #[arg(long)]
    pub login_subject: Option<String>,

    /// password 指定 ImgBB 登录密码，也可用 IMGBB_PASSWORD。
    #[arg(long)]
    pub password: Option<String>,
}

/// IbbOutputArgs 保存通用输出参数。
#[derive(Clone, Debug, Default, Args)]
pub struct IbbOutputArgs {
    /// json 使用 JSON 输出完整结果。
    #[arg(long)]
    pub json: bool,
}

/// DEFAULT_SEARCH_DISPLAY_LIMIT 表示 CLI 默认搜索展示条数。
pub const DEFAULT_SEARCH_DISPLAY_LIMIT: usize = 20;

/// IbbDownloadOptions 保存下载目录和命名参数。
#[derive(Clone, Debug, Args)]
pub struct IbbDownloadOptions {
    /// base_path 指定下载基础目录，默认当前目录。
    #[arg(short = 'o', long, default_value = ".")]
    pub base_path: PathBuf,

    /// format 指定计数命名模板，支持 {count}、{album}、{name}。
    #[arg(long)]
    pub format: Option<String>,
}

impl Default for IbbDownloadOptions {
    /// 创建默认下载参数。
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("."),
            format: None,
        }
    }
}

/// IbbAlbumArgs 保存 ImgBB 相册任务参数。
#[derive(Debug, Parser)]
pub struct IbbAlbumArgs {
    /// url 指定 ImgBB 相册地址。
    pub url: String,

    /// download 保存下载相关选项。
    #[command(flatten)]
    pub download: IbbDownloadOptions,
}

/// IbbProfileArgs 保存 ImgBB 用户主页任务参数。
#[derive(Debug, Parser)]
pub struct IbbProfileArgs {
    /// url 指定 ImgBB 用户相册列表地址；省略时使用已保存登录态中的个人空间。
    pub url: Option<String>,

    /// auth 保存可选登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,

    /// output 保存输出格式。
    #[command(flatten)]
    pub output: IbbOutputArgs,
}

/// IbbMineArgs 保存当前账号相册列表参数。
#[derive(Debug, Parser)]
pub struct IbbMineArgs {
    /// auth 保存可选登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,

    /// output 保存输出格式。
    #[command(flatten)]
    pub output: IbbOutputArgs,
}

/// IbbParseAlbumArgs 保存相册解析参数。
#[derive(Debug, Parser)]
pub struct IbbParseAlbumArgs {
    /// url 指定 ImgBB 相册地址。
    pub url: String,

    /// auth 保存可选登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,

    /// output 保存输出格式。
    #[command(flatten)]
    pub output: IbbOutputArgs,
}

/// IbbImagesArgs 保存选中图片下载参数。
#[derive(Debug, Parser)]
pub struct IbbImagesArgs {
    /// url 指定 ImgBB 相册地址。
    pub url: String,

    /// image_id 指定要下载的图片 ID，可重复传入；通常等于解析结果中的 image_url。
    #[arg(long = "image-id", required = true)]
    pub image_ids: Vec<String>,

    /// download 保存下载相关选项。
    #[command(flatten)]
    pub download: IbbDownloadOptions,

    /// auth 保存可选登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbProfileDownloadArgs 保存个人空间批量下载参数。
#[derive(Debug, Parser)]
pub struct IbbProfileDownloadArgs {
    /// url 指定 ImgBB 用户相册列表地址；省略时使用已保存登录态中的个人空间。
    pub url: Option<String>,

    /// album_url 指定要下载的相册地址，可重复传入；不传则下载当前个人空间全部相册。
    #[arg(long = "album-url")]
    pub album_urls: Vec<String>,

    /// download 保存下载相关选项。
    #[command(flatten)]
    pub download: IbbDownloadOptions,

    /// auth 保存可选登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbLoginArgs 保存登录验证参数。
#[derive(Debug, Parser)]
pub struct IbbLoginArgs {
    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbPrivacyArg 表示 CLI 中的相册隐私参数。
#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum IbbPrivacyArg {
    Public,
    Private,
    Password,
}

/// IbbCreateAlbumArgs 保存创建相册参数。
#[derive(Debug, Parser)]
pub struct IbbCreateAlbumArgs {
    /// name 指定相册名称。
    pub name: String,

    /// description 指定相册描述。
    #[arg(long)]
    pub description: Option<String>,

    /// privacy 指定相册可见性。
    #[arg(long, value_enum, default_value_t = IbbPrivacyArg::Public)]
    pub privacy: IbbPrivacyArg,

    /// album_password 指定密码相册的访问密码。
    #[arg(long = "album-password")]
    pub album_password: Option<String>,

    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbUploadImageArgs 保存上传图片参数。
#[derive(Debug, Parser)]
pub struct IbbUploadImageArgs {
    /// album_id 指定目标相册 ID。
    pub album_id: String,

    /// file_path 指定本地图片路径。
    pub file_path: PathBuf,

    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbDeleteImageArgs 保存删除图片参数。
#[derive(Debug, Parser)]
pub struct IbbDeleteImageArgs {
    /// image_id 指定要删除的图片 ID。
    pub image_id: String,

    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbDeleteAlbumArgs 保存删除相册参数。
#[derive(Debug, Parser)]
pub struct IbbDeleteAlbumArgs {
    /// album_id 指定要删除的相册 ID。
    pub album_id: String,

    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbUploadProfileBackgroundArgs 保存上传背景图参数。
#[derive(Debug, Parser)]
pub struct IbbUploadProfileBackgroundArgs {
    /// file_path 指定本地背景图路径。
    pub file_path: PathBuf,

    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbDeleteProfileBackgroundArgs 保存删除背景图参数。
#[derive(Debug, Parser)]
pub struct IbbDeleteProfileBackgroundArgs {
    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbEditImageArgs 保存编辑图片参数。
#[derive(Debug, Parser)]
pub struct IbbEditImageArgs {
    /// image_id 指定要编辑的图片 ID。
    pub image_id: String,

    /// title 指定新的图片标题。
    #[arg(long)]
    pub title: Option<String>,

    /// description 指定新的图片描述。
    #[arg(long)]
    pub description: Option<String>,

    /// album_id 指定目标相册 ID。
    #[arg(long)]
    pub album_id: Option<String>,

    /// new_album 指定是否移动到新相册。
    #[arg(long, default_value_t = false)]
    pub new_album: bool,

    /// auth 保存登录凭据。
    #[command(flatten)]
    pub auth: IbbAuthArgs,
}

/// IbbSearchArgs 保存聚合搜索参数。
#[derive(Debug, Parser)]
pub struct IbbSearchArgs {
    /// query 指定搜索关键词。
    #[arg(required = true)]
    pub query: Vec<String>,

    /// limit 指定搜索结果条数。
    #[arg(short, long, default_value_t = DEFAULT_SEARCH_DISPLAY_LIMIT)]
    pub limit: usize,

    /// output 保存输出格式。
    #[command(flatten)]
    pub output: IbbOutputArgs,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 CLI 可以解析通用配置路径和 ImgBB 相册子命令。
    #[test]
    fn cli_parses_config_and_album_command() {
        let cli = Cli::parse_from([
            "imgbb",
            "--config",
            "custom.toml",
            "album",
            "--base-path",
            "downloads",
            "https://ibb.co/album/ABC123",
        ]);

        assert_eq!(cli.config, PathBuf::from("custom.toml"));
        assert_eq!(cli.session, PathBuf::from(DEFAULT_SESSION_PATH));
        match cli.command {
            ImgbbCommand::Album(args) => {
                assert_eq!(args.url, "https://ibb.co/album/ABC123");
                assert_eq!(args.download.base_path, PathBuf::from("downloads"));
                assert_eq!(args.download.format, None);
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 ImgBB 子命令默认下载到当前目录。
    #[test]
    fn album_defaults_base_path_to_current_dir() {
        let cli = Cli::parse_from(["imgbb", "album", "https://ibb.co/album/ABC123"]);

        match cli.command {
            ImgbbCommand::Album(args) => {
                assert_eq!(args.download.base_path, PathBuf::from("."));
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 ImgBB 相册子命令可以解析计数命名模板。
    #[test]
    fn album_accepts_format() {
        let cli = Cli::parse_from([
            "imgbb",
            "album",
            "--format",
            "{album}_{count}_{name}",
            "https://ibb.co/album/ABC123",
        ]);

        match cli.command {
            ImgbbCommand::Album(args) => {
                assert_eq!(
                    args.download.format,
                    Some("{album}_{count}_{name}".to_string())
                );
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 CLI 可以解析 ImgBB 用户主页子命令。
    #[test]
    fn cli_parses_profile_command() {
        let cli = Cli::parse_from([
            "imgbb",
            "profile",
            "https://beautif11.imgbb.com/albums?list=albums",
        ]);

        match cli.command {
            ImgbbCommand::Profile(args) => {
                assert_eq!(
                    args.url.as_deref(),
                    Some("https://beautif11.imgbb.com/albums?list=albums")
                );
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 CLI 可以省略个人空间 URL。
    #[test]
    fn profile_url_is_optional() {
        let cli = Cli::parse_from(["imgbb", "profile"]);

        match cli.command {
            ImgbbCommand::Profile(args) => assert!(args.url.is_none()),
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 CLI 可以解析当前账号相册列表子命令。
    #[test]
    fn cli_parses_mine_command() {
        let cli = Cli::parse_from(["imgbb", "mine", "--json"]);

        match cli.command {
            ImgbbCommand::Mine(args) => assert!(args.output.json),
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证当前账号相册列表子命令保留易读别名。
    #[test]
    fn my_albums_alias_maps_to_mine_command() {
        let cli = Cli::parse_from(["imgbb", "my-albums"]);

        match cli.command {
            ImgbbCommand::Mine(args) => assert!(!args.output.json),
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证旧版 ibb-album 子命令仍然可以作为别名使用。
    #[test]
    fn old_ibb_album_command_is_kept_as_alias() {
        let cli = Cli::parse_from(["imgbb", "ibb-album", "https://ibb.co/album/ABC123"]);

        match cli.command {
            ImgbbCommand::Album(args) => assert_eq!(args.url, "https://ibb.co/album/ABC123"),
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 CLI 可以解析相册解析子命令和 JSON 输出。
    #[test]
    fn cli_parses_parse_album_command() {
        let cli = Cli::parse_from([
            "imgbb",
            "parse-album",
            "--json",
            "https://ibb.co/album/ABC123",
        ]);

        match cli.command {
            ImgbbCommand::ParseAlbum(args) => {
                assert_eq!(args.url, "https://ibb.co/album/ABC123");
                assert!(args.output.json);
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 CLI 可以解析选中图片下载子命令。
    #[test]
    fn cli_parses_images_command() {
        let cli = Cli::parse_from([
            "imgbb",
            "images",
            "--image-id",
            "https://i.ibb.co/a.jpg",
            "--image-id",
            "https://i.ibb.co/b.jpg",
            "https://ibb.co/album/ABC123",
        ]);

        match cli.command {
            ImgbbCommand::Images(args) => {
                assert_eq!(args.image_ids.len(), 2);
                assert_eq!(args.download.base_path, PathBuf::from("."));
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 CLI 可以解析管理类子命令。
    #[test]
    fn cli_parses_create_album_command() {
        let cli = Cli::parse_from([
            "imgbb",
            "create-album",
            "--login-subject",
            "demo@example.com",
            "--password",
            "secret",
            "--privacy",
            "private",
            "Demo",
        ]);

        match cli.command {
            ImgbbCommand::CreateAlbum(args) => {
                assert_eq!(args.name, "Demo");
                assert_eq!(args.privacy, IbbPrivacyArg::Private);
                assert_eq!(args.auth.login_subject.as_deref(), Some("demo@example.com"));
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 CLI 可以解析聚合搜索子命令。
    #[test]
    fn cli_parses_search_command() {
        let cli = Cli::parse_from(["imgbb", "search", "--json", "rust", "async"]);

        match cli.command {
            ImgbbCommand::Search(args) => {
                assert_eq!(args.query, vec!["rust".to_string(), "async".to_string()]);
                assert_eq!(args.limit, DEFAULT_SEARCH_DISPLAY_LIMIT);
                assert!(args.output.json);
            }
            _ => panic!("解析到了错误的子命令"),
        }
    }
}
