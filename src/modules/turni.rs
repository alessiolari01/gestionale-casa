//! Turni e routine: prima fetta (10 settembre 2026), decisa con Alessio l'8
//! settembre 2026 come prossimo blocco della sequenza Alimentazione dopo la
//! lista della spesa (`docs/previsto/turni-e-routine.md`), con il design di
//! dettaglio del 10 settembre 2026.
//!
//! Un **modello** (`turno_modelli`/`turno_modello_pasti`) descrive una
//! giornata tipo (es. "Chiusura", "Università") con i suoi pasti di
//! default. **Assegnare** un modello a una data per un profilo alimentare
//! (`turno_assegnazioni`/`turno_assegnazione_pasti`) ne COPIA i pasti in
//! quel momento: modificare o rimuovere un pasto assegnato non tocca mai il
//! modello, e modificare il modello dopo non cambia le assegnazioni già
//! fatte -- lo stesso principio di snapshot già usato da
//! `planner_alimentare.rs` e `lista_spesa.rs`.
//!
//! Il modulo è diviso in tre parti, come gli altri due: dominio puro
//! (validazione orario/nome/nota, calcolo di cosa manca ancora da
//! pianificare per un giorno con turno assegnato -- testato senza
//! database), funzioni database (`sqlite::memory:` nei test), poi la UI
//! Telegram.
//!
//! **Fuori scope per questa fetta** (vedi `docs/moduli/turni-e-routine.md`):
//! condivisione/copia di un modello, invio a un altro utente, reminder alla
//! creazione (l'infrastruttura non esiste ancora), riordino dei pasti di un
//! modello, badge "🆕" di `novita::REGISTRO`.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::{Arc, Mutex, OnceLock},
};

use anyhow::Context as _;
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::modules::{calendario, liste, planner_alimentare::MealType};

type Bot = crate::context_bot::ContextBot;

// ===========================================================================
// Dominio puro: validazione e calcolo di cosa manca da pianificare.
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situazione {
    Casa,
    Lavoro,
    Fuori,
    Saltato,
    Altro,
}

impl Situazione {
    pub fn token(self) -> &'static str {
        match self {
            Self::Casa => "casa",
            Self::Lavoro => "lavoro",
            Self::Fuori => "fuori",
            Self::Saltato => "saltato",
            Self::Altro => "altro",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Casa => "Casa",
            Self::Lavoro => "Lavoro",
            Self::Fuori => "Fuori",
            Self::Saltato => "Saltato",
            Self::Altro => "Altro",
        }
    }

    /// Icona della situazione. `⏭` per "saltato" riusa lo stesso simbolo
    /// che C4 riserva altrove a "saltato" -- stesso significato, stessa
    /// icona.
    pub fn emoji(self) -> &'static str {
        match self {
            Self::Casa => "🏡",
            Self::Lavoro => "💼",
            Self::Fuori => "🚶",
            Self::Saltato => "⏭",
            Self::Altro => "➖",
        }
    }

    pub fn from_token(value: &str) -> Option<Self> {
        match value {
            "casa" => Some(Self::Casa),
            "lavoro" => Some(Self::Lavoro),
            "fuori" => Some(Self::Fuori),
            "saltato" => Some(Self::Saltato),
            "altro" => Some(Self::Altro),
            _ => None,
        }
    }

    pub const TUTTE: [Situazione; 5] = [
        Situazione::Casa,
        Situazione::Lavoro,
        Situazione::Fuori,
        Situazione::Saltato,
        Situazione::Altro,
    ];
}

/// Icona del tipo di pasto, stesse icone di `planner_alimentare::MealType`
/// (privata lì, non riusabile da qui: piccola duplicazione accettata,
/// stesso principio già scelto in `lista_spesa::clausola_visibilita_alimento`
/// per non forzare un accoppiamento fra moduli).
fn meal_emoji(meal: MealType) -> &'static str {
    match meal {
        MealType::Breakfast => "☕",
        MealType::MorningSnack => "🍎",
        MealType::Lunch => "🍝",
        MealType::AfternoonSnack => "🥪",
        MealType::Dinner => "🍽️",
        MealType::Other => "🍴",
    }
}

const NOME_MODELLO_MAX_CARATTERI: usize = 60;
const NOTA_MAX_CARATTERI: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NomeModelloError {
    Vuoto,
    TroppoLungo,
}

impl fmt::Display for NomeModelloError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let messaggio = match self {
            Self::Vuoto => "Scrivi un nome, non può essere vuoto.",
            Self::TroppoLungo => "Il nome è troppo lungo (massimo 60 caratteri).",
        };
        f.write_str(messaggio)
    }
}
impl Error for NomeModelloError {}

/// Valida il nome di un modello turno (es. "Chiusura", "Università").
pub fn valida_nome_modello(testo: &str) -> Result<String, NomeModelloError> {
    let testo = testo.trim();
    if testo.is_empty() {
        return Err(NomeModelloError::Vuoto);
    }
    if testo.chars().count() > NOME_MODELLO_MAX_CARATTERI {
        return Err(NomeModelloError::TroppoLungo);
    }
    Ok(testo.to_string())
}

pub fn normalizza_nome(valore: &str) -> String {
    valore.trim().to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotaTroppoLunga;

impl fmt::Display for NotaTroppoLunga {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Il testo è troppo lungo (massimo {NOTA_MAX_CARATTERI} caratteri)."
        )
    }
}
impl Error for NotaTroppoLunga {}

/// Valida una nota libera opzionale (nota del pasto, nota di preparazione):
/// una stringa vuota o solo spazi diventa `None`, non un errore -- "salta" è
/// sempre una scelta valida.
pub fn valida_nota_opzionale(testo: &str) -> Result<Option<String>, NotaTroppoLunga> {
    let testo = testo.trim();
    if testo.is_empty() {
        return Ok(None);
    }
    if testo.chars().count() > NOTA_MAX_CARATTERI {
        return Err(NotaTroppoLunga);
    }
    Ok(Some(testo.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrarioNonValido;

impl fmt::Display for OrarioNonValido {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Orario non valido. Scrivilo nel formato 24 ore HH:MM, es. 12:30.")
    }
}
impl Error for OrarioNonValido {}

/// Come `spazi_membri::valid_time_strict`, reimplementata qui: quella
/// funzione è privata al suo modulo e la validazione dell'orario è
/// abbastanza piccola da non giustificare un accoppiamento fra moduli per
/// riusarla (stessa scelta fatta altrove nel progetto, vedi il commento su
/// `meal_emoji`).
pub fn valid_time_strict(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return false;
    }
    if !bytes.iter().enumerate().all(|(index, byte)| {
        if index == 2 {
            true
        } else {
            byte.is_ascii_digit()
        }
    }) {
        return false;
    }
    let ore: u32 = value[..2].parse().unwrap_or(99);
    let minuti: u32 = value[3..5].parse().unwrap_or(99);
    ore < 24 && minuti < 60
}

/// Valida un orario scritto a mano nel formato HH:MM.
pub fn valida_orario(testo: &str) -> Result<String, OrarioNonValido> {
    let testo = testo.trim();
    if valid_time_strict(testo) {
        Ok(testo.to_string())
    } else {
        Err(OrarioNonValido)
    }
}

/// Un pasto di un turno assegnato, come serve al calcolo di cosa manca
/// ancora da pianificare -- dominio puro, senza dipendere dalle righe del
/// database.
#[derive(Debug, Clone, PartialEq)]
pub struct PastoRoutine {
    pub profilo_nome: String,
    pub tipo_pasto: String,
    pub orario: Option<String>,
    pub situazione: Situazione,
    pub preparazione_anticipata: bool,
}

/// I pasti del turno assegnato che NON hanno ancora un pasto vero
/// pianificato nel planner per lo stesso tipo, in quel giorno.
///
/// Il planner mostra solo un riferimento testuale (mai un pasto scritto in
/// automatico, vedi il modulo): una volta che l'utente ha davvero
/// pianificato "pranzo" per quel giorno, il suggerimento del turno per
/// "pranzo" non serve più e sparisce da solo.
pub fn pasti_da_segnalare(
    assegnati: &[PastoRoutine],
    tipi_gia_pianificati: &HashSet<String>,
) -> Vec<PastoRoutine> {
    assegnati
        .iter()
        .filter(|pasto| !tipi_gia_pianificati.contains(&pasto.tipo_pasto))
        .cloned()
        .collect()
}

fn formatta_riga_routine(pasto: &PastoRoutine) -> String {
    let tipo = MealType::from_token(&pasto.tipo_pasto)
        .map(|meal| format!("{} {}", meal_emoji(meal), meal.label()))
        .unwrap_or_else(|| "🍴 Pasto".to_string());
    let orario = pasto
        .orario
        .as_deref()
        .map(|o| format!(" {o}"))
        .unwrap_or_default();
    let prep = if pasto.preparazione_anticipata {
        ", da preparare prima"
    } else {
        ""
    };
    format!(
        "🔸 {}: {tipo}{orario} ({}{prep})",
        pasto.profilo_nome,
        pasto.situazione.label().to_lowercase()
    )
}

/// Il blocco testuale da anteporre alla schermata "Giorno" del planner,
/// `None` se non c'è nulla da segnalare (nessun turno assegnato quel
/// giorno, o tutti i suoi pasti sono già pianificati per davvero).
pub fn formatta_blocco_routine(righe: &[PastoRoutine]) -> Option<String> {
    if righe.is_empty() {
        return None;
    }
    let mut testo = String::from("📋 Turno assegnato:");
    for riga in righe {
        testo.push('\n');
        testo.push_str(&formatta_riga_routine(riga));
    }
    Some(testo)
}

#[cfg(test)]
mod domain_tests {
    use super::*;

    #[test]
    fn nome_modello_valido_e_rifiuta_vuoto() {
        assert_eq!(
            valida_nome_modello("  Chiusura  "),
            Ok("Chiusura".to_string())
        );
        assert_eq!(valida_nome_modello("   "), Err(NomeModelloError::Vuoto));
        assert_eq!(
            valida_nome_modello(&"x".repeat(61)),
            Err(NomeModelloError::TroppoLungo)
        );
    }

    #[test]
    fn nota_vuota_diventa_none_non_errore() {
        assert_eq!(valida_nota_opzionale("   "), Ok(None));
        assert_eq!(
            valida_nota_opzionale(" Pronta in frigo "),
            Ok(Some("Pronta in frigo".to_string()))
        );
        assert_eq!(
            valida_nota_opzionale(&"x".repeat(201)),
            Err(NotaTroppoLunga)
        );
    }

    #[test]
    fn orario_accetta_solo_hh_mm_valido() {
        assert_eq!(valida_orario("07:30"), Ok("07:30".to_string()));
        assert_eq!(valida_orario("23:59"), Ok("23:59".to_string()));
        for invalido in ["24:00", "12:60", "7:30", "12:3", "abcde", "", "12-30"] {
            assert_eq!(valida_orario(invalido), Err(OrarioNonValido), "{invalido}");
        }
    }

    #[test]
    fn situazione_token_andata_e_ritorno() {
        for s in Situazione::TUTTE {
            assert_eq!(Situazione::from_token(s.token()), Some(s));
        }
        assert_eq!(Situazione::from_token("altrove"), None);
    }

    fn pasto(tipo: &str, prep: bool) -> PastoRoutine {
        PastoRoutine {
            profilo_nome: "Alessio".to_string(),
            tipo_pasto: tipo.to_string(),
            orario: Some("12:00".to_string()),
            situazione: Situazione::Casa,
            preparazione_anticipata: prep,
        }
    }

    #[test]
    fn segnala_solo_i_tipi_non_ancora_pianificati() {
        let assegnati = vec![pasto("pranzo", false), pasto("cena", true)];
        let mut pianificati = HashSet::new();
        pianificati.insert("pranzo".to_string());
        let mancanti = pasti_da_segnalare(&assegnati, &pianificati);
        assert_eq!(mancanti.len(), 1);
        assert_eq!(mancanti[0].tipo_pasto, "cena");
    }

    #[test]
    fn nessun_pasto_mancante_non_produce_blocco() {
        let assegnati = vec![pasto("pranzo", false)];
        let mut pianificati = HashSet::new();
        pianificati.insert("pranzo".to_string());
        let mancanti = pasti_da_segnalare(&assegnati, &pianificati);
        assert!(formatta_blocco_routine(&mancanti).is_none());
    }

    #[test]
    fn blocco_elenca_ogni_pasto_mancante_con_profilo_orario_e_preparazione() {
        let mancanti = vec![pasto("cena", true)];
        let blocco = formatta_blocco_routine(&mancanti).unwrap();
        assert!(blocco.starts_with("📋 Turno assegnato:"));
        assert!(blocco.contains("Alessio"));
        assert!(blocco.contains("12:00"));
        assert!(blocco.contains("da preparare prima"));
    }
}

// ===========================================================================
// Database (`sqlite::memory:` nei test).
// ===========================================================================

#[derive(Debug, Clone, FromRow)]
pub struct TurnoModello {
    pub id: i64,
    pub nome: String,
    #[allow(dead_code)]
    pub archiviato: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct PastoModelloRow {
    pub id: i64,
    #[allow(dead_code)]
    pub modello_id: i64,
    pub tipo_pasto: String,
    pub orario: Option<String>,
    pub situazione: String,
    pub preparazione_anticipata: i64,
    pub preparazione_note: Option<String>,
    pub nota: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AssegnazioneRow {
    pub id: i64,
    pub modello_nome_snapshot: String,
    pub profilo_nome_snapshot: String,
    pub data: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct PastoAssegnatoRow {
    pub id: i64,
    pub assegnazione_id: i64,
    pub tipo_pasto: String,
    pub orario: Option<String>,
    pub situazione: String,
    pub preparazione_anticipata: i64,
    pub preparazione_note: Option<String>,
    pub nota: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProfiloScelta {
    pub id: i64,
    pub name: String,
}

/// Ambito di visibilità di `turno_modelli` e `turno_assegnazioni`: lo
/// stesso spazio dell'attore corrente. `AuditActor::spazio_id` non è mai
/// nullo (ogni utente ha sempre uno spazio predefinito, `LEGACY_SPACE_ID`
/// per chi non ne ha ancora scelto uno proprio), quindi un confronto
/// diretto basta -- a differenza di `lista_spesa::righe_da_aggregare`, che
/// confronta con lo `spazio_id` *salvato su una riga* (quello sì
/// nullable).
pub async fn conta_modelli(pool: &SqlitePool) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    sqlx::query_scalar("SELECT COUNT(*) FROM turno_modelli WHERE archiviato = 0 AND spazio_id = ?")
        .bind(actor.spazio_id)
        .fetch_one(pool)
        .await
        .context("Impossibile contare i modelli turno")
}

pub async fn lista_modelli_pagina(
    pool: &SqlitePool,
    page: i64,
) -> anyhow::Result<Vec<TurnoModello>> {
    let actor = crate::identity::current_actor();
    sqlx::query_as(
        "SELECT id, nome, archiviato FROM turno_modelli \
         WHERE archiviato = 0 AND spazio_id = ? \
         ORDER BY nome COLLATE NOCASE, id LIMIT ? OFFSET ?",
    )
    .bind(actor.spazio_id)
    .bind(liste::VOCI_PER_PAGINA as i64)
    .bind(page.max(0) * liste::VOCI_PER_PAGINA as i64)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i modelli turno")
}

pub async fn trova_modello(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<TurnoModello>> {
    sqlx::query_as("SELECT id, nome, archiviato FROM turno_modelli WHERE id = ? AND archiviato = 0")
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Impossibile rileggere il modello turno")
}

pub async fn crea_modello(pool: &SqlitePool, nome: &str) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let id = sqlx::query(
        "INSERT INTO turno_modelli \
         (proprietario_utente_id, spazio_id, nome, nome_normalizzato) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(actor.spazio_id)
    .bind(nome)
    .bind(normalizza_nome(nome))
    .execute(pool)
    .await
    .context("Impossibile creare il modello turno")?
    .last_insert_rowid();
    Ok(id)
}

pub async fn rinomina_modello(pool: &SqlitePool, id: i64, nome: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE turno_modelli SET nome = ?, nome_normalizzato = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(nome)
    .bind(normalizza_nome(nome))
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile rinominare il modello turno")?;
    Ok(())
}

pub async fn archivia_modello(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE turno_modelli SET archiviato = 1, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile archiviare il modello turno")?;
    Ok(())
}

async fn prossimo_ordinamento_modello(pool: &SqlitePool, modello_id: i64) -> anyhow::Result<i64> {
    let massimo: Option<i64> =
        sqlx::query_scalar("SELECT MAX(ordinamento) FROM turno_modello_pasti WHERE modello_id = ?")
            .bind(modello_id)
            .fetch_one(pool)
            .await
            .context("Impossibile leggere l'ordinamento massimo")?;
    Ok(massimo.unwrap_or(0) + 1)
}

#[allow(clippy::too_many_arguments)]
pub async fn aggiungi_pasto_modello(
    pool: &SqlitePool,
    modello_id: i64,
    tipo_pasto: &str,
    orario: Option<&str>,
    situazione: &str,
    preparazione_anticipata: bool,
    preparazione_note: Option<&str>,
    nota: Option<&str>,
) -> anyhow::Result<i64> {
    let ordinamento = prossimo_ordinamento_modello(pool, modello_id).await?;
    let id = sqlx::query(
        "INSERT INTO turno_modello_pasti \
         (modello_id, tipo_pasto, orario, situazione, preparazione_anticipata, \
          preparazione_note, nota, ordinamento) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(modello_id)
    .bind(tipo_pasto)
    .bind(orario)
    .bind(situazione)
    .bind(preparazione_anticipata)
    .bind(preparazione_note)
    .bind(nota)
    .bind(ordinamento)
    .execute(pool)
    .await
    .context("Impossibile aggiungere il pasto al modello")?
    .last_insert_rowid();
    Ok(id)
}

pub async fn lista_pasti_modello(
    pool: &SqlitePool,
    modello_id: i64,
) -> anyhow::Result<Vec<PastoModelloRow>> {
    sqlx::query_as(
        "SELECT id, modello_id, tipo_pasto, orario, situazione, preparazione_anticipata, \
                preparazione_note, nota \
         FROM turno_modello_pasti WHERE modello_id = ? ORDER BY ordinamento, id",
    )
    .bind(modello_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i pasti del modello")
}

pub async fn trova_pasto_modello(
    pool: &SqlitePool,
    id: i64,
) -> anyhow::Result<Option<PastoModelloRow>> {
    sqlx::query_as(
        "SELECT id, modello_id, tipo_pasto, orario, situazione, preparazione_anticipata, \
                preparazione_note, nota \
         FROM turno_modello_pasti WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere il pasto del modello")
}

pub async fn rimuovi_pasto_modello(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM turno_modello_pasti WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Impossibile rimuovere il pasto dal modello")?;
    Ok(())
}

pub async fn conta_profili_visibili(pool: &SqlitePool) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let view_all = i64::from(actor.view_all);
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM profili_alimentari pa WHERE pa.archiviato = 0 AND \
         (pa.gestore_utente_id = ? OR pa.utente_collegato_id = ? OR EXISTS (\
           SELECT 1 FROM profilo_alimentare_spazi pas JOIN membri_spazio ms ON ms.spazio_id = pas.spazio_id \
           WHERE pas.profilo_alimentare_id = pa.id AND ms.utente_id = ? AND (? = 1 OR pas.spazio_id = ?)))",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .fetch_one(pool)
    .await
    .context("Impossibile contare i profili")
}

pub async fn profili_visibili_pagina(
    pool: &SqlitePool,
    page: i64,
) -> anyhow::Result<Vec<ProfiloScelta>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let view_all = i64::from(actor.view_all);
    sqlx::query_as(
        "SELECT pa.id, pa.nome AS name FROM profili_alimentari pa WHERE pa.archiviato = 0 AND \
         (pa.gestore_utente_id = ? OR pa.utente_collegato_id = ? OR EXISTS (\
           SELECT 1 FROM profilo_alimentare_spazi pas JOIN membri_spazio ms ON ms.spazio_id = pas.spazio_id \
           WHERE pas.profilo_alimentare_id = pa.id AND ms.utente_id = ? AND (? = 1 OR pas.spazio_id = ?))) \
         ORDER BY pa.nome_normalizzato, pa.id LIMIT ? OFFSET ?",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .bind(liste::VOCI_PER_PAGINA as i64)
    .bind(page.max(0) * liste::VOCI_PER_PAGINA as i64)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i profili")
}

pub async fn trova_profilo_visibile(
    pool: &SqlitePool,
    id: i64,
) -> anyhow::Result<Option<ProfiloScelta>> {
    sqlx::query_as(
        "SELECT id, nome AS name FROM profili_alimentari WHERE id = ? AND archiviato = 0",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Impossibile verificare il profilo")
}

pub async fn trova_assegnazione_profilo_data(
    pool: &SqlitePool,
    profilo_id: i64,
    data: &str,
) -> anyhow::Result<Option<AssegnazioneRow>> {
    sqlx::query_as(
        "SELECT id, modello_nome_snapshot, profilo_nome_snapshot, data \
         FROM turno_assegnazioni WHERE profilo_alimentare_id = ? AND data = ?",
    )
    .bind(profilo_id)
    .bind(data)
    .fetch_optional(pool)
    .await
    .context("Impossibile cercare l'assegnazione")
}

pub async fn trova_assegnazione(
    pool: &SqlitePool,
    id: i64,
) -> anyhow::Result<Option<AssegnazioneRow>> {
    sqlx::query_as(
        "SELECT id, modello_nome_snapshot, profilo_nome_snapshot, data \
         FROM turno_assegnazioni WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere l'assegnazione")
}

pub async fn elimina_assegnazione(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM turno_assegnazioni WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Impossibile eliminare l'assegnazione")?;
    Ok(())
}

/// Assegna un modello a una data per un profilo: COPIA i pasti del modello
/// in quel momento in `turno_assegnazione_pasti`. Non fa nulla per
/// eventuali assegnazioni preesistenti sullo stesso profilo/data -- chi
/// chiama deve aver già gestito il conflitto (vedi
/// `trova_assegnazione_profilo_data` + `elimina_assegnazione` nella UI).
pub async fn assegna_modello(
    pool: &SqlitePool,
    modello_id: i64,
    profilo_id: i64,
    data: &str,
) -> anyhow::Result<i64> {
    if !calendario::valid_date(data) {
        anyhow::bail!("Data non valida");
    }
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let modello = trova_modello(pool, modello_id)
        .await?
        .context("Modello turno non trovato")?;
    let profilo = trova_profilo_visibile(pool, profilo_id)
        .await?
        .context("Profilo non trovato")?;
    let pasti = lista_pasti_modello(pool, modello_id).await?;

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    let assegnazione_id = sqlx::query(
        "INSERT INTO turno_assegnazioni \
         (modello_id, modello_nome_snapshot, proprietario_utente_id, spazio_id, \
          profilo_alimentare_id, profilo_nome_snapshot, data) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(modello_id)
    .bind(&modello.nome)
    .bind(user_id)
    .bind(actor.spazio_id)
    .bind(profilo_id)
    .bind(&profilo.name)
    .bind(data)
    .execute(&mut *tx)
    .await
    .context("Impossibile creare l'assegnazione")?
    .last_insert_rowid();

    // `pasti` è già ordinata per `ordinamento, id` (vedi `lista_pasti_modello`):
    // l'indice della copia preserva quell'ordine nella nuova assegnazione,
    // indipendentemente dai valori originali di `ordinamento`.
    for (indice, pasto) in pasti.iter().enumerate() {
        sqlx::query(
            "INSERT INTO turno_assegnazione_pasti \
             (assegnazione_id, tipo_pasto, orario, situazione, preparazione_anticipata, \
              preparazione_note, nota, ordinamento) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(assegnazione_id)
        .bind(&pasto.tipo_pasto)
        .bind(&pasto.orario)
        .bind(&pasto.situazione)
        .bind(pasto.preparazione_anticipata)
        .bind(&pasto.preparazione_note)
        .bind(&pasto.nota)
        .bind(indice as i64)
        .execute(&mut *tx)
        .await
        .context("Impossibile copiare un pasto nell'assegnazione")?;
    }
    tx.commit()
        .await
        .context("Impossibile salvare l'assegnazione")?;
    Ok(assegnazione_id)
}

pub async fn lista_pasti_assegnati(
    pool: &SqlitePool,
    assegnazione_id: i64,
) -> anyhow::Result<Vec<PastoAssegnatoRow>> {
    sqlx::query_as(
        "SELECT id, assegnazione_id, tipo_pasto, orario, situazione, preparazione_anticipata, \
                preparazione_note, nota \
         FROM turno_assegnazione_pasti WHERE assegnazione_id = ? ORDER BY ordinamento, id",
    )
    .bind(assegnazione_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i pasti assegnati")
}

pub async fn trova_pasto_assegnato(
    pool: &SqlitePool,
    id: i64,
) -> anyhow::Result<Option<PastoAssegnatoRow>> {
    sqlx::query_as(
        "SELECT id, assegnazione_id, tipo_pasto, orario, situazione, preparazione_anticipata, \
                preparazione_note, nota \
         FROM turno_assegnazione_pasti WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere il pasto assegnato")
}

pub async fn modifica_situazione_pasto_assegnato(
    pool: &SqlitePool,
    id: i64,
    situazione: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE turno_assegnazione_pasti SET situazione = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(situazione)
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile cambiare la situazione")?;
    Ok(())
}

pub async fn modifica_orario_pasto_assegnato(
    pool: &SqlitePool,
    id: i64,
    orario: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE turno_assegnazione_pasti SET orario = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(orario)
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile cambiare l'orario")?;
    Ok(())
}

/// Cambia se il pasto assegnato va preparato in anticipo. Passare a `false`
/// azzera anche la nota di preparazione: il CHECK a database vieta una nota
/// senza `preparazione_anticipata = 1` (vedi la migration).
pub async fn imposta_preparazione_pasto_assegnato(
    pool: &SqlitePool,
    id: i64,
    anticipata: bool,
) -> anyhow::Result<()> {
    if anticipata {
        sqlx::query(
            "UPDATE turno_assegnazione_pasti SET preparazione_anticipata = 1, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
    } else {
        sqlx::query(
            "UPDATE turno_assegnazione_pasti SET preparazione_anticipata = 0, \
             preparazione_note = NULL, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
    }
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile cambiare la preparazione anticipata")?;
    Ok(())
}

pub async fn modifica_nota_preparazione_pasto_assegnato(
    pool: &SqlitePool,
    id: i64,
    nota: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE turno_assegnazione_pasti SET preparazione_note = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(nota)
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile cambiare la nota di preparazione")?;
    Ok(())
}

pub async fn modifica_nota_pasto_assegnato(
    pool: &SqlitePool,
    id: i64,
    nota: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE turno_assegnazione_pasti SET nota = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(nota)
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile cambiare la nota")?;
    Ok(())
}

pub async fn rimuovi_pasto_assegnato(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM turno_assegnazione_pasti WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Impossibile rimuovere il pasto assegnato")?;
    Ok(())
}

/// Le date (ISO) del mese che hanno già un'assegnazione per quel profilo,
/// per marcarle nel calendario (`•`, convenzione C13).
pub async fn giorni_con_assegnazione_nel_mese(
    pool: &SqlitePool,
    profilo_id: i64,
    anno: i32,
    mese: u32,
) -> anyhow::Result<HashSet<String>> {
    let Some(inizio) = calendario::month_start(anno, mese) else {
        return Ok(HashSet::new());
    };
    // Un margine di 31 giorni copre sempre l'intero mese (anche i più
    // lunghi): eventuali date del mese successivo incluse per eccesso non
    // creano problemi, perché il chiamante interroga l'insieme solo per le
    // date che appartengono davvero alla griglia del mese richiesto.
    let Some(fine) = calendario::shift_date(&inizio, 31) else {
        return Ok(HashSet::new());
    };
    let righe: Vec<(String,)> = sqlx::query_as(
        "SELECT data FROM turno_assegnazioni \
         WHERE profilo_alimentare_id = ? AND date(data) BETWEEN date(?) AND date(?)",
    )
    .bind(profilo_id)
    .bind(&inizio)
    .bind(&fine)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i giorni con assegnazione")?;
    Ok(righe.into_iter().map(|(data,)| data).collect())
}

/// Il blocco informativo da mostrare nella schermata "Giorno" del planner
/// (sola lettura, mai una scrittura automatica su `planner_pasti`): tutte
/// le assegnazioni turno di quel giorno nello stesso ambito (spazio o
/// proprietario) dell'attore corrente, filtrate ai pasti il cui tipo non è
/// già un pasto vero pianificato per quella stessa data.
///
/// `None` in ogni caso limite (nessuna assegnazione, errore di lettura):
/// un problema qui non deve mai bloccare l'apertura del planner.
pub async fn info_giorno_per_planner(pool: &SqlitePool, data: &str) -> Option<String> {
    let actor = crate::identity::current_actor();

    let tipi_pianificati: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT pp.tipo_pasto FROM planner_pasti pp \
         JOIN planner_alimentari p ON p.id = pp.planner_id \
         WHERE date(pp.data_pasto) = date(?1) AND p.spazio_id = ?2",
    )
    .bind(data)
    .bind(actor.spazio_id)
    .fetch_all(pool)
    .await
    .ok()?;
    let tipi_pianificati: HashSet<String> = tipi_pianificati.into_iter().map(|(t,)| t).collect();

    let righe: Vec<(String, String, Option<String>, String, i64)> = sqlx::query_as(
        "SELECT ta.profilo_nome_snapshot, tap.tipo_pasto, tap.orario, tap.situazione, \
                tap.preparazione_anticipata \
         FROM turno_assegnazioni ta \
         JOIN turno_assegnazione_pasti tap ON tap.assegnazione_id = ta.id \
         WHERE date(ta.data) = date(?1) AND ta.spazio_id = ?2 \
         ORDER BY ta.profilo_nome_snapshot, tap.ordinamento, tap.id",
    )
    .bind(data)
    .bind(actor.spazio_id)
    .fetch_all(pool)
    .await
    .ok()?;

    let assegnati: Vec<PastoRoutine> = righe
        .into_iter()
        .filter_map(|(profilo_nome, tipo_pasto, orario, situazione, prep)| {
            Some(PastoRoutine {
                profilo_nome,
                tipo_pasto,
                orario,
                situazione: Situazione::from_token(&situazione)?,
                preparazione_anticipata: prep != 0,
            })
        })
        .collect();

    let mancanti = pasti_da_segnalare(&assegnati, &tipi_pianificati);
    formatta_blocco_routine(&mancanti)
}

// ===========================================================================
// UI Telegram.
// ===========================================================================

#[derive(Clone, Default)]
pub struct TurniSessionStore {
    inner: Arc<Mutex<HashMap<i64, TurniConversationState>>>,
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
enum TurniConversationState {
    AwaitingNomeModello,
    AwaitingRinominaModello { modello_id: i64 },
    AwaitingOrarioPasto,
    AwaitingNotaPreparazione,
    AwaitingNotaLibera,
    AwaitingOrarioAssegnato { pasto_id: i64 },
    AwaitingNotaPreparazioneAssegnato { pasto_id: i64 },
    AwaitingNotaAssegnato { pasto_id: i64 },
}

impl TurniSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, chat_id: i64) -> Option<TurniConversationState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&chat_id)
            .cloned()
    }

    fn set(&self, chat_id: i64, state: TurniConversationState) {
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

/// Bozza di un pasto-modello in costruzione (flusso guidato ➕ Aggiungi
/// pasto): vive in una mappa a parte da `TurniSessionStore`, come
/// `lista_spesa::RANGE_DRAFTS`, perché serve anche nei passi guidati a
/// bottoni (tipo, situazione, preparazione sì/no), non solo in quelli di
/// testo libero.
#[derive(Debug, Clone)]
struct DraftPastoModello {
    modello_id: i64,
    tipo_pasto: MealType,
    orario: Option<String>,
    situazione: Option<Situazione>,
    preparazione_anticipata: Option<bool>,
    preparazione_note: Option<String>,
}

static PASTO_DRAFTS: OnceLock<Mutex<HashMap<i64, DraftPastoModello>>> = OnceLock::new();

fn pasto_drafts() -> &'static Mutex<HashMap<i64, DraftPastoModello>> {
    PASTO_DRAFTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn draft_set(chat_id: i64, draft: DraftPastoModello) {
    pasto_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(chat_id, draft);
}

fn draft_get(chat_id: i64) -> Option<DraftPastoModello> {
    pasto_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&chat_id)
        .cloned()
}

fn draft_clear(chat_id: i64) {
    pasto_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&chat_id);
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

fn annulla_markup(cancel_cb: impl Into<String>) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", cancel_cb.into()),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn skip_e_annulla_markup(
    skip_label: &str,
    skip_cb: impl Into<String>,
    cancel_cb: impl Into<String>,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(skip_label, skip_cb.into())],
        vec![
            button("❌ Annulla", cancel_cb.into()),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

async fn invalid(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "⚠️ Pulsante non valido o non più disponibile.")
        .reply_markup(nav_markup("turni:menu"))
        .await?;
    Ok(())
}

async fn expired(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "ℹ️ Questa operazione non è più attiva. Riapri la sezione Turni e routine.",
    )
    .reply_markup(nav_markup("turni:menu"))
    .await?;
    Ok(())
}

fn parse_ids2(rest: &str) -> Option<(i64, i64)> {
    let mut parti = rest.splitn(2, ':');
    let a = parti.next()?.parse().ok()?;
    let b = parti.next()?.parse().ok()?;
    Some((a, b))
}

fn parse_id_and_rest(rest: &str) -> Option<(i64, &str)> {
    let mut parti = rest.splitn(2, ':');
    let a = parti.next()?.parse().ok()?;
    let b = parti.next()?;
    Some((a, b))
}

// --- Schermate: elenco modelli --------------------------------------------

async fn show_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let totale = conta_modelli(pool).await.unwrap_or(0);
    let pagine = liste::totale_pagine(totale);
    let page = page.clamp(0, pagine - 1);
    let modelli = lista_modelli_pagina(pool, page).await.unwrap_or_default();

    let mut rows: Vec<Vec<InlineKeyboardButton>> = modelli
        .iter()
        .map(|modello| {
            vec![button(
                format!("📋 {}", modello.nome),
                format!("turni:model:{}", modello.id),
            )]
        })
        .collect();
    if let Some(riga) = liste::riga_paginazione(page, pagine, "turni:noop", |p| {
        format!("turni:menu:page:{p}")
    }) {
        rows.push(riga);
    }
    rows.push(vec![button("➕ Nuovo modello", "turni:model:new")]);
    rows.push(vec![button("📅 Vedi/modifica assegnazione", "turni:vedi")]);
    rows.push(nav_row("food:menu"));

    let mut testo = notice
        .map(|value| format!("{value}\n\n"))
        .unwrap_or_default();
    testo.push_str(&liste::intestazione("📋 Turni e routine", totale, page));
    if totale == 0 {
        testo.push_str("\n\nNessun modello turno creato. Un modello descrive una giornata tipo (es. \"Chiusura\", \"Università\") con i suoi pasti di default.");
    }

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

// --- Schermate: dettaglio modello ------------------------------------------

async fn show_model_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    modello_id: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let Some(modello) = trova_modello(pool, modello_id).await.unwrap_or(None) else {
        return invalid(bot, chat_id).await;
    };
    let pasti = lista_pasti_modello(pool, modello_id)
        .await
        .unwrap_or_default();

    let mut rows: Vec<Vec<InlineKeyboardButton>> = pasti
        .iter()
        .map(|pasto| {
            let tipo = MealType::from_token(&pasto.tipo_pasto)
                .map(|meal| format!("{} {}", meal_emoji(meal), meal.label()))
                .unwrap_or_else(|| "🍴 Pasto".to_string());
            let situazione = Situazione::from_token(&pasto.situazione)
                .map(|s| format!("{} {}", s.emoji(), s.label()))
                .unwrap_or_default();
            let orario = pasto.orario.clone().unwrap_or_else(|| "--:--".to_string());
            // C15: più di due parti aggiunte (orario, situazione, eventuale
            // preparazione) vanno a capo, non accodate con "·".
            let mut etichetta = format!("{tipo}\n{orario} · {situazione}");
            if pasto.preparazione_anticipata != 0 {
                etichetta.push_str("\n🧺 Da preparare prima");
            }
            vec![button(
                etichetta,
                format!("turni:pasto:remove:{}", pasto.id),
            )]
        })
        .collect();

    rows.push(vec![button(
        "➕ Aggiungi pasto",
        format!("turni:pasto:add:{modello_id}"),
    )]);
    rows.push(vec![button(
        "📅 Assegna a una data",
        format!("turni:model:assign:{modello_id}"),
    )]);
    rows.push(vec![button(
        "✏️ Rinomina",
        format!("turni:model:rename:{modello_id}"),
    )]);
    rows.push(vec![button(
        "🗑 Archivia",
        format!("turni:model:archive:{modello_id}"),
    )]);
    rows.push(nav_row("turni:menu"));

    let mut testo = notice
        .map(|value| format!("{value}\n\n"))
        .unwrap_or_default();
    testo.push_str(&format!("📋 {}", modello.nome));
    if pasti.is_empty() {
        testo.push_str("\n\nNessun pasto ancora. Aggiungine uno con ➕ Aggiungi pasto.");
    } else {
        testo.push_str("\n\nTocca un pasto per rimuoverlo dal modello.");
    }

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

fn tipo_pasto_keyboard(callback_di: impl Fn(MealType) -> String) -> InlineKeyboardMarkup {
    let tipi = [
        MealType::Breakfast,
        MealType::MorningSnack,
        MealType::Lunch,
        MealType::AfternoonSnack,
        MealType::Dinner,
        MealType::Other,
    ];
    let mut rows: Vec<Vec<InlineKeyboardButton>> = tipi
        .chunks(2)
        .map(|coppia| {
            coppia
                .iter()
                .map(|tipo| {
                    button(
                        format!("{} {}", meal_emoji(*tipo), tipo.label()),
                        callback_di(*tipo),
                    )
                })
                .collect()
        })
        .collect();
    rows.push(vec![button("❌ Annulla", "turni:pasto:add:cancel")]);
    InlineKeyboardMarkup::new(rows)
}

fn situazione_keyboard(callback_di: impl Fn(Situazione) -> String) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Situazione::TUTTE
        .chunks(2)
        .map(|coppia| {
            coppia
                .iter()
                .map(|s| button(format!("{} {}", s.emoji(), s.label()), callback_di(*s)))
                .collect()
        })
        .collect();
    rows.push(vec![button("❌ Annulla", "turni:pasto:add:cancel")]);
    InlineKeyboardMarkup::new(rows)
}

async fn show_tipo_picker(bot: &Bot, chat_id: ChatId, modello_id: i64) -> ResponseResult<()> {
    bot.send_message(chat_id, "➕ Aggiungi pasto\n\nScegli il tipo di pasto.")
        .reply_markup(tipo_pasto_keyboard(|tipo| {
            format!("turni:pasto:add:tipo:{modello_id}:{}", tipo.token())
        }))
        .await?;
    Ok(())
}

fn testo_orario_pasto(tipo: MealType) -> String {
    format!(
        "➕ {} {}\n\nScrivi l'orario suggerito nel formato HH:MM (es. 12:30), oppure salta.",
        meal_emoji(tipo),
        tipo.label()
    )
}

async fn show_situazione_picker(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "In che situazione? (casa, lavoro, fuori...)")
        .reply_markup(situazione_keyboard(|s| {
            format!("turni:pasto:add:situazione:{}", s.token())
        }))
        .await?;
    Ok(())
}

fn prep_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("✅ Sì", "turni:pasto:add:prep:si"),
            button("⬜ No", "turni:pasto:add:prep:no"),
        ],
        vec![button("❌ Annulla", "turni:pasto:add:cancel")],
    ])
}

async fn show_prep_picker(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "🧺 Va preparato in anticipo?")
        .reply_markup(prep_keyboard())
        .await?;
    Ok(())
}

async fn show_prepnota_prompt(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🧺 Quando/come va preparato? Scrivi una nota breve, oppure salta.",
    )
    .reply_markup(skip_e_annulla_markup(
        "➖ Salta",
        "turni:pasto:add:prepnota:skip",
        "turni:pasto:add:cancel",
    ))
    .await?;
    Ok(())
}

async fn show_nota_prompt(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "📝 Una nota libera per questo pasto? Scrivila, oppure salta.",
    )
    .reply_markup(skip_e_annulla_markup(
        "➖ Salta",
        "turni:pasto:add:nota:skip",
        "turni:pasto:add:cancel",
    ))
    .await?;
    Ok(())
}

async fn finalizza_aggiunta_pasto(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &TurniSessionStore,
    nota: Option<String>,
) -> ResponseResult<()> {
    let Some(draft) = draft_get(chat_id.0) else {
        sessions.clear_chat(chat_id.0);
        return expired(bot, chat_id).await;
    };
    sessions.clear_chat(chat_id.0);
    draft_clear(chat_id.0);
    let situazione = draft.situazione.unwrap_or(Situazione::Casa);
    let esito = aggiungi_pasto_modello(
        pool,
        draft.modello_id,
        draft.tipo_pasto.token(),
        draft.orario.as_deref(),
        situazione.token(),
        draft.preparazione_anticipata.unwrap_or(false),
        draft.preparazione_note.as_deref(),
        nota.as_deref(),
    )
    .await;
    let notice = match esito {
        Ok(_) => "✅ Pasto aggiunto.",
        Err(_) => "⚠️ Non sono riuscito ad aggiungere il pasto.",
    };
    show_model_detail(bot, chat_id, pool, draft.modello_id, Some(notice)).await
}

// --- Schermate: assegnazione a una data -----------------------------------

#[allow(clippy::too_many_arguments)]
async fn show_profile_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
    titolo: &str,
    callback_scelta: impl Fn(i64) -> String,
    callback_pagina: impl Fn(i64) -> String,
    back: &str,
) -> ResponseResult<()> {
    let totale = conta_profili_visibili(pool).await.unwrap_or(0);
    let pagine = liste::totale_pagine(totale);
    let page = page.clamp(0, pagine - 1);
    let profili = profili_visibili_pagina(pool, page)
        .await
        .unwrap_or_default();

    let mut rows: Vec<Vec<InlineKeyboardButton>> = profili
        .iter()
        .map(|profilo| {
            vec![button(
                format!("👤 {}", profilo.name),
                callback_scelta(profilo.id),
            )]
        })
        .collect();
    if let Some(riga) = liste::riga_paginazione(page, pagine, "turni:noop", callback_pagina) {
        rows.push(riga);
    }
    rows.push(nav_row(back));

    let mut testo = titolo.to_string();
    if profili.is_empty() {
        testo.push_str(
            "\n\nNessun profilo alimentare disponibile. Creane uno da 👥 Profili alimentari.",
        );
    }
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_assign_calendar(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    modello_id: i64,
    profilo_id: i64,
    year: i32,
    month: u32,
) -> ResponseResult<()> {
    let oggi: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| "2026-01-01".to_string());
    let marcati = giorni_con_assegnazione_nel_mese(pool, profilo_id, year, month)
        .await
        .unwrap_or_default();

    let giorno = |data: &str| calendario::Giorno {
        stato: calendario::GiornoStato::Libero,
        marcatore: marcati.contains(data).then_some("•"),
    };
    let callback_giorno = |data: &str| format!("turni:assign:do:{modello_id}:{profilo_id}:{data}");
    let callback_mese = |anno: i32, mese: u32| {
        format!("turni:assign:cal:{modello_id}:{profilo_id}:{anno:04}-{mese:02}")
    };
    let config = calendario::Calendario {
        year,
        month,
        oggi: &oggi,
        callback_giorno: &callback_giorno,
        callback_mese: &callback_mese,
        callback_inerte: "turni:noop",
        giorno: &giorno,
        mese_minimo: None,
    };
    let mut rows = calendario::righe(&config);
    rows.push(nav_row(&format!("turni:model:{modello_id}")));

    let mut testo = "📅 Scegli la data da assegnare".to_string();
    if !marcati.is_empty() {
        testo.push_str("\n\nI giorni con • hanno già un turno assegnato per questo profilo.");
    }
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn esegui_assegnazione(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    modello_id: i64,
    profilo_id: i64,
    data: &str,
    notice: Option<&str>,
) -> ResponseResult<()> {
    match assegna_modello(pool, modello_id, profilo_id, data).await {
        Ok(assegnazione_id) => {
            show_assignment_detail(
                bot,
                chat_id,
                pool,
                assegnazione_id,
                Some(notice.unwrap_or("✅ Turno assegnato.")),
            )
            .await
        }
        Err(_) => {
            bot.send_message(chat_id, "⚠️ Non sono riuscito ad assegnare il turno.")
                .reply_markup(nav_markup(&format!("turni:model:{modello_id}")))
                .await?;
            Ok(())
        }
    }
}

fn conferma_sovrascrivi_markup(
    modello_id: i64,
    profilo_id: i64,
    data: &str,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "✅ Sì, sostituisci",
            format!("turni:assign:confirm:{modello_id}:{profilo_id}:{data}"),
        )],
        nav_row(&format!("turni:model:{modello_id}")),
    ])
}

// --- Schermate: assegnazione di un giorno ---------------------------------

async fn show_assignment_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    assegnazione_id: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let Some(assegnazione) = trova_assegnazione(pool, assegnazione_id)
        .await
        .unwrap_or(None)
    else {
        return invalid(bot, chat_id).await;
    };
    let pasti = lista_pasti_assegnati(pool, assegnazione_id)
        .await
        .unwrap_or_default();

    let mut rows: Vec<Vec<InlineKeyboardButton>> = pasti
        .iter()
        .map(|pasto| {
            let tipo = MealType::from_token(&pasto.tipo_pasto)
                .map(|meal| format!("{} {}", meal_emoji(meal), meal.label()))
                .unwrap_or_else(|| "🍴 Pasto".to_string());
            let situazione = Situazione::from_token(&pasto.situazione)
                .map(|s| format!("{} {}", s.emoji(), s.label()))
                .unwrap_or_default();
            let orario = pasto.orario.clone().unwrap_or_else(|| "--:--".to_string());
            let mut etichetta = format!("{tipo}\n{orario} · {situazione}");
            if pasto.preparazione_anticipata != 0 {
                etichetta.push_str("\n🧺 Da preparare prima");
            }
            vec![button(etichetta, format!("turni:apasto:{}", pasto.id))]
        })
        .collect();
    rows.push(nav_row("turni:menu"));

    let mut testo = notice
        .map(|value| format!("{value}\n\n"))
        .unwrap_or_default();
    testo.push_str(&format!(
        "📅 {} · {}\n👤 {}",
        calendario::display_date(&assegnazione.data),
        assegnazione.modello_nome_snapshot,
        assegnazione.profilo_nome_snapshot
    ));
    if pasti.is_empty() {
        testo.push_str("\n\nNessun pasto in questa assegnazione.");
    } else {
        testo.push_str("\n\nTocca un pasto per modificarlo.");
    }

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_apasto_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    pasto_id: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let Some(pasto) = trova_pasto_assegnato(pool, pasto_id).await.unwrap_or(None) else {
        return invalid(bot, chat_id).await;
    };
    let tipo = MealType::from_token(&pasto.tipo_pasto)
        .map(|meal| format!("{} {}", meal_emoji(meal), meal.label()))
        .unwrap_or_else(|| "🍴 Pasto".to_string());
    let situazione_label = Situazione::from_token(&pasto.situazione)
        .map(|s| format!("{} {}", s.emoji(), s.label()))
        .unwrap_or_default();

    let mut rows = vec![
        vec![button(
            "✏️ Situazione",
            format!("turni:apasto:sit:{pasto_id}"),
        )],
        vec![button(
            "✏️ Orario",
            format!("turni:apasto:orario:{pasto_id}"),
        )],
    ];
    if pasto.preparazione_anticipata != 0 {
        rows.push(vec![button(
            "⬜ Non serve più preparazione",
            format!("turni:apasto:prep:{pasto_id}:no"),
        )]);
        rows.push(vec![button(
            "✏️ Nota di preparazione",
            format!("turni:apasto:prepnota:{pasto_id}"),
        )]);
    } else {
        rows.push(vec![button(
            "✅ Da preparare in anticipo",
            format!("turni:apasto:prep:{pasto_id}:si"),
        )]);
    }
    rows.push(vec![button(
        "✏️ Nota",
        format!("turni:apasto:nota:{pasto_id}"),
    )]);
    rows.push(vec![button(
        "🗑 Rimuovi pasto",
        format!("turni:apasto:remove:{pasto_id}"),
    )]);
    rows.push(nav_row(&format!(
        "turni:assegnazione:{}",
        pasto.assegnazione_id
    )));

    let mut testo = notice
        .map(|value| format!("{value}\n\n"))
        .unwrap_or_default();
    testo.push_str(&format!(
        "{tipo}\n🕐 {}\n📍 {situazione_label}",
        pasto.orario.as_deref().unwrap_or("orario non impostato")
    ));
    if pasto.preparazione_anticipata != 0 {
        testo.push_str("\n🧺 Da preparare in anticipo");
        if let Some(nota) = &pasto.preparazione_note {
            testo.push_str(&format!(": {nota}"));
        }
    }
    if let Some(nota) = &pasto.nota {
        testo.push_str(&format!("\n📝 {nota}"));
    }

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

// --- Ingresso principale ---------------------------------------------------

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &TurniSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if text.trim() == "/annulla" {
        if let Some(stato) = sessions.get(chat_id) {
            sessions.clear_chat(chat_id);
            bot.annulla_e_avvisa(chat_id, "❌ Operazione annullata.");
            match stato {
                TurniConversationState::AwaitingNomeModello => {
                    show_menu(bot, msg.chat.id, pool, 0, None).await?;
                }
                TurniConversationState::AwaitingRinominaModello { modello_id } => {
                    show_model_detail(bot, msg.chat.id, pool, modello_id, None).await?;
                }
                TurniConversationState::AwaitingOrarioPasto
                | TurniConversationState::AwaitingNotaPreparazione
                | TurniConversationState::AwaitingNotaLibera => {
                    let modello_id = draft_get(chat_id).map(|d| d.modello_id);
                    draft_clear(chat_id);
                    match modello_id {
                        Some(id) => show_model_detail(bot, msg.chat.id, pool, id, None).await?,
                        None => show_menu(bot, msg.chat.id, pool, 0, None).await?,
                    }
                }
                TurniConversationState::AwaitingOrarioAssegnato { pasto_id }
                | TurniConversationState::AwaitingNotaPreparazioneAssegnato { pasto_id }
                | TurniConversationState::AwaitingNotaAssegnato { pasto_id } => {
                    show_apasto_detail(bot, msg.chat.id, pool, pasto_id, None).await?;
                }
            }
            return Ok(true);
        }
        return Ok(false);
    }

    let Some(stato) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match stato {
        TurniConversationState::AwaitingNomeModello => match valida_nome_modello(text) {
            Ok(nome) => {
                sessions.clear_chat(chat_id);
                match crea_modello(pool, &nome).await {
                    Ok(id) => {
                        show_model_detail(bot, msg.chat.id, pool, id, Some("✅ Modello creato."))
                            .await?
                    }
                    Err(_) => {
                        show_menu(
                            bot,
                            msg.chat.id,
                            pool,
                            0,
                            Some("⚠️ Non sono riuscito a creare il modello."),
                        )
                        .await?
                    }
                }
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(annulla_markup("turni:model:new:cancel"))
                    .await?;
            }
        },
        TurniConversationState::AwaitingRinominaModello { modello_id } => {
            match valida_nome_modello(text) {
                Ok(nome) => {
                    sessions.clear_chat(chat_id);
                    let _ = rinomina_modello(pool, modello_id, &nome).await;
                    show_model_detail(
                        bot,
                        msg.chat.id,
                        pool,
                        modello_id,
                        Some("✅ Modello rinominato."),
                    )
                    .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(annulla_markup(format!(
                            "turni:model:rename:cancel:{modello_id}"
                        )))
                        .await?;
                }
            }
        }
        TurniConversationState::AwaitingOrarioPasto => match valida_orario(text) {
            Ok(orario) => {
                let Some(mut draft) = draft_get(chat_id) else {
                    sessions.clear_chat(chat_id);
                    return expired(bot, msg.chat.id).await.map(|_| true);
                };
                draft.orario = Some(orario);
                draft_set(chat_id, draft);
                sessions.clear_chat(chat_id);
                show_situazione_picker(bot, msg.chat.id).await?;
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(skip_e_annulla_markup(
                        "➖ Salta",
                        "turni:pasto:add:orario:skip",
                        "turni:pasto:add:cancel",
                    ))
                    .await?;
            }
        },
        TurniConversationState::AwaitingNotaPreparazione => match valida_nota_opzionale(text) {
            Ok(nota) => {
                let Some(mut draft) = draft_get(chat_id) else {
                    sessions.clear_chat(chat_id);
                    return expired(bot, msg.chat.id).await.map(|_| true);
                };
                draft.preparazione_note = nota;
                draft_set(chat_id, draft);
                sessions.set(chat_id, TurniConversationState::AwaitingNotaLibera);
                show_nota_prompt(bot, msg.chat.id).await?;
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(skip_e_annulla_markup(
                        "➖ Salta",
                        "turni:pasto:add:prepnota:skip",
                        "turni:pasto:add:cancel",
                    ))
                    .await?;
            }
        },
        TurniConversationState::AwaitingNotaLibera => match valida_nota_opzionale(text) {
            Ok(nota) => {
                finalizza_aggiunta_pasto(bot, msg.chat.id, pool, sessions, nota).await?;
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(skip_e_annulla_markup(
                        "➖ Salta",
                        "turni:pasto:add:nota:skip",
                        "turni:pasto:add:cancel",
                    ))
                    .await?;
            }
        },
        TurniConversationState::AwaitingOrarioAssegnato { pasto_id } => match valida_orario(text) {
            Ok(orario) => {
                sessions.clear_chat(chat_id);
                let _ = modifica_orario_pasto_assegnato(pool, pasto_id, Some(&orario)).await;
                show_apasto_detail(
                    bot,
                    msg.chat.id,
                    pool,
                    pasto_id,
                    Some("✅ Orario aggiornato."),
                )
                .await?;
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(skip_e_annulla_markup(
                        "➖ Salta",
                        format!("turni:apasto:orario:skip:{pasto_id}"),
                        format!("turni:apasto:cancel:{pasto_id}"),
                    ))
                    .await?;
            }
        },
        TurniConversationState::AwaitingNotaPreparazioneAssegnato { pasto_id } => {
            match valida_nota_opzionale(text) {
                Ok(nota) => {
                    sessions.clear_chat(chat_id);
                    let _ =
                        modifica_nota_preparazione_pasto_assegnato(pool, pasto_id, nota.as_deref())
                            .await;
                    show_apasto_detail(
                        bot,
                        msg.chat.id,
                        pool,
                        pasto_id,
                        Some("✅ Nota aggiornata."),
                    )
                    .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(skip_e_annulla_markup(
                            "➖ Salta",
                            format!("turni:apasto:prepnota:skip:{pasto_id}"),
                            format!("turni:apasto:cancel:{pasto_id}"),
                        ))
                        .await?;
                }
            }
        }
        TurniConversationState::AwaitingNotaAssegnato { pasto_id } => {
            match valida_nota_opzionale(text) {
                Ok(nota) => {
                    sessions.clear_chat(chat_id);
                    let _ = modifica_nota_pasto_assegnato(pool, pasto_id, nota.as_deref()).await;
                    show_apasto_detail(
                        bot,
                        msg.chat.id,
                        pool,
                        pasto_id,
                        Some("✅ Nota aggiornata."),
                    )
                    .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(skip_e_annulla_markup(
                            "➖ Salta",
                            format!("turni:apasto:nota:skip:{pasto_id}"),
                            format!("turni:apasto:cancel:{pasto_id}"),
                        ))
                        .await?;
                }
            }
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_lines)]
pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &TurniSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    if data == "turni:menu" {
        sessions.clear_chat(chat_id.0);
        draft_clear(chat_id.0);
        show_menu(bot, chat_id, pool, 0, None).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:menu:page:") {
        let page: i64 = rest.parse().unwrap_or(0);
        show_menu(bot, chat_id, pool, page, None).await?;
        return Ok(true);
    }
    if data == "turni:model:new" {
        sessions.set(chat_id.0, TurniConversationState::AwaitingNomeModello);
        bot.send_message(
            chat_id,
            "➕ Nuovo modello\n\nScrivi il nome del modello (es. \"Chiusura\", \"Università\").",
        )
        .reply_markup(annulla_markup("turni:model:new:cancel"))
        .await?;
        return Ok(true);
    }
    if data == "turni:model:new:cancel" {
        sessions.clear_chat(chat_id.0);
        bot.annulla_e_avvisa(chat_id.0, "❌ Creazione annullata.");
        show_menu(bot, chat_id, pool, 0, None).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:model:rename:cancel:") {
        let modello_id: i64 = rest.parse().unwrap_or(0);
        sessions.clear_chat(chat_id.0);
        bot.annulla_e_avvisa(chat_id.0, "❌ Modifica annullata.");
        show_model_detail(bot, chat_id, pool, modello_id, None).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:model:rename:") {
        let modello_id: i64 = rest.parse().unwrap_or(0);
        sessions.set(
            chat_id.0,
            TurniConversationState::AwaitingRinominaModello { modello_id },
        );
        bot.send_message(chat_id, "✏️ Scrivi il nuovo nome del modello.")
            .reply_markup(annulla_markup(format!(
                "turni:model:rename:cancel:{modello_id}"
            )))
            .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:model:archive:") {
        let modello_id: i64 = rest.parse().unwrap_or(0);
        let _ = archivia_modello(pool, modello_id).await;
        show_menu(bot, chat_id, pool, 0, Some("✅ Modello archiviato.")).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:model:assign:") {
        let modello_id: i64 = rest.parse().unwrap_or(0);
        show_profile_picker(
            bot,
            chat_id,
            pool,
            0,
            "📅 Assegna a una data\n\nScegli il profilo.",
            |profilo_id| format!("turni:assign:cal:{modello_id}:{profilo_id}"),
            move |page| format!("turni:assign:pick:{modello_id}:{page}"),
            &format!("turni:model:{modello_id}"),
        )
        .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:assign:pick:") {
        if let Some((modello_id, page)) = parse_ids2(rest) {
            show_profile_picker(
                bot,
                chat_id,
                pool,
                page,
                "📅 Assegna a una data\n\nScegli il profilo.",
                |profilo_id| format!("turni:assign:cal:{modello_id}:{profilo_id}"),
                move |p| format!("turni:assign:pick:{modello_id}:{p}"),
                &format!("turni:model:{modello_id}"),
            )
            .await?;
        } else {
            invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:assign:cal:") {
        if let Some((modello_id, resto)) = parse_id_and_rest(rest) {
            if let Some((profilo_id, mese)) = parse_id_and_rest(resto) {
                let (anno, mese) = if let Some((a, m)) = mese.split_once('-') {
                    (a.parse().unwrap_or(2026), m.parse().unwrap_or(1))
                } else {
                    (2026, 1)
                };
                show_assign_calendar(bot, chat_id, pool, modello_id, profilo_id, anno, mese)
                    .await?;
            } else {
                // Nessun mese specificato: prima apertura, usa il mese corrente.
                let profilo_id: i64 = resto.parse().unwrap_or(0);
                let oggi: String = sqlx::query_scalar("SELECT date('now','localtime')")
                    .fetch_one(pool)
                    .await
                    .unwrap_or_else(|_| "2026-01-01".to_string());
                let (anno, mese) = (
                    oggi[..4].parse().unwrap_or(2026),
                    oggi[5..7].parse().unwrap_or(1),
                );
                show_assign_calendar(bot, chat_id, pool, modello_id, profilo_id, anno, mese)
                    .await?;
            }
        } else {
            invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:assign:confirm:") {
        let parti: Vec<&str> = rest.splitn(3, ':').collect();
        if let [modello_id, profilo_id, data_assegnata] = parti[..] {
            if let (Ok(modello_id), Ok(profilo_id)) =
                (modello_id.parse::<i64>(), profilo_id.parse::<i64>())
            {
                if let Some(esistente) =
                    trova_assegnazione_profilo_data(pool, profilo_id, data_assegnata)
                        .await
                        .unwrap_or(None)
                {
                    let _ = elimina_assegnazione(pool, esistente.id).await;
                }
                esegui_assegnazione(
                    bot,
                    chat_id,
                    pool,
                    modello_id,
                    profilo_id,
                    data_assegnata,
                    Some("🔁 Turno sostituito."),
                )
                .await?;
                return Ok(true);
            }
        }
        invalid(bot, chat_id).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:assign:do:") {
        let parti: Vec<&str> = rest.splitn(3, ':').collect();
        if let [modello_id, profilo_id, data_scelta] = parti[..] {
            if let (Ok(modello_id), Ok(profilo_id)) =
                (modello_id.parse::<i64>(), profilo_id.parse::<i64>())
            {
                if !calendario::valid_date(data_scelta) {
                    invalid(bot, chat_id).await?;
                    return Ok(true);
                }
                match trova_assegnazione_profilo_data(pool, profilo_id, data_scelta)
                    .await
                    .unwrap_or(None)
                {
                    Some(_) => {
                        bot.send_message(
                            chat_id,
                            format!(
                                "⚠️ Esiste già un turno assegnato per il {} a questo profilo.\n\nSostituirlo?",
                                calendario::display_date(data_scelta)
                            ),
                        )
                        .reply_markup(conferma_sovrascrivi_markup(
                            modello_id,
                            profilo_id,
                            data_scelta,
                        ))
                        .await?;
                    }
                    None => {
                        esegui_assegnazione(
                            bot,
                            chat_id,
                            pool,
                            modello_id,
                            profilo_id,
                            data_scelta,
                            None,
                        )
                        .await?;
                    }
                }
                return Ok(true);
            }
        }
        invalid(bot, chat_id).await?;
        return Ok(true);
    }

    if let Some(rest) = data.strip_prefix("turni:pasto:add:tipo:") {
        if let Some((modello_id, tipo_token)) = parse_id_and_rest(rest) {
            let Some(tipo) = MealType::from_token(tipo_token) else {
                invalid(bot, chat_id).await?;
                return Ok(true);
            };
            draft_set(
                chat_id.0,
                DraftPastoModello {
                    modello_id,
                    tipo_pasto: tipo,
                    orario: None,
                    situazione: None,
                    preparazione_anticipata: None,
                    preparazione_note: None,
                },
            );
            sessions.set(chat_id.0, TurniConversationState::AwaitingOrarioPasto);
            bot.send_message(chat_id, testo_orario_pasto(tipo))
                .reply_markup(skip_e_annulla_markup(
                    "➖ Salta",
                    "turni:pasto:add:orario:skip",
                    "turni:pasto:add:cancel",
                ))
                .await?;
        } else {
            invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if data == "turni:pasto:add:orario:skip" {
        let Some(mut draft) = draft_get(chat_id.0) else {
            sessions.clear_chat(chat_id.0);
            return expired(bot, chat_id).await.map(|_| true);
        };
        draft.orario = None;
        draft_set(chat_id.0, draft);
        sessions.clear_chat(chat_id.0);
        show_situazione_picker(bot, chat_id).await?;
        return Ok(true);
    }
    if let Some(token) = data.strip_prefix("turni:pasto:add:situazione:") {
        let Some(situazione) = Situazione::from_token(token) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let Some(mut draft) = draft_get(chat_id.0) else {
            return expired(bot, chat_id).await.map(|_| true);
        };
        draft.situazione = Some(situazione);
        draft_set(chat_id.0, draft);
        show_prep_picker(bot, chat_id).await?;
        return Ok(true);
    }
    if data == "turni:pasto:add:prep:si" {
        let Some(mut draft) = draft_get(chat_id.0) else {
            return expired(bot, chat_id).await.map(|_| true);
        };
        draft.preparazione_anticipata = Some(true);
        draft_set(chat_id.0, draft);
        sessions.set(chat_id.0, TurniConversationState::AwaitingNotaPreparazione);
        show_prepnota_prompt(bot, chat_id).await?;
        return Ok(true);
    }
    if data == "turni:pasto:add:prep:no" {
        let Some(mut draft) = draft_get(chat_id.0) else {
            return expired(bot, chat_id).await.map(|_| true);
        };
        draft.preparazione_anticipata = Some(false);
        draft.preparazione_note = None;
        draft_set(chat_id.0, draft);
        sessions.set(chat_id.0, TurniConversationState::AwaitingNotaLibera);
        show_nota_prompt(bot, chat_id).await?;
        return Ok(true);
    }
    if data == "turni:pasto:add:prepnota:skip" {
        let Some(mut draft) = draft_get(chat_id.0) else {
            sessions.clear_chat(chat_id.0);
            return expired(bot, chat_id).await.map(|_| true);
        };
        draft.preparazione_note = None;
        draft_set(chat_id.0, draft);
        sessions.set(chat_id.0, TurniConversationState::AwaitingNotaLibera);
        show_nota_prompt(bot, chat_id).await?;
        return Ok(true);
    }
    if data == "turni:pasto:add:nota:skip" {
        finalizza_aggiunta_pasto(bot, chat_id, pool, sessions, None).await?;
        return Ok(true);
    }
    if data == "turni:pasto:add:cancel" {
        let modello_id = draft_get(chat_id.0).map(|d| d.modello_id);
        sessions.clear_chat(chat_id.0);
        draft_clear(chat_id.0);
        bot.annulla_e_avvisa(chat_id.0, "❌ Aggiunta annullata.");
        match modello_id {
            Some(id) => show_model_detail(bot, chat_id, pool, id, None).await?,
            None => show_menu(bot, chat_id, pool, 0, None).await?,
        }
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:pasto:add:") {
        let modello_id: i64 = rest.parse().unwrap_or(0);
        show_tipo_picker(bot, chat_id, modello_id).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:pasto:remove:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        let modello_id = trova_pasto_modello(pool, pasto_id)
            .await
            .unwrap_or(None)
            .map(|p| p.modello_id);
        let Some(modello_id) = modello_id else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let _ = rimuovi_pasto_modello(pool, pasto_id).await;
        show_model_detail(
            bot,
            chat_id,
            pool,
            modello_id,
            Some("🗑 Pasto rimosso dal modello."),
        )
        .await?;
        return Ok(true);
    }

    if data.strip_prefix("turni:model:").is_some()
        && !data.starts_with("turni:model:new")
        && !data.starts_with("turni:model:rename")
        && !data.starts_with("turni:model:archive")
        && !data.starts_with("turni:model:assign")
    {
        let modello_id: i64 = data
            .strip_prefix("turni:model:")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        show_model_detail(bot, chat_id, pool, modello_id, None).await?;
        return Ok(true);
    }

    if data == "turni:vedi" {
        show_profile_picker(
            bot,
            chat_id,
            pool,
            0,
            "📅 Vedi/modifica assegnazione\n\nScegli il profilo.",
            |profilo_id| format!("turni:vedi:profile:{profilo_id}"),
            |page| format!("turni:vedi:pick:{page}"),
            "turni:menu",
        )
        .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:vedi:pick:") {
        let page: i64 = rest.parse().unwrap_or(0);
        show_profile_picker(
            bot,
            chat_id,
            pool,
            page,
            "📅 Vedi/modifica assegnazione\n\nScegli il profilo.",
            |profilo_id| format!("turni:vedi:profile:{profilo_id}"),
            |p| format!("turni:vedi:pick:{p}"),
            "turni:menu",
        )
        .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:vedi:month:") {
        if let Some((profilo_id, mese)) = parse_id_and_rest(rest) {
            let (anno, mese) = if let Some((a, m)) = mese.split_once('-') {
                (a.parse().unwrap_or(2026), m.parse().unwrap_or(1))
            } else {
                (2026, 1)
            };
            show_vedi_calendar(bot, chat_id, pool, profilo_id, anno, mese).await?;
        } else {
            invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:vedi:profile:") {
        let profilo_id: i64 = rest.parse().unwrap_or(0);
        let oggi: String = sqlx::query_scalar("SELECT date('now','localtime')")
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| "2026-01-01".to_string());
        let (anno, mese) = (
            oggi[..4].parse().unwrap_or(2026),
            oggi[5..7].parse().unwrap_or(1),
        );
        show_vedi_calendar(bot, chat_id, pool, profilo_id, anno, mese).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:vedi:day:") {
        if let Some((profilo_id, giorno)) = parse_id_and_rest(rest) {
            match trova_assegnazione_profilo_data(pool, profilo_id, giorno)
                .await
                .unwrap_or(None)
            {
                Some(assegnazione) => {
                    show_assignment_detail(bot, chat_id, pool, assegnazione.id, None).await?;
                }
                None => {
                    bot.send_message(
                        chat_id,
                        format!(
                            "ℹ️ Nessun turno assegnato per il {}.",
                            calendario::display_date(giorno)
                        ),
                    )
                    .reply_markup(nav_markup("turni:vedi"))
                    .await?;
                }
            }
        } else {
            invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }

    if let Some(rest) = data.strip_prefix("turni:assegnazione:") {
        let assegnazione_id: i64 = rest.parse().unwrap_or(0);
        show_assignment_detail(bot, chat_id, pool, assegnazione_id, None).await?;
        return Ok(true);
    }

    if let Some(rest) = data.strip_prefix("turni:apasto:sit:set:") {
        if let Some((pasto_id, token)) = parse_id_and_rest(rest) {
            if Situazione::from_token(token).is_some() {
                let _ = modifica_situazione_pasto_assegnato(pool, pasto_id, token).await;
                show_apasto_detail(
                    bot,
                    chat_id,
                    pool,
                    pasto_id,
                    Some("✅ Situazione aggiornata."),
                )
                .await?;
                return Ok(true);
            }
        }
        invalid(bot, chat_id).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:sit:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        bot.send_message(chat_id, "In che situazione?")
            .reply_markup(situazione_keyboard(|s| {
                format!("turni:apasto:sit:set:{pasto_id}:{}", s.token())
            }))
            .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:orario:skip:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        sessions.clear_chat(chat_id.0);
        let _ = modifica_orario_pasto_assegnato(pool, pasto_id, None).await;
        show_apasto_detail(bot, chat_id, pool, pasto_id, Some("✅ Orario rimosso.")).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:orario:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        sessions.set(
            chat_id.0,
            TurniConversationState::AwaitingOrarioAssegnato { pasto_id },
        );
        bot.send_message(
            chat_id,
            "✏️ Scrivi il nuovo orario nel formato HH:MM, oppure salta.",
        )
        .reply_markup(skip_e_annulla_markup(
            "➖ Salta",
            format!("turni:apasto:orario:skip:{pasto_id}"),
            format!("turni:apasto:cancel:{pasto_id}"),
        ))
        .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:prep:") {
        if let Some((pasto_id, scelta)) = parse_id_and_rest(rest) {
            match scelta {
                "si" => {
                    sessions.set(
                        chat_id.0,
                        TurniConversationState::AwaitingNotaPreparazioneAssegnato { pasto_id },
                    );
                    let _ = imposta_preparazione_pasto_assegnato(pool, pasto_id, true).await;
                    bot.send_message(
                        chat_id,
                        "🧺 Quando/come va preparato? Scrivi una nota breve, oppure salta.",
                    )
                    .reply_markup(skip_e_annulla_markup(
                        "➖ Salta",
                        format!("turni:apasto:prepnota:skip:{pasto_id}"),
                        format!("turni:apasto:cancel:{pasto_id}"),
                    ))
                    .await?;
                }
                "no" => {
                    let _ = imposta_preparazione_pasto_assegnato(pool, pasto_id, false).await;
                    show_apasto_detail(
                        bot,
                        chat_id,
                        pool,
                        pasto_id,
                        Some("✅ Non serve più preparazione anticipata."),
                    )
                    .await?;
                }
                _ => invalid(bot, chat_id).await?,
            }
        } else {
            invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:prepnota:skip:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        sessions.clear_chat(chat_id.0);
        let _ = modifica_nota_preparazione_pasto_assegnato(pool, pasto_id, None).await;
        show_apasto_detail(bot, chat_id, pool, pasto_id, Some("✅ Nota rimossa.")).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:prepnota:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        sessions.set(
            chat_id.0,
            TurniConversationState::AwaitingNotaPreparazioneAssegnato { pasto_id },
        );
        bot.send_message(
            chat_id,
            "✏️ Scrivi la nuova nota di preparazione, oppure salta.",
        )
        .reply_markup(skip_e_annulla_markup(
            "➖ Salta",
            format!("turni:apasto:prepnota:skip:{pasto_id}"),
            format!("turni:apasto:cancel:{pasto_id}"),
        ))
        .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:nota:skip:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        sessions.clear_chat(chat_id.0);
        let _ = modifica_nota_pasto_assegnato(pool, pasto_id, None).await;
        show_apasto_detail(bot, chat_id, pool, pasto_id, Some("✅ Nota rimossa.")).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:nota:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        sessions.set(
            chat_id.0,
            TurniConversationState::AwaitingNotaAssegnato { pasto_id },
        );
        bot.send_message(chat_id, "✏️ Scrivi la nuova nota, oppure salta.")
            .reply_markup(skip_e_annulla_markup(
                "➖ Salta",
                format!("turni:apasto:nota:skip:{pasto_id}"),
                format!("turni:apasto:cancel:{pasto_id}"),
            ))
            .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:cancel:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        sessions.clear_chat(chat_id.0);
        bot.annulla_e_avvisa(chat_id.0, "❌ Modifica annullata.");
        show_apasto_detail(bot, chat_id, pool, pasto_id, None).await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:remove:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        let assegnazione_id = trova_pasto_assegnato(pool, pasto_id)
            .await
            .unwrap_or(None)
            .map(|p| p.assegnazione_id);
        let Some(assegnazione_id) = assegnazione_id else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let _ = rimuovi_pasto_assegnato(pool, pasto_id).await;
        show_assignment_detail(
            bot,
            chat_id,
            pool,
            assegnazione_id,
            Some("🗑 Pasto rimosso."),
        )
        .await?;
        return Ok(true);
    }
    if let Some(rest) = data.strip_prefix("turni:apasto:") {
        let pasto_id: i64 = rest.parse().unwrap_or(0);
        show_apasto_detail(bot, chat_id, pool, pasto_id, None).await?;
        return Ok(true);
    }

    if data == "turni:noop" {
        return Ok(true);
    }

    Ok(false)
}

async fn show_vedi_calendar(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profilo_id: i64,
    year: i32,
    month: u32,
) -> ResponseResult<()> {
    let oggi: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| "2026-01-01".to_string());
    let marcati = giorni_con_assegnazione_nel_mese(pool, profilo_id, year, month)
        .await
        .unwrap_or_default();

    let giorno = |data: &str| calendario::Giorno {
        stato: calendario::GiornoStato::Libero,
        marcatore: marcati.contains(data).then_some("•"),
    };
    let callback_giorno = |data: &str| format!("turni:vedi:day:{profilo_id}:{data}");
    let callback_mese =
        |anno: i32, mese: u32| format!("turni:vedi:month:{profilo_id}:{anno:04}-{mese:02}");
    let config = calendario::Calendario {
        year,
        month,
        oggi: &oggi,
        callback_giorno: &callback_giorno,
        callback_mese: &callback_mese,
        callback_inerte: "turni:noop",
        giorno: &giorno,
        mese_minimo: None,
    };
    let mut rows = calendario::righe(&config);
    rows.push(nav_row("turni:vedi"));

    let mut testo = "📅 Vedi/modifica assegnazione\n\nScegli un giorno.".to_string();
    if !marcati.is_empty() {
        testo.push_str("\n\nI giorni con • hanno già un turno assegnato.");
    }
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}
#[cfg(test)]
mod db_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn actor(user_id: i64, space_id: i64, name: &str) -> crate::identity::AuditActor {
        crate::identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: name.to_string(),
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

    async fn create_user(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("utente")
            .last_insert_rowid()
    }

    async fn create_space(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO spazi (nome, tipo) VALUES (?, 'condiviso')")
            .bind(name)
            .execute(pool)
            .await
            .expect("spazio")
            .last_insert_rowid()
    }

    async fn add_membership(pool: &SqlitePool, space_id: i64, user_id: i64) {
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
        )
        .bind(space_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("membership");
    }

    async fn create_profilo(pool: &SqlitePool, utente_id: i64, nome: &str) -> i64 {
        sqlx::query(
            "INSERT INTO profili_alimentari (gestore_utente_id, nome, nome_normalizzato) \
             VALUES (?, ?, ?)",
        )
        .bind(utente_id)
        .bind(nome)
        .bind(nome.to_lowercase())
        .execute(pool)
        .await
        .expect("profilo")
        .last_insert_rowid()
    }

    /// Predispone utente + spazio + membership: ogni riga di
    /// `turno_modelli`/`turno_assegnazioni` porta sempre uno `spazio_id`
    /// concreto (`AuditActor::spazio_id` non è mai nullo), quindi il
    /// trigger di membership scatta a ogni inserimento, in test come in
    /// produzione.
    async fn setup(pool: &SqlitePool) -> (i64, i64) {
        let user_id = create_user(pool, "Alessio").await;
        let space_id = create_space(pool, "Casa").await;
        add_membership(pool, space_id, user_id).await;
        (user_id, space_id)
    }

    #[tokio::test]
    async fn crea_modello_e_lo_ritrova() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let id = crea_modello(&pool, "Chiusura").await.expect("creato");
            let modello = trova_modello(&pool, id)
                .await
                .expect("query")
                .expect("trovato");
            assert_eq!(modello.nome, "Chiusura");
        })
        .await;
    }

    #[tokio::test]
    async fn rinomina_e_archivia_modello() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let id = crea_modello(&pool, "Chiusura").await.expect("creato");

            rinomina_modello(&pool, id, "Chiusura serale")
                .await
                .expect("rinomina");
            let modello = trova_modello(&pool, id).await.unwrap().unwrap();
            assert_eq!(modello.nome, "Chiusura serale");

            archivia_modello(&pool, id).await.expect("archivia");
            assert!(trova_modello(&pool, id).await.unwrap().is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn aggiunge_e_lista_pasti_del_modello_in_ordine() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let modello_id = crea_modello(&pool, "Chiusura").await.expect("creato");

            aggiungi_pasto_modello(
                &pool,
                modello_id,
                "pranzo",
                Some("12:00"),
                "casa",
                false,
                None,
                None,
            )
            .await
            .expect("pranzo");
            aggiungi_pasto_modello(
                &pool,
                modello_id,
                "cena",
                Some("19:00"),
                "lavoro",
                true,
                Some("Pronta il giorno prima"),
                Some("Portare la gavetta"),
            )
            .await
            .expect("cena");

            let pasti = lista_pasti_modello(&pool, modello_id).await.unwrap();
            assert_eq!(pasti.len(), 2);
            assert_eq!(pasti[0].tipo_pasto, "pranzo");
            assert_eq!(pasti[1].tipo_pasto, "cena");
            assert_eq!(pasti[1].preparazione_anticipata, 1);
            assert_eq!(
                pasti[1].preparazione_note.as_deref(),
                Some("Pronta il giorno prima")
            );
        })
        .await;
    }

    #[tokio::test]
    async fn rimuovi_pasto_modello_lo_toglie_dalla_lista() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let modello_id = crea_modello(&pool, "Chiusura").await.expect("creato");
            let pasto_id = aggiungi_pasto_modello(
                &pool, modello_id, "pranzo", None, "casa", false, None, None,
            )
            .await
            .expect("pranzo");

            rimuovi_pasto_modello(&pool, pasto_id)
                .await
                .expect("rimuovi");
            assert!(lista_pasti_modello(&pool, modello_id)
                .await
                .unwrap()
                .is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn assegnare_copia_i_pasti_senza_toccare_il_modello() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;
        let profilo_id = create_profilo(&pool, user_id, "Alessio").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let modello_id = crea_modello(&pool, "Chiusura").await.expect("creato");
            aggiungi_pasto_modello(
                &pool,
                modello_id,
                "pranzo",
                Some("12:00"),
                "casa",
                false,
                None,
                None,
            )
            .await
            .expect("pranzo");

            let assegnazione_id = assegna_modello(&pool, modello_id, profilo_id, "2026-09-27")
                .await
                .expect("assegnato");

            let pasti_assegnati = lista_pasti_assegnati(&pool, assegnazione_id).await.unwrap();
            assert_eq!(pasti_assegnati.len(), 1);
            let pasto_assegnato_id = pasti_assegnati[0].id;

            // Modificare il pasto assegnato non deve toccare il modello.
            modifica_situazione_pasto_assegnato(&pool, pasto_assegnato_id, "fuori")
                .await
                .expect("modifica");
            let pasti_modello = lista_pasti_modello(&pool, modello_id).await.unwrap();
            assert_eq!(pasti_modello[0].situazione, "casa");

            let pasti_assegnati = lista_pasti_assegnati(&pool, assegnazione_id).await.unwrap();
            assert_eq!(pasti_assegnati[0].situazione, "fuori");
        })
        .await;
    }

    #[tokio::test]
    async fn assegnazione_ricorda_il_nome_del_modello_anche_se_rinominato_dopo() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;
        let profilo_id = create_profilo(&pool, user_id, "Alessio").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let modello_id = crea_modello(&pool, "Chiusura").await.expect("creato");

            let assegnazione_id = assegna_modello(&pool, modello_id, profilo_id, "2026-09-27")
                .await
                .expect("assegnato");

            rinomina_modello(&pool, modello_id, "Chiusura serale")
                .await
                .expect("rinomina");

            let assegnazione = trova_assegnazione(&pool, assegnazione_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(assegnazione.modello_nome_snapshot, "Chiusura");
        })
        .await;
    }

    #[tokio::test]
    async fn imposta_preparazione_a_falso_azzera_la_nota() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;
        let profilo_id = create_profilo(&pool, user_id, "Alessio").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let modello_id = crea_modello(&pool, "Chiusura").await.expect("creato");
            aggiungi_pasto_modello(
                &pool,
                modello_id,
                "cena",
                None,
                "lavoro",
                true,
                Some("Pronta prima"),
                None,
            )
            .await
            .expect("cena");
            let assegnazione_id = assegna_modello(&pool, modello_id, profilo_id, "2026-09-27")
                .await
                .expect("assegnato");
            let pasti = lista_pasti_assegnati(&pool, assegnazione_id).await.unwrap();
            let pasto_id = pasti[0].id;
            assert_eq!(pasti[0].preparazione_note.as_deref(), Some("Pronta prima"));

            imposta_preparazione_pasto_assegnato(&pool, pasto_id, false)
                .await
                .expect("disattiva");
            let pasto = trova_pasto_assegnato(&pool, pasto_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(pasto.preparazione_anticipata, 0);
            assert_eq!(pasto.preparazione_note, None);
        })
        .await;
    }

    #[tokio::test]
    async fn rimuovi_pasto_assegnato_non_tocca_gli_altri_ne_il_modello() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;
        let profilo_id = create_profilo(&pool, user_id, "Alessio").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let modello_id = crea_modello(&pool, "Chiusura").await.expect("creato");
            aggiungi_pasto_modello(&pool, modello_id, "pranzo", None, "casa", false, None, None)
                .await
                .expect("pranzo");
            aggiungi_pasto_modello(&pool, modello_id, "cena", None, "casa", false, None, None)
                .await
                .expect("cena");
            let assegnazione_id = assegna_modello(&pool, modello_id, profilo_id, "2026-09-27")
                .await
                .expect("assegnato");
            let pasti = lista_pasti_assegnati(&pool, assegnazione_id).await.unwrap();
            assert_eq!(pasti.len(), 2);

            rimuovi_pasto_assegnato(&pool, pasti[0].id)
                .await
                .expect("rimuovi");
            let restanti = lista_pasti_assegnati(&pool, assegnazione_id).await.unwrap();
            assert_eq!(restanti.len(), 1);
            assert_eq!(restanti[0].tipo_pasto, "cena");
            assert_eq!(
                lista_pasti_modello(&pool, modello_id).await.unwrap().len(),
                2
            );
        })
        .await;
    }

    #[tokio::test]
    async fn info_giorno_per_planner_nasconde_i_pasti_gia_pianificati_davvero() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;
        let profilo_id = create_profilo(&pool, user_id, "Alessio").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let modello_id = crea_modello(&pool, "Chiusura").await.expect("creato");
            aggiungi_pasto_modello(
                &pool,
                modello_id,
                "pranzo",
                Some("12:00"),
                "casa",
                false,
                None,
                None,
            )
            .await
            .expect("pranzo");
            aggiungi_pasto_modello(
                &pool,
                modello_id,
                "cena",
                Some("19:00"),
                "lavoro",
                true,
                None,
                None,
            )
            .await
            .expect("cena");
            assegna_modello(&pool, modello_id, profilo_id, "2026-09-27")
                .await
                .expect("assegnato");

            // Senza nessun pasto vero pianificato, entrambi compaiono.
            let blocco = info_giorno_per_planner(&pool, "2026-09-27")
                .await
                .expect("blocco presente");
            assert!(blocco.contains("Pranzo"));
            assert!(blocco.contains("Cena"));

            // Un planner vero con "pranzo" già pianificato per quel giorno e
            // quello spazio: "pranzo" sparisce dal suggerimento, "cena" resta.
            let planner_id: i64 = sqlx::query(
                "INSERT INTO planner_alimentari \
                 (proprietario_utente_id, spazio_id, nome, nome_normalizzato, \
                  data_inizio, data_fine) \
                 VALUES (?, ?, 'Settimana', 'settimana', '2026-09-21', '2026-09-27')",
            )
            .bind(user_id)
            .bind(space_id)
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
            sqlx::query(
                "INSERT INTO planner_pasti \
                 (planner_id, data_pasto, tipo_pasto, ricetta_nome_snapshot, \
                  ricetta_porzione_base_snapshot) \
                 VALUES (?, '2026-09-27', 'pranzo', 'Pasta', 1)",
            )
            .bind(planner_id)
            .execute(&pool)
            .await
            .unwrap();

            let blocco = info_giorno_per_planner(&pool, "2026-09-27")
                .await
                .expect("blocco ancora presente per la cena");
            assert!(!blocco.contains("Pranzo"));
            assert!(blocco.contains("Cena"));
        })
        .await;
    }

    #[tokio::test]
    async fn nessuna_assegnazione_non_produce_blocco() {
        let pool = test_pool().await;
        let (user_id, space_id) = setup(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let blocco = info_giorno_per_planner(&pool, "2026-09-27").await;
            assert!(blocco.is_none());
        })
        .await;
    }
}
