use anyhow::{Context, Result};

use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Client;

const XCONTEST_URL: &str = "https://www.xcontest.org/rss/flights/?ccc";

pub struct XContest {
    client: Client,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Flight {
    pub title: String,
    pub url: String,
    pub pilot_username: String,
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
