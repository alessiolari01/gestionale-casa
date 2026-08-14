//! Punto di ingresso del Gestionale Casa.
//!
//! Step corrente: Step 5A, primo modulo applicativo "Oggetti generici" sopra
//! l'infrastruttura Telegram + SQLx + SQLite verificata nello Step 4.

mod auth;
mod config;
mod db;
mod modules;

use std::sync::Arc;

use anyhow::Context;
use config::Config;
use modules::oggetti::SessionStore;
use sqlx::SqlitePool;
use teloxide::{dptree, prelude::*};

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

    let sessions = SessionStore::new();
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![config, pool, sessions])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    config: Arc<Config>,
    pool: SqlitePool,
    sessions: SessionStore,
) -> ResponseResult<()> {
    let chat_id = msg.chat.id.0;
    if !auth::is_authorized(chat_id, &config.allowed_chat_ids) {
        tracing::warn!(chat_id, "Messaggio ignorato da chat non autorizzata");
        return respond(());
    }

    let Some(text) = msg.text() else {
        return respond(());
    };

    if modules::oggetti::handle_message(&bot, &msg, &pool, &sessions, text).await? {
        return respond(());
    }

    let command = first_command(text);
    match command {
        Some("/start") => {
            sessions.clear_chat(chat_id);
            send_main_menu(&bot, msg.chat.id).await?;
        }
        Some("/ping") => {
            bot.send_message(msg.chat.id, "Pong! Gestionale Casa è online.")
                .await?;
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
            send_main_menu(&bot, chat_id).await?;
        }
        "menu:soon" => {
            bot.send_message(
                chat_id,
                "Questo modulo non è ancora implementato. Per ora è disponibile 📦 Oggetti.",
            )
            .await?;
        }
        "system:status" => {
            send_status(&bot, chat_id, &pool).await?;
        }
        _ => {
            if !modules::oggetti::handle_callback(&bot, chat_id, &pool, &sessions, data).await? {
                bot.send_message(chat_id, "Pulsante non riconosciuto o non più valido.")
                    .await?;
            }
        }
    }

    respond(())
}

async fn send_main_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🏠 Gestionale Casa\n\nScegli una sezione. I moduli non ancora disponibili sono indicati come prossimamente.\n\nComandi rapidi: /oggetti · /status · /ping",
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
            bot.send_message(chat_id, message).await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore durante la lettura dello stato");
            bot.send_message(
                chat_id,
                "⚠️ Il bot è online, ma non riesco a leggere lo stato del database.",
            )
            .await?;
        }
    }
    Ok(())
}

fn first_command(text: &str) -> Option<&str> {
    let token = text.split_whitespace().next()?;
    if !token.starts_with('/') {
        return None;
    }
    token.split('@').next()
}
