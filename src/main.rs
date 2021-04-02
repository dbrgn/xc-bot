use std::{convert::Infallible, net::SocketAddr, process, str::FromStr, time::Duration};

use anyhow::{Context, Result};
use hyper::{
    service::{make_service_fn, service_fn},
    Server,
};
use reqwest::Client;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Pool, Sqlite,
};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::format::FmtSpan, FmtSubscriber};

mod cli;
mod config;
mod notifiers;
mod server;
mod xcontest;

use config::Config;
use xcontest::XContest;

const NAME: &str = "XC Bot";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = env!("CARGO_PKG_AUTHORS");
const DESCRIPTION: &str =
    "A chat bot that notifies you about new paragliding cross-country flights.";

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line args
    let app = cli::App::new(NAME, VERSION, DESCRIPTION, AUTHOR, "config.toml");

    // Load config
    let configfile = app.get_configfile();
    let config = Config::load(&configfile).unwrap_or_else(|e| {
        eprintln!("Could not load config file {:?}: {}", configfile, e);
        process::exit(2);
    });

    // Init logging
    LogTracer::init()?;
    let filter: String = config
        .logging
        .as_ref()
        .and_then(|logging| logging.filter.to_owned())
        .unwrap_or_else(|| "info,sqlx::query=warn".into());
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(&filter)
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

    // Create shared HTTP client
    let client = Client::builder()
        .https_only(true)
        .pool_idle_timeout(Duration::from_secs(300))
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .context("Could not create HTTP client")?;

    // Create XContest client
    let xc = XContest::new(client.clone());

    // Create Threema Gateway API instance
    let api = threema_gateway::ApiBuilder::new(
        &config.threema.gateway_id,
        &config.threema.gateway_secret,
    )
    .with_private_key_str(&config.threema.private_key)
    .and_then(|builder| builder.into_e2e())
    .context("Could not create Threema Gateway API client")?;

    // Listening address for HTTP server
    let addr: SocketAddr = config
        .server
        .listen
        .parse()
        .context("Could not parse HTTP server listening address")?;

    // And a MakeService to handle each HTTP connection
    let pool_clone = pool.clone();
    let make_service = make_service_fn(move |_conn| {
        let api = api.clone();
        let pool = pool_clone.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let api = api.clone();
                let pool = pool.clone();
                async move { server::handle_http_request(req, api, pool).await }
            }))
        }
    });

    // Then bind and serve...
    let server = Server::bind(&addr).serve(make_service);
    tokio::spawn(async move {
        tracing::info!("Starting HTTP server on {}", addr);
        if let Err(e) = server.await {
            tracing::error!("Server error: {}", e);
        }
    });

    // Main loop, run every minute
    let interval_duration = Duration::from_secs(60);
    let mut interval = tokio::time::interval(interval_duration);
    tracing::info!(
        "Starting XContest fetch loop with {:?} interval",
        interval_duration
    );
    loop {
        interval.tick().await;
        match update(&pool, &xc, &client, &config).await {
            Ok(_) => {}
            Err(e) => tracing::warn!("Update failed: {}", e),
        };
    }
}

/// This function will be called regularly to fetch new flights.
#[tracing::instrument(skip(pool, xc, client, config))]
async fn update(
    pool: &Pool<Sqlite>,
    xc: &XContest,
    client: &Client,
    config: &Config,
) -> Result<()> {
    tracing::info!("Update started");

    // Connect to XContest, fetch flights
    let flights = xc.fetch_flights().await?;

    // Process flights
    let mut conn = pool.acquire().await?;
    let total_flights = flights.len();
    let mut new_flights = 0;
    for flight in flights {
        // Store flight in database.
        let result = sqlx::query(
            r#"
            INSERT INTO xcontest_flights (url, title, pilot_username)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(&flight.url)
        .bind(&flight.title)
        .bind(&flight.pilot_username)
        .execute(&mut conn)
        .await;

        // If inserting fails with a unique constraint, that means that the
        // flight was already processed before.
        match result {
            Err(sqlx::Error::Database(e))
                if e.message() == "UNIQUE constraint failed: xcontest_flights.url" =>
            {
                tracing::debug!("Flight {} already processed, skipping", flight.url);
                continue;
            }
            Err(other) => {
                // Uh oh...
                tracing::error!(
                    "Error inserting flight {} into database: {}",
                    flight.url,
                    other
                );
                continue;
            }
            Ok(_) => { /* Database entry did not yet exist, carry on with processing */ }
        }

        // Notify
        tracing::info!("New flight: {}", flight.title);
        new_flights += 1;
        // TODO: Only fetch if subscribers present
        let details = match xc.fetch_flight_details(&flight).await {
            Ok(details) => Some(details),
            Err(e) => {
                tracing::warn!("Could not fetch flight details: {}", e);
                None
            }
        };
        let mut notifier = match notifiers::Notifier::new(&mut conn, client.clone(), &config) {
            Ok(n) => n,
            Err(e) => {
                tracing::error!("Could not instantiate notifier: {}", e);
                continue;
            }
        };
        notifier.notify(&flight, details).await?;
    }

    tracing::info!(
        "Update done, found {}/{} new flights",
        new_flights,
        total_flights
    );
    Ok(())
}
