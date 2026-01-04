use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use scraper::{Html, Selector};
use specs::{SourceInfo, Spec};
use std::{collections::BTreeMap, fs, path::Path, str::FromStr};
use tokio::time::{sleep, Duration};

use sites::{oppo, samsung, xiaomi};

/// Input sources:
/// - single URL passed in function
/// - newline file of URLs
/// - or HTTP endpoint specified by env SCRUB_ENDPOINT that returns JSON array of URLs

pub async fn collect_urls_from_source(input: &str) -> Result<Vec<String>> {
    if input.starts_with("http://") || input.starts_with("https://") {
        if std::env::var("SCRUB_ENDPOINT").is_ok() {
            let endpoint = std::env::var("SCRUB_ENDPOINT").unwrap();
            let client = reqwest::Client::new();
            let resp = client.get(&endpoint).send().await?.error_for_status()?;
            let urls: Vec<String> = resp.json().await.context("parsing urls from endpoint")?;
            Ok(urls)
        } else if input.ends_with(".json") {
            // maybe a local mapping file
            let s = std::fs::read_to_string(input)?;
            let urls: Vec<String> = serde_json::from_str(&s)?;
            Ok(urls)
        } else {
            Ok(vec![input.to_string()])
        }
    } else {
        // treat as a path to newline-separated urls
        let s = std::fs::read_to_string(input)?;
        Ok(s.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect())
    }
}

/// Top-level fetch that picks a site-specific parser if domain matches; otherwise falls back to generic parser.
pub async fn fetch_and_save_json(client: &Client, url: &str, out_dir: &Path) -> Result<()> {
    // dispatch based on host
    if let Ok(u) = reqwest::Url::from_str(url) {
        if let Some(host) = u.host_str() {
            let host = host.to_lowercase();
            let specs = if host.contains("samsung galaxy") {
                samsung::parse(url).await?
            } else if host.contains("mi") || host.contains("xiaomi.com") {
                xiaomi::parse(url).await?
            } else if host.contains("oppo") {
                oppo::parse(url).await?
            } else {
                // fallback to generic
                generic_parse(url, client).await?
            };

            // write out
            fs::create_dir_all(out_dir).with_context(|| format!("create out dir {:?}", out_dir))?;
            let slug = slugify(&specs.brand, &specs.model);
            let out_path = out_dir.join(format!("{}.json", slug));
            let s = serde_json::to_string_pretty(&specs)?;
            fs::write(out_path, s)?;
            println!("saved {}", slug);
            // polite delay
            sleep(Duration::from_millis(300)).await;
        } else {
            anyhow::bail!("invalid url host: {}", url);
        }
    } else {
        anyhow::bail!("invalid url: {}", url);
    }
}

/// Minimal generic parse for unknown domains (keeps previous simple behavior)
async fn generic_parse(url: &str, client: &Client) -> Result<Spec> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let body = resp.text().await?;
    let doc = Html::parse_document(&body);

    let title = doc
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|t| t.inner_html())
        .unwrap_or_else(|| "unknown".to_string());

    let mut kv: BTreeMap<String, String> = BTreeMap::new();

    if let Ok(tr_sel) = Selector::parse("table tr") {
        for tr in doc.select(&tr_sel) {
            let th_sel = Selector::parse("th").unwrap();
            let td_sel = Selector::parse("td").unwrap();
            let mut key = None;
            if let Some(th) = tr.select(&th_sel).next() {
                key = Some(th.text().collect::<Vec<_>>().join(" ").trim().to_string());
            } else if let Some(first_td) = tr.select(&td_sel).next() {
                let t = first_td
                    .text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();
                if !t.is_empty() {
                    key = Some(t);
                }
            }
            if let Some(k) = key {
                let tds: Vec<_> = tr.select(&td_sel).collect();
                let val = if tds.len() >= 2 {
                    tds[1]
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string()
                } else {
                    "".to_string()
                };
                if !k.is_empty() && !val.is_empty() {
                    kv.insert(k, val);
                }
            }
        }
    }

    if kv.is_empty() {
        if let Ok(dt_sel) = Selector::parse("dl dt") {
            let dd_sel = Selector::parse("dl dd").unwrap();
            let dts: Vec<_> = doc.select(&dt_sel).collect();
            let dds: Vec<_> = doc.select(&dd_sel).collect();
            for (i, dt) in dts.iter().enumerate() {
                let k = dt.text().collect::<Vec<_>>().join(" ").trim().to_string();
                let v = dds
                    .get(i)
                    .map(|dd| dd.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    .unwrap_or_default();
                if !k.is_empty() && !v.is_empty() {
                    kv.insert(k, v);
                }
            }
        }
    }

    let mut specs = Spec::new(title.clone());
    if title.contains('-') {
        let mut parts: Vec<_> = title.split('-').map(|s| s.trim()).collect();
        let model = parts.pop().unwrap_or(&title).to_string();
        let brand = parts.first().map(|s| s.to_string());
        specs.model = model;
        specs.brand = brand;
    } else {
        specs.model = title.clone();
    }

    for (k, v) in kv.into_iter() {
        specs.raw.insert(k, v);
    }

    specs.sources.push(SourceInfo {
        url: url.to_string(),
        scraped_at: Utc::now().to_rfc3339(),
        source_name: None,
    });

    Ok(specs)
}

pub fn slugify(brand: &Option<String>, model: &str) -> String {
    let base = match brand {
        Some(b) if !b.is_empty() => format!("{} {}", b, model),
        _ => model.to_string(),
    };
    base.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
