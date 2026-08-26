//! Backlog interno dei miglioramenti del gestionale.
//!
//! I miglioramenti appartengono all'utente che li crea e non a uno spazio
//! domestico. Tutti gli utenti Telegram approvati possono creare e leggere i
//! propri miglioramenti; gli amministratori di sistema possono leggere tutti
//! i miglioramenti e cambiarne lo stato.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    net::Download,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile},
};
use tokio::fs::File;

use crate::identity;

const MEDIA_ROOT: &str = "data/media/miglioramenti";
const LIST_LIMIT: i64 = 10;

#[derive(Clone, Default)]
pub struct ImprovementSessionStore {
    inner: Arc<Mutex<HashMap<i64, ImprovementConversationState>>>,
}

#[derive(Debug, Clone)]
enum ImprovementConversationState {
    Description,
    OptionalPhoto { description: String },
    ExistingPhoto { improvement_id: i64 },
}

impl ImprovementSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.with_sessions(|sessions| {
            sessions.remove(&chat_id);
        });
    }

    pub fn has_active(&self, chat_id: i64) -> bool {
        self.with_sessions(|sessions| sessions.contains_key(&chat_id))
    }

    fn get(&self, chat_id: i64) -> Option<ImprovementConversationState> {
        self.with_sessions(|sessions| sessions.get(&chat_id).cloned())
    }

    fn set(&self, chat_id: i64, state: ImprovementConversationState) {
        self.with_sessions(|sessions| {
            sessions.insert(chat_id, state);
        });
    }

    fn take(&self, chat_id: i64) -> Option<ImprovementConversationState> {
        self.with_sessions(|sessions| sessions.remove(&chat_id))
    }

    fn with_sessions<T>(
        &self,
        f: impl FnOnce(&mut HashMap<i64, ImprovementConversationState>) -> T,
    ) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

#[derive(Debug, Clone, FromRow)]
struct ImprovementRecord {
    id: i64,
    autore_nome: String,
    descrizione: String,
    modulo: Option<String>,
    stato: String,
    letto_admin_il: Option<String>,
    creato_il: String,
    allegati: i64,
}
#[derive(Debug, Clone, FromRow)]
struct ArchivedImprovementRecord {
    autore_nome: String,
    descrizione: String,
    archiviato_il: String,
    allegati: i64,
}
#[derive(Debug, Clone, FromRow)]
struct AttachmentRecord {
    id: i64,
    percorso_file: String,
    descrizione: Option<String>,
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
    text_hint: Option<&str>,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;
    let command = text_hint.and_then(first_command);

    if matches!(command, Some("/miglioramenti") | Some("/miglioramento")) {
        sessions.clear_chat(chat_id);
        show_menu(bot, msg.chat.id, pool).await?;
        return Ok(true);
    }

    if command == Some("/miglioramento_nuovo") {
        sessions.set(chat_id, ImprovementConversationState::Description);
        bot.send_message(
            msg.chat.id,
            "💡 Nuovo miglioramento\n\nDescrivi cosa vorresti migliorare nel gestionale.\n\nPuoi usare /annulla per uscire.",
        )
        .reply_markup(flow_cancel_keyboard())
        .await?;
        return Ok(true);
    }

    if command == Some("/annulla") && sessions.has_active(chat_id) {
        sessions.clear_chat(chat_id);
        bot.send_message(msg.chat.id, "❌ Inserimento miglioramento annullato.")
            .reply_markup(menu_keyboard(is_admin(pool).await.unwrap_or(false)))
            .await?;
        return Ok(true);
    }

    // Un altro comando esplicito appartiene al router principale: non lo
    // assorbiamo dentro una bozza miglioramento.
    if command.is_some() {
        return Ok(false);
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        ImprovementConversationState::Description => {
            let Some(text) = text_hint else {
                bot.send_message(msg.chat.id, "📝 Invia prima una descrizione testuale.")
                    .reply_markup(flow_cancel_keyboard())
                    .await?;
                return Ok(true);
            };
            if command.is_some() {
                return Ok(false);
            }
            let description = text.trim();
            if description.len() < 3 {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ La descrizione è troppo breve. Scrivi almeno qualche parola.",
                )
                .reply_markup(flow_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            if description.chars().count() > 2000 {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ La descrizione è troppo lunga. Mantienila entro 2000 caratteri.",
                )
                .reply_markup(flow_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            sessions.set(
                chat_id,
                ImprovementConversationState::OptionalPhoto {
                    description: description.to_string(),
                },
            );
            bot.send_message(
                msg.chat.id,
                "📷 Screenshot facoltativo\n\nSe vuoi, invia ora una foto o uno screenshot della chat.\nAltrimenti salva il miglioramento senza foto.",
            )
            .reply_markup(optional_photo_keyboard())
            .await?;
            Ok(true)
        }
        ImprovementConversationState::OptionalPhoto { description } => {
            if msg.photo().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "📷 Sto aspettando una foto/screenshot. In alternativa usa ✅ Salva senza foto o ❌ Annulla.",
                )
                .reply_markup(optional_photo_keyboard())
                .await?;
                return Ok(true);
            }

            let improvement_id = match create_improvement(pool, &description).await {
                Ok(id) => id,
                Err(error) => {
                    tracing::error!(?error, "Errore creazione miglioramento con foto");
                    bot.send_message(msg.chat.id, "⚠️ Non riesco a creare il miglioramento.")
                        .await?;
                    return Ok(true);
                }
            };

            match save_photo_attachment(bot, msg, pool, improvement_id).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Miglioramento salvato con screenshot.")
                        .reply_markup(after_save_keyboard(improvement_id))
                        .await?;
                }
                Err(error) => {
                    tracing::error!(?error, improvement_id, "Errore allegato miglioramento");
                    if let Err(delete_error) = delete_improvement(pool, improvement_id).await {
                        tracing::error!(
                            ?delete_error,
                            improvement_id,
                            "Rollback miglioramento fallito"
                        );
                    }
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Non sono riuscito a salvare lo screenshot. Il miglioramento non è stato registrato: puoi riprovare.",
                    )
                    .reply_markup(optional_photo_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
        ImprovementConversationState::ExistingPhoto { improvement_id } => {
            if msg.photo().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "📷 Invia una foto/screenshot oppure usa ❌ Annulla.",
                )
                .reply_markup(flow_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            match save_photo_attachment(bot, msg, pool, improvement_id).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Screenshot aggiunto al miglioramento.")
                        .reply_markup(after_save_keyboard(improvement_id))
                        .await?;
                }
                Err(error) => {
                    tracing::error!(?error, improvement_id, "Errore aggiunta screenshot");
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Non riesco a salvare lo screenshot. Riprova.",
                    )
                    .reply_markup(flow_cancel_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
    }
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    match data {
        "improve:menu" => {
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id, pool).await?;
            return Ok(true);
        }
        "improve:new" => {
            sessions.set(chat_id.0, ImprovementConversationState::Description);
            bot.send_message(
                chat_id,
                "💡 Nuovo miglioramento\n\nDescrivi cosa vorresti migliorare nel gestionale.\n\nPuoi usare /annulla per uscire.",
            )
            .reply_markup(flow_cancel_keyboard())
            .await?;
            return Ok(true);
        }
        "improve:cancel" => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, "❌ Operazione annullata.")
                .reply_markup(menu_keyboard(is_admin(pool).await.unwrap_or(false)))
                .await?;
            return Ok(true);
        }
        "improve:save:no_photo" => {
            let Some(ImprovementConversationState::OptionalPhoto { description }) =
                sessions.take(chat_id.0)
            else {
                bot.send_message(chat_id, "⚠️ Non c'è un miglioramento pronto da salvare.")
                    .await?;
                return Ok(true);
            };
            match create_improvement(pool, &description).await {
                Ok(id) => {
                    bot.send_message(chat_id, "✅ Miglioramento salvato.")
                        .reply_markup(after_save_keyboard(id))
                        .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore salvataggio miglioramento");
                    bot.send_message(chat_id, "⚠️ Non riesco a salvare il miglioramento.")
                        .await?;
                }
            }
            return Ok(true);
        }
        "improve:mine" => {
            sessions.clear_chat(chat_id.0);
            show_list(bot, chat_id, pool, false).await?;
            return Ok(true);
        }
        "improve:all" => {
            sessions.clear_chat(chat_id.0);
            if !is_admin(pool).await.unwrap_or(false) {
                bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                    .await?;
            } else {
                show_list(bot, chat_id, pool, true).await?;
            }
            return Ok(true);
        }
        "improve:archive:list" => {
            sessions.clear_chat(chat_id.0);
            show_archive(bot, chat_id, pool).await?;
            return Ok(true);
        }
        _ => {}
    }
    if let Some(id) = parse_id(data, "improve:view:") {
        sessions.clear_chat(chat_id.0);
        show_detail(bot, chat_id, pool, id).await?;
        return Ok(true);
    }
    if let Some(id) = parse_id(data, "improve:add_photo:") {
        if can_view(pool, id).await.unwrap_or(false) {
            sessions.set(
                chat_id.0,
                ImprovementConversationState::ExistingPhoto { improvement_id: id },
            );
            bot.send_message(
                chat_id,
                "📷 Invia ora lo screenshot da associare al miglioramento.",
            )
            .reply_markup(flow_cancel_keyboard())
            .await?;
        } else {
            bot.send_message(chat_id, "⚠️ Miglioramento non disponibile.")
                .await?;
        }
        return Ok(true);
    }
    if let Some(id) = parse_id(data, "improve:photos:") {
        show_attachments(bot, chat_id, pool, id).await?;
        return Ok(true);
    }
    if let Some(id) = parse_id(data, "improve:archive:") {
        if !is_admin(pool).await.unwrap_or(false) {
            bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                .await?;
        } else if let Err(error) = archive_completed_improvement(pool, id).await {
            tracing::warn!(?error, id, "Archiviazione miglioramento non riuscita");
            bot.send_message(chat_id, format!("⚠️ {error}")).await?;
        } else {
            bot.send_message(chat_id, "✅ Miglioramento completato e archiviato.")
                .reply_markup(menu_keyboard(true))
                .await?;
        }
        return Ok(true);
    }
    if let Some(id) = parse_id(data, "improve:delete_discarded:") {
        if !is_admin(pool).await.unwrap_or(false) {
            bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                .await?;
        } else if let Err(error) = delete_discarded_improvement(pool, id).await {
            tracing::warn!(
                ?error,
                id,
                "Eliminazione miglioramento scartato non riuscita"
            );
            bot.send_message(chat_id, format!("⚠️ {error}")).await?;
        } else {
            bot.send_message(chat_id, "🗑️ Miglioramento scartato eliminato.")
                .reply_markup(menu_keyboard(true))
                .await?;
        }
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("improve:status:") {
        let mut parts = rest.split(':');
        let id = parts.next().and_then(|value| value.parse::<i64>().ok());
        let state = parts.next();
        if parts.next().is_none() {
            if let (Some(id), Some(state)) = (id, state) {
                if !is_admin(pool).await.unwrap_or(false) {
                    bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                        .await?;
                } else if let Err(error) = set_status(pool, id, state).await {
                    tracing::warn!(?error, id, "Cambio stato miglioramento non riuscito");
                    bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                } else {
                    show_detail(bot, chat_id, pool, id).await?;
                }
                return Ok(true);
            }
        }
    }
    Ok(false)
}
pub async fn show_menu(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let admin = is_admin(pool).await.unwrap_or(false);
    bot.send_message(
        chat_id,
        "💡 Miglioramenti\n\nUsa questa sezione per annotare idee, problemi e dettagli UX mentre provi il bot. Puoi allegare screenshot della chat.",
    )
    .reply_markup(menu_keyboard(admin))
    .await?;
    Ok(())
}

async fn show_list(bot: &Bot, chat_id: ChatId, pool: &SqlitePool, all: bool) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        bot.send_message(chat_id, "⚠️ Miglioramenti non disponibili.")
            .await?;
        return Ok(());
    };

    if all && !is_admin(pool).await.unwrap_or(false) {
        bot.send_message(chat_id, "⚠️ Comando non disponibile.")
            .await?;
        return Ok(());
    }
    let rows: Result<Vec<ImprovementRecord>, sqlx::Error> = if all {
        sqlx::query_as(
            "SELECT m.id, u.nome_visualizzato AS autore_nome, \
                    m.descrizione, m.modulo, m.stato, m.letto_admin_il, \
                    strftime('%d/%m/%Y %H:%M', m.creato_il, 'localtime') AS creato_il, \
                    COUNT(a.id) AS allegati \
             FROM miglioramenti m \
             JOIN utenti u ON u.id = m.autore_utente_id \
             LEFT JOIN miglioramento_allegati a ON a.miglioramento_id = m.id \
             GROUP BY m.id \
             ORDER BY m.aggiornato_il DESC, m.id DESC LIMIT ?",
        )
        .bind(LIST_LIMIT)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as(
            "SELECT m.id, u.nome_visualizzato AS autore_nome, \
                    m.descrizione, m.modulo, m.stato, m.letto_admin_il, \
                    strftime('%d/%m/%Y %H:%M', m.creato_il, 'localtime') AS creato_il, \
                    COUNT(a.id) AS allegati \
             FROM miglioramenti m \
             JOIN utenti u ON u.id = m.autore_utente_id \
             LEFT JOIN miglioramento_allegati a ON a.miglioramento_id = m.id \
             WHERE m.autore_utente_id = ? \
             GROUP BY m.id \
             ORDER BY m.aggiornato_il DESC, m.id DESC LIMIT ?",
        )
        .bind(user_id)
        .bind(LIST_LIMIT)
        .fetch_all(pool)
        .await
    };
    match rows {
        Ok(rows) if rows.is_empty() => {
            bot.send_message(
                chat_id,
                if all {
                    "📋 Nessun miglioramento registrato."
                } else {
                    "📋 Non hai ancora registrato miglioramenti."
                },
            )
            .reply_markup(menu_keyboard(is_admin(pool).await.unwrap_or(false)))
            .await?;
        }
        Ok(rows) => {
            let mut lines = vec![if all {
                "📋 Miglioramenti recenti".to_string()
            } else {
                "📋 I miei miglioramenti".to_string()
            }];
            let mut buttons = Vec::new();
            for item in rows {
                let unread_suffix = if all && item.letto_admin_il.is_none() {
                    " 🆕"
                } else {
                    ""
                };
                lines.push(String::new());
                lines.push(format!(
                    "{} {}{}\n{}{}",
                    status_icon(&item.stato),
                    truncate(&item.descrizione, 90),
                    unread_suffix,
                    if all {
                        format!("👤 {} · ", item.autore_nome)
                    } else {
                        String::new()
                    },
                    if item.allegati > 0 {
                        format!("📷 {}", item.allegati)
                    } else {
                        "Nessuno screenshot".to_string()
                    }
                ));
                buttons.push(vec![InlineKeyboardButton::callback(
                    format!(
                        "{} {}{}",
                        status_icon(&item.stato),
                        truncate(&item.descrizione, 42),
                        unread_suffix
                    ),
                    format!("improve:view:{}", item.id),
                )]);
            }
            buttons.push(vec![InlineKeyboardButton::callback(
                "⬅️ Miglioramenti".to_string(),
                "improve:menu".to_string(),
            )]);
            buttons.push(vec![InlineKeyboardButton::callback(
                "🏠 Menu principale".to_string(),
                "menu:main".to_string(),
            )]);
            bot.send_message(chat_id, lines.join("\n"))
                .reply_markup(InlineKeyboardMarkup::new(buttons))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco miglioramenti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i miglioramenti.")
                .await?;
        }
    }
    Ok(())
}

async fn show_archive(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    if !is_admin(pool).await.unwrap_or(false) {
        bot.send_message(chat_id, "⚠️ Comando non disponibile.")
            .await?;
        return Ok(());
    }
    let rows: Result<Vec<ArchivedImprovementRecord>, sqlx::Error> = sqlx::query_as(
        "SELECT u.nome_visualizzato AS autore_nome, a.descrizione, \
                strftime('%d/%m/%Y %H:%M', a.archiviato_il, 'localtime') AS archiviato_il, \
                COUNT(aa.id) AS allegati \
         FROM miglioramenti_archivio a \
         JOIN utenti u ON u.id = a.autore_utente_id \
         LEFT JOIN miglioramento_archivio_allegati aa ON aa.miglioramento_archivio_id = a.id \
         GROUP BY a.id \
         ORDER BY a.archiviato_il DESC, a.id DESC LIMIT ?",
    )
    .bind(LIST_LIMIT)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) if rows.is_empty() => {
            bot.send_message(chat_id, "📦 L'archivio dei miglioramenti è vuoto.")
                .reply_markup(menu_keyboard(true))
                .await?;
        }
        Ok(rows) => {
            let mut lines = vec!["📦 Archivio miglioramenti completati".to_string()];
            for item in rows {
                lines.push(String::new());
                lines.push(format!(
                    "✅ {}\n👤 {} · Archiviato: {}{}",
                    truncate(&item.descrizione, 100),
                    item.autore_nome,
                    item.archiviato_il,
                    if item.allegati > 0 {
                        format!(" · 📷 {}", item.allegati)
                    } else {
                        String::new()
                    }
                ));
            }
            bot.send_message(chat_id, lines.join("\n"))
                .reply_markup(menu_keyboard(true))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore archivio miglioramenti");
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a leggere l'archivio dei miglioramenti.",
            )
            .reply_markup(menu_keyboard(true))
            .await?;
        }
    }
    Ok(())
}
async fn show_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    improvement_id: i64,
) -> ResponseResult<()> {
    let Some(item) = visible_improvement(pool, improvement_id)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(?error, improvement_id, "Errore dettaglio miglioramento");
            None
        })
    else {
        bot.send_message(chat_id, "⚠️ Miglioramento non disponibile.")
            .await?;
        return Ok(());
    };
    let admin = is_admin(pool).await.unwrap_or(false);
    if admin && item.letto_admin_il.is_none() {
        if let Err(error) = mark_read(pool, improvement_id).await {
            tracing::warn!(
                ?error,
                improvement_id,
                "Lettura miglioramento non registrata"
            );
        }
    }
    let module_line = item
        .modulo
        .as_deref()
        .map(|value| format!("\nSezione: {value}"))
        .unwrap_or_default();
    let message = format!(
        "💡 Miglioramento\n\n{}\n\nStato: {} {}\nAutore: {}\nCreato: {}{}\nScreenshot: {}",
        item.descrizione,
        status_icon(&item.stato),
        status_label(&item.stato),
        item.autore_nome,
        item.creato_il,
        module_line,
        item.allegati
    );
    bot.send_message(chat_id, message)
        .reply_markup(detail_keyboard(
            improvement_id,
            item.allegati,
            admin,
            &item.stato,
        ))
        .await?;
    Ok(())
}
async fn show_attachments(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    improvement_id: i64,
) -> ResponseResult<()> {
    if !can_view(pool, improvement_id).await.unwrap_or(false) {
        bot.send_message(chat_id, "⚠️ Miglioramento non disponibile.")
            .await?;
        return Ok(());
    }

    let attachments: Vec<AttachmentRecord> = match sqlx::query_as(
        "SELECT id, percorso_file, descrizione \
         FROM miglioramento_allegati WHERE miglioramento_id = ? ORDER BY creato_il, id",
    )
    .bind(improvement_id)
    .fetch_all(pool)
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(
                ?error,
                improvement_id,
                "Errore lettura screenshot miglioramento"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a leggere gli screenshot.")
                .await?;
            return Ok(());
        }
    };

    if attachments.is_empty() {
        bot.send_message(chat_id, "📷 Nessuno screenshot associato.")
            .reply_markup(after_save_keyboard(improvement_id))
            .await?;
        return Ok(());
    }

    for attachment in attachments {
        let path = PathBuf::from(&attachment.percorso_file);
        let caption = attachment
            .descrizione
            .unwrap_or_else(|| format!("📷 Screenshot #{}", attachment.id));
        if path.exists() {
            bot.send_photo(chat_id, InputFile::file(path))
                .caption(caption)
                .await?;
        } else {
            bot.send_message(
                chat_id,
                format!("⚠️ File screenshot non trovato: {caption}"),
            )
            .await?;
        }
    }

    bot.send_message(chat_id, "📷 Fine screenshot.")
        .reply_markup(after_save_keyboard(improvement_id))
        .await?;
    Ok(())
}

async fn create_improvement(pool: &SqlitePool, description: &str) -> Result<i64> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Miglioramento non disponibile per un attore di sistema")?;
    let admin = identity::is_system_admin(pool, &actor).await?;
    let state = if admin { "da_fare" } else { "da_approvare" };
    let result = sqlx::query(
        "INSERT INTO miglioramenti \
         (autore_utente_id, descrizione, stato, letto_admin_il) \
         VALUES (?, ?, ?, CASE WHEN ? = 1 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE NULL END)",
    )
    .bind(user_id)
    .bind(description.trim())
    .bind(state)
    .bind(if admin { 1_i64 } else { 0_i64 })
    .execute(pool)
    .await
    .context("Impossibile creare il miglioramento")?;
    Ok(result.last_insert_rowid())
}
async fn delete_improvement(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Miglioramento non disponibile per un attore di sistema")?;
    sqlx::query("DELETE FROM miglioramenti WHERE id = ? AND autore_utente_id = ?")
        .bind(improvement_id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Impossibile annullare il miglioramento")?;
    Ok(())
}

async fn save_photo_attachment(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    improvement_id: i64,
) -> Result<()> {
    if !can_view(pool, improvement_id).await? {
        bail!("Miglioramento non disponibile");
    }
    let photo_sizes = msg.photo().context("Foto Telegram non presente")?;
    let photo = photo_sizes
        .iter()
        .max_by_key(|photo| u64::from(photo.width) * u64::from(photo.height))
        .context("Foto Telegram non leggibile")?;
    let telegram_file = bot
        .get_file(photo.file.id.clone())
        .await
        .context("Impossibile leggere il file Telegram")?;
    let extension = safe_extension(&telegram_file.path);
    let directory = PathBuf::from(MEDIA_ROOT).join(improvement_id.to_string());
    tokio::fs::create_dir_all(&directory)
        .await
        .context("Impossibile creare la cartella screenshot")?;
    let filename = format!("telegram_{}_{}.{}", msg.chat.id.0, msg.id.0, extension);
    let local_path = directory.join(filename);
    let mut destination = File::create(&local_path)
        .await
        .context("Impossibile creare il file screenshot")?;
    if let Err(error) = bot
        .download_file(&telegram_file.path, &mut destination)
        .await
    {
        drop(destination);
        let _ = tokio::fs::remove_file(&local_path).await;
        return Err(error).context("Download screenshot Telegram fallito");
    }
    drop(destination);

    let description = msg
        .caption()
        .map(str::trim)
        .filter(|caption| !caption.is_empty());
    let path_for_db = local_path.to_string_lossy().into_owned();
    if let Err(error) = sqlx::query(
        "INSERT INTO miglioramento_allegati (miglioramento_id, percorso_file, descrizione) \
         VALUES (?, ?, ?)",
    )
    .bind(improvement_id)
    .bind(&path_for_db)
    .bind(description)
    .execute(pool)
    .await
    {
        let _ = tokio::fs::remove_file(&local_path).await;
        return Err(error).context("Impossibile registrare lo screenshot");
    }
    Ok(())
}

async fn visible_improvement(
    pool: &SqlitePool,
    improvement_id: i64,
) -> Result<Option<ImprovementRecord>> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(None);
    };
    let admin = identity::is_system_admin(pool, &actor).await?;
    let sql = if admin {
        "SELECT m.id, u.nome_visualizzato AS autore_nome, \
                m.descrizione, m.modulo, m.stato, m.letto_admin_il, \
                strftime('%d/%m/%Y %H:%M', m.creato_il, 'localtime') AS creato_il, \
                COUNT(a.id) AS allegati \
         FROM miglioramenti m \
         JOIN utenti u ON u.id = m.autore_utente_id \
         LEFT JOIN miglioramento_allegati a ON a.miglioramento_id = m.id \
         WHERE m.id = ? GROUP BY m.id"
    } else {
        "SELECT m.id, u.nome_visualizzato AS autore_nome, \
                m.descrizione, m.modulo, m.stato, m.letto_admin_il, \
                strftime('%d/%m/%Y %H:%M', m.creato_il, 'localtime') AS creato_il, \
                COUNT(a.id) AS allegati \
         FROM miglioramenti m \
         JOIN utenti u ON u.id = m.autore_utente_id \
         LEFT JOIN miglioramento_allegati a ON a.miglioramento_id = m.id \
         WHERE m.id = ? AND m.autore_utente_id = ? GROUP BY m.id"
    };
    let query = sqlx::query_as::<_, ImprovementRecord>(sql).bind(improvement_id);
    if admin {
        query
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere il miglioramento")
    } else {
        query
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere il miglioramento")
    }
}
async fn can_view(pool: &SqlitePool, improvement_id: i64) -> Result<bool> {
    Ok(visible_improvement(pool, improvement_id).await?.is_some())
}
async fn mark_read(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    if !is_admin(pool).await? {
        bail!("Operazione riservata agli amministratori");
    }
    sqlx::query(
        "UPDATE miglioramenti \
         SET letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ?",
    )
    .bind(improvement_id)
    .execute(pool)
    .await
    .context("Impossibile segnare il miglioramento come letto")?;
    Ok(())
}
async fn set_status(pool: &SqlitePool, improvement_id: i64, state: &str) -> Result<()> {
    if !is_admin(pool).await? {
        bail!("Operazione riservata agli amministratori");
    }
    if !matches!(state, "da_fare" | "scartato") {
        bail!("Stato miglioramento non valido");
    }
    let affected = sqlx::query(
        "UPDATE miglioramenti \
         SET stato = ?, \
             letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
    )
    .bind(state)
    .bind(improvement_id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare lo stato")?
    .rows_affected();
    if affected != 1 {
        bail!("Miglioramento non trovato");
    }
    Ok(())
}
async fn archive_completed_improvement(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    if !is_admin(pool).await? {
        bail!("Operazione riservata agli amministratori");
    }
    let actor = identity::current_actor();
    let admin_user_id = actor
        .utente_id
        .context("Amministratore privo di identità interna")?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione di archiviazione")?;
    let inserted = sqlx::query(
        "INSERT INTO miglioramenti_archivio (\
            miglioramento_origine_id, autore_utente_id, descrizione, modulo, creato_il, \
            completato_il, archiviato_da_utente_id\
         ) \
         SELECT id, autore_utente_id, descrizione, modulo, creato_il, \
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ? \
         FROM miglioramenti WHERE id = ? AND stato = 'da_fare'",
    )
    .bind(admin_user_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare il miglioramento")?;
    if inserted.rows_affected() != 1 {
        bail!("Il miglioramento deve essere nello stato Da fare prima dell'archiviazione");
    }
    let archive_id = inserted.last_insert_rowid();
    sqlx::query(
        "INSERT INTO miglioramento_archivio_allegati (\
            miglioramento_archivio_id, tipo, percorso_file, descrizione, creato_il\
         ) \
         SELECT ?, tipo, percorso_file, descrizione, creato_il \
         FROM miglioramento_allegati WHERE miglioramento_id = ?",
    )
    .bind(archive_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare gli allegati del miglioramento")?;
    sqlx::query("DELETE FROM miglioramenti WHERE id = ? AND stato = 'da_fare'")
        .bind(improvement_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile rimuovere il miglioramento dal backlog attivo")?;
    tx.commit()
        .await
        .context("Impossibile salvare l'archiviazione")?;
    Ok(())
}
async fn delete_discarded_improvement(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    if !is_admin(pool).await? {
        bail!("Operazione riservata agli amministratori");
    }
    let paths: Vec<String> = sqlx::query_scalar(
        "SELECT a.percorso_file \
         FROM miglioramento_allegati a \
         JOIN miglioramenti m ON m.id = a.miglioramento_id \
         WHERE m.id = ? AND m.stato = 'scartato'",
    )
    .bind(improvement_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli allegati da eliminare")?;
    let affected = sqlx::query("DELETE FROM miglioramenti WHERE id = ? AND stato = 'scartato'")
        .bind(improvement_id)
        .execute(pool)
        .await
        .context("Impossibile eliminare il miglioramento scartato")?
        .rows_affected();
    if affected != 1 {
        bail!("Il miglioramento non è scartato o non esiste");
    }
    for path in paths {
        if let Err(error) = tokio::fs::remove_file(&path).await {
            if Path::new(&path).exists() {
                tracing::warn!(?error, %path, "File allegato scartato non eliminato");
            }
        }
    }
    let directory = PathBuf::from(MEDIA_ROOT).join(improvement_id.to_string());
    if directory.exists() {
        if let Err(error) = tokio::fs::remove_dir_all(&directory).await {
            tracing::warn!(
                ?error,
                ?directory,
                "Cartella allegati scartati non eliminata"
            );
        }
    }
    Ok(())
}
async fn is_admin(pool: &SqlitePool) -> Result<bool> {
    identity::is_system_admin(pool, &identity::current_actor()).await
}

fn menu_keyboard(admin: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![InlineKeyboardButton::callback(
            "➕ Nuovo miglioramento".to_string(),
            "improve:new".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "📋 I miei miglioramenti".to_string(),
            "improve:mine".to_string(),
        )],
    ];
    if admin {
        rows.push(vec![InlineKeyboardButton::callback(
            "🗂️ Tutti i miglioramenti".to_string(),
            "improve:all".to_string(),
        )]);
        rows.push(vec![InlineKeyboardButton::callback(
            "📦 Archivio completati".to_string(),
            "improve:archive:list".to_string(),
        )]);
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "🏠 Menu principale".to_string(),
        "menu:main".to_string(),
    )]);
    InlineKeyboardMarkup::new(rows)
}
fn flow_cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("❌ Annulla".to_string(), "improve:cancel".to_string()),
        InlineKeyboardButton::callback("🏠 Menu principale".to_string(), "menu:main".to_string()),
    ]])
}

fn optional_photo_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✅ Salva senza foto".to_string(),
            "improve:save:no_photo".to_string(),
        )],
        vec![
            InlineKeyboardButton::callback("❌ Annulla".to_string(), "improve:cancel".to_string()),
            InlineKeyboardButton::callback(
                "🏠 Menu principale".to_string(),
                "menu:main".to_string(),
            ),
        ],
    ])
}

fn after_save_keyboard(improvement_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "💡 Apri miglioramento".to_string(),
            format!("improve:view:{improvement_id}"),
        )],
        vec![
            InlineKeyboardButton::callback(
                "⬅️ Miglioramenti".to_string(),
                "improve:menu".to_string(),
            ),
            InlineKeyboardButton::callback(
                "🏠 Menu principale".to_string(),
                "menu:main".to_string(),
            ),
        ],
    ])
}

fn detail_keyboard(
    improvement_id: i64,
    attachments: i64,
    admin: bool,
    state: &str,
) -> InlineKeyboardMarkup {
    let mut rows = vec![vec![InlineKeyboardButton::callback(
        "📷 Aggiungi screenshot".to_string(),
        format!("improve:add_photo:{improvement_id}"),
    )]];
    if attachments > 0 {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("🖼️ Vedi screenshot ({attachments})"),
            format!("improve:photos:{improvement_id}"),
        )]);
    }
    if admin {
        match state {
            "da_approvare" => rows.push(vec![
                InlineKeyboardButton::callback(
                    "✅ Approva".to_string(),
                    format!("improve:status:{improvement_id}:da_fare"),
                ),
                InlineKeyboardButton::callback(
                    "❌ Scarta".to_string(),
                    format!("improve:status:{improvement_id}:scartato"),
                ),
            ]),
            "da_fare" => rows.push(vec![
                InlineKeyboardButton::callback(
                    "✅ Archivia completato".to_string(),
                    format!("improve:archive:{improvement_id}"),
                ),
                InlineKeyboardButton::callback(
                    "❌ Scarta".to_string(),
                    format!("improve:status:{improvement_id}:scartato"),
                ),
            ]),
            "scartato" => rows.push(vec![InlineKeyboardButton::callback(
                "🗑️ Elimina scartato".to_string(),
                format!("improve:delete_discarded:{improvement_id}"),
            )]),
            _ => {}
        }
    }
    rows.push(vec![
        InlineKeyboardButton::callback("⬅️ Miglioramenti".to_string(), "improve:menu".to_string()),
        InlineKeyboardButton::callback("🏠 Menu principale".to_string(), "menu:main".to_string()),
    ]);
    InlineKeyboardMarkup::new(rows)
}
fn status_icon(value: &str) -> &'static str {
    match value {
        "da_approvare" => "🟡",
        "da_fare" => "🟢",
        "scartato" => "❌",
        _ => "•",
    }
}

fn status_label(value: &str) -> &'static str {
    match value {
        "da_approvare" => "Da approvare",
        "da_fare" => "Da fare",
        "scartato" => "Scartato",
        _ => "Sconosciuto",
    }
}
fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn parse_id(data: &str, prefix: &str) -> Option<i64> {
    data.strip_prefix(prefix)?.parse().ok()
}

fn first_command(text: &str) -> Option<&str> {
    let token = text.split_whitespace().next()?;
    if !token.starts_with('/') {
        return None;
    }
    Some(token.split('@').next().unwrap_or(token))
}

fn safe_extension(path: &str) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    if !extension.is_empty()
        && extension.len() <= 5
        && extension.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        extension
    } else {
        "jpg".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database in memoria");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign key");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration");
        pool
    }

    #[tokio::test]
    async fn struttura_miglioramenti_supporta_piu_allegati() {
        let pool = test_pool().await;
        let user_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Tester')")
            .execute(&pool)
            .await
            .expect("utente")
            .last_insert_rowid();
        let improvement_id = sqlx::query(
            "INSERT INTO miglioramenti (autore_utente_id, descrizione) VALUES (?, 'Migliorare pulsante')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("miglioramento")
        .last_insert_rowid();
        for path in ["data/media/a.jpg", "data/media/b.jpg"] {
            sqlx::query(
                "INSERT INTO miglioramento_allegati (miglioramento_id, percorso_file) VALUES (?, ?)",
            )
            .bind(improvement_id)
            .bind(path)
            .execute(&pool)
            .await
            .expect("allegato");
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM miglioramento_allegati WHERE miglioramento_id = ?",
        )
        .bind(improvement_id)
        .fetch_one(&pool)
        .await
        .expect("conteggio");
        assert_eq!(count, 2);
    }

    #[test]
    fn stati_e_troncamento_sono_stabili() {
        assert_eq!(status_label("da_approvare"), "Da approvare");
        assert_eq!(status_label("da_fare"), "Da fare");
        assert_eq!(truncate("abcdef", 3), "abc…");
        assert_eq!(truncate("abc", 3), "abc");
    }
}
