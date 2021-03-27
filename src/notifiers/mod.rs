use anyhow::Result;
use futures::TryStreamExt;
use reqwest::Client;

use crate::{
    config::Config,
    xcontest::{Flight, FlightDetails},
};

mod threema;

type Conn<'a> = &'a mut sqlx::pool::PoolConnection<sqlx::Sqlite>;

pub struct Notifier<'a> {
    conn: Conn<'a>,
    threema: threema::ThreemaNotifier,
}

impl<'a> Notifier<'a> {
    pub fn new(conn: Conn<'a>, client: Client, config: &'a Config) -> Result<Self> {
        Ok(Self {
            conn,
            threema: threema::ThreemaNotifier::new(&config.threema, client)?,
        })
    }

    /// Notify all subscribers about this flight.
    pub async fn notify(&mut self, flight: &Flight, details: Option<FlightDetails>) -> Result<()> {
        let mut subscribers = sqlx::query!(
            r#"
            SELECT u.username, u.usertype
            FROM subscriptions s
            INNER JOIN users u ON s.user_id = u.id
            WHERE s.pilot_username = ?
            "#,
            flight.pilot_username,
        )
        .fetch(&mut *self.conn);
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
                    .notify(flight, details.as_ref(), &subscriber.username)
                    .await
                    .unwrap_or_else(|e| tracing::error!("Could not notify threema user: {}", e)),
                other => tracing::warn!("Unsupported notification channel: {}", other),
            }
        }
        Ok(())
    }
}
