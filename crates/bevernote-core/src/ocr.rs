use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

use crate::error::{BeverError, Result};

/// On-device OCR using bundled, openly licensed models. No network access is used.
pub struct LocalOcr {
    engine: OcrEngine,
}

impl LocalOcr {
    pub fn new() -> Result<Self> {
        let detection_model = Model::load(include_bytes!("../models/text-detection.onnx").to_vec())
            .map_err(|error| BeverError::Invalid(format!("load OCR detection model: {error}")))?;
        let recognition_model = Model::load(
            include_bytes!("../models/text-recognition.onnx").to_vec(),
        )
        .map_err(|error| BeverError::Invalid(format!("load OCR recognition model: {error}")))?;
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .map_err(|error| BeverError::Invalid(format!("initialize OCR: {error}")))?;
        Ok(Self { engine })
    }

    pub fn recognize(&mut self, bytes: &[u8]) -> Result<Option<String>> {
        // ENEX already bounds this by the current resource. Avoid spending minutes on
        // oversized images; the original bytes are always retained untouched.
        if bytes.len() > 40 * 1024 * 1024 {
            return Ok(None);
        }
        let decoded = image::load_from_memory(bytes)
            .map_err(|error| BeverError::Invalid(format!("decode OCR image: {error}")))?;
        let image = decoded.thumbnail(2200, 2200).into_rgb8();
        let source = ImageSource::from_bytes(image.as_raw(), image.dimensions())
            .map_err(|error| BeverError::Invalid(format!("prepare OCR image: {error}")))?;
        let input = self
            .engine
            .prepare_input(source)
            .map_err(|error| BeverError::Invalid(format!("prepare OCR input: {error}")))?;
        let text = self
            .engine
            .get_text(&input)
            .map_err(|error| BeverError::Invalid(format!("recognize image text: {error}")))?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        Ok((!text.is_empty()).then_some(text))
    }
}
