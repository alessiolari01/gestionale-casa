//! Infrastruttura trasversale dello storico - Step 6B.
//!
//! In questo sotto-step sono presenti le primitive di SCRITTURA realmente
//! usate da oggetti, foto e luoghi. Le API di lettura/paginazione verranno
//! aggiunte insieme alla UI Telegram dello storico, evitando codice morto.

use sqlx::{FromRow, SqliteConnection, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewHistoryEvent<'a> {
    pub(crate) entita_storico_id: i64,
    pub(crate) modulo: &'a str,
    pub(crate) componente: &'a str,
    pub(crate) operazione: &'a str,
    pub(crate) nome_entita_snapshot: &'a str,
    pub(crate) abitazione_storico_id: Option<i64>,
    pub(crate) abitazione_nome_snapshot: Option<&'a str>,
    pub(crate) stanza_storico_id: Option<i64>,
    pub(crate) stanza_nome_snapshot: Option<&'a str>,
    pub(crate) evento_padre_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewFieldChange {
    pub(crate) campo: &'static str,
    pub(crate) tipo_valore: &'static str,
    pub(crate) valore_prima: Option<String>,
    pub(crate) valore_dopo: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LocationSnapshot {
    pub(crate) abitazione_storico_id: Option<i64>,
    pub(crate) abitazione_nome: Option<String>,
    pub(crate) stanza_storico_id: Option<i64>,
    pub(crate) stanza_nome: Option<String>,
}

pub(crate) async fn ensure_entity(
    conn: &mut SqliteConnection,
    tipo_entita: &str,
    id_origine: i64,
    nome: &str,
) -> Result<i64, sqlx::Error> {
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM storico_entita \
         WHERE tipo_entita = ? AND id_origine = ? AND eliminato_il IS NULL \
         ORDER BY id DESC LIMIT 1",
    )
    .bind(tipo_entita)
    .bind(id_origine)
    .fetch_optional(&mut *conn)
    .await?
    {
        sqlx::query(
            "UPDATE storico_entita SET nome_ultimo = ? \
             WHERE id = ? AND nome_ultimo <> ?",
        )
        .bind(nome)
        .bind(id)
        .bind(nome)
        .execute(&mut *conn)
        .await?;
        return Ok(id);
    }

    let result = sqlx::query(
        "INSERT INTO storico_entita (tipo_entita, id_origine, nome_ultimo) \
         VALUES (?, ?, ?)",
    )
    .bind(tipo_entita)
    .bind(id_origine)
    .bind(nome)
    .execute(&mut *conn)
    .await?;

    Ok(result.last_insert_rowid())
}

pub(crate) async fn rename_entity(
    conn: &mut SqliteConnection,
    storico_entita_id: i64,
    nuovo_nome: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE storico_entita \
         SET nome_ultimo = ? \
         WHERE id = ? AND eliminato_il IS NULL",
    )
    .bind(nuovo_nome)
    .bind(storico_entita_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(crate) async fn mark_entity_deleted(
    conn: &mut SqliteConnection,
    storico_entita_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE storico_entita \
         SET eliminato_il = COALESCE(eliminato_il, CURRENT_TIMESTAMP) \
         WHERE id = ?",
    )
    .bind(storico_entita_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(crate) async fn record_event(
    conn: &mut SqliteConnection,
    event: &NewHistoryEvent<'_>,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO storico_eventi (\
            entita_storico_id, modulo, componente, operazione, \
            nome_entita_snapshot, \
            abitazione_storico_id, abitazione_nome_snapshot, \
            stanza_storico_id, stanza_nome_snapshot, evento_padre_id\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.entita_storico_id)
    .bind(event.modulo)
    .bind(event.componente)
    .bind(event.operazione)
    .bind(event.nome_entita_snapshot)
    .bind(event.abitazione_storico_id)
    .bind(event.abitazione_nome_snapshot)
    .bind(event.stanza_storico_id)
    .bind(event.stanza_nome_snapshot)
    .bind(event.evento_padre_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.last_insert_rowid())
}

pub(crate) async fn record_field_changes(
    conn: &mut SqliteConnection,
    evento_id: i64,
    changes: &[NewFieldChange],
) -> Result<(), sqlx::Error> {
    for (index, change) in changes.iter().enumerate() {
        if change.valore_prima == change.valore_dopo {
            continue;
        }

        sqlx::query(
            "INSERT INTO storico_cambiamenti (\
                evento_id, campo, tipo_valore, valore_prima, valore_dopo, ordine\
             ) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(evento_id)
        .bind(change.campo)
        .bind(change.tipo_valore)
        .bind(change.valore_prima.as_deref())
        .bind(change.valore_dopo.as_deref())
        .bind(index as i64)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub(crate) async fn record_location_change(
    conn: &mut SqliteConnection,
    evento_id: i64,
    before: &LocationSnapshot,
    after: &LocationSnapshot,
) -> Result<(), sqlx::Error> {
    if before == after {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO storico_cambi_luogo (\
            evento_id, \
            abitazione_prima_id, abitazione_prima_nome, \
            stanza_prima_id, stanza_prima_nome, \
            abitazione_dopo_id, abitazione_dopo_nome, \
            stanza_dopo_id, stanza_dopo_nome\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(evento_id)
    .bind(before.abitazione_storico_id)
    .bind(before.abitazione_nome.as_deref())
    .bind(before.stanza_storico_id)
    .bind(before.stanza_nome.as_deref())
    .bind(after.abitazione_storico_id)
    .bind(after.abitazione_nome.as_deref())
    .bind(after.stanza_storico_id)
    .bind(after.stanza_nome.as_deref())
    .execute(&mut *conn)
    .await?;

    Ok(())
}

const HISTORY_PAGE_SIZE: i64 = 6;

#[derive(Debug, Clone, FromRow)]
struct HistoryListRow {
    id: i64,
    tipo_entita: String,
    when_local: String,
    operazione: String,
    nome_entita_snapshot: String,
    abitazione_nome_snapshot: Option<String>,
    stanza_nome_snapshot: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct HistoryEventDetail {
    id: i64,
    tipo_entita: String,
    when_local: String,
    modulo: String,
    componente: String,
    operazione: String,
    nome_entita_snapshot: String,
    abitazione_nome_snapshot: Option<String>,
    stanza_nome_snapshot: Option<String>,
    evento_padre_id: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct HistoryFieldChangeUi {
    campo: String,
    tipo_valore: String,
    valore_prima: Option<String>,
    valore_dopo: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct HistoryLocationChangeUi {
    abitazione_prima_nome: Option<String>,
    stanza_prima_nome: Option<String>,
    abitazione_dopo_nome: Option<String>,
    stanza_dopo_nome: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryScope {
    Global,
    Item(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryAction {
    GlobalPage(i64),
    ItemPage {
        item_id: i64,
        page: i64,
    },
    GlobalEvent {
        event_id: i64,
        page: i64,
    },
    ItemEvent {
        item_id: i64,
        event_id: i64,
        page: i64,
    },
}

pub async fn show_global_history(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
) -> ResponseResult<()> {
    let requested_page = page.max(0);

    match load_global_history_page(pool, requested_page).await {
        Ok((events, total)) => {
            let pages = total_pages(total);
            let page = requested_page.min(pages.saturating_sub(1));
            let events = if events.is_empty() && total > 0 && page != requested_page {
                load_global_history_page(pool, page)
                    .await
                    .map(|(events, _)| events)
                    .unwrap_or_default()
            } else {
                events
            };

            bot.send_message(
                chat_id,
                format_history_list("📜 Storico globale", &events, page, total),
            )
            .reply_markup(history_list_keyboard(
                &events,
                page,
                total,
                HistoryScope::Global,
            ))
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore lettura storico globale");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere lo storico.")
                .reply_markup(history_home_keyboard())
                .await?;
        }
    }

    Ok(())
}

pub async fn show_item_history(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    item_id: i64,
    page: i64,
) -> ResponseResult<()> {
    let requested_page = page.max(0);

    let entity = match current_item_history_entity(pool, item_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            bot.send_message(
                chat_id,
                format!("Storico non disponibile: oggetto #{item_id} non trovato."),
            )
            .reply_markup(item_return_keyboard(item_id))
            .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, item_id, "Errore identita storico oggetto");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere lo storico dell'oggetto.")
                .reply_markup(item_return_keyboard(item_id))
                .await?;
            return Ok(());
        }
    };

    match load_entity_history_page(pool, entity.0, requested_page).await {
        Ok((events, total)) => {
            let pages = total_pages(total);
            let page = requested_page.min(pages.saturating_sub(1));
            let events = if events.is_empty() && total > 0 && page != requested_page {
                load_entity_history_page(pool, entity.0, page)
                    .await
                    .map(|(events, _)| events)
                    .unwrap_or_default()
            } else {
                events
            };

            bot.send_message(
                chat_id,
                format_history_list(
                    &format!("📜 Storico\n📦 {}", entity.1),
                    &events,
                    page,
                    total,
                ),
            )
            .reply_markup(history_list_keyboard(
                &events,
                page,
                total,
                HistoryScope::Item(item_id),
            ))
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, item_id, "Errore lettura storico oggetto");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere lo storico dell'oggetto.")
                .reply_markup(item_return_keyboard(item_id))
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    if data == "history:noop" {
        return Ok(true);
    }

    let Some(action) = parse_history_action(data) else {
        if data.starts_with("history:") {
            bot.send_message(
                chat_id,
                "Pulsante storico non valido o non più disponibile.",
            )
            .reply_markup(history_home_keyboard())
            .await?;
            return Ok(true);
        }
        return Ok(false);
    };

    match action {
        HistoryAction::GlobalPage(page) => {
            show_global_history(bot, chat_id, pool, page).await?;
        }
        HistoryAction::ItemPage { item_id, page } => {
            show_item_history(bot, chat_id, pool, item_id, page).await?;
        }
        HistoryAction::GlobalEvent { event_id, page } => {
            show_event_detail(bot, chat_id, pool, event_id, HistoryScope::Global, page).await?;
        }
        HistoryAction::ItemEvent {
            item_id,
            event_id,
            page,
        } => {
            show_event_detail(
                bot,
                chat_id,
                pool,
                event_id,
                HistoryScope::Item(item_id),
                page,
            )
            .await?;
        }
    }

    Ok(true)
}

async fn show_event_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    event_id: i64,
    scope: HistoryScope,
    page: i64,
) -> ResponseResult<()> {
    match load_event_detail(pool, event_id).await {
        Ok(Some((event, changes, location))) => {
            bot.send_message(
                chat_id,
                format_event_detail(&event, &changes, location.as_ref()),
            )
            .reply_markup(event_detail_keyboard(scope, page))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, format!("Evento storico #{event_id} non trovato."))
                .reply_markup(event_detail_keyboard(scope, page))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, event_id, "Errore dettaglio storico");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo evento.")
                .reply_markup(event_detail_keyboard(scope, page))
                .await?;
        }
    }

    Ok(())
}

async fn current_item_history_entity(
    pool: &SqlitePool,
    item_id: i64,
) -> Result<Option<(i64, String)>, sqlx::Error> {
    sqlx::query_as::<_, (i64, String)>(
        "SELECT se.id, i.nome \
         FROM storico_entita se \
         JOIN items i ON i.id = se.id_origine AND i.tipo = 'oggetto' \
         WHERE se.tipo_entita = 'oggetto' \
           AND se.id_origine = ? \
           AND se.eliminato_il IS NULL \
         ORDER BY se.id DESC LIMIT 1",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await
}

async fn load_global_history_page(
    pool: &SqlitePool,
    page: i64,
) -> Result<(Vec<HistoryListRow>, i64), sqlx::Error> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM storico_eventi")
        .fetch_one(pool)
        .await?;

    let events = sqlx::query_as::<_, HistoryListRow>(
        "SELECT e.id, se.tipo_entita, \
                strftime('%d/%m/%Y %H:%M', e.avvenuto_il, 'localtime') AS when_local, \
                e.operazione, e.nome_entita_snapshot, \
                e.abitazione_nome_snapshot, e.stanza_nome_snapshot \
         FROM storico_eventi e \
         JOIN storico_entita se ON se.id = e.entita_storico_id \
         ORDER BY e.avvenuto_il DESC, e.id DESC \
         LIMIT ? OFFSET ?",
    )
    .bind(HISTORY_PAGE_SIZE)
    .bind(page.max(0) * HISTORY_PAGE_SIZE)
    .fetch_all(pool)
    .await?;

    Ok((events, total))
}

async fn load_entity_history_page(
    pool: &SqlitePool,
    entity_id: i64,
    page: i64,
) -> Result<(Vec<HistoryListRow>, i64), sqlx::Error> {
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM storico_eventi WHERE entita_storico_id = ?")
            .bind(entity_id)
            .fetch_one(pool)
            .await?;

    let events = sqlx::query_as::<_, HistoryListRow>(
        "SELECT e.id, se.tipo_entita, \
                strftime('%d/%m/%Y %H:%M', e.avvenuto_il, 'localtime') AS when_local, \
                e.operazione, e.nome_entita_snapshot, \
                e.abitazione_nome_snapshot, e.stanza_nome_snapshot \
         FROM storico_eventi e \
         JOIN storico_entita se ON se.id = e.entita_storico_id \
         WHERE e.entita_storico_id = ? \
         ORDER BY e.avvenuto_il DESC, e.id DESC \
         LIMIT ? OFFSET ?",
    )
    .bind(entity_id)
    .bind(HISTORY_PAGE_SIZE)
    .bind(page.max(0) * HISTORY_PAGE_SIZE)
    .fetch_all(pool)
    .await?;

    Ok((events, total))
}

async fn load_event_detail(
    pool: &SqlitePool,
    event_id: i64,
) -> Result<
    Option<(
        HistoryEventDetail,
        Vec<HistoryFieldChangeUi>,
        Option<HistoryLocationChangeUi>,
    )>,
    sqlx::Error,
> {
    let event = sqlx::query_as::<_, HistoryEventDetail>(
        "SELECT e.id, se.tipo_entita, \
                strftime('%d/%m/%Y %H:%M', e.avvenuto_il, 'localtime') AS when_local, \
                e.modulo, e.componente, e.operazione, e.nome_entita_snapshot, \
                e.abitazione_nome_snapshot, e.stanza_nome_snapshot, e.evento_padre_id \
         FROM storico_eventi e \
         JOIN storico_entita se ON se.id = e.entita_storico_id \
         WHERE e.id = ?",
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;

    let Some(event) = event else {
        return Ok(None);
    };

    let changes = sqlx::query_as::<_, HistoryFieldChangeUi>(
        "SELECT campo, tipo_valore, valore_prima, valore_dopo \
         FROM storico_cambiamenti \
         WHERE evento_id = ? \
         ORDER BY ordine, id",
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?;

    let location = sqlx::query_as::<_, HistoryLocationChangeUi>(
        "SELECT abitazione_prima_nome, stanza_prima_nome, \
                abitazione_dopo_nome, stanza_dopo_nome \
         FROM storico_cambi_luogo \
         WHERE evento_id = ?",
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;

    Ok(Some((event, changes, location)))
}

fn format_history_list(title: &str, events: &[HistoryListRow], page: i64, total: i64) -> String {
    let pages = total_pages(total);
    let mut message = format!(
        "{title}\n\nPagina {} di {} · {} eventi",
        page + 1,
        pages,
        total
    );

    if events.is_empty() {
        message.push_str("\n\nNessun evento registrato.");
        return message;
    }

    for event in events {
        message.push_str("\n\n");
        message.push_str(&format!(
            "{}\n{} {} · {} {}",
            event.when_local,
            operation_icon(&event.operazione),
            operation_label(&event.operazione),
            entity_icon(&event.tipo_entita),
            event.nome_entita_snapshot,
        ));

        if let Some(location) = event_context(event) {
            message.push_str("\n📍 ");
            message.push_str(&location);
        }
    }

    message.push_str("\n\nTocca un evento sotto per vedere il dettaglio.");
    message
}

fn format_event_detail(
    event: &HistoryEventDetail,
    changes: &[HistoryFieldChangeUi],
    location: Option<&HistoryLocationChangeUi>,
) -> String {
    let mut message = format!(
        "📜 Dettaglio storico\n\n{}\n{} {}\n{} {}\nModulo: {} · {}\nEvento #{}",
        event.when_local,
        operation_icon(&event.operazione),
        operation_label(&event.operazione),
        entity_icon(&event.tipo_entita),
        event.nome_entita_snapshot,
        event.modulo,
        event.componente,
        event.id,
    );

    if let Some(parent) = event.evento_padre_id {
        message.push_str(&format!("\nCollegato all'evento #{parent}"));
    }

    if let Some(context) = detail_context(event) {
        message.push_str("\n📍 ");
        message.push_str(&context);
    }

    if !changes.is_empty() {
        message.push_str("\n\nCambiamenti:");
        for change in changes {
            let before =
                format_history_value(&change.tipo_valore, change.valore_prima.as_deref(), true);
            let after =
                format_history_value(&change.tipo_valore, change.valore_dopo.as_deref(), false);
            message.push_str(&format!(
                "\n• {}:\n  {} → {}",
                field_label(&change.campo),
                before,
                after
            ));
        }
    }

    if let Some(location) = location {
        message.push_str("\n\n🚚 Luogo:");
        message.push_str(&format!(
            "\n{} → {}",
            format_location(
                location.abitazione_prima_nome.as_deref(),
                location.stanza_prima_nome.as_deref(),
            ),
            format_location(
                location.abitazione_dopo_nome.as_deref(),
                location.stanza_dopo_nome.as_deref(),
            ),
        ));
    }

    message
}

fn history_list_keyboard(
    events: &[HistoryListRow],
    page: i64,
    total: i64,
    scope: HistoryScope,
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();

    for event in events {
        let callback = match scope {
            HistoryScope::Global => format!("history:event:g:{}:{page}", event.id),
            HistoryScope::Item(item_id) => {
                format!("history:event:i:{item_id}:{}:{page}", event.id)
            }
        };
        rows.push(vec![button(&event_button_label(event), &callback)]);
    }

    let pages = total_pages(total);
    if pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            let callback = match scope {
                HistoryScope::Global => format!("history:global:{}", page - 1),
                HistoryScope::Item(item_id) => format!("history:item:{item_id}:{}", page - 1),
            };
            nav.push(button("⬅️", &callback));
        }
        nav.push(button(&format!("{} / {}", page + 1, pages), "history:noop"));
        if page + 1 < pages {
            let callback = match scope {
                HistoryScope::Global => format!("history:global:{}", page + 1),
                HistoryScope::Item(item_id) => format!("history:item:{item_id}:{}", page + 1),
            };
            nav.push(button("➡️", &callback));
        }
        rows.push(nav);
    }

    match scope {
        HistoryScope::Global => rows.push(vec![button("🏠 Menu principale", "menu:main")]),
        HistoryScope::Item(item_id) => {
            rows.push(vec![button(
                "⬅️ Torna all'oggetto",
                &format!("oggetti:view:{item_id}"),
            )]);
            rows.push(vec![button("🏠 Menu principale", "menu:main")]);
        }
    }

    InlineKeyboardMarkup::new(rows)
}

fn event_detail_keyboard(scope: HistoryScope, page: i64) -> InlineKeyboardMarkup {
    let back = match scope {
        HistoryScope::Global => format!("history:global:{page}"),
        HistoryScope::Item(item_id) => format!("history:item:{item_id}:{page}"),
    };

    InlineKeyboardMarkup::new(vec![
        vec![button("⬅️ Torna allo storico", &back)],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn history_home_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button("🏠 Menu principale", "menu:main")]])
}

fn item_return_keyboard(item_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "⬅️ Torna all'oggetto",
            &format!("oggetti:view:{item_id}"),
        )],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn button(text: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.to_string(), data.to_string())
}

fn parse_history_action(data: &str) -> Option<HistoryAction> {
    let parts: Vec<&str> = data.split(':').collect();
    match parts.as_slice() {
        ["history", "global", page] => Some(HistoryAction::GlobalPage(parse_nonnegative(page)?)),
        ["history", "item", item_id, page] => Some(HistoryAction::ItemPage {
            item_id: parse_positive(item_id)?,
            page: parse_nonnegative(page)?,
        }),
        ["history", "event", "g", event_id, page] => Some(HistoryAction::GlobalEvent {
            event_id: parse_positive(event_id)?,
            page: parse_nonnegative(page)?,
        }),
        ["history", "event", "i", item_id, event_id, page] => Some(HistoryAction::ItemEvent {
            item_id: parse_positive(item_id)?,
            event_id: parse_positive(event_id)?,
            page: parse_nonnegative(page)?,
        }),
        _ => None,
    }
}

fn parse_positive(value: &str) -> Option<i64> {
    let value = value.parse::<i64>().ok()?;
    (value > 0).then_some(value)
}

fn parse_nonnegative(value: &str) -> Option<i64> {
    let value = value.parse::<i64>().ok()?;
    (value >= 0).then_some(value)
}

fn event_button_label(event: &HistoryListRow) -> String {
    format!(
        "{} {} · {}",
        operation_icon(&event.operazione),
        operation_label(&event.operazione),
        truncate_chars(&event.nome_entita_snapshot, 22)
    )
}

fn truncate_chars(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn event_context(event: &HistoryListRow) -> Option<String> {
    event
        .abitazione_nome_snapshot
        .as_deref()
        .map(|home| format_location(Some(home), event.stanza_nome_snapshot.as_deref()))
}

fn detail_context(event: &HistoryEventDetail) -> Option<String> {
    event
        .abitazione_nome_snapshot
        .as_deref()
        .map(|home| format_location(Some(home), event.stanza_nome_snapshot.as_deref()))
}

fn format_location(home: Option<&str>, room: Option<&str>) -> String {
    match (home, room) {
        (Some(home), Some(room)) => format!("{home} / {room}"),
        (Some(home), None) => home.to_string(),
        (None, _) => "Nessun luogo".to_string(),
    }
}

fn operation_icon(operation: &str) -> &'static str {
    match operation {
        "creazione" => "➕",
        "modifica" | "rinomina" => "✏️",
        "eliminazione" => "🗑",
        "assegnazione" => "📍",
        "spostamento" => "🚚",
        "rimozione" => "🧹",
        "foto_aggiunta" => "📷",
        _ => "📝",
    }
}

fn operation_label(operation: &str) -> &'static str {
    match operation {
        "creazione" => "Creato",
        "modifica" => "Modificato",
        "rinomina" => "Rinominato",
        "eliminazione" => "Eliminato",
        "assegnazione" => "Assegnato",
        "spostamento" => "Spostato",
        "rimozione" => "Luogo rimosso",
        "foto_aggiunta" => "Foto aggiunta",
        _ => "Evento",
    }
}

fn entity_icon(entity_type: &str) -> &'static str {
    match entity_type {
        "oggetto" => "📦",
        "abitazione" => "🏠",
        "stanza" => "🚪",
        "veicolo" => "🚗",
        "vestito" => "👕",
        _ => "🔹",
    }
}

fn field_label(field: &str) -> &str {
    match field {
        "nome" => "Nome",
        "descrizione" => "Descrizione",
        "marca" => "Marca",
        "modello" => "Modello",
        "numero_serie" => "Numero seriale",
        "posizione" => "Dettaglio posizione",
        "data_acquisto" => "Data acquisto",
        "prezzo_acquisto_centesimi" => "Prezzo acquisto",
        "venditore" => "Venditore",
        "valore_stimato_centesimi" => "Valore stimato",
        "condizione" => "Condizione",
        "note" => "Note",
        "foto_id" => "ID foto",
        "ruolo" => "Ruolo foto",
        "percorso_file" => "File",
        _ => field,
    }
}

fn format_history_value(value_type: &str, value: Option<&str>, before: bool) -> String {
    let Some(value) = value else {
        return if before {
            "non impostato".to_string()
        } else {
            "rimosso".to_string()
        };
    };

    match value_type {
        "denaro_centesimi" => value
            .parse::<i64>()
            .ok()
            .map(format_money_cents)
            .unwrap_or_else(|| value.to_string()),
        "testo" if matches!(value, "ottimo" | "buono" | "usurato" | "da_riparare") => {
            condition_label(value).to_string()
        }
        _ => value.to_string(),
    }
}

fn condition_label(value: &str) -> &str {
    match value {
        "ottimo" => "Ottimo",
        "buono" => "Buono",
        "usurato" => "Usurato",
        "da_riparare" => "Da riparare",
        _ => value,
    }
}

fn format_money_cents(cents: i64) -> String {
    let euros = cents / 100;
    let cents_part = cents.abs() % 100;
    format!("€{euros},{cents_part:02}")
}

fn total_pages(total: i64) -> i64 {
    ((total + HISTORY_PAGE_SIZE - 1) / HISTORY_PAGE_SIZE).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

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
    async fn migration_storico_non_inventa_eventi() {
        let pool = test_pool().await;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM storico_eventi")
            .fetch_one(&pool)
            .await
            .expect("conteggio storico");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn prima_dopo_viene_salvato_in_forma_strutturata() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("connessione");

        let entity_id = ensure_entity(&mut conn, "oggetto", 77, "Trapano")
            .await
            .expect("entita");

        let event_id = record_event(
            &mut conn,
            &NewHistoryEvent {
                entita_storico_id: entity_id,
                modulo: "oggetti",
                componente: "anagrafica",
                operazione: "modifica",
                nome_entita_snapshot: "Trapano",
                abitazione_storico_id: None,
                abitazione_nome_snapshot: None,
                stanza_storico_id: None,
                stanza_nome_snapshot: None,
                evento_padre_id: None,
            },
        )
        .await
        .expect("evento");

        record_field_changes(
            &mut conn,
            event_id,
            &[NewFieldChange {
                campo: "marca",
                tipo_valore: "testo",
                valore_prima: Some("Bosch".to_string()),
                valore_dopo: Some("Makita".to_string()),
            }],
        )
        .await
        .expect("cambiamento");

        let values: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT valore_prima, valore_dopo FROM storico_cambiamenti WHERE evento_id = ?",
        )
        .bind(event_id)
        .fetch_one(&mut *conn)
        .await
        .expect("lettura");

        assert_eq!(values.0.as_deref(), Some("Bosch"));
        assert_eq!(values.1.as_deref(), Some("Makita"));
    }

    #[test]
    fn callback_storico_vengono_parsati_senza_stato_in_memoria() {
        assert_eq!(
            parse_history_action("history:global:2"),
            Some(HistoryAction::GlobalPage(2))
        );
        assert_eq!(
            parse_history_action("history:item:7:1"),
            Some(HistoryAction::ItemPage {
                item_id: 7,
                page: 1
            })
        );
        assert_eq!(
            parse_history_action("history:event:g:15:2"),
            Some(HistoryAction::GlobalEvent {
                event_id: 15,
                page: 2
            })
        );
        assert_eq!(
            parse_history_action("history:event:i:7:15:2"),
            Some(HistoryAction::ItemEvent {
                item_id: 7,
                event_id: 15,
                page: 2
            })
        );
        assert_eq!(parse_history_action("history:item:0:1"), None);
    }

    #[test]
    fn valori_storico_sono_formattati_per_telegram() {
        assert_eq!(
            format_history_value("denaro_centesimi", Some("8990"), false),
            "€89,90"
        );
        assert_eq!(
            format_history_value("testo", Some("da_riparare"), false),
            "Da riparare"
        );
        assert_eq!(format_history_value("testo", None, true), "non impostato");
        assert_eq!(format_history_value("testo", None, false), "rimosso");
    }

    #[tokio::test]
    async fn lettura_ui_storico_restituisce_eventi_globali_e_individuali() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("connessione");

        let entity_id = ensure_entity(&mut conn, "oggetto", 55, "Trapano UI")
            .await
            .expect("entita");
        record_event(
            &mut conn,
            &NewHistoryEvent {
                entita_storico_id: entity_id,
                modulo: "oggetti",
                componente: "anagrafica",
                operazione: "creazione",
                nome_entita_snapshot: "Trapano UI",
                abitazione_storico_id: None,
                abitazione_nome_snapshot: None,
                stanza_storico_id: None,
                stanza_nome_snapshot: None,
                evento_padre_id: None,
            },
        )
        .await
        .expect("evento");
        drop(conn);

        let (global, global_total) = load_global_history_page(&pool, 0).await.expect("globale");
        assert_eq!(global_total, 1);
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].nome_entita_snapshot, "Trapano UI");

        let (individual, individual_total) = load_entity_history_page(&pool, entity_id, 0)
            .await
            .expect("individuale");
        assert_eq!(individual_total, 1);
        assert_eq!(individual.len(), 1);
        assert_eq!(individual[0].nome_entita_snapshot, "Trapano UI");
    }
}
