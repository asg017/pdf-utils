use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::pdf_utils::*;

pub fn run(input: PathBuf, output: PathBuf, pages: Option<String>) -> Result<()> {
    let doc = load_pdf(&input)?;
    let ctx = doc.context();
    let all_page_refs = doc.get_page_refs();

    let page_refs: Vec<_> = if let Some(spec) = &pages {
        let indices = parse_page_list(spec, all_page_refs.len())?;
        indices.iter().map(|&i| all_page_refs[i].clone()).collect()
    } else {
        all_page_refs.to_vec()
    };

    let images = collect_images(ctx, &page_refs);

    if images.is_empty() {
        println!("No images found in {}", input.display());
        return Ok(());
    }

    std::fs::create_dir_all(&output)
        .with_context(|| format!("Failed to create directory {}", output.display()))?;

    for (idx, img) in images.iter().enumerate() {
        let ext = image_extension(img.stream);
        let filename = format!("page{}-img{}.{}", img.page, idx + 1, ext);
        let path = output.join(&filename);
        match write_image(img.stream, &path) {
            Ok(size) => {
                println!("  {} ({:.1} KB)", filename, size as f64 / 1024.0);
            }
            Err(e) => {
                eprintln!("  {} - failed: {}", filename, e);
            }
        }
    }
    println!(
        "Extracted {} image(s) to {}",
        images.len(),
        output.display()
    );
    Ok(())
}
