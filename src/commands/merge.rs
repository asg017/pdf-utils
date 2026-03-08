use anyhow::{Context, Result};
use pdf_lib_rs::api::PdfDocument;
use std::path::PathBuf;

use crate::pdf_utils::*;

pub fn run(inputs: Vec<PathBuf>, output: PathBuf) -> Result<()> {
    if inputs.len() < 2 {
        anyhow::bail!("Need at least 2 input files to merge");
    }

    let mut dest = PdfDocument::create();
    let mut total = 0;

    for path in &inputs {
        let src = load_pdf(path)?;
        let page_count = src.get_page_count();
        let indices: Vec<usize> = (0..page_count).collect();
        dest.copy_pages(&src, &indices);
        total += page_count;
        println!("  {} ({} pages)", path.display(), page_count);
    }

    let bytes = dest.save();
    std::fs::write(&output, bytes)
        .with_context(|| format!("Failed to write {}", output.display()))?;
    println!("Merged {} pages into {}", total, output.display());
    Ok(())
}
