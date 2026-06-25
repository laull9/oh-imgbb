use std::sync::Arc;

use anyhow::{Context, Result, anyhow, ensure};
use llpha::{FetchRequest, HtmlQuery, LlphaClient, browser_page_headers, insert_header};
use reqwest::{
    Url,
    header::{CONTENT_TYPE, COOKIE, HeaderMap, LOCATION, ORIGIN, SET_COOKIE},
};

use super::utils::extract_auth_token;

const IMGBB_LOGIN_URL: &str = "https://imgbb.com/login";
const IMGBB_ORIGIN: &str = "https://imgbb.com";
const FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";
const IBB_AUTH_PREFIX: &str = "https://ibb.co/auth?login=";

/// IbbCookie 保存一次 ImgBB 登录响应中的 Cookie。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IbbCookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<String>,
    pub max_age: Option<String>,
    pub same_site: Option<String>,
    pub secure: bool,
    pub http_only: bool,
}

/// IbbLoginSession 保存 ImgBB 登录后的内存会话。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IbbLoginSession {
    pub login_subject: String,
    pub redirect_url: String,
    pub profile: IbbAuthenticatedProfile,
    pub cookies: Vec<IbbCookie>,
    pub cookie_header: String,
}

/// IbbAuthenticatedProfile 保存已验证登录态下的用户主页接口信息。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IbbAuthenticatedProfile {
    pub url: String,
    pub json_url: String,
    pub auth_token: String,
    pub owner_id: String,
}

/// LoginPagePayload 保存登录页解析出的提交上下文。
struct LoginPagePayload {
    auth_token: String,
    cookies: Vec<IbbCookie>,
}

/// 使用账号密码登录 ImgBB 并解析会话 Cookie。
pub(super) async fn login(
    client: Arc<LlphaClient>,
    login_subject: &str,
    password: &str,
) -> Result<IbbLoginSession> {
    let login_subject = login_subject.trim();
    ensure!(!login_subject.is_empty(), "ImgBB 登录账号不能为空");
    ensure!(!password.is_empty(), "ImgBB 登录密码不能为空");

    let login_page = fetch_login_page(&client).await?;
    let login_cookie_header = build_cookie_header(&login_page.cookies);
    let body = build_login_body(&login_page.auth_token, login_subject, password)?;
    let headers = build_login_headers(&login_cookie_header)?;
    let response = client
        .fetch(
            FetchRequest::post(IMGBB_LOGIN_URL, body)
                .with_headers(headers)
                .without_redirects(),
        )
        .await?;

    let mut cookies = login_page.cookies;
    for cookie in extract_set_cookies(&response.headers) {
        upsert_cookie(&mut cookies, cookie);
    }
    let redirect_url = extract_auth_redirect_url(&response)?;
    ensure!(
        cookies.iter().any(|cookie| cookie.name == "LID"),
        "ImgBB 登录响应缺少 LID Cookie，状态码: {}",
        response.status
    );
    if let Some(redirect_url) = &redirect_url {
        complete_auth_redirect(&client, redirect_url, &mut cookies).await?;
    }
    let cookie_header = build_cookie_header(&cookies);
    let profile = verify_authenticated_profile(&client, login_subject, &cookie_header).await?;

    Ok(IbbLoginSession {
        login_subject: login_subject.to_string(),
        redirect_url: redirect_url.unwrap_or_else(|| response.url.clone()),
        profile,
        cookies,
        cookie_header,
    })
}

/// 从登录响应中提取授权跳转地址，兼容直接返回 200 OK 的情况。
fn extract_auth_redirect_url(response: &llpha::FetchResponse) -> Result<Option<String>> {
    ensure!(
        response.status.is_redirection() || response.status.is_success(),
        "ImgBB 登录请求失败，状态码: {}",
        response.status
    );

    let redirect_url = response
        .headers
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if let Some(redirect_url) = &redirect_url {
        ensure!(
            redirect_url.starts_with(IBB_AUTH_PREFIX),
            "ImgBB 登录跳转地址异常: {redirect_url}"
        );
    }

    Ok(redirect_url)
}

/// 拉取登录页并解析动态 token。
async fn fetch_login_page(client: &LlphaClient) -> Result<LoginPagePayload> {
    let response = client
        .fetch(
            FetchRequest::get(IMGBB_LOGIN_URL).with_headers(browser_page_headers(IMGBB_LOGIN_URL)?),
        )
        .await?;
    ensure!(
        response.is_success(),
        "ImgBB 登录页请求失败: {} {}",
        response.status,
        response.url
    );

    Ok(LoginPagePayload {
        auth_token: extract_login_auth_token(&response.body)?,
        cookies: extract_set_cookies(&response.headers),
    })
}

/// 构造登录表单请求头。
fn build_login_headers(cookie_header: &str) -> Result<HeaderMap> {
    let mut headers = browser_page_headers(IMGBB_LOGIN_URL)?;
    insert_header(&mut headers, CONTENT_TYPE, FORM_CONTENT_TYPE)?;
    insert_header(&mut headers, ORIGIN, IMGBB_ORIGIN)?;
    if !cookie_header.is_empty() {
        insert_header(&mut headers, COOKIE, cookie_header)?;
    }

    Ok(headers)
}

/// 访问登录跳转地址以完成 ImgBB 与 ibb.co 的授权跳转。
async fn complete_auth_redirect(
    client: &LlphaClient,
    redirect_url: &str,
    cookies: &mut Vec<IbbCookie>,
) -> Result<()> {
    let cookie_header = build_cookie_header(cookies);
    let mut headers = browser_page_headers(IMGBB_LOGIN_URL)?;
    if !cookie_header.is_empty() {
        insert_header(&mut headers, COOKIE, &cookie_header)?;
    }
    let response = client
        .fetch(
            FetchRequest::get(redirect_url.to_string())
                .with_headers(headers)
                .without_redirects(),
        )
        .await?;
    ensure!(
        response.status.is_redirection() || response.is_success(),
        "ImgBB 授权跳转请求失败: {} {}",
        response.status,
        response.url
    );

    for cookie in extract_set_cookies(&response.headers) {
        upsert_cookie(cookies, cookie);
    }

    Ok(())
}

/// 验证 Cookie 可访问用户子域并提取后续接口 token。
async fn verify_authenticated_profile(
    client: &LlphaClient,
    login_subject: &str,
    cookie_header: &str,
) -> Result<IbbAuthenticatedProfile> {
    let profile_url = build_profile_url(login_subject)?;
    let mut headers = browser_page_headers(&profile_url)?;
    insert_header(&mut headers, COOKIE, cookie_header)?;
    let response = client
        .fetch(FetchRequest::get(profile_url.clone()).with_headers(headers))
        .await?;
    ensure!(
        response.is_success(),
        "ImgBB 用户主页登录校验失败: {} {}",
        response.status,
        response.url
    );
    ensure!(
        !has_sign_in_entry(&response.body),
        "ImgBB 登录 Cookie 未在用户主页生效"
    );

    let auth_token = extract_auth_token(&response.body)?;
    let owner_id = extract_owner_id(&response.body)?;
    let json_url = extract_json_api_url(&response.body).unwrap_or_else(|| {
        let origin = profile_url.trim_end_matches('/');
        format!("{origin}/json")
    });

    Ok(IbbAuthenticatedProfile {
        url: profile_url,
        json_url,
        auth_token,
        owner_id,
    })
}

/// 根据用户名构造用户主页 URL。
fn build_profile_url(login_subject: &str) -> Result<String> {
    let username = login_subject.trim().to_ascii_lowercase();
    ensure!(
        !username.contains('@'),
        "ImgBB 登录校验需要使用用户名，暂不支持邮箱"
    );
    ensure!(
        username
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-'),
        "ImgBB 用户名格式不支持: {login_subject}"
    );

    Ok(format!("https://{username}.imgbb.com/"))
}

/// 判断页面是否仍显示未登录入口。
fn has_sign_in_entry(html: &str) -> bool {
    html.contains(r#"id="top-bar-signin""#) || html.contains("https://imgbb.com/login")
}

/// 从页面脚本中提取用户子域 JSON API 地址。
fn extract_json_api_url(html: &str) -> Option<String> {
    let marker = "PF.obj.config.json_api=\"";
    let start = html.find(marker)? + marker.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    let json_url = rest[..end].trim();
    if json_url.is_empty() {
        return None;
    }

    Some(json_url.to_string())
}

/// 从页面脚本中提取当前用户 owner ID。
fn extract_owner_id(html: &str) -> Result<String> {
    let marker = r#""id":""#;
    let start = html
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| anyhow!("未找到 ImgBB 用户 owner ID"))?;
    let rest = &html[start..];
    let end = rest
        .find('"')
        .ok_or_else(|| anyhow!("ImgBB 用户 owner ID 格式异常"))?;
    let owner_id = rest[..end].trim();
    ensure!(!owner_id.is_empty(), "ImgBB 用户 owner ID 为空");

    Ok(owner_id.to_string())
}

/// 构造登录表单正文。
fn build_login_body(auth_token: &str, login_subject: &str, password: &str) -> Result<String> {
    let url = Url::parse_with_params(
        IMGBB_LOGIN_URL,
        [
            ("auth_token", auth_token),
            ("login-subject", login_subject),
            ("password", password),
        ],
    )?;

    url.query()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("构造 ImgBB 登录表单失败"))
}

/// 从登录页 HTML 中读取 auth_token。
fn extract_login_auth_token(html: &str) -> Result<String> {
    if let Ok(auth_token) = extract_auth_token(html) {
        return Ok(auth_token);
    }

    let document = HtmlQuery::document(html);
    let auth_token = document
        .first_attr(r#"input[name="auth_token"]"#, "value")
        .context("解析 ImgBB 登录页 auth_token 失败")?
        .ok_or_else(|| anyhow!("未找到 ImgBB 登录页 auth_token"))?;
    ensure!(!auth_token.is_empty(), "ImgBB 登录页 auth_token 为空");

    Ok(auth_token)
}

/// 从响应头中提取所有 Set-Cookie。
fn extract_set_cookies(headers: &HeaderMap) -> Vec<IbbCookie> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(parse_set_cookie)
        .collect()
}

/// 解析单条 Set-Cookie 响应头。
fn parse_set_cookie(value: &str) -> Option<IbbCookie> {
    let mut parts = value.split(';').map(str::trim);
    let (name, cookie_value) = parts.next()?.split_once('=')?;
    let mut cookie = IbbCookie {
        name: name.to_string(),
        value: cookie_value.to_string(),
        domain: None,
        path: None,
        expires: None,
        max_age: None,
        same_site: None,
        secure: false,
        http_only: false,
    };

    for part in parts {
        if part.eq_ignore_ascii_case("secure") {
            cookie.secure = true;
            continue;
        }
        if part.eq_ignore_ascii_case("httponly") {
            cookie.http_only = true;
            continue;
        }
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key.to_ascii_lowercase().as_str() {
            "domain" => cookie.domain = Some(value.to_string()),
            "path" => cookie.path = Some(value.to_string()),
            "expires" => cookie.expires = Some(value.to_string()),
            "max-age" => cookie.max_age = Some(value.to_string()),
            "samesite" => cookie.same_site = Some(value.to_string()),
            _ => {}
        }
    }

    Some(cookie)
}

/// 合并同名 Cookie，后到的响应值优先。
fn upsert_cookie(cookies: &mut Vec<IbbCookie>, cookie: IbbCookie) {
    if let Some(existing) = cookies.iter_mut().find(|existing| {
        existing.name == cookie.name
            && existing.domain == cookie.domain
            && existing.path == cookie.path
    }) {
        *existing = cookie;
        return;
    }

    cookies.push(cookie);
}

/// 构造后续 ImgBB 请求可用的 Cookie 请求头。
fn build_cookie_header(cookies: &[IbbCookie]) -> String {
    cookies
        .iter()
        .filter(|cookie| !cookie.name.is_empty())
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;
    use reqwest::header::HeaderValue;

    /// 验证登录表单正文会按抓包字段编码。
    #[test]
    fn login_body_contains_required_fields() {
        let body = build_login_body("token value", "demo-user", "demo pass").unwrap();

        assert!(body.contains("auth_token=token+value"));
        assert!(body.contains("login-subject=demo-user"));
        assert!(body.contains("password=demo+pass"));
    }

    /// 验证 200 OK 登录响应可以继续依赖 Cookie 做后续校验。
    #[test]
    fn ok_login_response_without_redirect_is_accepted() {
        let response = llpha::FetchResponse {
            url: IMGBB_LOGIN_URL.to_string(),
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: String::new(),
        };

        assert_eq!(extract_auth_redirect_url(&response).unwrap(), None);
    }

    /// 验证登录页可以从隐藏表单读取 auth_token。
    #[test]
    fn login_auth_token_can_be_extracted_from_hidden_input() {
        let html = r#"<form><input type="hidden" name="auth_token" value="login-token"></form>"#;

        assert_eq!(extract_login_auth_token(html).unwrap(), "login-token");
    }

    /// 验证用户名可以映射到 ImgBB 用户子域。
    #[test]
    fn profile_url_can_be_built_from_username() {
        assert_eq!(
            build_profile_url("Laull9").unwrap(),
            "https://laull9.imgbb.com/"
        );
        assert!(build_profile_url("demo@example.com").is_err());
    }

    /// 验证未登录页面标记可以被识别。
    #[test]
    fn sign_in_entry_can_be_detected() {
        assert!(has_sign_in_entry(r#"<li id="top-bar-signin">Sign in</li>"#));
        assert!(!has_sign_in_entry(r#"<li id="top-bar-user">Account</li>"#));
    }

    /// 验证页面脚本中可以提取用户 JSON API。
    #[test]
    fn json_api_url_can_be_extracted() {
        let html = r#"<script>PF.obj.config.json_api="https://demo.imgbb.com/json";</script>"#;

        assert_eq!(
            extract_json_api_url(html),
            Some("https://demo.imgbb.com/json".to_string())
        );
    }

    /// 验证用户 owner ID 可以从页面资源脚本中提取。
    #[test]
    fn owner_id_can_be_extracted() {
        let html = r#"CHV.obj.resource={"user":{"name":"Demo","id":"OWNER123"}}"#;

        assert_eq!(extract_owner_id(html).unwrap(), "OWNER123");
    }

    /// 验证抓包里的 LID Cookie 可以被解析。
    #[test]
    fn lid_cookie_can_be_parsed_from_set_cookie() {
        let cookie = parse_set_cookie(
            "LID=abc123; expires=Fri, 25 Jun 2027 14:25:34 GMT; Max-Age=31536000; path=/; domain=.imgbb.com; secure; HttpOnly; SameSite=None",
        )
        .unwrap();

        assert_eq!(cookie.name, "LID");
        assert_eq!(cookie.value, "abc123");
        assert_eq!(cookie.domain, Some(".imgbb.com".to_string()));
        assert_eq!(cookie.path, Some("/".to_string()));
        assert_eq!(cookie.max_age, Some("31536000".to_string()));
        assert_eq!(cookie.same_site, Some("None".to_string()));
        assert!(cookie.secure);
        assert!(cookie.http_only);
    }

    /// 验证多条响应 Cookie 可以组合成请求 Cookie。
    #[test]
    fn cookie_header_joins_cookie_pairs() {
        let mut headers = HeaderMap::new();
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("PHPSESSID=session-id; path=/"),
        );
        headers.append(SET_COOKIE, HeaderValue::from_static("LID=lid-id; path=/"));

        let cookies = extract_set_cookies(&headers);

        assert_eq!(
            build_cookie_header(&cookies),
            "PHPSESSID=session-id; LID=lid-id"
        );
    }
}
