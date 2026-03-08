use anyhow::Result;
use pdf_lib_rs::core::objects::{PdfName, PdfObject};
use std::path::PathBuf;

use crate::pdf_utils::*;

pub fn run(input: PathBuf) -> Result<()> {
    let doc = load_pdf(&input)?;
    println!("File: {}", input.display());
    println!("Pages: {}", doc.get_page_count());
    println!("Encrypted: {}", doc.is_encrypted());
    if let Some(title) = doc.get_title() {
        println!("Title: {}", title);
    }
    if let Some(author) = doc.get_author() {
        println!("Author: {}", author);
    }

    let ctx = doc.context();
    let page_refs = doc.get_page_refs();
    println!();
    for (i, page_ref) in page_refs.iter().enumerate() {
        let mut text_size: usize = 0;
        let mut image_count: usize = 0;
        let mut image_size: usize = 0;

        if let Some(PdfObject::Dict(page_dict)) = ctx.lookup(page_ref) {
            match page_dict.get(&PdfName::of("Contents")) {
                Some(PdfObject::Ref(r)) => {
                    text_size += stream_raw_size(ctx, r);
                }
                Some(PdfObject::Array(arr)) => {
                    for j in 0..arr.size() {
                        if let Some(PdfObject::Ref(r)) = arr.get(j) {
                            text_size += stream_raw_size(ctx, r);
                        }
                    }
                }
                _ => {}
            }

            if let Some(res) = get_page_resources(ctx, page_dict) {
                if let Some(xobj_dict) = res
                    .get(&PdfName::of("XObject"))
                    .and_then(|o| resolve_dict(ctx, o))
                {
                    for (_name, value) in xobj_dict.entries() {
                        if let PdfObject::Ref(r) = value {
                            if let Some(PdfObject::Stream(s)) = ctx.lookup(r) {
                                if let Some(PdfObject::Name(subtype)) =
                                    s.dict.get(&PdfName::of("Subtype"))
                                {
                                    if subtype.as_string() == "/Image" {
                                        image_count += 1;
                                        image_size += s.contents.len();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let text_kb = text_size as f64 / 1024.0;
        let image_mb = image_size as f64 / (1024.0 * 1024.0);
        println!(
            "  Page {:>3}: text {:.1} KB | {} image(s) {:.2} MB",
            i + 1,
            text_kb,
            image_count,
            image_mb
        );
    }
    Ok(())
}
