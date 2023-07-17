use anyhow::{Context, Result};
use futures::TryStreamExt;
use reqwest::Client;
use sqlx::{Pool, Sqlite};

use crate::{
    config::Config,
    db::User,
    xcontest::{Flight, FlightDetails},
};

mod threema;

pub struct Notifier {
    pool: Pool<Sqlite>,
    threema: threema::ThreemaNotifier,
}

impl Notifier {
    pub fn new(pool: Pool<Sqlite>, client: Client, config: &Config) -> Result<Self> {
        Ok(Self {
            pool: pool.clone(),
            threema: threema::ThreemaNotifier::new(&config.threema, client, pool)?,
        })
    }

    /// Notify all subscribers about this flight.
    pub async fn notify(&mut self, flight: &Flight, details: Option<FlightDetails>) -> Result<()> {
        // Get connection
        let mut conn = self
            .pool
            .acquire()
            .await
            .context("Could not acquire db connection")?;

        let mut subscribers = sqlx::query_as::<_, User>(
            r#"
            SELECT u.id, u.username, u.usertype, u.threema_public_key
            FROM subscriptions s
            INNER JOIN users u ON s.user_id = u.id
            WHERE s.pilot_username = ? COLLATE NOCASE
            "#,
        )
        .bind(&flight.pilot_username)
        .fetch(&mut *conn);

        while let Some(subscriber) = subscribers.try_next().await? {
            tracing::info!(
                "Notifying {}/{} about flight {}",
                subscriber.usertype,
                subscriber.username,
                flight.url,
            );

            match &*subscriber.usertype {
                "threema" => self
                    .threema
                    .notify(flight, details.as_ref(), &subscriber)
                    .await
                    .unwrap_or_else(|e| tracing::error!("Could not notify threema user: {}", e)),
                other => tracing::warn!("Unsupported notification channel: {}", other),
            }
        }
        Ok(())
    }
}
