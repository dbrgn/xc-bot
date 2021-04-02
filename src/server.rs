use std::convert::Infallible;

use bytes::Bytes;
use hyper::{Body, Method, Request, Response, StatusCode};
use lazy_static::lazy_static;
use regex::Regex;
use sqlx::{Pool, Sqlite};

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
#[tracing::instrument(skip(req, api, pool))]
pub async fn handle_http_request(
    req: Request<Body>,
    api: threema_gateway::E2eApi,
    pool: Pool<Sqlite>,
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

            // Process message
            let response = handle_threema_request(body, api, pool).await;
            tracing::info!("Responding with HTTP {}", response.status());
            Ok(response)
        }

        // Return 404 for all other requests
        (p, m) => {
            // Not found
            tracing::warn!("Unexpected HTTP {} request to {}", m, p);
            Ok(http_404())
        }
    }
}

pub async fn handle_threema_request(
    bytes: Bytes,
    api: threema_gateway::E2eApi,
    _pool: Pool<Sqlite>,
) -> Response<Body> {
    // Parse body
    let msg = match api.decode_incoming_message(bytes) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Could not decode incoming Threema message: {}", e);
            return http_500();
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
            return http_500();
        }
    };

    // Decrypt
    let data = match api.decrypt_incoming_message(&msg, &pubkey) {
        Ok(key) => key,
        Err(e) => {
            tracing::error!("Could not fetch public key for {}: {}", &msg.from, e);
            return http_500();
        }
    };
    tracing::debug!("Decrypted data: {:?}", data);

    // Handle depending on type
    match data.get(0) {
        Some(0x01) => {
            // Text message, UTF-8
            let text = match std::str::from_utf8(&data[1..]) {
                Ok(decoded) => decoded,
                Err(_) => {
                    tracing::warn!("Received non-UTF8 bytes: {:?}, discarding", &data[1..]);
                    return http_200();
                }
            };

            macro_rules! reply {
                ($msg:expr) => {{
                    let reply = api.encrypt_text_msg($msg, &pubkey.into());
                    match api.send(&msg.from, &reply, false).await {
                        Ok(msgid) => tracing::debug!("Reply sent (msgid={})", msgid),
                        Err(e) => tracing::error!("Could not send reply: {}", e),
                    }
                }};
            }

            tracing::info!("Incoming request from {}: {:?}", msg.from, text);
            lazy_static! {
                static ref RE: Regex = Regex::new(
                    r"(?x)
                    (?P<command>[a-zA-Z]*)
                    \s*(?P<data>.*)"
                )
                .unwrap();
            }
            let caps = match RE.captures(&text) {
                Some(caps) => caps,
                None => {
                    tracing::error!("Regex did not match incoming text {:?}", &text);
                    return http_500();
                }
            };
            let command = caps.name("command").unwrap().as_str().to_ascii_lowercase();
            match &*command {
                "folge" | "follow" => {
                    reply!("🚧 Noch nicht implementiert");
                }
                "stopp" | "stop" => {
                    reply!("🚧 Noch nicht implementiert");
                }
                "liste" | "list" => {
                    reply!("Du folgst folgenden Piloten:\n\n- 🚧 Noch nicht implementiert");
                }
                "github" => {
                    reply!(
                        "Dieser Bot ist Open Source (AGPLv3). \
                        Den Quellcode findest du hier: https://github.com/dbrgn/xc-bot/"
                    );
                }
                other => {
                    tracing::debug!("Unknown command: {:?}", other);
                    let nickname: &str = msg.nickname.as_ref().unwrap_or(&msg.from).trim();
                    reply!(&format!(
                        "Hallo {}! 👋\n\n\
                        Mit diesem Bot kannst du Piloten im CCC (XContest Schweiz) folgen. Du kriegst dann eine sofortige Benachrichtigung, wenn diese einen neuen Flug hochladen. 🪂\n\n\
                        Verfügbare Befehle:\n\n\
                        - *folge _<benutzername>_*: Werde benachrichtigt, wenn der Pilot _<benutzername>_ einen neuen Flug hochlädt. Du musst dabei den Benutzernamen von XContest verwenden.\n\
                        - *stopp _<benutzername>_*: Werde nicht mehr benachrichtigt, wenn der Pilot _<benutzername>_ einen neuen Flug hochlädt. Du musst dabei den Benutzernamen von XContest verwenden.\n\
                        - *liste*: Zeige die Liste der Piloten, deren Flüge du abonniert hast.\n\
                        - *github*: Zeige den Link zum Quellcode dieses Bots.\n\n\
                        Bei Fragen, schicke einfach eine Threema-Nachricht an https://threema.id/EBEP4UCA !\
                        ",
                        nickname,
                    ));
                }
            }

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
