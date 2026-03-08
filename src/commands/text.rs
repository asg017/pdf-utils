use anyhow::{Context, Result};
use pdf_lib_rs::core::objects::PdfObject;
use std::io::Write;
use std::path::PathBuf;

use crate::pdf_utils::*;

pub fn run(input: PathBuf, output: Option<PathBuf>, pages: Option<String>) -> Result<()> {
    let doc = load_pdf(&input)?;
    let ctx = doc.context();
    let page_refs = doc.get_page_refs();
    let page_count = page_refs.len();

    let indices = if let Some(ref p) = pages {
        parse_page_list(p, page_count)?
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

    if let Some(path) = output {
        std::fs::write(&path, &all_text)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        eprintln!(
            "Extracted text from {} page(s) to {}",
            indices.len(),
            path.display()
        );
    } else {
        std::io::stdout().write_all(all_text.as_bytes())?;
        if !all_text.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}
