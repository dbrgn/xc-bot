use anyhow::Result;
use futures::TryStreamExt;

use crate::xcontest::Flight;

type Conn<'a> = &'a mut sqlx::pool::PoolConnection<sqlx::Sqlite>;

pub struct Notifier<'a> {
    conn: Conn<'a>,
}

impl<'a> Notifier<'a> {
    pub fn new(conn: Conn<'a>) -> Self {
        Self { conn }
    }

    /// Notify all subscribers about this flight.
    pub async fn notify(&mut self, flight: &Flight) -> Result<()> {
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
                flight.url
            );
        }
        Ok(())
    }
}
