// ... keep existing imports
use std::path::PathBuf;
use phone_spec_scraper;
use s3_uploader;
use file_optimizer;

#[derive(Subcommand)]
enum Commands {
    // existing...
    /// Scrape using input (file, single URL, or env endpoint)
    Scrape {
        #[arg(long)]
        input: String,
        #[arg(long, default_value = "out/json")]
        out: String,
        #[arg(long)]
        from_endpoint: bool, // if true uses SCRAPER_INPUT_ENDPOINT env to get JSON array
    },

    /// Run image pipeline: generate variants and upload manifest to S3
    ImagePipeline {
        #[arg(long)]
        input: String,
        #[arg(long)]
        out_dir: String,
        #[arg(long)]
        bucket: String,
        #[arg(long, default_value = "phones/images")]
        prefix: String,
    },

    // existing...
}

match cli.cmd {
    Commands::Scrape { input, out, from_endpoint } => {
        let mut urls = if from_endpoint {
            // expect SCRAPER_INPUT_ENDPOINT env var
            let endpoint = std::env::var("SCRAPER_INPUT_ENDPOINT").expect("SCRAPER_INPUT_ENDPOINT env required");
            let client = reqwest::Client::new();
            let resp = client.get(&endpoint).send().await?.error_for_status()?;
            resp.json::<Vec<String>>().await?
        } else {
            read_urls(&input)?
        };
        let client = reqwest::Client::new();
        let out_path = PathBuf::from(out);
        tokio::fs::create_dir_all(&out_path).await?;
        for url in urls {
            if let Err(e) = phone_spec_scraper::fetch_and_save_json(&client, &url, &out_path).await {
                eprintln!("scrape failed for {}: {:?}", url, e);
            } else {
                println!("scraped {}", url);
            }
        }
    }

    Commands::ImagePipeline { input, out_dir, bucket, prefix } => {
        let sizes = vec![320, 640, 1024, 1600];
        let outp = PathBuf::from(out_dir);
        tokio::fs::create_dir_all(&outp).await?;
        let generated = file_optimizer::generate_image_variants(&PathBuf::from(&input), &outp, &sizes, 85, 0.06, 0.08).await?;
        // manifest
        let manifest = s3_uploader::UploadManifest { items: generated.iter().map(|p| p.to_string_lossy().to_string()).collect() };
        let uploaded = s3_uploader::upload_manifest_to_s3(&bucket, &prefix, &manifest).await?;
        println!("Uploaded files:");
        for url in uploaded.files {
            println!("{}", url);
        }
    }

    // ... existing branches
}