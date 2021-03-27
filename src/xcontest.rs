use anyhow::{Context, Result};
use bytes::Bytes;

use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Client;

const XCONTEST_URL: &str = "https://www.xcontest.org/rss/flights/?ccc";

pub struct XContest {
    client: Client,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Flight {
    /// Flight title
    pub title: String,
    /// Flight URL
    pub url: String,
    /// Username of the pilot
    pub pilot_username: String,
}

#[derive(Debug, Clone)]
pub struct FlightDetails {
    /// Flight thumbnail (PNG data)
    pub thumbnail: Bytes,
}

impl Flight {
    pub fn new(title: String, url: String) -> Result<Self> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                r"(?x)
                http.*xcontest\.org.*
                /detail:(?P<pilot>[^/]*)
                /(?P<date>[^/]*)
                /(?P<time>[0-2][0-9]:[0-6][0-9])
            "
            )
            .unwrap();
        }
        let caps = RE
            .captures(&url)
            .context(format!("Regex did not match XContest URL ({})", &url))?;
        let pilot_username = caps.name("pilot").unwrap().as_str().to_string();
        Ok(Self {
            title,
            url,
            pilot_username,
        })
    }
}

impl XContest {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Fetch the latest RSS feed and parse it into a `Channel`.
    async fn fetch_feed(&self) -> Result<rss::Channel> {
        let feed_bytes = self.client.get(XCONTEST_URL).send().await?.bytes().await?;
        let channel = rss::Channel::read_from(&feed_bytes[..])?;
        Ok(channel)
    }

    pub async fn fetch_flights(&self) -> Result<Vec<Flight>> {
        let channel = self.fetch_feed().await?;
        let flights = channel
            .into_items()
            .into_iter()
            .filter_map(|item: rss::Item| match (item.title, item.link) {
                (Some(title), Some(link)) => match Flight::new(title, link) {
                    Ok(flight) => Some(flight),
                    Err(e) => {
                        tracing::warn!("Could not parse flight URL: {}", e);
                        None
                    }
                },
                _ => None,
            })
            .collect::<Vec<Flight>>();
        Ok(flights)
    }

    /// Fetch additional details for this flight.
    pub async fn fetch_flight_details(&self, flight: &Flight) -> Result<FlightDetails> {
        // Fetch flight details HTML
        let details_resp = self.client.get(&flight.url).send().await?;
        details_resp.error_for_status_ref()?;
        let html = details_resp.text().await?;

        // Extract thumbnail URL
        lazy_static! {
            static ref THUMBNAIL_RE: Regex = Regex::new(r#"<meta\s*property="og:image"\s*content="(?P<url>[^"]*)"\s*/>"#).unwrap();
        }
        let caps = THUMBNAIL_RE
            .captures(&html)
            .context("Thumbnail URL not found in flight details HTML")?;
        let thumbnail_url = caps.name("url").unwrap().as_str();

        // Fetch thumbnail
        let thumbnail_resp = self.client.get(thumbnail_url).send().await?;
        thumbnail_resp.error_for_status_ref()?;
        Ok(FlightDetails {
            thumbnail: thumbnail_resp.bytes().await?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url() {
        let title = "09.08.20 [21.98 km :: free_flight] Firstname Lastname".to_string();
        let url =
            "https://www.xcontest.org/2020/switzerland/en/flights/detail:dbrgn/9.8.2020/10:45"
                .to_string();
        let flight = Flight::new(title.clone(), url.clone()).unwrap();
        assert_eq!(flight.title, title);
        assert_eq!(flight.url, url);
        assert_eq!(flight.pilot_username, "dbrgn");
    }
}
