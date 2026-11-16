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
use threema_gateway::{errors::CryptoOrMessageDecodeError, E2eApi, E2eMessage};
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
    let span =
        tracing::debug_span!("incoming_message", from = msg.from.as_str(), id = %msg.message_id);
    let _enter = span.enter();
    tracing::trace!("Incoming message from {}", msg.from);
    tracing::trace!("Raw message: {:?}", msg);

    // Fetch user
    let user = match db::get_or_create_user(pool, msg.from.as_str(), "threema").await {
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

    // Decrypt and decode
    let data = match api.decrypt_and_decode_incoming_message(&msg, &public_key) {
        Ok(data) => data,
        Err(CryptoOrMessageDecodeError::Crypto(e)) => {
            tracing::error!("Could not decrypt message from {}: {}", &msg.from, e);
            return http_500();
        }
        Err(CryptoOrMessageDecodeError::Decode(e)) => {
            tracing::warn!(
                "Could not decode message from {}: {}, discarding",
                &msg.from,
                e
            );
            return http_200();
        }
    };
    tracing::debug!("Decrypted data: {:?}", data);

    // Handle depending on type
    match data {
        E2eMessage::Text(text) => {
            // Process text message
            match command_handlers::handle_threema_text_message(
                &text,
                msg.from.as_str(),
                msg.nickname.as_deref(),
                config.threema.admin_id.as_deref(),
                &user,
                pool,
            )
            .await
            {
                HandleResult::Reply(text) => {
                    let reply = E2eMessage::Text(text.into_owned());
                    match api.encode_and_encrypt(&reply, &public_key) {
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
        E2eMessage::DeliveryReceipt(_) => {
            // Delivery receipt, ignore
            tracing::info!("Ignoring delivery receipt");
            http_200()
        }
        other => {
            // Unsupported message type, ignore
            tracing::warn!(
                "Ignoring unsupported message type: {:?}",
                other.encode().message_type
            );
            http_200()
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
