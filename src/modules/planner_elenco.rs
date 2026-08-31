//! Step 7.3B.1 - Elenco, creazione e archiviazione dei planner alimentari.
//!
//! Un planner e' un periodo di giorni in cui pianificare i pasti. Puo' essere
//! personale oppure legato a uno spazio condiviso, esattamente come i Profili
//! alimentari, e ne riusa le stesse regole di visibilita'.
//!
//! Questo modulo copre solo l'anagrafica del planner. La vista settimanale
//! arriva in 7.3B.2 e i pasti in 7.3B.3; il dominio delle date e degli snapshot
//! resta in `planner_alimentare`.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::{
    identity,
    modules::{
        planner_alimentare as dominio,
        storico::{self, NewFieldChange, NewHistoryEvent},
    },
};

type Bot = crate::context_bot::ContextBot;

const PLANNER_PAGE_SIZE: i64 = 5;
const SPACE_PAGE_SIZE: i64 = 5;
const PLANNER_NAME_MAX_CHARS: usize = 60;

#[derive(Clone, Default)]
pub struct PlannerSessionStore {
    inner: Arc<Mutex<HashMap<i64, PlannerConversationState>>>,
}

/// Bozza della creazione. Resta in memoria finche' l'utente non conferma:
/// nessuna riga viene scritta prima del salvataggio.
#[derive(Debug, Clone)]
enum PlannerConversationState {
    NewName,
    NewScope { name: String },
    NewPeriod { name: String, space_id: Option<i64> },
    Rename { planner_id: i64 },
}

impl PlannerSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, chat_id: i64) -> Option<PlannerConversationState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&chat_id)
            .cloned()
    }

    fn set(&self, chat_id: i64, state: PlannerConversationState) {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(chat_id, state);
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&chat_id);
    }

    pub fn has_active(&self, chat_id: i64) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains_key(&chat_id)
    }
}

#[derive(Debug, Clone, FromRow)]
struct PlannerRecord {
    id: i64,
    name: String,
    start_date: String,
    end_date: String,
    space_id: Option<i64>,
    space_name: Option<String>,
}

#[derive(Debug, Clone)]
struct PlannerPage {
    items: Vec<PlannerRecord>,
    total: i64,
    page: i64,
}

#[derive(Debug, Clone, FromRow)]
struct WritableSpaceRecord {
    id: i64,
    name: String,
}

#[derive(Debug, Clone)]
struct WritableSpacePage {
    items: Vec<WritableSpaceRecord>,
    total: i64,
    page: i64,
}

// ---------------------------------------------------------------------------
// Ingressi del modulo
// ---------------------------------------------------------------------------

pub async fn show_menu(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    show_list(bot, chat_id, pool, 0, false).await
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &PlannerSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;
    let command = first_command(text);

    if command == Some("/planner") {
        sessions.clear_chat(chat_id);
        show_menu(bot, msg.chat.id, pool).await?;
        return Ok(true);
    }

    if command == Some("/annulla") && sessions.has_active(chat_id) {
        let state = sessions.get(chat_id);
        sessions.clear_chat(chat_id);
        match state {
            Some(PlannerConversationState::Rename { planner_id }) => {
                show_detail(bot, msg.chat.id, pool, planner_id).await?;
            }
            _ => show_menu(bot, msg.chat.id, pool).await?,
        }
        return Ok(true);
    }

    if command.is_some() {
        sessions.clear_chat(chat_id);
        return Ok(false);
    }

    match sessions.get(chat_id) {
        Some(PlannerConversationState::NewName) => {
            match clean_name(text) {
                Ok(name) => {
                    sessions.set(chat_id, PlannerConversationState::NewScope { name });
                    show_scope_choice(bot, msg.chat.id, pool, 0).await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nScrivi un altro nome oppure premi ❌ Annulla."),
                    )
                    .reply_markup(cancel_new_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
        Some(PlannerConversationState::Rename { planner_id }) => {
            match rename_planner(pool, planner_id, text).await {
                Ok(changed) => {
                    sessions.clear_chat(chat_id);
                    let avviso = if changed {
                        "✅ Planner rinominato."
                    } else {
                        "ℹ️ Il nome è già quello indicato."
                    };
                    bot.send_message(msg.chat.id, avviso).await?;
                    show_detail(bot, msg.chat.id, pool, planner_id).await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nScrivi un altro nome oppure annulla."),
                    )
                    .reply_markup(rename_cancel_keyboard(planner_id))
                    .await?;
                }
            }
            Ok(true)
        }
        // Negli stati guidati dai pulsanti un testo libero non deve rompere la
        // schermata attiva: vale la regola dello Step 7.2H.4E.
        Some(_) | None => Ok(false),
    }
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &PlannerSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    if !data.starts_with("planner:") {
        return Ok(false);
    }

    match data {
        "planner:noop" => return Ok(true),
        "planner:menu" => {
            sessions.clear_chat(chat_id.0);
            show_list(bot, chat_id, pool, 0, false).await?;
            return Ok(true);
        }
        "planner:archived" => {
            sessions.clear_chat(chat_id.0);
            show_list(bot, chat_id, pool, 0, true).await?;
            return Ok(true);
        }
        "planner:new" => {
            sessions.set(chat_id.0, PlannerConversationState::NewName);
            bot.send_message(
                chat_id,
                "➕ Nuovo planner\n\n\
                 Scrivi il nome del planner.\n\
                 Esempio: Settimana in famiglia\n\n\
                 Premi ❌ Annulla per uscire.",
            )
            .reply_markup(cancel_new_keyboard())
            .await?;
            return Ok(true);
        }
        "planner:new:cancel" => {
            sessions.clear_chat(chat_id.0);
            show_list(bot, chat_id, pool, 0, false).await?;
            return Ok(true);
        }
        _ => {}
    }

    if let Some(page) = parse_suffix_i64(data, "planner:list:page:") {
        sessions.clear_chat(chat_id.0);
        show_list(bot, chat_id, pool, page, false).await?;
        return Ok(true);
    }
    if let Some(page) = parse_suffix_i64(data, "planner:archived:page:") {
        sessions.clear_chat(chat_id.0);
        show_list(bot, chat_id, pool, page, true).await?;
        return Ok(true);
    }
    if let Some(page) = parse_suffix_i64(data, "planner:new:scope:page:") {
        show_scope_choice(bot, chat_id, pool, page).await?;
        return Ok(true);
    }

    if let Some(scelta) = data.strip_prefix("planner:new:scope:") {
        let Some(PlannerConversationState::NewScope { name }) = sessions.get(chat_id.0) else {
            return expired_draft(bot, chat_id, pool, sessions).await;
        };
        let space_id = if scelta == "self" {
            None
        } else {
            match parse_positive_i64(scelta) {
                Some(value) => Some(value),
                None => return Ok(true),
            }
        };
        sessions.set(
            chat_id.0,
            PlannerConversationState::NewPeriod { name, space_id },
        );
        show_period_choice(bot, chat_id, pool).await?;
        return Ok(true);
    }

    if let Some(preset) = data.strip_prefix("planner:new:period:") {
        let Some(PlannerConversationState::NewPeriod { name, space_id }) = sessions.get(chat_id.0)
        else {
            return expired_draft(bot, chat_id, pool, sessions).await;
        };
        let today = match today(pool).await {
            Ok(value) => value,
            Err(error) => {
                tracing::error!(?error, "Impossibile leggere la data corrente");
                bot.send_message(chat_id, "⚠️ Non riesco a leggere la data di oggi.")
                    .reply_markup(list_return_keyboard())
                    .await?;
                return Ok(true);
            }
        };
        let Some((start, end)) = period_from_preset(preset, &today) else {
            return Ok(true);
        };
        match create_planner(pool, &name, space_id, &start, &end).await {
            Ok(planner_id) => {
                sessions.clear_chat(chat_id.0);
                bot.send_message(chat_id, "✅ Planner creato.").await?;
                show_detail(bot, chat_id, pool, planner_id).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(period_keyboard())
                    .await?;
            }
        }
        return Ok(true);
    }

    if let Some(planner_id) = parse_suffix_i64(data, "planner:view:") {
        sessions.clear_chat(chat_id.0);
        show_detail(bot, chat_id, pool, planner_id).await?;
        return Ok(true);
    }
    if let Some(planner_id) = parse_suffix_i64(data, "planner:manage:") {
        sessions.clear_chat(chat_id.0);
        show_manage(bot, chat_id, pool, planner_id).await?;
        return Ok(true);
    }
    if let Some(planner_id) = parse_suffix_i64(data, "planner:rename:") {
        if get_manageable_planner(pool, planner_id)
            .await
            .ok()
            .flatten()
            .is_none()
        {
            bot.send_message(chat_id, "⚠️ Non puoi modificare questo planner.")
                .reply_markup(list_return_keyboard())
                .await?;
            return Ok(true);
        }
        sessions.set(chat_id.0, PlannerConversationState::Rename { planner_id });
        bot.send_message(
            chat_id,
            "✏️ Rinomina planner\n\nScrivi il nuovo nome oppure premi ❌ Annulla.",
        )
        .reply_markup(rename_cancel_keyboard(planner_id))
        .await?;
        return Ok(true);
    }
    if let Some(planner_id) = parse_suffix_i64(data, "planner:archive:yes:") {
        sessions.clear_chat(chat_id.0);
        match archive_planner(pool, planner_id).await {
            Ok(()) => {
                bot.send_message(chat_id, "📦 Planner archiviato.").await?;
                show_list(bot, chat_id, pool, 0, false).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(list_return_keyboard())
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(planner_id) = parse_suffix_i64(data, "planner:archive:") {
        sessions.clear_chat(chat_id.0);
        show_archive_confirmation(bot, chat_id, pool, planner_id).await?;
        return Ok(true);
    }

    Ok(false)
}

// ---------------------------------------------------------------------------
// Schermate
// ---------------------------------------------------------------------------

async fn show_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
    archived: bool,
) -> ResponseResult<()> {
    match list_visible_planners(pool, page, archived).await {
        Ok(elenco) => {
            let intestazione = if archived {
                "📦 Planner archiviati"
            } else {
                "📅 Planner"
            };
            let corpo = if elenco.total == 0 {
                if archived {
                    "\n\nNessun planner archiviato.".to_string()
                } else {
                    "\n\nNon hai ancora un planner.\n\nUn planner è un periodo di giorni in cui organizzare i pasti: puoi tenerlo per te o condividerlo in uno spazio.".to_string()
                }
            } else {
                format!(
                    "\n\nScegli un planner oppure creane uno nuovo.\nTotale: {}",
                    elenco.total
                )
            };
            bot.send_message(chat_id, format!("{intestazione}{corpo}"))
                .reply_markup(list_keyboard(&elenco, archived))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore lettura elenco planner");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i planner.")
                .reply_markup(list_return_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_scope_choice(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
) -> ResponseResult<()> {
    match list_writable_spaces(pool, page).await {
        Ok(spazi) => {
            let testo = if spazi.total == 0 {
                "👤 Ambito del planner\n\nNon fai parte di spazi condivisi: il planner sarà personale."
                    .to_string()
            } else {
                "👤 Ambito del planner\n\nTienilo personale oppure legalo a uno spazio, così lo vedranno anche gli altri membri.".to_string()
            };
            bot.send_message(chat_id, testo)
                .reply_markup(scope_keyboard(&spazi))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore lettura spazi scrivibili");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i tuoi spazi.")
                .reply_markup(list_return_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_period_choice(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let anteprima = match today(pool).await {
        Ok(oggi) => {
            let mut righe = String::new();
            for (etichetta, preset) in [
                ("Questa settimana", "week"),
                ("Prossima settimana", "next"),
                ("Questo mese", "month"),
            ] {
                if let Some((inizio, fine)) = period_from_preset(preset, &oggi) {
                    righe.push_str(&format!(
                        "\n• {etichetta}: {}",
                        dominio::format_human_range(&inizio, &fine)
                    ));
                }
            }
            righe
        }
        Err(_) => String::new(),
    };

    bot.send_message(
        chat_id,
        format!(
            "🗓 Periodo del planner\n\nScegli quanto deve durare.{anteprima}\n\nLe settimane iniziano di lunedì."
        ),
    )
    .reply_markup(period_keyboard())
    .await?;
    Ok(())
}

async fn show_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    planner_id: i64,
) -> ResponseResult<()> {
    match get_visible_planner(pool, planner_id).await {
        Ok(Some(planner)) => {
            let gestibile = get_manageable_planner(pool, planner_id)
                .await
                .ok()
                .flatten()
                .is_some();
            bot.send_message(chat_id, planner_summary(&planner))
                .reply_markup(detail_keyboard(planner.id, gestibile))
                .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Planner non disponibile.")
                .reply_markup(list_return_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore lettura planner");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere il planner.")
                .reply_markup(list_return_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_manage(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    planner_id: i64,
) -> ResponseResult<()> {
    match get_manageable_planner(pool, planner_id).await {
        Ok(Some(planner)) => {
            bot.send_message(
                chat_id,
                format!("⚙️ Gestisci planner\n\n{}", planner_summary(&planner)),
            )
            .reply_markup(manage_keyboard(planner.id))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Non puoi modificare questo planner.")
                .reply_markup(list_return_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore verifica gestione planner");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere il planner.")
                .reply_markup(list_return_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_archive_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    planner_id: i64,
) -> ResponseResult<()> {
    match get_manageable_planner(pool, planner_id).await {
        Ok(Some(planner)) => {
            bot.send_message(
                chat_id,
                format!(
                    "📦 Archiviare «{}»?\n\nIl planner e i suoi pasti restano consultabili nell'archivio e non vengono eliminati.",
                    planner.name
                ),
            )
            .reply_markup(archive_confirmation_keyboard(planner.id))
            .await?;
        }
        _ => {
            bot.send_message(chat_id, "⚠️ Non puoi archiviare questo planner.")
                .reply_markup(list_return_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn expired_draft(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &PlannerSessionStore,
) -> ResponseResult<bool> {
    sessions.clear_chat(chat_id.0);
    bot.send_message(
        chat_id,
        "ℹ️ La creazione del planner non è più in corso. Ricominciamo dall'elenco.",
    )
    .await?;
    show_list(bot, chat_id, pool, 0, false).await?;
    Ok(true)
}

fn planner_summary(planner: &PlannerRecord) -> String {
    let ambito = match planner.space_name.as_deref() {
        Some(nome) => format!("👥 Spazio: {nome}"),
        None => "🔒 Personale".to_string(),
    };
    let giorni = dominio::days_between(&planner.start_date, &planner.end_date)
        .map(|valore| valore + 1)
        .unwrap_or_default();
    format!(
        "📅 {}\n\n🗓 {}\n📆 {giorni} giorni\n{ambito}",
        planner.name,
        dominio::format_human_range(&planner.start_date, &planner.end_date)
    )
}

fn planner_row_label(planner: &PlannerRecord) -> String {
    let simbolo = if planner.space_id.is_some() {
        "👥"
    } else {
        "🔒"
    };
    format!(
        "{simbolo} {} · {}",
        planner.name,
        dominio::format_human_range(&planner.start_date, &planner.end_date)
    )
}

// ---------------------------------------------------------------------------
// Tastiere
// ---------------------------------------------------------------------------

fn button(text: impl Into<String>, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), data.into())
}

fn navigation_row(back: &str) -> Vec<InlineKeyboardButton> {
    vec![
        button("⬅️ Indietro", back.to_string()),
        button("🏠 Menù principale", "menu:main"),
    ]
}

fn list_keyboard(page: &PlannerPage, archived: bool) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = page
        .items
        .iter()
        .map(|planner| {
            vec![button(
                planner_row_label(planner),
                format!("planner:view:{}", planner.id),
            )]
        })
        .collect();

    let prefisso = if archived {
        "planner:archived:page"
    } else {
        "planner:list:page"
    };
    push_pagination_row(&mut rows, page.page, page.total, prefisso);

    if archived {
        rows.push(navigation_row("planner:menu"));
    } else {
        rows.push(vec![button("➕ Nuovo planner", "planner:new")]);
        rows.push(vec![button("📦 Archiviati", "planner:archived")]);
        rows.push(navigation_row("food:menu"));
    }
    InlineKeyboardMarkup::new(rows)
}

fn scope_keyboard(spaces: &WritableSpacePage) -> InlineKeyboardMarkup {
    let mut rows = vec![vec![button("👤 Personale", "planner:new:scope:self")]];
    for spazio in &spaces.items {
        rows.push(vec![button(
            format!("👥 {}", spazio.name),
            format!("planner:new:scope:{}", spazio.id),
        )]);
    }
    push_pagination_row(
        &mut rows,
        spaces.page,
        spaces.total,
        "planner:new:scope:page",
    );
    rows.push(vec![button("❌ Annulla", "planner:new:cancel")]);
    InlineKeyboardMarkup::new(rows)
}

fn period_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("📆 Questa settimana", "planner:new:period:week"),
            button("📆 Prossima settimana", "planner:new:period:next"),
        ],
        vec![button("🗓 Questo mese", "planner:new:period:month")],
        vec![button("❌ Annulla", "planner:new:cancel")],
    ])
}

fn detail_keyboard(planner_id: i64, manageable: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if manageable {
        rows.push(vec![button(
            "⚙️ Gestisci",
            format!("planner:manage:{planner_id}"),
        )]);
    }
    rows.push(navigation_row("planner:menu"));
    InlineKeyboardMarkup::new(rows)
}

fn manage_keyboard(planner_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "✏️ Rinomina",
            format!("planner:rename:{planner_id}"),
        )],
        vec![button(
            "📦 Archivia",
            format!("planner:archive:{planner_id}"),
        )],
        navigation_row(&format!("planner:view:{planner_id}")),
    ])
}

fn archive_confirmation_keyboard(planner_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "📦 Sì, archivia",
            format!("planner:archive:yes:{planner_id}"),
        )],
        vec![button(
            "↩️ No, torna indietro",
            format!("planner:view:{planner_id}"),
        )],
    ])
}

fn cancel_new_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button("❌ Annulla", "planner:new:cancel")]])
}

fn rename_cancel_keyboard(planner_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button(
        "❌ Annulla",
        format!("planner:view:{planner_id}"),
    )]])
}

fn list_return_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![navigation_row("planner:menu")])
}

fn push_pagination_row(
    rows: &mut Vec<Vec<InlineKeyboardButton>>,
    page: i64,
    total: i64,
    callback_prefix: &str,
) {
    let pages = page_count(total, PLANNER_PAGE_SIZE);
    if pages <= 1 {
        return;
    }
    let mut riga = Vec::new();
    if page > 0 {
        riga.push(button(
            "⬅️ Pagina precedente",
            format!("{callback_prefix}:{}", page - 1),
        ));
    }
    riga.push(button(format!("{}/{}", page + 1, pages), "planner:noop"));
    if page + 1 < pages {
        riga.push(button(
            "Pagina successiva ➡️",
            format!("{callback_prefix}:{}", page + 1),
        ));
    }
    rows.push(riga);
}

// ---------------------------------------------------------------------------
// Accesso ai dati
// ---------------------------------------------------------------------------

async fn today(pool: &SqlitePool) -> Result<String> {
    sqlx::query_scalar("SELECT date('now', 'localtime')")
        .fetch_one(pool)
        .await
        .context("Impossibile leggere la data corrente")
}

fn period_from_preset(preset: &str, today: &str) -> Option<(String, String)> {
    match preset {
        "week" => dominio::week_range(today),
        "next" => {
            let prossima = dominio::add_days(today, 7)?;
            dominio::week_range(&prossima)
        }
        "month" => dominio::month_range(today),
        _ => None,
    }
}

async fn list_visible_planners(
    pool: &SqlitePool,
    requested_page: i64,
    archived: bool,
) -> Result<PlannerPage> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Identità utente non disponibile per i planner")?;
    let view_all = i64::from(actor.view_all);
    let archived_flag = i64::from(archived);

    const CONDIZIONE: &str = "p.archiviato = ? AND (p.proprietario_utente_id = ? OR (p.spazio_id IS NOT NULL AND EXISTS (SELECT 1 FROM membri_spazio ms WHERE ms.spazio_id = p.spazio_id AND ms.utente_id = ? AND (? = 1 OR ms.spazio_id = ?))))";

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM planner_alimentari p WHERE {CONDIZIONE}"
    ))
    .bind(archived_flag)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .fetch_one(pool)
    .await
    .context("Impossibile contare i planner")?;

    let pages = page_count(total, PLANNER_PAGE_SIZE);
    let page = requested_page.max(0).min(pages.saturating_sub(1));

    let items = sqlx::query_as::<_, PlannerRecord>(&format!(
        "SELECT p.id, p.nome AS name, p.data_inizio AS start_date, p.data_fine AS end_date, \
                p.spazio_id AS space_id, s.nome AS space_name \
         FROM planner_alimentari p \
         LEFT JOIN spazi s ON s.id = p.spazio_id \
         WHERE {CONDIZIONE} \
         ORDER BY p.data_inizio DESC, p.id DESC LIMIT ? OFFSET ?"
    ))
    .bind(archived_flag)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .bind(PLANNER_PAGE_SIZE)
    .bind(page * PLANNER_PAGE_SIZE)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i planner")?;

    Ok(PlannerPage { items, total, page })
}

async fn get_visible_planner(pool: &SqlitePool, planner_id: i64) -> Result<Option<PlannerRecord>> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Identità utente non disponibile per i planner")?;
    let view_all = i64::from(actor.view_all);

    sqlx::query_as::<_, PlannerRecord>(
        "SELECT p.id, p.nome AS name, p.data_inizio AS start_date, p.data_fine AS end_date, \
                p.spazio_id AS space_id, s.nome AS space_name \
         FROM planner_alimentari p \
         LEFT JOIN spazi s ON s.id = p.spazio_id \
         WHERE p.id = ? AND (p.proprietario_utente_id = ? OR (p.spazio_id IS NOT NULL AND EXISTS (\
             SELECT 1 FROM membri_spazio ms WHERE ms.spazio_id = p.spazio_id AND ms.utente_id = ? \
             AND (? = 1 OR ms.spazio_id = ?))))",
    )
    .bind(planner_id)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il planner")
}

/// Solo il proprietario amministra il planner. La collaborazione dei membri
/// dello spazio sui pasti arriva con i blocchi successivi.
async fn get_manageable_planner(
    pool: &SqlitePool,
    planner_id: i64,
) -> Result<Option<PlannerRecord>> {
    let user_id = current_user_id()?;
    sqlx::query_as::<_, PlannerRecord>(
        "SELECT p.id, p.nome AS name, p.data_inizio AS start_date, p.data_fine AS end_date, \
                p.spazio_id AS space_id, s.nome AS space_name \
         FROM planner_alimentari p \
         LEFT JOIN spazi s ON s.id = p.spazio_id \
         WHERE p.id = ? AND p.proprietario_utente_id = ? AND p.archiviato = 0",
    )
    .bind(planner_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile verificare la gestione del planner")
}

async fn list_writable_spaces(pool: &SqlitePool, requested_page: i64) -> Result<WritableSpacePage> {
    let user_id = current_user_id()?;
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM membri_spazio WHERE utente_id = ? AND ruolo IN ('proprietario', 'amministratore', 'membro')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile contare gli spazi disponibili")?;

    let pages = page_count(total, SPACE_PAGE_SIZE);
    let page = requested_page.max(0).min(pages.saturating_sub(1));

    let items = sqlx::query_as::<_, WritableSpaceRecord>(
        "SELECT s.id, s.nome AS name FROM membri_spazio ms JOIN spazi s ON s.id = ms.spazio_id \
         WHERE ms.utente_id = ? AND ms.ruolo IN ('proprietario', 'amministratore', 'membro') \
         ORDER BY CASE WHEN s.id = ? THEN 0 ELSE 1 END, s.nome COLLATE NOCASE, s.id \
         LIMIT ? OFFSET ?",
    )
    .bind(user_id)
    .bind(identity::current_actor().spazio_id)
    .bind(SPACE_PAGE_SIZE)
    .bind(page * SPACE_PAGE_SIZE)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi disponibili")?;

    Ok(WritableSpacePage { items, total, page })
}

async fn create_planner(
    pool: &SqlitePool,
    raw_name: &str,
    space_id: Option<i64>,
    start_date: &str,
    end_date: &str,
) -> Result<i64> {
    let user_id = current_user_id()?;
    let name = clean_name(raw_name)?;
    let normalized = normalize_name(&name);

    if dominio::parse_iso_date(start_date).is_none() || dominio::parse_iso_date(end_date).is_none()
    {
        bail!("Il periodo del planner non è valido");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la creazione del planner")?;

    if let Some(space_id) = space_id {
        ensure_writable_space(&mut tx, space_id, user_id).await?;
    }

    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM planner_alimentari WHERE archiviato = 0 \
         AND nome_normalizzato = ? AND data_inizio = ? AND data_fine = ? \
         AND ((? IS NULL AND spazio_id IS NULL AND proprietario_utente_id = ?) \
              OR (? IS NOT NULL AND spazio_id = ?)))",
    )
    .bind(&normalized)
    .bind(start_date)
    .bind(end_date)
    .bind(space_id)
    .bind(user_id)
    .bind(space_id)
    .bind(space_id)
    .fetch_one(&mut *tx)
    .await
    .context("Impossibile verificare i planner esistenti")?;
    if duplicate {
        bail!("Esiste già un planner attivo con questo nome e periodo");
    }

    let planner_id = sqlx::query(
        "INSERT INTO planner_alimentari (proprietario_utente_id, spazio_id, nome, nome_normalizzato, data_inizio, data_fine) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(space_id)
    .bind(&name)
    .bind(&normalized)
    .bind(start_date)
    .bind(end_date)
    .execute(&mut *tx)
    .await
    .context("Impossibile creare il planner")?
    .last_insert_rowid();

    record_history(
        &mut tx,
        planner_id,
        &name,
        "creazione",
        "planner",
        &[
            NewFieldChange {
                campo: "nome",
                tipo_valore: "testo",
                valore_prima: None,
                valore_dopo: Some(name.clone()),
            },
            NewFieldChange {
                campo: "periodo",
                tipo_valore: "testo",
                valore_prima: None,
                valore_dopo: Some(dominio::format_human_range(start_date, end_date)),
            },
        ],
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare la creazione del planner")?;
    Ok(planner_id)
}

async fn rename_planner(pool: &SqlitePool, planner_id: i64, raw_name: &str) -> Result<bool> {
    let name = clean_name(raw_name)?;
    let normalized = normalize_name(&name);

    let Some(planner) = get_manageable_planner(pool, planner_id).await? else {
        bail!("Non puoi modificare questo planner");
    };
    if planner.name == name {
        return Ok(false);
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la rinomina del planner")?;

    sqlx::query(
        "UPDATE planner_alimentari SET nome = ?, nome_normalizzato = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(&name)
    .bind(&normalized)
    .bind(planner_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile rinominare il planner")?;

    record_history(
        &mut tx,
        planner_id,
        &name,
        "modifica",
        "planner",
        &[NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: Some(planner.name.clone()),
            valore_dopo: Some(name.clone()),
        }],
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare la rinomina del planner")?;
    Ok(true)
}

async fn archive_planner(pool: &SqlitePool, planner_id: i64) -> Result<()> {
    let Some(planner) = get_manageable_planner(pool, planner_id).await? else {
        bail!("Non puoi archiviare questo planner");
    };

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare l'archiviazione del planner")?;

    sqlx::query(
        "UPDATE planner_alimentari SET archiviato = 1, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(planner_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare il planner")?;

    record_history(
        &mut tx,
        planner_id,
        &planner.name,
        "archiviazione",
        "archiviazione_planner",
        &[NewFieldChange {
            campo: "stato",
            tipo_valore: "testo",
            valore_prima: Some("Attivo".to_string()),
            valore_dopo: Some("Archiviato".to_string()),
        }],
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare l'archiviazione del planner")?;
    Ok(())
}

async fn ensure_writable_space(
    conn: &mut SqliteConnection,
    space_id: i64,
    user_id: i64,
) -> Result<()> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM membri_spazio WHERE spazio_id = ? AND utente_id = ? AND ruolo IN ('proprietario', 'amministratore', 'membro'))",
    )
    .bind(space_id)
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await
    .context("Impossibile verificare i permessi nello spazio")?;
    if !allowed {
        bail!("Non puoi creare un planner in questo spazio");
    }
    Ok(())
}

async fn record_history(
    conn: &mut SqliteConnection,
    planner_id: i64,
    planner_name: &str,
    operation: &'static str,
    component: &'static str,
    changes: &[NewFieldChange],
) -> Result<()> {
    let entity_id = storico::ensure_entity(conn, "planner", planner_id, planner_name)
        .await
        .context("Impossibile preparare lo storico del planner")?;
    let event_id = storico::record_event(
        conn,
        &NewHistoryEvent {
            entita_storico_id: entity_id,
            modulo: "alimentazione",
            componente: component,
            operazione: operation,
            nome_entita_snapshot: planner_name,
            abitazione_storico_id: None,
            abitazione_nome_snapshot: None,
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await
    .context("Impossibile registrare l'evento del planner nello storico")?;
    storico::record_field_changes(conn, event_id, changes)
        .await
        .context("Impossibile registrare i cambiamenti del planner nello storico")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Utilita'
// ---------------------------------------------------------------------------

fn current_user_id() -> Result<i64> {
    identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")
}

fn clean_name(raw: &str) -> Result<String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = name.chars().count();
    if len == 0 || len > PLANNER_NAME_MAX_CHARS {
        bail!("Il nome del planner deve contenere da 1 a {PLANNER_NAME_MAX_CHARS} caratteri");
    }
    Ok(name)
}

fn normalize_name(value: &str) -> String {
    value.to_lowercase()
}

fn page_count(total: i64, page_size: i64) -> i64 {
    if total <= 0 || page_size <= 0 {
        1
    } else {
        (total + page_size - 1) / page_size
    }
}

fn first_command(text: &str) -> Option<&str> {
    let first = text.split_whitespace().next()?;
    first.starts_with('/').then_some(first)
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|parsed| *parsed > 0)
}

fn parse_suffix_i64(data: &str, prefix: &str) -> Option<i64> {
    data.strip_prefix(prefix)?.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_ripulito_e_limitato() {
        assert_eq!(
            clean_name("  Settimana   in famiglia ").unwrap(),
            "Settimana in famiglia"
        );
        assert!(clean_name("   ").is_err());
        assert!(clean_name(&"a".repeat(PLANNER_NAME_MAX_CHARS + 1)).is_err());
        assert!(clean_name(&"a".repeat(PLANNER_NAME_MAX_CHARS)).is_ok());
    }

    #[test]
    fn preset_di_periodo() {
        assert_eq!(
            period_from_preset("week", "2026-09-03"),
            Some(("2026-08-31".to_string(), "2026-09-06".to_string()))
        );
        assert_eq!(
            period_from_preset("next", "2026-09-03"),
            Some(("2026-09-07".to_string(), "2026-09-13".to_string()))
        );
        assert_eq!(
            period_from_preset("month", "2026-09-03"),
            Some(("2026-09-01".to_string(), "2026-09-30".to_string()))
        );
        assert_eq!(period_from_preset("boh", "2026-09-03"), None);
    }

    #[test]
    fn pagine_calcolate_sul_totale() {
        assert_eq!(page_count(0, PLANNER_PAGE_SIZE), 1);
        assert_eq!(page_count(5, PLANNER_PAGE_SIZE), 1);
        assert_eq!(page_count(6, PLANNER_PAGE_SIZE), 2);
        assert_eq!(page_count(11, PLANNER_PAGE_SIZE), 3);
    }

    #[test]
    fn callback_riconosciute_solo_se_ben_formate() {
        assert_eq!(
            parse_suffix_i64("planner:view:12", "planner:view:"),
            Some(12)
        );
        assert_eq!(parse_suffix_i64("planner:view:x", "planner:view:"), None);
        assert_eq!(
            parse_suffix_i64("planner:list:page:0", "planner:view:"),
            None
        );
        assert_eq!(parse_positive_i64("0"), None);
        assert_eq!(parse_positive_i64("-3"), None);
        assert_eq!(parse_positive_i64("7"), Some(7));
    }

    #[test]
    fn comando_riconosciuto_solo_a_inizio_testo() {
        assert_eq!(first_command("/planner"), Some("/planner"));
        assert_eq!(first_command("/planner extra"), Some("/planner"));
        assert_eq!(first_command("ciao /planner"), None);
        assert_eq!(first_command(""), None);
    }
}
