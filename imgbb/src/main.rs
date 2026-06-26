mod cli;
mod session;

use anyhow::{Context, Result};
use cli::*;
use imgbb::ibb_spider::*;
use llpha::*;
use session::{
    CliContext, load_saved_session, optional_login_session, print_session_summary,
    require_login_session, resolve_credentials, resolve_profile_url, save_session,
};

/// main 初始化框架并分发 ImgBB CLI 子命令。
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();
    let mut config = AppConfig::from_path(&cli.config)?;
    apply_cli_logging_defaults(&mut config);
    let _logging_guard = init_logging(&config.logging)?;
    LlphaClient::init_global(&config.request)?;
    let context = CliContext {
        session_path: cli.session.clone(),
    };

    match cli.command {
        ImgbbCommand::Album(args) => run_ibb_album(args).await?,
        ImgbbCommand::Profile(args) => run_ibb_profile(&context, args).await?,
        ImgbbCommand::Mine(args) => run_mine(&context, args).await?,
        ImgbbCommand::ParseAlbum(args) => run_parse_album(&context, args).await?,
        ImgbbCommand::Images(args) => run_ibb_images(&context, args).await?,
        ImgbbCommand::ProfileDownload(args) => run_profile_download(&context, args).await?,
        ImgbbCommand::Login(args) => run_login(&context, args).await?,
        ImgbbCommand::Status => run_status(&context)?,
        ImgbbCommand::Logout => run_logout(&context)?,
        ImgbbCommand::CreateAlbum(args) => run_create_album(&context, args).await?,
        ImgbbCommand::UploadImage(args) => run_upload_image(&context, args).await?,
        ImgbbCommand::DeleteImage(args) => run_delete_image(&context, args).await?,
        ImgbbCommand::DeleteAlbum(args) => run_delete_album(&context, args).await?,
        ImgbbCommand::UploadProfileBackground(args) => {
            run_upload_profile_background(&context, args).await?
        }
        ImgbbCommand::DeleteProfileBackground(args) => {
            run_delete_profile_background(&context, args).await?
        }
        ImgbbCommand::EditImage(args) => run_edit_image(&context, args).await?,
        ImgbbCommand::Search(args) => run_search(args).await?,
    }

    Ok(())
}

/// 应用 CLI 模式的日志默认值。
fn apply_cli_logging_defaults(config: &mut AppConfig) {
    config.logging.level = "warn".to_string();
}

/// 执行 ImgBB 相册抓取和下载任务。
async fn run_ibb_album(args: IbbAlbumArgs) -> Result<()> {
    let IbbAlbumArgs {
        url,
        download: IbbDownloadOptions { base_path, format },
    } = args;
    let manager = configured_manager(base_path, format);
    let report = manager.download_album(url).await?;

    print_download_summary(
        report.download_summary.downloaded_files,
        report.download_summary.bytes_written,
        &report.download_summary.directory.display().to_string(),
    );
    println!("规整相册地址: {}", report.normalized_url);
    if let Some(author_url) = report.author_url {
        println!("作者相册地址: {author_url}");
    }

    Ok(())
}

/// 执行 ImgBB 用户主页子专辑遍历任务。
async fn run_ibb_profile(context: &CliContext, args: IbbProfileArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = optional_login_session(context, &manager, &args.auth).await?;
    let url = resolve_profile_url(args.url, session.as_ref())?;
    let report = list_profile_albums(&manager, session.as_ref(), &url).await?;

    if args.output.json {
        print_json(&report)?;
        return Ok(());
    }

    print_profile_report(&report);

    Ok(())
}

/// 执行当前已登录账号相册列表任务。
async fn run_mine(context: &CliContext, args: IbbMineArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = list_profile_albums(&manager, Some(&session), &session.profile.url).await?;

    if args.output.json {
        print_json(&report)?;
        return Ok(());
    }

    print_profile_report(&report);

    Ok(())
}

/// 执行 ImgBB 相册解析任务。
async fn run_parse_album(context: &CliContext, args: IbbParseAlbumArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = optional_login_session(context, &manager, &args.auth).await?;
    let detail = parse_album_detail(&manager, session.as_ref(), &args.url).await?;

    if args.output.json {
        print_json(&detail)?;
        return Ok(());
    }

    print_album_detail(&detail);

    Ok(())
}

/// 执行 ImgBB 相册选中图片下载任务。
async fn run_ibb_images(context: &CliContext, args: IbbImagesArgs) -> Result<()> {
    let IbbImagesArgs {
        url,
        image_ids,
        download: IbbDownloadOptions { base_path, format },
        auth,
    } = args;
    let manager = configured_manager(base_path, format);
    let session = optional_login_session(context, &manager, &auth).await?;
    let detail = parse_album_detail(&manager, session.as_ref(), &url).await?;
    let report = manager
        .download_album_images(&detail, &image_ids)
        .await
        .context("下载选中图片失败")?;

    print_download_summary(
        report.download_summary.downloaded_files,
        report.download_summary.bytes_written,
        &report.download_summary.directory.display().to_string(),
    );
    println!("规整相册地址: {}", report.normalized_url);

    Ok(())
}

/// 执行 ImgBB 用户主页批量下载任务。
async fn run_profile_download(context: &CliContext, args: IbbProfileDownloadArgs) -> Result<()> {
    let IbbProfileDownloadArgs {
        url,
        album_urls,
        download: IbbDownloadOptions { base_path, format },
        auth,
    } = args;
    let manager = configured_manager(base_path, format);
    let session = optional_login_session(context, &manager, &auth).await?;
    let urls = if album_urls.is_empty() {
        let url = resolve_profile_url(url, session.as_ref())?;
        list_profile_albums(&manager, session.as_ref(), &url)
            .await?
            .albums
            .into_iter()
            .map(|album| album.url)
            .collect::<Vec<_>>()
    } else {
        album_urls
    };
    let total = urls.len();
    let mut downloaded_files = 0usize;
    let mut bytes_written = 0usize;

    for album_url in urls {
        let report = manager.download_album(&album_url).await?;
        downloaded_files =
            downloaded_files.saturating_add(report.download_summary.downloaded_files);
        bytes_written = bytes_written.saturating_add(report.download_summary.bytes_written);
        println!(
            "已下载相册: {}\t{} 个文件",
            report.normalized_url, report.download_summary.downloaded_files
        );
    }

    println!("批量下载完成: {total} 个相册");
    print_download_summary(downloaded_files, bytes_written, "-");

    Ok(())
}

/// 执行 ImgBB 登录验证任务。
async fn run_login(context: &CliContext, args: IbbLoginArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let (login_subject, password) = resolve_credentials(&args.auth)?
        .ok_or_else(|| anyhow::anyhow!("login 命令需要 --login-subject/--password"))?;
    let session = manager.login(login_subject, password).await?;
    save_session(&context.session_path, &session)?;

    println!("登录成功: {}", session.login_subject);
    println!("登录态已保存: {}", context.session_path.display());
    println!("跳转地址: {}", session.redirect_url);
    println!("个人空间: {}", session.profile.url);
    println!("JSON 接口: {}", session.profile.json_url);
    println!("Owner ID: {}", session.profile.owner_id);

    Ok(())
}

/// 输出已保存的 CLI 登录态。
fn run_status(context: &CliContext) -> Result<()> {
    let Some(session) = load_saved_session(&context.session_path)? else {
        println!("未保存登录态: {}", context.session_path.display());
        return Ok(());
    };

    println!("已保存登录态: {}", context.session_path.display());
    print_session_summary(&session);

    Ok(())
}

/// 删除已保存的 CLI 登录态。
fn run_logout(context: &CliContext) -> Result<()> {
    match std::fs::remove_file(&context.session_path) {
        Ok(()) => println!("已删除登录态: {}", context.session_path.display()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            println!("未保存登录态: {}", context.session_path.display());
        }
        Err(err) => return Err(err).context("删除登录态文件失败"),
    }

    Ok(())
}

/// 执行 ImgBB 创建相册任务。
async fn run_create_album(context: &CliContext, args: IbbCreateAlbumArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = manager
        .create_album(
            &session,
            IbbCreateAlbumInput {
                name: args.name,
                description: args.description,
                privacy: privacy_arg_to_model(args.privacy),
                password: args.album_password,
            },
        )
        .await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 上传图片任务。
async fn run_upload_image(context: &CliContext, args: IbbUploadImageArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = manager
        .upload_album_image(&session, args.album_id, args.file_path)
        .await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 删除图片任务。
async fn run_delete_image(context: &CliContext, args: IbbDeleteImageArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = manager.delete_image(&session, args.image_id).await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 删除相册任务。
async fn run_delete_album(context: &CliContext, args: IbbDeleteAlbumArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = manager.delete_album(&session, args.album_id).await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 上传个人空间背景图任务。
async fn run_upload_profile_background(
    context: &CliContext,
    args: IbbUploadProfileBackgroundArgs,
) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = manager
        .upload_profile_background(&session, args.file_path)
        .await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 删除个人空间背景图任务。
async fn run_delete_profile_background(
    context: &CliContext,
    args: IbbDeleteProfileBackgroundArgs,
) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = manager.delete_profile_background(&session).await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 编辑图片任务。
async fn run_edit_image(context: &CliContext, args: IbbEditImageArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(context, &manager, &args.auth).await?;
    let report = manager
        .edit_image(
            &session,
            IbbEditImageInput {
                image_id: args.image_id,
                title: args.title,
                description: args.description,
                album_id: args.album_id,
                new_album: args.new_album,
            },
        )
        .await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行聚合搜索任务。
async fn run_search(args: IbbSearchArgs) -> Result<()> {
    let search = AggregateSearch::builder().limit(args.limit).build()?;
    let query = args.query.join(" ");
    let response = search.search(&query).await?;

    if args.output.json {
        print_json(&response)?;
        return Ok(());
    }

    print_search_results(&response);

    Ok(())
}

/// 创建按 CLI 下载参数配置好的管理器。
fn configured_manager(base_path: std::path::PathBuf, format: Option<String>) -> IbbSpiderManager {
    let mut manager = IbbSpiderManager::new().with_base_path(base_path);
    if let Some(pattern) = format {
        manager = manager.with_file_name_pattern(pattern);
    }

    manager
}

/// 按可选登录态解析相册详情。
async fn parse_album_detail(
    manager: &IbbSpiderManager,
    session: Option<&IbbLoginSession>,
    url: &str,
) -> Result<IbbAlbumDetail> {
    if let Some(session) = session {
        return manager.parse_authenticated_album(session, url).await;
    }

    manager.parse_album(url).await
}

/// 按可选登录态遍历个人空间相册。
async fn list_profile_albums(
    manager: &IbbSpiderManager,
    session: Option<&IbbLoginSession>,
    url: &str,
) -> Result<IbbProfileReport> {
    if let Some(session) = session {
        return manager
            .list_authenticated_profile_albums(session, url)
            .await;
    }

    manager.list_profile_albums(url).await
}

/// 输出个人空间相册列表。
fn print_profile_report(report: &IbbProfileReport) {
    for album in &report.albums {
        println!(
            "{}\t{}\n---\t{}",
            album.name,
            album.cover_url.as_deref().unwrap_or("-"),
            album.url,
        );
    }
}

/// 输出相册解析详情。
fn print_album_detail(detail: &IbbAlbumDetail) {
    println!("相册: {}", detail.title);
    println!("地址: {}", detail.url);
    if let Some(author_url) = &detail.author_url {
        println!("作者相册地址: {author_url}");
    }
    println!("图片数量: {}", detail.images.len());
    for image in &detail.images {
        println!(
            "{}\t{}\t{}",
            image.sort_index,
            image.filename,
            image.thumbnail_url.as_deref().unwrap_or("-")
        );
        println!("---\t{}", image.image_url);
    }
}

/// 输出搜索结果。
fn print_search_results(response: &SearchResponse) {
    println!("搜索引擎: {:?}", response.engine);
    println!("关键词: {}", response.query);
    println!("结果数: {}", response.results.len());
    for (index, result) in response.results.iter().enumerate() {
        println!(
            "{}. {}\n{}\n{}",
            index + 1,
            result.title,
            result.url,
            result.snippet
        );
    }
}

/// 输出下载摘要。
fn print_download_summary(downloaded_files: usize, bytes_written: usize, directory: &str) {
    println!("下载完成: {downloaded_files} 个文件，{bytes_written} 字节，目录 {directory}");
}

/// 输出管理接口摘要和原始 JSON。
fn print_api_report(report: &IbbApiReport) -> Result<()> {
    println!("状态码: {}", report.status_code);
    if let Some(id) = &report.id {
        println!("ID: {id}");
    }
    if let Some(url) = &report.url {
        println!("URL: {url}");
    }
    print_json(&report.raw)
}

/// 输出格式化 JSON。
fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// 转换 CLI 隐私枚举为库模型。
fn privacy_arg_to_model(privacy: IbbPrivacyArg) -> IbbAlbumPrivacy {
    match privacy {
        IbbPrivacyArg::Public => IbbAlbumPrivacy::Public,
        IbbPrivacyArg::Private => IbbAlbumPrivacy::Private,
        IbbPrivacyArg::Password => IbbAlbumPrivacy::Password,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证隐私参数可以转换为库模型。
    #[test]
    fn privacy_arg_converts_to_model() {
        assert_eq!(
            privacy_arg_to_model(IbbPrivacyArg::Password),
            IbbAlbumPrivacy::Password
        );
    }

    /// 验证 CLI 模式会把日志级别降到 warn。
    #[test]
    fn cli_logging_defaults_to_warn() {
        let mut config = AppConfig::default();
        config.logging.level = "info".to_string();

        apply_cli_logging_defaults(&mut config);

        assert_eq!(config.logging.level, "warn");
    }
}
