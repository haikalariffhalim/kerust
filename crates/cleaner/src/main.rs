use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use chrono::{Duration, Utc};
use clap::Parser;
use std::str::FromStr;

/// Delete objects under a prefix older than N days.
#[derive(Parser)]
struct Args {
    #[arg(long)]
    bucket: String,

    #[arg(long, default_value = "generated/tmp/")]
    prefix: String,

    #[arg(long, default_value_t = 30)]
    older_than_days: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let region_provider =
        RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    let client = Client::new(&config);

    let threshold = Utc::now() - Duration::days(args.older_than_days);

    let mut continuation_token = None::<String>;

    loop {
        let mut req = client
            .list_objects_v2()
            .bucket(&args.bucket)
            .prefix(&args.prefix);
        if let Some(token) = continuation_token {
            req = req.continuation_token(token);
        }
        let resp = req.send().await.context("list_objects")?;
        if let Some(contents) = resp.contents() {
            let mut to_delete = vec![];
            for obj in contents {
                if let Some(key) = obj.key() {
                    if let Some(lt) = obj.last_modified() {
                        if lt < &threshold {
                            to_delete.push(key.to_string());
                        }
                    }
                }
            }
            if !to_delete.is_empty() {
                println!("Deleting {} objects", to_delete.len());
                // delete in batches up to 1000
                let objects = to_delete
                    .into_iter()
                    .map(|k| {
                        aws_sdk_s3::types::ObjectIdentifier::builder()
                            .key(k)
                            .build()
                    })
                    .collect::<Vec<_>>();
                client
                    .delete_objects()
                    .bucket(&args.bucket)
                    .delete(
                        aws_sdk_s3::model::Delete::builder()
                            .set_objects(Some(objects))
                            .build(),
                    )
                    .send()
                    .await
                    .context("delete_objects")?;
            }
        }

        if resp.next_continuation_token().is_none() {
            break;
        } else {
            continuation_token =
                resp.next_continuation_token().map(|s| s.to_string());
        }
    }

    println!("Cleanup complete");
    Ok(())
}
