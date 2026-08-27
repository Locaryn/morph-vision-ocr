//! Locaryn Vision & OCR Plugin
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("models")
    }
}

pub fn list_vision_models() -> Vec<String> {
    let dir = models_dir();
    let mut models = Vec::new();
    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ["onnx", "bin", "safetensors", "gguf"].contains(&ext.to_lowercase().as_str())
                    {
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

/// Non implemente. La signature est conservee pour que l'interface et le
/// serveur MCP gardent leur forme, mais l'appel echoue franchement plutot
/// que de fabriquer un resultat.
pub async fn ocr_extract_text(_req: OcrRequest) -> Result<OcrResult, String> {
    Err("L'OCR n'est pas implemente : ce morph ne lit aucune image.".into())
}

/// Non implemente. La signature est conservee pour que l'interface et le
/// serveur MCP gardent leur forme, mais l'appel echoue franchement plutot
/// que de fabriquer un resultat.
pub async fn detect_objects(_req: DetectObjectsRequest) -> Result<DetectObjectsResult, String> {
    Err("La detection d'objets n'est pas implementee : ce morph ne lit aucune image.".into())
}
