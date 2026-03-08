use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::pdf_utils::*;

#[allow(clippy::too_many_arguments)]
pub fn run(
    input: PathBuf,
    output: PathBuf,
    pages: Option<String>,
    dpi: u32,
    format: String,
    scale_to: Option<u32>,
    gray: bool,
    single: bool,
) -> Result<()> {
    let which = std::process::Command::new("which")
        .arg("pdftoppm")
        .output();
    match which {
        Ok(o) if o.status.success() => {}
        _ => anyhow::bail!("pdftoppm not found on PATH. Install poppler: brew install poppler (macOS) or apt install poppler-utils (Linux)"),
    }

    fs::create_dir_all(&output)
        .with_context(|| format!("Failed to create directory {}", output.display()))?;

    let (first_page, last_page) = if let Some(ref p) = pages {
        let (f, l) = parse_page_range(p)?;
        (Some(f), Some(l))
    } else {
        (None, None)
    };

    let prefix = output.join("page");
    let sa = ScreenshotArgs {
        input: input.display().to_string(),
        output_prefix: prefix.display().to_string(),
        first_page,
        last_page,
        dpi,
        format: format.clone(),
        scale_to,
        gray,
        single,
    };

    let args = build_pdftoppm_args(&sa);
    let status = std::process::Command::new("pdftoppm")
        .args(&args)
        .status()
        .context("Failed to run pdftoppm")?;

    if !status.success() {
        anyhow::bail!("pdftoppm exited with status {}", status);
    }

    let ext = match format.as_str() {
        "jpeg" | "jpg" => "jpg",
        "tiff" | "tif" => "tif",
        _ => "png",
    };
    let mut count = 0;
    for entry in fs::read_dir(&output)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            if let Some(name) = path.file_name() {
                println!("  {}", name.to_string_lossy());
                count += 1;
            }
        }
    }
    eprintln!("Rendered {} page(s) to {}", count, output.display());
    Ok(())
}
