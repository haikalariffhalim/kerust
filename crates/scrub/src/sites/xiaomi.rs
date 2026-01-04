use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use scraper::{Html, Selector};
use specs::{Cameras, SourceInfo, Spec};

/// Parse Xiaomi / MI Malaysia product pages (example structure: /product/.../specs)
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

    // xiaomi often uses keyed specs in <div class="specs"> or section with <dt>/<dd>
    if let Ok(dt_sel) = Selector::parse("dt") {
        let dd_sel = Selector::parse("dd").unwrap();
        let dts: Vec<_> = doc.select(&dt_sel).collect();
        let dds: Vec<_> = doc.select(&dd_sel).collect();
        for (i, dt) in dts.iter().enumerate() {
            let k = dt.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let v = dds
                .get(i)
                .map(|d| d.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .unwrap_or_default();
            if k.to_lowercase().contains("display") {
                specs.display = Some(v);
                continue;
            }
            if k.to_lowercase().contains("battery") {
                specs.battery = Some(v);
                continue;
            }
            if k.to_lowercase().contains("camera") {
                specs.cameras = Some(parse_camera(&v));
                continue;
            }
            specs.raw.insert(k, v);
        }
    }

    // price
    if let Some(price_sel) = doc
        .select(&Selector::parse(".price, .productPrice, .product__price").unwrap())
        .next()
    {
        let p = price_sel
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        specs.price = Some(phone_spec_core::Price {
            raw: Some(p.clone()),
            amount: p
                .replace(|c: char| !c.is_numeric() && c != '.', "")
                .parse::<f64>()
                .ok(),
            currency: None,
            country: Some("MY".to_string()),
        });
    }

    // images
    if let Ok(img_sel) = Selector::parse("img") {
        for img in doc.select(&img_sel) {
            if let Some(src) = img.value().attr("src") {
                if src.contains("/images/") || src.contains("mi.com") {
                    specs.raw.insert("image".to_string(), src.to_string());
                    break;
                }
            }
        }
    }

    specs.sources.push(SourceInfo {
        url: url.to_string(),
        scraped_at: Utc::now().to_rfc3339(),
        source_name: Some("mi.com".to_string()),
    });
    Ok(specs)
}

fn extract_brand_model(title: &str) -> (Option<String>, String) {
    // often "Xiaomi 15T Pro - Specifications"
    let t = title
        .replace("Specifications", "")
        .replace("specs", "")
        .trim()
        .to_string();
    (Some("Xiaomi".to_string()), t)
}

fn parse_camera(s: &str) -> Cameras {
    let mut lens = Cameras::default();
    let parts: Vec<_> = s
        .split(|c| c == ',' || c == '+')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    for (i, p) in parts.iter().enumerate() {
        let low = p.to_lowercase();
        if low.contains("tele") {
            lens.telephoto = Some(p.to_string());
        } else if low.contains("ultra") || low.contains("ultrawide") {
            lens.ultra_wide = Some(p.to_string());
        } else if low.contains("wide") && !low.contains("ultra") {
            lens.wide = Some(p.to_string());
        } else {
            if i == 0 && lens.main.is_none() {
                lens.main = Some(p.to_string());
            } else {
                lens.other = Some(match &lens.other {
                    Some(prev) => format!("{}, {}", prev, p),
                    None => p.to_string(),
                });
            }
        }
    }
    lens
}
