mod cli;

use std::env;

use anyhow::{Context, Result, bail};
use cli::*;
use imgbb::ibb_spider::*;
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
        ImgbbCommand::ParseAlbum(args) => run_parse_album(args).await?,
        ImgbbCommand::Images(args) => run_ibb_images(args).await?,
        ImgbbCommand::ProfileDownload(args) => run_profile_download(args).await?,
        ImgbbCommand::Login(args) => run_login(args).await?,
        ImgbbCommand::CreateAlbum(args) => run_create_album(args).await?,
        ImgbbCommand::UploadImage(args) => run_upload_image(args).await?,
        ImgbbCommand::DeleteImage(args) => run_delete_image(args).await?,
        ImgbbCommand::DeleteAlbum(args) => run_delete_album(args).await?,
        ImgbbCommand::UploadProfileBackground(args) => run_upload_profile_background(args).await?,
        ImgbbCommand::DeleteProfileBackground(args) => run_delete_profile_background(args).await?,
        ImgbbCommand::EditImage(args) => run_edit_image(args).await?,
    }

    Ok(())
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
async fn run_ibb_profile(args: IbbProfileArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = optional_login_session(&manager, &args.auth).await?;
    let report = list_profile_albums(&manager, session.as_ref(), &args.url).await?;

    if args.output.json {
        print_json(&report)?;
        return Ok(());
    }

    print_profile_report(&report);

    Ok(())
}

/// 执行 ImgBB 相册解析任务。
async fn run_parse_album(args: IbbParseAlbumArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = optional_login_session(&manager, &args.auth).await?;
    let detail = parse_album_detail(&manager, session.as_ref(), &args.url).await?;

    if args.output.json {
        print_json(&detail)?;
        return Ok(());
    }

    print_album_detail(&detail);

    Ok(())
}

/// 执行 ImgBB 相册选中图片下载任务。
async fn run_ibb_images(args: IbbImagesArgs) -> Result<()> {
    let IbbImagesArgs {
        url,
        image_ids,
        download: IbbDownloadOptions { base_path, format },
        auth,
    } = args;
    let manager = configured_manager(base_path, format);
    let session = optional_login_session(&manager, &auth).await?;
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
async fn run_profile_download(args: IbbProfileDownloadArgs) -> Result<()> {
    let IbbProfileDownloadArgs {
        url,
        album_urls,
        download: IbbDownloadOptions { base_path, format },
        auth,
    } = args;
    let manager = configured_manager(base_path, format);
    let session = optional_login_session(&manager, &auth).await?;
    let urls = if album_urls.is_empty() {
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
async fn run_login(args: IbbLoginArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;

    println!("登录成功: {}", session.login_subject);
    println!("跳转地址: {}", session.redirect_url);
    println!("个人空间: {}", session.profile.url);
    println!("JSON 接口: {}", session.profile.json_url);
    println!("Owner ID: {}", session.profile.owner_id);

    Ok(())
}

/// 执行 ImgBB 创建相册任务。
async fn run_create_album(args: IbbCreateAlbumArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;
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
async fn run_upload_image(args: IbbUploadImageArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;
    let report = manager
        .upload_album_image(&session, args.album_id, args.file_path)
        .await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 删除图片任务。
async fn run_delete_image(args: IbbDeleteImageArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;
    let report = manager.delete_image(&session, args.image_id).await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 删除相册任务。
async fn run_delete_album(args: IbbDeleteAlbumArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;
    let report = manager.delete_album(&session, args.album_id).await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 上传个人空间背景图任务。
async fn run_upload_profile_background(args: IbbUploadProfileBackgroundArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;
    let report = manager
        .upload_profile_background(&session, args.file_path)
        .await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 删除个人空间背景图任务。
async fn run_delete_profile_background(args: IbbDeleteProfileBackgroundArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;
    let report = manager.delete_profile_background(&session).await?;

    print_api_report(&report)?;

    Ok(())
}

/// 执行 ImgBB 编辑图片任务。
async fn run_edit_image(args: IbbEditImageArgs) -> Result<()> {
    let manager = IbbSpiderManager::new();
    let session = require_login_session(&manager, &args.auth).await?;
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

/// 创建按 CLI 下载参数配置好的管理器。
fn configured_manager(base_path: std::path::PathBuf, format: Option<String>) -> IbbSpiderManager {
    let mut manager = IbbSpiderManager::new().with_base_path(base_path);
    if let Some(pattern) = format {
        manager = manager.with_file_name_pattern(pattern);
    }

    manager
}

/// 返回可选登录会话。
async fn optional_login_session(
    manager: &IbbSpiderManager,
    auth: &IbbAuthArgs,
) -> Result<Option<IbbLoginSession>> {
    let Some((login_subject, password)) = resolve_credentials(auth)? else {
        return Ok(None);
    };

    manager.login(login_subject, password).await.map(Some)
}

/// 返回必须存在的登录会话。
async fn require_login_session(
    manager: &IbbSpiderManager,
    auth: &IbbAuthArgs,
) -> Result<IbbLoginSession> {
    let Some((login_subject, password)) = resolve_credentials(auth)? else {
        bail!(
            "此命令需要登录凭据，请传入 --login-subject/--password 或设置 IMGBB_LOGIN_SUBJECT/IMGBB_PASSWORD"
        );
    };

    manager.login(login_subject, password).await
}

/// 从 CLI 参数或环境变量读取登录凭据。
fn resolve_credentials(auth: &IbbAuthArgs) -> Result<Option<(String, String)>> {
    let login_subject = auth
        .login_subject
        .clone()
        .or_else(|| env::var("IMGBB_LOGIN_SUBJECT").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let password = auth
        .password
        .clone()
        .or_else(|| env::var("IMGBB_PASSWORD").ok())
        .filter(|value| !value.is_empty());

    match (login_subject, password) {
        (Some(login_subject), Some(password)) => Ok(Some((login_subject, password))),
        (None, None) => Ok(None),
        _ => bail!("登录账号和密码必须同时提供"),
    }
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

    /// 验证空登录参数不会触发登录。
    #[test]
    fn credentials_are_optional() {
        let credentials = resolve_credentials(&IbbAuthArgs::default()).unwrap();

        assert!(credentials.is_none());
    }

    /// 验证隐私参数可以转换为库模型。
    #[test]
    fn privacy_arg_converts_to_model() {
        assert_eq!(
            privacy_arg_to_model(IbbPrivacyArg::Password),
            IbbAlbumPrivacy::Password
        );
    }
}
