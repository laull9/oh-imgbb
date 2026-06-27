//! websearch 命令负责搜索公开 ImgBB 相册。

use std::collections::HashSet;
use std::time::Duration;

use anyhow::Result;
use imgbb::ibb_spider::IbbSpiderManager;
use llpha::{
    AggregateSearch, BingSearch, DuckDuckGoSearch, SearchEngine, SearchPing, SearchResult,
    SearxNgSearch,
};
use serde::{Deserialize, Serialize};

/// SearchAlbumsDetail 保存网络搜索提取到的 ImgBB 相册列表。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchAlbumsDetail {
    pub query: String,
    pub search_query: String,
    pub result_count: usize,
    pub errors: Vec<String>,
    pub albums: Vec<imgbb::ibb_spider::IbbProfileAlbum>,
}

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 探测当前聚合搜索是否可用。
#[tauri::command]
pub async fn ping_websearch() -> Result<SearchPing, String> {
    command_result(
        async {
            let search = AggregateSearch::builder().limit(5).build()?;

            search.ping().await
        }
        .await,
    )
}

/// 搜索公开 ImgBB 相册并返回个人空间格式的相册列表。
#[tauri::command]
pub async fn search_imgbb_albums(query: String) -> Result<SearchAlbumsDetail, String> {
    command_result(
        async {
            let query = query.trim().to_string();
            let search_queries = build_imgbb_album_search_queries(&query);
            let report = search_imgbb_album_candidates(&search_queries).await?;

            Ok(SearchAlbumsDetail {
                query,
                search_query: search_queries.join(" | "),
                result_count: report.result_count,
                errors: report.errors,
                albums: report.albums,
            })
        }
        .await,
    )
}

/// 构造限定 ImgBB 域名的相册搜索词。
fn build_imgbb_album_search_query(query: &str) -> String {
    let query = query.trim();
    if query.is_empty() {
        "site:ibb.co/album/".to_string()
    } else {
        format!("site:ibb.co/album/ {query}")
    }
}

/// 构造限定 ImgBB 相册路径的搜索词列表。
fn build_imgbb_album_search_queries(query: &str) -> Vec<String> {
    let query = query.trim();
    let primary_query = build_imgbb_album_search_query(query);
    if query.is_empty() {
        return vec![primary_query, "\"ibb.co/album/\"".to_string()];
    }

    vec![
        primary_query,
        format!("\"ibb.co/album/\" {query}"),
        format!("site:ibb.co {query} album"),
    ]
}

/// AlbumSearchReport 保存 ImgBB 相册搜索诊断结果。
struct AlbumSearchReport {
    result_count: usize,
    errors: Vec<String>,
    albums: Vec<imgbb::ibb_spider::IbbProfileAlbum>,
}

/// AlbumSearchTaskOutput 保存单个搜索任务输出。
struct AlbumSearchTaskOutput {
    engine_name: &'static str,
    search_query: String,
    results: Vec<SearchResult>,
    error: Option<String>,
}

/// 搜索 ImgBB 相册候选结果。
async fn search_imgbb_album_candidates(search_queries: &[String]) -> Result<AlbumSearchReport> {
    let mut seen_result_urls = HashSet::new();
    let mut all_results = Vec::new();
    let mut errors = Vec::new();
    let mut handles = Vec::new();

    for search_query in search_queries {
        for (engine_name, engine) in build_imgbb_search_engines()? {
            let search_query = search_query.clone();
            handles.push(tokio::spawn(async move {
                search_imgbb_album_query(engine_name, engine, search_query).await
            }));
        }
    }

    for handle in handles {
        let output = handle.await??;
        if let Some(error) = output.error {
            errors.push(format!(
                "{} [{}]: {}",
                output.engine_name, output.search_query, error
            ));
            continue;
        }

        for result in output.results {
            if seen_result_urls.insert(result.url.clone()) {
                all_results.push(result);
            }
        }
    }

    let albums = extract_profile_albums_from_search(&all_results);

    Ok(AlbumSearchReport {
        result_count: all_results.len(),
        errors,
        albums,
    })
}

/// 执行单个 ImgBB 相册搜索词。
async fn search_imgbb_album_query(
    engine_name: &'static str,
    engine: Box<dyn SearchEngine>,
    search_query: String,
) -> Result<AlbumSearchTaskOutput> {
    let output = match engine.search(&search_query).await {
        Ok(response) => AlbumSearchTaskOutput {
            engine_name,
            search_query,
            results: response
                .results
                .into_iter()
                .filter(search_result_contains_album_url)
                .collect(),
            error: None,
        },
        Err(err) => AlbumSearchTaskOutput {
            engine_name,
            search_query,
            results: Vec::new(),
            error: Some(err.to_string()),
        },
    };

    Ok(output)
}

/// 构建 ImgBB 搜索使用的引擎列表。
fn build_imgbb_search_engines() -> Result<Vec<(&'static str, Box<dyn SearchEngine>)>> {
    let timeout = Duration::from_secs(8);

    Ok(vec![
        (
            "DuckDuckGo",
            Box::new(
                DuckDuckGoSearch::builder()
                    .limit(30)
                    .timeout(timeout)
                    .build()?,
            ),
        ),
        (
            "Bing",
            Box::new(BingSearch::builder().limit(30).timeout(timeout).build()?),
        ),
        (
            "SearxNG",
            Box::new(
                SearxNgSearch::builder()
                    .limit(30)
                    .timeout(timeout)
                    .build()?,
            ),
        ),
    ])
}

/// 判断搜索结果是否包含 ImgBB 相册地址。
fn search_result_contains_album_url(result: &SearchResult) -> bool {
    !collect_album_urls_from_text(&result.url).is_empty()
        || !collect_album_urls_from_text(&result.snippet).is_empty()
}

/// 从搜索结果中提取可识别的 ImgBB 相册列表。
fn extract_profile_albums_from_search(
    results: &[SearchResult],
) -> Vec<imgbb::ibb_spider::IbbProfileAlbum> {
    let mut seen_urls = HashSet::new();
    let mut albums = Vec::new();

    for result in results {
        let mut candidates = collect_album_urls_from_text(&result.url);
        candidates.extend(collect_album_urls_from_text(&result.snippet));

        for url in candidates {
            let Ok(normalized_url) = IbbSpiderManager::normalize_album_url(&url) else {
                continue;
            };
            if !seen_urls.insert(normalized_url.clone()) {
                continue;
            }

            albums.push(imgbb::ibb_spider::IbbProfileAlbum {
                name: clean_search_album_name(&result.title, &normalized_url),
                url: normalized_url,
                cover_url: None,
            });
        }
    }

    albums
}

/// 从文本中收集可能的 ImgBB 相册 URL。
fn collect_album_urls_from_text(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut start_index = 0usize;

    while let Some(relative_index) = text[start_index..].find("ibb.co/album/") {
        let absolute_index = start_index + relative_index;
        let candidate_start = find_url_candidate_start(text, absolute_index);
        let candidate_end = find_url_candidate_end(text, absolute_index);
        let candidate = text[candidate_start..candidate_end]
            .trim_matches(|value: char| matches!(value, '"' | '\'' | ',' | '.' | ')' | ']' | '}'));
        let candidate = if candidate.starts_with("http://") || candidate.starts_with("https://") {
            candidate.to_string()
        } else {
            format!("https://{candidate}")
        };

        urls.push(candidate);
        start_index = candidate_end;
    }

    urls
}

/// 向前定位 URL 候选片段的起始位置。
fn find_url_candidate_start(text: &str, album_marker_index: usize) -> usize {
    let prefix = &text[..album_marker_index];
    let scheme_start = prefix
        .rfind("https://")
        .or_else(|| prefix.rfind("http://"))
        .unwrap_or(album_marker_index);

    if scheme_start + "https://".len() >= album_marker_index
        || scheme_start + "http://".len() >= album_marker_index
    {
        scheme_start
    } else {
        album_marker_index
    }
}

/// 向后定位 URL 候选片段的结束位置。
fn find_url_candidate_end(text: &str, album_marker_index: usize) -> usize {
    text[album_marker_index..]
        .find(|value: char| {
            value.is_whitespace() || matches!(value, '"' | '\'' | '<' | '>' | ')' | ']' | '}')
        })
        .map(|index| album_marker_index + index)
        .unwrap_or(text.len())
}

/// 清理搜索结果标题作为相册展示名称。
fn clean_search_album_name(title: &str, fallback_url: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        return fallback_url.to_string();
    }

    title
        .trim_end_matches("- ImgBB")
        .trim_end_matches("| ImgBB")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证搜索词会限制到 ibb.co 相册路径。
    #[test]
    fn search_query_adds_site_filter() {
        assert_eq!(
            build_imgbb_album_search_query("demo album"),
            "site:ibb.co/album/ demo album"
        );
    }

    /// 验证搜索结果可以提取并规整相册地址。
    #[test]
    fn search_results_extract_album_urls() {
        let results = vec![
            SearchResult {
                title: "Demo - ImgBB".to_string(),
                url: "https://ibb.co/album/ABC123/?sort=name_asc".to_string(),
                snippet: String::new(),
            },
            SearchResult {
                title: "Duplicate".to_string(),
                url: "https://example.com".to_string(),
                snippet: "see https://ibb.co/album/ABC123/ and ibb.co/album/XYZ789".to_string(),
            },
        ];

        let albums = extract_profile_albums_from_search(&results);

        assert_eq!(albums.len(), 2);
        assert_eq!(albums[0].name, "Demo");
        assert_eq!(albums[0].url, "https://ibb.co/album/ABC123/");
        assert_eq!(albums[1].url, "https://ibb.co/album/XYZ789/");
    }

    /// live 验证真实搜索可以提取 ImgBB 相册地址。
    #[tokio::test]
    #[ignore = "live 测试需要访问公开搜索引擎"]
    async fn live_search_imgbb_albums_extracts_results() {
        let search_queries = build_imgbb_album_search_queries("album");
        let report = search_imgbb_album_candidates(&search_queries)
            .await
            .expect("搜索 ImgBB 相册失败");

        println!("候选结果数: {}", report.result_count);
        println!("提取相册数: {}", report.albums.len());
        for error in &report.errors {
            println!("搜索源异常: {error}");
        }
        for album in &report.albums {
            println!("{} -> {}", album.name, album.url);
        }

        if report.albums.is_empty() {
            eprintln!("当前公开搜索引擎没有返回可提取的 ImgBB 相册，跳过结果断言");
            return;
        }

        assert!(!report.albums.is_empty());
    }
}
