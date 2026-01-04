use anyhow::{Context, Result};
use clap::Parser;
use reqwest::Client;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::{sleep, Duration};

/// Transcription with provider selection (AssemblyAI default). Supports chunking via ffmpeg.
/// Requires:
/// - TRANSCRIBE_PROVIDER (assemblyai | openai)
/// - ASSEMBLYAI_API_KEY (if using assemblyai)
/// - OPENAI_API_KEY (if using openai)
#[derive(Parser)]
struct Args {
    #[arg(long)]
    url: Option<String>,

    /// Local audio file path (mutually exclusive with url)
    #[arg(long)]
    file: Option<String>,

    #[arg(long, default_value = "out/transcripts")]
    out: String,

    /// chunk length in seconds for long audio
    #[arg(long, default_value_t = 300)]
    chunk_seconds: u32,

    /// number of top candidate quotes to extract
    #[arg(long, default_value_t = 3)]
    top_quotes: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.out)?;

    // Step 1: obtain audio file. Either download via yt-dlp from URL, or use provided file.
    let mut audio_path = None::<PathBuf>;
    if let Some(url) = args.url {
        let out_file = PathBuf::from(&args.out).join("tmp_audio.m4a");
        // run yt-dlp
        let status = Command::new("yt-dlp")
            .arg("-x")
            .arg("--audio-format")
            .arg("m4a")
            .arg("-o")
            .arg(out_file.to_str().unwrap())
            .arg(url)
            .status()
            .await
            .context("running yt-dlp")?;
        if !status.success() {
            anyhow::bail!("yt-dlp failed");
        }
        audio_path = Some(out_file);
    } else if let Some(f) = args.file {
        audio_path = Some(PathBuf::from(f));
    } else {
        anyhow::bail!("either --url or --file must be provided");
    }

    let audio = audio_path.expect("audio path set");
    // Step 2: chunk audio with ffmpeg if duration > chunk_seconds
    let chunks = chunk_audio_if_needed(&audio, args.chunk_seconds)
        .context("chunking audio")?;

    // Step 3: call provider (assemblyai default)
    let provider = std::env::var("TRANSCRIBE_PROVIDER")
        .unwrap_or_else(|_| "assemblyai".to_string());
    let client = Client::new();
    let mut stitched_text = String::new();
    let mut stitched_segments = vec![];

    if provider.to_lowercase() == "assemblyai" {
        let api_key = std::env::var("ASSEMBLYAI_API_KEY")
            .context("ASSEMBLYAI_API_KEY is required for assemblyai")?;
        for (idx, chunk_path) in chunks.iter().enumerate() {
            let upload_url =
                upload_to_assemblyai(&client, &api_key, chunk_path).await?;
            let transcript =
                create_assemblyai_transcript(&client, &api_key, &upload_url)
                    .await?;
            // transcript contains text and segments if available
            stitched_text.push_str(&format!(
                "\n\n{}",
                transcript["text"].as_str().unwrap_or("")
            ));
            // gather segments with offset adjusted by chunk offset
            if let Some(segments) =
                transcript.get("segments").and_then(|s| s.as_array())
            {
                // each segment has start/end/send text
                for seg in segments {
                    stitched_segments.push(seg.clone());
                }
            }
            println!("chunk {} done", idx);
            // polite pause
            sleep(Duration::from_millis(300)).await;
        }
    } else {
        // fallback to OpenAI (whisper) - single upload per whole file (no segments)
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY required for openai")?;
        // If multiple chunks, upload and transcribe each similarly (skipping for brevity)
        for chunk in &chunks {
            let resp_text = openai_transcribe(&client, &api_key, chunk).await?;
            stitched_text.push_str(&resp_text);
        }
    }

    // Step 4: write stitched transcript JSON
    let out_json = PathBuf::from(&args.out).join("transcript.json");
    let mut obj = serde_json::Map::new();
    obj.insert(
        "text".to_string(),
        serde_json::Value::String(stitched_text.clone()),
    );
    obj.insert(
        "segments".to_string(),
        serde_json::Value::Array(stitched_segments.clone()),
    );
    fs::write(
        &out_json,
        serde_json::to_string_pretty(&serde_json::Value::Object(obj))?,
    )?;

    // Step 5: extract candidate quotes (simple heuristics: pick top N sentence-like chunks)
    let quotes = extract_candidate_quotes(&stitched_text, args.top_quotes);
    let quotes_json = PathBuf::from(&args.out).join("quotes.json");
    fs::write(&quotes_json, serde_json::to_string_pretty(&quotes)?)?;

    println!("Transcript and quotes saved to {}", args.out);
    Ok(())
}

/// Splits audio into chunks using ffmpeg if necessary.
/// Returns Vec<PathBuf> of chunk files (if no split, single element).
fn chunk_audio_if_needed(
    audio: &PathBuf,
    chunk_seconds: u32,
) -> Result<Vec<PathBuf>> {
    // get duration via ffprobe (if available); otherwise assume no chunking
    let ffprobe_out = std::process::Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(audio.to_str().unwrap())
        .output();

    if let Ok(out) = ffprobe_out {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Ok(sec) = s.trim().parse::<f32>() {
                if sec > (chunk_seconds as f32 + 1.0) {
                    // split
                    let mut chunks = vec![];
                    let mut start = 0;
                    let mut idx = 0;
                    while (start as f32) < sec {
                        let out_path = audio.with_file_name(format!(
                            "chunk-{}-{}.m4a",
                            audio.file_stem().unwrap().to_string_lossy(),
                            idx
                        ));
                        let end = (start + chunk_seconds) as f32;
                        let status = std::process::Command::new("ffmpeg")
                            .args(&[
                                "-y",
                                "-i",
                                audio.to_str().unwrap(),
                                "-ss",
                                &format!("{}", start),
                                "-t",
                                &format!("{}", chunk_seconds),
                                "-c",
                                "copy",
                                out_path.to_str().unwrap(),
                            ])
                            .status()?;
                        if !status.success() {
                            anyhow::bail!("ffmpeg chunk failed");
                        }
                        chunks.push(out_path);
                        idx += 1;
                        start += chunk_seconds as i32;
                    }
                    return Ok(chunks);
                }
            }
        }
    }
    // fallback: single chunk
    Ok(vec![audio.clone()])
}

/// Upload a chunk to AssemblyAI and return the upload_url
async fn upload_to_assemblyai(
    client: &Client,
    key: &str,
    path: &PathBuf,
) -> Result<String> {
    let url = "https://api.assemblyai.com/v2/upload";
    let bytes = tokio::fs::read(path).await?;
    let resp = client
        .post(url)
        .header("authorization", key)
        .body(bytes)
        .send()
        .await?
        .error_for_status()?;
    let v: serde_json::Value = resp.json().await?;
    Ok(v["upload_url"]
        .as_str()
        .context("upload_url missing")?
        .to_string())
}

/// Create a transcript job and poll until completion. Returns transcript object.
async fn create_assemblyai_transcript(
    client: &Client,
    key: &str,
    upload_url: &str,
) -> Result<serde_json::Value> {
    let create_url = "https://api.assemblyai.com/v2/transcript";
    let body = serde_json::json!({
        "audio_url": upload_url,
        "speaker_labels": false,
        "format_text": true,
        "disfluencies": true,
        "auto_chapters": false
    });
    let mut resp = client
        .post(create_url)
        .header("authorization", key)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    let v: serde_json::Value = resp.json().await?;
    let id = v["id"].as_str().context("transcript id")?.to_string();

    let poll_url = format!("https://api.assemblyai.com/v2/transcript/{}", id);
    // poll
    loop {
        let r = client
            .get(&poll_url)
            .header("authorization", key)
            .send()
            .await?
            .error_for_status()?;
        let j: serde_json::Value = r.json().await?;
        let status = j["status"].as_str().unwrap_or("");
        if status == "completed" {
            return Ok(j);
        } else if status == "error" {
            anyhow::bail!("transcription error: {:?}", j);
        } else {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

/// Fallback OpenAI transcription (simple single upload)
async fn openai_transcribe(
    client: &Client,
    key: &str,
    path: &PathBuf,
) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(bytes).file_name("audio.m4a"),
        )
        .text("model", "whisper-1");
    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(key)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(resp)
}

/// Very simple quote extractor: split into sentences and pick top N sentences by length (as proxy for "notable")
fn extract_candidate_quotes(
    text: &str,
    top_n: usize,
) -> Vec<serde_json::Value> {
    let sentences: Vec<_> = text
        .split_terminator(|c| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let mut scored: Vec<_> = sentences.iter().map(|s| (s.len(), s)).collect();
    scored.sort_by_key(|(len, _)| *len);
    scored.reverse();
    scored
        .iter()
        .take(top_n)
        .map(|(_, s)| serde_json::json!({"quote": *s}))
        .collect()
}
