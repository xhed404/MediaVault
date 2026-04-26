use crate::error::AppError;
use rand::RngCore;
use sha2::Digest;
use time::OffsetDateTime;

pub fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

pub fn now_rfc3339() -> String {
    now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = sha2::Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub fn sha256_hex_str(s: &str) -> String {
    sha256_hex(s.as_bytes())
}

pub fn random_token_hex(len_bytes: usize) -> String {
    let mut b = vec![0u8; len_bytes];
    rand::rng().fill_bytes(&mut b);
    hex::encode(b)
}

pub fn validate_email(email: &str) -> Result<(), AppError> {
    let e = email.trim();
    if e.len() < 3 || e.len() > 254 || !e.contains('@') {
        return Err(AppError::bad_request("invalid email"));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 10 || password.len() > 200 {
        return Err(AppError::bad_request("password too short"));
    }
    Ok(())
}

pub fn safe_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len().min(180));
    for ch in name.chars().take(180) {
        if ch == '"' || ch == '\\' || ch == '\n' || ch == '\r' {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "file".to_string()
    } else {
        out
    }
}
