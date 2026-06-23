mod cli;
mod ibb_spider;

use anyhow::Result;
use cli::{Cli, IbbAlbumArgs, IbbProfileArgs, ImgbbCommand};
use ibb_spider::IbbSpiderManager;
use llpha::*;

/// main 初始化框架并分发 ImgBB CLI 子命令。
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();
    let config = AppConfig::from_path(&cli.config)?;
    let _logging_guard = init_logging(&config.logging)?;
    LlphaClient::init_global(&config.request)?;

    match cli.command {
        ImgbbCommand::Album(args) => run_ibb_album(args).await?,
        ImgbbCommand::Profile(args) => run_ibb_profile(args).await?,
    }

    Ok(())
}

/// 执行 ImgBB 相册抓取和下载任务。
async fn run_ibb_album(args: IbbAlbumArgs) -> Result<()> {
    let report = IbbSpiderManager::new()
        .with_base_path(args.base_path)
        .download_album(args.url)
        .await?;

    println!(
        "下载完成: {} 个文件，{} 字节，目录 {}",
        report.download_summary.downloaded_files,
        report.download_summary.bytes_written,
        report.download_summary.directory.display()
    );
    println!("规整相册地址: {}", report.normalized_url);
    if let Some(author_url) = report.author_url {
        println!("作者相册地址: {author_url}");
    }

    Ok(())
}

/// 执行 ImgBB 用户主页子专辑遍历任务。
async fn run_ibb_profile(args: IbbProfileArgs) -> Result<()> {
    let report = IbbSpiderManager::new()
        .list_profile_albums(args.url)
        .await?;

    for album in report.albums {
        println!(
            "{}\t{}\n---\t{}",
            album.name,
            album.cover_url.as_deref().unwrap_or("-"),
            album.url,
        );
    }

    Ok(())
}
