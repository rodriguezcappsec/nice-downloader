use crate::errors::AppError;
use reqwest::{
    header::{ACCEPT, LOCATION},
    redirect::Policy,
    Client, StatusCode,
};
use std::time::Duration;
use url::Url;

const MAX_REDIRECTS: usize = 5;
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

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
}
