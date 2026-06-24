use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

/// DownloadProgressFuture 表示异步进度回调返回的 future。
pub type DownloadProgressFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// DownloadProgressCallback 表示可共享的异步下载进度回调。
pub type DownloadProgressCallback =
    Arc<dyn Fn(DownloadProgress) -> DownloadProgressFuture + Send + Sync + 'static>;

/// DownloadProgressEvent 表示一次下载进度事件类型。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadProgressEvent {
    Started,
    Advanced,
    PartStarted {
        part_index: usize,
        start_byte: u64,
        end_byte: u64,
    },
    PartFinished {
        part_index: usize,
        start_byte: u64,
        end_byte: u64,
    },
    Finished,
}

/// DownloadProgress 保存一次下载进度回调的完整上下文。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadProgress {
    pub url: String,
    pub path: PathBuf,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub event: DownloadProgressEvent,
}

impl DownloadProgress {
    /// 创建新的下载进度事件。
    pub fn new(
        url: impl Into<String>,
        path: impl Into<PathBuf>,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        event: DownloadProgressEvent,
    ) -> Self {
        Self {
            url: url.into(),
            path: path.into(),
            downloaded_bytes,
            total_bytes,
            event,
        }
    }
}

/// 创建可共享的异步下载进度回调。
pub fn download_progress_callback<F, Fut>(callback: F) -> DownloadProgressCallback
where
    F: Fn(DownloadProgress) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Arc::new(move |progress| Box::pin(callback(progress)))
}

/// 执行可选的异步下载进度回调。
pub(crate) async fn emit_download_progress(
    callback: &Option<DownloadProgressCallback>,
    progress: DownloadProgress,
) {
    if let Some(callback) = callback {
        callback(progress).await;
    }
}
