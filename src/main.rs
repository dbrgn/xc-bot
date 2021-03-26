use std::str::FromStr;

use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::format::FmtSpan, FmtSubscriber};

mod xcontest;

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    LogTracer::init()?;
    let subscriber = FmtSubscriber::builder()
        .with_env_filter("debug,sqlx::query=warn")
        .with_span_events(FmtSpan::CLOSE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");
    tracing::info!("Hello, pilots!");

    // Connect to database
    let connect_options = SqliteConnectOptions::from_str("sqlite:data.db")?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .min_connections(2)
        .max_connections(5)
        .connect_with(connect_options)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Connect to XContest, fetch flights
    let xc = xcontest::XContest::new();
    let flights = xc.fetch_flights().await?;

    // Process flights
    let mut conn = pool.acquire().await?;
    for flight in flights {
        // Store flight in database.
        let result = sqlx::query!(
            r#"
            INSERT INTO xcontest_flights (url, title, pilot_username)
            VALUES (?, ?, ?)
            "#,
            flight.url,
            flight.title,
            flight.pilot_username,
        )
        .execute(&mut conn)
        .await;

        // If inserting fails with a unique constraint, that means that the
        // flight was already processed before.
        match result {
            Err(sqlx::Error::Database(e)) if e.message() == "UNIQUE constraint failed: xcontest_flights.url" => {
                tracing::debug!("Flight {} already processed, skipping", flight.url); 
                continue;
            }
            Err(other) => {
                // Uh oh...
                tracing::error!("Error inserting flight {} into database: {}", flight.url, other);
                continue;
            },
            Ok(_) => { /* Database entry did not yet exist, carry on with processing */ }
        }

        // Notify
        tracing::info!("New flight: {}", flight.title);
    }

    Ok(())
}
