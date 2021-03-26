use anyhow::Result;

mod xcontest;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Hello, world!");
    let xc = xcontest::XContest::new();
    let channel = xc.fetch_flights().await?;
    println!("{:#?}", channel);
    Ok(())
}
