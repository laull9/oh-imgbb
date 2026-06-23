use anyhow::{Context, Result, anyhow, ensure};
use reqwest::Url;
use scraper::{Html, Selector};

use super::utils::normalize_url_input;

/// IbbProfileReport 保存 ImgBB 用户主页专辑遍历结果。
pub struct IbbProfileReport {
    pub albums: Vec<IbbProfileAlbum>,
}

/// IbbProfileAlbum 保存用户主页中的单个子专辑。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IbbProfileAlbum {
    pub name: String,
    pub url: String,
    pub cover_url: Option<String>,
}

/// IbbProfileUrl 保存 ImgBB 用户主页 URL 的规整结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct IbbProfileUrl {
    pub(super) normalized_url: String,
    pub(super) origin: String,
    pub(super) json_url: String,
    pathname: String,
    list: String,
    sort: Option<String>,
}

impl IbbProfileUrl {
    /// 解析并规整 ImgBB 用户主页相册列表 URL。
    pub(super) fn parse(input: &str) -> Result<Self> {
        let input = normalize_url_input(input);
        let mut url = Url::parse(&input).with_context(|| format!("解析 URL 失败: {input}"))?;
        let host = url.host_str().unwrap_or_default().to_string();
        ensure!(
            host.ends_with(".imgbb.com"),
            "仅支持 imgbb.com 用户主页 URL: {input}"
        );

        if url.path() == "/" {
            url.set_path("/albums");
        }
        ensure!(
            url.path() == "/albums",
            "URL 不是 ImgBB 用户相册列表: {input}"
        );

        let list = url
            .query_pairs()
            .find(|(key, _)| key == "list")
            .map(|(_, value)| value.to_string())
            .unwrap_or_else(|| "albums".to_string());
        ensure!(list == "albums", "仅支持 ImgBB 用户 albums 列表: {input}");

        let sort = url
            .query_pairs()
            .find(|(key, _)| key == "sort")
            .map(|(_, value)| value.to_string());
        url.query_pairs_mut().clear().append_pair("list", "albums");
        if let Some(sort) = &sort {
            url.query_pairs_mut().append_pair("sort", sort);
        }

        let origin = format!("{}://{}", url.scheme(), host);
        let json_url = format!("{origin}/json");

        Ok(Self {
            normalized_url: url.to_string(),
            origin,
            json_url,
            pathname: "/albums".to_string(),
            list,
            sort,
        })
    }

    /// 构造用户主页相册增量接口表单正文。
    pub(super) fn build_albums_json_body(
        &self,
        auth_token: &str,
        page: usize,
        seek: &str,
    ) -> Result<String> {
        let page = page.to_string();
        let mut params = vec![
            ("list".to_string(), self.list.clone()),
            ("action".to_string(), "list".to_string()),
            ("page".to_string(), page),
            ("seek".to_string(), seek.to_string()),
            ("auth_token".to_string(), auth_token.to_string()),
            ("pathname".to_string(), self.pathname.clone()),
        ];

        if let Some(sort) = &self.sort {
            params.insert(1, ("sort".to_string(), sort.clone()));
        }

        let url = Url::parse_with_params(&self.json_url, params)?;
        url.query()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("构造 ImgBB 用户主页 JSON 表单失败"))
    }
}

/// 从用户主页 HTML 片段提取专辑列表。
pub(super) fn parse_profile_albums(html: &str) -> Result<Vec<IbbProfileAlbum>> {
    let document = Html::parse_fragment(html);
    let item_selector = parse_selector(r#"div.list-item[data-type="album"]"#)?;
    let link_selector = parse_selector("div.list-item-image.fixed-size > a")?;
    let cover_selector = parse_selector("div.list-item-image.fixed-size > a > img")?;
    let mut albums = Vec::new();

    for item in document.select(&item_selector) {
        let Some(name) = item.value().attr("data-name").map(str::trim) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        let Some(url) = item
            .select(&link_selector)
            .next()
            .and_then(|link| link.value().attr("href"))
        else {
            continue;
        };

        let cover_url = item
            .select(&cover_selector)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(str::to_string);

        albums.push(IbbProfileAlbum {
            name: name.to_string(),
            url: url.to_string(),
            cover_url: cover_url,
        });
    }

    Ok(albums)
}

/// 从用户主页首屏 HTML 提取下一页 seek。
pub(super) fn extract_next_seek(html: &str) -> Option<String> {
    let document = Html::parse_fragment(html);
    let button_selector = parse_selector(r#"button[data-action="load-more"]"#).ok()?;
    let next_selector = parse_selector(r#"a[data-pagination="next"]"#).ok()?;

    if let Some(seek) = document
        .select(&button_selector)
        .find_map(|button| button.value().attr("data-seek"))
        .map(str::to_string)
    {
        return Some(seek);
    }

    document
        .select(&next_selector)
        .find_map(|link| link.value().attr("href"))
        .and_then(|href| Url::parse(href).ok())
        .and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "seek")
                .map(|(_, value)| value.to_string())
        })
}

/// 解析 CSS 选择器并转换错误类型。
fn parse_selector(selector: &str) -> Result<Selector> {
    Selector::parse(selector).map_err(|err| anyhow!("CSS 选择器解析失败 {selector}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证用户主页 URL 可以规整为 albums 列表。
    #[test]
    fn profile_url_normalizes_album_list() {
        let profile = IbbProfileUrl::parse("beautif11.imgbb.com/").unwrap();

        assert_eq!(
            profile.normalized_url,
            "https://beautif11.imgbb.com/albums?list=albums"
        );
        assert_eq!(profile.origin, "https://beautif11.imgbb.com");
        assert_eq!(profile.json_url, "https://beautif11.imgbb.com/json");
    }

    /// 验证用户主页 URL 会保留排序参数。
    #[test]
    fn profile_url_keeps_sort_param() {
        let profile =
            IbbProfileUrl::parse("https://beautif11.imgbb.com/albums?list=albums&sort=name_asc")
                .unwrap();
        let body = profile
            .build_albums_json_body("token", 3, "seek-value")
            .unwrap();

        assert!(body.contains("list=albums"));
        assert!(body.contains("sort=name_asc"));
        assert!(body.contains("page=3"));
        assert!(body.contains("seek=seek-value"));
    }

    /// 验证用户主页 HTML 可以解析专辑名称和链接。
    #[test]
    fn profile_albums_parse_from_html() {
        let albums = parse_profile_albums(sample_profile_html()).unwrap();

        assert_eq!(
            albums,
            vec![IbbProfileAlbum {
                name: "demo album".to_string(),
                url: "https://ibb.co/album/ABC123".to_string(),
                cover_url: Some("https://i.ibb.co/cover.jpg".to_string()),
            }]
        );
    }

    /// 验证用户主页 HTML 可以提取下一页 seek。
    #[test]
    fn next_seek_parse_from_load_more_button() {
        assert_eq!(
            extract_next_seek(sample_profile_html()),
            Some("next-seek".to_string())
        );
    }

    /// 构造测试用用户主页 HTML。
    fn sample_profile_html() -> &'static str {
        r#"
        <div id="list-most-recent">
            <div class="pad-content-listing">
                <div class="list-item c8" data-type="album" data-name="demo album">
                    <div class="list-item-image fixed-size">
                        <a href="https://ibb.co/album/ABC123">
                            <img src="https://i.ibb.co/cover.jpg">
                        </a>
                    </div>
                </div>
            </div>
            <button data-action="load-more" data-seek="next-seek">Load more</button>
        </div>
        "#
    }
}
