use anyhow::{Context, Result};
use pdf_lib_rs::core::objects::PdfObject;
use std::path::PathBuf;

use crate::pdf_utils::*;

pub fn run(input: PathBuf, format: String, page: Option<usize>, raw: bool) -> Result<()> {
    let fmt = format.to_lowercase();
    let is_png = fmt == "png";
    let is_text = fmt == "text" || fmt == "txt";
    if !is_png && !is_text {
        anyhow::bail!("Unsupported format '{}'. Use: text, txt, or png", format);
    }

    let doc = load_pdf(&input)?;
    let ctx = doc.context();
    let page_refs = doc.get_page_refs();
    let page_count = page_refs.len();

    if is_text {
        let indices = if let Some(p) = page {
            if p == 0 || p > page_count {
                anyhow::bail!(
                    "Page {} out of range (document has {} pages)",
                    p,
                    page_count
                );
            }
            vec![p - 1]
        } else {
            (0..page_count).collect()
        };

        let mut all_text = String::new();
        for &idx in &indices {
            if let Some(PdfObject::Dict(page_dict)) = ctx.lookup(&page_refs[idx]) {
                let text = extract_text_from_page(ctx, page_dict);
                if !text.is_empty() {
                    if !all_text.is_empty() {
                        all_text.push_str("\n\n");
                    }
                    all_text.push_str(&text);
                }
            }
        }

        let mut clipboard =
            arboard::Clipboard::new().context("Failed to access clipboard")?;
        clipboard
            .set_text(&all_text)
            .context("Failed to copy text to clipboard")?;
        eprintln!(
            "Copied text from {} page(s) to clipboard ({} chars)",
            indices.len(),
            all_text.len()
        );
    } else {
        // PNG mode
        if raw {
            let page_idx = if let Some(p) = page {
                if p == 0 || p > page_count {
                    anyhow::bail!(
                        "Page {} out of range (document has {} pages)",
                        p,
                        page_count
                    );
                }
                p - 1
            } else {
                if page_count != 1 {
                    anyhow::bail!(
                        "Document has {} pages, use -p to specify which page",
                        page_count
                    );
                }
                0
            };

            let page_images = collect_images(ctx, &[page_refs[page_idx].clone()]);
            if page_images.is_empty() {
                anyhow::bail!("No images found on page {}", page_idx + 1);
            }
            if page_images.len() > 1 {
                anyhow::bail!(
                    "Page {} has {} images (--raw requires exactly 1)",
                    page_idx + 1,
                    page_images.len()
                );
            }

            let tmp =
                tempfile::NamedTempFile::new().context("Failed to create temp file")?;
            let tmp_path = tmp.path().with_extension("png");
            write_image(page_images[0].stream, &tmp_path)?;

            let img = image::open(&tmp_path).context("Failed to decode image")?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();

            let mut clipboard =
                arboard::Clipboard::new().context("Failed to access clipboard")?;
            clipboard
                .set_image(arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: std::borrow::Cow::Borrowed(rgba.as_raw()),
                })
                .context("Failed to copy image to clipboard")?;

            let _ = std::fs::remove_file(&tmp_path);
            eprintln!(
                "Copied raw image from page {} to clipboard ({}x{})",
                page_idx + 1,
                w,
                h
            );
        } else {
            let which = std::process::Command::new("which")
                .arg("pdftoppm")
                .output();
            match which {
                Ok(o) if o.status.success() => {}
                _ => anyhow::bail!("pdftoppm not found on PATH. Install poppler: brew install poppler (macOS) or apt install poppler-utils (Linux)"),
            }

            let page_num = if let Some(p) = page {
                if p == 0 || p > page_count {
                    anyhow::bail!(
                        "Page {} out of range (document has {} pages)",
                        p,
                        page_count
                    );
                }
                p as u32
            } else {
                if page_count != 1 {
                    anyhow::bail!(
                        "Document has {} pages, use -p to specify which page",
                        page_count
                    );
                }
                1
            };

            let tmp_dir =
                tempfile::tempdir().context("Failed to create temp directory")?;
            let prefix = tmp_dir.path().join("page");

            let sa = ScreenshotArgs {
                input: input.display().to_string(),
                output_prefix: prefix.display().to_string(),
                first_page: Some(page_num),
                last_page: Some(page_num),
                dpi: 150,
                format: "png".to_string(),
                scale_to: None,
                gray: false,
                single: true,
            };

            let args = build_pdftoppm_args(&sa);
            let status = std::process::Command::new("pdftoppm")
                .args(&args)
                .status()
                .context("Failed to run pdftoppm")?;
            if !status.success() {
                anyhow::bail!("pdftoppm exited with status {}", status);
            }

            let png_path = prefix.with_extension("png");
            if !png_path.exists() {
                anyhow::bail!(
                    "pdftoppm did not produce expected output at {}",
                    png_path.display()
                );
            }

            let img =
                image::open(&png_path).context("Failed to decode rendered PNG")?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();

            let mut clipboard =
                arboard::Clipboard::new().context("Failed to access clipboard")?;
            clipboard
                .set_image(arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: std::borrow::Cow::Borrowed(rgba.as_raw()),
                })
                .context("Failed to copy image to clipboard")?;

            eprintln!(
                "Copied page {} to clipboard as PNG ({}x{})",
                page_num, w, h
            );
        }
    }
    Ok(())
}
