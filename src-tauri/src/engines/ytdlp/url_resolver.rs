use crate::errors::AppError;
use reqwest::{
    header::{ACCEPT, LOCATION},
    redirect::Policy,
    Client, StatusCode,
};
use std::time::Duration;
use url::Url;

const MAX_REDIRECTS: usize = 5;
const MAX_EMBED_PAGE_BYTES: usize = 2 * 1024 * 1024;
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedditEmbedMedia {
    pub title: String,
    pub uploader: Option<String>,
    pub thumbnail: Option<String>,
    pub webpage_url: Url,
    pub download_url: Url,
}

pub async fn resolve_for_analysis(url: Url) -> Result<Url, AppError> {
    if let Some(canonical) = canonicalize_redd_it(&url) {
        return Ok(canonical);
    }
    if !is_reddit_share_url(&url) {
        return Ok(url);
    }

    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent(BROWSER_USER_AGENT)
        .build()
        .map_err(|error| AppError::internal("could not initialize the URL resolver", error))?;

    let mut attempts = vec![url.clone()];
    if url.host_str() != Some("old.reddit.com") {
        let mut old_reddit = url.clone();
        if old_reddit.set_host(Some("old.reddit.com")).is_ok() {
            attempts.push(old_reddit);
        }
    }

    let mut last_error = String::new();
    for candidate in attempts {
        match follow_reddit_redirects(&client, candidate).await {
            Ok(canonical) => return Ok(canonical),
            Err(error) => last_error = error,
        }
    }

    Err(AppError::with_details(
        "reddit_share_resolution_failed",
        "This Reddit share link could not be resolved. Open the post in a browser and copy its full comments URL.",
        last_error,
    ))
}

pub async fn resolve_reddit_embed_media(url: &Url) -> Result<Option<RedditEmbedMedia>, String> {
    let Some(post_id) = reddit_post_id(url) else {
        return Ok(None);
    };
    let embed_url = Url::parse(&format!(
        "https://embed.reddit.com/r/reddit/comments/{post_id}?embed=true"
    ))
    .map_err(|error| format!("Could not construct the Reddit embed URL: {error}"))?;
    let client = Client::builder()
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("Reddit embed exceeded the redirect limit")
            } else if is_reddit_host(attempt.url().host_str()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .timeout(Duration::from_secs(20))
        .user_agent(BROWSER_USER_AGENT)
        .build()
        .map_err(|error| format!("Could not initialize the Reddit embed client: {error}"))?;
    let response = client
        .get(embed_url)
        .header(ACCEPT, "text/html,application/xhtml+xml")
        .send()
        .await
        .map_err(|error| format!("Could not request the Reddit embed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Reddit embed returned HTTP {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_EMBED_PAGE_BYTES as u64)
    {
        return Err("Reddit embed response exceeded the size limit".to_owned());
    }
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("Could not read the Reddit embed response: {error}"))?;
    if body.len() > MAX_EMBED_PAGE_BYTES {
        return Err("Reddit embed response exceeded the size limit".to_owned());
    }
    parse_reddit_embed(&String::from_utf8_lossy(&body), url, &post_id).map(Some)
}

async fn follow_reddit_redirects(client: &Client, mut current: Url) -> Result<Url, String> {
    for _ in 0..MAX_REDIRECTS {
        let response = client
            .get(current.clone())
            .header(ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await
            .map_err(|error| format!("Could not request Reddit share link: {error}"))?;
        let status = response.status();

        if status.is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .ok_or_else(|| format!("Reddit returned {status} without a redirect location"))?
                .to_str()
                .map_err(|_| "Reddit returned an invalid redirect location".to_owned())?;
            let next = current
                .join(location)
                .map_err(|error| format!("Reddit returned an invalid redirect URL: {error}"))?;
            validate_reddit_target(&next)?;
            if is_reddit_comments_url(&next) {
                return Ok(strip_tracking(next));
            }
            current = next;
            continue;
        }

        if status.is_success() {
            let body = response
                .bytes()
                .await
                .map_err(|error| format!("Could not read Reddit share response: {error}"))?;
            if let Some(canonical) = find_reddit_comments_url(&String::from_utf8_lossy(&body)) {
                return Ok(strip_tracking(canonical));
            }
            return Err("Reddit did not provide a canonical comments URL".to_owned());
        }

        if status == StatusCode::FORBIDDEN {
            return Err("Reddit blocked share-link resolution with HTTP 403".to_owned());
        }
        return Err(format!(
            "Reddit share-link resolution returned HTTP {status}"
        ));
    }
    Err("Reddit share link exceeded the redirect limit".to_owned())
}

fn is_reddit_share_url(url: &Url) -> bool {
    if !is_reddit_host(url.host_str()) {
        return false;
    }
    let segments: Vec<_> = url
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    segments.len() >= 4
        && matches!(segments[0], "r" | "user")
        && segments[2] == "s"
        && !segments[3].is_empty()
}

fn canonicalize_redd_it(url: &Url) -> Option<Url> {
    if url.host_str() != Some("redd.it") && url.host_str() != Some("www.redd.it") {
        return None;
    }
    let post_id = url.path_segments()?.find(|segment| !segment.is_empty())?;
    if !post_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    Url::parse(&format!("https://www.reddit.com/comments/{post_id}")).ok()
}

fn reddit_post_id(url: &Url) -> Option<String> {
    if !is_reddit_host(url.host_str()) {
        return None;
    }
    let segments: Vec<_> = url.path_segments()?.collect();
    let comments_index = segments.iter().position(|segment| *segment == "comments")?;
    let post_id = *segments.get(comments_index + 1)?;
    if post_id.is_empty()
        || post_id.len() > 16
        || !post_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(post_id.to_ascii_lowercase())
}

fn validate_reddit_target(url: &Url) -> Result<(), String> {
    if matches!(url.scheme(), "http" | "https") && is_reddit_host(url.host_str()) {
        Ok(())
    } else {
        Err("Reddit share link redirected outside the reddit.com domain".to_owned())
    }
}

fn is_reddit_host(host: Option<&str>) -> bool {
    host.is_some_and(|host| host == "reddit.com" || host.ends_with(".reddit.com"))
}

fn is_reddit_comments_url(url: &Url) -> bool {
    is_reddit_host(url.host_str())
        && url
            .path_segments()
            .is_some_and(|mut segments| segments.any(|segment| segment == "comments"))
}

fn strip_tracking(mut url: Url) -> Url {
    url.set_query(None);
    url.set_fragment(None);
    url
}

fn find_reddit_comments_url(html: &str) -> Option<Url> {
    let lowercase = html.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(relative_start) = lowercase[offset..].find("<link") {
        let start = offset + relative_start;
        let end = lowercase[start..].find('>').map(|end| start + end)?;
        let tag = &html[start..=end];
        let lowercase_tag = &lowercase[start..=end];
        if lowercase_tag.contains("canonical") {
            if let Some(href) = html_attribute(tag, "href") {
                let href = href.replace("&amp;", "&");
                if let Ok(url) = Url::parse(&href) {
                    if is_reddit_comments_url(&url) {
                        return Some(url);
                    }
                }
            }
        }
        offset = end + 1;
    }
    None
}

fn html_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lowercase = tag.to_ascii_lowercase();
    let start = lowercase.find(&format!("{name}="))? + name.len() + 1;
    let quote = tag[start..].chars().next()?;
    if !matches!(quote, '"' | '\'') {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = tag[value_start..].find(quote)? + value_start;
    Some(&tag[value_start..value_end])
}

fn parse_reddit_embed(
    html: &str,
    requested_url: &Url,
    post_id: &str,
) -> Result<RedditEmbedMedia, String> {
    let player_tag = opening_tag(html, "<shreddit-player")
        .ok_or_else(|| "Reddit embed did not contain a native video player".to_owned())?;
    let player_url = html_attribute(player_tag, "src")
        .map(decode_html_entities)
        .and_then(|value| Url::parse(&value).ok())
        .ok_or_else(|| "Reddit embed did not provide a valid video manifest".to_owned())?;
    let download_url = normalized_reddit_hls_url(&player_url)?;

    let canonical = opening_tag_containing(html, "canonical-url-updater")
        .and_then(|tag| html_attribute(tag, "value"))
        .map(decode_html_entities)
        .and_then(|value| Url::parse(&value).ok())
        .filter(|url| reddit_post_id(url).as_deref() == Some(post_id))
        .map(strip_tracking)
        .unwrap_or_else(|| strip_tracking(requested_url.clone()));

    let title = extract_embed_title(html)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| format!("Reddit video {post_id}"));
    let uploader = extract_reddit_uploader(html);
    let thumbnail = html_attribute(player_tag, "poster")
        .map(decode_html_entities)
        .and_then(|value| Url::parse(&value).ok())
        .filter(|url| url.scheme() == "https")
        .map(|url| url.to_string());

    Ok(RedditEmbedMedia {
        title,
        uploader,
        thumbnail,
        webpage_url: canonical,
        download_url,
    })
}

fn normalized_reddit_hls_url(player_url: &Url) -> Result<Url, String> {
    if player_url.scheme() != "https" || player_url.host_str() != Some("v.redd.it") {
        return Err("Reddit embed returned a video manifest outside v.redd.it".to_owned());
    }
    let media_id = player_url
        .path_segments()
        .and_then(|mut segments| segments.find(|segment| !segment.is_empty()))
        .filter(|media_id| {
            !media_id.is_empty()
                && media_id.len() <= 64
                && media_id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .ok_or_else(|| "Reddit embed returned an invalid video identifier".to_owned())?;
    Url::parse(&format!("https://v.redd.it/{media_id}/HLSPlaylist.m3u8"))
        .map_err(|error| format!("Could not construct the Reddit media URL: {error}"))
}

fn opening_tag<'a>(html: &'a str, prefix: &str) -> Option<&'a str> {
    let start = html.find(prefix)?;
    let end = html[start..].find('>')? + start;
    Some(&html[start..=end])
}

fn opening_tag_containing<'a>(html: &'a str, needle: &str) -> Option<&'a str> {
    let needle_position = html.find(needle)?;
    let start = html[..needle_position].rfind('<')?;
    let end = html[needle_position..].find('>')? + needle_position;
    Some(&html[start..=end])
}

fn extract_embed_title(html: &str) -> Option<String> {
    let anchor_position = html.find("id=\"embed-title\"")?;
    let title_region = html.get(anchor_position..)?;
    let heading_start = title_region.find("<h1")?;
    let content_start = title_region[heading_start..].find('>')? + heading_start + 1;
    let content_end = title_region[content_start..].find("</h1>")? + content_start;
    Some(decode_html_entities(&strip_html_tags(
        &title_region[content_start..content_end],
    )))
}

fn extract_reddit_uploader(html: &str) -> Option<String> {
    let marker = "https://www.reddit.com/user/";
    let start = html.find(marker)? + marker.len();
    let username = html[start..].split(['/', '?', '"', '\'']).next()?.trim();
    if username.is_empty()
        || username.len() > 32
        || !username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return None;
    }
    Some(username.to_owned())
}

fn strip_html_tags(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut inside_tag = false;
    for character in value.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => result.push(character),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_reddit_share_links_only() {
        assert!(is_reddit_share_url(
            &Url::parse("https://www.reddit.com/r/rust/s/gfAMS40rbR").unwrap()
        ));
        assert!(!is_reddit_share_url(
            &Url::parse("https://www.reddit.com/r/rust/comments/abc123/title").unwrap()
        ));
        assert!(!is_reddit_share_url(
            &Url::parse("https://example.com/r/rust/s/gfAMS40rbR").unwrap()
        ));
    }

    #[test]
    fn canonicalizes_redd_it_post_links_without_network_access() {
        assert_eq!(
            canonicalize_redd_it(&Url::parse("https://redd.it/abc123?share=1").unwrap())
                .unwrap()
                .as_str(),
            "https://www.reddit.com/comments/abc123"
        );
    }

    #[test]
    fn extracts_canonical_comments_url_from_html() {
        let html = r#"<link rel="canonical" href="https://www.reddit.com/r/rust/comments/abc123/a_post/?share_id=secret">"#;
        assert_eq!(
            strip_tracking(find_reddit_comments_url(html).unwrap()).as_str(),
            "https://www.reddit.com/r/rust/comments/abc123/a_post/"
        );
    }

    #[test]
    fn ignores_noncanonical_feed_links() {
        let html = r#"<link rel="canonical" href="https://www.reddit.com/r/videos/"><a href="https://www.reddit.com/r/videos/comments/wrong/post">post</a>"#;
        assert_eq!(find_reddit_comments_url(html), None);
    }

    #[test]
    fn rejects_redirects_outside_reddit() {
        assert!(validate_reddit_target(&Url::parse("https://evil.example/post").unwrap()).is_err());
        assert!(validate_reddit_target(
            &Url::parse("https://reddit.com.evil.example/post").unwrap()
        )
        .is_err());
    }

    #[test]
    fn extracts_safe_native_media_from_reddit_embed_html() {
        let parsed = parse_reddit_embed(
            include_str!("fixtures/reddit_embed.html"),
            &Url::parse("https://www.reddit.com/comments/abc123").unwrap(),
            "abc123",
        )
        .unwrap();
        assert_eq!(parsed.title, "A title & details");
        assert_eq!(parsed.uploader.as_deref(), Some("example-user"));
        assert_eq!(
            parsed.webpage_url.as_str(),
            "https://www.reddit.com/r/videos/comments/abc123/a_title/"
        );
        assert_eq!(
            parsed.download_url.as_str(),
            "https://v.redd.it/media987/HLSPlaylist.m3u8"
        );
        assert_eq!(
            parsed.thumbnail.as_deref(),
            Some("https://preview.redd.it/poster.jpeg?width=640&format=pjpg")
        );
    }

    #[test]
    fn rejects_embed_media_from_an_untrusted_host() {
        let html = r#"<shreddit-player src="https://evil.example/HLSPlaylist.m3u8">"#;
        assert!(parse_reddit_embed(
            html,
            &Url::parse("https://www.reddit.com/comments/abc123").unwrap(),
            "abc123"
        )
        .is_err());
    }

    #[tokio::test]
    #[ignore = "requires public Reddit network access"]
    async fn resolves_public_reddit_share_fixture() {
        let resolved = resolve_for_analysis(
            Url::parse("https://www.reddit.com/r/Volumeeating/s/rbpk7NYWv0").unwrap(),
        )
        .await
        .expect("resolve share link");
        assert!(resolved.as_str().contains("/comments/10b6ze7/"));
        assert!(resolved.query().is_none());
    }

    #[tokio::test]
    #[ignore = "requires public Reddit network access"]
    async fn resolves_public_reddit_embed_video_fixture() {
        let resolved = resolve_reddit_embed_media(
            &Url::parse("https://www.reddit.com/comments/1vkq6jk").unwrap(),
        )
        .await
        .expect("request Reddit embed")
        .expect("native Reddit video");
        assert_eq!(resolved.title, "Pekeasmr");
        assert_eq!(resolved.uploader.as_deref(), Some("Emergency-Win8230"));
        assert_eq!(resolved.download_url.host_str(), Some("v.redd.it"));
        assert!(resolved.download_url.path().ends_with("/HLSPlaylist.m3u8"));
    }
}
