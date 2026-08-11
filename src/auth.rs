//! Whitelist degli utenti autorizzati a usare il bot.
//!
//! Il bot deve rispondere solo ai `chat_id` presenti in `ALLOWED_CHAT_IDS`
//! (vedi `config.rs`). Qualunque altro utente scriva al bot va ignorato o
//! ricevere un messaggio esplicito di accesso negato — mai eseguire comandi
//! per chat_id non in whitelist.

// TODO: `is_authorized(chat_id: i64, allowed: &[i64]) -> bool`.
// TODO: middleware/filtro da agganciare al dispatcher di teloxide così che
//       ogni handler dei moduli lo erediti automaticamente, invece di
//       doverlo controllare manualmente in ogni comando.
