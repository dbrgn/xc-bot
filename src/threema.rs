use anyhow::{Context, Result};
use sqlx::{Pool, Sqlite};
use threema_gateway::{E2eApi, PublicKey};

use crate::db::{cache_public_key, User};

/// Return the public key of this user. If it isn't known yet, fetch and cache it.
pub async fn get_public_key(user: &User, api: &E2eApi, pool: &Pool<Sqlite>) -> Result<PublicKey> {
    Ok(match user.threema_public_key {
        Some(pubkey) => {
            tracing::info!("Using cached public key for {}", user.username);
            pubkey
        }
        None => {
            tracing::info!(
                "No cached public key for {}, fetching from API",
                user.username
            );

            // Fetch public key from API
            let pubkey = api
                .lookup_pubkey(&user.username)
                .await
                .context("Could not look up recipient public key")?;

            // Cache public key
            let pool_clone = pool.clone();
            let user_id = user.id;
            tokio::spawn(async move {
                if let Err(e) = cache_public_key(&pool_clone, user_id, &pubkey).await {
                    tracing::error!(
                        "Could not cache public key for user with id {}: {}",
                        user_id,
                        e
                    );
                }
            });

            pubkey
        }
    })
}
