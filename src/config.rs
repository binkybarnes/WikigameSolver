// src/config.rs

use anyhow::bail;
use dotenv::dotenv;
use std::env;

#[derive(Clone, Debug)]
pub struct EnvironmentVariables {
    pub jwt_secret: String,
    pub database_url: String,
    pub leaderboard_limit: u32,
    pub google_client_id: String,
    pub is_production: bool,
}

impl EnvironmentVariables {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenv().ok();

        let jwt_secret = match env::var("JWT_SECRET") {
            Ok(val) => val,
            Err(_) => bail!("Missing JWT_SECRET"),
        };

        let database_url = match env::var("DATABASE_URL") {
            Ok(val) => val,
            Err(_) => bail!("Missing DATABASE_URL"),
        };

        let leaderboard_limit = match env::var("LEADERBOARD_LIMIT") {
            Ok(val) => val.parse::<u32>()?,
            Err(_) => bail!("Missing LEADERBOARD_LIMIT"),
        };

        let google_client_id = match env::var("GOOGLE_CLIENT_ID") {
            Ok(val) => val,
            Err(_) => bail!("Missing GOOGLE_CLIENT_ID"),
        };

        let is_production = match env::var("IS_PRODUCTION") {
            Ok(val) => val == "1" || val.to_lowercase() == "true",
            Err(_) => false,
        };

        Ok(Self {
            jwt_secret,
            database_url,
            leaderboard_limit,
            google_client_id,
            is_production,
        })
    }
}
