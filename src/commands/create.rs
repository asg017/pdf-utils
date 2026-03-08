use anyhow::{Context, Result};
use pdf_lib_rs::api::PdfDocument;
use std::path::PathBuf;

use crate::pdf_utils::*;

pub fn run(output: PathBuf, pages: usize, size: String) -> Result<()> {
    let page_size = parse_page_size(&size)?;
    let mut doc = PdfDocument::create();
    for _ in 0..pages {
        doc.add_page(page_size);
    }

    let bytes = doc.save();
    std::fs::write(&output, bytes)
        .with_context(|| format!("Failed to write {}", output.display()))?;
    println!(
        "Created {} with {} {} page(s)",
        output.display(),
        pages,
        size
    );
    Ok(())
}
