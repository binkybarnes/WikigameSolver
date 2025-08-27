// src/leaderboard.rs

use deadpool_redis::Pool as RedisPool;
use sqlx::{Row, SqlitePool};

pub async fn try_add_to_leaderboard(
    sqlite_pool: &sqlx::SqlitePool,
    redis_pool: &deadpool_redis::Pool,
    leaderboard_type: &str,
    start_id: u32,
    end_id: u32,
    score: u32,
    search_id: &str,
    top_n: u32,
) -> Option<u32> {
    let claim = sqlx::query!(
        r#"
        INSERT OR IGNORE INTO claimed_paths (start_id, end_id, leaderboard, search_id)
        VALUES (?1, ?2, ?3, ?4)
        "#,
        start_id,
        end_id,
        leaderboard_type,
        search_id,
    )
    .execute(sqlite_pool)
    .await
    .unwrap();

    if claim.rows_affected() != 1 {
        return None;
    }

    let leaderboard_key = &format!("leaderboard:{}", leaderboard_type);

    let mut conn = redis_pool.get().await.unwrap();

    // 1️⃣ Check current size
    let size: u32 = conn.zcard(leaderboard_key).await.unwrap();
    let path_value = format!("{}|{}", start_id, end_id);

    if size < top_n {
        // Leaderboard not full → insert
        conn.zadd::<&str, u32, &str, ()>(leaderboard_key, &path_value, score)
            .await
            .unwrap();
        tracing::info!("Added path {} to {}", path_value, leaderboard_key);
    } else {
        // Leaderboard full → check lowest score
        let lowest: Option<(String, u32)> = conn
            .zrange_withscores::<&str, Vec<(String, u32)>>(leaderboard_key, 0, 0)
            .await
            .unwrap()
            .into_iter()
            .next();

        if let Some((_, lowest_score)) = lowest {
            if score > lowest_score {
                // New search qualifies → insert
                conn.zadd::<&str, u32, &str, ()>(leaderboard_key, &path_value, score)
                    .await
                    .unwrap();
                tracing::info!("Added path {} to {}", path_value, leaderboard_key);
                // Trim to top N
                let start = 0;
                let stop = -(top_n as isize) - 1;
                let _: () = conn
                    .zremrangebyrank(leaderboard_key, start, stop)
                    .await
                    .unwrap();
            } else {
                // Does not qualify
                return None;
            }
        }
    }

    // 2️⃣ Return the new rank (0-based)
    let rank: Option<u32> = conn.zrevrank(leaderboard_key, path_value).await.unwrap();
    rank
}

pub async fn populate_leaderboard(
    sqlite_pool: &sqlx::SqlitePool,
    redis_pool: &deadpool_redis::Pool,
    leaderboard: &str,   // "most or longest"
    metric_column: &str, // e.g., "path_length" or "num_paths"
    top_n: u32,
) -> anyhow::Result<()> {
    let mut redis_conn = redis_pool.get().await?;

    // 1️⃣ Clear the leaderboard
    let _: () = redis_conn.del(leaderboard).await?;

    // 2️⃣ Fetch top N searches from SQLite
    let query = format!(
        "SELECT cp.start_id, cp.end_id, cp.search_id, s.{}
        FROM claimed_paths cp
        JOIN searches s ON s.id = cp.search_id
        WHERE cp.leaderboard = ?
        ORDER BY s.{} DESC
        LIMIT ?;
        ",
        metric_column, metric_column
    );

    let rows = sqlx::query(&query)
        .bind(leaderboard)
        .bind(top_n)
        .fetch_all(sqlite_pool)
        .await?;

    // 3️⃣ Insert into Redis leaderboard
    let leaderboard_key = format!("leaderboard:{}", leaderboard);
    for row in rows {
        let start_id: u32 = row.get("start_id");
        let end_id: u32 = row.get("end_id");
        let score: u32 = row.try_get(metric_column)?; // path_length or num_paths

        let path_value = format!("{}|{}", start_id, end_id);

        let _: () = redis_conn.zadd(&leaderboard_key, path_value, score).await?;
    }

    Ok(())
}
