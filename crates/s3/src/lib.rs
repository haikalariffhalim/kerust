use anyhow::{Context, Result};
use aws_sdk_s3::{types::ByteStream, Client};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadManifest {
    pub items: Vec<String>, // local file paths to upload
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadedManifest {
    pub files: Vec<String>, // s3 urls
}

/// Upload all files in manifest to S3 under given key prefix.
/// Returns manifest of s3 urls.
pub async fn upload_to_s3(
    bucket: &str,
    prefix: &str,
    manifest: &UploadManifest,
) -> Result<UploadedManifest> {
    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);
    let mut urls = vec![];
    let region = config
        .region()
        .map(|r| r.as_ref().to_string())
        .unwrap_or_default();

    for local in &manifest.items {
        let path = Path::new(local);
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .context("file name")?;
        let key = format!("{}/{}", prefix.trim_end_matches('/'), file_name);
        let data = tokio::fs::read(path).await.context("read file")?;
        let body = ByteStream::from(data);
        client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .body(body)
            .send()
            .await
            .context("put object")?;
        let url =
            format!("https://{}.s3.{}.amazonaws.com/{}", bucket, region, key);
        urls.push(url);
    }

    Ok(UploadedManifest { files: urls })
}
