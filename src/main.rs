use std::io::Cursor;
use std::path::PathBuf;
use indicatif;
use clap::Parser;
use thiserror::Error;
use pdfium_render::prelude::*;
use leptess::LepTess;
use image::{DynamicImage, RgbImage, ImageFormat};
use anyhow::{Context, Result};
use epub_builder::{EpubBuilder, EpubContent, ZipLibrary, ReferenceType};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input file path
    #[arg(short, long)]
    input: PathBuf,

    /// Title of the book
    #[arg(long)]
    title: Option<String>,

    /// Author of the book
    #[arg(long)]
    author: Option<String>,

    /// If set to true, remove pagenum from the bottom of the page
    #[arg(long)]
    extract_pagenum: bool,
}

#[derive(Debug, Error)]
pub enum Pdf2EPubErr {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("PdfiumError error: {0}")]
    PdfiumError(#[from] PdfiumError),

    #[error("AnyHowError error: {0}")]
    AnyHowError(#[from] anyhow::Error),

    #[error("ZipLibrary error")]
    ZipLibraryError(#[from] epub_builder::Error),
}

/// Convert a single `PdfPage` into the RGB byte buffer
/// - `target_dpi` controls the rasterisation resolution
pub fn img_source_from_page(
    page: &PdfPage,
    target_dpi: u16,
) -> Result<RgbImage, Pdf2EPubErr> {
    let w_inch = page.paper_size().width().to_inches();
    let w_pixels = (w_inch * (target_dpi as f32)) as i32;

    let h_inch = page.paper_size().height().to_inches();
    let h_pixels = (h_inch * (target_dpi as f32)) as i32;

    let render_config = PdfRenderConfig::new()
        .set_target_width(w_pixels)
        .set_target_height(h_pixels)
        .use_grayscale_rendering(true);

    let bitmap = page.render_with_config(&render_config)?;
    let dyn_image: DynamicImage = bitmap.as_image();
    let rgb8: RgbImage = dyn_image.into_rgb8();

    Ok(rgb8)
}

/// Perform ocr on `RbgImage` using Tesseract
pub fn ocr_rgb_png(img: &RgbImage) -> Result<String, Pdf2EPubErr> {
    let mut png_bytes: Vec<u8> = Vec::new();
    DynamicImage::ImageRgb8(img.clone())
        .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .context("failed to encode PNG")?;

    let mut lt = LepTess::new(None, "eng")
        .context("could not create Tesseract engine")?;

    lt.set_image_from_mem(&png_bytes)
        .context("Tesseract failed to load image from memory")?;

    let text = lt.get_utf8_text()
        .context("Tesseract failed to recognise text")?;

    Ok(text)
}

/// Remove a trailing page number like "...some text\n\n11" and return it.
/// On failure the original text is left intact and page_num is None.
pub fn peel_trailing_page_num(s: &str) -> (&str, Option<u32>) {
    let trimmed = s.trim_end();
    match trimmed.rsplit_once(char::is_whitespace) {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            (head.trim_end(), Some(tail.parse::<u32>().expect("number")))
        }
        _ => (trimmed, None),
    }
}

/// Incrementally unwraps hard-wrapped lines *and* removes fake page-break
/// blank lines.  Call `push_line()` for every raw line (in reading order),
/// `page_break()` after finishing a page, and `finish()` at the very end.
pub struct LineUnwrapper {
    /// current paragraph being built
    buf: String,

    // fully emitted text
    out: String,

    pending_blank: bool,
}

impl LineUnwrapper {
    pub fn new() -> Self {
        Self { buf: String::new(), out: String::new(), pending_blank: false }
    }

    /// Push one **raw** line (possibly blank, with trailing `\n` removed).
    pub fn push_line(&mut self, raw: &str) {
        let line = raw.trim();

        if line.is_empty() {
            // postpone decision until we see the next non-blank line
            self.pending_blank = true;
            return;
        }

        // Decide what that previous blank really meant
        if self.pending_blank {
            self.pending_blank = false;

            let prev_ended_sentence = self
                .buf
                .chars()
                .rev()
                .find(|c| !c.is_whitespace())
                .map(|c| ".?!".contains(c))
                .unwrap_or(false);

            let this_starts_lower = line
                .chars()
                .next()
                .map(|c| c.is_lowercase())
                .unwrap_or(false);

            if prev_ended_sentence || !this_starts_lower {
                // Real paragraph break → flush current paragraph.
                if !self.buf.is_empty() {
                    self.out.push_str(self.buf.trim_end());
                    self.out.push_str("\n\n");
                    self.buf.clear();
                }
            }
            // else: fake blank (from a page break); keep building same ¶
        }

        // Join the current line onto the paragraph buffer
        if !self.buf.is_empty() {
            if self.buf.ends_with('-') {
                self.buf.pop();
            } else {
                self.buf.push(' ');
            }
        }
        self.buf.push_str(line);
    }

    /// Consume the unwrapper and return the cleaned text
    pub fn finish(mut self) -> String {
        if !self.buf.is_empty() {
            self.out.push_str(self.buf.trim_end());
        }
        self.out
    }
}

fn text_to_xhtml(title: &str, body: &str) -> String {
    use html_escape::encode_text;

    let paras = body
        .split("\n\n")                 // our “real” paragraph breaks
        .map(|p| format!("<p>{}</p>", encode_text(p)))
        .collect::<String>();

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
           <html xmlns="http://www.w3.org/1999/xhtml">
             <head><title>{}</title></head>
             <body>{}</body>
           </html>"#,
        encode_text(title),
        paras
    )
}

fn main() -> Result<(), Pdf2EPubErr> {
    let args = Args::parse();

    let pdfium = Pdfium::new(Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./pdfium/lib")).unwrap());
    let pdf = pdfium.load_pdf_from_file(args.input.to_str().expect("Invalid input path"), None)?;
    let progress_bar = indicatif::ProgressBar::new(pdf.pages().len() as u64);
    let mut cleaner = LineUnwrapper::new();

    for (_index, page) in pdf.pages().iter().enumerate() {
        progress_bar.inc(1);
        let img = img_source_from_page(&page, 300)?;
        let raw_text = ocr_rgb_png(&img)?;

        let (text, _pagenum_opt) = if args.extract_pagenum {
            peel_trailing_page_num(&raw_text)
        } else {
            (raw_text.as_str(), None)
        };

        for line in text.lines() {
            cleaner.push_line(line);
        }
    }
    progress_bar.finish();
    let final_text = cleaner.finish();

    let title = args.title.unwrap_or("ebook-output".to_string());
    let author = args.author.unwrap_or("unknown author".to_string());

    let mut epub = EpubBuilder::new(ZipLibrary::new()?)?;
    epub.metadata("title",  &title)?;
    epub.metadata("author", &author)?;
    epub.set_lang("en");

    let xhtml = text_to_xhtml(&title, &final_text);
    epub.add_content(
        EpubContent::new("FILENAME".to_string(), xhtml.as_bytes())
        .title(&title)
        .level(1)              // depth in the TOC
        .reftype(ReferenceType::Text),
    )?;

    let outfile = format!("{}-by-{}.epub", title, author);
    let mut out = std::fs::File::create(outfile)?;
    epub.generate(&mut out)?;

    Ok(())
}
