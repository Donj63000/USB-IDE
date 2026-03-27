use std::fs;
use std::io::{self, BufRead, Read};
use std::path::Path;

use anyhow::{Context, Result};
use encoding_rs::Encoding;
use regex::Regex;

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

fn pep263_encoding_line(line: &str) -> Option<String> {
    // La regex suit PEP 263 : "coding[:=] <encoding>".
    let re = Regex::new(r"(?i)coding[:=]\s*([-\w.]+)").ok()?;
    re.captures(line)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

fn detect_python_encoding(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut reader = io::BufReader::new(file);
    let mut buf = Vec::new();
    for _ in 0..2 {
        buf.clear();
        let read = reader.read_until(b'\n', &mut buf).ok()?;
        if read == 0 {
            break;
        }
        let line = String::from_utf8_lossy(&buf);
        if let Some(enc) = pep263_encoding_line(&line) {
            return Some(enc);
        }
    }
    None
}

fn decode_with_encoding(bytes: &[u8], encoding: &str) -> Option<(String, bool)> {
    let encoding_lower = encoding.trim().to_lowercase();
    if encoding_lower == "utf-8-sig" {
        let stripped = if bytes.starts_with(UTF8_BOM) {
            &bytes[UTF8_BOM.len()..]
        } else {
            bytes
        };
        match String::from_utf8(stripped.to_vec()) {
            Ok(text) => return Some((text, false)),
            Err(err) => {
                return Some((String::from_utf8_lossy(&err.into_bytes()).to_string(), true));
            }
        }
    }

    if encoding_lower == "utf-8" {
        match String::from_utf8(bytes.to_vec()) {
            Ok(text) => return Some((text, false)),
            Err(err) => {
                return Some((String::from_utf8_lossy(&err.into_bytes()).to_string(), true));
            }
        }
    }

    let enc = Encoding::for_label(encoding_lower.as_bytes())?;
    let (cow, _, had_errors) = enc.decode(bytes);
    Some((cow.into_owned(), had_errors))
}

/// Détecte un encodage raisonnable pour un fichier (PEP 263 pour .py).
pub fn detect_text_encoding(path: &Path) -> String {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.eq_ignore_ascii_case("py"))
        .unwrap_or(false)
    {
        if let Some(enc) = detect_python_encoding(path) {
            return enc;
        }
        return "utf-8".to_string();
    }

    let bytes = match fs::read(path) {
        Ok(data) => data,
        Err(_) => return "utf-8".to_string(),
    };

    for enc in ["utf-8", "utf-8-sig", "windows-1252", "latin-1"] {
        if let Some((_, had_errors)) = decode_with_encoding(&bytes, enc)
            && !had_errors
        {
            return enc.to_string();
        }
    }

    "utf-8".to_string()
}

/// Lit un fichier texte avec un encodage donné (fallback lossy en cas d'erreur).
pub fn read_text_with_encoding(path: &Path, encoding: &str) -> io::Result<String> {
    let bytes = fs::read(path)?;
    if let Some((text, _)) = decode_with_encoding(&bytes, encoding) {
        return Ok(text);
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

/// Ecrit un fichier texte avec l'encodage demande. En fallback, ecrit en UTF-8.
pub fn write_text_with_encoding(path: &Path, encoding: &str, content: &str) -> Result<bool> {
    let encoding_lower = encoding.to_lowercase();
    if encoding_lower == "utf-8" {
        fs::write(path, content.as_bytes()).context("ecriture fichier")?;
        return Ok(false);
    }
    if encoding_lower == "utf-8-sig" {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(content.as_bytes());
        fs::write(path, data).context("ecriture fichier")?;
        return Ok(false);
    }
    if let Some(enc) = Encoding::for_label(encoding_lower.as_bytes()) {
        let (cow, _, had_errors) = enc.encode(content);
        if had_errors {
            fs::write(path, content.as_bytes()).context("ecriture fallback utf-8")?;
            return Ok(true);
        }
        fs::write(path, cow.as_ref()).context("ecriture fichier")?;
        return Ok(false);
    }
    fs::write(path, content.as_bytes()).context("ecriture fallback utf-8")?;
    Ok(true)
}

/// Heuristique simple pour éviter d'ouvrir des binaires dans l'éditeur.
pub fn is_probably_binary(path: &Path, sniff_bytes: usize) -> io::Result<bool> {
    if sniff_bytes == 0 {
        return Ok(false);
    }
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; sniff_bytes];
    let read = file.read(&mut buf)?;
    buf.truncate(read);

    if buf.contains(&0) {
        return Ok(true);
    }
    if buf.is_empty() {
        return Ok(false);
    }

    let mut ctrl = 0usize;
    for &b in &buf {
        if b == 9 || b == 10 || b == 13 {
            continue;
        }
        if b < 32 || b == 127 {
            ctrl += 1;
        }
    }
    Ok((ctrl as f32 / buf.len() as f32) > 0.10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detecte_fichier_texte() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("texte.txt");
        fs::write(&path, "Bonjour\nCeci est un test.\n").unwrap();
        assert!(!is_probably_binary(&path, 2048).unwrap());
    }

    #[test]
    fn detecte_fichier_binaire() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("data.bin");
        fs::write(&path, b"\x00\x01\x02texte").unwrap();
        assert!(is_probably_binary(&path, 2048).unwrap());
    }

    #[test]
    fn taille_invalide_ne_bloque_pas() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("texte.txt");
        fs::write(&path, "abc").unwrap();
        assert!(!is_probably_binary(&path, 0).unwrap());
    }

    #[test]
    fn acces_impossible_declenche_erreur() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("absent.bin");
        assert!(is_probably_binary(&path, 2048).is_err());
    }

    #[test]
    fn fallback_py_inaccessible() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("absent.py");
        assert_eq!(detect_text_encoding(&path), "utf-8");
    }

    #[test]
    fn fallback_txt_inaccessible() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("absent.txt");
        assert_eq!(detect_text_encoding(&path), "utf-8");
    }

    #[test]
    fn ecrit_utf8_sig() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bom.txt");
        let fallback = write_text_with_encoding(&path, "utf-8-sig", "Bonjour").unwrap();
        let bytes = fs::read(&path).unwrap();

        assert!(!fallback);
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    }

    #[test]
    fn ecrit_windows_1252() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("latin.txt");
        let fallback = write_text_with_encoding(&path, "windows-1252", "école").unwrap();
        let content = read_text_with_encoding(&path, "windows-1252").unwrap();

        assert!(!fallback);
        assert_eq!(content, "école");
    }

    #[test]
    fn fallback_utf8_si_caractere_non_supporte() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fallback.txt");
        let fallback = write_text_with_encoding(&path, "windows-1252", "Bonjour 🚀").unwrap();
        let bytes = fs::read(&path).unwrap();

        assert!(fallback);
        assert_eq!(String::from_utf8(bytes).unwrap(), "Bonjour 🚀");
    }
}
