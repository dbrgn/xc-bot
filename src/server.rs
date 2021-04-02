use std::convert::Infallible;

use bytes::Bytes;
use hyper::{Body, Method, Request, Response, StatusCode};

fn http_200() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(""))
        .unwrap()
}

fn http_404() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}

fn http_500() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .unwrap()
}

/// Handle an incoming HTTP request.
#[tracing::instrument(skip(req, api))]
pub async fn handle_http_request(
    req: Request<Body>,
    api: threema_gateway::E2eApi,
) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();
    let method = req.method();
    match (path, method) {
        // Threema: Handle POST requests to /receive/threema/
        ("/receive/threema/", &Method::POST) => {
            // Read body bytes
            let body: Bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("Could not read body bytes: {}", e);
                    return Ok(http_500());
                }
            };

            // Parse body
            let msg = match api.decode_incoming_message(&body) {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("Could not decode incoming Threema message: {}", e);
                    return Ok(http_500());
                }
            };
            let span = tracing::info_span!("incoming_message", from = &*msg.from, id = &*msg.message_id);
            let _enter = span.enter();
            tracing::trace!("Incoming message from {}", msg.from);
            tracing::trace!("Raw message: {:?}", msg);

            // Fetch sender public key
            // TODO: Cache
            let pubkey = match api.lookup_pubkey(&msg.from).await {
                Ok(key) => key,
                Err(e) => {
                    tracing::error!("Could not fetch public key for {}: {}", &msg.from, e);
                    return Ok(http_500());
                }
            };

            // Decrypt
            let data = match api.decrypt_incoming_message(&msg, &pubkey) {
                Ok(key) => key,
                Err(e) => {
                    tracing::error!("Could not fetch public key for {}: {}", &msg.from, e);
                    return Ok(http_500());
                }
            };
            tracing::debug!("Decrypted data: {:?}", data);

            // Handle depending on type
            Ok(match data.get(0) {
                Some(0x01) => {
                    // Text message, UTF-8
                    let text = match std::str::from_utf8(&data[1..]) {
                        Ok(decoded) => decoded,
                        Err(_) => {
                            tracing::warn!("Received non-UTF8 bytes: {:?}, discarding", &data[1..]);
                            return Ok(http_200());
                        }
                    };
                    tracing::info!("Text: {:?}", text);
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
            })
        }

        // Return 404 for all other requests
        (p, m) => {
            // Not found
            tracing::warn!("Unexpected HTTP {} request to {}", m, p);
            Ok(http_404())
        }
    }
}
