//! Caricamento della configurazione da variabili d'ambiente / file `.env`.
//!
//! Le variabili attese sono definite in `.env.example`:
//! - `TELOXIDE_TOKEN`: token del bot, ottenuto da @BotFather.
//! - `ALLOWED_CHAT_IDS`: elenco di chat_id autorizzati a usare il bot,
//!   separati da virgola (vedi `auth.rs`).
//! - `DATABASE_URL`: percorso del file SQLite (es. `./data/db/gestionale.db`).

// TODO: definire una struct `Config` con i campi sopra.
// TODO: implementare `Config::load()` che legge `.env` (tramite `dotenvy`)
//       e valida che tutte le variabili obbligatorie siano presenti,
//       fallendo con un errore chiaro se manca qualcosa.
