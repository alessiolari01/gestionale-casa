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

const LEGACY_SPACE_ID: i64 = 1;
const LEGACY_SPACE_NAME: &str = "Spazio principale";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditActor {
    pub(crate) utente_id: Option<i64>,
    pub(crate) nome_snapshot: String,
    pub(crate) spazio_id: i64,
    pub(crate) spazio_nome_snapshot: String,
    pub(crate) origine: &'static str,
    pub(crate) telegram_user_id: Option<i64>,
    pub(crate) telegram_username: Option<String>,
}

impl AuditActor {
    pub(crate) fn system() -> Self {
        Self {
            utente_id: None,
            nome_snapshot: "Sistema".to_string(),
            spazio_id: LEGACY_SPACE_ID,
            spazio_nome_snapshot: LEGACY_SPACE_NAME.to_string(),
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
    CURRENT_ACTOR
        .try_with(Clone::clone)
        .unwrap_or_else(|_| AuditActor::system())
}

/// Risolve o crea l'utente interno collegato a un account Telegram.
///
/// Durante la fase di compatibilità 7.1 ogni account Telegram autorizzato
/// viene aggiunto allo spazio bootstrap #1. Il primo diventa proprietario; i
/// successivi amministratori. Il flusso operativo di creazione/switch/invito
/// degli spazi verrà esposto solo quando le query CRUD saranno space-aware.
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

    ensure_legacy_membership(&mut tx, user_id).await?;

    let (space_id, space_name): (i64, String) = sqlx::query_as(
        "SELECT s.id, s.nome \
         FROM preferenze_utente p \
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
        origine: "telegram",
        telegram_user_id: Some(telegram_user_id),
        telegram_username: profile.username.clone(),
    })
}

async fn ensure_legacy_membership(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
) -> Result<()> {
    let membership_exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?\
         )",
    )
    .bind(LEGACY_SPACE_ID)
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .context("Impossibile verificare la membership bootstrap")?;

    if membership_exists == 0 {
        let members: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM membri_spazio WHERE spazio_id = ?")
                .bind(LEGACY_SPACE_ID)
                .fetch_one(&mut **tx)
                .await
                .context("Impossibile contare i membri dello spazio bootstrap")?;

        let role = if members == 0 {
            "proprietario"
        } else {
            "amministratore"
        };

        sqlx::query("INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, ?)")
            .bind(LEGACY_SPACE_ID)
            .bind(user_id)
            .bind(role)
            .execute(&mut **tx)
            .await
            .context("Impossibile aggiungere l'utente allo spazio bootstrap")?;
    }

    let preference_exists: i64 =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM preferenze_utente WHERE utente_id = ?)")
            .bind(user_id)
            .fetch_one(&mut **tx)
            .await
            .context("Impossibile verificare le preferenze utente")?;

    if preference_exists == 0 {
        sqlx::query("INSERT INTO preferenze_utente (utente_id, spazio_attivo_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(LEGACY_SPACE_ID)
            .execute(&mut **tx)
            .await
            .context("Impossibile inizializzare lo spazio attivo")?;
    }

    Ok(())
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

    Ok(format!(
        "👤 Profilo\n\nNome: {}\nTelegram: {}\nSpazio attivo: {}\nRuolo: {}\nMembro dal: {}\n\nℹ️ Step 7.1: lo spazio principale è ancora il contesto runtime di compatibilità. Creazione, inviti e cambio spazio verranno abilitati quando tutte le query saranno isolate per spazio.",
        actor.nome_snapshot,
        telegram,
        actor.spazio_nome_snapshot,
        role_label(&role),
        member_since,
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
    async fn secondo_account_bootstrap_diventa_amministratore_e_non_duplica_utenti() {
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
        resolve_telegram_profile(&pool, &second_again)
            .await
            .expect("secondo di nuovo");

        let role: String = sqlx::query_scalar(
            "SELECT ruolo FROM membri_spazio WHERE spazio_id = 1 AND utente_id = ?",
        )
        .bind(actor.utente_id)
        .fetch_one(&pool)
        .await
        .expect("ruolo");
        assert_eq!(role, "amministratore");

        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM utenti")
            .fetch_one(&pool)
            .await
            .expect("utenti");
        assert_eq!(users, 2);
    }

    #[tokio::test]
    async fn task_local_conserva_attore_solo_nel_contesto() {
        let actor = AuditActor {
            utente_id: Some(9),
            nome_snapshot: "Test".to_string(),
            spazio_id: 1,
            spazio_nome_snapshot: "Spazio principale".to_string(),
            origine: "telegram",
            telegram_user_id: Some(9),
            telegram_username: None,
        };

        let inside = with_actor(actor.clone(), async { current_actor() }).await;
        assert_eq!(inside, actor);
        assert_eq!(current_actor().nome_snapshot, "Sistema");
    }
}
