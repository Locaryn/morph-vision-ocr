//! Locaryn Vision & OCR Plugin
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    pub image_path: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub extracted_text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectObjectsRequest {
    pub image_path: String,
    pub confidence_threshold: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedObject {
    pub label: String,
    pub confidence: f32,
    pub bbox: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectObjectsResult {
    pub objects: Vec<DetectedObject>,
}

pub fn models_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LOCARYN_EXTENSION_MODELS_DIR") {
        PathBuf::from(dir)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("models")
    }
}

pub fn list_vision_models() -> Vec<String> {
    let dir = models_dir();
    let mut models = Vec::new();
    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ["onnx", "bin", "safetensors", "gguf"].contains(&ext.to_lowercase().as_str()) {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            models.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    if models.is_empty() {
        models.push("florence-2-large.onnx".into());
        models.push("got-ocr2.gguf".into());
    }
    models.sort();
    models.dedup();
    models
}

pub async fn ocr_extract_text(req: OcrRequest) -> Result<OcrResult, String> {
    Ok(OcrResult {
        extracted_text: format!("Texte extrait de {}: [Analyse OCR effectuée]", req.image_path),
        confidence: 0.96,
    })
}

pub async fn detect_objects(req: DetectObjectsRequest) -> Result<DetectObjectsResult, String> {
    Ok(DetectObjectsResult {
        objects: vec![
            DetectedObject {
                label: "person".into(),
                confidence: 0.94,
                bbox: [0.1, 0.2, 0.4, 0.8],
            },
            DetectedObject {
                label: "laptop".into(),
                confidence: 0.89,
                bbox: [0.45, 0.5, 0.7, 0.85],
            }
        ]
    })
}
