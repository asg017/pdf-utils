use anyhow::{Context, Result};
use pdf_lib_rs::api::PdfDocument;
use std::path::PathBuf;

pub fn run(
    input: PathBuf,
    output: PathBuf,
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
) -> Result<()> {
    let bytes = std::fs::read(&input)
        .with_context(|| format!("Failed to read {}", input.display()))?;
    let mut doc =
        PdfDocument::load(&bytes).map_err(|e| anyhow::anyhow!("Failed to parse PDF: {}", e))?;

    if let Some(t) = &title {
        doc.set_title(t);
        println!("Set title: {}", t);
    }
    if let Some(a) = &author {
        doc.set_author(a);
        println!("Set author: {}", a);
    }
    if let Some(s) = &subject {
        doc.set_subject(s);
        println!("Set subject: {}", s);
    }

    let out_bytes = doc.save();
    std::fs::write(&output, out_bytes)
        .with_context(|| format!("Failed to write {}", output.display()))?;
    println!("Written to {}", output.display());
    Ok(())
}
