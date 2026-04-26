use crate::config::Config;
use crate::error::AppError;
use crate::util::now_rfc3339;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn connect(config: &Config) -> Result<Self, AppError> {
        let mut opts = SqliteConnectOptions::from_str(&config.database_url)
            .map_err(|_| AppError::config("DATABASE_URL invalid"))?;
        opts = opts
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(10));

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(opts)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn bootstrap(&self, config: &Config) -> Result<(), AppError> {
        let email = match (
            &config.bootstrap_admin_email,
            &config.bootstrap_admin_password,
        ) {
            (Some(e), Some(_)) => e.trim(),
            _ => return Ok(()),
        };

        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM users WHERE email = ?")
            .bind(email)
            .fetch_one(&self.pool)
            .await?;
        if exists > 0 {
            return Ok(());
        }

        let password = config
            .bootstrap_admin_password
            .as_deref()
            .unwrap_or_default()
            .to_string();

        let hash = crate::auth::hash_password(&password)?;
        let now = now_rfc3339();
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO users (id, email, password_hash, role, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(email)
        .bind(hash)
        .bind("admin")
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
