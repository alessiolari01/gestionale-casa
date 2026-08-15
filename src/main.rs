//! Punto di ingresso del Gestionale Casa.
//!
//! Step corrente: Step 6A, case, stanze e posizione strutturata condivisa
//! sopra l'infrastruttura Telegram + SQLx + SQLite gia' verificata.

mod auth;
mod config;
mod db;
mod modules;

use std::sync::Arc;

use anyhow::Context;
use config::Config;
use modules::{foto::PhotoSessionStore, luoghi::LocationSessionStore, oggetti::SessionStore};
use sqlx::SqlitePool;
use teloxide::{
    dptree,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let config = Arc::new(Config::load()?);
    tracing::info!(
        authorized_chats = config.allowed_chat_ids.len(),
        "Configurazione caricata"
    );

    let pool = db::connect(&config.database_url).await?;
    let database_status = db::status(&pool).await?;
    tracing::info!(
        applied_migrations = database_status.applied_migrations,
        schema_core = database_status.schema_core_present,
        "Database SQLite pronto"
    );

    let bot = Bot::new(config.telegram_token.clone());
    let me = bot
        .get_me()
        .await
        .context("Impossibile collegarsi al bot Telegram")?;
    tracing::info!(bot_username = ?me.username(), "Gestionale Casa online");

    // Il messaggio di avvio rende subito evidente che il backend e' tornato
    // online e offre direttamente il menu principale senza richiedere /start.
    for chat_id in config.allowed_chat_ids.iter().copied() {
        if let Err(error) = send_online_menu(&bot, ChatId(chat_id)).await {
            tracing::warn!(
                chat_id,
                ?error,
                "Impossibile inviare la notifica di avvio alla chat autorizzata"
            );
        }
    }

    let sessions = SessionStore::new();
    let location_sessions = LocationSessionStore::new();
    let photo_sessions = PhotoSessionStore::new();
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    // Il dispatcher prende possesso di bot/config. Conserviamo solo cio' che
    // serve per notificare uno shutdown controllato (Ctrl+C compreso).
    let shutdown_bot = bot.clone();
    let shutdown_chat_ids = config.allowed_chat_ids.clone();

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            config,
            pool,
            sessions,
            location_sessions,
            photo_sessions
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    for chat_id in shutdown_chat_ids {
        if let Err(error) = shutdown_bot
            .send_message(ChatId(chat_id), "🔴 Gestionale Casa è offline.")
            .await
        {
            tracing::warn!(
                chat_id,
                ?error,
                "Impossibile inviare la notifica di spegnimento alla chat autorizzata"
            );
        }
    }

    tracing::info!("Gestionale Casa offline");
    Ok(())
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    config: Arc<Config>,
    pool: SqlitePool,
    sessions: SessionStore,
    location_sessions: LocationSessionStore,
    photo_sessions: PhotoSessionStore,
) -> ResponseResult<()> {
    let chat_id = msg.chat.id.0;
    if !auth::is_authorized(chat_id, &config.allowed_chat_ids) {
        tracing::warn!(chat_id, "Messaggio ignorato da chat non autorizzata");
        return respond(());
    }

    // Se si entra esplicitamente nel flusso foto da comando, chiudiamo una
    // eventuale bozza oggetto rimasta aperta per evitare stati concorrenti.
    if matches!(
        msg.text().and_then(first_command),
        Some("/foto") | Some("/foto_aggiungi")
    ) {
        sessions.clear_chat(chat_id);
        location_sessions.clear_chat(chat_id);
    }

    // I comandi foto e, soprattutto, le foto vere e proprie devono essere
    // gestiti prima del controllo msg.text(), perche' una foto non e' testo.
    if modules::foto::handle_message(&bot, &msg, &pool, &photo_sessions).await? {
        return respond(());
    }

    let Some(text) = msg.text() else {
        if msg.photo().is_some() {
            bot.send_message(
                msg.chat.id,
                "📷 Non sto aspettando una foto. Apri la scheda di un oggetto e usa 📷 Foto → ➕ Aggiungi foto.",
            )
            .await?;
        }
        return respond(());
    };

    let command = first_command(text);

    // Qualunque altro comando esplicito interrompe un'eventuale attesa foto:
    // evita che una foto inviata piu' tardi venga associata per errore.
    if command.is_some() {
        photo_sessions.clear_chat(chat_id);
    }

    if modules::luoghi::handle_message(&bot, &msg, &pool, &location_sessions, text).await? {
        sessions.clear_chat(chat_id);
        return respond(());
    }

    if modules::oggetti::handle_message(&bot, &msg, &pool, &sessions, text).await? {
        location_sessions.clear_chat(chat_id);
        return respond(());
    }

    match command {
        Some("/start") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            send_main_menu(&bot, msg.chat.id).await?;
        }
        Some("/ping") => {
            bot.send_message(msg.chat.id, "Pong! Gestionale Casa è online.")
                .await?;
        }
        Some("/storico") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            modules::storico::show_global_history(&bot, msg.chat.id, &pool, 0).await?;
        }
        Some("/status") => {
            send_status(&bot, msg.chat.id, &pool).await?;
        }
        Some(_) => {
            bot.send_message(
                msg.chat.id,
                "Comando non riconosciuto.\nUsa /start per aprire il menu principale.",
            )
            .await?;
        }
        None => {
            bot.send_message(
                msg.chat.id,
                "Non c'è un'operazione attiva. Usa /start oppure i pulsanti del menu.",
            )
            .await?;
        }
    }

    respond(())
}

async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    config: Arc<Config>,
    pool: SqlitePool,
    sessions: SessionStore,
    location_sessions: LocationSessionStore,
    photo_sessions: PhotoSessionStore,
) -> ResponseResult<()> {
    bot.answer_callback_query(q.id.clone()).await?;

    let Some(message) = q.regular_message() else {
        return respond(());
    };
    let chat_id = message.chat.id;
    if !auth::is_authorized(chat_id.0, &config.allowed_chat_ids) {
        tracing::warn!(
            chat_id = chat_id.0,
            "Callback ignorata da chat non autorizzata"
        );
        return respond(());
    }

    let Some(data) = q.data.as_deref() else {
        return respond(());
    };

    match data {
        "menu:main" => {
            sessions.clear_chat(chat_id.0);
            location_sessions.clear_chat(chat_id.0);
            photo_sessions.clear_chat(chat_id.0);
            send_main_menu(&bot, chat_id).await?;
        }
        "menu:soon" => {
            bot.send_message(
                chat_id,
                "Questo modulo non è ancora implementato. Per ora sono disponibili 📦 Oggetti e 🏠 Case e stanze.",
            )
            .await?;
        }
        "system:status" => {
            send_status(&bot, chat_id, &pool).await?;
        }
        _ if data.starts_with("history:") => {
            sessions.clear_chat(chat_id.0);
            location_sessions.clear_chat(chat_id.0);
            photo_sessions.clear_chat(chat_id.0);
            if !modules::storico::handle_callback(&bot, chat_id, &pool, data).await? {
                bot.send_message(
                    chat_id,
                    "Pulsante storico non riconosciuto o non più valido.",
                )
                .await?;
            }
        }
        _ => {
            if data.starts_with("oggetti:") {
                photo_sessions.clear_chat(chat_id.0);
            }

            if modules::foto::handle_callback(&bot, chat_id, &pool, &photo_sessions, data).await? {
                location_sessions.clear_chat(chat_id.0);
                return respond(());
            }

            if modules::luoghi::handle_callback(&bot, chat_id, &pool, &location_sessions, data)
                .await?
            {
                sessions.clear_chat(chat_id.0);
                photo_sessions.clear_chat(chat_id.0);
                return respond(());
            }

            if !modules::oggetti::handle_callback(&bot, chat_id, &pool, &sessions, data).await? {
                bot.send_message(chat_id, "Pulsante non riconosciuto o non più valido.")
                    .await?;
            } else {
                location_sessions.clear_chat(chat_id.0);
            }
        }
    }

    respond(())
}

async fn send_online_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🟢 Gestionale Casa è online.\n\n🏠 Menu principale\nScegli una sezione.\n\nComandi rapidi: /oggetti · /luoghi · /storico · /status · /ping",
    )
    .reply_markup(modules::oggetti::main_menu_keyboard())
    .await?;
    Ok(())
}

async fn send_main_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🏠 Gestionale Casa\n\nScegli una sezione. I moduli non ancora disponibili sono indicati come prossimamente.\n\nComandi rapidi: /oggetti · /luoghi · /storico · /status · /ping",
    )
    .reply_markup(modules::oggetti::main_menu_keyboard())
    .await?;
    Ok(())
}

async fn send_status(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    match db::status(pool).await {
        Ok(status) => {
            let fk = if status.foreign_keys_enabled {
                "✅"
            } else {
                "❌"
            };
            let schema = if status.schema_core_present {
                "✅"
            } else {
                "❌"
            };
            let message = format!(
                "🏠 Gestionale Casa\n\n\
                 Bot Telegram: ✅\n\
                 Database SQLite: ✅\n\
                 Foreign key: {fk}\n\
                 Migrazioni applicate: {}\n\
                 Schema core: {schema}",
                status.applied_migrations
            );
            bot.send_message(chat_id, message)
                .reply_markup(status_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore durante la lettura dello stato");
            bot.send_message(
                chat_id,
                "⚠️ Il bot è online, ma non riesco a leggere lo stato del database.",
            )
            .reply_markup(status_keyboard())
            .await?;
        }
    }
    Ok(())
}

fn status_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "🏠 Menu principale".to_string(),
        "menu:main".to_string(),
    )]])
}

fn first_command(text: &str) -> Option<&str> {
    let token = text.split_whitespace().next()?;
    if !token.starts_with('/') {
        return None;
    }
    token.split('@').next()
}
