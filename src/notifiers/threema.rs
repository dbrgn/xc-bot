//! Threema gateway notification channel.

use anyhow::{Context, Result};
use reqwest::Client;
use threema_gateway::{ApiBuilder, E2eApi, RecipientKey};

use crate::{config::ThreemaConfig, xcontest::Flight};

pub struct ThreemaNotifier {
    api: E2eApi,
}

impl ThreemaNotifier {
    pub fn new(config: &ThreemaConfig, client: Client) -> Result<Self> {
        let api = ApiBuilder::new(&config.gateway_id, &config.gateway_secret)
            .with_client(client)
            .with_private_key_str(&config.private_key)
            .and_then(|builder| builder.into_e2e())
            .context("Could not create Threema API object")?;
        Ok(Self { api })
    }

    pub async fn notify(&mut self, flight: &Flight, identity: &str) -> Result<()> {
        tracing::debug!("notify");

        // Fetch public key of recipient
        // TODO: Cache
        let public_key = self
            .api
            .lookup_pubkey(identity)
            .await
            .context("Could not look up recipient public key")?;
        let recipient_key: RecipientKey = public_key
            .parse()
            .context("Could not parse recipient public key")?;

        // Encrypt notification message
        let text = format!("{}\n{}", flight.title, flight.url);
        let encrypted = self.api.encrypt_text_msg(&text, &recipient_key);

        // Send
        match self.api.send(identity, &encrypted, false).await {
            Ok(msg_id) => println!("Sent. Message id is {}.", msg_id),
            Err(e) => println!("Could not send message: {:?}", e),
        }

        Ok(())
    }
}
