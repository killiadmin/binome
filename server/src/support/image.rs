//! Images stockées en data URI base64 (comme sous Laravel) : contrôle des JPEG
//! envoyés par le navigateur (avatars, emojis), encodage des fichiers importés,
//! décodage pour les servir. On ne lit que l'en-tête de l'image (`imagesize`),
//! jamais de décodage complet.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use imagesize::ImageType;

pub const JPEG_PREFIX: &str = "data:image/jpeg;base64,";

/// Type MIME d'une image reconnue (jpeg, png, gif, webp), d'après son contenu.
pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    match imagesize::image_type(bytes).ok()? {
        ImageType::Jpeg => Some("image/jpeg"),
        ImageType::Png => Some("image/png"),
        ImageType::Gif => Some("image/gif"),
        ImageType::Webp => Some("image/webp"),
        _ => None,
    }
}

/// Fichier importé → data URI (type déduit du contenu, comme `getMimeType()`).
pub fn to_data_uri(bytes: &[u8]) -> Option<String> {
    let mime = sniff_mime(bytes)?;
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

/// Contrôle d'un JPEG carré en data URI : (octets, côté) ou message d'erreur.
pub fn check_square_jpeg(data_uri: &str, max_bytes: usize, min_side: usize, max_side: usize, errors: (&str, &str)) -> Result<Vec<u8>, String> {
    let (too_big_or_not_jpeg, bad_size) = errors;
    let bytes = data_uri
        .strip_prefix(JPEG_PREFIX)
        .and_then(|b64| STANDARD.decode(b64).ok())
        .ok_or_else(|| too_big_or_not_jpeg.to_string())?;
    if bytes.len() > max_bytes || sniff_mime(&bytes) != Some("image/jpeg") {
        return Err(too_big_or_not_jpeg.to_string());
    }
    let size = imagesize::blob_size(&bytes).map_err(|_| too_big_or_not_jpeg.to_string())?;
    if size.width != size.height || size.width < min_side || size.width > max_side {
        return Err(bad_size.to_string());
    }
    Ok(bytes)
}

/// Data URI → (type MIME, octets), seulement pour une image reconnue.
pub fn decode_data_uri(data_uri: &str) -> Option<(String, Vec<u8>)> {
    let rest = data_uri.strip_prefix("data:")?;
    let (mime, b64) = rest.split_once(";base64,")?;
    if !matches!(mime, "image/jpeg" | "image/png" | "image/gif" | "image/webp") {
        return None;
    }
    Some((mime.to_string(), STANDARD.decode(b64).ok()?))
}

/// Re-encode des octets JPEG en data URI.
pub fn jpeg_data_uri(bytes: &[u8]) -> String {
    format!("{JPEG_PREFIX}{}", STANDARD.encode(bytes))
}
