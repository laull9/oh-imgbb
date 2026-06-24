//! cli 模块提供 ImgBB 命令行解析入口。

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use llpha::DEFAULT_CONFIG_PATH;

/// Cli 保存 ImgBB 命令行全局参数和子命令。
#[derive(Debug, Parser)]
#[command(name = "imgbb", version, about = "基于 Llpha 的 ImgBB 爬虫工具")]
pub struct Cli {
    /// config 指定全局配置文件路径。
    #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
    pub config: PathBuf,

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
}

/// IbbAlbumArgs 保存 ImgBB 相册任务参数。
#[derive(Debug, Parser)]
pub struct IbbAlbumArgs {
    /// url 指定 ImgBB 相册地址。
    pub url: String,

    /// base_path 指定下载基础目录，默认当前目录。
    #[arg(short = 'o', long, default_value = ".")]
    pub base_path: PathBuf,

    /// format 指定计数命名模板，支持 {count}、{album}、{name}。
    #[arg(long)]
    pub format: Option<String>,
}

/// IbbProfileArgs 保存 ImgBB 用户主页任务参数。
#[derive(Debug, Parser)]
pub struct IbbProfileArgs {
    /// url 指定 ImgBB 用户相册列表地址。
    pub url: String,
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
        match cli.command {
            ImgbbCommand::Album(args) => {
                assert_eq!(args.url, "https://ibb.co/album/ABC123");
                assert_eq!(args.base_path, PathBuf::from("downloads"));
                assert_eq!(args.format, None);
            }
            ImgbbCommand::Profile(_) => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证 ImgBB 子命令默认下载到当前目录。
    #[test]
    fn album_defaults_base_path_to_current_dir() {
        let cli = Cli::parse_from(["imgbb", "album", "https://ibb.co/album/ABC123"]);

        match cli.command {
            ImgbbCommand::Album(args) => {
                assert_eq!(args.base_path, PathBuf::from("."));
            }
            ImgbbCommand::Profile(_) => panic!("解析到了错误的子命令"),
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
                    args.format,
                    Some("{album}_{count}_{name}".to_string())
                );
            }
            ImgbbCommand::Profile(_) => panic!("解析到了错误的子命令"),
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
                assert_eq!(args.url, "https://beautif11.imgbb.com/albums?list=albums");
            }
            ImgbbCommand::Album(_) => panic!("解析到了错误的子命令"),
        }
    }

    /// 验证旧版 ibb-album 子命令仍然可以作为别名使用。
    #[test]
    fn old_ibb_album_command_is_kept_as_alias() {
        let cli = Cli::parse_from(["imgbb", "ibb-album", "https://ibb.co/album/ABC123"]);

        match cli.command {
            ImgbbCommand::Album(args) => assert_eq!(args.url, "https://ibb.co/album/ABC123"),
            ImgbbCommand::Profile(_) => panic!("解析到了错误的子命令"),
        }
    }
}
