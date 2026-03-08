mod cli;
mod commands;
pub mod pdf_utils;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Info { input } => commands::info::run(input),
        Commands::Text {
            input,
            output,
            pages,
        } => commands::text::run(input, output, pages),
        Commands::Extract {
            input,
            output,
            pages,
        } => commands::extract::run(input, output, pages),
        Commands::ExtractImages { input, output, pages } => commands::images::run(input, output, pages),
        Commands::Merge { inputs, output } => commands::merge::run(inputs, output),
        Commands::Create {
            output,
            pages,
            size,
        } => commands::create::run(output, pages, size),
        Commands::Screenshot {
            input,
            output,
            pages,
            dpi,
            format,
            scale_to,
            gray,
            single,
        } => commands::screenshot::run(input, output, pages, dpi, format, scale_to, gray, single),
        Commands::Copy {
            input,
            format,
            page,
            raw,
        } => commands::copy::run(input, format, page, raw),
        Commands::SetMeta {
            input,
            output,
            title,
            author,
            subject,
        } => commands::set_meta::run(input, output, title, author, subject),
    }
}

#[cfg(test)]
mod tests {
    use crate::pdf_utils::*;
    use insta::assert_snapshot;

    fn default_args() -> ScreenshotArgs {
        ScreenshotArgs {
            input: "input.pdf".to_string(),
            output_prefix: "out/page".to_string(),
            first_page: None,
            last_page: None,
            dpi: 150,
            format: "png".to_string(),
            scale_to: None,
            gray: false,
            single: false,
        }
    }

    #[test]
    fn screenshot_default_args() {
        let args = build_pdftoppm_args(&default_args());
        assert_snapshot!(args.join(" "), @"-png -r 150 input.pdf out/page");
    }

    #[test]
    fn screenshot_jpeg_300dpi() {
        let args = build_pdftoppm_args(&ScreenshotArgs {
            dpi: 300,
            format: "jpeg".to_string(),
            ..default_args()
        });
        assert_snapshot!(args.join(" "), @"-jpeg -r 300 input.pdf out/page");
    }

    #[test]
    fn screenshot_page_range() {
        let args = build_pdftoppm_args(&ScreenshotArgs {
            first_page: Some(1),
            last_page: Some(5),
            ..default_args()
        });
        assert_snapshot!(args.join(" "), @"-png -r 150 -f 1 -l 5 input.pdf out/page");
    }

    #[test]
    fn screenshot_single_page() {
        let args = build_pdftoppm_args(&ScreenshotArgs {
            first_page: Some(3),
            last_page: Some(3),
            single: true,
            ..default_args()
        });
        assert_snapshot!(args.join(" "), @"-png -r 150 -f 3 -l 3 -singlefile input.pdf out/page");
    }

    #[test]
    fn screenshot_scale_to() {
        let args = build_pdftoppm_args(&ScreenshotArgs {
            scale_to: Some(1024),
            ..default_args()
        });
        assert_snapshot!(args.join(" "), @"-png -scale-to 1024 input.pdf out/page");
    }

    #[test]
    fn screenshot_grayscale_tiff() {
        let args = build_pdftoppm_args(&ScreenshotArgs {
            format: "tiff".to_string(),
            gray: true,
            ..default_args()
        });
        assert_snapshot!(args.join(" "), @"-tiff -r 150 -gray input.pdf out/page");
    }

    #[test]
    fn screenshot_all_options() {
        let args = build_pdftoppm_args(&ScreenshotArgs {
            input: "my doc.pdf".to_string(),
            output_prefix: "/tmp/out/page".to_string(),
            first_page: Some(2),
            last_page: Some(10),
            dpi: 200,
            format: "jpg".to_string(),
            scale_to: None,
            gray: true,
            single: false,
        });
        assert_snapshot!(args.join(" "), @"-jpeg -r 200 -f 2 -l 10 -gray my doc.pdf /tmp/out/page");
    }
}
