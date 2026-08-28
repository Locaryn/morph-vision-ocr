//! Lire une image : le texte qu'elle porte, et ce qu'elle montre.
//!
//! Deux chemins distincts, parce que ce sont deux problèmes distincts :
//!
//! * **L'OCR** passe par `tesseract`, comme le fait déjà le socle pour lire
//!   l'écran d'un téléphone. Il n'est pas livré avec le morph — embarquer le
//!   binaire d'un tiers, c'est embarquer ses mises à jour et ses failles ; il
//!   est détecté, et s'il manque on dit comment l'obtenir.
//! * **La description** passe par le moteur d'inférence local, au format
//!   vision d'OpenAI, avec un modèle qui sait regarder une image.
//!
//! Ce morph ne rend **aucune boîte englobante**. Un modèle conversationnel qui
//! regarde une image sait nommer ce qu'il voit ; il ne sait pas dire où, à la
//! dizaine de pixels près. Rendre des coordonnées obligerait à les inventer —
//! c'est précisément ce que faisait la version d'avant.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Réglages ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Adresse du serveur compatible OpenAI, pour la description d'image.
    #[serde(default = "endpoint_par_defaut")]
    pub endpoint: String,
    /// Modèle capable de regarder une image. Un modèle de texte seul refusera
    /// ou décrira n'importe quoi.
    #[serde(default)]
    pub vision_model: String,
    /// Langues passées à tesseract, au format qu'il attend : `fra`, `eng`, ou
    /// `fra+eng`.
    #[serde(default = "langues_par_defaut")]
    pub ocr_languages: String,
}

fn endpoint_par_defaut() -> String {
    "http://127.0.0.1:8080".into()
}
fn langues_par_defaut() -> String {
    "fra+eng".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: endpoint_par_defaut(),
            vision_model: String::new(),
            ocr_languages: langues_par_defaut(),
        }
    }
}

pub fn config() -> Config {
    let Some(p) = std::env::var("LOCARYN_EXTENSION_CONFIG_FILE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
    else {
        return Config::default();
    };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ── Inventaire ──────────────────────────────────────────────────────────────

pub fn models_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for key in ["LOCARYN_MODELS_DIR", "LOCARYN_EXTENSION_MODELS_DIR"] {
        if let Ok(dir) = std::env::var(key) {
            if !dir.trim().is_empty() {
                out.push(PathBuf::from(dir));
            }
        }
    }
    out.push(
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("models"),
    );
    out
}

pub fn models_dir() -> PathBuf {
    models_dirs()
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("models"))
}

/// Les modèles installés qui savent regarder une image.
///
/// Reconnus à leur nom : c'est ce dont on dispose, et les familles concernées
/// le portent lisiblement. La liste est vide quand aucun n'est là — annoncer
/// des modèles absents ne ferait que déplacer l'échec.
pub fn list_vision_models() -> Vec<String> {
    const INDICES: &[&str] = &[
        "vl",
        "vision",
        "llava",
        "moondream",
        "florence",
        "got-ocr",
        "qwen-vl",
        "minicpm-v",
        "internvl",
        "pixtral",
        "mmproj",
    ];
    let mut out: Vec<String> = Vec::new();
    for dir in models_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let nom = entry.file_name().to_string_lossy().to_string();
            let bas = nom.to_ascii_lowercase();
            if INDICES.iter().any(|k| bas.contains(k)) {
                out.push(nom);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

// ── OCR ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    pub image_path: String,
    /// Langues au format tesseract (`fra`, `eng`, `fra+eng`). Absent : le
    /// réglage du morph.
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub extracted_text: String,
    /// Moyenne des confiances que tesseract donne mot à mot, entre 0 et 1.
    /// C'est une mesure, pas une estimation : les mots dont il n'est pas sûr
    /// tirent le chiffre vers le bas.
    pub confidence: f32,
    pub words: usize,
    pub languages: String,
}

/// Extraire le texte d'une image.
pub async fn ocr_extract_text(req: OcrRequest) -> Result<OcrResult, String> {
    let chemin = PathBuf::from(&req.image_path);
    if !chemin.is_file() {
        return Err(format!("Image introuvable : {}", chemin.display()));
    }
    if !tesseract_present() {
        return Err(TESSERACT_ABSENT.into());
    }
    let cfg = config();
    let langues = req
        .language
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or(&cfg.ocr_languages)
        .to_string();

    // Le format TSV porte une confiance par mot ; le mode texte n'en donne
    // aucune, et il faudrait alors en inventer une.
    let sortie = tokio::process::Command::new(resolve_program("tesseract"))
        .arg(chemin.as_os_str())
        .arg("stdout")
        .arg("-l")
        .arg(&langues)
        .arg("tsv")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Impossible de lancer tesseract : {e}"))?;

    if !sortie.status.success() {
        let err = String::from_utf8_lossy(&sortie.stderr);
        let fin: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).collect();
        return Err(format!(
            "tesseract a échoué : {}",
            fin.iter()
                .rev()
                .take(2)
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }

    let (texte, confiance, mots) = lire_tsv(&String::from_utf8_lossy(&sortie.stdout));
    Ok(OcrResult {
        extracted_text: texte,
        confidence: confiance,
        words: mots,
        languages: langues,
    })
}

/// Lire la sortie TSV de tesseract : les mots, et leur confiance.
///
/// Les lignes de confiance `-1` sont des blocs de structure, pas des mots ;
/// les compter écraserait la moyenne.
fn lire_tsv(tsv: &str) -> (String, f32, usize) {
    let mut mots: Vec<String> = Vec::new();
    let mut total = 0.0f32;
    let mut n = 0usize;
    for ligne in tsv.lines().skip(1) {
        let cols: Vec<&str> = ligne.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }
        let conf: f32 = cols[10].trim().parse().unwrap_or(-1.0);
        let mot = cols[11].trim();
        if conf < 0.0 || mot.is_empty() {
            continue;
        }
        mots.push(mot.to_string());
        total += conf;
        n += 1;
    }
    let confiance = if n == 0 {
        0.0
    } else {
        (total / n as f32) / 100.0
    };
    (mots.join(" "), confiance, n)
}

const TESSERACT_ABSENT: &str = "tesseract n'est pas installé. \
     Installez-le (winget install UB-Mannheim.TesseractOCR, brew install tesseract, \
     ou apt install tesseract-ocr tesseract-ocr-fra), puis réessayez.";

pub fn tesseract_present() -> bool {
    program_exists("tesseract")
}

/// Les langues que tesseract sait lire sur cette machine.
pub async fn ocr_languages() -> Result<Vec<String>, String> {
    if !tesseract_present() {
        return Err(TESSERACT_ABSENT.into());
    }
    let out = tokio::process::Command::new(resolve_program("tesseract"))
        .arg("--list-langs")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Impossible de lancer tesseract : {e}"))?;
    // La première ligne annonce le décompte ; les suivantes sont les langues.
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

// ── Description d'image ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeImageRequest {
    pub image_path: String,
    /// Ce qu'on veut savoir de l'image. Absent : une description libre.
    #[serde(default)]
    pub question: Option<String>,
    /// Modèle de vision, s'il doit différer du réglage.
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeImageResult {
    pub description: String,
    pub model: String,
}

/// Décrire une image, ou répondre à une question à son sujet.
pub async fn describe_image(req: DescribeImageRequest) -> Result<DescribeImageResult, String> {
    let chemin = PathBuf::from(&req.image_path);
    if !chemin.is_file() {
        return Err(format!("Image introuvable : {}", chemin.display()));
    }
    let cfg = config();
    let modele = req
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .or_else(|| {
            Some(cfg.vision_model.trim())
                .filter(|m| !m.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| {
            let installes = list_vision_models();
            if installes.is_empty() {
                "Aucun modèle de vision n'est réglé ni installé. Ajoutez-en un \
                 (Qwen-VL, MiniCPM-V, LLaVA…) et nommez-le dans les réglages du morph."
                    .to_string()
            } else {
                format!(
                    "Aucun modèle de vision n'est réglé. Installés ici : {}.",
                    installes.join(", ")
                )
            }
        })?;

    let octets = std::fs::read(&chemin)
        .map_err(|e| format!("Lecture de {} impossible : {e}", chemin.display()))?;
    let mime = mime_de(&chemin);
    let url = format!("data:{mime};base64,{}", base64(&octets));

    let question = req
        .question
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .unwrap_or("Décris cette image. Nomme ce qui s'y trouve, sans inventer.");

    // Le format vision d'OpenAI : le contenu devient un tableau de parties.
    let corps = serde_json::json!({
        "model": modele,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": question },
                { "type": "image_url", "image_url": { "url": url } }
            ]
        }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/v1/chat/completions",
            cfg.endpoint.trim_end_matches('/')
        ))
        .timeout(std::time::Duration::from_secs(300))
        .json(&corps)
        .send()
        .await
        .map_err(|_| {
            "Le moteur d'inférence ne répond pas. Démarrez-le, puis réessayez.".to_string()
        })?;

    if !resp.status().is_success() {
        let statut = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Le moteur a refusé la demande ({statut}){}. Un modèle de texte seul ne sait \
             pas regarder une image : il faut un modèle de vision.",
            if detail.trim().is_empty() {
                String::new()
            } else {
                format!(" : {}", tronquer(&detail, 200))
            }
        ));
    }

    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("réponse illisible du moteur : {e}"))?;
    let description = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("Le moteur n'a rien répondu.")?
        .to_string();

    Ok(DescribeImageResult {
        description,
        model: modele,
    })
}

// ── Utilitaires ─────────────────────────────────────────────────────────────

fn tronquer(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= n {
        return t.to_string();
    }
    t.chars().take(n).collect::<String>() + "…"
}

fn mime_de(p: &Path) -> &'static str {
    match p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    }
}

/// Encodage base64, écrit ici pour ne pas ajouter une dépendance à un morph
/// qui n'en a besoin qu'une fois.
fn base64(octets: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(octets.len().div_ceil(3) * 4);
    for bloc in octets.chunks(3) {
        let b = [
            bloc[0],
            *bloc.get(1).unwrap_or(&0),
            *bloc.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if bloc.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if bloc.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Trouver l'exécutable. Sous Windows un outil installé par winget s'appelle
/// souvent `x.exe`, et `CreateProcess` ne devine pas l'extension.
fn resolve_program(command: &str) -> std::ffi::OsString {
    #[cfg(not(windows))]
    {
        std::ffi::OsString::from(command)
    }
    #[cfg(windows)]
    {
        const EXTS: [&str; 4] = [".exe", ".cmd", ".bat", ".com"];
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                for ext in EXTS {
                    let c = dir.join(format!("{command}{ext}"));
                    if c.is_file() {
                        return c.into_os_string();
                    }
                }
            }
        }
        std::ffi::OsString::from(command)
    }
}

fn program_exists(command: &str) -> bool {
    let resolved = resolve_program(command);
    if std::path::Path::new(&resolved).is_file() {
        return true;
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join(command).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La confiance vient de tesseract, mot à mot. Les lignes à `-1` sont des
    /// blocs de structure : les compter écraserait la moyenne.
    #[test]
    fn le_tsv_donne_les_mots_et_leur_confiance() {
        let tsv = "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t100\t100\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t10\t20\t10\t90\tBonjour\n\
                   5\t1\t1\t1\t1\t2\t40\t10\t20\t10\t70\tmonde\n";
        let (texte, conf, n) = lire_tsv(tsv);
        assert_eq!(texte, "Bonjour monde");
        assert_eq!(n, 2);
        assert!((conf - 0.80).abs() < 1e-4, "confiance = {conf}");
    }

    #[test]
    fn un_tsv_sans_mot_ne_pretend_a_aucune_confiance() {
        let (texte, conf, n) = lire_tsv("level\tconf\ttext\n");
        assert!(texte.is_empty());
        assert_eq!(conf, 0.0);
        assert_eq!(n, 0);
    }

    #[test]
    fn le_base64_suit_la_norme() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn le_type_suit_l_extension() {
        assert_eq!(mime_de(Path::new("a.png")), "image/png");
        assert_eq!(mime_de(Path::new("a.WEBP")), "image/webp");
        assert_eq!(mime_de(Path::new("a.jpg")), "image/jpeg");
        assert_eq!(mime_de(Path::new("a.inconnu")), "image/jpeg");
    }

    #[test]
    fn une_image_absente_est_refusee_sans_lancer_tesseract() {
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(ocr_extract_text(OcrRequest {
                image_path: "n-existe-pas.png".into(),
                language: None,
            }))
            .unwrap_err();
        assert!(err.contains("introuvable"), "{err}");
    }

    /// Sans modèle de vision réglé, l'appel doit le dire et proposer une suite,
    /// pas décrire une image qu'il n'a pas regardée.
    #[test]
    fn sans_modele_de_vision_l_appel_le_dit() {
        std::env::remove_var("LOCARYN_EXTENSION_CONFIG_FILE");
        let image = std::env::temp_dir().join("morph-vision-test.png");
        std::fs::write(&image, b"pas vraiment une image").unwrap();
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(describe_image(DescribeImageRequest {
                image_path: image.to_string_lossy().to_string(),
                question: None,
                model: None,
            }))
            .unwrap_err();
        assert!(err.contains("modèle de vision"), "{err}");
        let _ = std::fs::remove_file(&image);
    }
}
