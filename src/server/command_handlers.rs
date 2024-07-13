use std::borrow::Cow;

use lazy_static::lazy_static;
use regex::{Match, Regex};
use sqlx::{Pool, Sqlite};

use crate::{
    config::Config,
    db::{self, User},
};

pub enum HandleResult {
    /// Send a reply containing the enclosed text to the sender of the command
    Reply(Cow<'static, str>),
    /// Do nothing, processing is done
    NoOp,
    /// Return a server error (HTTP 500)
    ServerError,
}

/// Handle a Threema message HTTP request
pub async fn handle_threema_text_message(
    text: &str,
    sender_identity: &str,
    sender_nickname: Option<&str>,
    user: &User,
    pool: &Pool<Sqlite>,
    config: &Config,
) -> HandleResult {
    // Parse command and data
    tracing::info!("Incoming request from {}: {:?}", sender_identity, text);
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
            return HandleResult::ServerError;
        }
    };
    let command = caps.name("command").unwrap().as_str().to_ascii_lowercase();

    // Process command
    match &*command {
        "stats" if Some(sender_identity) == config.threema.admin_id.as_deref() => {
            handle_admin_stats(sender_identity, &pool).await
        }
        "folge" | "follow" | "add" => handle_follow(caps.name("data"), &user, &pool).await,
        "stopp" | "stop" | "remove" => handle_unfollow(caps.name("data"), &user, &pool).await,
        "liste" | "list" => handle_list(&user, &pool).await,
        "github" => handle_github().await,
        "version" => handle_version().await,
        other => handle_unknown_command(other, sender_identity, sender_nickname).await,
    }
}

/// Handle command to show admin stats
async fn handle_admin_stats(sender_identity: &str, pool: &Pool<Sqlite>) -> HandleResult {
    tracing::info!("Received stats request from admin {}", sender_identity);
    match db::get_stats(pool).await {
        Ok(stats) => HandleResult::Reply(
            format!(
                "Database stats:\n\n- Users: {}\n- Subscriptions: {}\n- Flights: {}",
                stats.user_count, stats.subscription_count, stats.flight_count
            )
            .into(),
        ),
        Err(e) => {
            tracing::error!("Could not fetch stats: {}", e);
            HandleResult::NoOp
        }
    }
}

/// Handle command to follow a pilot
async fn handle_follow(
    command_data: Option<Match<'_>>,
    user: &User,
    pool: &Pool<Sqlite>,
) -> HandleResult {
    let usage = "Um einem Piloten zu folgen, sende \"folge _<benutzername>_\" \
        (Beispiel: \"folge chrigel\"). \
        Du musst dabei den Benutzernamen von XContest verwenden.";

    let pilot = match command_data {
        Some(data) => data.as_str().trim(),
        None => return HandleResult::Reply(Cow::Borrowed(usage)),
    };

    // Validate pilot name
    if pilot.is_empty() {
        return HandleResult::Reply(Cow::Borrowed(usage));
    }
    if pilot.contains(' ') {
        return HandleResult::Reply(
            format!(
                "⚠️ Fehler: Der XContest-Benutzername darf kein Leerzeichen enthalten!\n\n{}",
                usage
            )
            .into(),
        );
    }

    // Add subscription
    match db::add_subscription(pool, user.id, pilot).await {
        Ok(_) => HandleResult::Reply(format!("Du folgst jetzt {}!", pilot).into()),
        Err(e) => {
            tracing::error!("Could not add subscription: {}", e);
            HandleResult::ServerError
        }
    }
}

/// Handle command to unfollow a pilot
async fn handle_unfollow(
    command_data: Option<Match<'_>>,
    user: &User,
    pool: &Pool<Sqlite>,
) -> HandleResult {
    let usage = "Um einem Piloten zu entfolgen, sende \"stopp _<benutzername>_\" \
        (Beispiel: \"stopp chrigel\"). \
        Du musst dabei den Benutzernamen von XContest verwenden.";

    let pilot = match command_data {
        Some(data) => data.as_str().trim(),
        None => return HandleResult::Reply(Cow::Borrowed(usage)),
    };

    // Validate pilot name
    if pilot.is_empty() {
        return HandleResult::Reply(Cow::Borrowed(usage));
    }

    // Remove subscription
    match db::remove_subscription(pool, user.id, pilot).await {
        Ok(true) => HandleResult::Reply(format!("Du folgst jetzt {} nicht mehr.", pilot).into()),
        Ok(false) => HandleResult::Reply(format!("Du folgst {} nicht.", pilot).into()),
        Err(e) => {
            tracing::error!("Could not remove subscription: {}", e);
            HandleResult::ServerError
        }
    }
}

/// Handle command to list subscriptions
async fn handle_list(user: &User, pool: &Pool<Sqlite>) -> HandleResult {
    // Fetch subscriptions
    let subscriptions = match db::get_subscriptions(pool, user.id).await {
        Ok(subs) => subs,
        Err(e) => {
            tracing::error!("Could not fetch subscriptions for uid {}: {}", user.id, e);
            return HandleResult::ServerError;
        }
    };

    // Reply with subscriptions
    if subscriptions.is_empty() {
        HandleResult::Reply(Cow::Borrowed(
            "Du folgst noch keinen Piloten.\n\n\
            Um einem Piloten zu folgen, sende \"folge _<benutzername>_\" (Beispiel: \"folge chrigel\"). \
            Du musst dabei den Benutzernamen von XContest verwenden."
        ))
    } else {
        let mut reply = String::from("Du folgst folgenden Piloten:\n");
        for pilot in subscriptions {
            reply.push_str("\n- ");
            reply.push_str(&pilot);
        }
        HandleResult::Reply(reply.into())
    }
}

/// Show information about source code of this bot
async fn handle_github() -> HandleResult {
    HandleResult::Reply(Cow::Borrowed(
        "Dieser Bot ist Open Source (AGPLv3). \
        Den Quellcode findest du hier: https://github.com/dbrgn/xc-bot/",
    ))
}

/// Show information about bot version
async fn handle_version() -> HandleResult {
    HandleResult::Reply(format!("xc-bot v{}", crate::VERSION).into())
}

/// Handle unknown command
async fn handle_unknown_command(
    command: &str,
    sender_identity: &str,
    sender_nickname: Option<&str>,
) -> HandleResult {
    tracing::debug!("Unknown command: {:?}", command);
    let nickname_or_identity: &str = sender_nickname.as_ref().unwrap_or(&sender_identity).trim();
    HandleResult::Reply(format!(
        "Hallo {}! 👋\n\n\
        Mit diesem Bot kannst du Piloten im CCC (XContest Schweiz) folgen. Du kriegst dann eine sofortige Benachrichtigung, wenn diese einen neuen Flug hochladen. 🪂\n\n\
        Verfügbare Befehle:\n\n\
        - *folge _<benutzername>_*: Werde benachrichtigt, wenn der Pilot _<benutzername>_ einen neuen Flug hochlädt. Du musst dabei den Benutzernamen von XContest verwenden.\n\
        - *stopp _<benutzername>_*: Werde nicht mehr benachrichtigt, wenn der Pilot _<benutzername>_ einen neuen Flug hochlädt. Du musst dabei den Benutzernamen von XContest verwenden.\n\
        - *liste*: Zeige die Liste der Piloten, deren Flüge du abonniert hast.\n\
        - *github*: Zeige den Link zum Quellcode dieses Bots.\n\n\
        Bei Fragen, schicke einfach eine Threema-Nachricht an https://threema.id/EBEP4UCA?text= !\
        ",
        nickname_or_identity,
    ).into())
}
