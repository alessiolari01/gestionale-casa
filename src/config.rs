//! Caricamento e validazione della configurazione.
//!
//! I segreti non devono mai essere scritti nel codice sorgente. La
//! configurazione viene letta dalle variabili d'ambiente e, se presente,
//! anche da un file `.env` locale escluso da Git.

use anyhow::{bail, Context, Result};

/// Configurazione necessaria per avviare il backend.
///
/// Non implementa `Debug` volutamente: il token Telegram non deve finire
/// accidentalmente nei log tramite una stampa dell'intera configurazione.
#[derive(Clone)]
pub struct Config {
    pub telegram_token: String,
    pub allowed_chat_ids: Vec<i64>,
}

impl Config {
    /// Carica e valida le variabili richieste dal progetto.
    pub fn load() -> Result<Self> {
        // Se esiste un file .env viene caricato. Le variabili già presenti
        // nell'ambiente restano prioritarie.
        let _ = dotenvy::dotenv();

        let telegram_token = std::env::var("TELOXIDE_TOKEN")
            .context("Variabile TELOXIDE_TOKEN non trovata")?;

        if telegram_token.trim().is_empty() {
            bail!("TELOXIDE_TOKEN è vuota");
        }

        let raw_chat_ids = std::env::var("ALLOWED_CHAT_IDS")
            .context("Variabile ALLOWED_CHAT_IDS non trovata")?;

        let allowed_chat_ids = raw_chat_ids
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| {
                id.parse::<i64>()
                    .with_context(|| format!("Chat ID non valido: {id}"))
            })
            .collect::<Result<Vec<_>>>()?;

        if allowed_chat_ids.is_empty() {
            bail!("ALLOWED_CHAT_IDS non contiene nessun chat ID");
        }

        Ok(Self {
            telegram_token,
            allowed_chat_ids,
        })
    }
}
