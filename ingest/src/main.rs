use anyhow::{Context, Result};
use reqwest::Client;
use std::{fs, path::Path};

/// Push JSON artifacts to a server-side ingestion endpoint that upserts into Convex.
/// This avoids embedding Convex admin keys in the Rust CLI and centralizes security webapp.
///
/// The ingestion endpoint should:
/// - verify a shared secret (X-PIPELINE-SECRET)
/// - accept JSON body with a canonical record
/// - upsert into Convex using a server-side admin key
///
/// env:
/// - PIPELINE_SECRET: header value the endpoint expects
/// - INGEST_URL: the endpoint URL (https://web-app/api/ingest)

pub async fn push_dir_to_ingest(
    endpoint: &str,
    secret: &str,
    dir: &Path,
) -> Result<()> {
    let client = Client::new();
    for entry in std::fs::read_dir(dir).context("reading dir")? {
        let ent = entry?;
        let path = ent.path();
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("json")
        {
            let s = fs::read_to_string(&path)?;
            let resp = client
                .post(endpoint)
                .header("X-PIPELINE-SECRET", secret)
                .header("Content-Type", "application/json")
                .body(s)
                .send()
                .await
                .context("posting to ingest")?;
            if !resp.status().is_success() {
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("ingest failed: {} - {}", resp.status(), text);
            } else {
                println!("pushed {:?}", path.file_name());
            }
        }
    }
    Ok(())
}
