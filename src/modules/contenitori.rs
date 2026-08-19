//! Step 6C.3A - contenitori gerarchici integrati nella navigazione dei luoghi.
//! La UI consente anche di creare un oggetto direttamente dal contenitore corrente.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{bail, ensure, Result};
use sqlx::{FromRow, Sqlite, SqliteConnection, SqlitePool, Transaction};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

#[derive(Clone, Default)]
pub struct ContainerSessionStore {
    inner: Arc<Mutex<HashMap<i64, ContainerConversationState>>>,
}

impl ContainerSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.with_sessions(|sessions| {
            sessions.remove(&chat_id);
        });
    }

    fn get(&self, chat_id: i64) -> Option<ContainerConversationState> {
        self.with_sessions(|sessions| sessions.get(&chat_id).cloned())
    }

    fn set(&self, chat_id: i64, state: ContainerConversationState) {
        self.with_sessions(|sessions| {
            sessions.insert(chat_id, state);
        });
    }

    fn with_sessions<T>(
        &self,
        f: impl FnOnce(&mut HashMap<i64, ContainerConversationState>) -> T,
    ) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

#[derive(Debug, Clone)]
enum ContainerConversationState {
    AwaitingName {
        home_id: i64,
        room_id: Option<i64>,
        parent_id: Option<i64>,
        rename_id: Option<i64>,
    },
}

#[derive(Debug, Clone, FromRow)]
struct UiHome {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, FromRow)]
struct UiRoom {
    id: i64,
    home_id: i64,
    name: String,
    home_name: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ContainerRecord {
    pub id: i64,
    pub home_id: i64,
    pub room_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerBreadcrumb {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPath {
    pub home_id: i64,
    pub home_name: String,
    pub room_id: Option<i64>,
    pub room_name: Option<String>,
    pub containers: Vec<ContainerBreadcrumb>,
}

#[derive(Debug, FromRow)]
struct BreadcrumbRow {
    id: i64,
    name: String,
}

#[derive(Debug, FromRow)]
struct ScopeNames {
    home_name: String,
    room_name: Option<String>,
}

pub async fn create_container(
    pool: &SqlitePool,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
    name: &str,
    description: Option<&str>,
) -> Result<i64> {
    let name = clean_required_name(name)?;
    let description = clean_optional_text(description);
    let mut tx = pool.begin().await?;

    validate_scope(&mut tx, home_id, room_id).await?;
    validate_parent_target(&mut tx, None, home_id, room_id, parent_id).await?;

    let result = sqlx::query(
        "INSERT INTO contenitori \
         (abitazione_id, stanza_id, contenitore_padre_id, nome, descrizione) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(home_id)
    .bind(room_id)
    .bind(parent_id)
    .bind(name)
    .bind(description)
    .execute(&mut *tx)
    .await?;

    let id = result.last_insert_rowid();
    tx.commit().await?;
    Ok(id)
}

pub async fn get_container(pool: &SqlitePool, id: i64) -> Result<Option<ContainerRecord>> {
    Ok(sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list_container_children(
    pool: &SqlitePool,
    parent_id: i64,
) -> Result<Vec<ContainerRecord>> {
    Ok(sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori WHERE contenitore_padre_id = ? \
         ORDER BY nome COLLATE NOCASE, id",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await?)
}

pub async fn rename_container(pool: &SqlitePool, id: i64, name: &str) -> Result<bool> {
    let name = clean_required_name(name)?;
    let Some(current) = get_container(pool, id).await? else {
        return Ok(false);
    };
    if current.name == name {
        return Ok(false);
    }

    let result = sqlx::query(
        "UPDATE contenitori SET nome = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(name)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn set_container_description(
    pool: &SqlitePool,
    id: i64,
    description: Option<&str>,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE contenitori SET descrizione = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(clean_optional_text(description))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn container_path(pool: &SqlitePool, id: i64) -> Result<Option<ContainerPath>> {
    let Some(container) = get_container(pool, id).await? else {
        return Ok(None);
    };

    let scope = sqlx::query_as::<_, ScopeNames>(
        "SELECT a.nome AS home_name, s.nome AS room_name \
         FROM abitazioni a LEFT JOIN stanze s ON s.id = ? WHERE a.id = ?",
    )
    .bind(container.room_id)
    .bind(container.home_id)
    .fetch_one(pool)
    .await?;

    let containers = sqlx::query_as::<_, BreadcrumbRow>(
        "WITH RECURSIVE chain(id, nome, contenitore_padre_id, depth) AS ( \
            SELECT id, nome, contenitore_padre_id, 0 FROM contenitori WHERE id = ? \
            UNION ALL \
            SELECT c.id, c.nome, c.contenitore_padre_id, chain.depth + 1 \
            FROM contenitori c JOIN chain ON chain.contenitore_padre_id = c.id \
         ) SELECT id, nome AS name FROM chain ORDER BY depth DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| ContainerBreadcrumb {
        id: row.id,
        name: row.name,
    })
    .collect();

    Ok(Some(ContainerPath {
        home_id: container.home_id,
        home_name: scope.home_name,
        room_id: container.room_id,
        room_name: scope.room_name,
        containers,
    }))
}

pub async fn assign_item_to_container(
    pool: &SqlitePool,
    item_id: i64,
    container_id: i64,
) -> Result<()> {
    let container = get_container(pool, container_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("contenitore #{container_id} inesistente"))?;

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE id = ?)")
        .bind(item_id)
        .fetch_one(pool)
        .await?;
    ensure!(exists, "item #{item_id} inesistente");

    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(item_id) DO UPDATE SET \
             abitazione_id = excluded.abitazione_id, \
             stanza_id = excluded.stanza_id, \
             contenitore_id = excluded.contenitore_id",
    )
    .bind(item_id)
    .bind(container.home_id)
    .bind(container.room_id)
    .bind(container.id)
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn insert_item_location_in_container(
    tx: &mut Transaction<'_, Sqlite>,
    item_id: i64,
    container_id: i64,
) -> Result<()> {
    let scope = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT abitazione_id, stanza_id FROM contenitori WHERE id = ?",
    )
    .bind(container_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| anyhow::anyhow!("contenitore #{container_id} inesistente"))?;

    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(item_id) DO UPDATE SET \
             abitazione_id = excluded.abitazione_id, \
             stanza_id = excluded.stanza_id, \
             contenitore_id = excluded.contenitore_id",
    )
    .bind(item_id)
    .bind(scope.0)
    .bind(scope.1)
    .bind(container_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn move_container(
    pool: &SqlitePool,
    id: i64,
    new_home_id: i64,
    new_room_id: Option<i64>,
    new_parent_id: Option<i64>,
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let Some(current) = get_container_conn(&mut tx, id).await? else {
        return Ok(false);
    };

    validate_scope(&mut tx, new_home_id, new_room_id).await?;
    validate_parent_target(&mut tx, Some(id), new_home_id, new_room_id, new_parent_id).await?;

    if current.home_id == new_home_id
        && current.room_id == new_room_id
        && current.parent_id == new_parent_id
    {
        return Ok(false);
    }

    sqlx::query("UPDATE contenitori SET contenitore_padre_id = NULL WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE contenitori SET abitazione_id = ?, stanza_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id IN ( \
            WITH RECURSIVE subtree(id) AS ( \
                SELECT ? UNION ALL \
                SELECT c.id FROM contenitori c \
                JOIN subtree s ON c.contenitore_padre_id = s.id \
            ) SELECT id FROM subtree \
         )",
    )
    .bind(new_home_id)
    .bind(new_room_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE item_luogo SET abitazione_id = ?, stanza_id = ? \
         WHERE contenitore_id IN ( \
            WITH RECURSIVE subtree(id) AS ( \
                SELECT ? UNION ALL \
                SELECT c.id FROM contenitori c \
                JOIN subtree s ON c.contenitore_padre_id = s.id \
            ) SELECT id FROM subtree \
         )",
    )
    .bind(new_home_id)
    .bind(new_room_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE contenitori SET contenitore_padre_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(new_parent_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(true)
}

pub async fn delete_container(pool: &SqlitePool, id: i64) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let Some(container) = get_container_conn(&mut tx, id).await? else {
        return Ok(false);
    };

    sqlx::query(
        "UPDATE contenitori SET contenitore_padre_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE contenitore_padre_id = ?",
    )
    .bind(container.parent_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE item_luogo SET contenitore_id = ? WHERE contenitore_id = ?")
        .bind(container.parent_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    let result = sqlx::query("DELETE FROM contenitori WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

async fn get_container_conn(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Option<ContainerRecord>> {
    Ok(sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

async fn validate_scope(
    conn: &mut SqliteConnection,
    home_id: i64,
    room_id: Option<i64>,
) -> Result<()> {
    let home_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM abitazioni WHERE id = ?)")
            .bind(home_id)
            .fetch_one(&mut *conn)
            .await?;
    ensure!(home_exists, "abitazione #{home_id} inesistente");

    if let Some(room_id) = room_id {
        let coherent: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM stanze WHERE id = ? AND abitazione_id = ?)",
        )
        .bind(room_id)
        .bind(home_id)
        .fetch_one(&mut *conn)
        .await?;
        ensure!(coherent, "stanza non appartenente all'abitazione");
    }
    Ok(())
}

async fn validate_parent_target(
    conn: &mut SqliteConnection,
    moving_id: Option<i64>,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
) -> Result<()> {
    let Some(parent_id) = parent_id else {
        return Ok(());
    };

    let parent = get_container_conn(conn, parent_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("contenitore padre #{parent_id} inesistente"))?;

    ensure!(
        parent.home_id == home_id && parent.room_id == room_id,
        "il contenitore padre appartiene a un luogo differente"
    );

    if let Some(moving_id) = moving_id {
        ensure!(
            moving_id != parent_id,
            "un contenitore non può contenere se stesso"
        );

        let inside: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
                WITH RECURSIVE subtree(id) AS ( \
                    SELECT id FROM contenitori WHERE id = ? \
                    UNION ALL \
                    SELECT c.id FROM contenitori c \
                    JOIN subtree s ON c.contenitore_padre_id = s.id \
                ) SELECT 1 FROM subtree WHERE id = ? \
             )",
        )
        .bind(moving_id)
        .bind(parent_id)
        .fetch_one(conn)
        .await?;

        if inside {
            bail!("il nuovo padre è un discendente: ciclo rifiutato");
        }
    }
    Ok(())
}

fn clean_required_name(value: &str) -> Result<String> {
    let value = value.trim();
    ensure!(!value.is_empty(), "nome contenitore vuoto");
    ensure!(
        value.chars().count() <= 120,
        "nome contenitore troppo lungo (massimo 120 caratteri)"
    );
    Ok(value.to_string())
}

fn clean_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

// -----------------------------------------------------------------------------
// Step 6C.2 - Interfaccia Telegram dei contenitori.
// -----------------------------------------------------------------------------

pub async fn show_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "📦 Contenitori\n\nGestisci armadi, scatole, ripiani e altre sotto-posizioni annidabili.\n\nPuoi creare un oggetto direttamente dal luogo corrente; la gestione completa degli spostamenti oggetto ↔ contenitore continua nel 6C.3.",
    )
    .reply_markup(containers_menu_keyboard())
    .await?;
    Ok(())
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &ContainerSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if let Some((command, args)) = parse_command(text) {
        match command {
            "/contenitori" => {
                sessions.clear_chat(chat_id);
                show_menu(bot, msg.chat.id).await?;
                return Ok(true);
            }
            "/contenitore" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_decimal_id(args) {
                    show_container_detail(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /contenitore <id>\nEsempio: /contenitore 3",
                    )
                    .reply_markup(containers_menu_keyboard())
                    .await?;
                }
                return Ok(true);
            }
            "/annulla" => {
                if sessions.get(chat_id).is_some() {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "Operazione sui contenitori annullata.")
                        .reply_markup(containers_menu_keyboard())
                        .await?;
                    return Ok(true);
                }
                return Ok(false);
            }
            _ => return Ok(false),
        }
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        ContainerConversationState::AwaitingName {
            home_id,
            room_id,
            parent_id,
            rename_id: None,
        } => {
            create_container_from_input(
                bot,
                msg.chat.id,
                pool,
                sessions,
                home_id,
                room_id,
                parent_id,
                text,
            )
            .await?;
        }
        ContainerConversationState::AwaitingName {
            rename_id: Some(id),
            ..
        } => {
            rename_container_from_input(bot, msg.chat.id, pool, sessions, id, text).await?;
        }
    }

    Ok(true)
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ContainerSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    if !data.starts_with("c:") {
        return Ok(false);
    }

    let raw_chat_id = chat_id.0;

    match data {
        "c:menu" => {
            sessions.clear_chat(raw_chat_id);
            show_menu(bot, chat_id).await?;
            return Ok(true);
        }
        "c:n" => {
            sessions.clear_chat(raw_chat_id);
            show_home_picker(bot, chat_id, pool, HomePickerMode::Create).await?;
            return Ok(true);
        }
        "c:l" => {
            sessions.clear_chat(raw_chat_id);
            show_home_picker(bot, chat_id, pool, HomePickerMode::List).await?;
            return Ok(true);
        }
        "c:a" => {
            sessions.clear_chat(raw_chat_id);
            show_all_containers(bot, chat_id, pool).await?;
            return Ok(true);
        }
        _ => {}
    }

    if let Some(home_id) = parse_one_callback(data, "c:nh:") {
        sessions.clear_chat(raw_chat_id);
        show_scope_picker_for_new(bot, chat_id, pool, home_id).await?;
        return Ok(true);
    }

    if let Some(home_id) = parse_one_callback(data, "c:lh:") {
        sessions.clear_chat(raw_chat_id);
        show_home_scope(bot, chat_id, pool, home_id).await?;
        return Ok(true);
    }

    if let Some(room_id) = parse_one_callback(data, "c:lr:") {
        sessions.clear_chat(raw_chat_id);
        show_room_scope(bot, chat_id, pool, room_id).await?;
        return Ok(true);
    }

    if let Some((home_id, room_raw)) = parse_two_callback(data, "c:nn:") {
        let room_id = zero_as_none(room_raw);
        if !scope_exists(pool, home_id, room_id).await {
            bot.send_message(chat_id, "⚠️ La destinazione scelta non esiste più.")
                .reply_markup(containers_menu_keyboard())
                .await?;
            return Ok(true);
        }
        sessions.set(
            raw_chat_id,
            ContainerConversationState::AwaitingName {
                home_id,
                room_id,
                parent_id: None,
                rename_id: None,
            },
        );
        ask_container_name(bot, chat_id, None).await?;
        return Ok(true);
    }

    if let Some(parent_id) = parse_one_callback(data, "c:nc:") {
        match get_container(pool, parent_id).await {
            Ok(Some(parent)) => {
                sessions.set(
                    raw_chat_id,
                    ContainerConversationState::AwaitingName {
                        home_id: parent.home_id,
                        room_id: parent.room_id,
                        parent_id: Some(parent.id),
                        rename_id: None,
                    },
                );
                ask_container_name(bot, chat_id, Some(&parent.name)).await?;
            }
            Ok(None) => {
                bot.send_message(chat_id, format!("Contenitore #{parent_id} non trovato."))
                    .reply_markup(containers_menu_keyboard())
                    .await?;
            }
            Err(error) => {
                tracing::error!(?error, parent_id, "Errore lettura contenitore padre");
                bot.send_message(chat_id, "⚠️ Non riesco a leggere il contenitore.")
                    .await?;
            }
        }
        return Ok(true);
    }

    if let Some(id) = parse_one_callback(data, "c:v:") {
        sessions.clear_chat(raw_chat_id);
        show_container_detail(bot, chat_id, pool, id).await?;
        return Ok(true);
    }

    if let Some(id) = parse_one_callback(data, "c:r:") {
        match get_container(pool, id).await {
            Ok(Some(container)) => {
                sessions.set(
                    raw_chat_id,
                    ContainerConversationState::AwaitingName {
                        home_id: container.home_id,
                        room_id: container.room_id,
                        parent_id: container.parent_id,
                        rename_id: Some(id),
                    },
                );
                bot.send_message(
                    chat_id,
                    format!(
                        "✏️ Rinomina contenitore\n\nNome attuale: {}\n\nScrivi il nuovo nome oppure /annulla.",
                        container.name
                    ),
                )
                .await?;
            }
            Ok(None) => {
                bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
                    .reply_markup(containers_menu_keyboard())
                    .await?;
            }
            Err(error) => {
                tracing::error!(?error, container_id = id, "Errore lettura contenitore");
                bot.send_message(chat_id, "⚠️ Non riesco a leggere il contenitore.")
                    .await?;
            }
        }
        return Ok(true);
    }

    if let Some(id) = parse_one_callback(data, "c:da:") {
        sessions.clear_chat(raw_chat_id);
        show_delete_confirmation(bot, chat_id, pool, id).await?;
        return Ok(true);
    }

    if let Some(id) = parse_one_callback(data, "c:dd:") {
        sessions.clear_chat(raw_chat_id);
        delete_container_and_report(bot, chat_id, pool, id).await?;
        return Ok(true);
    }

    if let Some(id) = parse_one_callback(data, "c:m:") {
        sessions.clear_chat(raw_chat_id);
        show_move_home_picker(bot, chat_id, pool, id).await?;
        return Ok(true);
    }

    if let Some((id, home_id)) = parse_two_callback(data, "c:mh:") {
        sessions.clear_chat(raw_chat_id);
        show_move_scope_picker(bot, chat_id, pool, id, home_id).await?;
        return Ok(true);
    }

    if let Some((id, home_id, room_raw)) = parse_three_callback(data, "c:ms:") {
        sessions.clear_chat(raw_chat_id);
        show_move_parent_picker(bot, chat_id, pool, id, home_id, zero_as_none(room_raw)).await?;
        return Ok(true);
    }

    if let Some((id, home_id, room_raw, parent_raw)) = parse_four_callback(data, "c:mt:") {
        sessions.clear_chat(raw_chat_id);
        move_container_and_report(
            bot,
            chat_id,
            pool,
            id,
            home_id,
            zero_as_none(room_raw),
            zero_as_none(parent_raw),
        )
        .await?;
        return Ok(true);
    }

    Ok(false)
}

#[derive(Clone, Copy)]
enum HomePickerMode {
    Create,
    List,
}

async fn show_home_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    mode: HomePickerMode,
) -> ResponseResult<()> {
    let homes = match list_ui_homes(pool).await {
        Ok(homes) => homes,
        Err(error) => {
            tracing::error!(?error, "Errore elenco case per contenitori");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le case.")
                .reply_markup(containers_menu_keyboard())
                .await?;
            return Ok(());
        }
    };

    if homes.is_empty() {
        bot.send_message(
            chat_id,
            "📦 Per usare i contenitori serve prima almeno una casa.",
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![button("➕ Crea una casa", "loc:home:new")],
            vec![
                button("↩️ Case, stanze e contenitori", "loc:menu"),
                button("🏠 Menu principale", "menu:main"),
            ],
        ]))
        .await?;
        return Ok(());
    }

    let (title, prefix) = match mode {
        HomePickerMode::Create => (
            "➕ Nuovo contenitore\n\nScegli la casa in cui si trova.",
            "c:nh:",
        ),
        HomePickerMode::List => ("📋 Elenco contenitori\n\nScegli una casa.", "c:lh:"),
    };

    let mut rows = homes
        .iter()
        .take(30)
        .map(|home| {
            vec![button(
                &format!("🏠 {}", truncate_chars(&home.name, 42)),
                &format!("{prefix}{}", encode_id(home.id)),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![
        button("↩️ Contenitori", "c:menu"),
        button("🏠 Menu principale", "menu:main"),
    ]);

    bot.send_message(chat_id, title)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_scope_picker_for_new(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    home_id: i64,
) -> ResponseResult<()> {
    let Some(home) = read_ui_home(pool, home_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Casa #{home_id} non trovata."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };
    let rooms = list_ui_rooms_for_home(pool, home_id)
        .await
        .unwrap_or_default();

    let mut rows = vec![vec![button(
        "🏠 Direttamente nella casa",
        &format!("c:nn:{}:0", encode_id(home_id)),
    )]];
    for room in rooms.iter().take(30) {
        rows.push(vec![button(
            &format!("🚪 {}", truncate_chars(&room.name, 42)),
            &format!("c:nn:{}:{}", encode_id(home_id), encode_id(room.id)),
        )]);
    }
    rows.push(vec![button("↩️ Cambia casa", "c:n")]);
    rows.push(vec![button("🏠 Menu principale", "menu:main")]);

    bot.send_message(
        chat_id,
        format!(
            "➕ Nuovo contenitore\n\n🏠 {}\n\nScegli se il contenitore è direttamente nella casa oppure in una stanza.",
            home.name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn ask_container_name(
    bot: &Bot,
    chat_id: ChatId,
    parent_name: Option<&str>,
) -> ResponseResult<()> {
    let text = if let Some(parent_name) = parent_name {
        format!(
            "➕ Nuovo contenitore interno\n\nDentro: 📦 {parent_name}\n\nScrivi il nome del nuovo contenitore oppure /annulla."
        )
    } else {
        "➕ Nuovo contenitore\n\nScrivi il nome del contenitore oppure /annulla.".to_string()
    };
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button("↩️ Contenitori", "c:menu"),
            button("🏠 Menu principale", "menu:main"),
        ]]))
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_container_from_input(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ContainerSessionStore,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
    input: &str,
) -> ResponseResult<()> {
    let Some(name) = clean_ui_name(input) else {
        bot.send_message(
            chat_id,
            "⚠️ Il nome deve contenere da 1 a 120 caratteri. Riprova oppure usa /annulla.",
        )
        .await?;
        return Ok(());
    };

    match create_container(pool, home_id, room_id, parent_id, &name, None).await {
        Ok(id) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("✅ Contenitore creato: 📦 {name}"))
                .await?;
            show_container_detail(bot, chat_id, pool, id).await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                home_id,
                room_id,
                parent_id,
                "Errore creazione contenitore"
            );
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a creare il contenitore. Verifica che non esista già un contenitore con lo stesso nome allo stesso livello.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn rename_container_from_input(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ContainerSessionStore,
    id: i64,
    input: &str,
) -> ResponseResult<()> {
    let Some(name) = clean_ui_name(input) else {
        bot.send_message(
            chat_id,
            "⚠️ Il nome deve contenere da 1 a 120 caratteri. Riprova oppure usa /annulla.",
        )
        .await?;
        return Ok(());
    };

    let before = match get_container(pool, id).await {
        Ok(Some(container)) => container,
        Ok(None) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
                .reply_markup(containers_menu_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, container_id = id, "Errore lettura contenitore");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere il contenitore.")
                .await?;
            return Ok(());
        }
    };

    match rename_container(pool, id, &name).await {
        Ok(true) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(
                chat_id,
                format!("✅ Contenitore rinominato: {} → {}", before.name, name),
            )
            .await?;
            show_container_detail(bot, chat_id, pool, id).await?;
        }
        Ok(false) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(
                chat_id,
                "ℹ️ Il nome è già quello attuale: nessuna modifica.",
            )
            .await?;
            show_container_detail(bot, chat_id, pool, id).await?;
        }
        Err(error) => {
            tracing::error!(?error, container_id = id, "Errore rinomina contenitore");
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a rinominare il contenitore. Potrebbe esistere già un contenitore con quel nome allo stesso livello.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn show_all_containers(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let containers = sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori ORDER BY nome COLLATE NOCASE, id LIMIT 100",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if containers.is_empty() {
        bot.send_message(chat_id, "📦 Non ci sono ancora contenitori registrati.")
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    }

    let mut text = "📦 Elenco contenitori\n\n".to_string();
    let mut rows = Vec::new();
    for container in containers.iter().take(40) {
        let path = container_path(pool, container.id)
            .await
            .ok()
            .flatten()
            .map(|path| format_container_path(&path))
            .unwrap_or_else(|| "Percorso non disponibile".to_string());
        text.push_str(&format!(
            "#{} · {}\n📍 {}\n/luogo_c{}\n\n",
            container.id, container.name, path, container.id
        ));
        rows.push(vec![button(
            &format!(
                "📦 #{} · {}",
                container.id,
                truncate_chars(&container.name, 32)
            ),
            &format!("c:v:{}", encode_id(container.id)),
        )]);
        if text.chars().count() > 3200 {
            text.push_str(
                "… elenco testuale abbreviato. Usa i pulsanti o la struttura completa.\n",
            );
            break;
        }
    }
    rows.push(vec![
        button("↩️ Contenitori", "c:menu"),
        button("🏠 Menu principale", "menu:main"),
    ]);

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_home_scope(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    home_id: i64,
) -> ResponseResult<()> {
    let Some(home) = read_ui_home(pool, home_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Casa #{home_id} non trovata."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };
    let rooms = list_ui_rooms_for_home(pool, home_id)
        .await
        .unwrap_or_default();
    let roots = list_root_containers(pool, home_id, None)
        .await
        .unwrap_or_default();

    let mut rows = roots
        .iter()
        .take(20)
        .map(|container| {
            vec![button(
                &format!(
                    "📦 #{} · {}",
                    container.id,
                    truncate_chars(&container.name, 34)
                ),
                &format!("c:v:{}", encode_id(container.id)),
            )]
        })
        .collect::<Vec<_>>();

    for room in rooms.iter().take(25) {
        rows.push(vec![button(
            &format!("🚪 {}", truncate_chars(&room.name, 42)),
            &format!("c:lr:{}", encode_id(room.id)),
        )]);
    }
    rows.push(vec![button(
        "➕ Nuovo contenitore nella casa",
        &format!("c:nn:{}:0", encode_id(home_id)),
    )]);
    rows.push(vec![button(
        "🚪 Nuova stanza",
        &format!("loc:room:new:{home_id}"),
    )]);
    rows.push(vec![button(
        "➕ Nuovo oggetto qui",
        &format!("oggetti:newat:h:{home_id}"),
    )]);
    rows.push(vec![
        button("↩️ Elenco contenitori", "c:a"),
        button("🏠 Menu principale", "menu:main"),
    ]);

    bot.send_message(
        chat_id,
        format!(
            "📦 Contenitori — 🏠 {}\n\nDirettamente nella casa: {}\nStanze: {}\n\nApri un contenitore o una stanza.",
            home.name,
            roots.len(),
            rooms.len()
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_room_scope(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    room_id: i64,
) -> ResponseResult<()> {
    let Some(room) = read_ui_room(pool, room_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Stanza #{room_id} non trovata."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };
    let roots = list_root_containers(pool, room.home_id, Some(room.id))
        .await
        .unwrap_or_default();

    let mut rows = roots
        .iter()
        .take(30)
        .map(|container| {
            vec![button(
                &format!(
                    "📦 #{} · {}",
                    container.id,
                    truncate_chars(&container.name, 34)
                ),
                &format!("c:v:{}", encode_id(container.id)),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        "➕ Nuovo contenitore nella stanza",
        &format!("c:nn:{}:{}", encode_id(room.home_id), encode_id(room.id)),
    )]);
    rows.push(vec![button(
        "➕ Nuovo oggetto qui",
        &format!("oggetti:newat:r:{}", room.id),
    )]);
    rows.push(vec![
        button(
            "↩️ Torna alla casa",
            &format!("c:lh:{}", encode_id(room.home_id)),
        ),
        button("🏠 Menu principale", "menu:main"),
    ]);

    let empty_note = if roots.is_empty() {
        "\n\nNon ci sono ancora contenitori direttamente in questa stanza."
    } else {
        ""
    };

    bot.send_message(
        chat_id,
        format!(
            "📦 Contenitori\n\n🏠 {} / 🚪 {}\n\nContenitori principali: {}{}",
            room.home_name,
            room.name,
            roots.len(),
            empty_note
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

pub(crate) async fn show_container_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    let container = match get_container(pool, id).await {
        Ok(Some(container)) => container,
        Ok(None) => {
            bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
                .reply_markup(containers_menu_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, container_id = id, "Errore lettura contenitore");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere il contenitore.")
                .await?;
            return Ok(());
        }
    };

    let path = match container_path(pool, id).await {
        Ok(Some(path)) => path,
        Ok(None) => {
            bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, container_id = id, "Errore percorso contenitore");
            bot.send_message(chat_id, "⚠️ Non riesco a ricostruire il percorso.")
                .await?;
            return Ok(());
        }
    };

    let children = list_container_children(pool, id).await.unwrap_or_default();
    let direct_items = count_items_in_container(pool, id).await.unwrap_or(0);
    let path_text = format_container_path(&path);
    let description = container
        .description
        .as_deref()
        .map(|value| format!("\nDescrizione: {value}"))
        .unwrap_or_default();

    let mut rows = children
        .iter()
        .take(20)
        .map(|child| {
            vec![button(
                &format!("📦 {}", truncate_chars(&child.name, 42)),
                &format!("c:v:{}", encode_id(child.id)),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        "➕ Nuovo contenitore interno",
        &format!("c:nc:{}", encode_id(id)),
    )]);
    rows.push(vec![button(
        "➕ Nuovo oggetto qui",
        &format!("oggetti:newat:c:{id}"),
    )]);
    rows.push(vec![
        button("✏️ Rinomina", &format!("c:r:{}", encode_id(id))),
        button("🚚 Sposta", &format!("c:m:{}", encode_id(id))),
    ]);
    rows.push(vec![button(
        "🗑 Elimina",
        &format!("c:da:{}", encode_id(id)),
    )]);

    if let Some(parent_id) = container.parent_id {
        rows.push(vec![
            button(
                "↩️ Contenitore superiore",
                &format!("c:v:{}", encode_id(parent_id)),
            ),
            button("🏠 Menu principale", "menu:main"),
        ]);
    } else if let Some(room_id) = container.room_id {
        rows.push(vec![
            button(
                "↩️ Contenitori della stanza",
                &format!("c:lr:{}", encode_id(room_id)),
            ),
            button("🏠 Menu principale", "menu:main"),
        ]);
    } else {
        rows.push(vec![
            button(
                "↩️ Contenitori della casa",
                &format!("c:lh:{}", encode_id(container.home_id)),
            ),
            button("🏠 Menu principale", "menu:main"),
        ]);
    }

    bot.send_message(
        chat_id,
        format!(
            "📦 #{} · {}\n\n📍 {}\n\nContiene direttamente:\n📦 {} sottocontenitori\n📦 {} oggetti{}",
            container.id,
            container.name,
            path_text,
            children.len(),
            direct_items,
            description
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_delete_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    let Some(container) = get_container(pool, id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };
    let children = list_container_children(pool, id).await.unwrap_or_default();
    let direct_items = count_items_in_container(pool, id).await.unwrap_or(0);

    let promotion = if container.parent_id.is_some() {
        "I sottocontenitori e gli oggetti diretti verranno spostati nel contenitore superiore."
    } else {
        "I sottocontenitori e gli oggetti diretti resteranno direttamente nella stessa casa o stanza."
    };

    bot.send_message(
        chat_id,
        format!(
            "🗑 Eliminare 📦 {}?\n\nSottocontenitori diretti: {}\nOggetti diretti: {}\n\n{}\n\nNessun oggetto verrà eliminato.",
            container.name,
            children.len(),
            direct_items,
            promotion
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button(
            "🗑 Sì, elimina contenitore",
            &format!("c:dd:{}", encode_id(id)),
        )],
        vec![
            button("↩️ Annulla", &format!("c:v:{}", encode_id(id))),
            button("🏠 Menu principale", "menu:main"),
        ],
    ]))
    .await?;
    Ok(())
}

async fn delete_container_and_report(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    let Some(before) = get_container(pool, id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };

    match delete_container(pool, id).await {
        Ok(true) => {
            bot.send_message(
                chat_id,
                format!(
                    "✅ Contenitore eliminato: {}\nNessun oggetto è stato cancellato.",
                    before.name
                ),
            )
            .await?;
            if let Some(parent_id) = before.parent_id {
                show_container_detail(bot, chat_id, pool, parent_id).await?;
            } else if let Some(room_id) = before.room_id {
                show_room_scope(bot, chat_id, pool, room_id).await?;
            } else {
                show_home_scope(bot, chat_id, pool, before.home_id).await?;
            }
        }
        Ok(false) => {
            bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
                .reply_markup(containers_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, container_id = id, "Errore eliminazione contenitore");
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a eliminare il contenitore. Se la promozione creasse due contenitori con lo stesso nome allo stesso livello, rinomina prima uno dei due.",
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![button(
                    "↩️ Torna al contenitore",
                    &format!("c:v:{}", encode_id(id)),
                )],
                vec![button("🏠 Menu principale", "menu:main")],
            ]))
            .await?;
        }
    }
    Ok(())
}

async fn show_move_home_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    let Some(container) = get_container(pool, id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };
    let homes = list_ui_homes(pool).await.unwrap_or_default();
    let mut rows = homes
        .iter()
        .take(30)
        .map(|home| {
            vec![button(
                &format!("🏠 {}", truncate_chars(&home.name, 42)),
                &format!("c:mh:{}:{}", encode_id(id), encode_id(home.id)),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![
        button("↩️ Annulla", &format!("c:v:{}", encode_id(id))),
        button("🏠 Menu principale", "menu:main"),
    ]);

    bot.send_message(
        chat_id,
        format!(
            "🚚 Sposta contenitore\n\n📦 {}\n\nScegli la casa di destinazione. Tutto il contenuto seguirà il contenitore.",
            container.name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_move_scope_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    home_id: i64,
) -> ResponseResult<()> {
    if get_container(pool, id).await.unwrap_or(None).is_none() {
        bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    }
    let Some(home) = read_ui_home(pool, home_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Casa #{home_id} non trovata."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };
    let rooms = list_ui_rooms_for_home(pool, home_id)
        .await
        .unwrap_or_default();
    let mut rows = vec![vec![button(
        "🏠 Direttamente nella casa",
        &format!("c:ms:{}:{}:0", encode_id(id), encode_id(home_id)),
    )]];
    for room in rooms.iter().take(30) {
        rows.push(vec![button(
            &format!("🚪 {}", truncate_chars(&room.name, 42)),
            &format!(
                "c:ms:{}:{}:{}",
                encode_id(id),
                encode_id(home_id),
                encode_id(room.id)
            ),
        )]);
    }
    rows.push(vec![
        button("↩️ Cambia casa", &format!("c:m:{}", encode_id(id))),
        button("🏠 Menu principale", "menu:main"),
    ]);

    bot.send_message(
        chat_id,
        format!(
            "🚚 Sposta contenitore\n\nDestinazione: 🏠 {}\n\nScegli la stanza oppure la sola casa.",
            home.name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_move_parent_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    home_id: i64,
    room_id: Option<i64>,
) -> ResponseResult<()> {
    let Some(container) = get_container(pool, id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Contenitore #{id} non trovato."))
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    };
    if !scope_exists(pool, home_id, room_id).await {
        bot.send_message(chat_id, "⚠️ La destinazione scelta non esiste più.")
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    }

    let candidates = list_move_parent_candidates(pool, id, home_id, room_id)
        .await
        .unwrap_or_default();
    let scope_label = scope_label(pool, home_id, room_id).await;
    let direct_target_label = direct_scope_move_label(pool, home_id, room_id).await;

    let mut rows = vec![vec![button(
        &direct_target_label,
        &move_target_callback(id, home_id, room_id, None),
    )]];
    for candidate in candidates.iter().take(30) {
        rows.push(vec![button(
            &format!(
                "📦 #{} · {}",
                candidate.id,
                truncate_chars(&candidate.name, 34)
            ),
            &move_target_callback(id, home_id, room_id, Some(candidate.id)),
        )]);
    }
    rows.push(vec![
        button(
            "↩️ Cambia stanza",
            &format!("c:mh:{}:{}", encode_id(id), encode_id(home_id)),
        ),
        button("🏠 Menu principale", "menu:main"),
    ]);

    let note = if candidates.is_empty() {
        "\n\nNon ci sono altri contenitori validi qui: puoi spostarlo direttamente nella casa o stanza scelta."
    } else {
        "\n\nPuoi spostarlo direttamente nella casa/stanza scelta oppure dentro un altro contenitore."
    };

    bot.send_message(
        chat_id,
        format!(
            "🚚 Sposta 📦 {}\n\nDestinazione: {}{}",
            container.name, scope_label, note
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn move_container_and_report(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
) -> ResponseResult<()> {
    if !scope_exists(pool, home_id, room_id).await {
        bot.send_message(chat_id, "⚠️ La destinazione scelta non esiste più.")
            .reply_markup(containers_menu_keyboard())
            .await?;
        return Ok(());
    }

    match move_container(pool, id, home_id, room_id, parent_id).await {
        Ok(true) => {
            bot.send_message(
                chat_id,
                "✅ Contenitore spostato. Sottocontenitori e oggetti contenuti hanno seguito lo spostamento.",
            )
            .await?;
            show_container_detail(bot, chat_id, pool, id).await?;
        }
        Ok(false) => {
            bot.send_message(chat_id, "ℹ️ Il contenitore è già in quella posizione.")
                .await?;
            show_container_detail(bot, chat_id, pool, id).await?;
        }
        Err(error) => {
            tracing::error!(?error, container_id = id, "Errore spostamento contenitore");
            bot.send_message(
                chat_id,
                "⚠️ Spostamento non riuscito. La destinazione potrebbe creare un ciclo oppure un conflitto di nomi.",
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![button(
                    "↩️ Torna al contenitore",
                    &format!("c:v:{}", encode_id(id)),
                )],
                vec![button("🏠 Menu principale", "menu:main")],
            ]))
            .await?;
        }
    }
    Ok(())
}

async fn list_ui_homes(pool: &SqlitePool) -> Result<Vec<UiHome>, sqlx::Error> {
    sqlx::query_as::<_, UiHome>(
        "SELECT id, nome AS name FROM abitazioni ORDER BY nome COLLATE NOCASE, id",
    )
    .fetch_all(pool)
    .await
}

async fn read_ui_home(pool: &SqlitePool, id: i64) -> Result<Option<UiHome>, sqlx::Error> {
    sqlx::query_as::<_, UiHome>("SELECT id, nome AS name FROM abitazioni WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

async fn list_ui_rooms_for_home(
    pool: &SqlitePool,
    home_id: i64,
) -> Result<Vec<UiRoom>, sqlx::Error> {
    sqlx::query_as::<_, UiRoom>(
        "SELECT s.id, s.abitazione_id AS home_id, s.nome AS name, a.nome AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.abitazione_id = ? ORDER BY s.nome COLLATE NOCASE, s.id",
    )
    .bind(home_id)
    .fetch_all(pool)
    .await
}

async fn read_ui_room(pool: &SqlitePool, id: i64) -> Result<Option<UiRoom>, sqlx::Error> {
    sqlx::query_as::<_, UiRoom>(
        "SELECT s.id, s.abitazione_id AS home_id, s.nome AS name, a.nome AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

async fn list_root_containers(
    pool: &SqlitePool,
    home_id: i64,
    room_id: Option<i64>,
) -> Result<Vec<ContainerRecord>, sqlx::Error> {
    sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori \
         WHERE abitazione_id = ? AND stanza_id IS ? AND contenitore_padre_id IS NULL \
         ORDER BY nome COLLATE NOCASE, id",
    )
    .bind(home_id)
    .bind(room_id)
    .fetch_all(pool)
    .await
}

async fn list_move_parent_candidates(
    pool: &SqlitePool,
    moving_id: i64,
    home_id: i64,
    room_id: Option<i64>,
) -> Result<Vec<ContainerRecord>, sqlx::Error> {
    sqlx::query_as::<_, ContainerRecord>(
        "WITH RECURSIVE subtree(id) AS ( \
             SELECT id FROM contenitori WHERE id = ? \
             UNION ALL \
             SELECT c.id FROM contenitori c JOIN subtree s ON c.contenitore_padre_id = s.id \
         ) \
         SELECT c.id, c.abitazione_id AS home_id, c.stanza_id AS room_id, \
                c.contenitore_padre_id AS parent_id, c.nome AS name, c.descrizione AS description \
         FROM contenitori c \
         WHERE c.abitazione_id = ? AND c.stanza_id IS ? \
           AND c.id NOT IN (SELECT id FROM subtree) \
         ORDER BY c.nome COLLATE NOCASE, c.id",
    )
    .bind(moving_id)
    .bind(home_id)
    .bind(room_id)
    .fetch_all(pool)
    .await
}

async fn count_items_in_container(pool: &SqlitePool, id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM item_luogo WHERE contenitore_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

async fn scope_exists(pool: &SqlitePool, home_id: i64, room_id: Option<i64>) -> bool {
    match room_id {
        Some(room_id) => sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM stanze WHERE id = ? AND abitazione_id = ?)",
        )
        .bind(room_id)
        .bind(home_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false),
        None => {
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM abitazioni WHERE id = ?)")
                .bind(home_id)
                .fetch_one(pool)
                .await
                .unwrap_or(false)
        }
    }
}

async fn scope_label(pool: &SqlitePool, home_id: i64, room_id: Option<i64>) -> String {
    if let Some(room_id) = room_id {
        if let Ok(Some(room)) = read_ui_room(pool, room_id).await {
            return format!("🏠 {} / 🚪 {}", room.home_name, room.name);
        }
    }
    if let Ok(Some(home)) = read_ui_home(pool, home_id).await {
        return format!("🏠 {}", home.name);
    }
    format!("casa #{home_id}")
}

fn format_container_path(path: &ContainerPath) -> String {
    let mut parts = vec![path.home_name.clone()];
    if let Some(room_name) = &path.room_name {
        parts.push(room_name.clone());
    }
    parts.extend(path.containers.iter().map(|entry| entry.name.clone()));
    parts.join(" / ")
}

pub(crate) fn format_path_for_ui(path: &ContainerPath) -> String {
    format_container_path(path)
}

pub(crate) fn encode_callback_id(id: i64) -> String {
    encode_id(id)
}

async fn direct_scope_move_label(pool: &SqlitePool, home_id: i64, room_id: Option<i64>) -> String {
    if let Some(room_id) = room_id {
        if let Ok(Some(room)) = read_ui_room(pool, room_id).await {
            return format!("📍 Sposta in {}", truncate_chars(&room.name, 36));
        }
    }
    if let Ok(Some(home)) = read_ui_home(pool, home_id).await {
        return format!("📍 Sposta in {}", truncate_chars(&home.name, 36));
    }
    "📍 Sposta qui".to_string()
}

fn containers_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("➕ Nuovo contenitore", "c:n")],
        vec![button("📋 Tutti i contenitori", "c:a")],
        vec![button("🔎 Sfoglia per casa", "c:l")],
        vec![button("↩️ Case, stanze e contenitori", "loc:menu")],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn button(label: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.to_string(), data.to_string())
}

fn clean_ui_name(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 120 {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_command(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let command = parts.next()?;
    if !command.starts_with('/') {
        return None;
    }
    let command = command.split('@').next()?;
    let args = parts.next().unwrap_or("").trim();
    Some((command, args))
}

fn parse_positive_decimal_id(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok().filter(|id| *id > 0)
}

fn encode_id(value: i64) -> String {
    debug_assert!(value >= 0);
    if value == 0 {
        return "0".to_string();
    }

    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut value = value as u64;
    let mut encoded = Vec::new();
    while value > 0 {
        encoded.push(DIGITS[(value % 36) as usize] as char);
        value /= 36;
    }
    encoded.iter().rev().collect()
}

fn decode_id(value: &str) -> Option<i64> {
    let decoded = i64::from_str_radix(value, 36).ok()?;
    (decoded >= 0).then_some(decoded)
}

fn parse_one_callback(data: &str, prefix: &str) -> Option<i64> {
    let value = data.strip_prefix(prefix)?;
    if value.contains(':') {
        return None;
    }
    decode_id(value).filter(|id| *id > 0)
}

fn parse_two_callback(data: &str, prefix: &str) -> Option<(i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let a = decode_id(parts.next()?)?;
    let b = decode_id(parts.next()?)?;
    if parts.next().is_some() || a <= 0 || b < 0 {
        return None;
    }
    Some((a, b))
}

fn parse_three_callback(data: &str, prefix: &str) -> Option<(i64, i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let a = decode_id(parts.next()?)?;
    let b = decode_id(parts.next()?)?;
    let c = decode_id(parts.next()?)?;
    if parts.next().is_some() || a <= 0 || b <= 0 || c < 0 {
        return None;
    }
    Some((a, b, c))
}

fn parse_four_callback(data: &str, prefix: &str) -> Option<(i64, i64, i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let a = decode_id(parts.next()?)?;
    let b = decode_id(parts.next()?)?;
    let c = decode_id(parts.next()?)?;
    let d = decode_id(parts.next()?)?;
    if parts.next().is_some() || a <= 0 || b <= 0 || c < 0 || d < 0 {
        return None;
    }
    Some((a, b, c, d))
}

fn zero_as_none(value: i64) -> Option<i64> {
    (value > 0).then_some(value)
}

fn move_target_callback(
    id: i64,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
) -> String {
    format!(
        "c:mt:{}:{}:{}:{}",
        encode_id(id),
        encode_id(home_id),
        encode_id(room_id.unwrap_or(0)),
        encode_id(parent_id.unwrap_or(0))
    )
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut truncated = value
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>();
        truncated.push('…');
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate");
        pool
    }

    async fn home(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO abitazioni (nome) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("home")
            .last_insert_rowid()
    }
    async fn room(pool: &SqlitePool, home: i64, name: &str) -> i64 {
        sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, ?)")
            .bind(home)
            .bind(name)
            .execute(pool)
            .await
            .expect("room")
            .last_insert_rowid()
    }
    async fn item(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO items (tipo, nome) VALUES ('oggetto', ?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("item")
            .last_insert_rowid()
    }

    #[tokio::test]
    async fn annidamento_e_percorso() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let a = create_container(&pool, h, Some(r), None, "Armadio", None)
            .await
            .unwrap();
        let b = create_container(&pool, h, Some(r), Some(a), "Ripiano 2", None)
            .await
            .unwrap();
        let c = create_container(&pool, h, Some(r), Some(b), "Scatola", None)
            .await
            .unwrap();
        let p = container_path(&pool, c).await.unwrap().unwrap();
        assert_eq!(p.room_name.as_deref(), Some("Garage"));
        assert_eq!(
            p.containers
                .iter()
                .map(|x| x.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Armadio", "Ripiano 2", "Scatola"]
        );
    }

    #[tokio::test]
    async fn padre_deve_essere_nello_stesso_luogo() {
        let pool = test_pool().await;
        let h1 = home(&pool, "A").await;
        let h2 = home(&pool, "B").await;
        let r1 = room(&pool, h1, "R1").await;
        let r2 = room(&pool, h2, "R2").await;
        let p = create_container(&pool, h1, Some(r1), None, "P", None)
            .await
            .unwrap();
        assert!(create_container(&pool, h2, Some(r2), Some(p), "X", None)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn ciclo_rifiutato() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let a = create_container(&pool, h, Some(r), None, "A", None)
            .await
            .unwrap();
        let b = create_container(&pool, h, Some(r), Some(a), "B", None)
            .await
            .unwrap();
        let c = create_container(&pool, h, Some(r), Some(b), "C", None)
            .await
            .unwrap();
        assert!(move_container(&pool, a, h, Some(r), Some(c)).await.is_err());
    }

    #[tokio::test]
    async fn spostamento_muove_sottoalbero_e_item() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let g = room(&pool, h, "Garage").await;
        let c = room(&pool, h, "Camera").await;
        let a = create_container(&pool, h, Some(g), None, "Armadio", None)
            .await
            .unwrap();
        let s = create_container(&pool, h, Some(g), Some(a), "Scatola", None)
            .await
            .unwrap();
        let i = item(&pool, "Trapano").await;
        assign_item_to_container(&pool, i, s).await.unwrap();
        assert!(move_container(&pool, a, h, Some(c), None).await.unwrap());
        assert_eq!(
            get_container(&pool, s).await.unwrap().unwrap().room_id,
            Some(c)
        );
        let loc: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(loc, (h, Some(c), Some(s)));
    }

    #[tokio::test]
    async fn delete_promuove_al_padre() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let a = create_container(&pool, h, Some(r), None, "Armadio", None)
            .await
            .unwrap();
        let s = create_container(&pool, h, Some(r), Some(a), "Scatola", None)
            .await
            .unwrap();
        let child = create_container(&pool, h, Some(r), Some(s), "Punte", None)
            .await
            .unwrap();
        let i = item(&pool, "Trapano").await;
        assign_item_to_container(&pool, i, s).await.unwrap();
        assert!(delete_container(&pool, s).await.unwrap());
        assert_eq!(
            get_container(&pool, child)
                .await
                .unwrap()
                .unwrap()
                .parent_id,
            Some(a)
        );
        let target: Option<i64> =
            sqlx::query_scalar("SELECT contenitore_id FROM item_luogo WHERE item_id = ?")
                .bind(i)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(target, Some(a));
    }

    #[tokio::test]
    async fn delete_root_mantiene_casa_e_stanza() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let root = create_container(&pool, h, Some(r), None, "Scaffale", None)
            .await
            .unwrap();
        let child = create_container(&pool, h, Some(r), Some(root), "Cassetto", None)
            .await
            .unwrap();
        let i = item(&pool, "Chiavi").await;
        assign_item_to_container(&pool, i, root).await.unwrap();
        assert!(delete_container(&pool, root).await.unwrap());
        let child = get_container(&pool, child).await.unwrap().unwrap();
        assert_eq!(
            (child.parent_id, child.home_id, child.room_id),
            (None, h, Some(r))
        );
        let loc: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(loc, (h, Some(r), None));
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate");
        pool
    }

    async fn home(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO abitazioni (nome) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("home")
            .last_insert_rowid()
    }

    async fn room(pool: &SqlitePool, home_id: i64, name: &str) -> i64 {
        sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, ?)")
            .bind(home_id)
            .bind(name)
            .execute(pool)
            .await
            .expect("room")
            .last_insert_rowid()
    }

    #[test]
    fn callback_spostamento_restano_sotto_limite_telegram() {
        let callback = move_target_callback(i64::MAX, i64::MAX, Some(i64::MAX), Some(i64::MAX));
        assert!(callback.len() <= 64, "callback troppo lunga: {callback}");
        assert_eq!(
            parse_four_callback(&callback, "c:mt:"),
            Some((i64::MAX, i64::MAX, i64::MAX, i64::MAX))
        );
    }

    #[test]
    fn callback_base36_roundtrip() {
        for value in [0, 1, 35, 36, 12345, i64::MAX] {
            assert_eq!(decode_id(&encode_id(value)), Some(value));
        }
    }

    #[tokio::test]
    async fn candidati_spostamento_escludono_se_stesso_e_discendenti() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let a = create_container(&pool, h, Some(r), None, "A", None)
            .await
            .unwrap();
        let b = create_container(&pool, h, Some(r), Some(a), "B", None)
            .await
            .unwrap();
        let c = create_container(&pool, h, Some(r), Some(b), "C", None)
            .await
            .unwrap();
        let x = create_container(&pool, h, Some(r), None, "X", None)
            .await
            .unwrap();

        let candidates = list_move_parent_candidates(&pool, a, h, Some(r))
            .await
            .unwrap();
        let ids = candidates.iter().map(|item| item.id).collect::<Vec<_>>();
        assert!(!ids.contains(&a));
        assert!(!ids.contains(&b));
        assert!(!ids.contains(&c));
        assert!(ids.contains(&x));
    }

    #[tokio::test]
    async fn rinomina_identica_e_un_noop() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let id = create_container(&pool, h, None, None, "Armadio", None)
            .await
            .unwrap();
        assert!(!rename_container(&pool, id, "Armadio").await.unwrap());
        assert_eq!(
            get_container(&pool, id).await.unwrap().unwrap().name,
            "Armadio"
        );
    }
}
