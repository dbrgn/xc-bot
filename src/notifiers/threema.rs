//! Threema gateway notification channel.

use anyhow::Result;

use crate::xcontest::Flight;

pub struct ThreemaNotifier {
}

impl ThreemaNotifier {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn notify(&mut self, flight: &Flight, identity: &str) -> Result<()> {
        tracing::debug!("notify");
        Ok(())
    }
}
