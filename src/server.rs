use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    routing::post,
};
use bytes::Bytes;
use lazy_static::lazy_static;
use regex::Regex;
use sqlx::{Pool, Sqlite};
use threema_gateway::E2eApi;

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

    /// Macro: Reply to sender
    macro_rules! reply {
        ($msg:expr) => {{
            match api.encrypt_text_msg($msg, &public_key.into()) {
                Ok(reply) => match api.send(&msg.from, &reply, false).await {
                    Ok(msgid) => tracing::debug!("Reply sent (msgid={})", msgid),
                    Err(e) => tracing::error!("Could not send reply: {}", e),
                },
                Err(e) => tracing::error!("Could not encrypt reply: {}", e),
            }
        }};
    }

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

            tracing::info!("Incoming request from {}: {:?}", msg.from, text);
            lazy_static! {
                static ref RE: Regex = Regex::new(
                    r"(?x)
                    (?P<command>[a-zA-Z]*)
                    \s*(?P<data>.*)"
                )
                .unwrap();
            }
            let caps = match RE.captures(text) {
                Some(caps) => caps,
                None => {
                    tracing::error!("Regex did not match incoming text {:?}", &text);
                    return http_500();
                }
            };
            let command = caps.name("command").unwrap().as_str().to_ascii_lowercase();
            match &*command {
                "stats" if Some(&msg.from) == config.threema.admin_id.as_ref() => {
                    tracing::info!("Received stats request from admin {}", msg.from);
                    match db::get_stats(pool).await {
                        Ok(stats) => reply!(&format!(
                            "Database stats:\n\n- Users: {}\n- Subscriptions: {}\n- Flights: {}",
                            stats.user_count, stats.subscription_count, stats.flight_count
                        )),
                        Err(e) => tracing::error!("Could not fetch stats: {}", e),
                    }
                }
                "folge" | "follow" | "add" => {
                    let usage = "Um einem Piloten zu folgen, sende \"folge _<benutzername>_\" \
                        (Beispiel: \"folge chrigel\"). \
                        Du musst dabei den Benutzernamen von XContest verwenden.";
                    if let Some(data) = caps.name("data") {
                        let pilot = data.as_str().trim();
                        if pilot.is_empty() {
                            reply!(usage);
                        } else if pilot.contains(' ') {
                            reply!(&format!("⚠️ Fehler: Der XContest-Benutzername darf kein Leerzeichen enthalten!\n\n{}", usage));
                        } else {
                            match db::add_subscription(pool, user.id, pilot).await {
                                Ok(_) => reply!(&format!("Du folgst jetzt {}!", pilot)),
                                Err(e) => {
                                    tracing::error!("Could not add subscription: {}", e);
                                    return http_500();
                                }
                            }
                        }
                    } else {
                        reply!(usage);
                    }
                }
                "stopp" | "stop" | "remove" => {
                    let usage = "Um einem Piloten zu entfolgen, sende \"stopp _<benutzername>_\" \
                        (Beispiel: \"stopp chrigel\"). \
                        Du musst dabei den Benutzernamen von XContest verwenden.";
                    if let Some(data) = caps.name("data") {
                        let pilot = data.as_str().trim();
                        if pilot.is_empty() {
                            reply!(usage);
                        } else {
                            match db::remove_subscription(pool, user.id, pilot).await {
                                Ok(true) => {
                                    reply!(&format!("Du folgst jetzt {} nicht mehr.", pilot))
                                }
                                Ok(false) => reply!(&format!("Du folgst {} nicht.", pilot)),
                                Err(e) => {
                                    tracing::error!("Could not remove subscription: {}", e);
                                    return http_500();
                                }
                            }
                        }
                    } else {
                        reply!(usage);
                    }
                }
                "liste" | "list" => {
                    let subscriptions = match db::get_subscriptions(pool, user.id).await {
                        Ok(subs) => subs,
                        Err(e) => {
                            tracing::error!(
                                "Could not fetch subscriptions for uid {}: {}",
                                user.id,
                                e
                            );
                            return http_500();
                        }
                    };
                    if subscriptions.is_empty() {
                        reply!(
                            "Du folgst noch keinen Piloten.\n\n\
                            Um einem Piloten zu folgen, sende \"folge _<benutzername>_\" (Beispiel: \"folge chrigel\"). \
                            Du musst dabei den Benutzernamen von XContest verwenden."
                        );
                    } else {
                        let mut reply = String::from("Du folgst folgenden Piloten:\n");
                        for pilot in subscriptions {
                            reply.push_str("\n- ");
                            reply.push_str(&pilot);
                        }
                        reply!(&reply);
                    }
                }
                "github" => reply!(
                    "Dieser Bot ist Open Source (AGPLv3). \
                    Den Quellcode findest du hier: https://github.com/dbrgn/xc-bot/"
                ),
                "version" => reply!(&format!("xc-bot v{}", crate::VERSION)),
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
                        Bei Fragen, schicke einfach eine Threema-Nachricht an https://threema.id/EBEP4UCA?text= !\
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
        .with_state(Arc::new(state));

    // Then bind and serve...
    let listener = tokio::net::TcpListener::bind(listen_addr).await.unwrap();
    tokio::spawn(async move {
        tracing::info!("Starting HTTP server on {}", listen_addr);
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Server error: {}", e);
        }
    });
}
