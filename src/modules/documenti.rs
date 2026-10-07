//! 📄 Documenti (7 ottobre 2026, `docs/moduli/documenti.md`).
//!
//! Un documento ha un posto fisico (casa → stanza → contenitore, gli stessi
//! degli Oggetti), una copia digitale (foto e PDF conservati sull'S9, e un
//! link), delle scadenze (quelle dei Promemoria) e una cartella. È "solo mio"
//! o "dello spazio", scelto documento per documento: uno "solo mio" non
//! compare a nessun altro, in nessuna schermata né ricerca.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use anyhow::Context as _;
use sqlx::SqlitePool;
use teloxide::{
    net::Download,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile},
};

use crate::modules::{
    calendario, liste,
    promemoria::{
        self,
        scadenze::{self, Priorita, PRIORITA},
    },
};

type Bot = crate::context_bot::ContextBot;

const MEDIA_ROOT: &str = "data/media/documenti";

/// Le cartelle con cui parte uno spazio. "Ma quella è la base" (Alessio):
/// si eliminano anche queste.
pub const CARTELLE_INIZIALI: [&str; 8] = [
    "Identità",
    "Casa",
    "Auto",
    "Salute",
    "Lavoro",
    "Tasse",
    "Garanzie e scontrini",
    "Banca e assicurazioni",
];

// ===========================================================================
// Attese, una per chat.
// ===========================================================================

#[derive(Debug, Clone, PartialEq)]
enum Attesa {
    /// Il titolo di un documento nuovo; `cartella` è quella da cui si è
    /// partiti, se si è partiti da una cartella.
    NuovoTitolo { cartella: Option<i64> },
    /// In che cartella va (solo pulsanti).
    NuovaCartellaScelta { titolo: String },
    /// Solo mio o dello spazio (solo pulsanti).
    NuovoPrivato {
        titolo: String,
        cartella: Option<i64>,
    },
    /// Il valore di un campo.
    Campo { id: i64, campo: Campo },
    /// Una foto o un PDF.
    File { id: i64 },
    /// La data di una scadenza nuova del documento.
    ScadenzaData { id: i64 },
    /// La priorità di quella scadenza (solo pulsanti).
    ScadenzaPriorita { id: i64, data: String },
    /// Cosa cercare.
    Cerca,
    /// Il nome di una cartella nuova, dentro `padre`.
    NuovaCartella { padre: Option<i64> },
    /// Il nome nuovo di una cartella.
    RinominaCartella { id: i64 },
}

fn attese() -> &'static Mutex<HashMap<i64, Attesa>> {
    static ATTESE: OnceLock<Mutex<HashMap<i64, Attesa>>> = OnceLock::new();
    ATTESE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn attesa(chat_id: i64) -> Option<Attesa> {
    attese()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&chat_id)
        .cloned()
}

fn aspetta(chat_id: i64, nuova: Attesa) {
    attese()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(chat_id, nuova);
}

/// Chiude l'attesa dei Documenti di questa chat (la chiama chi apre
/// un'altra schermata, come per i Promemoria).
pub fn chiudi_attesa(chat_id: i64) {
    attese()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&chat_id);
}

pub fn attesa_attiva(chat_id: i64) -> bool {
    attesa(chat_id).is_some()
}

// ===========================================================================
// Dominio.
// ===========================================================================

/// I campi di testo di un documento che si scrivono a mano.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Titolo,
    Numero,
    Ente,
    Rilascio,
    Note,
    Link,
    Dettaglio,
}

const CAMPI: [Campo; 7] = [
    Campo::Titolo,
    Campo::Numero,
    Campo::Ente,
    Campo::Rilascio,
    Campo::Note,
    Campo::Link,
    Campo::Dettaglio,
];

impl Campo {
    fn token(self) -> &'static str {
        match self {
            Campo::Titolo => "titolo",
            Campo::Numero => "numero",
            Campo::Ente => "ente",
            Campo::Rilascio => "rilascio",
            Campo::Note => "note",
            Campo::Link => "link",
            Campo::Dettaglio => "dettaglio",
        }
    }

    fn da_token(valore: &str) -> Option<Self> {
        CAMPI.iter().copied().find(|campo| campo.token() == valore)
    }

    fn colonna(self) -> &'static str {
        match self {
            Campo::Titolo => "titolo",
            Campo::Numero => "numero",
            Campo::Ente => "ente",
            Campo::Rilascio => "rilasciato_il",
            Campo::Note => "note",
            Campo::Link => "link",
            Campo::Dettaglio => "posizione_dettaglio",
        }
    }

    fn etichetta(self) -> &'static str {
        match self {
            Campo::Titolo => "✏️ Titolo",
            Campo::Numero => "🔢 Numero",
            Campo::Ente => "🏛 Chi l'ha rilasciato",
            Campo::Rilascio => "📆 Data di rilascio",
            Campo::Note => "📝 Note",
            Campo::Link => "🔗 Link",
            Campo::Dettaglio => "📍 Dettaglio della posizione",
        }
    }

    fn domanda(self) -> &'static str {
        match self {
            Campo::Titolo => "Come si chiama il documento?",
            Campo::Numero => "Il numero del documento?",
            Campo::Ente => "Chi l'ha rilasciato? (es. Comune di Roma, Motorizzazione)",
            Campo::Rilascio => "Quando è stato rilasciato? (es. 15/03/2024)",
            Campo::Note => "Le note?",
            Campo::Link => "Il link alla copia (Drive, email…)?",
            Campo::Dettaglio => "Il dettaglio della posizione? (es. cassetto in alto, busta blu)",
        }
    }
}

/// Un valore scritto per un campo, letto come lo scrive una persona (C20).
/// `Err` con il motivo da dire.
pub fn leggi_campo(campo: Campo, scritto: &str, oggi: &str) -> Result<String, &'static str> {
    let scritto = scritto.trim();
    if scritto.is_empty() {
        return Err("Scrivi qualcosa, oppure usa ➖ Togli.");
    }
    match campo {
        Campo::Rilascio => {
            calendario::leggi_data_scritta(scritto, oggi, calendario::Verso::Passato)
                .ok_or("Non ho capito la data: scrivila così, 15/03/2024.")
        }
        Campo::Link => {
            let link = if scritto.contains("://") {
                scritto.to_string()
            } else {
                format!("https://{scritto}")
            };
            if link.contains(' ') || !link.contains('.') {
                Err("Non sembra un link: incollalo com'è, per esempio drive.google.com/…")
            } else {
                Ok(link)
            }
        }
        _ => Ok(scritto.to_string()),
    }
}

/// Il nome di un file scaricato: niente percorsi né caratteri strani.
fn nome_sicuro(nome: &str) -> String {
    let pulito: String = nome
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let pulito = pulito.trim_matches('.').to_string();
    if pulito.is_empty() {
        "file".to_string()
    } else {
        pulito.chars().take(80).collect()
    }
}

// ===========================================================================
// Database.
// ===========================================================================

fn utente_corrente() -> Option<(i64, i64)> {
    let attore = crate::identity::current_actor_opt()?;
    Some((attore.utente_id?, attore.spazio_id))
}

/// La condizione che decide chi vede un documento `d`: dello spazio, e se è
/// "solo mio" solo il suo proprietario. Due parametri: spazio e utente.
const VISIBILE: &str = "d.spazio_id = ? AND (d.privato = 0 OR d.proprietario_utente_id = ?)";

/// Le cartelle iniziali, una volta sola per spazio.
pub async fn inizia_cartelle(pool: &SqlitePool, spazio_id: i64) -> anyhow::Result<()> {
    let nuova =
        sqlx::query("INSERT INTO documenti_iniziati (spazio_id) VALUES (?) ON CONFLICT DO NOTHING")
            .bind(spazio_id)
            .execute(pool)
            .await
            .context("Impossibile iniziare i documenti")?
            .rows_affected()
            > 0;
    if nuova {
        for nome in CARTELLE_INIZIALI {
            sqlx::query(
                "INSERT INTO cartelle_documenti (spazio_id, nome) VALUES (?, ?) ON CONFLICT DO NOTHING",
            )
            .bind(spazio_id)
            .bind(nome)
            .execute(pool)
            .await
            .context("Impossibile creare le cartelle iniziali")?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Cartella {
    pub id: i64,
    pub padre_id: Option<i64>,
    pub nome: String,
}

pub async fn cartelle(pool: &SqlitePool, spazio_id: i64) -> Vec<Cartella> {
    sqlx::query_as(
        "SELECT id, padre_id, nome FROM cartelle_documenti WHERE spazio_id = ? \
         ORDER BY nome COLLATE NOCASE, id",
    )
    .bind(spazio_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// "Casa › Bollette": il percorso di una cartella.
fn percorso(tutte: &[Cartella], id: i64) -> String {
    let mut nomi = Vec::new();
    let mut corrente = Some(id);
    // Un tetto contro un ciclo che non dovrebbe esistere.
    for _ in 0..20 {
        let Some(cartella) = corrente.and_then(|id| tutte.iter().find(|c| c.id == id)) else {
            break;
        };
        nomi.push(cartella.nome.clone());
        corrente = cartella.padre_id;
    }
    nomi.reverse();
    nomi.join(" › ")
}

/// Se `dentro` sta dentro `cartella` (o è lei): spostare una cartella in una
/// sua sottocartella creerebbe un cerchio.
fn sta_dentro(tutte: &[Cartella], dentro: i64, cartella: i64) -> bool {
    let mut corrente = Some(dentro);
    for _ in 0..20 {
        match corrente {
            Some(id) if id == cartella => return true,
            Some(id) => corrente = tutte.iter().find(|c| c.id == id).and_then(|c| c.padre_id),
            None => return false,
        }
    }
    true
}

pub async fn crea_cartella(
    pool: &SqlitePool,
    spazio_id: i64,
    padre: Option<i64>,
    nome: &str,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO cartelle_documenti (spazio_id, padre_id, nome) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(spazio_id)
    .bind(padre)
    .bind(nome.trim())
    .fetch_one(pool)
    .await
    .context("Impossibile creare la cartella")
}

/// Una cartella si elimina solo vuota: niente documenti (di nessuno, anche
/// quelli "solo miei" di un altro) e niente sottocartelle.
pub async fn elimina_cartella(
    pool: &SqlitePool,
    spazio_id: i64,
    id: i64,
) -> Result<(), &'static str> {
    let documenti: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM documenti WHERE cartella_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap_or(1);
    let sotto: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cartelle_documenti WHERE padre_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap_or(1);
    if documenti > 0 || sotto > 0 {
        return Err("Dentro ci sono ancora documenti o cartelle: spostali prima.");
    }
    sqlx::query("DELETE FROM cartelle_documenti WHERE id = ? AND spazio_id = ?")
        .bind(id)
        .bind(spazio_id)
        .execute(pool)
        .await
        .map_err(|_| "Non sono riuscito a eliminarla.")?;
    Ok(())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Documento {
    pub id: i64,
    pub proprietario_utente_id: i64,
    pub privato: bool,
    pub titolo: String,
    pub cartella_id: Option<i64>,
    pub numero: Option<String>,
    pub ente: Option<String>,
    pub rilasciato_il: Option<String>,
    pub note: Option<String>,
    pub link: Option<String>,
    pub abitazione_id: Option<i64>,
    pub stanza_id: Option<i64>,
    pub contenitore_id: Option<i64>,
    pub posizione_dettaglio: Option<String>,
}

const COLONNE: &str = "d.id, d.proprietario_utente_id, d.privato, d.titolo, d.cartella_id, \
     d.numero, d.ente, d.rilasciato_il, d.note, d.link, d.abitazione_id, d.stanza_id, \
     d.contenitore_id, d.posizione_dettaglio";

pub async fn crea_documento(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: i64,
    titolo: &str,
    cartella: Option<i64>,
    privato: bool,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO documenti (spazio_id, proprietario_utente_id, privato, titolo, cartella_id) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(spazio_id)
    .bind(utente_id)
    .bind(privato)
    .bind(titolo.trim())
    .bind(cartella)
    .fetch_one(pool)
    .await
    .context("Impossibile salvare il documento")
}

/// Un documento, solo se chi chiede lo può vedere.
pub async fn leggi_documento(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: i64,
    id: i64,
) -> Option<Documento> {
    sqlx::query_as(&format!(
        "SELECT {COLONNE} FROM documenti d WHERE d.id = ? AND {VISIBILE}"
    ))
    .bind(id)
    .bind(spazio_id)
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// I documenti visibili di una cartella (`None` = fuori da ogni cartella).
pub async fn documenti_in(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: i64,
    cartella: Option<i64>,
) -> Vec<Documento> {
    sqlx::query_as(&format!(
        "SELECT {COLONNE} FROM documenti d WHERE d.cartella_id IS ? AND {VISIBILE} \
         ORDER BY d.titolo COLLATE NOCASE, d.id"
    ))
    .bind(cartella)
    .bind(spazio_id)
    .bind(utente_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Cerca nel titolo, nel numero, nell'ente e nelle note, senza badare a
/// maiuscole e accenti scritti diversi (C20).
pub async fn cerca(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: i64,
    testo: &str,
) -> Vec<Documento> {
    let tutti: Vec<Documento> = sqlx::query_as(&format!(
        "SELECT {COLONNE} FROM documenti d WHERE {VISIBILE} ORDER BY d.titolo COLLATE NOCASE"
    ))
    .bind(spazio_id)
    .bind(utente_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let cercato = normalizza(testo);
    if cercato.is_empty() {
        return Vec::new();
    }
    tutti
        .into_iter()
        .filter(|documento| {
            [
                Some(&documento.titolo),
                documento.numero.as_ref(),
                documento.ente.as_ref(),
                documento.note.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|campo| normalizza(campo).contains(&cercato))
        })
        .collect()
}

/// Minuscolo, senza accenti, spazi semplici.
pub fn normalizza(testo: &str) -> String {
    testo
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            altro => altro,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn imposta_campo(
    pool: &SqlitePool,
    id: i64,
    campo: Campo,
    valore: Option<&str>,
) -> anyhow::Result<()> {
    if campo == Campo::Titolo && valore.is_none() {
        anyhow::bail!("Il titolo non si toglie");
    }
    sqlx::query(&format!(
        "UPDATE documenti SET {} = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
        campo.colonna()
    ))
    .bind(valore)
    .bind(id)
    .execute(pool)
    .await
    .context("Impossibile salvare il campo")?;
    Ok(())
}

async fn esegui(pool: &SqlitePool, sql: &str, valori: &[Option<i64>]) -> bool {
    let mut query = sqlx::query(sql);
    for valore in valori {
        query = query.bind(*valore);
    }
    query
        .execute(pool)
        .await
        .map(|esito| esito.rows_affected() > 0)
        .unwrap_or(false)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FileDocumento {
    pub id: i64,
    pub tipo: String,
    pub percorso: String,
    pub nome_originale: Option<String>,
}

pub async fn file_del_documento(pool: &SqlitePool, documento_id: i64) -> Vec<FileDocumento> {
    sqlx::query_as(
        "SELECT id, tipo, percorso, nome_originale FROM documenti_file \
         WHERE documento_id = ? ORDER BY id",
    )
    .bind(documento_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Registra un file già scaricato in `cartella/<documento>/`.
pub async fn registra_file(
    pool: &SqlitePool,
    documento_id: i64,
    tipo: &str,
    percorso: &Path,
    nome_originale: Option<&str>,
    telegram_file_id: Option<&str>,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO documenti_file (documento_id, tipo, percorso, nome_originale, telegram_file_id) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(documento_id)
    .bind(tipo)
    .bind(percorso.to_string_lossy().into_owned())
    .bind(nome_originale)
    .bind(telegram_file_id)
    .fetch_one(pool)
    .await
    .context("Impossibile registrare il file")
}

/// Le scadenze di un documento.
pub async fn scadenze_del_documento(
    pool: &SqlitePool,
    documento_id: i64,
) -> Vec<(i64, String, String, String)> {
    sqlx::query_as(
        "SELECT id, data, priorita, stato FROM scadenze WHERE documento_id = ? \
         ORDER BY stato = 'fatta', data",
    )
    .bind(documento_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Dov'è, scritto per intero: "Casa › Studio › Cassettiera › Cassetto alto".
async fn dove_e(pool: &SqlitePool, documento: &Documento) -> Option<String> {
    let mut parti = Vec::new();
    if let Some(casa) = documento.abitazione_id {
        let nome: Option<String> = sqlx::query_scalar("SELECT nome FROM abitazioni WHERE id = ?")
            .bind(casa)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
        parti.extend(nome);
    }
    if let Some(stanza) = documento.stanza_id {
        let nome: Option<String> = sqlx::query_scalar("SELECT nome FROM stanze WHERE id = ?")
            .bind(stanza)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
        parti.extend(nome);
    }
    if let Some(contenitore) = documento.contenitore_id {
        let catena: Vec<String> = sqlx::query_scalar(
            "WITH RECURSIVE catena(id, nome, padre, giro) AS ( \
                SELECT id, nome, contenitore_padre_id, 0 FROM contenitori WHERE id = ? \
                UNION ALL \
                SELECT c.id, c.nome, c.contenitore_padre_id, catena.giro + 1 \
                FROM contenitori c JOIN catena ON catena.padre = c.id WHERE catena.giro < 20 \
             ) SELECT nome FROM catena ORDER BY giro DESC",
        )
        .bind(contenitore)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        parti.extend(catena);
    }
    if let Some(dettaglio) = &documento.posizione_dettaglio {
        parti.push(dettaglio.clone());
    }
    (!parti.is_empty()).then(|| parti.join(" › "))
}

// ===========================================================================
// Schermate.
// ===========================================================================

fn button(etichetta: impl Into<String>, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(etichetta.into(), data.into())
}

fn nav_row(indietro: &str) -> Vec<InlineKeyboardButton> {
    vec![
        button("⬅️ Indietro", indietro),
        button("🏠 Menù principale", "menu:main"),
    ]
}

fn con_avviso(avviso: Option<&str>, testo: &str) -> String {
    match avviso {
        Some(avviso) => format!("{avviso}\n\n{testo}"),
        None => testo.to_string(),
    }
}

fn etichetta_documento(documento: &Documento) -> String {
    let segno = if documento.privato { "🔒" } else { "📄" };
    format!("{segno} {}", liste::tronca(&documento.titolo, 34))
}

async fn conta_in(pool: &SqlitePool, utente_id: i64, spazio_id: i64, cartella: i64) -> i64 {
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM documenti d WHERE d.cartella_id = ? AND {VISIBILE}"
    ))
    .bind(cartella)
    .bind(spazio_id)
    .bind(utente_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}

pub async fn mostra_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    // In una scatola: `mostra_cartella` torna qui quando la cartella non
    // c'è più, e un `async fn` ricorsivo deve passare da `Box::pin`.
    Box::pin(mostra_cartella(bot, chat_id, pool, None, 0, avviso)).await
}

/// Una cartella (o la radice, con `None`): le sottocartelle, poi i
/// documenti.
async fn mostra_cartella(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    cartella: Option<i64>,
    pagina: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    if let Err(errore) = inizia_cartelle(pool, spazio_id).await {
        tracing::warn!(?errore, "Cartelle iniziali dei documenti non create");
    }
    let tutte = cartelle(pool, spazio_id).await;
    if let Some(id) = cartella {
        if !tutte.iter().any(|c| c.id == id) {
            return mostra_menu(bot, chat_id, pool, Some("⚠️ Questa cartella non c'è più.")).await;
        }
    }
    let sotto: Vec<&Cartella> = tutte.iter().filter(|c| c.padre_id == cartella).collect();
    let documenti = documenti_in(pool, utente_id, spazio_id, cartella).await;
    let titolo = match cartella {
        Some(id) => format!("📁 {}", percorso(&tutte, id)),
        None => "📄 Documenti".to_string(),
    };
    let totale = documenti.len() as i64;
    let pagina = liste::pagina_valida(pagina, totale);
    let mut testo = titolo;
    if sotto.is_empty() && documenti.is_empty() {
        testo.push_str("\n\nQui non c'è ancora niente.");
    } else if cartella.is_none() {
        testo.push_str("\n\n🔒 = solo tuo, nessun altro lo vede. 📄 = dello spazio.");
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    if pagina == 0 {
        for dentro in &sotto {
            let quanti = conta_in(pool, utente_id, spazio_id, dentro.id).await;
            rows.push(vec![button(
                format!("📁 {} · {quanti}", dentro.nome),
                format!("doc:dir:{}:0", dentro.id),
            )]);
        }
    }
    for documento in documenti
        .iter()
        .skip(liste::scarto(pagina) as usize)
        .take(liste::VOCI_PER_PAGINA)
    {
        rows.push(vec![button(
            etichetta_documento(documento),
            format!("doc:view:{}", documento.id),
        )]);
    }
    let qui = cartella.unwrap_or(0);
    if let Some(riga) = liste::riga_paginazione_da_totale(pagina, totale, "doc:noop", |p| {
        format!("doc:dir:{qui}:{p}")
    }) {
        rows.push(riga);
    }
    rows.push(vec![button("➕ Nuovo documento", format!("doc:new:{qui}"))]);
    match cartella {
        None => {
            rows.push(vec![
                button("🔎 Cerca", "doc:search"),
                button("📅 In scadenza", "doc:due"),
            ]);
            rows.push(vec![button("📁 Gestisci le cartelle", "doc:dirs")]);
            rows.push(vec![button("🏠 Menù principale", "menu:main")]);
        }
        Some(id) => {
            let padre = tutte
                .iter()
                .find(|c| c.id == id)
                .and_then(|c| c.padre_id)
                .map(|padre| format!("doc:dir:{padre}:0"))
                .unwrap_or_else(|| "doc:menu".to_string());
            rows.push(nav_row(&padre));
        }
    }
    bot.send_message(chat_id, con_avviso(avviso, &testo))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_documento(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let Some(documento) = leggi_documento(pool, utente_id, spazio_id, id).await else {
        return mostra_menu(bot, chat_id, pool, Some("⚠️ Questo documento non c'è più.")).await;
    };
    let tutte = cartelle(pool, spazio_id).await;
    let mut righe = vec![format!("📄 {}", documento.titolo)];
    let cartella = documento
        .cartella_id
        .map(|c| percorso(&tutte, c))
        .unwrap_or_else(|| "nessuna cartella".to_string());
    let chi = if documento.privato {
        "🔒 Solo tuo"
    } else {
        "👥 Dello spazio"
    };
    righe.push(format!("📁 {cartella} · {chi}"));
    let mut dati = Vec::new();
    if let Some(numero) = &documento.numero {
        dati.push(format!("🔢 {numero}"));
    }
    if let Some(ente) = &documento.ente {
        dati.push(format!("🏛 {ente}"));
    }
    if let Some(data) = &documento.rilasciato_il {
        dati.push(format!("📆 Rilasciato {}", calendario::display_date(data)));
    }
    if !dati.is_empty() {
        righe.push(dati.join("\n"));
    }
    righe.push(match dove_e(pool, &documento).await {
        Some(dove) => format!("📍 {dove}"),
        None => "📍 Dov'è: non detto".to_string(),
    });
    let file = file_del_documento(pool, id).await;
    let copia = match (file.len(), &documento.link) {
        (0, None) => "📎 Nessuna copia digitale".to_string(),
        (0, Some(link)) => format!("🔗 {link}"),
        (n, None) => format!("📎 {n} file"),
        (n, Some(link)) => format!("📎 {n} file · 🔗 {link}"),
    };
    righe.push(copia);
    let oggi = promemoria::adesso_locale(pool).await.date();
    let scadenze = scadenze_del_documento(pool, id).await;
    for (_, data, priorita, _) in scadenze.iter().filter(|s| s.3 == "aperta").take(3) {
        let giorni = calendario::parse_date(data)
            .map(|d| (d - oggi).num_days())
            .unwrap_or(0);
        righe.push(format!(
            "📅 {} Scade {}, {}",
            Priorita::da_token(priorita)
                .unwrap_or(Priorita::Media)
                .emoji(),
            calendario::display_date(data),
            scadenze::quanto_manca(giorni)
        ));
    }
    if let Some(note) = &documento.note {
        righe.push(format!("📝 {note}"));
    }

    let mut rows = Vec::new();
    if !file.is_empty() {
        rows.push(vec![button(
            "📎 Mostra la copia",
            format!("doc:showall:{id}"),
        )]);
    }
    rows.push(vec![
        button("➕ Foto o PDF", format!("doc:addfile:{id}")),
        button("🔗 Link", format!("doc:field:{id}:link")),
    ]);
    rows.push(vec![
        button("📅 Scadenze", format!("doc:scad:{id}")),
        button("📍 Dov'è", format!("doc:pos:{id}")),
    ]);
    rows.push(vec![
        button("✏️ Modifica", format!("doc:edit:{id}")),
        button("📁 Sposta", format!("doc:move:{id}")),
    ]);
    let mut ultima = Vec::new();
    if documento.proprietario_utente_id == utente_id {
        ultima.push(if documento.privato {
            button("👥 Rendi dello spazio", format!("doc:priv:{id}"))
        } else {
            button("🔒 Rendi solo mio", format!("doc:priv:{id}"))
        });
    }
    ultima.push(button("🗑️ Elimina", format!("doc:del:ask:{id}")));
    rows.push(ultima);
    let indietro = documento
        .cartella_id
        .map(|c| format!("doc:dir:{c}:0"))
        .unwrap_or_else(|| "doc:menu".to_string());
    rows.push(nav_row(&indietro));
    bot.send_message(chat_id, con_avviso(avviso, &righe.join("\n")))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn chiedi_campo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    campo: Campo,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let Some(documento) = leggi_documento(pool, utente_id, spazio_id, id).await else {
        return mostra_menu(bot, chat_id, pool, None).await;
    };
    aspetta(chat_id.0, Attesa::Campo { id, campo });
    let attuale = match campo {
        Campo::Titolo => Some(documento.titolo.clone()),
        Campo::Numero => documento.numero.clone(),
        Campo::Ente => documento.ente.clone(),
        Campo::Rilascio => documento
            .rilasciato_il
            .as_deref()
            .map(calendario::display_date),
        Campo::Note => documento.note.clone(),
        Campo::Link => documento.link.clone(),
        Campo::Dettaglio => documento.posizione_dettaglio.clone(),
    };
    let mut testo = format!("{}\n\n{}", campo.etichetta(), campo.domanda());
    if let Some(attuale) = &attuale {
        testo.push_str(&format!("\n\nAdesso: {attuale}"));
    }
    let mut rows = Vec::new();
    if attuale.is_some() && campo != Campo::Titolo {
        rows.push(vec![button(
            "➖ Togli",
            format!("doc:clear:{id}:{}", campo.token()),
        )]);
    }
    rows.push(vec![
        button("❌ Annulla", format!("doc:view:{id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, con_avviso(avviso, &testo))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_modifica(bot: &Bot, chat_id: ChatId, id: i64) -> ResponseResult<()> {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = CAMPI
        .iter()
        .filter(|campo| **campo != Campo::Link)
        .map(|campo| {
            vec![button(
                campo.etichetta(),
                format!("doc:field:{id}:{}", campo.token()),
            )]
        })
        .collect();
    rows.push(nav_row(&format!("doc:view:{id}")));
    bot.send_message(chat_id, "✏️ Cosa cambio?")
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Le cartelle come scelta: "Casa › Bollette", più "Nessuna cartella".
fn scelta_cartelle(
    tutte: &[Cartella],
    callback: impl Fn(i64) -> String,
    escludi: Option<i64>,
) -> Vec<Vec<InlineKeyboardButton>> {
    let mut voci: Vec<(String, i64)> = tutte
        .iter()
        .filter(|c| match escludi {
            None => true,
            Some(esclusa) => !sta_dentro(tutte, c.id, esclusa),
        })
        .map(|c| (percorso(tutte, c.id), c.id))
        .collect();
    voci.sort_by_key(|(nome, _)| normalizza(nome));
    let mut rows: Vec<Vec<InlineKeyboardButton>> = voci
        .into_iter()
        .take(40)
        .map(|(nome, id)| vec![button(format!("📁 {nome}"), callback(id))])
        .collect();
    rows.push(vec![button("➖ Nessuna cartella", callback(0))]);
    rows
}

async fn mostra_file(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let file = file_del_documento(pool, id).await;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = file
        .iter()
        .enumerate()
        .map(|(n, f)| {
            let nome = match (&f.tipo[..], &f.nome_originale) {
                ("pdf", Some(nome)) => format!("📄 {}", liste::tronca(nome, 24)),
                ("pdf", None) => format!("📄 PDF {}", n + 1),
                _ => format!("🖼 Foto {}", n + 1),
            };
            vec![
                button(format!("👁 {nome}"), format!("doc:show:{}", f.id)),
                button("🗑️", format!("doc:rmfile:ask:{}", f.id)),
            ]
        })
        .collect();
    rows.push(vec![button("➕ Foto o PDF", format!("doc:addfile:{id}"))]);
    rows.push(nav_row(&format!("doc:view:{id}")));
    let testo = if file.is_empty() {
        "📎 Copia digitale\n\nNessun file.".to_string()
    } else {
        format!(
            "📎 Copia digitale\n\n{} file. 👁 per vederlo, 🗑️ per toglierlo.",
            file.len()
        )
    };
    bot.send_message(chat_id, con_avviso(avviso, &testo))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Manda un file conservato: le foto come foto, i PDF come documento.
/// Spariscono alla prossima interazione, come le foto degli Oggetti.
async fn manda_file(bot: &Bot, chat_id: ChatId, file: &FileDocumento) {
    let percorso = PathBuf::from(&file.percorso);
    if !percorso.exists() {
        bot.avviso_che_sparisce(
            chat_id,
            "⚠️ Questo file non è più sull'S9.",
            std::time::Duration::from_secs(5),
        )
        .await;
        return;
    }
    if file.tipo == "foto" {
        if let Err(errore) = bot.send_photo(chat_id, InputFile::file(percorso)).await {
            tracing::warn!(?errore, "Foto del documento non mandata");
        }
    } else {
        match bot
            .send_document_untracked(chat_id, InputFile::file(percorso))
            .await
        {
            Ok(messaggio) => bot.mark_transient_message(chat_id.0, messaggio.id),
            Err(errore) => tracing::warn!(?errore, "PDF del documento non mandato"),
        }
    }
}

async fn mostra_posizione(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    livello: Livello,
) -> ResponseResult<()> {
    let Some((_, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let (testo, voci, qui): (&str, Vec<(i64, String)>, Option<String>) = match livello {
        Livello::Case => {
            let case: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, nome FROM abitazioni WHERE spazio_id = ? ORDER BY nome COLLATE NOCASE",
            )
            .bind(spazio_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            ("📍 Dov'è? In che casa?", case, None)
        }
        Livello::Stanze(casa) => {
            let stanze: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, nome FROM stanze WHERE abitazione_id = ? ORDER BY nome COLLATE NOCASE",
            )
            .bind(casa)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            (
                "📍 In che stanza?",
                stanze,
                Some(format!("doc:posset:{id}:{casa}:0:0")),
            )
        }
        Livello::Contenitori(casa, stanza) => {
            let dentro: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, nome FROM contenitori WHERE abitazione_id = ? AND stanza_id IS ? \
                 AND contenitore_padre_id IS NULL ORDER BY nome COLLATE NOCASE",
            )
            .bind(casa)
            .bind(stanza)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            (
                "📍 In che contenitore?",
                dentro,
                Some(format!("doc:posset:{id}:{casa}:{}:0", stanza.unwrap_or(0))),
            )
        }
        Livello::Dentro(casa, stanza, contenitore) => {
            let dentro: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, nome FROM contenitori WHERE contenitore_padre_id = ? ORDER BY nome COLLATE NOCASE",
            )
            .bind(contenitore)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            (
                "📍 Più precisamente?",
                dentro,
                Some(format!(
                    "doc:posset:{id}:{casa}:{}:{contenitore}",
                    stanza.unwrap_or(0)
                )),
            )
        }
    };
    let mut rows = Vec::new();
    if let Some(qui) = qui {
        rows.push(vec![button("📍 Qui", qui)]);
    }
    for (voce, nome) in &voci {
        let callback = match livello {
            Livello::Case => format!("doc:posh:{id}:{voce}"),
            Livello::Stanze(casa) => format!("doc:poss:{id}:{casa}:{voce}"),
            Livello::Contenitori(casa, stanza) | Livello::Dentro(casa, stanza, _) => {
                format!("doc:posc:{id}:{casa}:{}:{voce}", stanza.unwrap_or(0))
            }
        };
        let segno = match livello {
            Livello::Case => "🏠",
            Livello::Stanze(_) => "🚪",
            _ => "📦",
        };
        rows.push(vec![button(format!("{segno} {nome}"), callback)]);
    }
    let mut testo = testo.to_string();
    if voci.is_empty() && matches!(livello, Livello::Case) {
        testo.push_str("\n\nNon c'è ancora nessuna casa: creala da 🏠 Case, stanze e contenitori.");
    }
    rows.push(vec![button(
        "➖ Togli la posizione",
        format!("doc:posclear:{id}"),
    )]);
    rows.push(nav_row(&format!("doc:view:{id}")));
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Livello {
    Case,
    Stanze(i64),
    Contenitori(i64, Option<i64>),
    Dentro(i64, Option<i64>, i64),
}

async fn mostra_scadenze_documento(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let oggi = promemoria::adesso_locale(pool).await.date();
    let scadenze = scadenze_del_documento(pool, id).await;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = scadenze
        .iter()
        .map(|(scadenza, data, priorita, stato)| {
            let giorni = calendario::parse_date(data)
                .map(|d| (d - oggi).num_days())
                .unwrap_or(0);
            let segno = if stato == "fatta" {
                "✅".to_string()
            } else {
                Priorita::da_token(priorita)
                    .unwrap_or(Priorita::Media)
                    .emoji()
                    .to_string()
            };
            let quando = if stato == "fatta" {
                "fatta".to_string()
            } else {
                scadenze::quanto_manca(giorni)
            };
            vec![button(
                format!("{segno} {} · {quando}", calendario::display_date(data)),
                format!("remind:scad:view:{scadenza}"),
            )]
        })
        .collect();
    rows.push(vec![button(
        "➕ Aggiungi una scadenza",
        format!("doc:scadadd:{id}"),
    )]);
    rows.push(nav_row(&format!("doc:view:{id}")));
    let testo = if scadenze.is_empty() {
        "📅 Scadenze del documento\n\nNessuna. Una scadenza ti avvisa più volte man mano che si avvicina, secondo la sua priorità.".to_string()
    } else {
        "📅 Scadenze del documento\n\nToccane una per cambiarla o segnarla fatta (si apre in ⏰ Promemoria).".to_string()
    };
    bot.send_message(chat_id, con_avviso(avviso, &testo))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_in_scadenza(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let adesso = promemoria::adesso_locale(pool).await;
    let oggi = adesso.date();
    let limite = calendario::format_date(oggi + chrono::Duration::days(60));
    let righe: Vec<(i64, String, bool, String, String)> = sqlx::query_as(&format!(
        "SELECT d.id, d.titolo, d.privato, s.data, s.priorita FROM documenti d \
         JOIN scadenze s ON s.documento_id = d.id \
         WHERE s.stato = 'aperta' AND s.data <= ? AND {VISIBILE} ORDER BY s.data, d.titolo"
    ))
    .bind(&limite)
    .bind(spazio_id)
    .bind(utente_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut rows: Vec<Vec<InlineKeyboardButton>> = righe
        .iter()
        .take(30)
        .map(|(id, titolo, privato, data, priorita)| {
            let giorni = calendario::parse_date(data)
                .map(|d| (d - oggi).num_days())
                .unwrap_or(0);
            vec![button(
                format!(
                    "{} {}{} · {}",
                    Priorita::da_token(priorita)
                        .unwrap_or(Priorita::Media)
                        .emoji(),
                    if *privato { "🔒 " } else { "" },
                    liste::tronca(titolo, 24),
                    scadenze::quanto_manca(giorni)
                ),
                format!("doc:view:{id}"),
            )]
        })
        .collect();
    rows.push(nav_row("doc:menu"));
    let testo = if righe.is_empty() {
        "📅 In scadenza\n\nNessun documento scade nei prossimi 60 giorni."
    } else {
        "📅 In scadenza\n\nI documenti che scadono nei prossimi 60 giorni, e quelli già scaduti."
    };
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_gestione_cartelle(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((_, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let tutte = cartelle(pool, spazio_id).await;
    let mut rows = scelta_cartelle(&tutte, |id| format!("doc:dirman:{id}"), None);
    rows.pop(); // "Nessuna cartella" qui non serve.
    rows.push(vec![button("➕ Nuova cartella", "doc:dirnew:0")]);
    rows.push(nav_row("doc:menu"));
    bot.send_message(
        chat_id,
        con_avviso(
            avviso,
            "📁 Le cartelle\n\nToccane una per rinominarla, spostarla, crearci dentro una sottocartella o eliminarla. Anche quelle già pronte.",
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn mostra_cartella_da_gestire(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((_, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let tutte = cartelle(pool, spazio_id).await;
    if !tutte.iter().any(|c| c.id == id) {
        return mostra_gestione_cartelle(
            bot,
            chat_id,
            pool,
            Some("⚠️ Questa cartella non c'è più."),
        )
        .await;
    }
    let rows = vec![
        vec![
            button("✏️ Rinomina", format!("doc:dirren:{id}")),
            button("➕ Sottocartella", format!("doc:dirnew:{id}")),
        ],
        vec![
            button("📂 Sposta", format!("doc:dirmv:{id}")),
            button("🗑️ Elimina", format!("doc:dirdel:ask:{id}")),
        ],
        nav_row("doc:dirs"),
    ];
    bot.send_message(
        chat_id,
        con_avviso(avviso, &format!("📁 {}", percorso(&tutte, id))),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

fn conferma(testo: String, si: String, no: String) -> (String, InlineKeyboardMarkup) {
    (
        testo,
        InlineKeyboardMarkup::new(vec![
            vec![button("✅ Sì, elimina", si)],
            vec![
                button("❌ Annulla", no),
                button("🏠 Menù principale", "menu:main"),
            ],
        ]),
    )
}

// ===========================================================================
// Ingressi.
// ===========================================================================

async fn crea_e_mostra(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    titolo: &str,
    cartella: Option<i64>,
    privato: bool,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    match crea_documento(pool, utente_id, spazio_id, titolo, cartella, privato).await {
        Ok(id) => mostra_documento(
            bot,
            chat_id,
            pool,
            id,
            Some(
                "✅ Documento salvato. Ora puoi aggiungere la foto o il PDF, dov'è e le scadenze.",
            ),
        )
        .await,
        Err(errore) => {
            tracing::warn!(?errore, "Documento non salvato");
            mostra_menu(bot, chat_id, pool, Some("⚠️ Non sono riuscito a salvarlo.")).await
        }
    }
}

async fn chiedi_privato(bot: &Bot, chat_id: ChatId, titolo: &str) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "Chi lo vede?\n\n«{titolo}»\n\n🔒 Solo tu: nessun altro membro dello spazio lo vede.\n👥 Tutto lo spazio: lo vedono tutti i membri."
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![
            button("🔒 Solo io", "doc:newpriv:1"),
            button("👥 Tutto lo spazio", "doc:newpriv:0"),
        ],
        vec![
            button("❌ Annulla", "doc:menu"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ]))
    .await?;
    Ok(())
}

/// Un file mandato mentre i Documenti lo aspettano. `false` se non lo
/// aspettano: il messaggio è di qualcun altro.
pub async fn handle_file(bot: &Bot, msg: &Message, pool: &SqlitePool) -> ResponseResult<bool> {
    let chat_id = msg.chat.id;
    let Some(Attesa::File { id }) = attesa(chat_id.0) else {
        return Ok(false);
    };
    let (file_id, tipo, nome_originale) = if let Some(foto) = msg.photo().and_then(|foto| {
        foto.iter()
            .max_by_key(|f| u64::from(f.width) * u64::from(f.height))
    }) {
        (foto.file.id.clone(), "foto", None)
    } else if let Some(documento) = msg.document() {
        let mime = documento
            .mime_type
            .as_ref()
            .map(|m| m.essence_str().to_string())
            .unwrap_or_default();
        let nome = documento.file_name.clone();
        let tipo = if mime == "application/pdf"
            || nome
                .as_deref()
                .is_some_and(|n| n.to_lowercase().ends_with(".pdf"))
        {
            "pdf"
        } else if mime.starts_with("image/") {
            "foto"
        } else {
            bot.send_message(
                chat_id,
                "📎 Qui vanno foto o PDF: questo file non è né l'una né l'altro.",
            )
            .await?;
            return Ok(true);
        };
        (documento.file.id.clone(), tipo, nome)
    } else {
        return Ok(false);
    };
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(true);
    };
    if leggi_documento(pool, utente_id, spazio_id, id)
        .await
        .is_none()
    {
        chiudi_attesa(chat_id.0);
        bot.send_message(chat_id, "⚠️ Il documento non c'è più.")
            .await?;
        return Ok(true);
    }
    let telegram_file = match bot.get_file(file_id.clone()).await {
        Ok(file) => file,
        Err(errore) => {
            tracing::warn!(?errore, "File del documento non leggibile");
            bot.send_message(
                chat_id,
                "⚠️ Telegram non mi dà questo file (oltre i 20 MB non si può). Riprova con uno più piccolo.",
            )
            .await?;
            return Ok(true);
        }
    };
    let cartella = PathBuf::from(MEDIA_ROOT).join(id.to_string());
    if let Err(errore) = tokio::fs::create_dir_all(&cartella).await {
        tracing::error!(?errore, "Cartella dei documenti non creata");
        bot.send_message(chat_id, "⚠️ Non riesco a salvare il file sull'S9.")
            .await?;
        return Ok(true);
    }
    let estensione = if tipo == "pdf" { "pdf" } else { "jpg" };
    let nome_file = match &nome_originale {
        Some(nome) if tipo == "pdf" => format!("{}_{}", msg.id.0, nome_sicuro(nome)),
        _ => format!("{}.{estensione}", msg.id.0),
    };
    let percorso = cartella.join(nome_file);
    let scaricato = async {
        let mut destinazione = tokio::fs::File::create(&percorso).await.ok()?;
        bot.download_file(&telegram_file.path, &mut destinazione)
            .await
            .ok()
    }
    .await;
    if scaricato.is_none() {
        let _ = tokio::fs::remove_file(&percorso).await;
        bot.send_message(chat_id, "⚠️ Non sono riuscito a scaricarlo. Riprova.")
            .await?;
        return Ok(true);
    }
    if let Err(errore) = registra_file(
        pool,
        id,
        tipo,
        &percorso,
        nome_originale.as_deref(),
        Some(&file_id.0),
    )
    .await
    {
        tracing::error!(?errore, "File del documento non registrato");
        let _ = tokio::fs::remove_file(&percorso).await;
        bot.send_message(chat_id, "⚠️ Non sono riuscito a registrarlo.")
            .await?;
        return Ok(true);
    }
    bot.delete_user_input(chat_id, msg.id).await;
    let quanti = file_del_documento(pool, id).await.len();
    // L'attesa resta: fronte e retro si mandano uno dopo l'altro.
    aspetta(chat_id.0, Attesa::File { id });
    bot.send_message(
        chat_id,
        format!(
            "✅ {} salvat{} ({quanti} in tutto). Mandane un altro, oppure ✅ Fine.",
            if tipo == "pdf" { "PDF" } else { "Foto" },
            if tipo == "pdf" { "o" } else { "a" }
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![vec![button(
        "✅ Fine",
        format!("doc:view:{id}"),
    )]]))
    .await?;
    Ok(true)
}

/// Il testo scritto mentre i Documenti aspettano qualcosa.
pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id;
    let Some(attesa_aperta) = attesa(chat_id.0) else {
        return Ok(false);
    };
    if text.trim_start().starts_with('/') {
        return Ok(false);
    }
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(false);
    };
    let scritto = text.trim();
    match attesa_aperta {
        Attesa::NuovoTitolo { cartella } => {
            if scritto.is_empty() {
                return Ok(true);
            }
            match cartella {
                Some(cartella) => {
                    aspetta(
                        chat_id.0,
                        Attesa::NuovoPrivato {
                            titolo: scritto.to_string(),
                            cartella: Some(cartella),
                        },
                    );
                    chiedi_privato(bot, chat_id, scritto).await?;
                }
                None => {
                    aspetta(
                        chat_id.0,
                        Attesa::NuovaCartellaScelta {
                            titolo: scritto.to_string(),
                        },
                    );
                    let tutte = cartelle(pool, spazio_id).await;
                    let mut rows = scelta_cartelle(&tutte, |id| format!("doc:newdir:{id}"), None);
                    rows.push(vec![
                        button("❌ Annulla", "doc:menu"),
                        button("🏠 Menù principale", "menu:main"),
                    ]);
                    bot.send_message(chat_id, format!("📁 In che cartella?\n\n«{scritto}»"))
                        .reply_markup(InlineKeyboardMarkup::new(rows))
                        .await?;
                }
            }
        }
        Attesa::NuovaCartellaScelta { .. }
        | Attesa::NuovoPrivato { .. }
        | Attesa::ScadenzaPriorita { .. } => {
            bot.send_message(chat_id, "Scegli con i pulsanti del messaggio sopra.")
                .await?;
        }
        Attesa::Campo { id, campo } => {
            let oggi = calendario::oggi_locale(pool).await;
            match leggi_campo(campo, scritto, &oggi) {
                Ok(valore) => {
                    let avviso = match imposta_campo(pool, id, campo, Some(&valore)).await {
                        Ok(()) => "✅ Salvato.",
                        Err(_) => "⚠️ Non sono riuscito a salvarlo.",
                    };
                    mostra_documento(bot, chat_id, pool, id, Some(avviso)).await?;
                }
                Err(motivo) => {
                    chiedi_campo(bot, chat_id, pool, id, campo, Some(&format!("⚠️ {motivo}")))
                        .await?
                }
            }
        }
        Attesa::File { id } => {
            bot.send_message(chat_id, "📎 Sto aspettando una foto o un PDF.")
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![button(
                    "✅ Fine",
                    format!("doc:view:{id}"),
                )]]))
                .await?;
        }
        Attesa::ScadenzaData { id } => {
            let adesso = promemoria::adesso_locale(pool).await;
            match scadenze::leggi_data(scritto, adesso) {
                Some(giorno) if giorno >= adesso.date() => {
                    let data = calendario::format_date(giorno);
                    aspetta(
                        chat_id.0,
                        Attesa::ScadenzaPriorita {
                            id,
                            data: data.clone(),
                        },
                    );
                    chiedi_priorita_scadenza(bot, chat_id, id, &data).await?;
                }
                Some(_) => {
                    chiedi_data_scadenza(
                        bot,
                        chat_id,
                        id,
                        Some("⚠️ È già passata: scrivi una data che deve ancora venire."),
                    )
                    .await?
                }
                None => {
                    chiedi_data_scadenza(bot, chat_id, id, Some("⚠️ Non ho capito la data."))
                        .await?
                }
            }
        }
        Attesa::Cerca => {
            let trovati = cerca(pool, utente_id, spazio_id, scritto).await;
            let mut rows: Vec<Vec<InlineKeyboardButton>> = trovati
                .iter()
                .take(30)
                .map(|d| vec![button(etichetta_documento(d), format!("doc:view:{}", d.id))])
                .collect();
            rows.push(vec![button("🔎 Cerca ancora", "doc:search")]);
            rows.push(nav_row("doc:menu"));
            chiudi_attesa(chat_id.0);
            let testo = if trovati.is_empty() {
                format!("🔎 «{scritto}»\n\nNessun documento.")
            } else {
                format!("🔎 «{scritto}»\n\n{} documenti.", trovati.len())
            };
            bot.send_message(chat_id, testo)
                .reply_markup(InlineKeyboardMarkup::new(rows))
                .await?;
        }
        Attesa::NuovaCartella { padre } => {
            let avviso = match crea_cartella(pool, spazio_id, padre, scritto).await {
                Ok(_) => "✅ Cartella creata.",
                Err(_) => "⚠️ Non l'ho creata: c'è già una cartella con questo nome qui.",
            };
            match padre {
                Some(padre) => {
                    mostra_cartella_da_gestire(bot, chat_id, pool, padre, Some(avviso)).await?
                }
                None => mostra_gestione_cartelle(bot, chat_id, pool, Some(avviso)).await?,
            }
        }
        Attesa::RinominaCartella { id } => {
            let rinominata = sqlx::query(
                "UPDATE cartelle_documenti SET nome = ? WHERE id = ? AND spazio_id = ?",
            )
            .bind(scritto)
            .bind(id)
            .bind(spazio_id)
            .execute(pool)
            .await
            .is_ok();
            let avviso = if rinominata {
                "✅ Rinominata."
            } else {
                "⚠️ Non l'ho rinominata: c'è già una cartella con questo nome qui."
            };
            mostra_cartella_da_gestire(bot, chat_id, pool, id, Some(avviso)).await?;
        }
    }
    Ok(true)
}

async fn chiedi_data_scadenza(
    bot: &Bot,
    chat_id: ChatId,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    aspetta(chat_id.0, Attesa::ScadenzaData { id });
    bot.send_message(
        chat_id,
        con_avviso(
            avviso,
            "📅 Quando scade?\n\nScrivilo: 15/10/2030, 31 dicembre, fra 2 settimane.",
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", format!("doc:scad:{id}")),
        button("🏠 Menù principale", "menu:main"),
    ]]))
    .await?;
    Ok(())
}

async fn chiedi_priorita_scadenza(
    bot: &Bot,
    chat_id: ChatId,
    id: i64,
    data: &str,
) -> ResponseResult<()> {
    let righe: Vec<String> = PRIORITA
        .iter()
        .map(|p| format!("{} {}: {}.", p.emoji(), p.nome(), p.spiegazione()))
        .collect();
    let mut rows: Vec<Vec<InlineKeyboardButton>> = PRIORITA
        .iter()
        .map(|p| {
            vec![button(
                format!("{} {}", p.emoji(), p.nome()),
                format!("doc:scadprio:{id}:{}", p.token()),
            )]
        })
        .collect();
    rows.push(vec![
        button("❌ Annulla", format!("doc:scad:{id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(
        chat_id,
        format!(
            "🎚 Quanto è importante?\n\nScade {}.\n\n{}",
            calendario::display_date(data),
            righe.join("\n")
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

/// I pulsanti `doc:`. `false` se il callback non è di questo modulo.
pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    let Some(resto) = data.strip_prefix("doc:") else {
        return Ok(false);
    };
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(false);
    };
    // I numeri dopo il prefisso, solo se sono almeno `quanti`: un pulsante
    // malformato non deve mai far cadere il bot con un indice sbagliato.
    let numeri = |prefisso: &str, quanti: usize| -> Option<Vec<i64>> {
        let valori: Vec<i64> = resto
            .strip_prefix(prefisso)?
            .split(':')
            .map(|parte| parte.parse().ok())
            .collect::<Option<Vec<i64>>>()?;
        (valori.len() >= quanti).then_some(valori)
    };
    let opzione = |id: i64| (id > 0).then_some(id);

    match resto {
        "menu" => return mostra_menu(bot, chat_id, pool, None).await.map(|()| true),
        "noop" => return Ok(true),
        "search" => {
            aspetta(chat_id.0, Attesa::Cerca);
            bot.send_message(
                chat_id,
                "🔎 Cosa cerchi? Scrivi una parola del titolo, del numero o di chi l'ha rilasciato.",
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![nav_row("doc:menu")]))
            .await?;
            return Ok(true);
        }
        "due" => return mostra_in_scadenza(bot, chat_id, pool).await.map(|()| true),
        "dirs" => {
            return mostra_gestione_cartelle(bot, chat_id, pool, None)
                .await
                .map(|()| true)
        }
        _ => {}
    }

    // Prima i prefissi più lunghi: "dirman:" prima di "dir:".
    if let Some(v) = numeri("dirman:", 1) {
        mostra_cartella_da_gestire(bot, chat_id, pool, v[0], None).await?;
    } else if let Some(v) = numeri("dirnew:", 1) {
        aspetta(
            chat_id.0,
            Attesa::NuovaCartella {
                padre: opzione(v[0]),
            },
        );
        bot.send_message(chat_id, "📁 Come si chiama la cartella nuova?")
            .reply_markup(InlineKeyboardMarkup::new(vec![nav_row("doc:dirs")]))
            .await?;
    } else if let Some(v) = numeri("dirren:", 1) {
        aspetta(chat_id.0, Attesa::RinominaCartella { id: v[0] });
        bot.send_message(chat_id, "✏️ Il nome nuovo della cartella?")
            .reply_markup(InlineKeyboardMarkup::new(vec![nav_row(&format!(
                "doc:dirman:{}",
                v[0]
            ))]))
            .await?;
    } else if let Some(v) = numeri("dirmvto:", 1) {
        let (id, dove) = (v[0], opzione(v.get(1).copied().unwrap_or(0)));
        let tutte = cartelle(pool, spazio_id).await;
        let avviso = if dove.is_some_and(|dove| sta_dentro(&tutte, dove, id)) {
            "⚠️ Una cartella non può andare dentro sé stessa."
        } else if esegui(
            pool,
            "UPDATE cartelle_documenti SET padre_id = ? WHERE id = ? AND spazio_id = ?",
            &[dove, Some(id), Some(spazio_id)],
        )
        .await
        {
            "✅ Spostata."
        } else {
            "⚠️ Non l'ho spostata: dove va c'è già una cartella con questo nome."
        };
        mostra_cartella_da_gestire(bot, chat_id, pool, id, Some(avviso)).await?;
    } else if let Some(v) = numeri("dirmv:", 1) {
        let tutte = cartelle(pool, spazio_id).await;
        let id = v[0];
        let mut rows = scelta_cartelle(&tutte, |dove| format!("doc:dirmvto:{id}:{dove}"), Some(id));
        if let Some(ultima) = rows.last_mut() {
            *ultima = vec![button(
                "➖ In cima, fuori da tutte",
                format!("doc:dirmvto:{id}:0"),
            )];
        }
        rows.push(nav_row(&format!("doc:dirman:{id}")));
        bot.send_message(chat_id, "📂 Dentro quale cartella?")
            .reply_markup(InlineKeyboardMarkup::new(rows))
            .await?;
    } else if let Some(v) = numeri("dirdel:ask:", 1) {
        let tutte = cartelle(pool, spazio_id).await;
        let (testo, tastiera) = conferma(
            format!(
                "⚠️ Eliminare la cartella «{}» definitivamente? Non si può recuperare.",
                percorso(&tutte, v[0])
            ),
            format!("doc:dirdel:yes:{}", v[0]),
            format!("doc:dirman:{}", v[0]),
        );
        bot.send_message(chat_id, testo)
            .reply_markup(tastiera)
            .await?;
    } else if let Some(v) = numeri("dirdel:yes:", 1) {
        match elimina_cartella(pool, spazio_id, v[0]).await {
            Ok(()) => {
                mostra_gestione_cartelle(bot, chat_id, pool, Some("✅ Cartella eliminata.")).await?
            }
            Err(motivo) => {
                mostra_cartella_da_gestire(bot, chat_id, pool, v[0], Some(&format!("⚠️ {motivo}")))
                    .await?
            }
        }
    } else if let Some(v) = numeri("dir:", 1) {
        let pagina = v.get(1).copied().unwrap_or(0);
        mostra_cartella(bot, chat_id, pool, opzione(v[0]), pagina, None).await?;
    } else if let Some(v) = numeri("new:", 1) {
        let cartella = opzione(v[0]);
        aspetta(chat_id.0, Attesa::NuovoTitolo { cartella });
        bot.send_message(
            chat_id,
            "➕ Nuovo documento\n\nCome si chiama? (es. Carta d'identità, Contratto d'affitto, Libretto auto)",
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button("❌ Annulla", "doc:menu"),
            button("🏠 Menù principale", "menu:main"),
        ]]))
        .await?;
    } else if let Some(v) = numeri("newdir:", 1) {
        let Some(Attesa::NuovaCartellaScelta { titolo }) = attesa(chat_id.0) else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        aspetta(
            chat_id.0,
            Attesa::NuovoPrivato {
                titolo: titolo.clone(),
                cartella: opzione(v[0]),
            },
        );
        chiedi_privato(bot, chat_id, &titolo).await?;
    } else if let Some(v) = numeri("newpriv:", 1) {
        let Some(Attesa::NuovoPrivato { titolo, cartella }) = attesa(chat_id.0) else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        crea_e_mostra(bot, chat_id, pool, &titolo, cartella, v[0] == 1).await?;
    } else if let Some(v) = numeri("view:", 1) {
        mostra_documento(bot, chat_id, pool, v[0], None).await?;
    } else if let Some(v) = numeri("edit:", 1) {
        mostra_modifica(bot, chat_id, v[0]).await?;
    } else if let Some(valori) = resto.strip_prefix("field:") {
        if let Some((id, campo)) = valori.split_once(':') {
            if let (Ok(id), Some(campo)) = (id.parse(), Campo::da_token(campo)) {
                chiedi_campo(bot, chat_id, pool, id, campo, None).await?;
            }
        }
    } else if let Some(valori) = resto.strip_prefix("clear:") {
        if let Some((id, campo)) = valori.split_once(':') {
            if let (Ok(id), Some(campo)) = (id.parse::<i64>(), Campo::da_token(campo)) {
                let visibile = leggi_documento(pool, utente_id, spazio_id, id)
                    .await
                    .is_some();
                let avviso = if visibile && imposta_campo(pool, id, campo, None).await.is_ok() {
                    "✅ Tolto."
                } else {
                    "⚠️ Non sono riuscito a toglierlo."
                };
                mostra_documento(bot, chat_id, pool, id, Some(avviso)).await?;
            }
        }
    } else if let Some(v) = numeri("priv:", 1) {
        let id = v[0];
        let cambiato = esegui(
            pool,
            "UPDATE documenti SET privato = 1 - privato WHERE id = ? AND proprietario_utente_id = ?",
            &[Some(id), Some(utente_id)],
        )
        .await;
        let avviso = match leggi_documento(pool, utente_id, spazio_id, id).await {
            Some(documento) if cambiato && documento.privato => {
                "🔒 Ora è solo tuo: nessun altro lo vede."
            }
            Some(_) if cambiato => "👥 Ora lo vede tutto lo spazio.",
            _ => "⚠️ Solo chi l'ha creato può cambiarlo.",
        };
        mostra_documento(bot, chat_id, pool, id, Some(avviso)).await?;
    } else if let Some(v) = numeri("addfile:", 1) {
        let id = v[0];
        aspetta(chat_id.0, Attesa::File { id });
        bot.send_message(
            chat_id,
            "📎 Mandami la foto o il PDF.\n\nFronte e retro? Mandali uno dopo l'altro.",
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button("❌ Annulla", format!("doc:view:{id}")),
            button("🏠 Menù principale", "menu:main"),
        ]]))
        .await?;
    } else if let Some(v) = numeri("showall:", 1) {
        let id = v[0];
        if leggi_documento(pool, utente_id, spazio_id, id)
            .await
            .is_some()
        {
            for file in file_del_documento(pool, id).await {
                manda_file(bot, chat_id, &file).await;
            }
            mostra_file(bot, chat_id, pool, id, None).await?;
        }
    } else if let Some(v) = numeri("show:", 1) {
        if let Some((documento, file)) = file_visibile(pool, utente_id, spazio_id, v[0]).await {
            manda_file(bot, chat_id, &file).await;
            mostra_file(bot, chat_id, pool, documento, None).await?;
        }
    } else if let Some(v) = numeri("rmfile:ask:", 1) {
        if let Some((documento, _)) = file_visibile(pool, utente_id, spazio_id, v[0]).await {
            let (testo, tastiera) = conferma(
                "⚠️ Togliere questo file definitivamente? Non si può recuperare.".to_string(),
                format!("doc:rmfile:yes:{}", v[0]),
                format!("doc:files:{documento}"),
            );
            bot.send_message(chat_id, testo)
                .reply_markup(tastiera)
                .await?;
        }
    } else if let Some(v) = numeri("rmfile:yes:", 1) {
        if let Some((documento, file)) = file_visibile(pool, utente_id, spazio_id, v[0]).await {
            let _ = tokio::fs::remove_file(&file.percorso).await;
            esegui(
                pool,
                "DELETE FROM documenti_file WHERE id = ?",
                &[Some(file.id)],
            )
            .await;
            mostra_file(bot, chat_id, pool, documento, Some("✅ File tolto.")).await?;
        }
    } else if let Some(v) = numeri("files:", 1) {
        mostra_file(bot, chat_id, pool, v[0], None).await?;
    } else if let Some(v) = numeri("posh:", 2) {
        mostra_posizione(bot, chat_id, pool, v[0], Livello::Stanze(v[1])).await?;
    } else if let Some(v) = numeri("poss:", 3) {
        mostra_posizione(
            bot,
            chat_id,
            pool,
            v[0],
            Livello::Contenitori(v[1], Some(v[2])),
        )
        .await?;
    } else if let Some(v) = numeri("posc:", 4) {
        mostra_posizione(
            bot,
            chat_id,
            pool,
            v[0],
            Livello::Dentro(v[1], opzione(v[2]), v[3]),
        )
        .await?;
    } else if let Some(v) = numeri("posset:", 4) {
        let id = v[0];
        let fatto = leggi_documento(pool, utente_id, spazio_id, id).await.is_some()
            && esegui(
                pool,
                "UPDATE documenti SET abitazione_id = ?, stanza_id = ?, contenitore_id = ? WHERE id = ?",
                &[opzione(v[1]), opzione(v[2]), opzione(v[3]), Some(id)],
            )
            .await;
        let avviso = if fatto {
            "✅ Posizione salvata. Un dettaglio in più (es. busta blu)? ✏️ Modifica → 📍 Dettaglio."
        } else {
            "⚠️ Non sono riuscito a salvarla."
        };
        mostra_documento(bot, chat_id, pool, id, Some(avviso)).await?;
    } else if let Some(v) = numeri("posclear:", 1) {
        let id = v[0];
        if leggi_documento(pool, utente_id, spazio_id, id)
            .await
            .is_some()
        {
            esegui(
                pool,
                "UPDATE documenti SET abitazione_id = NULL, stanza_id = NULL, contenitore_id = NULL, \
                 posizione_dettaglio = NULL WHERE id = ?",
                &[Some(id)],
            )
            .await;
        }
        mostra_documento(bot, chat_id, pool, id, Some("✅ Posizione tolta.")).await?;
    } else if let Some(v) = numeri("pos:", 1) {
        mostra_posizione(bot, chat_id, pool, v[0], Livello::Case).await?;
    } else if let Some(v) = numeri("moveto:", 1) {
        let (id, dove) = (v[0], opzione(v.get(1).copied().unwrap_or(0)));
        let fatto = leggi_documento(pool, utente_id, spazio_id, id)
            .await
            .is_some()
            && esegui(
                pool,
                "UPDATE documenti SET cartella_id = ? WHERE id = ?",
                &[dove, Some(id)],
            )
            .await;
        let avviso = if fatto {
            "✅ Spostato."
        } else {
            "⚠️ Non sono riuscito a spostarlo."
        };
        mostra_documento(bot, chat_id, pool, id, Some(avviso)).await?;
    } else if let Some(v) = numeri("move:", 1) {
        let id = v[0];
        let tutte = cartelle(pool, spazio_id).await;
        let mut rows = scelta_cartelle(&tutte, |dove| format!("doc:moveto:{id}:{dove}"), None);
        rows.push(nav_row(&format!("doc:view:{id}")));
        bot.send_message(chat_id, "📁 In che cartella lo sposto?")
            .reply_markup(InlineKeyboardMarkup::new(rows))
            .await?;
    } else if let Some(v) = numeri("scadadd:", 1) {
        chiedi_data_scadenza(bot, chat_id, v[0], None).await?;
    } else if let Some(valori) = resto.strip_prefix("scadprio:") {
        let Some((id, token)) = valori.split_once(':') else {
            return Ok(true);
        };
        let (Ok(id), Some(priorita)) = (id.parse::<i64>(), Priorita::da_token(token)) else {
            return Ok(true);
        };
        let Some(Attesa::ScadenzaPriorita { id: atteso, data }) = attesa(chat_id.0) else {
            return mostra_scadenze_documento(bot, chat_id, pool, id, None)
                .await
                .map(|()| true);
        };
        let avviso = match leggi_documento(pool, utente_id, spazio_id, id).await {
            Some(documento) if atteso == id => {
                match crea_scadenza_documento(
                    pool, utente_id, spazio_id, &documento, &data, priorita,
                )
                .await
                {
                    Ok(()) => format!("✅ Scadenza salvata: {}.", priorita.spiegazione()),
                    Err(_) => "⚠️ Non sono riuscito a salvarla.".to_string(),
                }
            }
            _ => "⚠️ Non sono riuscito a salvarla.".to_string(),
        };
        mostra_scadenze_documento(bot, chat_id, pool, id, Some(&avviso)).await?;
    } else if let Some(v) = numeri("scad:", 1) {
        mostra_scadenze_documento(bot, chat_id, pool, v[0], None).await?;
    } else if let Some(v) = numeri("del:ask:", 1) {
        let Some(documento) = leggi_documento(pool, utente_id, spazio_id, v[0]).await else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        let (testo, tastiera) = conferma(
            format!(
                "⚠️ Eliminare «{}» definitivamente, con i suoi file e le sue scadenze? Non si può recuperare.",
                documento.titolo
            ),
            format!("doc:del:yes:{}", v[0]),
            format!("doc:view:{}", v[0]),
        );
        bot.send_message(chat_id, testo)
            .reply_markup(tastiera)
            .await?;
    } else if let Some(v) = numeri("del:yes:", 1) {
        let id = v[0];
        let Some(documento) = leggi_documento(pool, utente_id, spazio_id, id).await else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        let eliminato = esegui(pool, "DELETE FROM documenti WHERE id = ?", &[Some(id)]).await;
        if eliminato {
            let _ = tokio::fs::remove_dir_all(PathBuf::from(MEDIA_ROOT).join(id.to_string())).await;
        }
        let avviso = if eliminato {
            "✅ Documento eliminato."
        } else {
            "⚠️ Non sono riuscito a eliminarlo."
        };
        mostra_cartella(bot, chat_id, pool, documento.cartella_id, 0, Some(avviso)).await?;
    } else {
        return Ok(false);
    }
    Ok(true)
}

/// Un file, solo se il suo documento è visibile: `(documento, file)`.
async fn file_visibile(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: i64,
    file_id: i64,
) -> Option<(i64, FileDocumento)> {
    let documento: i64 = sqlx::query_scalar("SELECT documento_id FROM documenti_file WHERE id = ?")
        .bind(file_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()?;
    leggi_documento(pool, utente_id, spazio_id, documento).await?;
    let file = file_del_documento(pool, documento)
        .await
        .into_iter()
        .find(|f| f.id == file_id)?;
    Some((documento, file))
}

/// Una scadenza del documento: è una scadenza dei Promemoria, di chi la
/// crea, legata al documento (eliminandolo, va via anche lei).
pub async fn crea_scadenza_documento(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: i64,
    documento: &Documento,
    data: &str,
    priorita: Priorita,
) -> anyhow::Result<()> {
    let giorno = calendario::parse_date(data).context("Data della scadenza illeggibile")?;
    let id = scadenze::crea_scadenza(
        pool,
        utente_id,
        Some(spazio_id),
        &documento.titolo,
        giorno,
        priorita,
    )
    .await?;
    sqlx::query("UPDATE scadenze SET documento_id = ? WHERE id = ?")
        .bind(documento.id)
        .bind(id)
        .execute(pool)
        .await
        .context("Impossibile legare la scadenza al documento")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("chiavi");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration");
        pool
    }

    async fn utente(pool: &SqlitePool, nome: &str) -> i64 {
        sqlx::query_scalar("INSERT INTO utenti (nome_visualizzato) VALUES (?) RETURNING id")
            .bind(nome)
            .fetch_one(pool)
            .await
            .expect("utente")
    }

    async fn spazio(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar(
            "INSERT INTO spazi (nome, tipo) VALUES ('Casa', 'condiviso') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .expect("spazio")
    }

    /// "Solo mio" non lo vede nessun altro, né nella cartella né cercando.
    #[tokio::test]
    async fn un_documento_solo_mio_non_lo_vede_nessun_altro() {
        let pool = pool().await;
        let spazio = spazio(&pool).await;
        let alessio = utente(&pool, "Alessio").await;
        let altro = utente(&pool, "Ospite").await;
        let mio = crea_documento(&pool, alessio, spazio, "Carta d'identità", None, true)
            .await
            .unwrap();
        crea_documento(&pool, alessio, spazio, "Contratto d'affitto", None, false)
            .await
            .unwrap();
        assert_eq!(documenti_in(&pool, alessio, spazio, None).await.len(), 2);
        let visti_dall_altro = documenti_in(&pool, altro, spazio, None).await;
        assert_eq!(visti_dall_altro.len(), 1);
        assert_eq!(visti_dall_altro[0].titolo, "Contratto d'affitto");
        assert!(leggi_documento(&pool, altro, spazio, mio).await.is_none());
        assert!(cerca(&pool, altro, spazio, "identita").await.is_empty());
        assert_eq!(cerca(&pool, alessio, spazio, "IDENTITÀ").await.len(), 1);
    }

    /// Le cartelle iniziali nascono una volta: eliminarne una non la fa
    /// ricomparire. Una cartella con dentro qualcosa non si elimina.
    #[tokio::test]
    async fn le_cartelle_iniziali_si_eliminano_e_non_tornano() {
        let pool = pool().await;
        let spazio = spazio(&pool).await;
        let alessio = utente(&pool, "Alessio").await;
        inizia_cartelle(&pool, spazio).await.unwrap();
        let tutte = cartelle(&pool, spazio).await;
        assert_eq!(tutte.len(), CARTELLE_INIZIALI.len());
        let identita = tutte.iter().find(|c| c.nome == "Identità").unwrap().id;
        elimina_cartella(&pool, spazio, identita).await.unwrap();
        inizia_cartelle(&pool, spazio).await.unwrap();
        assert_eq!(
            cartelle(&pool, spazio).await.len(),
            CARTELLE_INIZIALI.len() - 1
        );

        let casa = cartelle(&pool, spazio)
            .await
            .into_iter()
            .find(|c| c.nome == "Casa")
            .unwrap()
            .id;
        // Anche un documento "solo mio" di un altro conta: non si butta via
        // quello che non si vede.
        crea_documento(&pool, alessio, spazio, "Rogito", Some(casa), true)
            .await
            .unwrap();
        assert!(elimina_cartella(&pool, spazio, casa).await.is_err());
        let bollette = crea_cartella(&pool, spazio, Some(casa), "Bollette")
            .await
            .unwrap();
        let tutte = cartelle(&pool, spazio).await;
        assert_eq!(percorso(&tutte, bollette), "Casa › Bollette");
        assert!(sta_dentro(&tutte, bollette, casa), "Bollette è dentro Casa");
        assert!(!sta_dentro(&tutte, casa, bollette));
    }

    /// Eliminare un documento porta via le sue scadenze.
    #[tokio::test]
    async fn le_scadenze_del_documento_vanno_via_con_lui() {
        let pool = pool().await;
        let spazio = spazio(&pool).await;
        let alessio = utente(&pool, "Alessio").await;
        let id = crea_documento(&pool, alessio, spazio, "Patente", None, true)
            .await
            .unwrap();
        let documento = leggi_documento(&pool, alessio, spazio, id).await.unwrap();
        crea_scadenza_documento(
            &pool,
            alessio,
            spazio,
            &documento,
            "2030-05-01",
            Priorita::Alta,
        )
        .await
        .unwrap();
        assert_eq!(scadenze_del_documento(&pool, id).await.len(), 1);
        sqlx::query("DELETE FROM documenti WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let rimaste: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scadenze")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rimaste, 0);
    }

    #[test]
    fn i_campi_si_leggono_come_li_scrive_una_persona() {
        assert_eq!(
            leggi_campo(Campo::Rilascio, "15/3/24", "2026-10-07"),
            Ok("2024-03-15".to_string())
        );
        assert!(leggi_campo(Campo::Rilascio, "boh", "2026-10-07").is_err());
        assert_eq!(
            leggi_campo(Campo::Link, "drive.google.com/abc", "2026-10-07"),
            Ok("https://drive.google.com/abc".to_string())
        );
        assert!(leggi_campo(Campo::Link, "non è un link", "2026-10-07").is_err());
        assert_eq!(
            leggi_campo(Campo::Numero, "  CA123  ", "2026-10-07"),
            Ok("CA123".to_string())
        );
        assert_eq!(nome_sicuro("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(
            nome_sicuro("Contratto affitto.pdf"),
            "Contratto_affitto.pdf"
        );
    }
}
