//! Infrastruttura trasversale dello storico - Step 6B/6C + audit Step 7.1.
//!
//! Oltre ai cambiamenti strutturati e ai percorsi, ogni nuovo evento registra
//! spazio, autore e origine dell'azione. Gli eventi precedenti allo Step 7
//! restano esplicitamente `legacy` senza autore inventato.

use sqlx::{FromRow, QueryBuilder, Sqlite, SqliteConnection, SqlitePool};
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
    pub(crate) contenitore_storico_id: Option<i64>,
    pub(crate) contenitore_percorso: Option<String>,
}

async fn entity_owner_space_id(
    conn: &mut SqliteConnection,
    tipo_entita: &str,
    id_origine: i64,
) -> Result<Option<i64>, sqlx::Error> {
    match tipo_entita {
        "oggetto" | "vestito" | "veicolo" | "ricetta" => {
            sqlx::query_scalar("SELECT spazio_id FROM items WHERE id = ?")
                .bind(id_origine)
                .fetch_optional(&mut *conn)
                .await
        }
        "abitazione" => {
            sqlx::query_scalar("SELECT spazio_id FROM abitazioni WHERE id = ?")
                .bind(id_origine)
                .fetch_optional(&mut *conn)
                .await
        }
        "stanza" => {
            sqlx::query_scalar(
                "SELECT a.spazio_id FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id WHERE s.id = ?",
            )
            .bind(id_origine)
            .fetch_optional(&mut *conn)
            .await
        }
        "contenitore" => {
            sqlx::query_scalar(
                "SELECT a.spazio_id FROM contenitori c JOIN abitazioni a ON a.id = c.abitazione_id WHERE c.id = ?",
            )
            .bind(id_origine)
            .fetch_optional(&mut *conn)
            .await
        }
        "profilo_alimentare" => {
            sqlx::query_scalar(
                "SELECT ms.spazio_id FROM profili_alimentari pa \
                 JOIN membri_spazio ms ON ms.utente_id = pa.gestore_utente_id \
                 JOIN spazi s ON s.id = ms.spazio_id \
                 WHERE pa.id = ? \
                 ORDER BY CASE WHEN s.tipo = 'personale' THEN 0 ELSE 1 END, ms.spazio_id \
                 LIMIT 1",
            )
            .bind(id_origine)
            .fetch_optional(&mut *conn)
            .await
        }
        _ => Ok(None),
    }
}

pub(crate) async fn ensure_entity(
    conn: &mut SqliteConnection,
    tipo_entita: &str,
    id_origine: i64,
    nome: &str,
) -> Result<i64, sqlx::Error> {
    let actor = crate::identity::current_actor();
    let space_id = entity_owner_space_id(conn, tipo_entita, id_origine)
        .await?
        .unwrap_or(actor.spazio_id);
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM storico_entita \
         WHERE tipo_entita = ? AND id_origine = ? AND spazio_id = ? \
           AND eliminato_il IS NULL \
         ORDER BY id DESC LIMIT 1",
    )
    .bind(tipo_entita)
    .bind(id_origine)
    .bind(space_id)
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
        "INSERT INTO storico_entita (tipo_entita, id_origine, nome_ultimo, spazio_id) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(tipo_entita)
    .bind(id_origine)
    .bind(nome)
    .bind(space_id)
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

async fn historical_entity_space(
    conn: &mut SqliteConnection,
    storico_entita_id: Option<i64>,
) -> Result<(Option<i64>, Option<String>), sqlx::Error> {
    let Some(storico_entita_id) = storico_entita_id else {
        return Ok((None, None));
    };

    let row = sqlx::query_as::<_, (i64, String)>(
        "SELECT se.spazio_id, s.nome \
         FROM storico_entita se JOIN spazi s ON s.id = se.spazio_id \
         WHERE se.id = ?",
    )
    .bind(storico_entita_id)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(match row {
        Some((space_id, space_name)) => (Some(space_id), Some(space_name)),
        None => (None, None),
    })
}

pub(crate) async fn record_event(
    conn: &mut SqliteConnection,
    event: &NewHistoryEvent<'_>,
) -> Result<i64, sqlx::Error> {
    let actor = crate::identity::current_actor();
    let (event_space_id, event_space_name): (i64, String) = sqlx::query_as(
        "SELECT se.spazio_id, s.nome FROM storico_entita se JOIN spazi s ON s.id = se.spazio_id WHERE se.id = ?",
    )
    .bind(event.entita_storico_id)
    .fetch_one(&mut *conn)
    .await?;
    let (location_space_id, location_space_name) =
        historical_entity_space(conn, event.abitazione_storico_id).await?;
    let automatico = i64::from(event.evento_padre_id.is_some());
    let actor_name = actor.utente_id.map(|_| actor.nome_snapshot.as_str());
    let result = sqlx::query(
        "INSERT INTO storico_eventi (\
            entita_storico_id, modulo, componente, operazione, \
            nome_entita_snapshot, \
            abitazione_storico_id, abitazione_nome_snapshot, \
            stanza_storico_id, stanza_nome_snapshot, evento_padre_id, \
            spazio_id, spazio_nome_snapshot, luogo_spazio_id, luogo_spazio_nome_snapshot, \
            attore_utente_id, attore_nome_snapshot, origine_azione, automatico\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(event_space_id)
    .bind(&event_space_name)
    .bind(location_space_id)
    .bind(location_space_name.as_deref())
    .bind(actor.utente_id)
    .bind(actor_name)
    .bind(actor.origine)
    .bind(automatico)
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

pub(crate) async fn record_event_location_context(
    conn: &mut SqliteConnection,
    evento_id: i64,
    location: &LocationSnapshot,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE storico_eventi SET \
            contenitore_storico_id = ?, contenitore_percorso_snapshot = ? \
         WHERE id = ?",
    )
    .bind(location.contenitore_storico_id)
    .bind(location.contenitore_percorso.as_deref())
    .bind(evento_id)
    .execute(&mut *conn)
    .await?;
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

    let (before_space_id, before_space_name) =
        historical_entity_space(conn, before.abitazione_storico_id).await?;
    let (after_space_id, after_space_name) =
        historical_entity_space(conn, after.abitazione_storico_id).await?;

    sqlx::query(
        "INSERT INTO storico_cambi_luogo (\
            evento_id, \
            spazio_prima_id, spazio_prima_nome, \
            abitazione_prima_id, abitazione_prima_nome, \
            stanza_prima_id, stanza_prima_nome, \
            contenitore_prima_id, contenitore_prima_percorso, \
            spazio_dopo_id, spazio_dopo_nome, \
            abitazione_dopo_id, abitazione_dopo_nome, \
            stanza_dopo_id, stanza_dopo_nome, \
            contenitore_dopo_id, contenitore_dopo_percorso\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(evento_id)
    .bind(before_space_id)
    .bind(before_space_name.as_deref())
    .bind(before.abitazione_storico_id)
    .bind(before.abitazione_nome.as_deref())
    .bind(before.stanza_storico_id)
    .bind(before.stanza_nome.as_deref())
    .bind(before.contenitore_storico_id)
    .bind(before.contenitore_percorso.as_deref())
    .bind(after_space_id)
    .bind(after_space_name.as_deref())
    .bind(after.abitazione_storico_id)
    .bind(after.abitazione_nome.as_deref())
    .bind(after.stanza_storico_id)
    .bind(after.stanza_nome.as_deref())
    .bind(after.contenitore_storico_id)
    .bind(after.contenitore_percorso.as_deref())
    .execute(&mut *conn)
    .await?;

    Ok(())
}

type Bot = crate::context_bot::ContextBot;

const HISTORY_PAGE_SIZE: i64 = 5;

#[derive(Debug, Clone, FromRow)]
struct HistoryListRow {
    id: i64,
    tipo_entita: String,
    when_local: String,
    operazione: String,
    componente: String,
    nome_entita_snapshot: String,
    abitazione_nome_snapshot: Option<String>,
    stanza_nome_snapshot: Option<String>,
    contenitore_percorso_snapshot: Option<String>,
    spazio_nome_snapshot: Option<String>,
    luogo_spazio_nome_snapshot: Option<String>,
    attore_nome_snapshot: Option<String>,
    origine_azione: String,
    automatico: i64,
}

#[derive(Debug, Clone, FromRow)]
struct HistoryEventDetail {
    tipo_entita: String,
    when_local: String,
    modulo: String,
    componente: String,
    operazione: String,
    nome_entita_snapshot: String,
    abitazione_nome_snapshot: Option<String>,
    stanza_nome_snapshot: Option<String>,
    contenitore_percorso_snapshot: Option<String>,
    luogo_spazio_nome_snapshot: Option<String>,
    evento_padre_id: Option<i64>,
    attore_nome_snapshot: Option<String>,
    spazio_nome_snapshot: Option<String>,
    origine_azione: String,
    automatico: i64,
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
    spazio_prima_nome: Option<String>,
    abitazione_prima_nome: Option<String>,
    stanza_prima_nome: Option<String>,
    contenitore_prima_percorso: Option<String>,
    spazio_dopo_nome: Option<String>,
    abitazione_dopo_nome: Option<String>,
    stanza_dopo_nome: Option<String>,
    contenitore_dopo_percorso: Option<String>,
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

const HISTORY_FILTER_PICKER_PAGE_SIZE: i64 = 7;
const BASE62: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HistoryPeriodFilter {
    #[default]
    All,
    Today,
    Last7Days,
    Last30Days,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HistoryModuleFilter {
    #[default]
    All,
    Oggetti,
    Luoghi,
    Alimentazione,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HistoryOperationFilter {
    #[default]
    All,
    Creazione,
    Modifica,
    Eliminazione,
    Assegnazione,
    Spostamento,
    Rimozione,
    FotoAggiunta,
    Rinomina,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct HistoryFilters {
    period: HistoryPeriodFilter,
    module: HistoryModuleFilter,
    operation: HistoryOperationFilter,
    home_id: Option<i64>,
    room_id: Option<i64>,
    entity_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryFilterKind {
    Period,
    Module,
    Operation,
    Home,
    Room,
    Entity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GlobalHistoryAction {
    Page {
        page: i64,
        filters: HistoryFilters,
    },
    Event {
        event_id: i64,
        page: i64,
        filters: HistoryFilters,
    },
    Filters {
        filters: HistoryFilters,
    },
    Picker {
        kind: HistoryFilterKind,
        page: i64,
        filters: HistoryFilters,
    },
    Select {
        kind: HistoryFilterKind,
        value: String,
        filters: HistoryFilters,
    },
}

#[derive(Debug, Clone, FromRow)]
struct HistoryPickerOption {
    id: i64,
    label: String,
    subtitle: Option<String>,
    deleted: i64,
}

async fn show_global_history_filtered(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
    filters: HistoryFilters,
) -> ResponseResult<()> {
    let requested_page = page.max(0);

    match load_filtered_global_history_page(pool, requested_page, filters).await {
        Ok((events, total)) => {
            let pages = total_pages(total);
            let page = requested_page.min(pages.saturating_sub(1));
            let events = if events.is_empty() && total > 0 && page != requested_page {
                load_filtered_global_history_page(pool, page, filters)
                    .await
                    .map(|(events, _)| events)
                    .unwrap_or_default()
            } else {
                events
            };

            let summary = history_filters_summary(pool, filters).await;
            let title = if filters.is_default() {
                "📜 Storico globale".to_string()
            } else {
                format!("📜 Storico globale\n🔎 {summary}")
            };

            bot.send_message(chat_id, format_history_list(&title, &events, page, total))
                .reply_markup(global_history_keyboard(&events, page, total, filters))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, ?filters, "Errore lettura storico globale filtrato");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere lo storico.")
                .reply_markup(history_home_keyboard())
                .await?;
        }
    }

    Ok(())
}

async fn show_global_event_detail_filtered(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    event_id: i64,
    page: i64,
    filters: HistoryFilters,
) -> ResponseResult<()> {
    match load_event_detail(pool, event_id).await {
        Ok(Some((event, changes, location))) => {
            bot.send_message(
                chat_id,
                format_event_detail(&event, &changes, location.as_ref()),
            )
            .reply_markup(global_event_detail_keyboard(page, filters))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "Evento storico non trovato.")
                .reply_markup(global_event_detail_keyboard(page, filters))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, event_id, "Errore dettaglio storico filtrato");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo evento.")
                .reply_markup(global_event_detail_keyboard(page, filters))
                .await?;
        }
    }
    Ok(())
}

async fn show_history_filter_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    filters: HistoryFilters,
) -> ResponseResult<()> {
    let summary = history_filters_summary(pool, filters).await;
    let text = if filters.is_default() {
        "🔎 Filtri storico\n\nNessun filtro attivo.\nPuoi combinarne più di uno.".to_string()
    } else {
        format!("🔎 Filtri storico\n\nAttivi:\n{summary}\n\nPuoi combinarne più di uno.")
    };

    let home = history_filter_entity_name(pool, filters.home_id).await;
    let room = history_filter_entity_name(pool, filters.room_id).await;
    let entity = history_filter_entity_name(pool, filters.entity_id).await;
    let token = filters.to_token();

    let mut rows = vec![
        vec![button(
            &format!("🗓 Periodo: {}", filters.period.label()),
            &format!("h:p:p:0:{token}"),
        )],
        vec![button(
            &format!("🧩 Modulo: {}", filters.module.label()),
            &format!("h:p:m:0:{token}"),
        )],
        vec![button(
            &format!("⚙️ Operazione: {}", filters.operation.label()),
            &format!("h:p:o:0:{token}"),
        )],
        vec![button(
            &format!("🏠 Casa: {}", home.as_deref().unwrap_or("Tutte")),
            &format!("h:p:h:0:{token}"),
        )],
        vec![button(
            &format!("🚪 Stanza: {}", room.as_deref().unwrap_or("Tutte")),
            &format!("h:p:r:0:{token}"),
        )],
        vec![button(
            &format!("🎯 Elemento: {}", entity.as_deref().unwrap_or("Tutti")),
            &format!("h:p:e:0:{token}"),
        )],
    ];

    if !filters.is_default() {
        rows.push(vec![button(
            "🧹 Azzera tutti i filtri",
            &format!("h:f:{}", HistoryFilters::default().to_token()),
        )]);
    }

    rows.push(vec![button(
        "✅ Mostra risultati",
        &format!("h:g:0:{token}"),
    )]);
    rows.push(vec![button("🏠 Menù principale", "menu:main")]);

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_history_filter_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    kind: HistoryFilterKind,
    page: i64,
    filters: HistoryFilters,
) -> ResponseResult<()> {
    match kind {
        HistoryFilterKind::Period => {
            bot.send_message(chat_id, "🗓 Filtra per periodo")
                .reply_markup(static_filter_keyboard(
                    kind,
                    filters,
                    &[
                        ("0", "Tutto"),
                        ("t", "Oggi"),
                        ("7", "Ultimi 7 giorni"),
                        ("3", "Ultimi 30 giorni"),
                    ],
                ))
                .await?;
        }
        HistoryFilterKind::Module => {
            bot.send_message(chat_id, "🧩 Filtra per modulo/sezione")
                .reply_markup(static_filter_keyboard(
                    kind,
                    filters,
                    &[
                        ("0", "Tutti"),
                        ("o", "Oggetti"),
                        ("l", "Luoghi"),
                        ("a", "Alimentazione"),
                    ],
                ))
                .await?;
        }
        HistoryFilterKind::Operation => {
            bot.send_message(chat_id, "⚙️ Filtra per tipo di operazione")
                .reply_markup(static_filter_keyboard(
                    kind,
                    filters,
                    &[
                        ("0", "Tutte"),
                        ("c", "➕ Creazione"),
                        ("m", "✏️ Modifica"),
                        ("d", "🗑 Eliminazione"),
                        ("a", "📍 Assegnazione"),
                        ("s", "🚚 Spostamento"),
                        ("r", "🧹 Rimozione luogo"),
                        ("f", "📷 Foto aggiunta"),
                        ("n", "✏️ Rinomina"),
                    ],
                ))
                .await?;
        }
        HistoryFilterKind::Home | HistoryFilterKind::Room | HistoryFilterKind::Entity => {
            match load_history_picker_options(pool, kind, page).await {
                Ok((options, total)) => {
                    let title = match kind {
                        HistoryFilterKind::Home => "🏠 Filtra per casa",
                        HistoryFilterKind::Room => "🚪 Filtra per stanza",
                        HistoryFilterKind::Entity => "🎯 Filtra per elemento specifico",
                        _ => unreachable!(),
                    };
                    bot.send_message(chat_id, title)
                        .reply_markup(dynamic_filter_keyboard(
                            kind, filters, &options, page, total,
                        ))
                        .await?;
                }
                Err(error) => {
                    tracing::error!(?error, ?kind, "Errore opzioni filtro storico");
                    bot.send_message(chat_id, "⚠️ Non riesco a caricare queste opzioni.")
                        .reply_markup(filter_back_keyboard(filters))
                        .await?;
                }
            }
        }
    }
    Ok(())
}

async fn handle_global_history_action(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    action: GlobalHistoryAction,
) -> ResponseResult<()> {
    match action {
        GlobalHistoryAction::Page { page, filters } => {
            show_global_history_filtered(bot, chat_id, pool, page, filters).await?;
        }
        GlobalHistoryAction::Event {
            event_id,
            page,
            filters,
        } => {
            show_global_event_detail_filtered(bot, chat_id, pool, event_id, page, filters).await?;
        }
        GlobalHistoryAction::Filters { filters } => {
            show_history_filter_menu(bot, chat_id, pool, filters).await?;
        }
        GlobalHistoryAction::Picker {
            kind,
            page,
            filters,
        } => {
            show_history_filter_picker(bot, chat_id, pool, kind, page, filters).await?;
        }
        GlobalHistoryAction::Select {
            kind,
            value,
            filters,
        } => {
            if let Some(next) = filters.with_selection(kind, &value) {
                show_history_filter_menu(bot, chat_id, pool, next).await?;
            } else {
                bot.send_message(chat_id, "Filtro non valido o non più disponibile.")
                    .reply_markup(filter_back_keyboard(filters))
                    .await?;
            }
        }
    }
    Ok(())
}

async fn load_filtered_global_history_page(
    pool: &SqlitePool,
    page: i64,
    filters: HistoryFilters,
) -> Result<(Vec<HistoryListRow>, i64), sqlx::Error> {
    let mut count = QueryBuilder::<Sqlite>::new(
        "SELECT COUNT(*) FROM storico_eventi e \
         JOIN storico_entita se ON se.id = e.entita_storico_id \
         WHERE ",
    );
    push_visible_space_filter(&mut count, "e");
    count.push(" AND se.spazio_id = e.spazio_id");
    push_history_resource_visibility(&mut count, "se");
    push_global_history_filters(&mut count, filters);
    let total: i64 = count.build_query_scalar().fetch_one(pool).await?;

    let mut list = QueryBuilder::<Sqlite>::new(
        "SELECT e.id, se.tipo_entita, \
                strftime('%d/%m/%Y %H:%M', e.avvenuto_il, 'localtime') AS when_local, \
                e.operazione, e.componente, e.nome_entita_snapshot, \
                e.abitazione_nome_snapshot, e.stanza_nome_snapshot, \
                e.contenitore_percorso_snapshot, e.spazio_nome_snapshot, \
                e.luogo_spazio_nome_snapshot, e.attore_nome_snapshot, \
                e.origine_azione, e.automatico \
         FROM storico_eventi e \
         JOIN storico_entita se ON se.id = e.entita_storico_id \
         WHERE ",
    );
    push_visible_space_filter(&mut list, "e");
    list.push(" AND se.spazio_id = e.spazio_id");
    push_history_resource_visibility(&mut list, "se");
    push_global_history_filters(&mut list, filters);
    list.push(" ORDER BY e.avvenuto_il DESC, e.id DESC LIMIT ");
    list.push_bind(HISTORY_PAGE_SIZE);
    list.push(" OFFSET ");
    list.push_bind(page.max(0) * HISTORY_PAGE_SIZE);

    let events = list
        .build_query_as::<HistoryListRow>()
        .fetch_all(pool)
        .await?;
    Ok((events, total))
}

fn push_visible_space_filter(query: &mut QueryBuilder<'_, Sqlite>, alias: &str) {
    let actor = crate::identity::current_actor();
    if actor.view_all {
        if let Some(user_id) = actor.utente_id {
            query.push(alias);
            query.push(".spazio_id IN (SELECT spazio_id FROM membri_spazio WHERE utente_id = ");
            query.push_bind(user_id);
            query.push(")");
            return;
        }
    }

    query.push(alias);
    query.push(".spazio_id = ");
    query.push_bind(actor.spazio_id);
}

fn push_history_resource_visibility(query: &mut QueryBuilder<'_, Sqlite>, entity_alias: &str) {
    let actor = crate::identity::current_actor();
    query.push(" AND (");
    query.push(entity_alias);
    query.push(
        ".tipo_entita <> 'prodotto_alimentare' OR EXISTS (\
        SELECT 1 FROM prodotti_alimentari p \
        JOIN alimenti a ON a.id = p.alimento_id \
        WHERE p.id = ",
    );
    query.push(entity_alias);
    query.push(".id_origine AND (");

    match actor.utente_id {
        Some(user_id) if actor.view_all => {
            query.push("a.catalogo_globale = 1 OR a.proprietario_utente_id = ");
            query.push_bind(user_id);
            query.push(
                " OR EXISTS (\
                SELECT 1 FROM alimento_spazi asp \
                JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
                WHERE asp.alimento_id = a.id AND ms.utente_id = ",
            );
            query.push_bind(user_id);
            query.push(")");
        }
        Some(user_id) => {
            query.push("a.catalogo_globale = 1 OR a.proprietario_utente_id = ");
            query.push_bind(user_id);
            query.push(
                " OR EXISTS (\
                SELECT 1 FROM alimento_spazi asp \
                WHERE asp.alimento_id = a.id AND asp.spazio_id = ",
            );
            query.push_bind(actor.spazio_id);
            query.push(")");
        }
        None => {
            query.push("a.catalogo_globale = 1");
        }
    }

    query.push(")))");
}

fn push_global_history_filters(query: &mut QueryBuilder<'_, Sqlite>, filters: HistoryFilters) {
    match filters.period {
        HistoryPeriodFilter::All => {}
        HistoryPeriodFilter::Today => {
            query.push(" AND date(e.avvenuto_il, 'localtime') = date('now', 'localtime')");
        }
        HistoryPeriodFilter::Last7Days => {
            query.push(" AND datetime(e.avvenuto_il) >= datetime('now', '-7 days')");
        }
        HistoryPeriodFilter::Last30Days => {
            query.push(" AND datetime(e.avvenuto_il) >= datetime('now', '-30 days')");
        }
    }

    if let Some(module) = filters.module.db_value() {
        query.push(" AND e.modulo = ");
        query.push_bind(module);
    }

    if let Some(operation) = filters.operation.db_value() {
        query.push(" AND e.operazione = ");
        query.push_bind(operation);
    }

    if let Some(home_id) = filters.home_id {
        query.push(" AND (e.abitazione_storico_id = ");
        query.push_bind(home_id);
        query.push(" OR (se.tipo_entita = 'abitazione' AND e.entita_storico_id = ");
        query.push_bind(home_id);
        query.push(") OR EXISTS (SELECT 1 FROM storico_cambi_luogo scl WHERE scl.evento_id = e.id AND (scl.abitazione_prima_id = ");
        query.push_bind(home_id);
        query.push(" OR scl.abitazione_dopo_id = ");
        query.push_bind(home_id);
        query.push(")))");
    }

    if let Some(room_id) = filters.room_id {
        query.push(" AND (e.stanza_storico_id = ");
        query.push_bind(room_id);
        query.push(" OR (se.tipo_entita = 'stanza' AND e.entita_storico_id = ");
        query.push_bind(room_id);
        query.push(") OR EXISTS (SELECT 1 FROM storico_cambi_luogo scl WHERE scl.evento_id = e.id AND (scl.stanza_prima_id = ");
        query.push_bind(room_id);
        query.push(" OR scl.stanza_dopo_id = ");
        query.push_bind(room_id);
        query.push(")))");
    }

    if let Some(entity_id) = filters.entity_id {
        query.push(" AND e.entita_storico_id = ");
        query.push_bind(entity_id);
    }
}

async fn load_history_picker_options(
    pool: &SqlitePool,
    kind: HistoryFilterKind,
    page: i64,
) -> Result<(Vec<HistoryPickerOption>, i64), sqlx::Error> {
    let page = page.max(0);
    let offset = page * HISTORY_FILTER_PICKER_PAGE_SIZE;
    let visible = crate::identity::visible_space_sql("se");
    let bind_id = crate::identity::visible_space_bind_id();

    match kind {
        HistoryFilterKind::Home => {
            let total_sql = format!(
                "SELECT COUNT(*) FROM storico_entita se WHERE se.tipo_entita = 'abitazione' AND {visible}"
            );
            let total: i64 = sqlx::query_scalar(&total_sql)
                .bind(bind_id)
                .fetch_one(pool)
                .await?;
            let list_sql = format!(
                "SELECT se.id, se.nome_ultimo AS label, sp.nome AS subtitle, \
                        CASE WHEN se.eliminato_il IS NULL THEN 0 ELSE 1 END AS deleted \
                 FROM storico_entita se JOIN spazi sp ON sp.id = se.spazio_id \
                 WHERE se.tipo_entita = 'abitazione' AND {visible} \
                 ORDER BY deleted ASC, sp.nome COLLATE NOCASE, se.nome_ultimo COLLATE NOCASE, se.id \
                 LIMIT ? OFFSET ?"
            );
            let options = sqlx::query_as::<_, HistoryPickerOption>(&list_sql)
                .bind(bind_id)
                .bind(HISTORY_FILTER_PICKER_PAGE_SIZE)
                .bind(offset)
                .fetch_all(pool)
                .await?;
            Ok((options, total))
        }
        HistoryFilterKind::Room => {
            let total_sql = format!(
                "SELECT COUNT(*) FROM storico_entita se WHERE se.tipo_entita = 'stanza' AND {visible}"
            );
            let total: i64 = sqlx::query_scalar(&total_sql)
                .bind(bind_id)
                .fetch_one(pool)
                .await?;
            let list_sql = format!(
                "SELECT se.id, se.nome_ultimo AS label, \
                        (sp.nome || ' · ' || COALESCE((SELECT e.abitazione_nome_snapshot FROM storico_eventi e \
                         WHERE e.entita_storico_id = se.id AND e.spazio_id = se.spazio_id \
                           AND e.abitazione_nome_snapshot IS NOT NULL \
                         ORDER BY e.avvenuto_il DESC, e.id DESC LIMIT 1), '')) AS subtitle, \
                        CASE WHEN se.eliminato_il IS NULL THEN 0 ELSE 1 END AS deleted \
                 FROM storico_entita se JOIN spazi sp ON sp.id = se.spazio_id \
                 WHERE se.tipo_entita = 'stanza' AND {visible} \
                 ORDER BY deleted ASC, sp.nome COLLATE NOCASE, se.nome_ultimo COLLATE NOCASE, se.id \
                 LIMIT ? OFFSET ?"
            );
            let options = sqlx::query_as::<_, HistoryPickerOption>(&list_sql)
                .bind(bind_id)
                .bind(HISTORY_FILTER_PICKER_PAGE_SIZE)
                .bind(offset)
                .fetch_all(pool)
                .await?;
            Ok((options, total))
        }
        HistoryFilterKind::Entity => {
            let mut count =
                QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM storico_entita se WHERE ");
            push_visible_space_filter(&mut count, "se");
            push_history_resource_visibility(&mut count, "se");
            count.push(
                " AND EXISTS (\
                    SELECT 1 FROM storico_eventi e \
                    WHERE e.entita_storico_id = se.id AND e.spazio_id = se.spazio_id)",
            );
            let total: i64 = count.build_query_scalar().fetch_one(pool).await?;

            let mut list = QueryBuilder::<Sqlite>::new(
                "SELECT se.id, se.nome_ultimo AS label, \
                        ((CASE se.tipo_entita \
                            WHEN 'prodotto_alimentare' THEN 'Prodotto alimentare' \
                            WHEN 'oggetto' THEN 'Oggetto' \
                            WHEN 'abitazione' THEN 'Casa' \
                            WHEN 'stanza' THEN 'Stanza' \
                            WHEN 'contenitore' THEN 'Contenitore' \
                            ELSE se.tipo_entita END) || ' · ' || sp.nome) AS subtitle, \
                        CASE WHEN se.eliminato_il IS NULL THEN 0 ELSE 1 END AS deleted \
                 FROM storico_entita se JOIN spazi sp ON sp.id = se.spazio_id \
                 WHERE ",
            );
            push_visible_space_filter(&mut list, "se");
            push_history_resource_visibility(&mut list, "se");
            list.push(
                " AND EXISTS (\
                       SELECT 1 FROM storico_eventi e \
                       WHERE e.entita_storico_id = se.id AND e.spazio_id = se.spazio_id) \
                 ORDER BY (SELECT MAX(e2.id) FROM storico_eventi e2 \
                           WHERE e2.entita_storico_id = se.id AND e2.spazio_id = se.spazio_id) DESC, se.id DESC \
                 LIMIT ",
            );
            list.push_bind(HISTORY_FILTER_PICKER_PAGE_SIZE);
            list.push(" OFFSET ");
            list.push_bind(offset);
            let options = list
                .build_query_as::<HistoryPickerOption>()
                .fetch_all(pool)
                .await?;
            Ok((options, total))
        }
        HistoryFilterKind::Period | HistoryFilterKind::Module | HistoryFilterKind::Operation => {
            Ok((Vec::new(), 0))
        }
    }
}

async fn history_filters_summary(pool: &SqlitePool, filters: HistoryFilters) -> String {
    if filters.is_default() {
        return "Nessun filtro".to_string();
    }

    let mut parts = Vec::new();
    if filters.period != HistoryPeriodFilter::All {
        parts.push(format!("🗓 {}", filters.period.label()));
    }
    if filters.module != HistoryModuleFilter::All {
        parts.push(format!("🧩 {}", filters.module.label()));
    }
    if filters.operation != HistoryOperationFilter::All {
        parts.push(format!("⚙️ {}", filters.operation.label()));
    }
    if let Some(name) = history_filter_entity_name(pool, filters.home_id).await {
        parts.push(format!("🏠 {name}"));
    }
    if let Some(name) = history_filter_entity_name(pool, filters.room_id).await {
        parts.push(format!("🚪 {name}"));
    }
    if let Some(name) = history_filter_entity_name(pool, filters.entity_id).await {
        parts.push(format!("🎯 {name}"));
    }
    parts.join("\n")
}

async fn history_filter_entity_name(pool: &SqlitePool, id: Option<i64>) -> Option<String> {
    let id = id?;
    let mut query =
        QueryBuilder::<Sqlite>::new("SELECT se.nome_ultimo FROM storico_entita se WHERE se.id = ");
    query.push_bind(id);
    query.push(" AND ");
    push_visible_space_filter(&mut query, "se");
    push_history_resource_visibility(&mut query, "se");
    query
        .build_query_scalar::<String>()
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

fn global_history_keyboard(
    events: &[HistoryListRow],
    page: i64,
    total: i64,
    filters: HistoryFilters,
) -> InlineKeyboardMarkup {
    let token = filters.to_token();
    let mut rows = Vec::new();

    for event in events {
        rows.push(vec![button(
            &event_button_label(event),
            &format!(
                "h:e:{}:{}:{token}",
                base62_encode(event.id),
                base62_encode(page)
            ),
        )]);
    }

    let pages = total_pages(total);
    if pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(button(
                "⬅️",
                &format!("h:g:{}:{token}", base62_encode(page - 1)),
            ));
        }
        nav.push(button(&format!("{} / {}", page + 1, pages), "history:noop"));
        if page + 1 < pages {
            nav.push(button(
                "➡️",
                &format!("h:g:{}:{token}", base62_encode(page + 1)),
            ));
        }
        rows.push(nav);
    }

    let mut filter_row = vec![button("🔎 Filtri", &format!("h:f:{token}"))];
    if !filters.is_default() {
        filter_row.push(button(
            "🧹 Azzera filtri",
            &format!("h:g:0:{}", HistoryFilters::default().to_token()),
        ));
    }
    rows.push(filter_row);
    rows.push(vec![
        button("⬅️ Indietro", "menu:main"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn global_event_detail_keyboard(page: i64, filters: HistoryFilters) -> InlineKeyboardMarkup {
    let token = filters.to_token();
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "⬅️ Torna allo storico",
            &format!("h:g:{}:{token}", base62_encode(page)),
        )],
        vec![button("🔎 Filtri", &format!("h:f:{token}"))],
        vec![button("🏠 Menù principale", "menu:main")],
    ])
}

fn static_filter_keyboard(
    kind: HistoryFilterKind,
    filters: HistoryFilters,
    options: &[(&str, &str)],
) -> InlineKeyboardMarkup {
    let token = filters.to_token();
    let kind_code = kind.code();
    let current = filters.selection_code(kind);
    let mut rows = Vec::new();

    for (value, label) in options {
        let selected = current.as_deref() == Some(*value);
        rows.push(vec![button(
            &format!("{}{}", if selected { "✅ " } else { "" }, label),
            &format!("h:s:{kind_code}:{value}:{token}"),
        )]);
    }
    rows.push(vec![button("⬅️ Torna ai filtri", &format!("h:f:{token}"))]);
    InlineKeyboardMarkup::new(rows)
}

fn dynamic_filter_keyboard(
    kind: HistoryFilterKind,
    filters: HistoryFilters,
    options: &[HistoryPickerOption],
    page: i64,
    total: i64,
) -> InlineKeyboardMarkup {
    let token = filters.to_token();
    let kind_code = kind.code();
    let current = filters.selection_id(kind);
    let mut rows = vec![vec![button(
        if current.is_none() {
            "✅ Tutti"
        } else {
            "Tutti"
        },
        &format!("h:s:{kind_code}:0:{token}"),
    )]];

    for option in options {
        let mut label = String::new();
        if current == Some(option.id) {
            label.push_str("✅ ");
        }
        if option.deleted != 0 {
            label.push_str("🗑 ");
        }
        label.push_str(&truncate_chars(&option.label, 24));
        if let Some(subtitle) = option.subtitle.as_deref() {
            label.push_str(" · ");
            label.push_str(&truncate_chars(&filter_subtitle_label(kind, subtitle), 16));
        }
        rows.push(vec![button(
            &label,
            &format!("h:s:{kind_code}:{}:{token}", base62_encode(option.id)),
        )]);
    }

    let pages =
        ((total + HISTORY_FILTER_PICKER_PAGE_SIZE - 1) / HISTORY_FILTER_PICKER_PAGE_SIZE).max(1);
    if pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(button(
                "⬅️",
                &format!("h:p:{kind_code}:{}:{token}", base62_encode(page - 1)),
            ));
        }
        nav.push(button(&format!("{} / {}", page + 1, pages), "history:noop"));
        if page + 1 < pages {
            nav.push(button(
                "➡️",
                &format!("h:p:{kind_code}:{}:{token}", base62_encode(page + 1)),
            ));
        }
        rows.push(nav);
    }

    rows.push(vec![button("⬅️ Torna ai filtri", &format!("h:f:{token}"))]);
    InlineKeyboardMarkup::new(rows)
}

fn filter_back_keyboard(filters: HistoryFilters) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button(
        "⬅️ Torna ai filtri",
        &format!("h:f:{}", filters.to_token()),
    )]])
}

fn filter_subtitle_label(kind: HistoryFilterKind, value: &str) -> String {
    if kind == HistoryFilterKind::Entity {
        match value {
            "oggetto" => "Oggetto".to_string(),
            "abitazione" => "Casa".to_string(),
            "stanza" => "Stanza".to_string(),
            other => other.to_string(),
        }
    } else {
        value.to_string()
    }
}

fn parse_global_history_action(data: &str) -> Option<GlobalHistoryAction> {
    if !data.starts_with("h:") {
        return None;
    }
    let parts: Vec<&str> = data.split(':').collect();
    match parts.as_slice() {
        ["h", "g", page, token] => Some(GlobalHistoryAction::Page {
            page: parse_base62_nonnegative(page)?,
            filters: HistoryFilters::from_token(token)?,
        }),
        ["h", "e", event_id, page, token] => Some(GlobalHistoryAction::Event {
            event_id: parse_base62_positive(event_id)?,
            page: parse_base62_nonnegative(page)?,
            filters: HistoryFilters::from_token(token)?,
        }),
        ["h", "f", token] => Some(GlobalHistoryAction::Filters {
            filters: HistoryFilters::from_token(token)?,
        }),
        ["h", "p", kind, page, token] => Some(GlobalHistoryAction::Picker {
            kind: HistoryFilterKind::from_code(kind)?,
            page: parse_base62_nonnegative(page)?,
            filters: HistoryFilters::from_token(token)?,
        }),
        ["h", "s", kind, value, token] => Some(GlobalHistoryAction::Select {
            kind: HistoryFilterKind::from_code(kind)?,
            value: (*value).to_string(),
            filters: HistoryFilters::from_token(token)?,
        }),
        _ => None,
    }
}

impl HistoryFilters {
    fn is_default(self) -> bool {
        self == Self::default()
    }

    fn to_token(self) -> String {
        format!(
            "{}{}{}.{}.{}.{}",
            self.period.code(),
            self.module.code(),
            self.operation.code(),
            base62_encode(self.home_id.unwrap_or(0)),
            base62_encode(self.room_id.unwrap_or(0)),
            base62_encode(self.entity_id.unwrap_or(0)),
        )
    }

    fn from_token(token: &str) -> Option<Self> {
        let mut parts = token.split('.');
        let compact = parts.next()?;
        let home = parts.next()?;
        let room = parts.next()?;
        let entity = parts.next()?;
        if parts.next().is_some() || compact.chars().count() != 3 {
            return None;
        }
        let mut codes = compact.chars();
        Some(Self {
            period: HistoryPeriodFilter::from_code(codes.next()?)?,
            module: HistoryModuleFilter::from_code(codes.next()?)?,
            operation: HistoryOperationFilter::from_code(codes.next()?)?,
            home_id: zero_to_none(base62_decode(home)?),
            room_id: zero_to_none(base62_decode(room)?),
            entity_id: zero_to_none(base62_decode(entity)?),
        })
    }

    fn selection_code(self, kind: HistoryFilterKind) -> Option<String> {
        match kind {
            HistoryFilterKind::Period => Some(self.period.code().to_string()),
            HistoryFilterKind::Module => Some(self.module.code().to_string()),
            HistoryFilterKind::Operation => Some(self.operation.code().to_string()),
            _ => None,
        }
    }

    fn selection_id(self, kind: HistoryFilterKind) -> Option<i64> {
        match kind {
            HistoryFilterKind::Home => self.home_id,
            HistoryFilterKind::Room => self.room_id,
            HistoryFilterKind::Entity => self.entity_id,
            _ => None,
        }
    }

    fn with_selection(self, kind: HistoryFilterKind, value: &str) -> Option<Self> {
        let mut next = self;
        match kind {
            HistoryFilterKind::Period => next.period = HistoryPeriodFilter::from_code_str(value)?,
            HistoryFilterKind::Module => next.module = HistoryModuleFilter::from_code_str(value)?,
            HistoryFilterKind::Operation => {
                next.operation = HistoryOperationFilter::from_code_str(value)?
            }
            HistoryFilterKind::Home => next.home_id = zero_to_none(base62_decode(value)?),
            HistoryFilterKind::Room => next.room_id = zero_to_none(base62_decode(value)?),
            HistoryFilterKind::Entity => next.entity_id = zero_to_none(base62_decode(value)?),
        }
        Some(next)
    }
}

impl HistoryPeriodFilter {
    fn code(self) -> char {
        match self {
            Self::All => '0',
            Self::Today => 't',
            Self::Last7Days => '7',
            Self::Last30Days => '3',
        }
    }
    fn from_code(code: char) -> Option<Self> {
        match code {
            '0' => Some(Self::All),
            't' => Some(Self::Today),
            '7' => Some(Self::Last7Days),
            '3' => Some(Self::Last30Days),
            _ => None,
        }
    }
    fn from_code_str(code: &str) -> Option<Self> {
        let mut chars = code.chars();
        let value = Self::from_code(chars.next()?)?;
        chars.next().is_none().then_some(value)
    }
    fn label(self) -> &'static str {
        match self {
            Self::All => "Tutto",
            Self::Today => "Oggi",
            Self::Last7Days => "7 giorni",
            Self::Last30Days => "30 giorni",
        }
    }
}

impl HistoryModuleFilter {
    fn code(self) -> char {
        match self {
            Self::All => '0',
            Self::Oggetti => 'o',
            Self::Luoghi => 'l',
            Self::Alimentazione => 'a',
        }
    }
    fn from_code(code: char) -> Option<Self> {
        match code {
            '0' => Some(Self::All),
            'o' => Some(Self::Oggetti),
            'l' => Some(Self::Luoghi),
            'a' => Some(Self::Alimentazione),
            _ => None,
        }
    }
    fn from_code_str(code: &str) -> Option<Self> {
        let mut chars = code.chars();
        let value = Self::from_code(chars.next()?)?;
        chars.next().is_none().then_some(value)
    }
    fn db_value(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Oggetti => Some("oggetti"),
            Self::Luoghi => Some("luoghi"),
            Self::Alimentazione => Some("alimentazione"),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::All => "Tutti",
            Self::Oggetti => "Oggetti",
            Self::Luoghi => "Luoghi",
            Self::Alimentazione => "Alimentazione",
        }
    }
}

impl HistoryOperationFilter {
    fn code(self) -> char {
        match self {
            Self::All => '0',
            Self::Creazione => 'c',
            Self::Modifica => 'm',
            Self::Eliminazione => 'd',
            Self::Assegnazione => 'a',
            Self::Spostamento => 's',
            Self::Rimozione => 'r',
            Self::FotoAggiunta => 'f',
            Self::Rinomina => 'n',
        }
    }
    fn from_code(code: char) -> Option<Self> {
        match code {
            '0' => Some(Self::All),
            'c' => Some(Self::Creazione),
            'm' => Some(Self::Modifica),
            'd' => Some(Self::Eliminazione),
            'a' => Some(Self::Assegnazione),
            's' => Some(Self::Spostamento),
            'r' => Some(Self::Rimozione),
            'f' => Some(Self::FotoAggiunta),
            'n' => Some(Self::Rinomina),
            _ => None,
        }
    }
    fn from_code_str(code: &str) -> Option<Self> {
        let mut chars = code.chars();
        let value = Self::from_code(chars.next()?)?;
        chars.next().is_none().then_some(value)
    }
    fn db_value(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Creazione => Some("creazione"),
            Self::Modifica => Some("modifica"),
            Self::Eliminazione => Some("eliminazione"),
            Self::Assegnazione => Some("assegnazione"),
            Self::Spostamento => Some("spostamento"),
            Self::Rimozione => Some("rimozione"),
            Self::FotoAggiunta => Some("foto_aggiunta"),
            Self::Rinomina => Some("rinomina"),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::All => "Tutte",
            Self::Creazione => "Creazione",
            Self::Modifica => "Modifica",
            Self::Eliminazione => "Eliminazione",
            Self::Assegnazione => "Assegnazione",
            Self::Spostamento => "Spostamento",
            Self::Rimozione => "Rimozione luogo",
            Self::FotoAggiunta => "Foto aggiunta",
            Self::Rinomina => "Rinomina",
        }
    }
}

impl HistoryFilterKind {
    fn code(self) -> char {
        match self {
            Self::Period => 'p',
            Self::Module => 'm',
            Self::Operation => 'o',
            Self::Home => 'h',
            Self::Room => 'r',
            Self::Entity => 'e',
        }
    }
    fn from_code(value: &str) -> Option<Self> {
        match value {
            "p" => Some(Self::Period),
            "m" => Some(Self::Module),
            "o" => Some(Self::Operation),
            "h" => Some(Self::Home),
            "r" => Some(Self::Room),
            "e" => Some(Self::Entity),
            _ => None,
        }
    }
}

fn base62_encode(mut value: i64) -> String {
    if value <= 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while value > 0 {
        out.push(BASE62[(value % 62) as usize] as char);
        value /= 62;
    }
    out.iter().rev().collect()
}

fn base62_decode(value: &str) -> Option<i64> {
    if value.is_empty() {
        return None;
    }
    let mut result = 0_i64;
    for byte in value.bytes() {
        let digit = match byte {
            b'0'..=b'9' => i64::from(byte - b'0'),
            b'a'..=b'z' => i64::from(byte - b'a') + 10,
            b'A'..=b'Z' => i64::from(byte - b'A') + 36,
            _ => return None,
        };
        result = result.checked_mul(62)?.checked_add(digit)?;
    }
    Some(result)
}

fn parse_base62_positive(value: &str) -> Option<i64> {
    let value = base62_decode(value)?;
    (value > 0).then_some(value)
}

fn parse_base62_nonnegative(value: &str) -> Option<i64> {
    base62_decode(value)
}

fn zero_to_none(value: i64) -> Option<i64> {
    (value > 0).then_some(value)
}

pub async fn show_global_history(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
) -> ResponseResult<()> {
    show_global_history_filtered(bot, chat_id, pool, page, HistoryFilters::default()).await
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
            bot.send_message(chat_id, "Storico non disponibile: oggetto non trovato.")
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
                    &format!("📜 Storico\n🏷️ {}", entity.1),
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

    if let Some(action) = parse_global_history_action(data) {
        handle_global_history_action(bot, chat_id, pool, action).await?;
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
            bot.send_message(chat_id, "Evento storico non trovato.")
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
    let sql = format!(
        "SELECT se.id, i.nome FROM storico_entita se \
         JOIN items i ON i.id = se.id_origine AND i.tipo = 'oggetto' \
         WHERE se.tipo_entita = 'oggetto' AND se.id_origine = ? \
           AND se.spazio_id = i.spazio_id AND {} \
           AND se.eliminato_il IS NULL ORDER BY se.id DESC LIMIT 1",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_as::<_, (i64, String)>(&sql)
        .bind(item_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await
}

async fn load_entity_history_page(
    pool: &SqlitePool,
    entity_id: i64,
    page: i64,
) -> Result<(Vec<HistoryListRow>, i64), sqlx::Error> {
    let visible = crate::identity::visible_space_sql("se");
    let check_sql =
        format!("SELECT se.spazio_id FROM storico_entita se WHERE se.id = ? AND {visible}");
    let space_id: i64 = sqlx::query_scalar(&check_sql)
        .bind(entity_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM storico_eventi WHERE entita_storico_id = ? AND spazio_id = ?",
    )
    .bind(entity_id)
    .bind(space_id)
    .fetch_one(pool)
    .await?;

    let events = sqlx::query_as::<_, HistoryListRow>(
        "SELECT e.id, se.tipo_entita, \
                strftime('%d/%m/%Y %H:%M', e.avvenuto_il, 'localtime') AS when_local, \
                e.operazione, e.componente, e.nome_entita_snapshot, \
                e.abitazione_nome_snapshot, e.stanza_nome_snapshot, \
                e.contenitore_percorso_snapshot, e.spazio_nome_snapshot, \
                e.luogo_spazio_nome_snapshot, e.attore_nome_snapshot, \
                e.origine_azione, e.automatico \
         FROM storico_eventi e JOIN storico_entita se ON se.id = e.entita_storico_id \
         WHERE e.entita_storico_id = ? AND e.spazio_id = ? AND se.spazio_id = e.spazio_id \
         ORDER BY e.avvenuto_il DESC, e.id DESC LIMIT ? OFFSET ?",
    )
    .bind(entity_id)
    .bind(space_id)
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
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT se.tipo_entita, \
                strftime('%d/%m/%Y %H:%M', e.avvenuto_il, 'localtime') AS when_local, \
                e.modulo, e.componente, e.operazione, e.nome_entita_snapshot, \
                e.abitazione_nome_snapshot, e.stanza_nome_snapshot, \
                e.contenitore_percorso_snapshot, e.luogo_spazio_nome_snapshot, \
                e.evento_padre_id, e.attore_nome_snapshot, e.spazio_nome_snapshot, \
                e.origine_azione, e.automatico \
         FROM storico_eventi e JOIN storico_entita se ON se.id = e.entita_storico_id \
         WHERE e.id = ",
    );
    query.push_bind(event_id);
    query.push(" AND se.spazio_id = e.spazio_id AND ");
    push_visible_space_filter(&mut query, "e");
    push_history_resource_visibility(&mut query, "se");
    let event = query
        .build_query_as::<HistoryEventDetail>()
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
        "SELECT spazio_prima_nome, abitazione_prima_nome, stanza_prima_nome, contenitore_prima_percorso, \
                spazio_dopo_nome, abitazione_dopo_nome, stanza_dopo_nome, contenitore_dopo_percorso \
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
            event_action_icon(&event.componente, &event.operazione),
            event_action_label(&event.componente, &event.operazione),
            entity_icon(&event.tipo_entita),
            event.nome_entita_snapshot,
        ));
        message.push('\n');
        message.push_str(&format_history_actor_line(
            event.attore_nome_snapshot.as_deref(),
            &event.origine_azione,
            event.automatico != 0,
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
        "📜 Dettaglio storico\n\n{}\n{} {}\n{} {}\nModulo: {} · {}",
        event.when_local,
        event_action_icon(&event.componente, &event.operazione),
        event_action_label(&event.componente, &event.operazione),
        entity_icon(&event.tipo_entita),
        event.nome_entita_snapshot,
        module_label(&event.modulo),
        component_label(&event.componente),
    );

    if event.automatico != 0 {
        message.push_str("\n⚙️ Effetto automatico");
    }
    message.push('\n');
    message.push_str(&format_history_actor_line(
        event.attore_nome_snapshot.as_deref(),
        &event.origine_azione,
        event.automatico != 0,
    ));
    if should_show_origin(&event.origine_azione) {
        message.push_str(&format!(
            "\nOrigine: {}",
            origin_label(&event.origine_azione)
        ));
    }
    if event.tipo_entita != "profilo_alimentare" {
        if let Some(space) = event.spazio_nome_snapshot.as_deref() {
            message.push_str(&format!("\n👥 Spazio dell'entità: {space}"));
        }
    }

    if event.evento_padre_id.is_some() {
        message.push_str("\n🔗 Collegato a un evento precedente");
    }

    if let Some(context) = detail_context(event) {
        message.push_str("\n📍 Posizione: ");
        message.push_str(&context);
    }

    let visible_changes = changes
        .iter()
        .filter(|change| !matches!(change.campo.as_str(), "foto_id" | "percorso_file"))
        .collect::<Vec<_>>();
    if !visible_changes.is_empty() {
        message.push_str("\n\nCambiamenti:");
        for change in visible_changes {
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
        let before = format_location_with_space(
            location.abitazione_prima_nome.as_deref(),
            location.stanza_prima_nome.as_deref(),
            location.contenitore_prima_percorso.as_deref(),
            location.spazio_prima_nome.as_deref(),
            true,
        );
        let after = format_location_with_space(
            location.abitazione_dopo_nome.as_deref(),
            location.stanza_dopo_nome.as_deref(),
            location.contenitore_dopo_percorso.as_deref(),
            location.spazio_dopo_nome.as_deref(),
            true,
        );
        let before = location_with_home_icon(&before);
        let after = location_with_home_icon(&after);
        message.push_str("\n\n🚚 Luogo:");
        message.push_str(&format!("\nDa: {before}\nA: {after}"));
    }

    message
}

fn format_history_actor_line(actor: Option<&str>, origin: &str, automatic: bool) -> String {
    match (actor, origin, automatic) {
        (Some(name), _, true) => format!("👤 Originato da: {name}"),
        (Some(name), _, false) => format!("👤 Autore: {name}"),
        (None, "legacy", _) => "👤 Autore: non disponibile (evento pre-Step 7)".to_string(),
        (None, _, true) => "🤖 Effetto automatico di sistema".to_string(),
        (None, _, false) => "🤖 Sistema".to_string(),
    }
}

fn should_show_origin(origin: &str) -> bool {
    origin != "telegram"
}

fn origin_label(origin: &str) -> &str {
    match origin {
        "telegram" => "Telegram",
        "sistema" => "Sistema",
        "google" => "Google",
        "automazione" => "Automazione",
        "legacy" => "Pre-Step 7 / non disponibile",
        _ => origin,
    }
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
        HistoryScope::Global => rows.push(vec![button("🏠 Menù principale", "menu:main")]),
        HistoryScope::Item(item_id) => {
            rows.push(vec![button(
                "⬅️ Torna all'oggetto",
                &format!("oggetti:view:{item_id}"),
            )]);
            rows.push(vec![button("🏠 Menù principale", "menu:main")]);
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
        vec![button("🏠 Menù principale", "menu:main")],
    ])
}

fn history_home_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button("🏠 Menù principale", "menu:main")]])
}

fn item_return_keyboard(item_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "⬅️ Torna all'oggetto",
            &format!("oggetti:view:{item_id}"),
        )],
        vec![button("🏠 Menù principale", "menu:main")],
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
        event_action_icon(&event.componente, &event.operazione),
        event_action_label(&event.componente, &event.operazione),
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
    event.abitazione_nome_snapshot.as_deref().map(|home| {
        let show_space = crate::identity::current_view_all()
            || event
                .luogo_spazio_nome_snapshot
                .as_deref()
                .zip(event.spazio_nome_snapshot.as_deref())
                .is_some_and(|(location_space, owner_space)| location_space != owner_space);
        format_location_with_space(
            Some(home),
            event.stanza_nome_snapshot.as_deref(),
            event.contenitore_percorso_snapshot.as_deref(),
            event.luogo_spazio_nome_snapshot.as_deref(),
            show_space,
        )
    })
}

fn detail_context(event: &HistoryEventDetail) -> Option<String> {
    event.abitazione_nome_snapshot.as_deref().map(|home| {
        format_location_with_space(
            Some(home),
            event.stanza_nome_snapshot.as_deref(),
            event.contenitore_percorso_snapshot.as_deref(),
            event.luogo_spazio_nome_snapshot.as_deref(),
            event.luogo_spazio_nome_snapshot.is_some(),
        )
    })
}

#[cfg(test)]
fn format_location(home: Option<&str>, room: Option<&str>, container_path: Option<&str>) -> String {
    format_location_with_space(home, room, container_path, None, false)
}

fn format_location_with_space(
    home: Option<&str>,
    room: Option<&str>,
    container_path: Option<&str>,
    space: Option<&str>,
    show_space: bool,
) -> String {
    let Some(home) = home else {
        return "Nessun luogo".to_string();
    };

    let mut parts = Vec::new();
    if show_space {
        if let Some(space) = space {
            parts.push(format!("{home} · {space}"));
        } else {
            parts.push(home.to_string());
        }
    } else {
        parts.push(home.to_string());
    }

    if let Some(room) = room {
        parts.push(room.to_string());
    }
    if let Some(path) = container_path {
        parts.extend(
            path.split(" / ")
                .filter(|part| !part.is_empty())
                .map(str::to_string),
        );
    }
    parts.join(" / ")
}

fn location_with_home_icon(location: &str) -> String {
    if location == "Nessun luogo" {
        location.to_string()
    } else {
        format!("🏠 {location}")
    }
}

fn event_action_icon(component: &str, operation: &str) -> &'static str {
    match component {
        "condivisione_profilo" => "👥",
        "privatizzazione_profilo" => "🔒",
        "archiviazione_profilo" => "📦",
        "visibilita_profili" => "👁️",
        _ => operation_icon(operation),
    }
}

fn event_action_label(component: &str, operation: &str) -> &'static str {
    match component {
        "condivisione_profilo" => "Condiviso",
        "privatizzazione_profilo" => "Reso privato",
        "archiviazione_profilo" => "Archiviato",
        "visibilita_profili" => "Visibilità modificata",
        _ => operation_label(operation),
    }
}

fn module_label(module: &str) -> &str {
    match module {
        "alimentazione" => "Alimentazione",
        _ => module,
    }
}

fn component_label(component: &str) -> &str {
    match component {
        "profili_alimentari"
        | "visibilita_profili"
        | "condivisione_profilo"
        | "privatizzazione_profilo"
        | "archiviazione_profilo" => "Profili alimentari",
        _ => component,
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
        "oggetto" => "🏷️",
        "abitazione" => "🏠",
        "stanza" => "🚪",
        "contenitore" => "📦",
        "veicolo" => "🚗",
        "vestito" => "👕",
        "alimento" => "🥕",
        "prodotto_alimentare" => "🛒",
        "profilo_alimentare" => "👤",
        _ => "🔹",
    }
}

fn field_label(field: &str) -> &str {
    match field {
        "nome" => "Nome",
        "descrizione" => "Descrizione",
        "marca" => "Marca",
        "nome_commerciale" => "Nome commerciale",
        "alimento_associato" => "Alimento associato",
        "confezione" => "Confezione",
        "barcode_ean" => "Barcode / EAN",
        "riferimento_nutrizionale" => "Riferimento nutrizionale",
        "energia_kcal" => "Energia (kcal)",
        "energia_kj" => "Energia (kJ)",
        "grassi_g" => "Grassi (g)",
        "saturi_g" => "Saturi (g)",
        "carboidrati_g" => "Carboidrati (g)",
        "zuccheri_g" => "Zuccheri (g)",
        "fibre_g" => "Fibre (g)",
        "proteine_g" => "Proteine (g)",
        "sale_g" => "Sale (g)",
        "modello" => "Modello",
        "numero_serie" => "Numero seriale",
        "posizione" => "Dettaglio posizione",
        "data_acquisto" => "Data acquisto",
        "prezzo_acquisto_centesimi" => "Prezzo acquisto",
        "venditore" => "Venditore",
        "valore_stimato_centesimi" => "Valore stimato",
        "condizione" => "Condizione",
        "note" => "Note",
        "visibilita" => "Visibilità",
        "stato" => "Stato",
        "foto_id" => "Foto",
        "ruolo" => "Ruolo foto",
        "percorso_file" => "File interno",
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
    fn percorso_storico_include_contenitori_annidati() {
        assert_eq!(
            format_location(
                Some("Casa principale"),
                Some("Garage"),
                Some("Armadio / Ripiano 2 / Scatola"),
            ),
            "Casa principale / Garage / Armadio / Ripiano 2 / Scatola"
        );
        assert_eq!(entity_icon("contenitore"), "📦");
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

        let (global, global_total) =
            load_filtered_global_history_page(&pool, 0, HistoryFilters::default())
                .await
                .expect("globale");
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

    #[test]
    fn filtri_storico_roundtrip_e_callback_restano_compatti() {
        let filters = HistoryFilters {
            period: HistoryPeriodFilter::Last7Days,
            module: HistoryModuleFilter::Oggetti,
            operation: HistoryOperationFilter::Modifica,
            home_id: Some(i64::MAX),
            room_id: Some(i64::MAX - 1),
            entity_id: Some(i64::MAX - 2),
        };

        let token = filters.to_token();
        assert_eq!(HistoryFilters::from_token(&token), Some(filters));

        let callback = format!(
            "h:e:{}:{}:{token}",
            base62_encode(i64::MAX),
            base62_encode(999_999)
        );
        assert!(
            callback.len() <= 64,
            "callback troppo lungo: {}",
            callback.len()
        );
    }

    #[test]
    fn filtro_modulo_alimentazione_roundtrip() {
        let filters = HistoryFilters {
            module: HistoryModuleFilter::Alimentazione,
            ..HistoryFilters::default()
        };
        let token = filters.to_token();
        assert_eq!(HistoryFilters::from_token(&token), Some(filters));
        assert_eq!(
            HistoryModuleFilter::Alimentazione.db_value(),
            Some("alimentazione")
        );
        assert_eq!(HistoryModuleFilter::Alimentazione.label(), "Alimentazione");
    }

    #[test]
    fn parser_azioni_filtri_conserva_lo_stato() {
        let filters = HistoryFilters {
            period: HistoryPeriodFilter::Today,
            module: HistoryModuleFilter::Luoghi,
            operation: HistoryOperationFilter::Spostamento,
            home_id: Some(12),
            room_id: Some(31),
            entity_id: None,
        };
        let token = filters.to_token();

        assert_eq!(
            parse_global_history_action(&format!("h:g:2:{token}")),
            Some(GlobalHistoryAction::Page { page: 2, filters })
        );
        assert_eq!(
            parse_global_history_action(&format!("h:f:{token}")),
            Some(GlobalHistoryAction::Filters { filters })
        );
    }

    #[tokio::test]
    async fn filtri_globali_si_combinano_su_modulo_operazione_luogo_ed_entita() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("connessione");

        let home_id = ensure_entity(&mut conn, "abitazione", 100, "Casa filtro")
            .await
            .expect("casa");
        let room_id = ensure_entity(&mut conn, "stanza", 200, "Garage filtro")
            .await
            .expect("stanza");
        let item_id = ensure_entity(&mut conn, "oggetto", 300, "Trapano filtro")
            .await
            .expect("oggetto");
        let other_id = ensure_entity(&mut conn, "oggetto", 301, "Altro oggetto")
            .await
            .expect("altro oggetto");

        for (entity, name) in [(item_id, "Trapano filtro"), (other_id, "Altro oggetto")] {
            record_event(
                &mut conn,
                &NewHistoryEvent {
                    entita_storico_id: entity,
                    modulo: "oggetti",
                    componente: "anagrafica",
                    operazione: "modifica",
                    nome_entita_snapshot: name,
                    abitazione_storico_id: Some(home_id),
                    abitazione_nome_snapshot: Some("Casa filtro"),
                    stanza_storico_id: Some(room_id),
                    stanza_nome_snapshot: Some("Garage filtro"),
                    evento_padre_id: None,
                },
            )
            .await
            .expect("evento");
        }
        drop(conn);

        let filters = HistoryFilters {
            period: HistoryPeriodFilter::Today,
            module: HistoryModuleFilter::Oggetti,
            operation: HistoryOperationFilter::Modifica,
            home_id: Some(home_id),
            room_id: Some(room_id),
            entity_id: Some(item_id),
        };

        let (rows, total) = load_filtered_global_history_page(&pool, 0, filters)
            .await
            .expect("filtri");
        assert_eq!(total, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].nome_entita_snapshot, "Trapano filtro");
    }

    #[tokio::test]
    async fn filtro_casa_include_anche_il_prima_dopo_di_un_cambio_luogo() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("connessione");

        let home_a = ensure_entity(&mut conn, "abitazione", 501, "Casa A")
            .await
            .expect("casa A");
        let home_b = ensure_entity(&mut conn, "abitazione", 502, "Casa B")
            .await
            .expect("casa B");
        let item_id = ensure_entity(&mut conn, "oggetto", 503, "Oggetto mobile")
            .await
            .expect("oggetto");

        let event_id = record_event(
            &mut conn,
            &NewHistoryEvent {
                entita_storico_id: item_id,
                modulo: "oggetti",
                componente: "luoghi",
                operazione: "spostamento",
                nome_entita_snapshot: "Oggetto mobile",
                abitazione_storico_id: Some(home_b),
                abitazione_nome_snapshot: Some("Casa B"),
                stanza_storico_id: None,
                stanza_nome_snapshot: None,
                evento_padre_id: None,
            },
        )
        .await
        .expect("evento");

        record_location_change(
            &mut conn,
            event_id,
            &LocationSnapshot {
                abitazione_storico_id: Some(home_a),
                abitazione_nome: Some("Casa A".to_string()),
                stanza_storico_id: None,
                stanza_nome: None,
                ..LocationSnapshot::default()
            },
            &LocationSnapshot {
                abitazione_storico_id: Some(home_b),
                abitazione_nome: Some("Casa B".to_string()),
                stanza_storico_id: None,
                stanza_nome: None,
                ..LocationSnapshot::default()
            },
        )
        .await
        .expect("cambio luogo");
        drop(conn);

        let filters = HistoryFilters {
            home_id: Some(home_a),
            ..HistoryFilters::default()
        };
        let (_, total) = load_filtered_global_history_page(&pool, 0, filters)
            .await
            .expect("filtro casa");
        assert_eq!(total, 1);
    }
    #[tokio::test]
    async fn nuovi_eventi_registrano_autore_e_distinguono_effetti_automatici() {
        let pool = test_pool().await;

        let user_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Alessio Test')")
            .execute(&pool)
            .await
            .expect("utente")
            .last_insert_rowid();
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) \
             VALUES (1, ?, 'proprietario')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("membership");

        let actor = crate::identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: "Alessio Test".to_string(),
            spazio_id: 1,
            spazio_nome_snapshot: "Spazio principale".to_string(),
            view_all: false,
            origine: "telegram",
            telegram_user_id: Some(123),
            telegram_username: Some("alessio_test".to_string()),
        };

        let (root_id, child_id) = crate::identity::with_actor(actor, async {
            let mut conn = pool.acquire().await.expect("connessione");
            let entity_id = ensure_entity(&mut conn, "oggetto", 800, "Trapano audit")
                .await
                .expect("entita");

            let root_id = record_event(
                &mut conn,
                &NewHistoryEvent {
                    entita_storico_id: entity_id,
                    modulo: "oggetti",
                    componente: "anagrafica",
                    operazione: "modifica",
                    nome_entita_snapshot: "Trapano audit",
                    abitazione_storico_id: None,
                    abitazione_nome_snapshot: None,
                    stanza_storico_id: None,
                    stanza_nome_snapshot: None,
                    evento_padre_id: None,
                },
            )
            .await
            .expect("evento principale");

            let child_id = record_event(
                &mut conn,
                &NewHistoryEvent {
                    entita_storico_id: entity_id,
                    modulo: "oggetti",
                    componente: "luoghi",
                    operazione: "spostamento",
                    nome_entita_snapshot: "Trapano audit",
                    abitazione_storico_id: None,
                    abitazione_nome_snapshot: None,
                    stanza_storico_id: None,
                    stanza_nome_snapshot: None,
                    evento_padre_id: Some(root_id),
                },
            )
            .await
            .expect("evento automatico");

            (root_id, child_id)
        })
        .await;

        let root: (
            Option<i64>,
            Option<String>,
            String,
            i64,
            i64,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT attore_utente_id, attore_nome_snapshot, origine_azione, automatico, \
                        spazio_id, spazio_nome_snapshot \
                 FROM storico_eventi WHERE id = ?",
        )
        .bind(root_id)
        .fetch_one(&pool)
        .await
        .expect("audit root");
        assert_eq!(root.0, Some(user_id));
        assert_eq!(root.1.as_deref(), Some("Alessio Test"));
        assert_eq!(root.2, "telegram");
        assert_eq!(root.3, 0);
        assert_eq!(root.4, 1);
        assert_eq!(root.5.as_deref(), Some("Spazio principale"));

        let child: (Option<i64>, i64, Option<i64>) = sqlx::query_as(
            "SELECT attore_utente_id, automatico, evento_padre_id \
             FROM storico_eventi WHERE id = ?",
        )
        .bind(child_id)
        .fetch_one(&pool)
        .await
        .expect("audit child");
        assert_eq!(child.0, Some(user_id));
        assert_eq!(child.1, 1);
        assert_eq!(child.2, Some(root_id));
    }

    #[test]
    fn dettaglio_luogo_disambigua_lo_spazio() {
        assert_eq!(
            format_location_with_space(
                Some("Casa principale"),
                Some("Camera"),
                Some("Scaffale prova"),
                Some("Test isolamento"),
                true,
            ),
            "Casa principale · Test isolamento / Camera / Scaffale prova"
        );
        assert_eq!(
            location_with_home_icon("Casa principale · Test isolamento"),
            "🏠 Casa principale · Test isolamento"
        );
        assert_eq!(location_with_home_icon("Nessun luogo"), "Nessun luogo");
    }

    #[test]
    fn storico_legacy_rende_esplicito_che_l_autore_non_e_disponibile() {
        assert_eq!(
            format_history_actor_line(None, "legacy", false),
            "👤 Autore: non disponibile (evento pre-Step 7)"
        );
        assert_eq!(origin_label("telegram"), "Telegram");
        assert!(!should_show_origin("telegram"));
        assert!(should_show_origin("google"));
        assert!(should_show_origin("sistema"));
        assert!(should_show_origin("legacy"));
    }
    #[tokio::test]
    async fn storico_globale_e_dettaglio_rispettano_lo_spazio_attivo() {
        let pool = test_pool().await;
        let space_two =
            sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Spazio due', 'personale')")
                .execute(&pool)
                .await
                .expect("spazio due")
                .last_insert_rowid();

        let actor_one = crate::identity::AuditActor::system();
        let actor_two = crate::identity::AuditActor {
            utente_id: None,
            nome_snapshot: "Sistema test".to_string(),
            spazio_id: space_two,
            spazio_nome_snapshot: "Spazio due".to_string(),
            view_all: false,
            origine: "sistema",
            telegram_user_id: None,
            telegram_username: None,
        };

        let event_one = crate::identity::with_actor(actor_one.clone(), async {
            let mut conn = pool.acquire().await.expect("connessione uno");
            let entity = ensure_entity(&mut conn, "oggetto", 9001, "Oggetto uno")
                .await
                .expect("entita uno");
            record_event(
                &mut conn,
                &NewHistoryEvent {
                    entita_storico_id: entity,
                    modulo: "oggetti",
                    componente: "anagrafica",
                    operazione: "creazione",
                    nome_entita_snapshot: "Oggetto uno",
                    abitazione_storico_id: None,
                    abitazione_nome_snapshot: None,
                    stanza_storico_id: None,
                    stanza_nome_snapshot: None,
                    evento_padre_id: None,
                },
            )
            .await
            .expect("evento uno")
        })
        .await;

        let event_two = crate::identity::with_actor(actor_two.clone(), async {
            let mut conn = pool.acquire().await.expect("connessione due");
            let entity = ensure_entity(&mut conn, "oggetto", 9002, "Oggetto due")
                .await
                .expect("entita due");
            record_event(
                &mut conn,
                &NewHistoryEvent {
                    entita_storico_id: entity,
                    modulo: "oggetti",
                    componente: "anagrafica",
                    operazione: "creazione",
                    nome_entita_snapshot: "Oggetto due",
                    abitazione_storico_id: None,
                    abitazione_nome_snapshot: None,
                    stanza_storico_id: None,
                    stanza_nome_snapshot: None,
                    evento_padre_id: None,
                },
            )
            .await
            .expect("evento due")
        })
        .await;

        crate::identity::with_actor(actor_one, async {
            let (events, total) =
                load_filtered_global_history_page(&pool, 0, HistoryFilters::default())
                    .await
                    .expect("storico spazio uno");
            assert_eq!(total, 1);
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].id, event_one);
            assert!(load_event_detail(&pool, event_two)
                .await
                .expect("dettaglio cross-space")
                .is_none());
        })
        .await;

        crate::identity::with_actor(actor_two, async {
            let (events, total) =
                load_filtered_global_history_page(&pool, 0, HistoryFilters::default())
                    .await
                    .expect("storico spazio due");
            assert_eq!(total, 1);
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].id, event_two);
            assert!(load_event_detail(&pool, event_one)
                .await
                .expect("dettaglio cross-space inverso")
                .is_none());
        })
        .await;
    }
}
