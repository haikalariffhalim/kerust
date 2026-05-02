use anyhow::{Context, Result};
use phone_specs_core::{specs, SourceInfo, Cameras};
use reqwest::Client;
use scraper::{Html, Selector};
use chrono::Utc;

/// Parse Samsung Malaysia product specs pages for brand/model/price/specss/images
pub async fn parse(url: &str) -> Result<specs> {
    let client = Client::new();
    let resp = client.get(url).send().await?.error_for_status()?;
    let body = resp.text().await?;
    let doc = Html::parse_document(&body);

    // Title often contains "Samsung Galaxy ... | Samsung"
    let title = doc.select(&Selector::parse("meta[property=\"og:title\"]").unwrap())
        .next()
        .and_then(|n| n.value().attr("content"))
        .map(|s| s.to_string())
        .or_else(|| doc.select(&Selector::parse("title").unwrap()).next().map(|t| t.inner_html()))
        .unwrap_or_else(|| "unknown".to_string());

    // brand & model
    let (brand, model) = extract_brand_model(&title);

    let mut specss = specs::new(model.clone());
    specss.brand = brand;

    // Price - Samsung pages sometimes have price in selector .price or meta price:amount
    if let Some(price_meta) = doc.select(&Selector::parse("meta[itemprop=\"price\"]").unwrap()).next() {
        if let Some(p) = price_meta.value().attr("content") {
            specss.price = Some(specss::Price {
                raw: Some(p.to_string()),
                amount: p.replace(|c: char| !c.is_numeric() && c != '.' , "").parse::<f64>().ok(),
                currency: None,
                country: Some("MY".to_string()),
            });
        }
    } else {
        if let Some(pn) = doc.select(&Selector::parse(".product-price__price, .price, .productPrice").unwrap()).next() {
            let text = pn.text().collect::<Vec<_>>().join(" ").trim().to_string();
            specs.price = Some(phone_specs_core::Price {
                raw: Some(text.clone()),
                amount: text.replace(|c: char| !c.is_numeric() && c != '.' , "").parse::<f64>().ok(),
                currency: None,
                country: Some("MY".to_string()),
            });
        }
    }

    // specss: Samsung uses table structures. We'll look for tables under .specss or table.specss table
    if let Ok(table_sel) = Selector::parse(".product-specss table, table.specss, table") {
        for table in doc.select(&table_sel) {
            let tr_sel = Selector::parse("tr").unwrap();
            let th_sel = Selector::parse("th").unwrap();
            let td_sel = Selector::parse("td").unwrap();
            for tr in table.select(&tr_sel) {
                let key = tr.select(&th_sel).next()
                    .map(|t| t.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    .or_else(|| tr.select(&td_sel).next().map(|t| t.text().collect::<Vec<_>>().join(" ").trim().to_string()));
                let value = {
                    let tds: Vec<_> = tr.select(&td_sel).collect();
                    if tds.len() >= 1 {
                        tds.last().unwrap().text().collect::<Vec<_>>().join(" ").trim().to_string()
                    } else { "".to_string() }
                };
                if let (Some(k), v) = (key, value) {
                    if k.to_lowercase().contains("display") { specs.display = Some(v.clone()); continue; }
                    if k.to_lowercase().contains("battery") { specs.battery = Some(v.clone()); continue; }
                    if k.to_lowercase().contains("chip") || k.to_lowercase().contains("processor") { specs.chipset = Some(v.clone()); continue; }
                    if k.to_lowercase().contains("memory") || k.to_lowercase().contains("ram") { specs.memory = Some(v.clone()); continue; }
                    // camera heuristics
                    if k.to_lowercase().contains("camera") {
                        specs.cameras = Some(parse_camera(&v));
                        continue;
                    }
                    specs.raw.insert(k, v);
                }
            }
        }
    }

    // Attempt to find product images
    if let Ok(img_sel) = Selector::parse("img") {
        for img in doc.select(&img_sel) {
            if let Some(src) = img.value().attr("src") {
                if src.contains("/productimages/") || src.contains("/assets/") || src.contains("samsungcdn") {
                    // include first image in raw map
                    specs.raw.insert("image".to_string(), src.to_string());
                    break;
                }
            }
        }
    }

    specs.sources.push(SourceInfo {
        url: url.to_string(),
        scraped_at: Utc::now().to_rfc3339(),
        source_name: Some("samsung.my".to_string()),
    });

    Ok(specs)
}

fn extract_brand_model(title: &str) -> (Option<String>, String) {
    // naive splitting on '|' or '-' or '–'
    let parts: Vec<_> = title.split(|c| c == '|' || c == '–' || c == '-').map(|s| s.trim()).collect();
    if parts.len() >= 1 {
        // try to find "Galaxy" as model
        for p in parts {
            if p.to_lowercase().contains("galaxy") {
                // assume "Samsung Galaxy S25 FE White 256GB ..."
                let brand = Some("Samsung".to_string());
                // remove Samsung tokens
                let model = p.replace("Samsung", "").trim().to_string();
                return (brand, model);
            }
        }
    }
    (Some("Samsung".to_string()), title.to_string())
}

/// very simple camera parser for samsung pages
fn parse_camera(s: &str) -> Cameras {
    let mut cams = Cameras::default();
    let parts: Vec<_> = s.split(|c| c == ',' || c == '+').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    for (i, p) in parts.iter().enumerate() {
        let low = p.to_lowercase();
        if low.contains("tele") || low.contains("periscope") {
            cams.telephoto = Some(p.to_string());
        } else if low.contains("ultra") || low.contains("ultrawide") {
            cams.ultra_wide = Some(p.to_string());
        } else if low.contains("wide") && !low.contains("ultra") {
            cams.wide = Some(p.to_string());
        } else {
            if i == 0 && cams.main.is_none() {
                cams.main = Some(p.to_string());
            } else {
                cams.other = Some(match &cams.other { Some(prev) => format!("{}, {}", prev, p), None => p.to_string() });
            }
        }
    }
    cams
}
