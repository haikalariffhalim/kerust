// app/src/main.rs
use clap::Parser;

#[derive(Parser)]
struct Pipeline {
    url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pipeline = Cli::parse();
    // call scraper (async)
    let specs = data_scraper::fetch_url(&pipeline.url).await?;
    // call markdown renderer
    let md = markdown_parser::generate_markdown(&specs)?;
    println!("{}", md);
    Ok(())
}
