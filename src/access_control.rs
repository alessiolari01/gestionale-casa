//! Accesso Telegram controllato tramite richiesta e approvazione.
//!
//! `ALLOWED_CHAT_IDS` resta un canale bootstrap/emergenza. Dopo il bootstrap,
//! l'autorizzazione applicativa dipende dalla presenza di un account Telegram
//! collegato a un utente attivo nel database.

use anyhow::{bail, Context, Result};
use sqlx::SqlitePool;
use teloxide::types::User;

use crate::identity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AccessRequestState {
    Unknown,
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct AccessRequestSummary {
    pub(crate) id: i64,
    pub(crate) telegram_user_id: i64,
    pub(crate) chat_id: i64,
    pub(crate) username_snapshot: Option<String>,
    pub(crate) nome_snapshot: String,
    pub(crate) cognome_snapshot: Option<String>,
    pub(crate) stato: String,
    pub(crate) letto_admin_il: Option<String>,
    pub(crate) richiesta_il: String,
}

fn telegram_id(user: &User) -> Result<i64> {
    i64::try_from(user.id.0).context("Telegram user ID non rappresentabile come INTEGER SQLite")
}

pub(crate) async fn request_state(pool: &SqlitePool, user: &User) -> Result<AccessRequestState> {
    let telegram_user_id = telegram_id(user)?;

    let approved: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM account_telegram at \
            JOIN utenti u ON u.id = at.utente_id \
            WHERE at.telegram_user_id = ? AND u.stato = 'attivo'\
         )",
    )
    .bind(telegram_user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare l'accesso Telegram")?;

    if approved {
        return Ok(AccessRequestState::Approved);
    }

    let state: Option<String> =
        sqlx::query_scalar("SELECT stato FROM richieste_accesso WHERE telegram_user_id = ?")
            .bind(telegram_user_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere lo stato della richiesta di accesso")?;

    Ok(match state.as_deref() {
        Some("pendente") => AccessRequestState::Pending,
        Some("approvata") => AccessRequestState::Approved,
        Some("rifiutata") => AccessRequestState::Rejected,
        _ => AccessRequestState::Unknown,
    })
}

/// Crea o riapre una richiesta. Una richiesta già pendente resta idempotente.
pub(crate) async fn submit_request(pool: &SqlitePool, chat_id: i64, user: &User) -> Result<i64> {
    let telegram_user_id = telegram_id(user)?;
    let display_name = user.full_name().trim().to_string();
    if display_name.is_empty() {
        bail!("Telegram non ha fornito un nome utilizzabile");
    }

    let existing_account: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM account_telegram WHERE telegram_user_id = ?)",
    )
    .bind(telegram_user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare l'account Telegram")?;
    if existing_account {
        bail!("Account già autorizzato");
    }

    sqlx::query(
        "INSERT INTO richieste_accesso (\
            telegram_user_id, chat_id, username_snapshot, nome_snapshot, cognome_snapshot, stato\
         ) VALUES (?, ?, ?, ?, ?, 'pendente') \
         ON CONFLICT(telegram_user_id) DO UPDATE SET \
            chat_id = excluded.chat_id, \
            username_snapshot = excluded.username_snapshot, \
            nome_snapshot = excluded.nome_snapshot, \
            cognome_snapshot = excluded.cognome_snapshot, \
            stato = CASE \
                WHEN richieste_accesso.stato = 'approvata' THEN 'approvata' \
                ELSE 'pendente' \
            END, \
            richiesta_il = CASE \
                WHEN richieste_accesso.stato = 'pendente' THEN richieste_accesso.richiesta_il \
                ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
            END, \
            decisa_il = CASE \
                WHEN richieste_accesso.stato = 'approvata' THEN richieste_accesso.decisa_il \
                ELSE NULL \
            END, \
            decisa_da_utente_id = CASE \
                WHEN richieste_accesso.stato = 'approvata' THEN richieste_accesso.decisa_da_utente_id \
                ELSE NULL \
            END, \
            letto_admin_il = CASE \
                WHEN richieste_accesso.stato = 'rifiutata' THEN NULL \
                ELSE richieste_accesso.letto_admin_il \
            END",
    )
    .bind(telegram_user_id)
    .bind(chat_id)
    .bind(user.username.as_deref())
    .bind(&user.first_name)
    .bind(user.last_name.as_deref())
    .execute(pool)
    .await
    .context("Impossibile registrare la richiesta di accesso")?;

    sqlx::query_scalar("SELECT id FROM richieste_accesso WHERE telegram_user_id = ?")
        .bind(telegram_user_id)
        .fetch_one(pool)
        .await
        .context("Impossibile rileggere la richiesta di accesso")
}

pub(crate) async fn pending_count(pool: &SqlitePool) -> Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM richieste_accesso WHERE stato = 'pendente'")
        .fetch_one(pool)
        .await
        .context("Impossibile contare le richieste di accesso")
}

pub(crate) async fn list_pending(pool: &SqlitePool) -> Result<Vec<AccessRequestSummary>> {
    sqlx::query_as::<_, AccessRequestSummary>(
        "SELECT id, telegram_user_id, chat_id, username_snapshot, nome_snapshot, \
                cognome_snapshot, stato, letto_admin_il, \
                strftime('%d/%m/%Y %H:%M', richiesta_il, 'localtime') AS richiesta_il \
         FROM richieste_accesso \
         WHERE stato = 'pendente' \
         ORDER BY richiesta_il, id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le richieste di accesso")
}

pub(crate) async fn get_request(
    pool: &SqlitePool,
    request_id: i64,
) -> Result<Option<AccessRequestSummary>> {
    sqlx::query_as::<_, AccessRequestSummary>(
        "SELECT id, telegram_user_id, chat_id, username_snapshot, nome_snapshot, \
                cognome_snapshot, stato, letto_admin_il, \
                strftime('%d/%m/%Y %H:%M', richiesta_il, 'localtime') AS richiesta_il \
         FROM richieste_accesso WHERE id = ?",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere la richiesta di accesso")
}

pub(crate) async fn mark_read(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    request_id: i64,
) -> Result<()> {
    if !identity::is_primary_admin(pool, actor).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    let affected = sqlx::query(
        "UPDATE richieste_accesso \
         SET letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ?",
    )
    .bind(request_id)
    .execute(pool)
    .await
    .context("Impossibile segnare la richiesta come letta")?
    .rows_affected();
    if affected != 1 {
        bail!("Richiesta di accesso non trovata");
    }
    Ok(())
}
pub(crate) async fn approve_request(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    request_id: i64,
) -> Result<AccessRequestSummary> {
    if !identity::is_primary_admin(pool, actor).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    let admin_user_id = actor
        .utente_id
        .context("Amministratore principale privo di identità interna")?;

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione di approvazione")?;

    let request = sqlx::query_as::<_, AccessRequestSummary>(
        "SELECT id, telegram_user_id, chat_id, username_snapshot, nome_snapshot, \
                cognome_snapshot, stato, letto_admin_il, richiesta_il \
         FROM richieste_accesso WHERE id = ?",
    )
    .bind(request_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile leggere la richiesta da approvare")?
    .context("Richiesta di accesso non trovata")?;

    if request.stato != "pendente" {
        bail!("La richiesta non è più pendente");
    }

    let first_name = request.nome_snapshot.trim();
    let display_name = request
        .cognome_snapshot
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|last_name| format!("{first_name} {last_name}"))
        .unwrap_or_else(|| first_name.to_string());
    identity::provision_approved_telegram_account(
        &mut tx,
        request.telegram_user_id,
        request.chat_id,
        &display_name,
        first_name,
        request.cognome_snapshot.as_deref(),
        request.username_snapshot.as_deref(),
    )
    .await?;

    let affected = sqlx::query(
        "UPDATE richieste_accesso \
         SET stato = 'approvata', \
             decisa_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
             decisa_da_utente_id = ?, \
             letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ? AND stato = 'pendente'",
    )
    .bind(admin_user_id)
    .bind(request_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile approvare la richiesta")?
    .rows_affected();

    if affected != 1 {
        bail!("La richiesta non è più pendente");
    }

    tx.commit()
        .await
        .context("Impossibile salvare l'approvazione")?;
    Ok(request)
}

pub(crate) async fn reject_request(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    request_id: i64,
) -> Result<AccessRequestSummary> {
    if !identity::is_primary_admin(pool, actor).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    let admin_user_id = actor
        .utente_id
        .context("Amministratore principale privo di identità interna")?;

    let request = get_request(pool, request_id)
        .await?
        .context("Richiesta di accesso non trovata")?;
    if request.stato != "pendente" {
        bail!("La richiesta non è più pendente");
    }

    let affected = sqlx::query(
        "UPDATE richieste_accesso \
         SET stato = 'rifiutata', \
             decisa_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
             decisa_da_utente_id = ?, \
             letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ? AND stato = 'pendente'",
    )
    .bind(admin_user_id)
    .bind(request_id)
    .execute(pool)
    .await
    .context("Impossibile rifiutare la richiesta")?
    .rows_affected();
    if affected != 1 {
        bail!("La richiesta non è più pendente");
    }
    Ok(request)
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

    async fn primary_admin_actor(pool: &SqlitePool) -> identity::AuditActor {
        let admin_id = sqlx::query(
            "INSERT INTO utenti (nome_visualizzato, ruolo_sistema, amministratore_principale) \
             VALUES ('Admin', 'admin', 1)",
        )
        .execute(pool)
        .await
        .expect("admin")
        .last_insert_rowid();
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (1, ?, 'proprietario')",
        )
        .bind(admin_id)
        .execute(pool)
        .await
        .expect("membership");
        sqlx::query("UPDATE spazi SET creato_da_utente_id = ? WHERE id = 1")
            .bind(admin_id)
            .execute(pool)
            .await
            .expect("owner spazio");
        sqlx::query("INSERT INTO preferenze_utente (utente_id, spazio_attivo_id) VALUES (?, 1)")
            .bind(admin_id)
            .execute(pool)
            .await
            .expect("preferenza");
        identity::AuditActor {
            utente_id: Some(admin_id),
            nome_snapshot: "Admin".to_string(),
            spazio_id: 1,
            spazio_nome_snapshot: "Spazio principale".to_string(),
            view_all: false,
            origine: "test",
            telegram_user_id: Some(1),
            telegram_username: Some("admin".to_string()),
        }
    }

    #[tokio::test]
    async fn approvazione_crea_utente_normale_con_spazio_personale() {
        let pool = test_pool().await;
        let actor = primary_admin_actor(&pool).await;
        let request_id = sqlx::query(
            "INSERT INTO richieste_accesso \
             (telegram_user_id, chat_id, username_snapshot, nome_snapshot, cognome_snapshot) \
             VALUES (2002, 3002, 'tester', 'Mario', 'Rossi')",
        )
        .execute(&pool)
        .await
        .expect("richiesta")
        .last_insert_rowid();

        approve_request(&pool, &actor, request_id)
            .await
            .expect("approvazione");

        let (user_id, role, name): (i64, String, String) = sqlx::query_as(
            "SELECT u.id, u.ruolo_sistema, u.nome_visualizzato \
             FROM account_telegram at JOIN utenti u ON u.id = at.utente_id \
             WHERE at.telegram_user_id = 2002",
        )
        .fetch_one(&pool)
        .await
        .expect("utente approvato");
        assert_eq!(role, "utente");
        assert_eq!(name, "Mario Rossi");

        let bootstrap_membership: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM membri_spazio WHERE spazio_id = 1 AND utente_id = ?",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("membership bootstrap");
        assert_eq!(bootstrap_membership, 0);

        let personal_spaces: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM membri_spazio ms \
             JOIN spazi s ON s.id = ms.spazio_id \
             WHERE ms.utente_id = ? AND s.tipo = 'personale' AND s.id <> 1",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("spazio personale");
        assert_eq!(personal_spaces, 1);

        let state: String = sqlx::query_scalar("SELECT stato FROM richieste_accesso WHERE id = ?")
            .bind(request_id)
            .fetch_one(&pool)
            .await
            .expect("stato richiesta");
        assert_eq!(state, "approvata");
    }

    #[tokio::test]
    async fn conteggio_richieste_pendenti_funzionante() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO richieste_accesso \
             (telegram_user_id, chat_id, nome_snapshot, stato, decisa_il, decisa_da_utente_id) \
             VALUES (7, 7, 'Test', 'pendente', NULL, NULL)",
        )
        .execute(&pool)
        .await
        .expect("richiesta");

        let count = pending_count(&pool).await.expect("conteggio");
        assert_eq!(count, 1);
    }
}
