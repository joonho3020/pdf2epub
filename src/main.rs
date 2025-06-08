use pdfium_render::prelude::*;
use thiserror::Error;
use image::{DynamicImage, RgbImage};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use std::path::PathBuf;
use rten::{Model, ModelLoadError};
#[allow(unused)]
use rten_tensor::prelude::*;

#[derive(Debug, Error)]
pub enum Pdf2EPubErr {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("PdfiumError error: {0}")]
    PdfiumError(#[from] PdfiumError),

    #[error("OCRSImageSourceError error: {0}")]
    OCRSImageSourceError(#[from] ocrs::ImageSourceError),

    #[error("ModelLoadError error: {0}")]
    ModelLoadError(#[from] ModelLoadError),

    #[error("AnyHowError error: {0}")]
    AnyHowError(#[from] anyhow::Error)
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

    // 1️⃣ Rasterise with Pdfium
    let bitmap = page.render_with_config(&render_config)?;
    let dyn_image: DynamicImage = bitmap.as_image();
    let rgb8: RgbImage = dyn_image.into_rgb8();

    Ok(rgb8)

}

pub fn perform_ocr(img: &RgbImage, engine: &OcrEngine) -> Result<String, Pdf2EPubErr> {
    let img_source = ImageSource::from_bytes(img.as_raw(), img.dimensions())?;
    let ocr_input = engine.prepare_input(img_source)?;
    let text = engine.get_text(&ocr_input)?;
    Ok(text)
}

/// Given a file path relative to the crate root, return the absolute path.
fn file_path(path: &str) -> PathBuf {
    let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    abs_path.push(path);
    abs_path
}

/// Create a new OCR engine
pub fn ocr_engine() -> Result<OcrEngine, Pdf2EPubErr> {
    let detection_model_path = file_path("examples/text-detection.rten");
    let rec_model_path = file_path("examples/text-recognition.rten");

    let detection_model = Model::load_file(detection_model_path)?;
    let recognition_model = Model::load_file(rec_model_path)?;

    Ok(OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })?)
}

fn main() -> Result<(), Pdf2EPubErr> {
// let pdfium = Pdfium::new(Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./pdfium/lib")).unwrap());
    for (index, page) in Pdfium::default().load_pdf_from_file("./examples/test-1.pdf", None)?.pages().iter().enumerate() {
        page.objects();
    }

    Ok(())
}
