//! Scorte: dispensa, frigo e freezer.
//!
//! Via libera di Alessio il 16 settembre 2026, subito dopo la chiusura della
//! spesa (`lista_spesa::chiudi_spesa`), che è il momento in cui la merce
//! entra in casa. Specifica e decisioni di struttura in
//! `docs/previsto/dispensa.md`, comportamento in `docs/moduli/dispensa.md`.
//!
//! La conservazione è un'entità propria, non un riuso di
//! case/stanze/contenitori: un contenitore dice *dove sta un oggetto*, una
//! scorta dice *in che condizione è conservata una quantità di alimento*, ed
//! è quella condizione a decidere quanto dura.
//!
//! Stessa divisione in tre parti degli altri moduli: dominio puro (testato
//! senza database), funzioni database (`sqlite::memory:` nei test), UI
//! Telegram.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Context as _;
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::modules::{
    calendario,
    lista_spesa::{
        alimento_visibile_per_id, cerca_nel_catalogo, formatta_quantita, prodotto_visibile_per_id,
        valida_descrizione_manuale, valida_quantita_con_default, IdentitaCatalogo,
        RisultatoCatalogo,
    },
    liste,
};

type Bot = crate::context_bot::ContextBot;

// ===========================================================================
// Dominio puro.
// ===========================================================================

/// Dove è conservata una scorta. I tre posti standard esistono sempre, senza
/// che l'utente debba configurare niente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conservazione {
    Dispensa,
    Frigo,
    Freezer,
}

impl Conservazione {
    pub const TUTTE: [Conservazione; 3] = [Self::Dispensa, Self::Frigo, Self::Freezer];

    /// Come viene salvata a database (CHECK sulla colonna `conservazione`).
    pub fn token(self) -> &'static str {
        match self {
            Self::Dispensa => "dispensa",
            Self::Frigo => "frigo",
            Self::Freezer => "freezer",
        }
    }

    pub fn da_token(valore: &str) -> Option<Self> {
        match valore {
            "dispensa" => Some(Self::Dispensa),
            "frigo" => Some(Self::Frigo),
            "freezer" => Some(Self::Freezer),
            _ => None,
        }
    }

    /// Etichetta con l'icona, una per luogo (C4: un simbolo, un significato).
    pub fn etichetta(self) -> &'static str {
        match self {
            Self::Dispensa => "🧺 Dispensa",
            Self::Frigo => "🧊 Frigo",
            Self::Freezer => "❄️ Freezer",
        }
    }
}

/// Interpreta una scadenza scritta a mano: `GG/MM/AAAA` (come la mostra il
/// bot) oppure `AAAA-MM-GG`. Ritorna la data in formato ISO, l'unico che
/// entra a database.
///
/// La validazione è semantica, non solo di forma: `30/02/2026` viene
/// rifiutata qui invece di diventare `NULL` una volta arrivata a SQLite --
/// stessa lezione già imparata con le date del planner.
pub fn interpreta_scadenza(testo: &str) -> Option<String> {
    let testo = testo.trim();
    if calendario::valid_date(testo) {
        return Some(testo.to_string());
    }
    let parti: Vec<&str> = testo.split(['/', '-', '.']).collect();
    if parti.len() != 3 {
        return None;
    }
    let giorno: u32 = parti[0].parse().ok()?;
    let mese: u32 = parti[1].parse().ok()?;
    let anno: i32 = parti[2].parse().ok()?;
    let iso = format!("{anno:04}-{mese:02}-{giorno:02}");
    calendario::valid_date(&iso).then_some(iso)
}

/// Etichetta di una scorta su un pulsante: nome, quantità e -- se c'è -- la
/// scadenza **a capo** (C15: una parte opzionale non si accoda con " · ",
/// altrimenti Telegram taglia l'etichetta senza avvisare).
pub fn etichetta_scorta(
    descrizione: &str,
    quantita: f64,
    unita: &str,
    scadenza: Option<&str>,
) -> String {
    let base = format!(
        "{} · {} {unita}",
        liste::tronca(descrizione, 30),
        formatta_quantita(quantita)
    );
    match scadenza {
        Some(data) => format!("{base}\n📅 scade il {}", calendario::display_date(data)),
        None => base,
    }
}

// ===========================================================================
// Database (`sqlite::memory:` nei test).
// ===========================================================================

#[derive(Debug, Clone, FromRow)]
pub struct Scorta {
    pub id: i64,
    pub conservazione: String,
    pub descrizione: String,
    pub quantita: f64,
    pub unita_simbolo: String,
    pub scadenza: Option<String>,
}

/// Quante scorte ci sono in un luogo di conservazione.
pub async fn conta_scorte(pool: &SqlitePool, dove: Conservazione) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    sqlx::query_scalar("SELECT COUNT(*) FROM scorte WHERE spazio_id = ? AND conservazione = ?")
        .bind(actor.spazio_id)
        .bind(dove.token())
        .fetch_one(pool)
        .await
        .context("Impossibile contare le scorte")
}

/// Una pagina di scorte di un luogo, cinque per pagina come ogni altra lista
/// del bot (C6 -- l'eccezione senza paginazione è solo della lista della
/// spesa). Prima quelle che scadono, poi le altre: una scorta con una
/// scadenza vicina è la cosa che si vuole vedere per prima.
pub async fn elenco_scorte(
    pool: &SqlitePool,
    dove: Conservazione,
    pagina: i64,
) -> anyhow::Result<Vec<Scorta>> {
    let actor = crate::identity::current_actor();
    sqlx::query_as(
        "SELECT id, conservazione, descrizione, quantita, unita_simbolo, scadenza \
         FROM scorte WHERE spazio_id = ? AND conservazione = ? \
         ORDER BY (scadenza IS NULL), scadenza ASC, descrizione COLLATE NOCASE, id \
         LIMIT ? OFFSET ?",
    )
    .bind(actor.spazio_id)
    .bind(dove.token())
    .bind(liste::VOCI_PER_PAGINA as i64)
    .bind(liste::scarto(pagina))
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le scorte")
}

pub async fn scorta_per_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Scorta>> {
    let actor = crate::identity::current_actor();
    sqlx::query_as(
        "SELECT id, conservazione, descrizione, quantita, unita_simbolo, scadenza \
         FROM scorte WHERE id = ? AND spazio_id = ?",
    )
    .bind(id)
    .bind(actor.spazio_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere la scorta")
}

/// Aggiunge una scorta. `identita` collega la scorta al catalogo quando la
/// si è scelta da lì: serve alla futura sottrazione dal fabbisogno della
/// lista della spesa, che senza un id non potrebbe riconoscere l'alimento.
pub async fn aggiungi_scorta(
    pool: &SqlitePool,
    dove: Conservazione,
    identita: Option<IdentitaCatalogo>,
    descrizione: &str,
    quantita: f64,
    unita_simbolo: &str,
) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    let utente_id = actor.utente_id.context("Utente non disponibile")?;
    let (alimento_id, prodotto_id) = match identita {
        Some(IdentitaCatalogo::Alimento(id)) => (Some(id), None),
        Some(IdentitaCatalogo::Prodotto(id)) => (None, Some(id)),
        None => (None, None),
    };
    let id = sqlx::query(
        "INSERT INTO scorte \
         (proprietario_utente_id, spazio_id, conservazione, alimento_id, \
          prodotto_alimentare_id, descrizione, quantita, unita_simbolo, origine) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'manuale')",
    )
    .bind(utente_id)
    .bind(actor.spazio_id)
    .bind(dove.token())
    .bind(alimento_id)
    .bind(prodotto_id)
    .bind(descrizione)
    .bind(quantita)
    .bind(unita_simbolo)
    .execute(pool)
    .await
    .context("Impossibile aggiungere la scorta")?
    .last_insert_rowid();
    Ok(id)
}

pub async fn aggiorna_quantita(
    pool: &SqlitePool,
    id: i64,
    quantita: f64,
    unita_simbolo: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE scorte SET quantita = ?, unita_simbolo = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(quantita)
    .bind(unita_simbolo)
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare la quantità")?;
    Ok(())
}

/// Imposta o toglie la scadenza (`None` la toglie): resta sempre opzionale,
/// deciso con Alessio.
pub async fn imposta_scadenza(
    pool: &SqlitePool,
    id: i64,
    scadenza: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE scorte SET scadenza = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(scadenza)
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare la scadenza")?;
    Ok(())
}

/// Sposta una scorta da un luogo all'altro (dalla spesa al freezer, dal
/// freezer al frigo per scongelare...).
pub async fn sposta_scorta(pool: &SqlitePool, id: i64, dove: Conservazione) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE scorte SET conservazione = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(dove.token())
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile spostare la scorta")?;
    Ok(())
}

pub async fn rimuovi_scorta(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM scorte WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Impossibile eliminare la scorta")?;
    Ok(())
}

/// Se la merce comprata entra da sola in dispensa chiudendo la spesa.
/// Acceso di default (deciso con Alessio): la quantità giusta è quasi sempre
/// quella comprata, e un passaggio obbligatorio in più a fine spesa è il
/// modo più sicuro per far smettere di usare la funzione.
pub async fn ingresso_automatico(pool: &SqlitePool) -> bool {
    let Some(utente_id) = crate::identity::current_actor().utente_id else {
        return false;
    };
    sqlx::query_scalar::<_, i64>(
        "SELECT dispensa_ingresso_automatico FROM preferenze_utente WHERE utente_id = ?",
    )
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|valore| valore != 0)
    .unwrap_or(true)
}

pub async fn imposta_ingresso_automatico(pool: &SqlitePool, attivo: bool) -> anyhow::Result<()> {
    let utente_id = crate::identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    sqlx::query(
        "UPDATE preferenze_utente SET dispensa_ingresso_automatico = ? WHERE utente_id = ?",
    )
    .bind(i64::from(attivo))
    .bind(utente_id)
    .execute(pool)
    .await
    .context("Impossibile salvare la preferenza")?;
    Ok(())
}

/// Fa entrare in dispensa la merce di una spesa appena chiusa.
///
/// Entrano solo le voci collegate al catalogo (alimento o prodotto): una
/// voce libera come "Detersivo piatti" non è un alimento e non ha nulla da
/// scalare da nessun fabbisogno -- resta fuori, e chi la vuole in dispensa
/// la aggiunge a mano. Il luogo di partenza è sempre la dispensa: è quello
/// giusto per la maggior parte della spesa, e spostare in frigo o freezer è
/// un tocco (`🔀 Sposta`).
///
/// Ritorna quante scorte sono entrate.
/// Riga grezza di una voce archiviata: alimento, prodotto, descrizione,
/// quantità e unità, così come le rilegge la query.
type RigaArchiviataGrezza = (
    Option<i64>,
    Option<i64>,
    String,
    Option<f64>,
    Option<String>,
);

pub async fn ingresso_da_chiusura(pool: &SqlitePool, chiusura_id: i64) -> anyhow::Result<usize> {
    let actor = crate::identity::current_actor();
    let utente_id = actor.utente_id.context("Utente non disponibile")?;
    let righe: Vec<RigaArchiviataGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione, quantita, unita_simbolo \
             FROM liste_spesa_voci_archiviate \
             WHERE chiusura_id = ? \
               AND (alimento_id IS NOT NULL OR prodotto_alimentare_id IS NOT NULL) \
               AND quantita IS NOT NULL AND unita_simbolo IS NOT NULL",
    )
    .bind(chiusura_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci archiviate da mettere in dispensa")?;

    if righe.is_empty() {
        return Ok(0);
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    let mut entrate = 0usize;
    for (alimento_id, prodotto_id, descrizione, quantita, unita_simbolo) in righe {
        let (Some(quantita), Some(unita_simbolo)) = (quantita, unita_simbolo) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO scorte \
             (proprietario_utente_id, spazio_id, conservazione, alimento_id, \
              prodotto_alimentare_id, descrizione, quantita, unita_simbolo, origine, chiusura_id) \
             VALUES (?, ?, 'dispensa', ?, ?, ?, ?, ?, 'spesa', ?)",
        )
        .bind(utente_id)
        .bind(actor.spazio_id)
        .bind(alimento_id)
        .bind(prodotto_id)
        .bind(&descrizione)
        .bind(quantita)
        .bind(&unita_simbolo)
        .bind(chiusura_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile far entrare una voce in dispensa")?;
        entrate += 1;
    }
    tx.commit()
        .await
        .context("Impossibile salvare l'ingresso in dispensa")?;
    Ok(entrate)
}

// ===========================================================================
// UI Telegram.
// ===========================================================================

// Il prefisso comune "Awaiting" descrive la natura di ogni stato (in attesa
// di un input testuale) ed è lo stesso schema delle altre mappe di sessione
// del progetto -- non è un nome ripetuto per distrazione.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone)]
enum DispensaConversationState {
    AwaitingRicerca {
        dove: Conservazione,
    },
    AwaitingDescrizione {
        dove: Conservazione,
    },
    AwaitingQuantita {
        dove: Conservazione,
        identita: Option<IdentitaCatalogo>,
        descrizione: String,
        unita_default: Option<String>,
    },
    AwaitingQuantitaModifica {
        scorta_id: i64,
        unita_attuale: String,
    },
    AwaitingScadenza {
        scorta_id: i64,
    },
}

/// Sessioni di input testuale del modulo, stesso schema di
/// `ListaSpesaSessionStore`.
#[derive(Clone, Default)]
pub struct DispensaSessionStore {
    inner: Arc<Mutex<HashMap<i64, DispensaConversationState>>>,
}

impl DispensaSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, chat_id: i64) -> Option<DispensaConversationState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&chat_id)
            .cloned()
    }

    fn set(&self, chat_id: i64, state: DispensaConversationState) {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(chat_id, state);
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&chat_id);
    }

    pub fn has_active(&self, chat_id: i64) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains_key(&chat_id)
    }

    /// Serve al controllo pre-swap dell'automazione, che su Windows non
    /// viene compilato (`#[cfg(unix)]` in `main.rs`): come le altre mappe,
    /// qui il metodo risulterebbe inutilizzato solo su questa macchina.
    #[allow(dead_code)]
    pub fn active_chat_ids(&self) -> Vec<i64> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .copied()
            .collect()
    }
}

fn button(label: impl Into<String>, callback: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.into(), callback.into())
}

fn nav_row(back: &str) -> Vec<InlineKeyboardButton> {
    vec![
        button("⬅️ Indietro", back),
        button("🏠 Menù principale", "menu:main"),
    ]
}

fn nav_markup(back: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![nav_row(back)])
}

fn annulla_row(annulla: &str) -> Vec<InlineKeyboardButton> {
    vec![
        button("❌ Annulla", annulla),
        button("🏠 Menù principale", "menu:main"),
    ]
}

async fn invalid(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "⚠️ Pulsante non valido o non più disponibile.")
        .reply_markup(nav_markup("dispensa:menu"))
        .await?;
    Ok(())
}

async fn expired(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "ℹ️ Questa operazione non è più attiva. Riapri le scorte.",
    )
    .reply_markup(nav_markup("dispensa:menu"))
    .await?;
    Ok(())
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &DispensaSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if text.trim() == "/annulla" && sessions.get(chat_id).is_some() {
        sessions.clear_chat(chat_id);
        bot.annulla_e_avvisa(chat_id, "❌ Operazione annullata.");
        mostra_menu(bot, msg.chat.id, pool, None).await?;
        return Ok(true);
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        DispensaConversationState::AwaitingRicerca { dove } => {
            let query = text.trim();
            if query.is_empty() {
                bot.send_message(msg.chat.id, "⚠️ Scrivi un nome da cercare.")
                    .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
                        "dispensa:menu",
                    )]))
                    .await?;
                return Ok(true);
            }
            match cerca_nel_catalogo(pool, query, 10).await {
                Ok(risultati) if risultati.is_empty() => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "🔎 Nessun risultato per \"{query}\".\n\nProva un altro nome oppure scrivila a mano."
                        ),
                    )
                    .reply_markup(risultati_keyboard(&[], dove))
                    .await?;
                }
                Ok(risultati) => {
                    bot.send_message(msg.chat.id, format!("🔎 Risultati per \"{query}\""))
                        .reply_markup(risultati_keyboard(&risultati, dove))
                        .await?;
                }
                Err(errore) => {
                    tracing::warn!(?errore, "Ricerca nel catalogo fallita");
                    bot.send_message(msg.chat.id, "⚠️ Non riesco a cercare nel catalogo.")
                        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
                            "dispensa:menu",
                        )]))
                        .await?;
                }
            }
        }
        DispensaConversationState::AwaitingDescrizione { dove } => {
            match valida_descrizione_manuale(text) {
                Ok(descrizione) => {
                    sessions.set(
                        chat_id,
                        DispensaConversationState::AwaitingQuantita {
                            dove,
                            identita: None,
                            descrizione: descrizione.clone(),
                            unita_default: None,
                        },
                    );
                    bot.send_message(
                        msg.chat.id,
                        format!("➕ {descrizione}\n\nScrivi quantità e unità (es. \"500 g\")."),
                    )
                    .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
                        "dispensa:menu",
                    )]))
                    .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
                            "dispensa:menu",
                        )]))
                        .await?;
                }
            }
        }
        DispensaConversationState::AwaitingQuantita {
            dove,
            identita,
            descrizione,
            unita_default,
        } => match valida_quantita_con_default(text, unita_default.as_deref()) {
            Ok((quantita, unita)) => {
                sessions.clear_chat(chat_id);
                match aggiungi_scorta(pool, dove, identita, &descrizione, quantita, &unita).await {
                    Ok(_) => {
                        mostra_luogo(bot, msg.chat.id, pool, dove, 0, Some("✅ Scorta aggiunta."))
                            .await?;
                    }
                    Err(errore) => {
                        tracing::warn!(?errore, "Salvataggio scorta fallito");
                        mostra_luogo(
                            bot,
                            msg.chat.id,
                            pool,
                            dove,
                            0,
                            Some("⚠️ Non riesco a salvare la scorta."),
                        )
                        .await?;
                    }
                }
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
                        "dispensa:menu",
                    )]))
                    .await?;
            }
        },
        DispensaConversationState::AwaitingQuantitaModifica {
            scorta_id,
            unita_attuale,
        } => match valida_quantita_con_default(text, Some(&unita_attuale)) {
            Ok((quantita, unita)) => {
                sessions.clear_chat(chat_id);
                if let Err(errore) = aggiorna_quantita(pool, scorta_id, quantita, &unita).await {
                    tracing::warn!(?errore, scorta_id, "Aggiornamento quantità fallito");
                }
                mostra_scorta(
                    bot,
                    msg.chat.id,
                    pool,
                    scorta_id,
                    Some("✅ Quantità aggiornata."),
                )
                .await?;
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(&format!(
                        "dispensa:scorta:{scorta_id}"
                    ))]))
                    .await?;
            }
        },
        DispensaConversationState::AwaitingScadenza { scorta_id } => {
            match interpreta_scadenza(text) {
                Some(data) => {
                    sessions.clear_chat(chat_id);
                    if let Err(errore) = imposta_scadenza(pool, scorta_id, Some(&data)).await {
                        tracing::warn!(?errore, scorta_id, "Salvataggio scadenza fallito");
                    }
                    mostra_scorta(
                        bot,
                        msg.chat.id,
                        pool,
                        scorta_id,
                        Some("✅ Scadenza aggiornata."),
                    )
                    .await?;
                }
                None => {
                    bot.send_message(msg.chat.id, "⚠️ Data non valida. Scrivila come 31/12/2026.")
                        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(&format!(
                            "dispensa:scorta:{scorta_id}"
                        ))]))
                        .await?;
                }
            }
        }
    }
    Ok(true)
}

fn risultati_keyboard(
    risultati: &[RisultatoCatalogo],
    dove: Conservazione,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = risultati
        .iter()
        .map(|risultato| {
            let callback = match risultato.identita() {
                IdentitaCatalogo::Alimento(id) => {
                    format!("dispensa:pick:alimento:{}:{id}", dove.token())
                }
                IdentitaCatalogo::Prodotto(id) => {
                    format!("dispensa:pick:prodotto:{}:{id}", dove.token())
                }
            };
            vec![button(risultato.etichetta(), callback)]
        })
        .collect();
    rows.push(vec![button(
        "📝 Scrivila a mano",
        format!("dispensa:add:libera:{}", dove.token()),
    )]);
    rows.push(annulla_row("dispensa:menu"));
    InlineKeyboardMarkup::new(rows)
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &DispensaSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    if data == "dispensa:noop" {
        return Ok(true);
    }
    if data == "dispensa:menu" {
        sessions.clear_chat(chat_id.0);
        mostra_menu(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    if data == "dispensa:auto" {
        let attuale = ingresso_automatico(pool).await;
        if let Err(errore) = imposta_ingresso_automatico(pool, !attuale).await {
            tracing::warn!(
                ?errore,
                "Salvataggio preferenza ingresso automatico fallito"
            );
        }
        let avviso = if attuale {
            "✅ Da ora la spesa chiusa non entra più da sola: la aggiungi tu."
        } else {
            "✅ Da ora la spesa chiusa entra da sola in 🧺 Dispensa."
        };
        mostra_menu(bot, chat_id, pool, Some(avviso)).await?;
        return Ok(true);
    }
    // Elenco di un luogo: "dispensa:luogo:{token}:{pagina}".
    if let Some(resto) = data.strip_prefix("dispensa:luogo:") {
        let mut parti = resto.split(':');
        let Some(dove) = parti.next().and_then(Conservazione::da_token) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let pagina = parti
            .next()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        mostra_luogo(bot, chat_id, pool, dove, pagina, None).await?;
        return Ok(true);
    }
    // La scelta del catalogo va letta prima delle voci più generiche che
    // iniziano con lo stesso prefisso: è lo stesso errore di ordine dei
    // callback già costato tre bug in `turni.rs` (sezione 2septies di
    // STATO.md).
    if let Some(resto) = data.strip_prefix("dispensa:pick:alimento:") {
        let mut parti = resto.split(':');
        let (Some(dove), Some(id)) = (
            parti.next().and_then(Conservazione::da_token),
            parti.next().and_then(|v| v.parse::<i64>().ok()),
        ) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match alimento_visibile_per_id(pool, id).await {
            Ok(Some((nome, unita_default))) => {
                sessions.set(
                    chat_id.0,
                    DispensaConversationState::AwaitingQuantita {
                        dove,
                        identita: Some(IdentitaCatalogo::Alimento(id)),
                        descrizione: nome.clone(),
                        unita_default: unita_default.clone(),
                    },
                );
                bot.send_message(chat_id, testo_quantita(&nome, unita_default.as_deref()))
                    .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
                        "dispensa:menu",
                    )]))
                    .await?;
            }
            _ => invalid(bot, chat_id).await?,
        }
        return Ok(true);
    }
    if let Some(resto) = data.strip_prefix("dispensa:pick:prodotto:") {
        let mut parti = resto.split(':');
        let (Some(dove), Some(id)) = (
            parti.next().and_then(Conservazione::da_token),
            parti.next().and_then(|v| v.parse::<i64>().ok()),
        ) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match prodotto_visibile_per_id(pool, id).await {
            Ok(Some((marca, nome_commerciale, unita_default))) => {
                let descrizione = format!("{marca} {nome_commerciale}");
                sessions.set(
                    chat_id.0,
                    DispensaConversationState::AwaitingQuantita {
                        dove,
                        identita: Some(IdentitaCatalogo::Prodotto(id)),
                        descrizione: descrizione.clone(),
                        unita_default: Some(unita_default.clone()),
                    },
                );
                bot.send_message(chat_id, testo_quantita(&descrizione, Some(&unita_default)))
                    .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
                        "dispensa:menu",
                    )]))
                    .await?;
            }
            _ => invalid(bot, chat_id).await?,
        }
        return Ok(true);
    }
    if let Some(token) = data.strip_prefix("dispensa:add:libera:") {
        let Some(dove) = Conservazione::da_token(token) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            DispensaConversationState::AwaitingDescrizione { dove },
        );
        bot.send_message(
            chat_id,
            "📝 Scrivi cosa hai (es. \"Passata di pomodoro\").".to_string(),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
            "dispensa:menu",
        )]))
        .await?;
        return Ok(true);
    }
    if let Some(token) = data.strip_prefix("dispensa:add:cerca:") {
        let Some(dove) = Conservazione::da_token(token) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            DispensaConversationState::AwaitingRicerca { dove },
        );
        bot.send_message(
            chat_id,
            "🔎 Scrivi il nome di un alimento (es. \"pasta\") o di un prodotto (es. \"de cecco\")."
                .to_string(),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(
            "dispensa:menu",
        )]))
        .await?;
        return Ok(true);
    }
    if let Some(token) = data.strip_prefix("dispensa:add:") {
        let Some(dove) = Conservazione::da_token(token) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        bot.send_message(
            chat_id,
            format!(
                "➕ Nuova scorta in {}\n\nCercala nel catalogo, così resta collegata all'alimento, oppure scrivila a mano.",
                dove.etichetta()
            ),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![button(
                "🔎 Cerca nel catalogo",
                format!("dispensa:add:cerca:{}", dove.token()),
            )],
            vec![button(
                "📝 Scrivila a mano",
                format!("dispensa:add:libera:{}", dove.token()),
            )],
            annulla_row(&format!("dispensa:luogo:{}:0", dove.token())),
        ]))
        .await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("dispensa:qty:") {
        let Some(scorta_id) = raw_id.parse::<i64>().ok().filter(|v| *v > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let Ok(Some(scorta)) = scorta_per_id(pool, scorta_id).await else {
            expired(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            DispensaConversationState::AwaitingQuantitaModifica {
                scorta_id,
                unita_attuale: scorta.unita_simbolo.clone(),
            },
        );
        bot.send_message(
            chat_id,
            format!(
                "✏️ {}\n\nScrivi la quantità che resta (es. \"{}\") -- uso \"{}\", oppure scrivi anche un'altra unità.",
                scorta.descrizione,
                formatta_quantita(scorta.quantita),
                scorta.unita_simbolo
            ),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row(&format!(
            "dispensa:scorta:{scorta_id}"
        ))]))
        .await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("dispensa:exp:none:") {
        let Some(scorta_id) = raw_id.parse::<i64>().ok().filter(|v| *v > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        if let Err(errore) = imposta_scadenza(pool, scorta_id, None).await {
            tracing::warn!(?errore, scorta_id, "Rimozione scadenza fallita");
        }
        mostra_scorta(bot, chat_id, pool, scorta_id, Some("✅ Scadenza tolta.")).await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("dispensa:exp:") {
        let Some(scorta_id) = raw_id.parse::<i64>().ok().filter(|v| *v > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            DispensaConversationState::AwaitingScadenza { scorta_id },
        );
        bot.send_message(
            chat_id,
            "📅 Scrivi la scadenza come 31/12/2026.\n\nLa scadenza è facoltativa: puoi anche toglierla.".to_string(),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![button(
                "🚫 Nessuna scadenza",
                format!("dispensa:exp:none:{scorta_id}"),
            )],
            annulla_row(&format!("dispensa:scorta:{scorta_id}")),
        ]))
        .await?;
        return Ok(true);
    }
    if let Some(resto) = data.strip_prefix("dispensa:move:to:") {
        let mut parti = resto.split(':');
        let (Some(scorta_id), Some(dove)) = (
            parti.next().and_then(|v| v.parse::<i64>().ok()),
            parti.next().and_then(Conservazione::da_token),
        ) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        if let Err(errore) = sposta_scorta(pool, scorta_id, dove).await {
            tracing::warn!(?errore, scorta_id, "Spostamento scorta fallito");
        }
        mostra_scorta(
            bot,
            chat_id,
            pool,
            scorta_id,
            Some(&format!("✅ Spostata in {}.", dove.etichetta())),
        )
        .await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("dispensa:move:") {
        let Some(scorta_id) = raw_id.parse::<i64>().ok().filter(|v| *v > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let Ok(Some(scorta)) = scorta_per_id(pool, scorta_id).await else {
            expired(bot, chat_id).await?;
            return Ok(true);
        };
        let attuale = Conservazione::da_token(&scorta.conservazione);
        let mut rows: Vec<Vec<InlineKeyboardButton>> = Conservazione::TUTTE
            .iter()
            .filter(|dove| Some(**dove) != attuale)
            .map(|dove| {
                vec![button(
                    dove.etichetta(),
                    format!("dispensa:move:to:{scorta_id}:{}", dove.token()),
                )]
            })
            .collect();
        rows.push(annulla_row(&format!("dispensa:scorta:{scorta_id}")));
        bot.send_message(
            chat_id,
            format!("🔀 {}\n\nDove la sposti?", scorta.descrizione),
        )
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
        return Ok(true);
    }
    // C16: un'eliminazione definitiva chiede sempre conferma esplicita.
    if let Some(raw_id) = data.strip_prefix("dispensa:del:ask:") {
        let Some(scorta_id) = raw_id.parse::<i64>().ok().filter(|v| *v > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        bot.send_message(
            chat_id,
            "⚠️ Eliminare questa scorta definitivamente? Non si può recuperare.".to_string(),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![button(
                "✅ Sì, elimina",
                format!("dispensa:del:yes:{scorta_id}"),
            )],
            annulla_row(&format!("dispensa:scorta:{scorta_id}")),
        ]))
        .await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("dispensa:del:yes:") {
        let Some(scorta_id) = raw_id.parse::<i64>().ok().filter(|v| *v > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let dove = scorta_per_id(pool, scorta_id)
            .await
            .ok()
            .flatten()
            .and_then(|scorta| Conservazione::da_token(&scorta.conservazione))
            .unwrap_or(Conservazione::Dispensa);
        match rimuovi_scorta(pool, scorta_id).await {
            Ok(()) => {
                mostra_luogo(bot, chat_id, pool, dove, 0, Some("✅ Scorta eliminata.")).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, scorta_id, "Eliminazione scorta fallita");
                mostra_luogo(
                    bot,
                    chat_id,
                    pool,
                    dove,
                    0,
                    Some("⚠️ Non riesco a eliminare la scorta."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("dispensa:scorta:") {
        let Some(scorta_id) = raw_id.parse::<i64>().ok().filter(|v| *v > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        mostra_scorta(bot, chat_id, pool, scorta_id, None).await?;
        return Ok(true);
    }

    Ok(false)
}

fn testo_quantita(descrizione: &str, unita_default: Option<&str>) -> String {
    match unita_default {
        Some(unita) => format!(
            "➕ {descrizione}\n\nScrivi solo la quantità (es. \"500\") -- uso l'unità predefinita \"{unita}\", oppure scrivi anche l'unità (es. \"2 pz\") per usarne un'altra."
        ),
        None => format!("➕ {descrizione}\n\nScrivi quantità e unità (es. \"500 g\")."),
    }
}

/// Menù delle scorte: i tre luoghi con il loro conteggio (C7), più la
/// preferenza sull'ingresso automatico dalla spesa.
async fn mostra_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str("🥫 Scorte\n\nQuello che hai già in casa.");

    let automatico = ingresso_automatico(pool).await;
    testo.push_str(if automatico {
        "\nChiudendo la spesa, la roba comprata entra da sola in 🧺 Dispensa."
    } else {
        "\nChiudendo la spesa, la roba comprata non entra da sola: la aggiungi tu."
    });

    let mut rows = Vec::new();
    for dove in Conservazione::TUTTE {
        let totale = conta_scorte(pool, dove).await.unwrap_or(0);
        rows.push(vec![button(
            liste::etichetta_con_conteggio(dove.etichetta(), totale),
            format!("dispensa:luogo:{}:0", dove.token()),
        )]);
    }
    rows.push(vec![button(
        if automatico {
            "⚙️ Ingresso automatico: attivo"
        } else {
            "⚙️ Ingresso automatico: spento"
        },
        "dispensa:auto",
    )]);
    rows.push(nav_row("food:menu"));

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Elenco paginato di un luogo di conservazione.
async fn mostra_luogo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    dove: Conservazione,
    pagina: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let totale = conta_scorte(pool, dove).await.unwrap_or(0);
    let pagina = liste::pagina_valida(pagina, totale);
    let scorte = elenco_scorte(pool, dove, pagina).await.unwrap_or_default();

    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str(&liste::intestazione(dove.etichetta(), totale, pagina));
    if totale == 0 {
        // C8: una schermata vuota dice cosa fare, non ripete "0".
        testo.push_str("\n\nNon c'è ancora niente qui.\nAggiungi la prima scorta.");
    } else {
        testo.push_str("\n\nPrima quelle che scadono.");
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = scorte
        .iter()
        .map(|scorta| {
            vec![button(
                etichetta_scorta(
                    &scorta.descrizione,
                    scorta.quantita,
                    &scorta.unita_simbolo,
                    scorta.scadenza.as_deref(),
                ),
                format!("dispensa:scorta:{}", scorta.id),
            )]
        })
        .collect();
    if let Some(riga) = liste::riga_paginazione_da_totale(pagina, totale, "dispensa:noop", |p| {
        format!("dispensa:luogo:{}:{p}", dove.token())
    }) {
        rows.push(riga);
    }
    rows.push(vec![button(
        "➕ Nuova scorta",
        format!("dispensa:add:{}", dove.token()),
    )]);
    rows.push(nav_row("dispensa:menu"));

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Dettaglio di una scorta: quantità, scadenza, dov'è, e cosa farci.
async fn mostra_scorta(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    scorta_id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let scorta = match scorta_per_id(pool, scorta_id).await {
        Ok(Some(scorta)) => scorta,
        _ => {
            expired(bot, chat_id).await?;
            return Ok(());
        }
    };
    let dove = Conservazione::da_token(&scorta.conservazione).unwrap_or(Conservazione::Dispensa);

    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str(&format!(
        "{}\n\n{} · {} {}\n{}",
        dove.etichetta(),
        scorta.descrizione,
        formatta_quantita(scorta.quantita),
        scorta.unita_simbolo,
        match scorta.scadenza.as_deref() {
            Some(data) => format!("📅 Scade il {}", calendario::display_date(data)),
            None => "📅 Nessuna scadenza".to_string(),
        }
    ));

    let rows = vec![
        vec![button("✏️ Quantità", format!("dispensa:qty:{scorta_id}"))],
        vec![button("📅 Scadenza", format!("dispensa:exp:{scorta_id}"))],
        vec![button("🔀 Sposta", format!("dispensa:move:{scorta_id}"))],
        vec![button("🗑 Elimina", format!("dispensa:del:ask:{scorta_id}"))],
        nav_row(&format!("dispensa:luogo:{}:0", dove.token())),
    ];

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn scadenza_accetta_i_due_formati_e_rifiuta_una_data_impossibile() {
        assert_eq!(
            interpreta_scadenza("31/12/2026"),
            Some("2026-12-31".to_string())
        );
        assert_eq!(
            interpreta_scadenza("2026-12-31"),
            Some("2026-12-31".to_string())
        );
        assert_eq!(
            interpreta_scadenza("1/2/2026"),
            Some("2026-02-01".to_string())
        );
        // Semantica, non solo forma: il 30 febbraio non esiste.
        assert_eq!(interpreta_scadenza("30/02/2026"), None);
        assert_eq!(interpreta_scadenza("domani"), None);
    }

    #[test]
    fn etichetta_manda_a_capo_la_scadenza() {
        // C15: la parte opzionale va a capo, mai accodata con " · ".
        let con = etichetta_scorta("Pollo", 800.0, "g", Some("2026-12-31"));
        assert_eq!(con, "Pollo · 800 g\n📅 scade il 31/12/2026");
        let senza = etichetta_scorta("Pollo", 800.0, "g", None);
        assert_eq!(senza, "Pollo · 800 g");
    }

    #[test]
    fn token_e_etichette_dei_tre_luoghi_restano_stabili() {
        for dove in Conservazione::TUTTE {
            assert_eq!(Conservazione::da_token(dove.token()), Some(dove));
        }
        assert_eq!(Conservazione::da_token("armadio"), None);
    }

    fn actor(user_id: i64, space_id: i64) -> crate::identity::AuditActor {
        crate::identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: "Alessio".to_string(),
            spazio_id: space_id,
            spazio_nome_snapshot: format!("Spazio {space_id}"),
            view_all: false,
            origine: "telegram",
            telegram_user_id: Some(user_id),
            telegram_username: None,
        }
    }

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

    async fn utente_e_spazio(pool: &SqlitePool) -> (i64, i64) {
        let user_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Alessio')")
            .execute(pool)
            .await
            .expect("utente")
            .last_insert_rowid();
        let space_id = sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Casa', 'condiviso')")
            .execute(pool)
            .await
            .expect("spazio")
            .last_insert_rowid();
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
        )
        .bind(space_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("membership");
        sqlx::query("INSERT INTO preferenze_utente (utente_id, spazio_attivo_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(space_id)
            .execute(pool)
            .await
            .expect("preferenze");
        (user_id, space_id)
    }

    #[tokio::test]
    async fn una_scorta_si_aggiunge_si_sposta_e_si_elimina() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let id = aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                None,
                "Passata di pomodoro",
                700.0,
                "g",
            )
            .await
            .expect("scorta");

            assert_eq!(
                conta_scorte(&pool, Conservazione::Dispensa)
                    .await
                    .expect("conteggio"),
                1
            );

            sposta_scorta(&pool, id, Conservazione::Freezer)
                .await
                .expect("spostamento");
            assert_eq!(
                conta_scorte(&pool, Conservazione::Dispensa)
                    .await
                    .expect("conteggio dispensa"),
                0
            );
            assert_eq!(
                conta_scorte(&pool, Conservazione::Freezer)
                    .await
                    .expect("conteggio freezer"),
                1
            );

            imposta_scadenza(&pool, id, Some("2026-12-31"))
                .await
                .expect("scadenza");
            let scorta = scorta_per_id(&pool, id)
                .await
                .expect("rilettura")
                .expect("presente");
            assert_eq!(scorta.scadenza.as_deref(), Some("2026-12-31"));

            imposta_scadenza(&pool, id, None).await.expect("senza");
            let scorta = scorta_per_id(&pool, id)
                .await
                .expect("rilettura")
                .expect("presente");
            assert_eq!(scorta.scadenza, None);

            rimuovi_scorta(&pool, id).await.expect("eliminazione");
            assert!(scorta_per_id(&pool, id).await.expect("rilettura").is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn le_scorte_in_scadenza_vengono_prima() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let senza = aggiungi_scorta(&pool, Conservazione::Frigo, None, "Burro", 250.0, "g")
                .await
                .expect("senza scadenza");
            let tardi = aggiungi_scorta(&pool, Conservazione::Frigo, None, "Yogurt", 4.0, "pz")
                .await
                .expect("scadenza lontana");
            let presto = aggiungi_scorta(&pool, Conservazione::Frigo, None, "Latte", 1.0, "l")
                .await
                .expect("scadenza vicina");
            imposta_scadenza(&pool, tardi, Some("2026-12-31"))
                .await
                .expect("scadenza");
            imposta_scadenza(&pool, presto, Some("2026-09-20"))
                .await
                .expect("scadenza");

            let elenco = elenco_scorte(&pool, Conservazione::Frigo, 0)
                .await
                .expect("elenco");
            let ordine: Vec<i64> = elenco.iter().map(|scorta| scorta.id).collect();
            assert_eq!(ordine, vec![presto, tardi, senza]);
        })
        .await;
    }

    #[tokio::test]
    async fn la_preferenza_dell_ingresso_automatico_parte_accesa_e_si_spegne() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            assert!(ingresso_automatico(&pool).await, "di default è acceso");
            imposta_ingresso_automatico(&pool, false)
                .await
                .expect("spegnimento");
            assert!(!ingresso_automatico(&pool).await);
            imposta_ingresso_automatico(&pool, true)
                .await
                .expect("riaccensione");
            assert!(ingresso_automatico(&pool).await);
        })
        .await;
    }
}
