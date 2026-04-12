use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserCache {
    pub user_id: String,
    pub username: String,
}

const CACHE_FILE: &str = "user_cache.json";

pub fn load_user_cache(username: &str) -> Option<UserCache> {
    let data = fs::read_to_string(CACHE_FILE).ok()?;
    let cache: UserCache = serde_json::from_str(&data).ok()?;

    if cache.username == username {
        Some(cache)
    } else {
        None
    }
}

pub fn save_user_cache(cache: &UserCache) -> Result<(), ()> {
    let data = serde_json::to_string_pretty(cache).map_err(|_| ())?;
    let _ = fs::write(CACHE_FILE, data);
    Ok(())
}
