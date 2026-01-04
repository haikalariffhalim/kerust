```text
rust-pipeline (Rust workspace)
===================================
```

## Overview

- crates/core: shared data types (Specs, Cameras, Price, SourceInfo)
- crates/scraper: fetch HTML from many URLs, parse generic key/value specs, write JSON
- crates/convex_function: POST JSON to a secure ingestion endpoint (Next.js / Convex server-side)
- crates/youtube_transcribe: download audio from YouTube (yt-dlp) and transcribe using OpenAI
- crates/s3_uploader: upload files to S3 (images, JSON, HTML snapshots)
- crates/optimizer: compress/optimize images & files for temporary cache before upload
- crates/cleaner: scheduled S3 cleanup job to delete old temporary objects
- crates/cli: single binary exposing subcommands to call the above components

### ENV
- Configure secrets via environment variables in CI or local env:
  - OPENAI_API_KEY for transcription
  - NEXTJS_INGEST_URL and NEXTJS_INGEST_SECRET for convex_pusher (or use Convex admin key in Next.js)
  - AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION, S3_BUCKET for S3 uploader/cleaner
- For yt_transcribe, install yt-dlp in the environment where you run it.
- The recommended pattern: have the Rust tools run in a dedicated CI (or server) and push JSON/artefacts to S3 and optionally call your Next.js ingestion endpoint which performs Convex upserts server-side.



### Running locally (examples)
- Build everything:
  cargo build -p rust-pipeline
- Run the CLI (help):
  ./target/debug/rust-pipeline --help

Example flows
- Scrape and write JSON:
  phone-pipeline-cli scrape --input inputs/models_urls.json --out out/json

- Push JSON to Next.js ingestion endpoint:
  phone-pipeline-cli push-to-convex --dir out/json

- Download & transcribe a YouTube URL:
  phone-pipeline-cli transcribe --url "https://www.youtube.com/watch?v=...." --out out/transcripts

- Optimize a file and upload to S3:
  phone-pipeline-cli optimize --file screenshots/foo.png --out tmp/foo.optim.png
  phone-pipeline-cli s3-upload --file tmp/foo.optim.png --key phones/images/foo.png

- Run scheduled cleaner (example):
  phone-pipeline-cli clean-s3 --prefix tmp/ --older-than-days 30

Further customization
- Add site-specific scrapers under crates/scraper/src/sites for more robust parsing.
- Implement Convex ingestion server function in Next.js to receive JSON and upsert to Convex (this repo sends a POST with secret header).
- Replace OpenAI transcription calls with your chosen provider or on-prem model if required.

If you want, I can:
- Add sample Next.js ingestion endpoint code (verifies secret or WorkOS JWT then upserts to Convex).
- Extend the scraper with site-specific parsers for a few Malaysian sources you name.
- Add GitHub Actions workflows (one to run the full pipeline nightly and one scheduled cleaner).
