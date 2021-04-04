//! Database related functions.

use anyhow::{Context, Result};
use sqlx::{sqlite::SqliteRow, FromRow, Pool, Row, Sqlite};
use threema_gateway::PublicKey;

#[derive(Debug)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub usertype: String,
    pub threema_public_key: Option<PublicKey>,
}

impl FromRow<'_, SqliteRow> for User {
    fn from_row(row: &SqliteRow) -> std::result::Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            username: row.try_get("username")?,
            usertype: row.try_get("usertype")?,
            threema_public_key: row
                .try_get::<Option<Vec<u8>>, _>("threema_public_key")?
                .and_then(|bytes: Vec<u8>| PublicKey::from_slice(&bytes)),
        })
    }
}

/// Return the specified user.
///
/// If the user does not yet exist, create it.
pub async fn get_or_create_user(
    pool: &Pool<Sqlite>,
    username: &str,
    usertype: &str,
) -> Result<User> {
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
    let user: User = sqlx::query_as("SELECT id, username, usertype, threema_public_key FROM users WHERE username = ? AND usertype = ?")
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
    Ok(user)
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

/// Add a subscription for the user with the specified user ID.
pub async fn add_subscription(pool: &Pool<Sqlite>, uid: i32, pilot: &str) -> Result<()> {
    // Get connection
    let mut conn = pool
        .acquire()
        .await
        .context("Could not acquire db connection")?;

    // Add subscription
    sqlx::query("INSERT OR IGNORE INTO subscriptions (user_id, pilot_username) VALUES (?, ?)")
        .bind(uid)
        .bind(pilot)
        .execute(&mut conn)
        .await
        .context("Could not add subscription")?;

    Ok(())
}

/// Remove a subscription for the user with the specified user ID.
///
/// Return whether a subscription was removed or not.
pub async fn remove_subscription(pool: &Pool<Sqlite>, uid: i32, pilot: &str) -> Result<bool> {
    // Start transaction
    let mut transaction = pool.begin().await.context("Could not start transaction")?;

    // Remove subscription
    sqlx::query("DELETE FROM subscriptions WHERE user_id = ? AND pilot_username = ?")
        .bind(uid)
        .bind(pilot)
        .execute(&mut transaction)
        .await
        .context("Could not remove subscription")?;

    // Get number of modified rows
    let deleted: bool = sqlx::query_scalar("SELECT changes() > 0")
        .fetch_one(&mut transaction)
        .await
        .context("Could not query number of deleted rows")?;

    // Commit transaction
    transaction
        .commit()
        .await
        .context("Could not commit transaction")?;

    Ok(deleted)
}
