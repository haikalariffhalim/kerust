use anyhow::{Context, Result};
use image::io::Reader as ImageReader;
use image::{
    imageops::FilterType, DynamicImage, GenericImageView, ImageOutputFormat,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;

/// Generate variants: JPEG (quality), WebP, thumbnails sizes (array), apply deterministic style.
/// Returns manifest (list of generated file paths).
pub async fn generate_image_variants(
    input: &Path,
    out_dir: &Path,
    sizes: &[u32],
    quality: u8,
    grain_strength: f32,
    saturation_mul: f32,
) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(out_dir)
        .await
        .context("create out dir")?;
    let img = ImageReader::open(input)?.decode().context("decode image")?;
    let mut out_paths = vec![];

    // for each requested size produce jpeg + webp
    for &w in sizes {
        let resized = resize_preserve(&img, w);
        let styled = apply_style(&resized, grain_strength, saturation_mul)?;
        // jpeg path
        let jpeg_path = out_dir.join(format!("{}w.jpg", w));
        {
            let mut buf = Vec::new();
            styled
                .write_to(&mut buf, ImageOutputFormat::Jpeg(quality.into()))
                .context("write jpeg")?;
            fs::write(&jpeg_path, &buf).await?;
        }
        // try progressive via cjpeg if present
        let prog_path = out_dir.join(format!("{}w-prog.jpg", w));
        if try_make_progressive(&jpeg_path, &prog_path).is_ok() {
            out_paths.push(prog_path);
        } else {
            out_paths.push(jpeg_path.clone());
        }

        // webp
        let webp_path = out_dir.join(format!("{}w.webp", w));
        {
            let mut buf = Vec::new();
            styled.write_to(&mut buf, ImageOutputFormat::WebP)?; // requires webp support in image crate
            fs::write(&webp_path, &buf).await?;
        }
        out_paths.push(webp_path);
    }

    Ok(out_paths)
}

fn resize_preserve(img: &DynamicImage, target_width: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w <= target_width {
        img.clone()
    } else {
        let ratio = target_width as f32 / w as f32;
        let new_h = (h as f32 * ratio).round() as u32;
        img.resize_exact(target_width, new_h, FilterType::Lanczos3)
    }
}

fn apply_style(
    img: &DynamicImage,
    grain_strength: f32,
    saturation_mul: f32,
) -> Result<DynamicImage> {
    // simple leklek watkul watpis: adjust contrast, basic saturation via conversion, and deterministic grain

    let mut out = img.adjust_contrast(10.0);
    let mut buf = out.to_rgba8();

    // saturation approximation: scale RGB from center
    for pixel in buf.pixels_mut() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;
        let avg = (r + g + b) / 3.0;
        let r2 = (avg + (r - avg) * (1.0 + saturation_mul)).min(1.0);
        let g2 = (avg + (g - avg) * (1.0 + saturation_mul)).min(1.0);
        let b2 = (avg + (b - avg) * (1.0 + saturation_mul)).min(1.0);
        pixel[0] = (r2 * 255.0).round() as u8;
        pixel[1] = (g2 * 255.0).round() as u8;
        pixel[2] = (b2 * 255.0).round() as u8;
    }

    // deterministic grain
    let mut rng = StdRng::seed_from_u64(42);
    for pixel in buf.pixels_mut() {
        let jitter =
            ((rng.gen::<f32>() - 0.5) * 2.0 * grain_strength * 255.0) as i32;
        for i in 0..3 {
            let v = (pixel[i] as i32 + jitter).clamp(0, 255) as u8;
            pixel[i] = v;
        }
    }

    Ok(DynamicImage::ImageRgba8(buf))
}

/// Try to create progressive jpeg using external cjpeg (libjpeg) if present
fn try_make_progressive(input: &Path, out: &Path) -> Result<(), anyhow::Error> {
    // cjpeg -quality 85 -progressive -outfile out input
    let status = Command::new("cjpeg")
        .arg("-quality")
        .arg("85")
        .arg("-progressive")
        .arg("-outfile")
        .arg(out)
        .arg(input)
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(anyhow::anyhow!("cjpeg not available or failed")),
    }
}
