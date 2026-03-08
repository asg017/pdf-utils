use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pdf-utils", about = "Basic PDF manipulation tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show info about a PDF file (page count, metadata, encrypted status)
    Info {
        /// Input PDF file
        input: PathBuf,
    },

    /// Extract specific pages from a PDF into a new file
    Extract {
        /// Input PDF file
        input: PathBuf,
        /// Output PDF file
        #[arg(short, long)]
        output: PathBuf,
        /// Page numbers to extract (1-based, comma-separated, e.g. "1,3,5")
        #[arg(short, long)]
        pages: String,
    },

    /// Merge multiple PDF files into one
    Merge {
        /// Input PDF files (two or more)
        inputs: Vec<PathBuf>,
        /// Output PDF file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Create a new blank PDF with the given number of pages
    Create {
        /// Output PDF file
        output: PathBuf,
        /// Number of blank pages to create
        #[arg(short, long, default_value = "1")]
        pages: usize,
        /// Page size: letter, a4, legal, tabloid
        #[arg(short, long, default_value = "letter")]
        size: String,
    },

    /// Extract text from a PDF
    Text {
        /// Input PDF file
        input: PathBuf,
        /// Output text file (defaults to <input>.txt)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Page numbers to extract (1-based, e.g. "1-10,11,12")
        #[arg(long)]
        pages: Option<String>,
    },

    /// Extract all images from a PDF into a directory
    ExtractImages {
        /// Input PDF file
        input: PathBuf,
        /// Output directory
        #[arg(short, long)]
        output: PathBuf,
        /// Page numbers to extract from (1-based, e.g. "1-3,5,21-40")
        #[arg(short, long)]
        pages: Option<String>,
    },

    /// Render PDF pages to images (requires pdftoppm)
    Screenshot {
        /// Input PDF file
        input: PathBuf,
        /// Output directory or file prefix
        #[arg(short, long)]
        output: PathBuf,
        /// Page range (e.g. "1", "1-5")
        #[arg(long)]
        pages: Option<String>,
        /// DPI resolution (default: 150)
        #[arg(long, default_value = "150")]
        dpi: u32,
        /// Output format: png, jpeg, tiff (default: png)
        #[arg(long, default_value = "png")]
        format: String,
        /// Fit within this pixel box (overrides --dpi)
        #[arg(long)]
        scale_to: Option<u32>,
        /// Grayscale output
        #[arg(long)]
        gray: bool,
        /// Single page mode (no page number suffix)
        #[arg(long)]
        single: bool,
    },

    /// Copy PDF content to clipboard (text by default, or as PNG image)
    Copy {
        /// Input PDF file
        input: PathBuf,
        /// Format: text/txt or png
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Page number (1-based, required for png with multi-page PDFs)
        #[arg(short, long)]
        page: Option<usize>,
        /// Copy the raw embedded image from the page (errors if not exactly 1 image)
        #[arg(long)]
        raw: bool,
    },

    /// Set metadata on a PDF file
    SetMeta {
        /// Input PDF file
        input: PathBuf,
        /// Output PDF file
        #[arg(short, long)]
        output: PathBuf,
        /// Document title
        #[arg(long)]
        title: Option<String>,
        /// Document author
        #[arg(long)]
        author: Option<String>,
        /// Document subject
        #[arg(long)]
        subject: Option<String>,
    },
}
