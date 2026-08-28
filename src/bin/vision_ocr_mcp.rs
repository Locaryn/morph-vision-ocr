//! Stdio MCP server shipped by morph-vision-ocr.
use locaryn_plugin_vision_ocr::{
    describe_image, list_vision_models, ocr_extract_text, ocr_languages, tesseract_present,
    DescribeImageRequest, OcrRequest,
};
use serde_json::{json, Value};
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
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
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match method {
        "initialize" => success(
            id,
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "morph-vision-ocr", "version": VERSION }
            }),
        ),
        "tools/list" => success(id, tools_list()),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
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
                "description": "Les modèles de vision installés sur cette machine, et si tesseract est présent pour l'OCR.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "ocr_extract_text",
                "description": "Extrait le texte d'une image ou d'un scan, par tesseract, sur cette machine. Rend le texte, le nombre de mots et la confiance moyenne que tesseract leur accorde — une mesure, pas une estimation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "image_path": { "type": "string", "description": "Chemin de l'image sur cette machine" },
                        "language": { "type": "string", "description": "Langues au format tesseract : fra, eng, ou fra+eng. Omis : le réglage du morph." }
                    },
                    "required": ["image_path"]
                }
            },
            {
                "name": "describe_image",
                "description": "Décrit une image, ou répond à une question à son sujet, par un modèle de vision servi localement. Ne rend AUCUNE boîte englobante : un modèle conversationnel sait nommer ce qu'il voit, pas dire où au pixel près.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "image_path": { "type": "string", "description": "Chemin de l'image sur cette machine" },
                        "question": { "type": "string", "description": "Ce qu'on veut savoir. Omis : une description libre." },
                        "model": { "type": "string", "description": "Modèle de vision, s'il doit différer du réglage." }
                    },
                    "required": ["image_path"]
                }
            }
        ]
    })
}

async fn call_tool(name: &str, args: Value) -> Result<Value, String> {
    match name {
        "list_vision_models" => {
            // L'OCR ne dépend pas d'un modèle mais d'un outil externe : le
            // dire ici évite un aller-retour pour l'apprendre par un échec.
            let langues = if tesseract_present() {
                ocr_languages().await.unwrap_or_default()
            } else {
                Vec::new()
            };
            Ok(json!({
                "vision_models": list_vision_models(),
                "tesseract_available": tesseract_present(),
                "ocr_languages": langues,
            }))
        }
        "ocr_extract_text" => {
            let req: OcrRequest = serde_json::from_value(args)
                .map_err(|e| format!("Paramètres OCR invalides: {e}"))?;
            let res = ocr_extract_text(req).await?;
            Ok(json!(res))
        }
        "describe_image" => {
            let req: DescribeImageRequest =
                serde_json::from_value(args).map_err(|e| format!("Paramètres invalides : {e}"))?;
            Ok(json!(describe_image(req).await?))
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
