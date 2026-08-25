//! Identità interne, collegamento Telegram e contesto audit dello Step 7.1.
//!
//! Telegram rimane un provider esterno: le operazioni applicative fanno
//! riferimento a `utenti.id`. Il contesto dell'attore viene installato come
//! task-local durante la gestione di ogni update Telegram, così le primitive
//! dello storico possono attribuire le modifiche senza propagare manualmente
//! l'autore attraverso tutti i moduli Step 6.

use std::future::Future;

use anyhow::{bail, Context, Result};
use sqlx::{SqlitePool, Transaction};
use teloxide::types::User;

pub(crate) const LEGACY_SPACE_ID: i64 = 1;
pub(crate) const SYSTEM_ROLE_ADMIN: &str = "admin";
#[cfg(test)]
const LEGACY_SPACE_NAME: &str = "Spazio principale";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditActor {
    pub(crate) utente_id: Option<i64>,
    pub(crate) nome_snapshot: String,
    pub(crate) spazio_id: i64,
    pub(crate) spazio_nome_snapshot: String,
    pub(crate) view_all: bool,
    pub(crate) origine: &'static str,
    pub(crate) telegram_user_id: Option<i64>,
    pub(crate) telegram_username: Option<String>,
}

impl AuditActor {
    #[cfg(test)]
    pub(crate) fn system() -> Self {
        Self {
            utente_id: None,
            nome_snapshot: "Sistema".to_string(),
            spazio_id: LEGACY_SPACE_ID,
            spazio_nome_snapshot: LEGACY_SPACE_NAME.to_string(),
            view_all: false,
            origine: "sistema",
            telegram_user_id: None,
            telegram_username: None,
        }
    }
}

tokio::task_local! {
    static CURRENT_ACTOR: AuditActor;
}

pub(crate) async fn with_actor<F>(actor: AuditActor, future: F) -> F::Output
where
    F: Future,
{
    CURRENT_ACTOR.scope(actor, future).await
}

pub(crate) fn current_actor() -> AuditActor {
    match CURRENT_ACTOR.try_with(Clone::clone) {
        Ok(actor) => actor,
        Err(_) => missing_actor_context(),
    }
}

#[cfg(test)]
fn missing_actor_context() -> AuditActor {
    // I test legacy dei moduli Step 6 eseguono molte primitive direttamente.
    // In test manteniamo quindi il contesto bootstrap esplicito di compatibilita'.
    AuditActor::system()
}

#[cfg(not(test))]
fn missing_actor_context() -> AuditActor {
    // In produzione un'operazione space-aware senza contesto attore e' un errore
    // di programmazione: fallire e' piu' sicuro che ricadere silenziosamente
    // nello spazio bootstrap #1 e rischiare una lettura/scrittura cross-space.
    panic!("contesto attore mancante per operazione space-aware")
}

/// Risolve o crea l'utente interno collegato a un account Telegram.
///
/// Il primo account Telegram autorizzato prende in carico lo spazio bootstrap
/// che contiene i dati pre-Step 7. I nuovi account successivi ricevono invece
/// uno spazio personale indipendente; eventuali spazi condivisi vengono poi
/// gestiti esplicitamente tramite membership.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TelegramIdentityInput {
    telegram_user_id: i64,
    chat_id: i64,
    display_name: String,
    first_name: String,
    last_name: Option<String>,
    username: Option<String>,
}

pub(crate) async fn resolve_telegram_actor(
    pool: &SqlitePool,
    chat_id: i64,
    telegram_user: &User,
) -> Result<AuditActor> {
    let telegram_user_id = i64::try_from(telegram_user.id.0)
        .context("Telegram user ID non rappresentabile come INTEGER SQLite")?;
    let profile = TelegramIdentityInput {
        telegram_user_id,
        chat_id,
        display_name: telegram_user.full_name().trim().to_string(),
        first_name: telegram_user.first_name.clone(),
        last_name: telegram_user.last_name.clone(),
        username: telegram_user.username.clone(),
    };
    resolve_telegram_profile(pool, &profile).await
}

pub(crate) async fn lookup_telegram_actor(
    pool: &SqlitePool,
    chat_id: i64,
    telegram_user: &User,
) -> Result<Option<AuditActor>> {
    let telegram_user_id = i64::try_from(telegram_user.id.0)
        .context("Telegram user ID non rappresentabile come INTEGER SQLite")?;

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione identità Telegram")?;

    let existing: Option<(i64, String)> = sqlx::query_as(
        "SELECT u.id, u.stato \
         FROM account_telegram at \
         JOIN utenti u ON u.id = at.utente_id \
         WHERE at.telegram_user_id = ?",
    )
    .bind(telegram_user_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile cercare l'account Telegram esistente")?;

    let Some((user_id, status)) = existing else {
        tx.rollback().await.ok();
        return Ok(None);
    };

    if status != "attivo" {
        bail!("Utente interno disabilitato");
    }

    let display_name = telegram_user.full_name().trim().to_string();
    if display_name.is_empty() {
        bail!("Telegram non ha fornito un nome utilizzabile per l'autore");
    }

    sqlx::query(
        "UPDATE utenti \
         SET nome_visualizzato = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
    )
    .bind(&display_name)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare il nome dell'utente")?;

    sqlx::query(
        "UPDATE account_telegram \
         SET chat_id = ?, username_snapshot = ?, nome_snapshot = ?, cognome_snapshot = ?, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE telegram_user_id = ?",
    )
    .bind(chat_id)
    .bind(telegram_user.username.as_deref())
    .bind(&telegram_user.first_name)
    .bind(telegram_user.last_name.as_deref())
    .bind(telegram_user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare lo snapshot Telegram")?;

    ensure_initial_space(&mut tx, user_id, &display_name).await?;
    ensure_system_admin_exists(&mut tx).await?;
    ensure_primary_admin_exists(&mut tx).await?;

    let (space_id, space_name, view_mode): (i64, String, String) = sqlx::query_as(
        "SELECT s.id, s.nome, p.vista_spazi \
         FROM preferenze_utente p \
         JOIN membri_spazio ms \
           ON ms.utente_id = p.utente_id AND ms.spazio_id = p.spazio_attivo_id \
         JOIN spazi s ON s.id = p.spazio_attivo_id \
         WHERE p.utente_id = ?",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .context("Impossibile leggere lo spazio attivo")?;

    tx.commit()
        .await
        .context("Impossibile salvare lo snapshot dell'identità Telegram")?;

    Ok(Some(AuditActor {
        utente_id: Some(user_id),
        nome_snapshot: display_name,
        spazio_id: space_id,
        spazio_nome_snapshot: space_name,
        view_all: view_mode == "tutti",
        origine: "telegram",
        telegram_user_id: Some(telegram_user_id),
        telegram_username: telegram_user.username.clone(),
    }))
}

async fn resolve_telegram_profile(
    pool: &SqlitePool,
    profile: &TelegramIdentityInput,
) -> Result<AuditActor> {
    if profile.display_name.is_empty() {
        bail!("Telegram non ha fornito un nome utilizzabile per l'autore");
    }
    let telegram_user_id = profile.telegram_user_id;
    let display_name = &profile.display_name;
    let chat_id = profile.chat_id;

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione identità Telegram")?;

    let existing = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT u.id, u.nome_visualizzato, u.stato \
         FROM account_telegram at \
         JOIN utenti u ON u.id = at.utente_id \
         WHERE at.telegram_user_id = ?",
    )
    .bind(telegram_user_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile cercare l'account Telegram")?;

    let user_id = if let Some((user_id, _, status)) = existing {
        if status != "attivo" {
            bail!("Utente interno disabilitato");
        }

        sqlx::query(
            "UPDATE utenti \
             SET nome_visualizzato = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?",
        )
        .bind(display_name)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile aggiornare il nome dell'utente")?;

        sqlx::query(
            "UPDATE account_telegram \
             SET chat_id = ?, username_snapshot = ?, nome_snapshot = ?, cognome_snapshot = ?, \
                 aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE telegram_user_id = ?",
        )
        .bind(chat_id)
        .bind(profile.username.as_deref())
        .bind(&profile.first_name)
        .bind(profile.last_name.as_deref())
        .bind(telegram_user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile aggiornare lo snapshot Telegram")?;

        user_id
    } else {
        let result = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES (?)")
            .bind(display_name)
            .execute(&mut *tx)
            .await
            .context("Impossibile creare l'utente interno")?;
        let user_id = result.last_insert_rowid();

        sqlx::query(
            "INSERT INTO account_telegram (\
                utente_id, telegram_user_id, chat_id, username_snapshot, nome_snapshot, cognome_snapshot\
             ) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(telegram_user_id)
        .bind(chat_id)
        .bind(profile.username.as_deref())
        .bind(&profile.first_name)
        .bind(profile.last_name.as_deref())
        .execute(&mut *tx)
        .await
        .context("Impossibile collegare l'account Telegram")?;

        user_id
    };

    ensure_initial_space(&mut tx, user_id, display_name).await?;
    ensure_system_admin_exists(&mut tx).await?;
    ensure_primary_admin_exists(&mut tx).await?;

    let (space_id, space_name, view_mode): (i64, String, String) = sqlx::query_as(
        "SELECT s.id, s.nome, p.vista_spazi \
         FROM preferenze_utente p \
         JOIN membri_spazio ms \
           ON ms.utente_id = p.utente_id AND ms.spazio_id = p.spazio_attivo_id \
         JOIN spazi s ON s.id = p.spazio_attivo_id \
         WHERE p.utente_id = ?",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .context("Impossibile leggere lo spazio attivo")?;

    tx.commit()
        .await
        .context("Impossibile salvare l'identità Telegram")?;

    Ok(AuditActor {
        utente_id: Some(user_id),
        nome_snapshot: display_name.clone(),
        spazio_id: space_id,
        spazio_nome_snapshot: space_name,
        view_all: view_mode == "tutti",
        origine: "telegram",
        telegram_user_id: Some(telegram_user_id),
        telegram_username: profile.username.clone(),
    })
}

async fn ensure_system_admin_exists(tx: &mut Transaction<'_, sqlx::Sqlite>) -> Result<()> {
    sqlx::query(
        "UPDATE utenti \
         SET ruolo_sistema = 'admin', \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = COALESCE(\
             (SELECT creato_da_utente_id FROM spazi \
              WHERE bootstrap_legacy = 1 AND creato_da_utente_id IS NOT NULL LIMIT 1),\
             (SELECT id FROM utenti ORDER BY creato_il, id LIMIT 1)\
         ) \
           AND NOT EXISTS (SELECT 1 FROM utenti WHERE ruolo_sistema = 'admin')",
    )
    .execute(&mut **tx)
    .await
    .context("Impossibile inizializzare il ruolo amministratore")?;
    Ok(())
}

async fn ensure_primary_admin_exists(tx: &mut Transaction<'_, sqlx::Sqlite>) -> Result<()> {
    sqlx::query(
        "UPDATE utenti \
         SET amministratore_principale = 1, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = COALESCE(\
             (SELECT creato_da_utente_id FROM spazi \
              WHERE bootstrap_legacy = 1 AND creato_da_utente_id IS NOT NULL LIMIT 1),\
             (SELECT id FROM utenti WHERE ruolo_sistema = 'admin' ORDER BY creato_il, id LIMIT 1)\
         ) \
           AND NOT EXISTS (SELECT 1 FROM utenti WHERE amministratore_principale = 1)",
    )
    .execute(&mut **tx)
    .await
    .context("Impossibile inizializzare l'amministratore principale")?;
    Ok(())
}

pub(crate) async fn provision_approved_telegram_account(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    telegram_user_id: i64,
    chat_id: i64,
    display_name: &str,
    first_name: &str,
    last_name: Option<&str>,
    username: Option<&str>,
) -> Result<i64> {
    if display_name.trim().is_empty() {
        bail!("Nome Telegram non valido");
    }

    let existing: Option<i64> =
        sqlx::query_scalar("SELECT utente_id FROM account_telegram WHERE telegram_user_id = ?")
            .bind(telegram_user_id)
            .fetch_optional(&mut **tx)
            .await
            .context("Impossibile verificare l'account Telegram da approvare")?;

    if let Some(user_id) = existing {
        return Ok(user_id);
    }

    let result = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES (?)")
        .bind(display_name.trim())
        .execute(&mut **tx)
        .await
        .context("Impossibile creare l'utente approvato")?;
    let user_id = result.last_insert_rowid();

    sqlx::query(
        "INSERT INTO account_telegram (\
            utente_id, telegram_user_id, chat_id, username_snapshot, nome_snapshot, cognome_snapshot\
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(telegram_user_id)
    .bind(chat_id)
    .bind(username)
    .bind(first_name)
    .bind(last_name)
    .execute(&mut **tx)
    .await
    .context("Impossibile collegare l'account Telegram approvato")?;

    ensure_initial_space(tx, user_id, display_name.trim()).await?;
    Ok(user_id)
}

async fn ensure_initial_space(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    display_name: &str,
) -> Result<()> {
    // Una preferenza e' valida solo se lo spazio attivo e' ancora una membership
    // reale dell'utente. Questo ricontrollo rende il bootstrap auto-riparante
    // anche su database creati prima dei trigger di coerenza definitivi.
    let valid_active_space: Option<i64> = sqlx::query_scalar(
        "SELECT p.spazio_attivo_id \
         FROM preferenze_utente p \
         JOIN membri_spazio ms \
           ON ms.utente_id = p.utente_id AND ms.spazio_id = p.spazio_attivo_id \
         WHERE p.utente_id = ?",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .context("Impossibile verificare la validita dello spazio attivo")?;

    if valid_active_space.is_some() {
        return Ok(());
    }

    let existing_membership: Option<i64> = sqlx::query_scalar(
        "SELECT spazio_id FROM membri_spazio WHERE utente_id = ? ORDER BY aggiunto_il, spazio_id LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .context("Impossibile verificare le membership dell'utente")?;

    let initial_space_id = if let Some(space_id) = existing_membership {
        space_id
    } else {
        let bootstrap_members: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM membri_spazio WHERE spazio_id = ?")
                .bind(LEGACY_SPACE_ID)
                .fetch_one(&mut **tx)
                .await
                .context("Impossibile contare i membri dello spazio bootstrap")?;

        if bootstrap_members == 0 {
            sqlx::query(
                "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
            )
            .bind(LEGACY_SPACE_ID)
            .bind(user_id)
            .execute(&mut **tx)
            .await
            .context("Impossibile assegnare il proprietario allo spazio bootstrap")?;

            sqlx::query(
                "UPDATE spazi \
                 SET creato_da_utente_id = COALESCE(creato_da_utente_id, ?), \
                     aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE id = ?",
            )
            .bind(user_id)
            .bind(LEGACY_SPACE_ID)
            .execute(&mut **tx)
            .await
            .context("Impossibile attribuire lo spazio bootstrap")?;

            LEGACY_SPACE_ID
        } else {
            let personal_name = format!("Spazio personale · {display_name}");
            let result = sqlx::query(
                "INSERT INTO spazi (nome, tipo, creato_da_utente_id) VALUES (?, 'personale', ?)",
            )
            .bind(&personal_name)
            .bind(user_id)
            .execute(&mut **tx)
            .await
            .context("Impossibile creare lo spazio personale")?;
            let space_id = result.last_insert_rowid();

            sqlx::query(
                "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
            )
            .bind(space_id)
            .bind(user_id)
            .execute(&mut **tx)
            .await
            .context("Impossibile assegnare lo spazio personale")?;
            space_id
        }
    };

    sqlx::query(
        "INSERT INTO preferenze_utente (utente_id, spazio_attivo_id) VALUES (?, ?) \
         ON CONFLICT(utente_id) DO UPDATE SET \
             spazio_attivo_id = excluded.spazio_attivo_id, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(user_id)
    .bind(initial_space_id)
    .execute(&mut **tx)
    .await
    .context("Impossibile inizializzare o riparare lo spazio attivo")?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct SystemUserSummary {
    pub(crate) nome: String,
    pub(crate) ruolo_sistema: String,
    pub(crate) stato: String,
    pub(crate) telegram_username: Option<String>,
    pub(crate) numero_spazi: i64,
}

pub(crate) async fn is_system_admin(pool: &SqlitePool, actor: &AuditActor) -> Result<bool> {
    let Some(user_id) = actor.utente_id else {
        return Ok(false);
    };
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(\
            SELECT 1 FROM utenti \
            WHERE id = ? AND stato = 'attivo' AND ruolo_sistema = 'admin'\
         )",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il ruolo di sistema")
}

pub(crate) async fn is_primary_admin(pool: &SqlitePool, actor: &AuditActor) -> Result<bool> {
    let Some(user_id) = actor.utente_id else {
        return Ok(false);
    };
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(\
            SELECT 1 FROM utenti \
            WHERE id = ? AND stato = 'attivo' \
              AND ruolo_sistema = 'admin' AND amministratore_principale = 1\
         )",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare l'amministratore principale")
}

pub(crate) async fn list_primary_admin_chat_ids(pool: &SqlitePool) -> Result<Vec<i64>> {
    sqlx::query_scalar(
        "SELECT DISTINCT at.chat_id \
         FROM account_telegram at \
         JOIN utenti u ON u.id = at.utente_id \
         WHERE u.stato = 'attivo' AND u.ruolo_sistema = 'admin' \
           AND u.amministratore_principale = 1 \
         ORDER BY at.chat_id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere la chat dell'amministratore principale")
}

pub(crate) async fn list_system_admin_chat_ids(pool: &SqlitePool) -> Result<Vec<i64>> {
    sqlx::query_scalar(
        "SELECT DISTINCT at.chat_id \
         FROM account_telegram at \
         JOIN utenti u ON u.id = at.utente_id \
         WHERE u.stato = 'attivo' AND u.ruolo_sistema = 'admin' \
         ORDER BY at.chat_id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le chat degli amministratori")
}

pub(crate) async fn list_system_users(pool: &SqlitePool) -> Result<Vec<SystemUserSummary>> {
    sqlx::query_as::<_, SystemUserSummary>(
        "SELECT u.nome_visualizzato AS nome, \
                u.ruolo_sistema, u.stato, at.username_snapshot AS telegram_username, \
                COUNT(DISTINCT ms.spazio_id) AS numero_spazi \
         FROM utenti u \
         LEFT JOIN account_telegram at ON at.utente_id = u.id \
         LEFT JOIN membri_spazio ms ON ms.utente_id = u.id \
         GROUP BY u.id, u.nome_visualizzato, u.ruolo_sistema, u.stato, at.username_snapshot \
         ORDER BY CASE u.ruolo_sistema WHEN 'admin' THEN 0 ELSE 1 END, \
                  u.nome_visualizzato COLLATE NOCASE, u.id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli utenti del gestionale")
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct SpaceMembership {
    pub(crate) id: i64,
    pub(crate) nome: String,
    pub(crate) tipo: String,
    pub(crate) ruolo: String,
    pub(crate) attivo: i64,
}

pub(crate) fn current_space_id() -> i64 {
    current_actor().spazio_id
}

pub(crate) fn current_view_all() -> bool {
    current_actor().view_all
}

pub(crate) async fn set_view_all(
    pool: &SqlitePool,
    actor: &AuditActor,
    view_all: bool,
) -> Result<()> {
    let user_id = actor
        .utente_id
        .context("Vista spazi non disponibile per un attore di sistema")?;
    let value = if view_all { "tutti" } else { "predefinito" };
    let affected = sqlx::query(
        "UPDATE preferenze_utente \
         SET vista_spazi = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE utente_id = ?",
    )
    .bind(value)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare la vista multi-spazio")?
    .rows_affected();
    if affected != 1 {
        bail!("Preferenze utente non trovate");
    }
    Ok(())
}

pub(crate) async fn can_write_space(pool: &SqlitePool, space_id: i64) -> Result<bool> {
    let actor = current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(true);
    };
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM membri_spazio \
         WHERE utente_id = ? AND spazio_id = ? \
           AND ruolo IN ('proprietario', 'amministratore', 'membro'))",
    )
    .bind(user_id)
    .bind(space_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare i permessi dello spazio")
}

pub(crate) async fn ensure_can_write_space(pool: &SqlitePool, space_id: i64) -> Result<()> {
    if can_write_space(pool, space_id).await? {
        Ok(())
    } else {
        bail!("Lo spazio di destinazione o proprietario non è modificabile da questo utente")
    }
}

pub(crate) async fn ensure_can_write_space_sqlx(
    pool: &SqlitePool,
    space_id: i64,
) -> Result<(), sqlx::Error> {
    let actor = current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(());
    };
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM membri_spazio \
         WHERE utente_id = ? AND spazio_id = ? \
           AND ruolo IN ('proprietario', 'amministratore', 'membro'))",
    )
    .bind(user_id)
    .bind(space_id)
    .fetch_one(pool)
    .await?;
    if allowed {
        Ok(())
    } else {
        Err(sqlx::Error::RowNotFound)
    }
}

pub(crate) fn visible_space_sql(alias: &str) -> String {
    let actor = current_actor();
    if actor.view_all {
        if actor.utente_id.is_some() {
            format!(
                "{alias}.spazio_id IN (SELECT spazio_id FROM membri_spazio WHERE utente_id = ?)"
            )
        } else {
            format!("{alias}.spazio_id = ?")
        }
    } else {
        format!("{alias}.spazio_id = ?")
    }
}

pub(crate) fn visible_space_bind_id() -> i64 {
    let actor = current_actor();
    if actor.view_all {
        actor.utente_id.unwrap_or(actor.spazio_id)
    } else {
        actor.spazio_id
    }
}

pub(crate) async fn list_user_spaces(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<SpaceMembership>> {
    sqlx::query_as::<_, SpaceMembership>(
        "SELECT s.id, s.nome, s.tipo, ms.ruolo, \
                CASE WHEN p.spazio_attivo_id = s.id THEN 1 ELSE 0 END AS attivo \
         FROM membri_spazio ms \
         JOIN spazi s ON s.id = ms.spazio_id \
         JOIN preferenze_utente p ON p.utente_id = ms.utente_id \
         WHERE ms.utente_id = ? \
         ORDER BY attivo DESC, s.nome COLLATE NOCASE, s.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi dell'utente")
}

pub(crate) async fn create_space(
    pool: &SqlitePool,
    actor: &AuditActor,
    name: &str,
    space_type: &str,
) -> Result<SpaceMembership> {
    let user_id = actor
        .utente_id
        .context("Un attore di sistema non può creare uno spazio")?;
    let clean_name = name.trim();
    if clean_name.is_empty() || clean_name.chars().count() > 80 {
        bail!("Il nome dello spazio deve contenere da 1 a 80 caratteri");
    }
    if !matches!(space_type, "personale" | "famiglia" | "condiviso") {
        bail!("Tipo spazio non valido");
    }

    let mut tx = pool.begin().await.context("Transazione creazione spazio")?;
    let result =
        sqlx::query("INSERT INTO spazi (nome, tipo, creato_da_utente_id) VALUES (?, ?, ?)")
            .bind(clean_name)
            .bind(space_type)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile creare lo spazio")?;
    let space_id = result.last_insert_rowid();

    sqlx::query(
        "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
    )
    .bind(space_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile assegnare il proprietario")?;

    sqlx::query(
        "UPDATE preferenze_utente \
         SET spazio_attivo_id = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE utente_id = ?",
    )
    .bind(space_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile attivare il nuovo spazio")?;

    tx.commit()
        .await
        .context("Impossibile salvare il nuovo spazio")?;

    Ok(SpaceMembership {
        id: space_id,
        nome: clean_name.to_string(),
        tipo: space_type.to_string(),
        ruolo: "proprietario".to_string(),
        attivo: 1,
    })
}

pub(crate) async fn switch_active_space(
    pool: &SqlitePool,
    actor: &AuditActor,
    space_id: i64,
) -> Result<SpaceMembership> {
    let user_id = actor
        .utente_id
        .context("Un attore di sistema non può cambiare spazio")?;

    let membership = sqlx::query_as::<_, SpaceMembership>(
        "SELECT s.id, s.nome, s.tipo, ms.ruolo, 1 AS attivo \
         FROM membri_spazio ms \
         JOIN spazi s ON s.id = ms.spazio_id \
         WHERE ms.utente_id = ? AND s.id = ?",
    )
    .bind(user_id)
    .bind(space_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile verificare lo spazio richiesto")?
    .context("Spazio non disponibile per questo utente")?;

    sqlx::query(
        "UPDATE preferenze_utente \
         SET spazio_attivo_id = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE utente_id = ?",
    )
    .bind(space_id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile cambiare lo spazio attivo")?;

    Ok(membership)
}

pub(crate) async fn rename_active_space(
    pool: &SqlitePool,
    actor: &AuditActor,
    name: &str,
) -> Result<String> {
    let user_id = actor
        .utente_id
        .context("Un attore di sistema non può rinominare uno spazio")?;
    let clean_name = name.trim();
    if clean_name.is_empty() || clean_name.chars().count() > 80 {
        bail!("Il nome dello spazio deve contenere da 1 a 80 caratteri");
    }

    let role: Option<String> =
        sqlx::query_scalar("SELECT ruolo FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?")
            .bind(actor.spazio_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere il ruolo corrente")?;

    if !matches!(role.as_deref(), Some("proprietario" | "amministratore")) {
        bail!("Solo proprietario o amministratore possono rinominare lo spazio");
    }

    let affected = sqlx::query(
        "UPDATE spazi \
         SET nome = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
    )
    .bind(clean_name)
    .bind(actor.spazio_id)
    .execute(pool)
    .await
    .context("Impossibile rinominare lo spazio")?
    .rows_affected();

    if affected != 1 {
        bail!("Spazio attivo non trovato");
    }
    Ok(clean_name.to_string())
}

pub(crate) async fn ensure_can_write(pool: &SqlitePool) -> Result<()> {
    let actor = current_actor();
    let Some(user_id) = actor.utente_id else {
        // Le operazioni di sistema interne già esistenti restano consentite.
        return Ok(());
    };

    let role: Option<String> =
        sqlx::query_scalar("SELECT ruolo FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?")
            .bind(actor.spazio_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile verificare i permessi di scrittura")?;

    match role.as_deref() {
        Some("proprietario" | "amministratore" | "membro") => Ok(()),
        Some("lettura") => bail!("Lo spazio è in sola lettura per questo utente"),
        _ => bail!("L'utente non appartiene allo spazio attivo"),
    }
}

pub(crate) async fn ensure_can_write_sqlx(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let actor = current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(());
    };

    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM membri_spazio \
            WHERE spazio_id = ? AND utente_id = ? \
              AND ruolo IN ('proprietario', 'amministratore', 'membro')\
         )",
    )
    .bind(actor.spazio_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    if allowed {
        Ok(())
    } else {
        Err(sqlx::Error::RowNotFound)
    }
}

pub(crate) async fn spaces_summary(pool: &SqlitePool, actor: &AuditActor) -> Result<String> {
    let user_id = actor
        .utente_id
        .context("Spazi non disponibili per un attore di sistema")?;
    let spaces = list_user_spaces(pool, user_id).await?;
    let mut lines = vec![
        "👥 Spazi".to_string(),
        String::new(),
        "Lo spazio predefinito determina dove vengono creati normalmente i nuovi dati.".to_string(),
        String::new(),
    ];
    for space in spaces {
        let marker = if space.attivo != 0 { "●" } else { "○" };
        lines.push(format!(
            "{marker} {} · {} · {}",
            space.nome,
            space_type_label(&space.tipo),
            role_label(&space.ruolo)
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "Vista: {}",
        if actor.view_all {
            "🌐 Tutti i miei spazi"
        } else {
            "🎯 Solo spazio predefinito"
        }
    ));
    lines.push(String::new());
    lines.push("Comandi:".to_string());
    lines.push("/spazio_nuovo <nome> — crea e attiva uno spazio condiviso".to_string());
    lines.push("/spazio_rinomina <nome> — rinomina lo spazio attivo".to_string());
    Ok(lines.join("\n"))
}

pub(crate) fn space_type_label(value: &str) -> &str {
    match value {
        "personale" => "Personale",
        "famiglia" => "Famiglia",
        "condiviso" => "Condiviso",
        _ => value,
    }
}

pub(crate) async fn profile_summary(pool: &SqlitePool, actor: &AuditActor) -> Result<String> {
    let user_id = actor
        .utente_id
        .context("Profilo non disponibile per un attore di sistema")?;

    let (role, member_since): (String, String) = sqlx::query_as(
        "SELECT ruolo, strftime('%d/%m/%Y %H:%M', aggiunto_il, 'localtime') \
         FROM membri_spazio \
         WHERE spazio_id = ? AND utente_id = ?",
    )
    .bind(actor.spazio_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile leggere il ruolo nello spazio")?;

    let telegram = actor
        .telegram_username
        .as_deref()
        .map(|value| format!("@{value}"))
        .or_else(|| actor.telegram_user_id.map(|value| value.to_string()))
        .unwrap_or_else(|| "non collegato".to_string());

    let space_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM membri_spazio WHERE utente_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .context("Impossibile contare gli spazi disponibili")?;

    let system_role = if is_system_admin(pool, actor).await? {
        "\nRuolo sistema: Amministratore"
    } else {
        ""
    };

    Ok(format!(
        "👤 Profilo\n\nNome: {}\nTelegram: {}\nSpazio predefinito: {}\nVista: {}\nRuolo nello spazio: {}{}\nMembro dal: {}\nSpazi disponibili: {}\n\nUsa /spazi per cambiare spazio predefinito o modalità di visualizzazione.",
        actor.nome_snapshot,
        telegram,
        actor.spazio_nome_snapshot,
        if actor.view_all { "Tutti i miei spazi" } else { "Solo spazio predefinito" },
        role_label(&role),
        system_role,
        member_since,
        space_count,
    ))
}

fn role_label(role: &str) -> &str {
    match role {
        "proprietario" => "Proprietario",
        "amministratore" => "Amministratore",
        "membro" => "Membro",
        "lettura" => "Solo lettura",
        _ => role,
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

    fn telegram_profile(
        id: i64,
        first_name: &str,
        username: Option<&str>,
    ) -> TelegramIdentityInput {
        TelegramIdentityInput {
            telegram_user_id: id,
            chat_id: id,
            display_name: first_name.to_string(),
            first_name: first_name.to_string(),
            last_name: None,
            username: username.map(str::to_string),
        }
    }

    #[tokio::test]
    async fn primo_account_autorizzato_diventa_proprietario_del_bootstrap() {
        let pool = test_pool().await;
        let profile = telegram_profile(1001, "Alessio", Some("alessio_test"));

        let actor = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor");

        assert_eq!(actor.nome_snapshot, "Alessio");
        assert_eq!(actor.spazio_id, LEGACY_SPACE_ID);
        assert_eq!(actor.origine, "telegram");
        assert!(is_system_admin(&pool, &actor).await.expect("ruolo sistema"));

        let role: String = sqlx::query_scalar(
            "SELECT ruolo FROM membri_spazio WHERE spazio_id = 1 AND utente_id = ?",
        )
        .bind(actor.utente_id)
        .fetch_one(&pool)
        .await
        .expect("ruolo");
        assert_eq!(role, "proprietario");
    }

    #[tokio::test]
    async fn secondo_account_riceve_uno_spazio_personale_e_non_duplica_utenti() {
        let pool = test_pool().await;
        let first = telegram_profile(1001, "Primo", None);
        let second = telegram_profile(1002, "Secondo", None);

        resolve_telegram_profile(&pool, &first)
            .await
            .expect("primo");
        let actor = resolve_telegram_profile(&pool, &second)
            .await
            .expect("secondo");
        let mut second_again = second.clone();
        second_again.chat_id = 2002;
        let actor_again = resolve_telegram_profile(&pool, &second_again)
            .await
            .expect("secondo di nuovo");

        assert_ne!(actor.spazio_id, LEGACY_SPACE_ID);
        assert_eq!(actor.spazio_id, actor_again.spazio_id);
        assert!(!is_system_admin(&pool, &actor)
            .await
            .expect("ruolo sistema secondo"));

        let (role, kind): (String, String) = sqlx::query_as(
            "SELECT ms.ruolo, s.tipo \
             FROM membri_spazio ms JOIN spazi s ON s.id = ms.spazio_id \
             WHERE ms.spazio_id = ? AND ms.utente_id = ?",
        )
        .bind(actor.spazio_id)
        .bind(actor.utente_id)
        .fetch_one(&pool)
        .await
        .expect("membership");
        assert_eq!(role, "proprietario");
        assert_eq!(kind, "personale");

        let leaked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM membri_spazio WHERE spazio_id = 1 AND utente_id = ?",
        )
        .bind(actor.utente_id)
        .fetch_one(&pool)
        .await
        .expect("membership bootstrap");
        assert_eq!(leaked, 0);

        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM utenti")
            .fetch_one(&pool)
            .await
            .expect("utenti");
        assert_eq!(users, 2);
    }

    #[tokio::test]
    async fn spazio_creato_diventa_attivo_e_switch_richiede_membership() {
        let pool = test_pool().await;
        let actor = resolve_telegram_profile(
            &pool,
            &telegram_profile(1001, "Alessio", Some("alessio_test")),
        )
        .await
        .expect("actor");

        let family = create_space(&pool, &actor, "Famiglia", "famiglia")
            .await
            .expect("spazio famiglia");
        assert_eq!(family.ruolo, "proprietario");

        let refreshed = resolve_telegram_profile(
            &pool,
            &telegram_profile(1001, "Alessio", Some("alessio_test")),
        )
        .await
        .expect("actor aggiornato");
        assert_eq!(refreshed.spazio_id, family.id);

        let legacy = switch_active_space(&pool, &refreshed, LEGACY_SPACE_ID)
            .await
            .expect("ritorno bootstrap");
        assert_eq!(legacy.id, LEGACY_SPACE_ID);

        let missing = switch_active_space(&pool, &refreshed, 999_999).await;
        assert!(missing.is_err());
    }

    #[tokio::test]
    async fn ruolo_lettura_blocca_le_scritture() {
        let pool = test_pool().await;
        let actor = resolve_telegram_profile(
            &pool,
            &telegram_profile(1001, "Alessio", Some("alessio_test")),
        )
        .await
        .expect("actor");
        let user_id = actor.utente_id.expect("utente");

        sqlx::query(
            "UPDATE membri_spazio SET ruolo = 'lettura' WHERE spazio_id = ? AND utente_id = ?",
        )
        .bind(actor.spazio_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("ruolo lettura");

        let result = with_actor(actor, async { ensure_can_write(&pool).await }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn eliminare_la_membership_attiva_fa_fallback_su_un_altro_spazio() {
        let pool = test_pool().await;
        let profile = telegram_profile(1001, "Alessio", Some("alessio_test"));
        let actor = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor");
        let user_id = actor.utente_id.expect("utente");

        let family = create_space(&pool, &actor, "Famiglia", "famiglia")
            .await
            .expect("spazio famiglia");
        assert_ne!(family.id, LEGACY_SPACE_ID);

        sqlx::query("DELETE FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?")
            .bind(family.id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("rimozione membership attiva");

        let active: i64 = sqlx::query_scalar(
            "SELECT spazio_attivo_id FROM preferenze_utente WHERE utente_id = ?",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("fallback spazio attivo");
        assert_eq!(active, LEGACY_SPACE_ID);

        let refreshed = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor dopo fallback");
        assert_eq!(refreshed.spazio_id, LEGACY_SPACE_ID);
    }

    #[tokio::test]
    async fn bootstrap_ripara_una_preferenza_diventata_stale() {
        let pool = test_pool().await;
        let profile = telegram_profile(1001, "Alessio", Some("alessio_test"));
        let actor = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor");
        let user_id = actor.utente_id.expect("utente");
        let family = create_space(&pool, &actor, "Famiglia", "famiglia")
            .await
            .expect("spazio famiglia");

        // Simula un database precedente al trigger definitivo dello Step 7.1:
        // la membership attiva viene rimossa lasciando la preferenza orfana.
        sqlx::query("DROP TRIGGER trg_membri_spazio_spazio_attivo_delete")
            .execute(&pool)
            .await
            .expect("rimozione trigger solo nel test");
        sqlx::query("DELETE FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?")
            .bind(family.id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("membership rimossa");

        let stale: i64 = sqlx::query_scalar(
            "SELECT spazio_attivo_id FROM preferenze_utente WHERE utente_id = ?",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("preferenza stale");
        assert_eq!(stale, family.id);

        let repaired = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor riparato");
        assert_eq!(repaired.spazio_id, LEGACY_SPACE_ID);

        let active: i64 = sqlx::query_scalar(
            "SELECT spazio_attivo_id FROM preferenze_utente WHERE utente_id = ?",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("preferenza riparata");
        assert_eq!(active, LEGACY_SPACE_ID);
    }

    #[tokio::test]
    async fn vista_tutti_persiste_e_non_cambia_lo_spazio_predefinito() {
        let pool = test_pool().await;
        let profile = telegram_profile(1001, "Alessio", Some("alessio_test"));
        let actor = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor");
        let family = create_space(&pool, &actor, "Famiglia", "famiglia")
            .await
            .expect("spazio famiglia");
        let actor = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor aggiornato");
        assert_eq!(actor.spazio_id, family.id);
        assert!(!actor.view_all);

        set_view_all(&pool, &actor, true)
            .await
            .expect("vista tutti");
        let all = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor vista tutti");
        assert_eq!(all.spazio_id, family.id);
        assert!(all.view_all);

        set_view_all(&pool, &all, false)
            .await
            .expect("vista predefinita");
        let single = resolve_telegram_profile(&pool, &profile)
            .await
            .expect("actor vista singola");
        assert_eq!(single.spazio_id, family.id);
        assert!(!single.view_all);
    }

    #[tokio::test]
    async fn task_local_conserva_attore_solo_nel_contesto() {
        let actor = AuditActor {
            utente_id: Some(9),
            nome_snapshot: "Test".to_string(),
            spazio_id: 1,
            spazio_nome_snapshot: "Spazio principale".to_string(),
            view_all: false,
            origine: "telegram",
            telegram_user_id: Some(9),
            telegram_username: None,
        };

        let inside = with_actor(actor.clone(), async { current_actor() }).await;
        assert_eq!(inside, actor);
        assert_eq!(current_actor().nome_snapshot, "Sistema");
    }
}
