//! Database related functions.

use anyhow::{Context, Result};
use sqlx::{Pool, Sqlite};

/// Return the user ID of the specified user.
///
/// If the user does not yet exist, create it.
pub async fn get_or_create_user(
    pool: &Pool<Sqlite>,
    username: &str,
    usertype: &str,
) -> Result<i32> {
    // Start transaction
    let mut transaction = pool.begin().await.context("Could not start transaction")?;

    // Ensure user exists
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO users (username, usertype, since)
        VALUES (?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(username)
    .bind(usertype)
    .execute(&mut transaction)
    .await
    .context(format!("Could not create user {}/{}", usertype, username))?;

    // Fetch user
    let uid: i32 = sqlx::query_scalar("SELECT id FROM users WHERE username = ? AND usertype = ?")
        .bind(username)
        .bind(usertype)
        .fetch_one(&mut transaction)
        .await
        .context(format!("Could not fetch user {}/{}", usertype, username))?;

    // Commit transaction
    transaction
        .commit()
        .await
        .context("Could not commit transaction")?;
    Ok(uid)
}

/// Return the subscriptions of the user with the specified user ID, sorted by name.
pub async fn get_subscriptions(pool: &Pool<Sqlite>, uid: i32) -> Result<Vec<String>> {
    // Get connection
    let mut conn = pool
        .acquire()
        .await
        .context("Could not acquire db connection")?;

    // Fetch subscriptions
    let subscriptions =
        sqlx::query_scalar("SELECT pilot_username FROM subscriptions WHERE user_id = ? ORDER BY pilot_username COLLATE NOCASE ASC")
            .bind(uid)
            .fetch_all(&mut conn)
            .await
            .context("Could not fetch subscriptions")?;

    Ok(subscriptions)
}
