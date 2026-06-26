use anyhow::{Context, Result, anyhow};
use reqwest::{
    Client, Proxy, StatusCode,
    header::{ACCEPT_ENCODING, HeaderValue, RANGE},
    redirect::Policy,
};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::fs::{self, File};
use tokio::io::{AsyncWriteExt, copy};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::sleep;

use super::super::{DEFAULT_DOWNLOAD_PART_SIZE, DEFAULT_MAX_DOWNLOAD_PARTS};
use super::DownloadProbe;
use crate::downloader::client::{DEFAULT_PARALLEL_DOWNLOAD_THRESHOLD, LlphaClient};
use crate::downloader::progress::{
    DownloadProgress, DownloadProgressCallback, DownloadProgressEvent, emit_download_progress,
};
use crate::downloader::proxy::ProxyPool;
use crate::downloader::request::{FetchRequest, HttpMethod, SavedDownload};
use crate::downloader::retry::RetryPolicy;

/// DownloadPart 保存单个分片的字节范围。
#[derive(Clone, Debug)]
struct DownloadPart {
    index: usize,
    start_byte: u64,
    end_byte: u64,
}

/// RangeDownloadJob 保存分片下载任务需要的共享状态。
struct RangeDownloadJob {
    client: Client,
    request_limiter: Arc<Semaphore>,
    retry_policy: RetryPolicy,
    proxy_pool: Option<Arc<dyn ProxyPool>>,
    request: FetchRequest,
    callback: Option<DownloadProgressCallback>,
    downloaded_bytes: Arc<AtomicU64>,
    total_bytes: u64,
    final_path: PathBuf,
    part_path: PathBuf,
    part: DownloadPart,
}

/// 使用 Range 分片并发下载文件后自动合并。
pub(super) async fn download_parallel_request_to_file(
    client: &LlphaClient,
    request: &FetchRequest,
    path: &Path,
    probe: DownloadProbe,
    callback: Option<DownloadProgressCallback>,
) -> Result<SavedDownload> {
    let total_bytes = probe
        .content_length
        .context("分片下载缺少 Content-Length")?;
    let parts = build_download_parts(total_bytes, client.max_concurrent_requests);
    let part_paths = build_part_paths(path, parts.len());
    let downloaded_bytes = Arc::new(AtomicU64::new(0));

    emit_download_progress(
        &callback,
        DownloadProgress::new(
            request.url.clone(),
            path.to_path_buf(),
            0,
            Some(total_bytes),
            DownloadProgressEvent::Started,
        ),
    )
    .await;

    let mut join_set = JoinSet::new();
    for (part, part_path) in parts.iter().cloned().zip(part_paths.iter().cloned()) {
        let job = RangeDownloadJob {
            client: client.client.clone(),
            request_limiter: client.request_limiter.clone(),
            retry_policy: client.retry_policy.clone(),
            proxy_pool: client.proxy_pool.clone(),
            request: request.clone(),
            callback: callback.clone(),
            downloaded_bytes: downloaded_bytes.clone(),
            total_bytes,
            final_path: path.to_path_buf(),
            part_path,
            part,
        };
        join_set.spawn(download_range_part_with_retry(job));
    }

    let mut download_error = None;
    while let Some(result) = join_set.join_next().await {
        if let Err(err) = result
            .context("分片下载任务执行失败")
            .and_then(|value| value)
        {
            download_error = Some(err);
            break;
        }
    }

    if let Some(err) = download_error {
        join_set.abort_all();
        cleanup_part_paths(&part_paths).await;
        return Err(err);
    }

    merge_download_parts(path, &part_paths).await?;
    cleanup_part_paths(&part_paths).await;
    emit_download_progress(
        &callback,
        DownloadProgress::new(
            request.url.clone(),
            path.to_path_buf(),
            total_bytes,
            Some(total_bytes),
            DownloadProgressEvent::Finished,
        ),
    )
    .await;

    Ok(SavedDownload {
        url: request.url.clone(),
        status: StatusCode::OK,
        headers: probe.headers,
        path: path.to_path_buf(),
        bytes_written: usize::try_from(total_bytes).context("下载文件过大，无法记录字节数")?,
    })
}

/// 判断当前请求是否适合使用分片下载。
pub(super) fn should_use_parallel_download(request: &FetchRequest, probe: &DownloadProbe) -> bool {
    request.method == HttpMethod::Get
        && probe.accepts_ranges
        && probe
            .content_length
            .is_some_and(|length| length > DEFAULT_PARALLEL_DOWNLOAD_THRESHOLD)
}

/// 构造分片下载计划。
fn build_download_parts(total_bytes: u64, max_concurrent_requests: usize) -> Vec<DownloadPart> {
    let part_count = total_bytes
        .div_ceil(DEFAULT_DOWNLOAD_PART_SIZE)
        .clamp(1, DEFAULT_MAX_DOWNLOAD_PARTS as u64)
        .min(max_concurrent_requests.max(1) as u64) as usize;
    let part_size = total_bytes.div_ceil(part_count as u64);

    (0..part_count)
        .map(|index| {
            let start_byte = index as u64 * part_size;
            let end_byte = ((index as u64 + 1) * part_size)
                .saturating_sub(1)
                .min(total_bytes.saturating_sub(1));
            DownloadPart {
                index,
                start_byte,
                end_byte,
            }
        })
        .collect()
}

/// 构造分片临时文件路径。
fn build_part_paths(path: &Path, part_count: usize) -> Vec<PathBuf> {
    let file_name = path
        .file_name()
        .map(|file_name| file_name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());
    let parent = path.parent().unwrap_or_else(|| Path::new(""));

    (0..part_count)
        .map(|index| parent.join(format!(".{file_name}.part-{index}")))
        .collect()
}

/// 使用重试策略下载单个 Range 分片。
async fn download_range_part_with_retry(job: RangeDownloadJob) -> Result<()> {
    let mut last_error = None;

    for attempt in 0..=job.retry_policy.max_retries {
        match download_range_part_once(&job).await {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = Some(err);
                if attempt == job.retry_policy.max_retries {
                    break;
                }
            }
        }

        sleep(job.retry_policy.delay_for_attempt(attempt)).await;
    }

    Err(last_error.unwrap_or_else(|| anyhow!("分片下载失败且没有返回错误详情")))
}

/// 下载单个 Range 分片且不做重试。
async fn download_range_part_once(job: &RangeDownloadJob) -> Result<()> {
    let _permit = job
        .request_limiter
        .clone()
        .acquire_owned()
        .await
        .context("获取分片下载限流额度失败")?;
    let mut headers = job.request.options.headers.clone();
    headers.insert(
        RANGE,
        HeaderValue::from_str(&format!(
            "bytes={}-{}",
            job.part.start_byte, job.part.end_byte
        ))?,
    );
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    let client = client_for_range_job(job)?;
    let mut builder = client.get(&job.request.url).headers(headers);

    if let Some(timeout) = job.request.options.timeout {
        builder = builder.timeout(timeout);
    }

    emit_range_job_progress(
        job,
        job.downloaded_bytes.load(Ordering::Relaxed),
        DownloadProgressEvent::PartStarted {
            part_index: job.part.index,
            start_byte: job.part.start_byte,
            end_byte: job.part.end_byte,
        },
    )
    .await;

    let mut response = builder.send().await?;
    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Err(anyhow!(
            "分片下载失败: {} {}",
            response.status(),
            job.request.url
        ));
    }

    let mut file = File::create(&job.part_path)
        .await
        .with_context(|| format!("创建分片文件失败: {}", job.part_path.display()))?;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)
            .await
            .with_context(|| format!("写入分片文件失败: {}", job.part_path.display()))?;
        let downloaded_bytes = job
            .downloaded_bytes
            .fetch_add(chunk.len() as u64, Ordering::Relaxed)
            + chunk.len() as u64;
        emit_range_job_progress(job, downloaded_bytes, DownloadProgressEvent::Advanced).await;
    }

    file.flush()
        .await
        .with_context(|| format!("刷新分片文件失败: {}", job.part_path.display()))?;
    emit_range_job_progress(
        job,
        job.downloaded_bytes.load(Ordering::Relaxed),
        DownloadProgressEvent::PartFinished {
            part_index: job.part.index,
            start_byte: job.part.start_byte,
            end_byte: job.part.end_byte,
        },
    )
    .await;

    Ok(())
}

/// 发送分片任务进度事件。
async fn emit_range_job_progress(
    job: &RangeDownloadJob,
    downloaded_bytes: u64,
    event: DownloadProgressEvent,
) {
    emit_download_progress(
        &job.callback,
        DownloadProgress::new(
            job.request.url.clone(),
            job.final_path.clone(),
            downloaded_bytes,
            Some(job.total_bytes),
            event,
        ),
    )
    .await;
}

/// 根据分片任务的代理池选择 reqwest 客户端。
fn client_for_range_job(job: &RangeDownloadJob) -> Result<Client> {
    let Some(proxy_pool) = &job.proxy_pool else {
        if job.request.options.follow_redirects {
            return Ok(job.client.clone());
        }

        return Client::builder()
            .redirect(Policy::none())
            .build()
            .map_err(Into::into);
    };

    let Some(proxy_url) = proxy_pool.next_proxy()? else {
        if job.request.options.follow_redirects {
            return Ok(job.client.clone());
        }

        return Client::builder()
            .redirect(Policy::none())
            .build()
            .map_err(Into::into);
    };

    let mut builder = Client::builder().proxy(Proxy::all(proxy_url)?);
    if !job.request.options.follow_redirects {
        builder = builder.redirect(Policy::none());
    }

    builder.build().map_err(Into::into)
}

/// 合并所有分片文件到最终目标路径。
async fn merge_download_parts(path: &Path, part_paths: &[PathBuf]) -> Result<()> {
    let mut output = File::create(path)
        .await
        .with_context(|| format!("创建下载文件失败: {}", path.display()))?;

    for part_path in part_paths {
        let mut input = File::open(part_path)
            .await
            .with_context(|| format!("打开分片文件失败: {}", part_path.display()))?;
        copy(&mut input, &mut output)
            .await
            .with_context(|| format!("合并分片文件失败: {}", part_path.display()))?;
    }

    output
        .flush()
        .await
        .with_context(|| format!("刷新下载文件失败: {}", path.display()))?;

    Ok(())
}

/// 清理分片临时文件。
async fn cleanup_part_paths(part_paths: &[PathBuf]) {
    for part_path in part_paths {
        let _ = fs::remove_file(part_path).await;
    }
}
