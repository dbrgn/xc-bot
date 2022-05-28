//! Threema gateway notification channel.

use std::{convert::TryInto, str::FromStr};

use anyhow::{Context, Result};
use reqwest::Client;
use sqlx::{Pool, Sqlite};
use threema_gateway::{
    encrypt_file_data, ApiBuilder, E2eApi, FileMessage, Mime, RecipientKey, RenderingType,
};

use crate::{
    config::ThreemaConfig,
    db::User,
    threema,
    xcontest::{Flight, FlightDetails},
};

pub struct ThreemaNotifier {
    api: E2eApi,
    pool: Pool<Sqlite>,
}

impl ThreemaNotifier {
    pub fn new(config: &ThreemaConfig, client: Client, pool: Pool<Sqlite>) -> Result<Self> {
        let api = ApiBuilder::new(&config.gateway_id, &config.gateway_secret)
            .with_client(client)
            .with_private_key_str(&config.private_key)
            .and_then(|builder| builder.into_e2e())
            .context("Could not create Threema API object")?;
        Ok(Self { api, pool })
    }

    /// Notify the specified Threema user about the flight.
    pub async fn notify(
        &mut self,
        flight: &Flight,
        details: Option<&FlightDetails>,
        user: &User,
    ) -> Result<()> {
        tracing::debug!("notify");

        // Fetch public key of recipient
        let public_key = threema::get_public_key(user, &self.api, &self.pool).await?;
        let recipient_key: RecipientKey = public_key.into();

        // Notification text
        let text = format!("{}\n{}", flight.title, flight.url);

        // Depending on whether or not we have details, we'll send a text or image message.
        let msg_id = if let Some(details) = details {
            // Encrypt file message contents
            let (file_data, thumb_data, key) =
                encrypt_file_data(&details.thumbnail_large, Some(&details.thumbnail_small));
            let thumb_data = thumb_data.unwrap();

            // Upload image data
            let file_blob_id = self
                .api
                .blob_upload_raw(&file_data, false)
                .await
                .context("Could not upload file blob")?;
            let thumb_blob_id = self
                .api
                .blob_upload_raw(&thumb_data, false)
                .await
                .context("Could not upload thumbnail blob")?;

            // Create file message
            let msg = FileMessage::builder(
                file_blob_id,
                key,
                Mime::from_str("image/png").unwrap(),
                file_data.len().try_into().unwrap(),
            )
            .thumbnail(thumb_blob_id, Mime::from_str("image/jpeg").unwrap())
            .description(text)
            .file_name("preview.png")
            .rendering_type(RenderingType::Media)
            .animated(false)
            .build()
            .context("Could not create file message")?;
            let encrypted = self.api.encrypt_file_msg(&msg, &public_key.into());

            // Send
            self.api.send(&user.username, &encrypted, false).await?
        } else {
            // Encrypt simple notification text message
            let encrypted = self.api.encrypt_text_msg(&text, &recipient_key);

            // Send
            self.api.send(&user.username, &encrypted, false).await?
        };

        tracing::debug!("Notification sent, message id is {}", msg_id);
        Ok(())
    }
}
