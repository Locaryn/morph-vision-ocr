//! Stdio MCP server shipped by morph-vision-ocr.
use locaryn_plugin_vision_ocr::{detect_objects, list_vision_models, ocr_extract_text, DetectObjectsRequest, OcrRequest};
use serde_json::{json, Value};
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};

const VERSION: &str = "1.1.0";

#[tokio::main]
async fn main() {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() { continue; }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle_request(request).await,
            Err(error) => error_response(Value::Null, -32700, format!("JSON invalide : {error}")),
        };
        if let Ok(serialized) = serde_json::to_string(&response) {
            println!("{serialized}");
            let _ = std::io::stdout().flush();
        }
    }
}

async fn handle_request(request: Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
    match method {
        "initialize" => success(id, json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "morph-vision-ocr", "version": VERSION }
        })),
        "tools/list" => success(id, tools_list()),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            match call_tool(name, args).await {
                Ok(value) => success(id, text_content(value)),
                Err(error) => error_response(id, -32000, error),
            }
        }
        notification if notification.starts_with("notifications/") => Value::Null,
        _ => error_response(id, -32601, format!("méthode MCP inconnue : {method}")),
    }
}

fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "list_vision_models",
                "description": "Liste les modèles de vision et OCR disponibles localement.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "ocr_extract_text",
                "description": "Extrait tout le texte d'un document, scan ou image.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "image_path": { "type": "string", "description": "Chemin ou URL de l'image" },
                        "language": { "type": "string", "description": "Langue préférentielle" }
                    },
                    "required": ["image_path"]
                }
            },
            {
                "name": "detect_objects",
                "description": "Détecte les objets, personnes et éléments avec leurs boîtes englobantes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "image_path": { "type": "string", "description": "Chemin de l'image" },
                        "confidence_threshold": { "type": "number", "description": "Seuil de confiance minimum (0.0 à 1.0)" }
                    },
                    "required": ["image_path"]
                }
            }
        ]
    })
}

async fn call_tool(name: &str, args: Value) -> Result<Value, String> {
    match name {
        "list_vision_models" => Ok(json!({ "models": list_vision_models() })),
        "ocr_extract_text" => {
            let req: OcrRequest = serde_json::from_value(args)
                .map_err(|e| format!("Paramètres OCR invalides: {e}"))?;
            let res = ocr_extract_text(req).await?;
            Ok(json!(res))
        }
        "detect_objects" => {
            let req: DetectObjectsRequest = serde_json::from_value(args)
                .map_err(|e| format!("Paramètres détection invalides: {e}"))?;
            let res = detect_objects(req).await?;
            Ok(json!(res))
        }
        _ => Err(format!("Outil vision inconnu : {name}")),
    }
}

fn text_content(value: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()) }] })
}
fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
