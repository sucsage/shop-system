use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

pub async fn init_db() -> SqlitePool {
    let opt = SqliteConnectOptions::from_str("sqlite:../databases/scr/shop-system.db")
        .unwrap()
        .create_if_missing(true);

    SqlitePool::connect_with(opt).await.unwrap()
}
