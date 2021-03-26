use anyhow::Result;

use reqwest::Client;

const XCONTEST_URL: &str = "https://www.xcontest.org/rss/flights/?ccc";

pub struct XContest {
    client: Client,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Flight {
    pub title: String,
    pub url: String,
}

impl XContest {
    pub fn new() -> Self {
        Self {
            client: Client::builder().user_agent("xc-bot").build().unwrap(),
        }
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
                (Some(title), Some(link)) => Some(Flight { title, url: link }),
                _ => None,
            })
            .collect::<Vec<Flight>>();
        Ok(flights)
    }
}
