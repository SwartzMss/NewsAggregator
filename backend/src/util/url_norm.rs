use anyhow::{Context, Result};
use url::Url;

const TRACKING_PREFIXES: &[&str] = &["utm_", "spm", "_hs", "mc_", "icn", "icp"];
const TRACKING_PARAMS: &[&str] = &[
    "fbclid", "gclid", "yclid", "cmp", "ref", "referrer", "source",
];

/// Normalize article URLs so that cosmetic differences (tracking参数、结尾斜杠等)
/// 不会导致重复写入。
pub fn normalize_article_url(raw: &str) -> Result<String> {
    let mut url = Url::parse(raw).with_context(|| format!("invalid url: {raw}"))?;

    url.set_fragment(None);

    if let Some(port) = url.port() {
        let remove =
            (url.scheme() == "http" && port == 80) || (url.scheme() == "https" && port == 443);
        if remove {
            url.set_port(None).ok();
        }
    }

    {
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !is_tracking_param(k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        if pairs.is_empty() {
            url.set_query(None);
        } else {
            let mut encoded = url.query_pairs_mut();
            encoded.clear();
            let mut sorted_pairs = pairs;
            sorted_pairs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            for (k, v) in sorted_pairs {
                encoded.append_pair(&k, &v);
            }
        }
    }

    if let Some(path) = trimmed_path(&url) {
        url.set_path(&path);
    }

    Ok(url.to_string())
}

fn trimmed_path(url: &Url) -> Option<String> {
    let path = url.path();
    if path == "/" {
        return None;
    }

    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        Some("/".to_string())
    } else if trimmed == path {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_tracking_param(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    TRACKING_PARAMS.contains(&lower.as_str())
        || TRACKING_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
}
