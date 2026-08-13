//! Punto di ingresso del Gestionale Casa.
//!
//! Step corrente: avvio del backend Telegram, caricamento sicuro della
//! configurazione e whitelist dei `chat_id`. Il database verrà collegato
//! nello step successivo.

mod auth;
mod config;
mod db;
mod modules;

use std::sync::Arc;

use anyhow::Context;
use config::Config;
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

    // Usiamo il token già validato da Config invece di Bot::from_env(),
    // così un errore di configurazione produce un messaggio controllato.
    let bot = Bot::new(config.telegram_token.clone());

    // get_me() verifica subito sia il token sia la raggiungibilità dell'API
    // Telegram. Se fallisce, il programma termina con un errore esplicito.
    let me = bot
        .get_me()
        .await
        .context("Impossibile collegarsi al bot Telegram")?;

    tracing::info!(bot_username = ?me.username(), "Gestionale Casa online");

    let handler = Update::filter_message().endpoint(handle_message);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![config])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, config: Arc<Config>) -> ResponseResult<()> {
    let chat_id = msg.chat.id.0;

    // Fail closed: una chat non presente in whitelist non riceve risposta e
    // soprattutto non può eseguire alcun comando.
    if !auth::is_authorized(chat_id, &config.allowed_chat_ids) {
        tracing::warn!(chat_id, "Messaggio ignorato da chat non autorizzata");
        return respond(());
    }

    let Some(text) = msg.text() else {
        return respond(());
    };

    // Consideriamo solo la prima parola e rimuoviamo l'eventuale suffisso
    // @nome_bot, utile se in futuro il bot verrà usato anche in un gruppo.
    let command = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .split('@')
        .next()
        .unwrap_or_default();

    match command {
        "/start" => {
            bot.send_message(
                msg.chat.id,
                "Gestionale Casa attivo.\n\nComandi disponibili:\n/ping",
            )
            .await?;
        }
        "/ping" => {
            bot.send_message(msg.chat.id, "Pong! Gestionale Casa è online.")
                .await?;
        }
        _ => {
            bot.send_message(
                msg.chat.id,
                "Comando non riconosciuto.\nUsa /ping per verificare il sistema.",
            )
            .await?;
        }
    }

    respond(())
}
