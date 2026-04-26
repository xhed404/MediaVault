use crate::error::AppError;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub storage_root: PathBuf,
    pub public_base_url: String,
    pub session_secret: String,
    pub cookie_secure: bool,
    pub max_upload_bytes: u64,
    pub web_dist: Option<PathBuf>,
    pub bootstrap_admin_email: Option<String>,
    pub bootstrap_admin_password: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        let bind_addr = get_env("BIND_ADDR").unwrap_or_else(|| "127.0.0.1:8080".to_string());
        let database_url =
            get_env("DATABASE_URL").unwrap_or_else(|| "sqlite:./app.sqlite".to_string());
        let storage_root =
            PathBuf::from(get_env("STORAGE_ROOT").unwrap_or_else(|| "./data".to_string()));
        let public_base_url =
            get_env("PUBLIC_BASE_URL").unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
        let session_secret =
            get_env("SESSION_SECRET").ok_or_else(|| AppError::config("SESSION_SECRET required"))?;

        if session_secret.trim().len() < 32 {
            return Err(AppError::config("SESSION_SECRET must be at least 32 chars"));
        }

        let cookie_secure = match get_env("COOKIE_SECURE") {
            Some(v) => parse_bool(&v).ok_or_else(|| AppError::config("COOKIE_SECURE invalid"))?,
            None => public_base_url.starts_with("https://"),
        };

        let max_upload_bytes = match get_env("MAX_UPLOAD_BYTES") {
            Some(v) => v
                .parse::<u64>()
                .map_err(|_| AppError::config("MAX_UPLOAD_BYTES invalid"))?,
            None => 100 * 1024 * 1024,
        };

        let web_dist = get_env("WEB_DIST").map(PathBuf::from);
        let bootstrap_admin_email = get_env("BOOTSTRAP_ADMIN_EMAIL");
        let bootstrap_admin_password = get_env("BOOTSTRAP_ADMIN_PASSWORD");

        Ok(Self {
            bind_addr,
            database_url,
            storage_root,
            public_base_url,
            session_secret,
            cookie_secure,
            max_upload_bytes,
            web_dist,
            bootstrap_admin_email,
            bootstrap_admin_password,
        })
    }

    pub fn storage_objects_dir(&self) -> PathBuf {
        self.storage_root.join("objects")
    }

    pub fn storage_tmp_dir(&self) -> PathBuf {
        self.storage_root.join("tmp")
    }

    pub fn object_rel_path(&self, sha256_hex: &str) -> String {
        let (a, b) = sha256_hex.split_at(2);
        format!("objects/{a}/{b}/{sha256_hex}")
    }

    pub fn storage_object_full_path(&self, rel: &str) -> PathBuf {
        self.storage_root.join(rel)
    }

    pub fn validate_rel_path(rel: &str) -> Result<(), AppError> {
        let p = Path::new(rel);
        if rel.is_empty()
            || rel.starts_with('/')
            || rel.contains('\\')
            || rel.contains("..")
            || p.is_absolute()
        {
            return Err(AppError::bad_request("invalid path"));
        }
        Ok(())
    }
}

fn get_env(key: &str) -> Option<String> {
    std::env::var(key).ok().map(|s| s.trim().to_string())
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" => Some(true),
        "0" | "false" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}
