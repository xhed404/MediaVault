use crate::config::Config;
use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
}
