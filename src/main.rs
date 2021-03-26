use anyhow::Result;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::format::FmtSpan, FmtSubscriber};

mod xcontest;

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    LogTracer::init()?;
    let subscriber = FmtSubscriber::builder()
        .with_env_filter("debug")
        .with_span_events(FmtSpan::CLOSE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");

    tracing::info!("Hello, world!");
    let xc = xcontest::XContest::new();
    let channel = xc.fetch_flights().await?;
    println!("{:#?}", channel);
    Ok(())
}
