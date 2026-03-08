use anyhow::{Context, Result};
use pdf_lib_rs::api::PdfDocument;
use std::path::PathBuf;

use crate::pdf_utils::*;

pub fn run(input: PathBuf, output: PathBuf, pages: String) -> Result<()> {
    let src = load_pdf(&input)?;
    let page_count = src.get_page_count();
    let indices = parse_page_list(&pages, page_count)?;

    let mut dest = PdfDocument::create();
    let copied = dest.copy_pages(&src, &indices);
    println!("Extracted {} pages from {}", copied.len(), input.display());

    let bytes = dest.save();
    std::fs::write(&output, bytes)
        .with_context(|| format!("Failed to write {}", output.display()))?;
    println!("Written to {}", output.display());
    Ok(())
}
