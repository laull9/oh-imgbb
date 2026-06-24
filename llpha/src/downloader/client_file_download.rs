use anyhow::{Context, Result, anyhow};
use reqwest::{
    Method,
    header::{ACCEPT_RANGES, CONTENT_LENGTH, HeaderMap, HeaderValue},
};
use std::path::Path;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

#[path = "client_range_download.rs"]
mod client_range_download;

use crate::downloader::client::LlphaClient;
use crate::downloader::progress::{
    DownloadProgress, DownloadProgressCallback, DownloadProgressEvent, emit_download_progress,
};
use crate::downloader::request::{FetchRequest, HttpMethod, SavedDownload};

/// DownloadProbe 保存下载前探测到的服务端能力。
#[derive(Clone, Debug)]
struct DownloadProbe {
    content_length: Option<u64>,
    accepts_ranges: bool,
    headers: HeaderMap,
}
/// 下载已准备好的请求并按需启用分片。
pub(super) async fn download_prepared_request_to_file(
    client: &LlphaClient,
    request: &FetchRequest,
    path: &Path,
    callback: Option<DownloadProgressCallback>,
) -> Result<SavedDownload> {
    ensure_parent_dir(path).await?;

    let probe = probe_download(client, request)
        .await
        .unwrap_or(DownloadProbe {
            content_length: None,
            accepts_ranges: false,
            headers: HeaderMap::new(),
        });

    if client_range_download::should_use_parallel_download(request, &probe) {
        return client_range_download::download_parallel_request_to_file(
            client, request, path, probe, callback,
        )
        .await;
    }

    download_single_request_to_file_with_retry(client, request, path, callback).await
}
/// 探测下载目标的长度和 Range 支持能力。
async fn probe_download(client: &LlphaClient, request: &FetchRequest) -> Result<DownloadProbe> {
    if request.method != HttpMethod::Get {
        return Ok(DownloadProbe {
            content_length: None,
            accepts_ranges: false,
            headers: HeaderMap::new(),
        });
    }

    let _permit = client
        .request_limiter
        .clone()
        .acquire_owned()
        .await
        .context("获取下载探测限流额度失败")?;
    let response = client
        .send_raw_with_method_once(request, Method::HEAD)
        .await?;
    let headers = response.headers();
    let content_length = parse_content_length(headers.get(CONTENT_LENGTH));
    let accepts_ranges = headers
        .get(ACCEPT_RANGES)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("bytes"));

    Ok(DownloadProbe {
        content_length,
        accepts_ranges,
        headers: headers.clone(),
    })
}
/// 使用单请求流式下载文件。
async fn download_single_request_to_file_with_retry(
    client: &LlphaClient,
    request: &FetchRequest,
    path: &Path,
    callback: Option<DownloadProgressCallback>,
) -> Result<SavedDownload> {
    let mut last_error = None;

    for attempt in 0..=client.retry_policy.max_retries {
        match download_single_request_to_file(client, request, path, callback.clone()).await {
            Ok(saved) if saved.is_success() => return Ok(saved),
            Ok(saved) => {
                last_error = Some(anyhow!("文件下载失败: {} {}", saved.status, saved.url));
                if !client.retry_policy.should_retry_status(saved.status)
                    || attempt == client.retry_policy.max_retries
                {
                    break;
                }
            }
            Err(err) => {
                last_error = Some(err);
                if attempt == client.retry_policy.max_retries {
                    break;
                }
            }
        }

        sleep(client.retry_policy.delay_for_attempt(attempt)).await;
    }

    Err(last_error.unwrap_or_else(|| anyhow!("下载失败且没有返回错误详情")))
}

/// 使用单请求流式下载文件且不做重试。
async fn download_single_request_to_file(
    client: &LlphaClient,
    request: &FetchRequest,
    path: &Path,
    callback: Option<DownloadProgressCallback>,
) -> Result<SavedDownload> {
    let _permit = client
        .request_limiter
        .clone()
        .acquire_owned()
        .await
        .context("获取下载限流额度失败")?;
    let mut response = client.send_raw_once(request).await?;
    let url = response.url().to_string();
    let status = response.status();
    let headers = response.headers().clone();
    let total_bytes = response.content_length();

    if !status.is_success() {
        return Ok(SavedDownload {
            url,
            status,
            headers,
            path: path.to_path_buf(),
            bytes_written: 0,
        });
    }

    emit_download_progress(
        &callback,
        DownloadProgress::new(
            url.clone(),
            path.to_path_buf(),
            0,
            total_bytes,
            DownloadProgressEvent::Started,
        ),
    )
    .await;

    let mut file = File::create(path)
        .await
        .with_context(|| format!("创建下载文件失败: {}", path.display()))?;
    let mut downloaded_bytes = 0_u64;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)
            .await
            .with_context(|| format!("写入下载文件失败: {}", path.display()))?;
        downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);
        emit_download_progress(
            &callback,
            DownloadProgress::new(
                url.clone(),
                path.to_path_buf(),
                downloaded_bytes,
                total_bytes,
                DownloadProgressEvent::Advanced,
            ),
        )
        .await;
    }

    file.flush()
        .await
        .with_context(|| format!("刷新下载文件失败: {}", path.display()))?;
    emit_download_progress(
        &callback,
        DownloadProgress::new(
            url.clone(),
            path.to_path_buf(),
            downloaded_bytes,
            total_bytes,
            DownloadProgressEvent::Finished,
        ),
    )
    .await;

    Ok(SavedDownload {
        url,
        status,
        headers,
        path: path.to_path_buf(),
        bytes_written: usize::try_from(downloaded_bytes).context("下载文件过大，无法记录字节数")?,
    })
}
/// 确保下载目标的父目录存在。
async fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("创建下载目录失败: {}", parent.display()))?;
    }

    Ok(())
}

/// 从响应头中解析 Content-Length。
fn parse_content_length(value: Option<&HeaderValue>) -> Option<u64> {
    value?.to_str().ok()?.parse().ok()
}
