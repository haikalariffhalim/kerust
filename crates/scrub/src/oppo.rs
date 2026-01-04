use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use scraper::{Html, Selector};
use specs::{Cameras, SourceInfo, Spec};

/// Simple parser for OPPO Malaysia product pages (example: find-n5/specs)
pub async fn parse(url: &str) -> Result<Spec> {
    let client = Client::new();
    let resp = client.get(url).send().await?.error_for_status()?;
    let body = resp.text().await?;
    let doc = Html::parse_document(&body);

    let title = doc
        .select(&Selector::parse("meta[property=\"og:title\"]").unwrap())
        .next()
        .and_then(|n| n.value().attr("content"))
        .map(|s| s.to_string())
        .or_else(|| {
            doc.select(&Selector::parse("title").unwrap())
                .next()
                .map(|t| t.inner_html())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let (brand, model) = extract_brand_model(&title);
    let mut specs = Spec::new(model.clone());
    specs.brand = brand;

    // OPPO often has .specs-list or table
    if let Ok(li_sel) = Selector::parse(".specs-list li, .specs_list li, .specs li") {
        for li in doc.select(&li_sel) {
            let text = li.text().collect::<Vec<_>>().join(" ").trim().to_string();
            // split key:value
            if let Some((k, v)) = text.split_once(':') {
                let key = k.trim().to_string();
                let val = v.trim().to_string();
                if key.to_lowercase().contains("display") {
                    spec.display = Some(val);
                    continue;
                }
                if key.to_lowercase().contains("battery") {
                    spec.battery = Some(val);
                    continue;
                }
                if key.to_lowercase().contains("camera") {
                    spec.cameras = Some(parse_camera(&val));
                    continue;
                }
                spec.raw.insert(key, val);
            }
        }
    } else {
        // fallback to table parsing
        if let Ok(tr_sel) = Selector::parse("table tr") {
            let th_sel = Selector::parse("th").unwrap();
            let td_sel = Selector::parse("td").unwrap();
            for tr in doc.select(&tr_sel) {
                let key = tr
                    .select(&th_sel)
                    .next()
                    .map(|t| t.text().collect::<Vec<_>>().join(" ").trim().to_string());
                let val = tr
                    .select(&td_sel)
                    .next()
                    .map(|t| t.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    .unwrap_or_default();
                if let Some(k) = key {
                    if k.to_lowercase().contains("camera") {
                        spec.cameras = Some(parse_camera(&val));
                    } else {
                        spec.raw.insert(k, val);
                    }
                }
            }
        }
    }

    // price (if present)
    if let Some(p_sel) = doc
        .select(&Selector::parse(".price, .product-price").unwrap())
        .next()
    {
        let p = p_sel
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        spec.price = Some(phone_spec_core::Price {
            raw: Some(p.clone()),
            amount: p
                .replace(|c: char| !c.is_numeric() && c != '.', "")
                .parse::<f64>()
                .ok(),
            currency: None,
            country: Some("MY".to_string()),
        });
    }

    spec.sources.push(SourceInfo {
        url: url.to_string(),
        scraped_at: Utc::now().to_rfc3339(),
        source_name: Some("oppo.com".to_string()),
    });
    Ok(spec)
}

fn extract_brand_model(title: &str) -> (Option<String>, String) {
    (
        Some("OPPO".to_string()),
        title.replace("OPPO", "").trim().to_string(),
    )
}

fn parse_camera(s: &str) -> Cameras {
    let mut cams = Cameras::default();
    let parts: Vec<_> = s
        .split(|c| c == ',' || c == '+')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    for (i, p) in parts.iter().enumerate() {
        let low = p.to_lowercase();
        if low.contains("tele") {
            cams.telephoto = Some(p.to_string());
        } else if low.contains("ultra") {
            cams.ultra_wide = Some(p.to_string());
        } else if low.contains("wide") {
            cams.wide = Some(p.to_string());
        } else {
            if i == 0 && cams.main.is_none() {
                cams.main = Some(p.to_string());
            } else {
                cams.other = Some(match &cams.other {
                    Some(prev) => format!("{}, {}", prev, p),
                    None => p.to_string(),
                });
            }
        }
    }
    cams
}
