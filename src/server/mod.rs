use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    routing::post,
};
use bytes::Bytes;
use command_handlers::HandleResult;
use sqlx::{Pool, Sqlite};
use threema_gateway::E2eApi;
use tower_http::trace::TraceLayer;

mod command_handlers;

use crate::{config::Config, db, threema};

fn http_200() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from("processed"))
        .unwrap()
}

fn http_500() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .unwrap()
}

/// Handle a Threema message HTTP request
async fn handle_threema_request(state: State<Arc<SharedState>>, bytes: Bytes) -> Response<Body> {
    let api = &state.api;
    let pool = &state.pool;
    let config = &state.config;

    // Parse body
    let msg = match state.api.decode_incoming_message(bytes) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Could not decode incoming Threema message: {}", e);
            return http_500();
        }
    };
    let span = tracing::debug_span!("incoming_message", from = &*msg.from, id = &*msg.message_id);
    let _enter = span.enter();
    tracing::trace!("Incoming message from {}", msg.from);
    tracing::trace!("Raw message: {:?}", msg);

    // Fetch user
    let user = match db::get_or_create_user(pool, &msg.from, "threema").await {
        Ok(user) => {
            tracing::debug!("User ID: {}", user.id);
            user
        }
        Err(e) => {
            tracing::error!("Error in get_or_create_user: {}", e);
            return http_500();
        }
    };

    // Fetch sender public key
    let public_key = match threema::get_public_key(&user, api, pool).await {
        Ok(pk) => pk,
        Err(e) => {
            tracing::error!("Could not fetch public key for {}: {}", &msg.from, e);
            return http_500();
        }
    };

    // Decrypt
    let data = match api.decrypt_incoming_message(&msg, &public_key) {
        Ok(key) => key,
        Err(e) => {
            tracing::error!("Could not fetch public key for {}: {}", &msg.from, e);
            return http_500();
        }
    };
    tracing::debug!("Decrypted data: {:?}", data);

    // Handle depending on type
    match data.first() {
        Some(0x01) => {
            // Text message, UTF-8
            let text = match std::str::from_utf8(&data[1..]) {
                Ok(decoded) => decoded,
                Err(_) => {
                    tracing::warn!("Received non-UTF8 bytes: {:?}, discarding", &data[1..]);
                    return http_200();
                }
            };

            // Process text message
            match command_handlers::handle_threema_text_message(
                &text,
                &msg.from,
                msg.nickname.as_deref(),
                config.threema.admin_id.as_deref(),
                &user,
                pool,
            )
            .await
            {
                HandleResult::Reply(text) => {
                    match api.encrypt_text_msg(text.as_ref(), &public_key.into()) {
                        Ok(reply) => match api.send(&msg.from, &reply, false).await {
                            Ok(msgid) => tracing::debug!("Reply sent (msgid={})", msgid),
                            Err(e) => tracing::error!("Could not send reply: {}", e),
                        },
                        Err(e) => tracing::error!("Could not encrypt reply: {}", e),
                    }
                }
                HandleResult::NoOp => {}
                HandleResult::ServerError => return http_500(),
            };

            // Done processing, confirm message
            http_200()
        }
        Some(0x80) => {
            // Delivery receipt, ignore
            tracing::info!("Ignoring delivery receipt");
            http_200()
        }
        Some(other) => {
            // Unsupported message type, ignore
            tracing::warn!("Ignoring unsupported message type: {}", other);
            http_200()
        }
        None => {
            // Empty data
            tracing::warn!("Incoming decrypted data is empty");
            http_500()
        }
    }
}

pub struct SharedState {
    pub api: E2eApi,
    pub pool: Pool<Sqlite>,
    pub config: Config,
}

/// Bind to `listen_addr` and serve forever.
///
/// The async call will return once the server task has been spawned.
pub async fn serve(state: SharedState, listen_addr: SocketAddr) {
    // Set up routing and shared state
    let app = axum::Router::new()
        .route("/receive/threema/", post(handle_threema_request))
        .with_state(Arc::new(state))
        .layer(TraceLayer::new_for_http());

    // Then bind and serve...
    let listener = tokio::net::TcpListener::bind(listen_addr).await.unwrap();
    tokio::spawn(async move {
        tracing::info!("Starting HTTP server on {}", listen_addr);
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Server error: {}", e);
        }
    });
}
