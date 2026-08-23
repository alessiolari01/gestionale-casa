//! Connessione al database SQLite e gestione delle migration.
//!
//! Lo schema core condiviso dai moduli è definito in
//! `migrations/20260812120000_schema_core.sql`. Le migration sono incorporate
//! nel binario e applicate automaticamente all'avvio.

use std::{fs, path::Path, str::FromStr};

use anyhow::{Context, Result};
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Informazioni essenziali usate dal comando Telegram `/status`.
#[derive(Debug, Clone, Copy)]
pub struct DatabaseStatus {
    pub foreign_keys_enabled: bool,
    pub applied_migrations: i64,
    pub schema_core_present: bool,
    pub shared_foundations_present: bool,
}

/// Apre SQLite, crea il file se necessario e applica tutte le migration.
pub async fn connect(database_url: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)
        .with_context(|| format!("DATABASE_URL SQLite non valido: {database_url}"))?
        .create_if_missing(true)
        // SQLx abilita gia' le foreign key per SQLite, ma lo rendiamo esplicito
        // perche' e' una proprieta' di sicurezza/integrita' importante.
        .foreign_keys(true);

    ensure_parent_directory(options.get_filename())?;

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .context("Impossibile aprire il database SQLite")?;

    MIGRATOR
        .run(&pool)
        .await
        .context("Errore durante l'applicazione delle migration SQLite")?;

    let status = status(&pool).await?;
    if !status.foreign_keys_enabled {
        anyhow::bail!("SQLite avviato senza foreign key abilitate");
    }

    Ok(pool)
}

/// Legge lo stato runtime del database senza modificarne i dati applicativi.
pub async fn status(pool: &SqlitePool) -> Result<DatabaseStatus> {
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(pool)
        .await
        .context("Impossibile verificare PRAGMA foreign_keys")?;

    let applied_migrations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(pool)
            .await
            .context("Impossibile leggere lo stato delle migration")?;

    // Lo schema core e' considerato presente solo se esistono tutte le cinque
    // tabelle condivise definite dalla migration iniziale. Controllare soltanto
    // `items` potrebbe produrre un falso positivo su un database incompleto.
    let core_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' \
         AND name IN ('items', 'foto', 'tag', 'item_tag', 'promemoria')",
    )
    .fetch_one(pool)
    .await
    .context("Impossibile verificare la presenza dello schema core")?;

    let shared_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' \
         AND name IN ('utenti', 'spazi', 'membri_spazio', 'account_telegram', 'preferenze_utente', 'inviti_spazio')",
    )
    .fetch_one(pool)
    .await
    .context("Impossibile verificare le fondazioni condivise Step 7")?;

    Ok(DatabaseStatus {
        foreign_keys_enabled: foreign_keys == 1,
        applied_migrations,
        schema_core_present: core_tables == 5,
        shared_foundations_present: shared_tables == 6,
    })
}

fn ensure_parent_directory(database_path: &Path) -> Result<()> {
    let Some(parent) = database_path.parent() else {
        return Ok(());
    };

    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(parent).with_context(|| {
        format!(
            "Impossibile creare la cartella del database: {}",
            parent.display()
        )
    })?;

    Ok(())
}
