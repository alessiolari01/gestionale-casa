//! Gestione delle foto collegate agli oggetti generici.
//!
//! Step 5B: il file ricevuto da Telegram viene scaricato realmente sul filesystem
//! locale e registrato nella tabella core `foto`. La prima foto di un oggetto
//! viene marcata `principale`, le successive `galleria`. Lo Step 5C riusa questo
//! modulo per eliminare anche la directory media quando un oggetto viene rimosso.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use sqlx::{FromRow, SqlitePool};
use teloxide::{
    net::Download,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile},
};
use tokio::fs::File;

const MEDIA_ROOT: &str = "data/media/oggetti";

pub async fn remove_object_media(item_id: i64) -> std::io::Result<()> {
    let directory = PathBuf::from(MEDIA_ROOT).join(item_id.to_string());
    match tokio::fs::remove_dir_all(directory).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[derive(Clone, Default)]
pub struct PhotoSessionStore {
    inner: Arc<Mutex<HashMap<i64, i64>>>,
}

impl PhotoSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.with_sessions(|sessions| {
            sessions.remove(&chat_id);
        });
    }

    fn get(&self, chat_id: i64) -> Option<i64> {
        self.with_sessions(|sessions| sessions.get(&chat_id).copied())
    }

    fn set(&self, chat_id: i64, item_id: i64) {
        self.with_sessions(|sessions| {
            sessions.insert(chat_id, item_id);
        });
    }

    fn take(&self, chat_id: i64) -> Option<i64> {
        self.with_sessions(|sessions| sessions.remove(&chat_id))
    }

    fn with_sessions<T>(&self, f: impl FnOnce(&mut HashMap<i64, i64>) -> T) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

#[derive(Debug, Clone, FromRow)]
struct PhotoRecord {
    id: i64,
    percorso_file: String,
    ruolo: Option<String>,
    descrizione: Option<String>,
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &PhotoSessionStore,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if let Some(text) = msg.text() {
        if let Some((command, args)) = parse_command(text) {
            match command {
                "/foto" => {
                    sessions.clear_chat(chat_id);
                    if let Some(item_id) = parse_id(args) {
                        show_photo_menu(bot, msg.chat.id, pool, item_id).await?;
                    } else {
                        bot.send_message(msg.chat.id, "Uso: /foto <id>\nEsempio: /foto 12")
                            .await?;
                    }
                    return Ok(true);
                }
                "/foto_aggiungi" => {
                    if let Some(item_id) = parse_id(args) {
                        begin_add_photo(bot, msg.chat.id, pool, sessions, item_id).await?;
                    } else {
                        bot.send_message(
                            msg.chat.id,
                            "Uso: /foto_aggiungi <id>\nEsempio: /foto_aggiungi 12",
                        )
                        .await?;
                    }
                    return Ok(true);
                }
                "/annulla" if sessions.get(chat_id).is_some() => {
                    if let Some(item_id) = sessions.take(chat_id) {
                        bot.send_message(msg.chat.id, "Aggiunta foto annullata.")
                            .await?;
                        show_photo_menu(bot, msg.chat.id, pool, item_id).await?;
                    }
                    return Ok(true);
                }
                _ => return Ok(false),
            }
        }
    }

    let Some(item_id) = sessions.get(chat_id) else {
        return Ok(false);
    };

    let Some(photo_sizes) = msg.photo() else {
        bot.send_message(
            msg.chat.id,
            "📷 Sto aspettando una foto. Invia un'immagine oppure usa /annulla.",
        )
        .await?;
        return Ok(true);
    };

    let Some(photo) = photo_sizes
        .iter()
        .max_by_key(|photo| u64::from(photo.width) * u64::from(photo.height))
    else {
        bot.send_message(msg.chat.id, "⚠️ Non riesco a leggere questa foto.")
            .await?;
        return Ok(true);
    };

    let Some(object_name) = object_name(pool, item_id).await.unwrap_or_else(|error| {
        tracing::error!(?error, item_id, "Errore verifica oggetto per foto");
        None
    }) else {
        sessions.clear_chat(chat_id);
        bot.send_message(msg.chat.id, "⚠️ L'oggetto non esiste più.")
            .await?;
        return Ok(true);
    };

    let telegram_file = bot.get_file(photo.file.id.clone()).await?;
    let extension = safe_extension(&telegram_file.path);
    let directory = PathBuf::from(MEDIA_ROOT).join(item_id.to_string());

    if let Err(error) = tokio::fs::create_dir_all(&directory).await {
        tracing::error!(?error, ?directory, "Impossibile creare cartella foto");
        bot.send_message(
            msg.chat.id,
            "⚠️ Non riesco a preparare la cartella locale per la foto. Puoi riprovare.",
        )
        .await?;
        return Ok(true);
    }

    let filename = format!("telegram_{}.{}", msg.id.0, extension);
    let local_path = directory.join(filename);
    let mut destination = match File::create(&local_path).await {
        Ok(file) => file,
        Err(error) => {
            tracing::error!(?error, ?local_path, "Impossibile creare file foto");
            bot.send_message(
                msg.chat.id,
                "⚠️ Non riesco a creare il file locale della foto. Puoi riprovare.",
            )
            .await?;
            return Ok(true);
        }
    };

    if let Err(error) = bot
        .download_file(&telegram_file.path, &mut destination)
        .await
    {
        tracing::error!(?error, ?local_path, "Download foto Telegram fallito");
        drop(destination);
        let _ = tokio::fs::remove_file(&local_path).await;
        bot.send_message(
            msg.chat.id,
            "⚠️ Telegram non mi ha permesso di scaricare la foto. Puoi riprovare.",
        )
        .await?;
        return Ok(true);
    }
    drop(destination);

    let description = msg
        .caption()
        .map(str::trim)
        .filter(|caption| !caption.is_empty());
    let path_for_db = local_path.to_string_lossy().into_owned();

    let role = match register_photo(pool, item_id, &path_for_db, description).await {
        Ok(role) => role,
        Err(error) => {
            tracing::error!(?error, item_id, ?local_path, "Registrazione foto fallita");
            let _ = tokio::fs::remove_file(&local_path).await;
            bot.send_message(
                msg.chat.id,
                "⚠️ La foto è stata scaricata ma non sono riuscito a registrarla nel database. Nessun file orfano è stato mantenuto.",
            )
            .await?;
            return Ok(true);
        }
    };

    sessions.clear_chat(chat_id);
    let count = count_photos(pool, item_id).await.unwrap_or(1);
    let role_label = if role == "principale" {
        "⭐ Foto principale"
    } else {
        "🖼 Foto di galleria"
    };
    bot.send_message(
        msg.chat.id,
        format!(
            "✅ Foto salvata per \"{object_name}\".\n{role_label}\n\nLe foto sono conservate anche localmente sull'S9 e rientrano nel backup di data/media."
        ),
    )
    .reply_markup(photo_menu_keyboard(item_id, count))
    .await?;

    Ok(true)
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &PhotoSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    if let Some(item_id) = callback_id(data, "foto:menu:") {
        sessions.clear_chat(chat_id.0);
        show_photo_menu(bot, chat_id, pool, item_id).await?;
        return Ok(true);
    }

    if let Some(item_id) = callback_id(data, "foto:add:") {
        begin_add_photo(bot, chat_id, pool, sessions, item_id).await?;
        return Ok(true);
    }

    if let Some(item_id) = callback_id(data, "foto:view:") {
        sessions.clear_chat(chat_id.0);
        send_photos(bot, chat_id, pool, item_id).await?;
        return Ok(true);
    }

    if let Some(item_id) = callback_id(data, "foto:cancel:") {
        sessions.clear_chat(chat_id.0);
        show_photo_menu(bot, chat_id, pool, item_id).await?;
        return Ok(true);
    }

    Ok(false)
}

async fn show_photo_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    item_id: i64,
) -> ResponseResult<()> {
    match object_name(pool, item_id).await {
        Ok(Some(name)) => {
            let count = count_photos(pool, item_id).await.unwrap_or(0);
            bot.send_message(
                chat_id,
                format!(
                    "📷 Foto — {name}\n\nFoto salvate: {count}\n\nLa prima foto viene usata come principale; le successive entrano nella galleria."
                ),
            )
            .reply_markup(photo_menu_keyboard(item_id, count))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "Oggetto non trovato.").await?;
        }
        Err(error) => {
            tracing::error!(?error, item_id, "Errore lettura menu foto");
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a leggere le foto di questo oggetto.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn begin_add_photo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &PhotoSessionStore,
    item_id: i64,
) -> ResponseResult<()> {
    match object_name(pool, item_id).await {
        Ok(Some(name)) => {
            sessions.set(chat_id.0, item_id);
            bot.send_message(
                chat_id,
                format!(
                    "📷 Inviami ora una foto per \"{name}\".\n\nPuoi aggiungere una didascalia: verrà salvata come descrizione della foto.\n\n/annulla per uscire."
                ),
            )
            .reply_markup(cancel_photo_keyboard(item_id))
            .await?;
        }
        Ok(None) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, "Oggetto non trovato.").await?;
        }
        Err(error) => {
            tracing::error!(?error, item_id, "Errore avvio aggiunta foto");
            bot.send_message(chat_id, "⚠️ Non riesco ad avviare l'aggiunta della foto.")
                .await?;
        }
    }
    Ok(())
}

async fn send_photos(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    item_id: i64,
) -> ResponseResult<()> {
    let object = match object_name(pool, item_id).await {
        Ok(Some(name)) => name,
        Ok(None) => {
            bot.send_message(chat_id, "Oggetto non trovato.").await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, item_id, "Errore lettura oggetto per galleria");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo oggetto.")
                .await?;
            return Ok(());
        }
    };

    let photos = match list_photos(pool, item_id).await {
        Ok(photos) => photos,
        Err(error) => {
            tracing::error!(?error, item_id, "Errore lettura galleria");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere la galleria.")
                .await?;
            return Ok(());
        }
    };

    if photos.is_empty() {
        bot.send_message(chat_id, format!("📷 {object}\n\nNon ci sono ancora foto."))
            .reply_markup(photo_menu_keyboard(item_id, 0))
            .await?;
        return Ok(());
    }

    let total = photos.len();
    let mut missing = 0usize;
    for (index, photo) in photos.iter().enumerate() {
        let path = PathBuf::from(&photo.percorso_file);
        if !path.is_file() {
            missing += 1;
            tracing::warn!(
                photo_id = photo.id,
                ?path,
                "Foto registrata nel database ma assente dal filesystem"
            );
            continue;
        }

        let role = if photo.ruolo.as_deref() == Some("principale") {
            "⭐ Principale"
        } else {
            "🖼 Galleria"
        };
        let mut caption = format!("{role} · {}/{}", index + 1, total);
        if let Some(description) = photo.descrizione.as_deref() {
            caption.push('\n');
            caption.push_str(description);
        }

        bot.send_photo(chat_id, InputFile::file(path))
            .caption(caption)
            .await?;
    }

    let mut summary = format!("📷 Galleria di {object}\n\nFoto registrate: {total}");
    if missing > 0 {
        summary.push_str(&format!(
            "\n⚠️ File locali mancanti: {missing}. Controlla backup/filesystem."
        ));
    }
    bot.send_message(chat_id, summary)
        .reply_markup(photo_menu_keyboard(item_id, total as i64))
        .await?;
    Ok(())
}

async fn object_name(pool: &SqlitePool, item_id: i64) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT nome FROM items \
         WHERE id = ? AND tipo = 'oggetto' AND spazio_id = ?",
    )
    .bind(item_id)
    .bind(crate::identity::current_space_id())
    .fetch_optional(pool)
    .await
}

async fn count_photos(pool: &SqlitePool, item_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM foto f JOIN items i ON i.id = f.item_id \
         WHERE f.item_id = ? AND i.tipo = 'oggetto' AND i.spazio_id = ?",
    )
    .bind(item_id)
    .bind(crate::identity::current_space_id())
    .fetch_one(pool)
    .await
}

async fn register_photo(
    pool: &SqlitePool,
    item_id: i64,
    path: &str,
    description: Option<&str>,
) -> Result<&'static str, sqlx::Error> {
    crate::identity::ensure_can_write_sqlx(pool).await?;
    let mut tx = pool.begin().await?;

    let space_id = crate::identity::current_space_id();
    let existing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM foto f JOIN items i ON i.id = f.item_id \
         WHERE f.item_id = ? AND i.tipo = 'oggetto' AND i.spazio_id = ?",
    )
    .bind(item_id)
    .bind(space_id)
    .fetch_one(&mut *tx)
    .await?;
    let role = if existing == 0 {
        "principale"
    } else {
        "galleria"
    };

    let object_name: String = sqlx::query_scalar(
        "SELECT nome FROM items \
         WHERE id = ? AND tipo = 'oggetto' AND spazio_id = ?",
    )
    .bind(item_id)
    .bind(space_id)
    .fetch_one(&mut *tx)
    .await?;
    let storico_id =
        crate::modules::storico::ensure_entity(&mut tx, "oggetto", item_id, &object_name).await?;

    let result = sqlx::query(
        "INSERT INTO foto (item_id, percorso_file, ruolo, descrizione) VALUES (?, ?, ?, ?)",
    )
    .bind(item_id)
    .bind(path)
    .bind(role)
    .bind(description)
    .execute(&mut *tx)
    .await?;
    let photo_id = result.last_insert_rowid();

    let event_location =
        crate::modules::luoghi::history_item_location_snapshot(&mut tx, item_id).await?;
    let event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: storico_id,
            modulo: "oggetti",
            componente: "foto",
            operazione: "foto_aggiunta",
            nome_entita_snapshot: &object_name,
            abitazione_storico_id: event_location.abitazione_storico_id,
            abitazione_nome_snapshot: event_location.abitazione_nome.as_deref(),
            stanza_storico_id: event_location.stanza_storico_id,
            stanza_nome_snapshot: event_location.stanza_nome.as_deref(),
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_event_location_context(&mut tx, event_id, &event_location)
        .await?;

    let mut changes = vec![
        crate::modules::storico::NewFieldChange {
            campo: "foto_id",
            tipo_valore: "numero",
            valore_prima: None,
            valore_dopo: Some(photo_id.to_string()),
        },
        crate::modules::storico::NewFieldChange {
            campo: "ruolo",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(role.to_string()),
        },
        crate::modules::storico::NewFieldChange {
            campo: "percorso_file",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(path.to_string()),
        },
    ];
    if let Some(description) = description {
        changes.push(crate::modules::storico::NewFieldChange {
            campo: "descrizione",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(description.to_string()),
        });
    }
    crate::modules::storico::record_field_changes(&mut tx, event_id, &changes).await?;

    tx.commit().await?;
    Ok(role)
}

async fn list_photos(pool: &SqlitePool, item_id: i64) -> Result<Vec<PhotoRecord>, sqlx::Error> {
    sqlx::query_as::<_, PhotoRecord>(
        "SELECT f.id, f.percorso_file, f.ruolo, f.descrizione \
         FROM foto f JOIN items i ON i.id = f.item_id \
         WHERE f.item_id = ? AND i.tipo = 'oggetto' AND i.spazio_id = ? \
         ORDER BY CASE WHEN f.ruolo = 'principale' THEN 0 ELSE 1 END, f.id",
    )
    .bind(item_id)
    .bind(crate::identity::current_space_id())
    .fetch_all(pool)
    .await
}

fn photo_menu_keyboard(item_id: i64, count: i64) -> InlineKeyboardMarkup {
    let mut rows = vec![vec![button(
        "➕ Aggiungi foto",
        &format!("foto:add:{item_id}"),
    )]];

    if count > 0 {
        rows.push(vec![button(
            &format!("🖼 Vedi foto ({count})"),
            &format!("foto:view:{item_id}"),
        )]);
    }

    rows.push(vec![button(
        "⬅️ Torna all'oggetto",
        &format!("oggetti:view:{item_id}"),
    )]);
    rows.push(vec![button("🏠 Menu principale", "menu:main")]);
    InlineKeyboardMarkup::new(rows)
}

fn cancel_photo_keyboard(item_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("❌ Annulla", &format!("foto:cancel:{item_id}"))],
        vec![button(
            "⬅️ Torna all'oggetto",
            &format!("oggetti:view:{item_id}"),
        )],
    ])
}

fn button(label: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.to_string(), data.to_string())
}

fn parse_command(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let split_at = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let token = &trimmed[..split_at];
    let args = trimmed[split_at..].trim();
    let command = token.split('@').next().unwrap_or(token);
    Some((command, args))
}

fn parse_id(value: &str) -> Option<i64> {
    let id = value.trim().parse::<i64>().ok()?;
    (id > 0).then_some(id)
}

fn callback_id(data: &str, prefix: &str) -> Option<i64> {
    parse_id(data.strip_prefix(prefix)?)
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
            .expect("foreign key di test");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration di test");
        pool
    }

    #[test]
    fn estensione_file_viene_limitata_a_valori_sicuri() {
        assert_eq!(safe_extension("photos/file_1.jpg"), "jpg");
        assert_eq!(safe_extension("photos/file_1.JPEG"), "jpeg");
        assert_eq!(safe_extension("photos/file_senza_estensione"), "jpg");
        assert_eq!(safe_extension("photos/file.bad-ext"), "jpg");
    }

    #[tokio::test]
    async fn prima_foto_e_principale_e_le_successive_galleria() {
        let pool = test_pool().await;
        let item = sqlx::query("INSERT INTO items (tipo, nome) VALUES ('oggetto', 'Test foto')")
            .execute(&pool)
            .await
            .expect("item");
        let item_id = item.last_insert_rowid();
        sqlx::query("INSERT INTO oggetti (item_id) VALUES (?)")
            .bind(item_id)
            .execute(&pool)
            .await
            .expect("dettaglio oggetto");

        let first = register_photo(&pool, item_id, "data/media/1.jpg", Some("prima"))
            .await
            .expect("prima foto");
        let second = register_photo(&pool, item_id, "data/media/2.jpg", None)
            .await
            .expect("seconda foto");

        assert_eq!(first, "principale");
        assert_eq!(second, "galleria");

        let photos = list_photos(&pool, item_id).await.expect("galleria");
        assert_eq!(photos.len(), 2);
        assert_eq!(photos[0].ruolo.as_deref(), Some("principale"));
        assert_eq!(photos[0].descrizione.as_deref(), Some("prima"));
        assert_eq!(photos[1].ruolo.as_deref(), Some("galleria"));
    }

    #[tokio::test]
    async fn storico_foto_conserva_il_luogo_dell_oggetto() {
        let pool = test_pool().await;

        let item =
            sqlx::query("INSERT INTO items (tipo, nome) VALUES ('oggetto', 'Foto con luogo')")
                .execute(&pool)
                .await
                .expect("item");
        let item_id = item.last_insert_rowid();

        sqlx::query("INSERT INTO oggetti (item_id) VALUES (?)")
            .bind(item_id)
            .execute(&pool)
            .await
            .expect("oggetto");

        let home = sqlx::query("INSERT INTO abitazioni (nome, spazio_id) VALUES ('Casa foto', 1)")
            .execute(&pool)
            .await
            .expect("casa");
        let home_id = home.last_insert_rowid();

        let room = sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, 'Studio')")
            .bind(home_id)
            .execute(&pool)
            .await
            .expect("stanza");
        let room_id = room.last_insert_rowid();

        let container_id = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            None,
            "Archivio foto",
            None,
        )
        .await
        .expect("contenitore");

        sqlx::query(
            "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(item_id)
        .bind(home_id)
        .bind(room_id)
        .bind(container_id)
        .execute(&pool)
        .await
        .expect("luogo");

        register_photo(&pool, item_id, "data/media/foto_contesto.jpg", None)
            .await
            .expect("foto");

        let context: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT abitazione_nome_snapshot, stanza_nome_snapshot, contenitore_percorso_snapshot \
             FROM storico_eventi \
             WHERE operazione = 'foto_aggiunta' AND nome_entita_snapshot = 'Foto con luogo' \
             ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("contesto foto");

        assert_eq!(
            context,
            (
                Some("Casa foto".to_string()),
                Some("Studio".to_string()),
                Some("Archivio foto".to_string())
            )
        );
    }
    #[tokio::test]
    async fn foto_non_sono_leggibili_ne_aggiungibili_cross_space_per_id() {
        let pool = test_pool().await;
        let space_two =
            sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Spazio due', 'personale')")
                .execute(&pool)
                .await
                .expect("spazio due")
                .last_insert_rowid();

        let item_two = sqlx::query(
            "INSERT INTO items (tipo, nome, spazio_id) VALUES ('oggetto', 'Oggetto due', ?)",
        )
        .bind(space_two)
        .execute(&pool)
        .await
        .expect("item spazio due")
        .last_insert_rowid();
        sqlx::query("INSERT INTO oggetti (item_id) VALUES (?)")
            .bind(item_two)
            .execute(&pool)
            .await
            .expect("oggetto spazio due");

        let actor_two = crate::identity::AuditActor {
            utente_id: None,
            nome_snapshot: "Sistema test".to_string(),
            spazio_id: space_two,
            spazio_nome_snapshot: "Spazio due".to_string(),
            origine: "sistema",
            telegram_user_id: None,
            telegram_username: None,
        };
        crate::identity::with_actor(actor_two.clone(), async {
            register_photo(&pool, item_two, "data/media/spazio_due.jpg", None)
                .await
                .expect("foto spazio due");
        })
        .await;

        crate::identity::with_actor(crate::identity::AuditActor::system(), async {
            assert!(object_name(&pool, item_two)
                .await
                .expect("nome cross-space")
                .is_none());
            assert_eq!(count_photos(&pool, item_two).await.expect("conteggio"), 0);
            assert!(list_photos(&pool, item_two)
                .await
                .expect("galleria cross-space")
                .is_empty());
            assert!(
                register_photo(&pool, item_two, "data/media/intrusa.jpg", None)
                    .await
                    .is_err()
            );
        })
        .await;

        crate::identity::with_actor(actor_two, async {
            let photos = list_photos(&pool, item_two)
                .await
                .expect("galleria propria");
            assert_eq!(photos.len(), 1);
            assert_eq!(photos[0].percorso_file, "data/media/spazio_due.jpg");
        })
        .await;
    }
}
