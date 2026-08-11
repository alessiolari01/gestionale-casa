//! Connessione al database SQLite e gestione delle migrazioni.
//!
//! Lo schema vero e proprio verrà definito nei file `.sql` dentro
//! `migrations/`, a partire dalle tabelle condivise da tutti i moduli
//! (foto, categorie/tag, promemoria) e poi con le tabelle specifiche di
//! ciascun modulo (oggetti, vestiti, veicoli, ricette).

// TODO: `connect(database_url: &str) -> anyhow::Result<SqlitePool>` che apre
//       la connessione (via `sqlx::SqlitePool`) e crea il file/le cartelle
//       se non esistono ancora.
// TODO: eseguire le migrazioni all'avvio (`sqlx::migrate!()`).
