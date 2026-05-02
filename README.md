


``` rust
rust-pipeline (Rust workspace)
===================================
```

## Overview

- crates/core: schema data types 
- crates/scrub: fetch HTML from many URLs, parse generic key/value specs into write JSON
- crates/convex: Backend Query and Mutation 
- crates/transcribe: download audio from YouTube (yt-dlp) and transcribe into text
- crates/s3: upload and store files to S3 bucket(images, JSON, HTML snapshots)
- crates/optimizer: rust tools to compress/optimize images & files for temporary cache before upload
- crates/cleaner: scheduled S3 cleanup job to delete old temporary objects
- crates/pipeline: single binary exposing subcommands to call the above components

## ENV

### Configure secrets via environment variables in CI or local env:

  - Nextjs API Endpoint Routes
  - Convex Deployment and Nextjs Site Url
  - AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION, S3_BUCKET for S3 uploader/cleaner
  - For youtubettranscription OPENAI_API_KEY for transcription

###  Rust Markdown Parser 

  -  Initial plan is to use Rust and convert dataset into markdown but markdown can be quite big in size.
  -  Additional Rust helpers or CLI is needed to share the markdown files remotely to git.github.com:haikelareff/kerust.git
  -  Maybe just use Rust tools run in a dedicated CI (or server) and push JSON to S3 cloud storage.
  -  Next.js api routes for Convex backend task.


### Running locally

1.Build each tools
   
  ``` sh
 
  cargo build -p rust-pipeline

  ```
2.Run the CLI for help
  
  ``` sh
  ./target/debug/rust-pipeline --help

  ```

### Example to scrape data with multiple input url and convert to JSON and Markdown if needed
  

  ``` sh
 
  pipe scrub --input url --output dist

  pipe push-out --dir out/index.ts
  
  pipe push-bucket --status 

  pipe push-transcribe --url "https://www.youtube.com/watch?v=...." --out out/transcripts

  ```

### Example how to optimize file sizes and S3 storage 

 ``` sh
  pipeline-cli optimize --file screenshots/foo.png --out tmp/foo.optim.png
  pipeline-cli s3-upload --file tmp/foo.optim.png --key phones/images/foo.png
  phone-pipeline-cli clean-s3 --prefix tmp/ --older-than-days 30
 ```

### Further customization
- Add site-specific scrapers under crates/scraper/src/sites for more robust parsing.
- Implement Convex ingestion server function in Next.js to receive JSON and upsert to Convex (this repo sends a POST with secret header).
- Replace OpenAI transcription calls with chosen provider or on-prem model if required.
- Add sample Next.js api endpoint code 
- Add GitHub Actions workflows
