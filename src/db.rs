//! Connessione al database SQLite e gestione delle migrazioni.
//!
//! Lo schema core (tabelle condivise da tutti i moduli: `items`, `foto`,
//! `tag`/`item_tag`, `promemoria`) è definito in
//! `migrations/20260812120000_schema_core.sql` — spiegazione completa in
//! `docs/schema-core.md`. Le tabelle specifiche dei singoli moduli
//! verranno aggiunte con nuovi file di migrazione mano a mano.

// TODO: `connect(database_url: &str) -> anyhow::Result<SqlitePool>` che apre
//       la connessione (via `sqlx::SqlitePool`) e crea il file/le cartelle
//       se non esistono ancora.
// TODO: dopo l'apertura di ogni connessione eseguire
//       `PRAGMA foreign_keys = ON;` — SQLite non lo attiva di default e
//       non lo ricorda tra una connessione e l'altra, va fatto ogni volta.
// TODO: eseguire le migrazioni all'avvio (`sqlx::migrate!()`).
