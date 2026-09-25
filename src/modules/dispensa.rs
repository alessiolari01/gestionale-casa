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
//! Dal 17 settembre 2026 (secondo giro, dal collaudo dal vivo) le scorte:
//! - vanno da sole nel posto giusto (`destinazione_per`), secondo una scelta
//!   dell'utente per lo spazio, poi il nome, poi la categoria;
//! - si mostrano sommate per alimento, con le singole confezioni e le loro
//!   scadenze nel dettaglio (`raggruppa_scorte`);
//! - si scalano quando un pasto viene preparato o consumato, e da sole a
//!   orario passato (`scala_scorte_per_pasto`, `scala_pasti_scaduti`), con
//!   la restituzione se il pasto viene poi saltato.
//!
//! Il principio, parole di Alessio: tutto il più automatico possibile, ma
//! l'utente può aggiustare a mano qualunque cosa.
//!
//! Stessa divisione in tre parti degli altri moduli: dominio puro (testato
//! senza database), funzioni database (`sqlite::memory:` nei test), UI
//! Telegram.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Context as _;
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::modules::{
    calendario,
    lista_spesa::{
        alimento_visibile_per_id, carica_mappa_unita, cerca_nel_catalogo, converti_da_base,
        converti_in_base, formatta_quantita, prodotto_visibile_per_id, valida_descrizione_manuale,
        valida_quantita_con_default, IdentitaCatalogo, InfoUnita, RisultatoCatalogo,
    },
    liste,
};

type Bot = crate::context_bot::ContextBot;

/// Sotto questa soglia una quantità si considera finita: le sottrazioni in
/// virgola mobile lasciano residui come `0.0000000001`.
const QUANTITA_TRASCURABILE: f64 = 1e-6;

// ===========================================================================
// Dominio puro.
// ===========================================================================

/// Dove è conservata una scorta. I tre posti standard esistono sempre, senza
/// che l'utente debba configurare niente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Toglie icone e punteggiatura e porta tutto in minuscolo: i nomi del
/// catalogo hanno l'icona della categoria incorporata (`🌾 Pasta`), che qui
/// non deve disturbare il confronto.
fn normalizza_nome(testo: &str) -> String {
    testo
        .chars()
        .map(|carattere| {
            if carattere.is_alphanumeric() {
                carattere.to_lowercase().next().unwrap_or(carattere)
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Il posto giusto per un alimento, ricavato dal nome, quando il nome basta a
/// deciderlo. `None` quando il nome non dice niente di speciale: allora
/// decide la categoria.
///
/// Regole decise il 17 settembre 2026 con Alessio (che ha chiesto di fare
/// più associazioni possibile e di cercare come si conserva la frutta), in
/// ordine di priorità:
/// 1. surgelati, congelati, gelati → freezer, qualunque sia la categoria
///    (gli spinaci congelati sono "verdura", ma non vanno in frigo);
/// 2. conservazione lunga (UHT, secchi, essiccati, uvetta) → dispensa;
/// 3. la verdura che il frigo rovina — patate, cipolle, aglio, scalogno,
///    pomodori, zucca, avocado, olive — → dispensa (la verdura in generale
///    va in frigo). Prima della frutta, perché "pomodori ciliegini" non sono
///    ciliegie;
/// 4. la frutta che sta meglio al fresco — frutti di bosco, fragole,
///    ciliegie, uva, mele, prugne — → frigo (la frutta in generale resta in
///    dispensa: banane, agrumi, frutta da far maturare).
pub fn conservazione_da_nome(nome: &str) -> Option<Conservazione> {
    let testo = normalizza_nome(nome);
    let parole: Vec<&str> = testo.split(' ').collect();
    let inizia = |prefissi: &[&str]| {
        parole
            .iter()
            .any(|parola| prefissi.iter().any(|prefisso| parola.starts_with(prefisso)))
    };
    let parola = |elenco: &[&str]| parole.iter().any(|p| elenco.contains(p));

    if inizia(&["surgelat", "congelat", "gelat", "sorbett", "ghiacciol"]) {
        return Some(Conservazione::Freezer);
    }
    if parola(&["uht", "uvetta"])
        || testo.contains("lunga conservazione")
        || inizia(&["essiccat", "secch", "secco", "secca"])
    {
        return Some(Conservazione::Dispensa);
    }
    // La verdura da tenere fuori viene prima della frutta da frigo: i
    // "pomodori ciliegini" non sono ciliegie (trovato dal primo test).
    if inizia(&["patat", "scalogn", "pomodor", "avocad", "oliv"])
        || parola(&["aglio", "cipolle", "cipolla", "zucca"])
    {
        return Some(Conservazione::Dispensa);
    }
    if inizia(&["fragol", "mirtill", "lampon", "prugn", "susin"])
        || parola(&[
            "uva", "mele", "mela", "more", "ribes", "ciliegie", "ciliegia",
        ])
        || testo.contains("frutti di bosco")
    {
        return Some(Conservazione::Frigo);
    }
    None
}

/// Una quantità nell'unità più comoda da leggere: `1500 g` diventa `1.5 kg`,
/// `2000 ml` diventa `2 l`. Le altre unità restano come sono.
pub fn formatta_quantita_leggibile(quantita: f64, unita: &str) -> String {
    match unita {
        "g" if quantita >= 1000.0 => format!("{} kg", formatta_quantita(quantita / 1000.0)),
        "ml" if quantita >= 1000.0 => format!("{} l", formatta_quantita(quantita / 1000.0)),
        _ => format!("{} {unita}", formatta_quantita(quantita)),
    }
}

/// Interpreta una scadenza scritta a mano: `GG/MM/AAAA` oppure `AAAA-MM-GG`.
/// Ritorna la data in formato ISO, l'unico che entra a database.
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

/// Una singola confezione (un "lotto") in un posto di conservazione.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct Scorta {
    pub id: i64,
    pub conservazione: String,
    pub alimento_id: Option<i64>,
    pub prodotto_alimentare_id: Option<i64>,
    pub descrizione: String,
    pub quantita: f64,
    pub unita_simbolo: String,
    pub scadenza: Option<String>,
}

/// Le confezioni dello stesso alimento, nello stesso posto e nella stessa
/// famiglia di unità, mostrate come una riga sola (chiesto da Alessio dopo
/// aver visto due "Pasta sfoglia · 500 g" separate): il totale sta sulla
/// riga, le singole confezioni con le loro scadenze nel dettaglio.
#[derive(Debug, Clone, PartialEq)]
pub struct GruppoScorte {
    pub descrizione: String,
    pub quantita_base: f64,
    pub unita_base: String,
    /// Ordinate per scadenza (prima quella più vicina, poi quelle senza).
    pub lotti: Vec<Scorta>,
}

impl GruppoScorte {
    /// L'id della prima confezione: identifica il gruppo nei callback.
    pub fn id_rappresentante(&self) -> i64 {
        self.lotti.first().map(|lotto| lotto.id).unwrap_or(0)
    }

    pub fn prima_scadenza(&self) -> Option<&str> {
        self.lotti
            .iter()
            .find_map(|lotto| lotto.scadenza.as_deref())
    }

    pub fn contiene(&self, scorta_id: i64) -> bool {
        self.lotti.iter().any(|lotto| lotto.id == scorta_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChiaveScorta {
    Prodotto(i64),
    Alimento(i64),
    Nome(String),
}

/// Che cosa tiene insieme due confezioni in una riga sola.
///
/// **L'alimento viene prima del prodotto**: 200 g di Parmareggio e 220 g di
/// parmigiano generico sono la stessa cosa in dispensa, e vederli su due
/// righe separate fa credere di averne meno (Alessio, collaudo del 25
/// settembre 2026). La marca non si perde: resta sulle confezioni dentro la
/// riga, che si aprono dal pulsante.
///
/// `alimento_id` di una confezione nata da un prodotto lo riempie la query
/// (`COLONNE_SCORTA`), con un `COALESCE` sul prodotto.
fn chiave_scorta(scorta: &Scorta) -> ChiaveScorta {
    match (scorta.alimento_id, scorta.prodotto_alimentare_id) {
        (Some(id), _) => ChiaveScorta::Alimento(id),
        (None, Some(id)) => ChiaveScorta::Prodotto(id),
        (None, None) => ChiaveScorta::Nome(normalizza_nome(&scorta.descrizione)),
    }
}

/// Ordine delle confezioni: prima quelle che scadono (dalla più vicina),
/// poi quelle senza scadenza. È anche l'ordine in cui si usano quando un
/// pasto le consuma, così si finisce prima quello che scade prima.
fn ordina_per_scadenza(lotti: &mut [Scorta]) {
    lotti.sort_by(|a, b| {
        (a.scadenza.is_none(), a.scadenza.as_deref(), a.id).cmp(&(
            b.scadenza.is_none(),
            b.scadenza.as_deref(),
            b.id,
        ))
    });
}

/// Raggruppa le confezioni di un posto per alimento (o prodotto, o nome per
/// le scorte scritte a mano) e unità-base. Dominio puro: `info_unita`
/// risolve un simbolo come fa `lista_spesa::aggrega_ingredienti`.
pub fn raggruppa_scorte(
    scorte: &[Scorta],
    info_unita: impl Fn(&str) -> Option<InfoUnita>,
) -> Vec<GruppoScorte> {
    let mut gruppi: Vec<(ChiaveScorta, GruppoScorte)> = Vec::new();
    for scorta in scorte {
        let (quantita_base, unita_base) = converti_in_base(
            scorta.quantita,
            &scorta.unita_simbolo,
            info_unita(&scorta.unita_simbolo),
        );
        let chiave = chiave_scorta(scorta);
        match gruppi
            .iter_mut()
            .find(|(k, gruppo)| *k == chiave && gruppo.unita_base == unita_base)
        {
            Some((_, gruppo)) => {
                gruppo.quantita_base += quantita_base;
                gruppo.lotti.push(scorta.clone());
            }
            None => gruppi.push((
                chiave,
                GruppoScorte {
                    descrizione: scorta.descrizione.clone(),
                    quantita_base,
                    unita_base,
                    lotti: vec![scorta.clone()],
                },
            )),
        }
    }
    let mut risultato: Vec<GruppoScorte> = gruppi.into_iter().map(|(_, gruppo)| gruppo).collect();
    for gruppo in &mut risultato {
        ordina_per_scadenza(&mut gruppo.lotti);
    }
    // Prima i gruppi con una scadenza (dalla più vicina), poi gli altri in
    // ordine alfabetico: una scadenza vicina è la cosa da vedere per prima.
    risultato.sort_by(|a, b| {
        (
            a.prima_scadenza().is_none(),
            a.prima_scadenza().map(str::to_string),
            normalizza_nome(&a.descrizione),
        )
            .cmp(&(
                b.prima_scadenza().is_none(),
                b.prima_scadenza().map(str::to_string),
                normalizza_nome(&b.descrizione),
            ))
    });
    risultato
}

/// Etichetta di un gruppo su un pulsante: nome e totale, e -- se c'è -- la
/// scadenza più vicina **a capo** (C15: una parte opzionale non si accoda
/// con " · ", altrimenti Telegram taglia l'etichetta senza avvisare).
pub fn etichetta_gruppo(gruppo: &GruppoScorte) -> String {
    let base = format!(
        "{} · {}",
        liste::tronca(&gruppo.descrizione, 30),
        formatta_quantita_leggibile(gruppo.quantita_base, &gruppo.unita_base)
    );
    let confezioni = gruppo.lotti.len();
    match (gruppo.prima_scadenza(), confezioni) {
        (Some(data), 1) => format!("{base}\n📅 scade {}", calendario::display_date(data)),
        (Some(data), n) => format!(
            "{base}\n📅 prima scadenza {} · {n} confezioni",
            calendario::display_date(data)
        ),
        (None, 1) => base,
        (None, n) => format!("{base}\n{n} confezioni"),
    }
}

// ===========================================================================
// Database (`sqlite::memory:` nei test).
// ===========================================================================

type MappaUnita = HashMap<String, InfoUnita>;

/// `alimento_id` arriva con un `COALESCE` sul prodotto: una confezione nata
/// da un prodotto commerciale a database ha solo `prodotto_alimentare_id`, ma
/// per raggrupparla con il resto di quell'alimento serve sapere qual e'
/// (`chiave_scorta`, 25 settembre 2026). Le scritture continuano a usare le
/// colonne vere, questa e' solo una lettura.
const COLONNE_SCORTA: &str = "s.id, s.conservazione, \
                              COALESCE(s.alimento_id, p.alimento_id) AS alimento_id, \
                              s.prodotto_alimentare_id, \
                              s.descrizione, s.quantita, s.unita_simbolo, s.scadenza";

/// Le scorte si leggono sempre con il loro prodotto accanto, per via del
/// `COALESCE` di `COLONNE_SCORTA`.
const DA_SCORTE: &str =
    "FROM scorte s LEFT JOIN prodotti_alimentari p ON p.id = s.prodotto_alimentare_id";

/// Le confezioni di un posto, nello spazio corrente.
pub async fn scorte_del_luogo(
    pool: &SqlitePool,
    dove: Conservazione,
) -> anyhow::Result<Vec<Scorta>> {
    let actor = crate::identity::current_actor();
    sqlx::query_as(&format!(
        "SELECT {COLONNE_SCORTA} {DA_SCORTE} WHERE s.spazio_id = ? AND s.conservazione = ?"
    ))
    .bind(actor.spazio_id)
    .bind(dove.token())
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le scorte")
}

/// Le righe (gruppi) di un posto, già ordinate.
pub async fn gruppi_del_luogo(
    pool: &SqlitePool,
    dove: Conservazione,
) -> anyhow::Result<Vec<GruppoScorte>> {
    let scorte = scorte_del_luogo(pool, dove).await?;
    let mappa = carica_mappa_unita(pool).await?;
    Ok(raggruppa_scorte(&scorte, |simbolo| {
        mappa.get(simbolo).copied()
    }))
}

pub async fn scorta_per_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Scorta>> {
    let actor = crate::identity::current_actor();
    sqlx::query_as(&format!(
        "SELECT {COLONNE_SCORTA} {DA_SCORTE} WHERE s.id = ? AND s.spazio_id = ?"
    ))
    .bind(id)
    .bind(actor.spazio_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere la scorta")
}

/// Il posto in cui deve finire un alimento, in quest'ordine:
/// 1. la scelta fatta a mano per lo spazio (`📌 Mettilo sempre qui`), prima
///    sul prodotto e poi sull'alimento;
/// 2. il nome (`conservazione_da_nome`), prima del prodotto poi
///    dell'alimento;
/// 3. la categoria dell'alimento (`categorie_alimento.conservazione_predefinita`);
/// 4. la dispensa.
pub async fn destinazione_per(
    conn: &mut SqliteConnection,
    spazio_id: i64,
    alimento_id: Option<i64>,
    prodotto_id: Option<i64>,
    descrizione: &str,
) -> anyhow::Result<Conservazione> {
    let alimento_id = match (alimento_id, prodotto_id) {
        (Some(id), _) => Some(id),
        (None, Some(prodotto)) => {
            sqlx::query_scalar("SELECT alimento_id FROM prodotti_alimentari WHERE id = ?")
                .bind(prodotto)
                .fetch_optional(&mut *conn)
                .await
                .context("Impossibile leggere l'alimento del prodotto")?
        }
        (None, None) => None,
    };

    if let Some(prodotto) = prodotto_id {
        let scelta: Option<String> = sqlx::query_scalar(
            "SELECT conservazione FROM scorte_destinazioni \
             WHERE spazio_id = ? AND prodotto_alimentare_id = ?",
        )
        .bind(spazio_id)
        .bind(prodotto)
        .fetch_optional(&mut *conn)
        .await
        .context("Impossibile leggere la destinazione del prodotto")?;
        if let Some(dove) = scelta.as_deref().and_then(Conservazione::da_token) {
            return Ok(dove);
        }
    }
    if let Some(alimento) = alimento_id {
        let scelta: Option<String> = sqlx::query_scalar(
            "SELECT conservazione FROM scorte_destinazioni \
             WHERE spazio_id = ? AND alimento_id = ?",
        )
        .bind(spazio_id)
        .bind(alimento)
        .fetch_optional(&mut *conn)
        .await
        .context("Impossibile leggere la destinazione dell'alimento")?;
        if let Some(dove) = scelta.as_deref().and_then(Conservazione::da_token) {
            return Ok(dove);
        }
    }

    // Dove quella roba sta già: se in casa c'è altro parmigiano, il nuovo va
    // insieme a quello invece di finire in un posto diverso perché ci è
    // arrivato da un'altra strada. Una scorta esistente è una scelta fatta,
    // anche quando non è stata dichiarata con "📌 Mettilo sempre qui".
    if let Some(alimento) = alimento_id {
        let dove_sta_gia: Option<String> = sqlx::query_scalar(
            "SELECT conservazione FROM scorte \
             WHERE spazio_id = ? AND alimento_id = ? \
             ORDER BY aggiornato_il DESC, id DESC LIMIT 1",
        )
        .bind(spazio_id)
        .bind(alimento)
        .fetch_optional(&mut *conn)
        .await
        .context("Impossibile vedere dove sta già questo alimento")?;
        if let Some(dove) = dove_sta_gia.as_deref().and_then(Conservazione::da_token) {
            return Ok(dove);
        }
    }

    if let Some(dove) = conservazione_da_nome(descrizione) {
        return Ok(dove);
    }
    if let Some(alimento) = alimento_id {
        let nome: Option<String> = sqlx::query_scalar("SELECT nome FROM alimenti WHERE id = ?")
            .bind(alimento)
            .fetch_optional(&mut *conn)
            .await
            .context("Impossibile leggere il nome dell'alimento")?;
        if let Some(dove) = nome.as_deref().and_then(conservazione_da_nome) {
            return Ok(dove);
        }
        // "Altro" è il ripiego assegnato da un trigger a ogni alimento nuovo:
        // se c'è anche una categoria vera, vince quella.
        let da_categoria: Option<String> = sqlx::query_scalar(
            "SELECT c.conservazione_predefinita FROM alimento_categorie ac \
             JOIN categorie_alimento c ON c.id = ac.categoria_id \
             WHERE ac.alimento_id = ? \
             ORDER BY (c.codice = 'altro'), c.ordinamento LIMIT 1",
        )
        .bind(alimento)
        .fetch_optional(&mut *conn)
        .await
        .context("Impossibile leggere la categoria dell'alimento")?;
        if let Some(dove) = da_categoria.as_deref().and_then(Conservazione::da_token) {
            return Ok(dove);
        }
    }
    Ok(Conservazione::Dispensa)
}

/// Ricorda per lo spazio dove va un alimento (o un prodotto): la scelta a
/// mano vince su nome e categoria da qui in avanti.
/// Dove finirà questo alimento dopo la spesa, e se il posto l'ha scelto
/// l'utente (`true`) o l'ha dedotto il bot da nome e categoria (`false`).
/// Serve alla sezione Alimenti, dove il posto si vede e si cambia prima
/// ancora di avere la roba in casa (chiesto da Alessio il 18 settembre 2026).
/// Lo stesso alimento negli **altri** posti, già sommato per posto. Serve a
/// dire "in frigo ne hai altri 200 g" mentre si guarda quello in dispensa:
/// senza, il parmigiano finito in tre posti diversi non si contava più
/// (Alessio, collaudo del 23 settembre 2026, punto 12).
pub async fn altrove_in_casa(
    pool: &SqlitePool,
    scorta_id: i64,
) -> anyhow::Result<Vec<(Conservazione, f64, String)>> {
    let spazio_id = crate::identity::current_actor().spazio_id;
    let righe: Vec<(String, f64, String)> = sqlx::query_as(
        "SELECT altre.conservazione, SUM(altre.quantita), altre.unita_simbolo \
         FROM scorte questa \
         JOIN scorte altre ON altre.spazio_id = questa.spazio_id \
           AND altre.alimento_id IS NOT NULL \
           AND altre.alimento_id = questa.alimento_id \
           AND altre.conservazione <> questa.conservazione \
         WHERE questa.id = ? AND questa.spazio_id = ? \
         GROUP BY altre.conservazione, altre.unita_simbolo \
         ORDER BY altre.conservazione",
    )
    .bind(scorta_id)
    .bind(spazio_id)
    .fetch_all(pool)
    .await
    .context("Impossibile vedere lo stesso alimento negli altri posti")?;
    Ok(righe
        .into_iter()
        .filter_map(|(conservazione, quantita, unita)| {
            Conservazione::da_token(&conservazione).map(|dove| (dove, quantita, unita))
        })
        .collect())
}

/// Il pasto aveva preso le sue scorte, ma quegli ingredienti sono stati
/// usati o buttati e il pasto cambia piatto (19 settembre 2026, "🔁
/// Sostituisci" su un pasto preparato). Le scorte restano tolte — la roba
/// non c'è più — ma i movimenti non sono più di questo pasto, e il pasto
/// torna a poter prendere le scorte del piatto nuovo.
pub async fn dimentica_scarico_pasto(pool: &SqlitePool, pasto_id: i64) -> anyhow::Result<()> {
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    sqlx::query("DELETE FROM scorte_movimenti WHERE pasto_id = ? AND restituito_il IS NULL")
        .bind(pasto_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile staccare i movimenti dal pasto")?;
    sqlx::query(
        "UPDATE planner_pasti SET preparato_il = NULL, scorte_scalate_il = NULL, \
         scorte_scalate_automaticamente = 0, scorte_mancanti = NULL WHERE id = ?",
    )
    .bind(pasto_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile azzerare lo scarico del pasto")?;
    tx.commit().await.context("Impossibile salvare")?;
    Ok(())
}

pub async fn destinazione_alimento(
    pool: &SqlitePool,
    alimento_id: i64,
    nome: &str,
) -> anyhow::Result<(Conservazione, bool)> {
    let spazio_id = crate::identity::current_actor().spazio_id;
    let scelta: Option<String> = sqlx::query_scalar(
        "SELECT conservazione FROM scorte_destinazioni \
         WHERE spazio_id = ? AND alimento_id = ?",
    )
    .bind(spazio_id)
    .bind(alimento_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere la destinazione scelta")?;
    if let Some(dove) = scelta.as_deref().and_then(Conservazione::da_token) {
        return Ok((dove, true));
    }
    let mut conn = pool
        .acquire()
        .await
        .context("Impossibile aprire la connessione")?;
    let dove = destinazione_per(&mut conn, spazio_id, Some(alimento_id), None, nome).await?;
    Ok((dove, false))
}

/// Torna alla scelta automatica: si cancella il posto fisso e riprendono a
/// decidere il nome e la categoria.
pub async fn togli_destinazione(pool: &SqlitePool, alimento_id: i64) -> anyhow::Result<()> {
    let spazio_id = crate::identity::current_actor().spazio_id;
    sqlx::query("DELETE FROM scorte_destinazioni WHERE spazio_id = ? AND alimento_id = ?")
        .bind(spazio_id)
        .bind(alimento_id)
        .execute(pool)
        .await
        .context("Impossibile togliere il posto fisso")?;
    Ok(())
}

pub async fn imposta_destinazione(
    pool: &SqlitePool,
    alimento_id: Option<i64>,
    prodotto_id: Option<i64>,
    dove: Conservazione,
) -> anyhow::Result<()> {
    let spazio_id = crate::identity::current_actor().spazio_id;
    let (colonna, id) = match (prodotto_id, alimento_id) {
        (Some(id), _) => ("prodotto_alimentare_id", id),
        (None, Some(id)) => ("alimento_id", id),
        (None, None) => anyhow::bail!("Una scorta scritta a mano non ha un posto abituale"),
    };
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    sqlx::query(&format!(
        "DELETE FROM scorte_destinazioni WHERE spazio_id = ? AND {colonna} = ?"
    ))
    .bind(spazio_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare la destinazione")?;
    sqlx::query(&format!(
        "INSERT INTO scorte_destinazioni (spazio_id, {colonna}, conservazione) VALUES (?, ?, ?)"
    ))
    .bind(spazio_id)
    .bind(id)
    .bind(dove.token())
    .execute(&mut *tx)
    .await
    .context("Impossibile salvare la destinazione")?;
    tx.commit()
        .await
        .context("Impossibile salvare la destinazione")?;
    Ok(())
}

/// Una scorta da far entrare in casa.
struct NuovaScorta<'a> {
    spazio_id: i64,
    utente_id: i64,
    dove: Conservazione,
    alimento_id: Option<i64>,
    prodotto_id: Option<i64>,
    descrizione: &'a str,
    quantita: f64,
    unita_simbolo: &'a str,
    scadenza: Option<&'a str>,
    origine: &'static str,
    chiusura_id: Option<i64>,
}

/// Aggiunge una scorta, sommandola a una confezione uguale se c'è già: stesso
/// alimento (o prodotto, o nome per quelle scritte a mano), stesso posto,
/// stessa scadenza (anche "nessuna"), unità convertibile. Confezioni con
/// scadenze diverse restano separate: fonderle farebbe perdere la data più
/// vicina, che è quella che serve sapere.
async fn unisci_o_inserisci(
    conn: &mut SqliteConnection,
    mappa: &MappaUnita,
    nuova: NuovaScorta<'_>,
) -> anyhow::Result<i64> {
    let candidati: Vec<Scorta> = sqlx::query_as(&format!(
        "SELECT {COLONNE_SCORTA} {DA_SCORTE} \
         WHERE s.spazio_id = ? AND s.conservazione = ? \
           AND s.alimento_id IS ? AND s.prodotto_alimentare_id IS ? AND s.scadenza IS ? \
         ORDER BY s.id"
    ))
    .bind(nuova.spazio_id)
    .bind(nuova.dove.token())
    .bind(nuova.alimento_id)
    .bind(nuova.prodotto_id)
    .bind(nuova.scadenza)
    .fetch_all(&mut *conn)
    .await
    .context("Impossibile cercare una confezione uguale")?;

    let (quantita_base, unita_base) = converti_in_base(
        nuova.quantita,
        nuova.unita_simbolo,
        mappa.get(nuova.unita_simbolo).copied(),
    );
    let nome_nuovo = normalizza_nome(nuova.descrizione);
    let uguale = candidati.into_iter().find(|candidato| {
        let stesso_nome = nuova.alimento_id.is_some()
            || nuova.prodotto_id.is_some()
            || normalizza_nome(&candidato.descrizione) == nome_nuovo;
        let (_, unita_candidato) = converti_in_base(
            candidato.quantita,
            &candidato.unita_simbolo,
            mappa.get(&candidato.unita_simbolo).copied(),
        );
        stesso_nome && unita_candidato == unita_base
    });

    if let Some(candidato) = uguale {
        let da_aggiungere =
            converti_da_base(quantita_base, mappa.get(&candidato.unita_simbolo).copied());
        sqlx::query(
            "UPDATE scorte SET quantita = quantita + ?, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
        .bind(da_aggiungere)
        .bind(candidato.id)
        .execute(&mut *conn)
        .await
        .context("Impossibile sommare la scorta")?;
        return Ok(candidato.id);
    }

    let id = sqlx::query(
        "INSERT INTO scorte \
         (proprietario_utente_id, spazio_id, conservazione, alimento_id, \
          prodotto_alimentare_id, descrizione, quantita, unita_simbolo, scadenza, \
          origine, chiusura_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(nuova.utente_id)
    .bind(nuova.spazio_id)
    .bind(nuova.dove.token())
    .bind(nuova.alimento_id)
    .bind(nuova.prodotto_id)
    .bind(nuova.descrizione)
    .bind(nuova.quantita)
    .bind(nuova.unita_simbolo)
    .bind(nuova.scadenza)
    .bind(nuova.origine)
    .bind(nuova.chiusura_id)
    .execute(&mut *conn)
    .await
    .context("Impossibile aggiungere la scorta")?
    .last_insert_rowid();
    Ok(id)
}

/// Aggiunge una scorta a mano. `identita` la collega al catalogo quando la
/// si è scelta da lì: serve a sottrarla dal fabbisogno della lista della
/// spesa, che senza un id non potrebbe riconoscere l'alimento.
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
    let mappa = carica_mappa_unita(pool).await?;
    let mut conn = pool
        .acquire()
        .await
        .context("Impossibile aprire la connessione")?;
    unisci_o_inserisci(
        &mut conn,
        &mappa,
        NuovaScorta {
            spazio_id: actor.spazio_id,
            utente_id,
            dove,
            alimento_id,
            prodotto_id,
            descrizione,
            quantita,
            unita_simbolo,
            scadenza: None,
            origine: "manuale",
            chiusura_id: None,
        },
    )
    .await
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

/// Sposta una confezione da un luogo all'altro (dalla spesa al freezer, dal
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

/// Se la merce comprata entra da sola in casa chiudendo la spesa. Acceso di
/// default (deciso con Alessio).
///
/// Dal 24 settembre 2026 la risposta è no anche quando sono spente del tutto
/// le Scorte: chi non tiene il conto di quello che ha in casa non deve
/// ritrovarsi una dispensa che si riempie da sola
/// (`impostazioni::Funzione::ScorteIngresso` ha `Scorte` come padre).
pub async fn ingresso_automatico(pool: &SqlitePool) -> bool {
    if crate::identity::current_actor_opt()
        .and_then(|attore| attore.utente_id)
        .is_none()
    {
        return false;
    }
    crate::modules::impostazioni::funzioni(pool)
        .await
        .attiva(crate::modules::impostazioni::Funzione::ScorteIngresso)
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

/// Riga grezza di una voce archiviata: alimento, prodotto, descrizione,
/// quantità e unità, così come le rilegge la query.
type RigaArchiviataGrezza = (
    Option<i64>,
    Option<i64>,
    String,
    Option<f64>,
    Option<String>,
);

/// Fa entrare in casa la merce di una spesa appena chiusa, ciascuna nel suo
/// posto (`destinazione_per`), sommandola alle confezioni uguali.
///
/// Entrano solo le voci collegate al catalogo: una voce libera come
/// "Detersivo piatti" non è un alimento e resta fuori — chi la vuole la
/// aggiunge a mano.
///
/// Ritorna quante voci sono entrate in ciascun posto (solo quelli non vuoti,
/// nell'ordine dispensa, frigo, freezer).
pub async fn ingresso_da_chiusura(
    pool: &SqlitePool,
    chiusura_id: i64,
) -> anyhow::Result<Vec<(Conservazione, usize)>> {
    let actor = crate::identity::current_actor();
    let utente_id = actor.utente_id.context("Utente non disponibile")?;
    let righe: Vec<RigaArchiviataGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione, quantita, unita_simbolo \
         FROM liste_spesa_voci_archiviate \
         WHERE chiusura_id = ? \
           AND (alimento_id IS NOT NULL OR prodotto_alimentare_id IS NOT NULL) \
           AND quantita IS NOT NULL AND unita_simbolo IS NOT NULL \
         ORDER BY id",
    )
    .bind(chiusura_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci archiviate da mettere in casa")?;

    if righe.is_empty() {
        return Ok(Vec::new());
    }

    let mappa = carica_mappa_unita(pool).await?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    let mut conteggi: HashMap<Conservazione, usize> = HashMap::new();
    for (alimento_id, prodotto_id, descrizione, quantita, unita_simbolo) in righe {
        let (Some(quantita), Some(unita_simbolo)) = (quantita, unita_simbolo) else {
            continue;
        };
        let dove = destinazione_per(
            &mut tx,
            actor.spazio_id,
            alimento_id,
            prodotto_id,
            &descrizione,
        )
        .await?;
        unisci_o_inserisci(
            &mut tx,
            &mappa,
            NuovaScorta {
                spazio_id: actor.spazio_id,
                utente_id,
                dove,
                alimento_id,
                prodotto_id,
                descrizione: &descrizione,
                quantita,
                unita_simbolo: &unita_simbolo,
                scadenza: None,
                origine: "spesa",
                chiusura_id: Some(chiusura_id),
            },
        )
        .await?;
        *conteggi.entry(dove).or_insert(0) += 1;
    }
    tx.commit()
        .await
        .context("Impossibile salvare l'ingresso in casa")?;

    Ok(Conservazione::TUTTE
        .iter()
        .filter_map(|dove| conteggi.get(dove).map(|quante| (*dove, *quante)))
        .collect())
}

/// Una scorta vista dalla lista della spesa: l'alimento a cui appartiene
/// (anche quando è un prodotto specifico), il prodotto se c'è, quantità e
/// unità così come sono salvate.
#[derive(Debug, Clone, FromRow)]
pub struct ScortaGrezza {
    pub alimento_id: Option<i64>,
    pub prodotto_alimentare_id: Option<i64>,
    pub descrizione: String,
    pub quantita: f64,
    pub unita_simbolo: String,
}

/// Tutte le scorte di uno spazio, per sottrarle dal fabbisogno della lista
/// della spesa (`lista_spesa::sottrai_scorte`).
pub async fn scorte_per_netto(
    pool: &SqlitePool,
    spazio_id: Option<i64>,
) -> anyhow::Result<Vec<ScortaGrezza>> {
    sqlx::query_as(
        "SELECT COALESCE(s.alimento_id, p.alimento_id) AS alimento_id, \
                s.prodotto_alimentare_id, s.descrizione, s.quantita, s.unita_simbolo \
         FROM scorte s \
         LEFT JOIN prodotti_alimentari p ON p.id = s.prodotto_alimentare_id \
         WHERE s.spazio_id IS ?",
    )
    .bind(spazio_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le scorte per la lista della spesa")
}

/// Scala dalle scorte gli ingredienti di un pasto: una volta sola per
/// pasto, qualunque sia la strada (preparato, consumato, orario passato).
///
/// Per ogni alimento si usano prima le confezioni che scadono prima, e anche
/// quelle di un prodotto specifico dello stesso alimento (se la ricetta
/// chiede "pasta", la pasta De Cecco va bene). Una confezione finita viene
/// eliminata. Se in casa non c'è abbastanza, si toglie quello che c'è: il
/// resto evidentemente è arrivato da altrove.
///
/// Ogni prelievo è registrato in `scorte_movimenti`, per poterlo
/// restituire. `automatico` dice se lo scarico è avvenuto da solo (orario
/// passato): solo quello viene restituito se il pasto viene poi saltato.
///
/// Ritorna quante confezioni sono state toccate.
pub async fn scala_scorte_per_pasto(
    pool: &SqlitePool,
    pasto_id: i64,
    automatico: bool,
) -> anyhow::Result<usize> {
    // Spente le Scorte (o il solo scarico dai pasti), un pasto non tocca
    // niente: le scorte non sono aggiornate da nessuno, e toglierne sarebbe
    // inventare un magazzino che l'utente non tiene (24 settembre 2026).
    if !crate::modules::impostazioni::funzioni(pool)
        .await
        .attiva(crate::modules::impostazioni::Funzione::ScorteScaricoPasti)
    {
        return Ok(0);
    }
    let mappa = carica_mappa_unita(pool).await?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;

    let stato: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT pp.scorte_scalate_il, p.spazio_id FROM planner_pasti pp \
         JOIN planner_alimentari p ON p.id = pp.planner_id WHERE pp.id = ?",
    )
    .bind(pasto_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile leggere il pasto")?;
    let Some((gia_scalate, spazio_id)) = stato else {
        return Ok(0);
    };
    if gia_scalate.is_some() {
        return Ok(0);
    }

    let bisogni = bisogni_del_pasto(&mut tx, pasto_id, &mappa).await?;

    let mut toccate = 0usize;
    let mut mancanze: Vec<Mancanza> = Vec::new();
    for bisogno in bisogni {
        let unita_base = bisogno.unita_base.clone();
        let mut da_prendere = bisogno.quantita;
        let candidati = scorte_dell_alimento(&mut tx, spazio_id, bisogno.alimento_id).await?;

        for scorta in candidati {
            if da_prendere <= QUANTITA_TRASCURABILE {
                break;
            }
            let info = mappa.get(&scorta.unita_simbolo).copied();
            let (disponibile, unita_scorta) =
                converti_in_base(scorta.quantita, &scorta.unita_simbolo, info);
            if unita_scorta != unita_base || disponibile <= QUANTITA_TRASCURABILE {
                continue;
            }
            let presa_base = da_prendere.min(disponibile);
            let presa = converti_da_base(presa_base, info);
            let resto_base = disponibile - presa_base;
            if resto_base <= QUANTITA_TRASCURABILE {
                sqlx::query("DELETE FROM scorte WHERE id = ?")
                    .bind(scorta.id)
                    .execute(&mut *tx)
                    .await
                    .context("Impossibile togliere una scorta finita")?;
            } else {
                sqlx::query(
                    "UPDATE scorte SET quantita = ?, \
                     aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
                )
                .bind(converti_da_base(resto_base, info))
                .bind(scorta.id)
                .execute(&mut *tx)
                .await
                .context("Impossibile scalare una scorta")?;
            }
            sqlx::query(
                "INSERT INTO scorte_movimenti \
                 (spazio_id, pasto_id, scorta_id, conservazione, alimento_id, \
                  prodotto_alimentare_id, descrizione, quantita, unita_simbolo, scadenza, \
                  automatico) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(spazio_id)
            .bind(pasto_id)
            .bind(if resto_base <= QUANTITA_TRASCURABILE {
                None
            } else {
                Some(scorta.id)
            })
            .bind(&scorta.conservazione)
            .bind(scorta.alimento_id)
            .bind(scorta.prodotto_alimentare_id)
            .bind(&scorta.descrizione)
            .bind(presa)
            .bind(&scorta.unita_simbolo)
            .bind(&scorta.scadenza)
            .bind(i64::from(automatico))
            .execute(&mut *tx)
            .await
            .context("Impossibile registrare il prelievo")?;
            da_prendere -= presa_base;
            toccate += 1;
        }
        if da_prendere > QUANTITA_TRASCURABILE {
            mancanze.push(Mancanza {
                nome: bisogno.nome.clone(),
                serve: bisogno.quantita,
                in_casa: bisogno.quantita - da_prendere,
                unita: unita_base,
            });
        }
    }

    // Quello che mancava resta scritto sul pasto: per uno scarico automatico
    // non c'è nessuno a cui chiedere conferma, e il dettaglio del pasto lo
    // mostra.
    sqlx::query(
        "UPDATE planner_pasti SET scorte_scalate_il = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
         scorte_scalate_automaticamente = ?, scorte_mancanti = ? WHERE id = ?",
    )
    .bind(i64::from(automatico))
    .bind((!mancanze.is_empty()).then(|| descrivi_mancanze(&mancanze)))
    .bind(pasto_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile segnare le scorte come scalate")?;
    tx.commit()
        .await
        .context("Impossibile salvare lo scarico delle scorte")?;
    Ok(toccate)
}

/// Un ingrediente di un pasto, sommato fra i partecipanti e nell'unità-base.
struct Bisogno {
    alimento_id: i64,
    nome: String,
    unita_base: String,
    quantita: f64,
}

async fn bisogni_del_pasto(
    conn: &mut SqliteConnection,
    pasto_id: i64,
    mappa: &MappaUnita,
) -> anyhow::Result<Vec<Bisogno>> {
    let ingredienti: Vec<(i64, String, String, f64)> = sqlx::query_as(
        "SELECT alimento_id, alimento_nome_snapshot, unita_simbolo_snapshot, \
                quantita_finale_snapshot \
         FROM planner_pasto_ingredienti_snapshot \
         WHERE pasto_id = ? AND alimento_id IS NOT NULL \
           AND quantita_finale_snapshot IS NOT NULL \
         ORDER BY id",
    )
    .bind(pasto_id)
    .fetch_all(&mut *conn)
    .await
    .context("Impossibile leggere gli ingredienti del pasto")?;

    // Stesso alimento per più profili: si somma prima.
    let mut bisogni: Vec<Bisogno> = Vec::new();
    for (alimento_id, nome, unita, quantita) in ingredienti {
        let (base, unita_base) = converti_in_base(quantita, &unita, mappa.get(&unita).copied());
        match bisogni
            .iter_mut()
            .find(|b| b.alimento_id == alimento_id && b.unita_base == unita_base)
        {
            Some(bisogno) => bisogno.quantita += base,
            None => bisogni.push(Bisogno {
                alimento_id,
                nome,
                unita_base,
                quantita: base,
            }),
        }
    }
    Ok(bisogni)
}

/// Le confezioni di un alimento (anche dei suoi prodotti) in uno spazio, nell'ordine
/// in cui si usano: prima quelle che scadono prima.
async fn scorte_dell_alimento(
    conn: &mut SqliteConnection,
    spazio_id: Option<i64>,
    alimento_id: i64,
) -> anyhow::Result<Vec<Scorta>> {
    let mut candidati: Vec<Scorta> = sqlx::query_as(&format!(
        "SELECT {COLONNE_SCORTA} {DA_SCORTE} \
         WHERE s.spazio_id IS ? AND (s.alimento_id = ? OR s.prodotto_alimentare_id IN \
               (SELECT id FROM prodotti_alimentari WHERE alimento_id = ?))"
    ))
    .bind(spazio_id)
    .bind(alimento_id)
    .bind(alimento_id)
    .fetch_all(&mut *conn)
    .await
    .context("Impossibile leggere le scorte dell'alimento")?;
    ordina_per_scadenza(&mut candidati);
    Ok(candidati)
}

/// Un ingrediente che in casa non basta.
#[derive(Debug, Clone, PartialEq)]
pub struct Mancanza {
    pub nome: String,
    pub serve: f64,
    pub in_casa: f64,
    pub unita: String,
}

/// `Pasta brisée 200 g, Spaghetti 45 g` — quanto manca di ciascuno.
pub fn descrivi_mancanze(mancanze: &[Mancanza]) -> String {
    mancanze
        .iter()
        .map(|m| {
            format!(
                "{} {}",
                m.nome,
                formatta_quantita_leggibile(m.serve - m.in_casa, &m.unita)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Una riga per la conferma: `• Pasta brisée: servono 200 g, ne hai 0 g`.
pub fn riga_mancanza(mancanza: &Mancanza) -> String {
    format!(
        "• {}: servono {}, ne hai {}",
        mancanza.nome,
        formatta_quantita_leggibile(mancanza.serve, &mancanza.unita),
        formatta_quantita_leggibile(mancanza.in_casa, &mancanza.unita)
    )
}

/// Cosa manca in casa per un pasto, senza toccare niente: il controllo
/// prima di segnarlo preparato o consumato (chiesto da Alessio il 17
/// settembre 2026 — se manca qualcosa è un'eccezione, e va confermata).
/// Vuoto se c'è tutto, o se le scorte di questo pasto sono già state
/// scalate (non c'è più niente da controllare).
pub async fn mancanti_per_pasto(pool: &SqlitePool, pasto_id: i64) -> anyhow::Result<Vec<Mancanza>> {
    let mappa = carica_mappa_unita(pool).await?;
    let mut conn = pool
        .acquire()
        .await
        .context("Impossibile aprire la connessione")?;
    let stato: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT pp.scorte_scalate_il, p.spazio_id FROM planner_pasti pp \
         JOIN planner_alimentari p ON p.id = pp.planner_id WHERE pp.id = ?",
    )
    .bind(pasto_id)
    .fetch_optional(&mut *conn)
    .await
    .context("Impossibile leggere il pasto")?;
    let Some((None, spazio_id)) = stato else {
        return Ok(Vec::new());
    };

    let mut mancanze = Vec::new();
    for bisogno in bisogni_del_pasto(&mut conn, pasto_id, &mappa).await? {
        let disponibile: f64 = scorte_dell_alimento(&mut conn, spazio_id, bisogno.alimento_id)
            .await?
            .iter()
            .filter_map(|scorta| {
                let (quantita, unita) = converti_in_base(
                    scorta.quantita,
                    &scorta.unita_simbolo,
                    mappa.get(&scorta.unita_simbolo).copied(),
                );
                (unita == bisogno.unita_base).then_some(quantita)
            })
            .sum();
        if bisogno.quantita - disponibile > QUANTITA_TRASCURABILE {
            mancanze.push(Mancanza {
                nome: bisogno.nome,
                serve: bisogno.quantita,
                in_casa: disponibile.max(0.0),
                unita: bisogno.unita_base,
            });
        }
    }
    Ok(mancanze)
}

/// Riga grezza di un prelievo da restituire.
#[derive(Debug, Clone, FromRow)]
struct Movimento {
    id: i64,
    spazio_id: Option<i64>,
    scorta_id: Option<i64>,
    conservazione: String,
    alimento_id: Option<i64>,
    prodotto_alimentare_id: Option<i64>,
    descrizione: String,
    quantita: f64,
    unita_simbolo: String,
    scadenza: Option<String>,
}

/// Rimette in casa quello che un pasto aveva preso, **come se non fosse mai
/// stato toccato** (richiesta esplicita di Alessio): nella stessa
/// confezione se c'è ancora, altrimenti in una confezione nuova con lo
/// stesso posto e la stessa scadenza.
///
/// `solo_automatici`: un pasto saltato restituisce solo lo scarico avvenuto
/// da solo a orario passato — uno scarico fatto a mano (preparato) resta,
/// perché il cibo è stato usato comunque. Un pasto sostituito con un altro
/// restituisce tutto.
///
/// Ritorna quante confezioni sono state rimesse a posto.
pub async fn restituisci_scorte_pasto(
    pool: &SqlitePool,
    pasto_id: i64,
    solo_automatici: bool,
) -> anyhow::Result<usize> {
    let mappa = carica_mappa_unita(pool).await?;
    let utente_id = crate::identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;

    let automatico_pasto: Option<i64> = sqlx::query_scalar(
        "SELECT scorte_scalate_automaticamente FROM planner_pasti \
         WHERE id = ? AND scorte_scalate_il IS NOT NULL",
    )
    .bind(pasto_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile leggere il pasto")?;
    let Some(automatico_pasto) = automatico_pasto else {
        return Ok(0);
    };
    if solo_automatici && automatico_pasto == 0 {
        return Ok(0);
    }

    let movimenti: Vec<Movimento> = sqlx::query_as(
        "SELECT id, spazio_id, scorta_id, conservazione, alimento_id, prodotto_alimentare_id, \
                descrizione, quantita, unita_simbolo, scadenza \
         FROM scorte_movimenti \
         WHERE pasto_id = ? AND restituito_il IS NULL AND (automatico = 1 OR ? = 0)",
    )
    .bind(pasto_id)
    .bind(i64::from(solo_automatici))
    .fetch_all(&mut *tx)
    .await
    .context("Impossibile leggere i prelievi del pasto")?;

    let mut rimesse = 0usize;
    for movimento in movimenti {
        let esistente: Option<Scorta> = match movimento.scorta_id {
            Some(id) => sqlx::query_as(&format!(
                "SELECT {COLONNE_SCORTA} {DA_SCORTE} WHERE s.id = ?"
            ))
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .context("Impossibile rileggere la scorta")?,
            None => None,
        };
        let (base, unita_base) = converti_in_base(
            movimento.quantita,
            &movimento.unita_simbolo,
            mappa.get(&movimento.unita_simbolo).copied(),
        );
        let stessa_unita = esistente.as_ref().is_some_and(|scorta| {
            converti_in_base(
                scorta.quantita,
                &scorta.unita_simbolo,
                mappa.get(&scorta.unita_simbolo).copied(),
            )
            .1 == unita_base
        });
        match esistente {
            Some(scorta) if stessa_unita => {
                let da_aggiungere =
                    converti_da_base(base, mappa.get(&scorta.unita_simbolo).copied());
                sqlx::query(
                    "UPDATE scorte SET quantita = quantita + ?, \
                     aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
                )
                .bind(da_aggiungere)
                .bind(scorta.id)
                .execute(&mut *tx)
                .await
                .context("Impossibile restituire la scorta")?;
            }
            _ => {
                sqlx::query(
                    "INSERT INTO scorte \
                     (proprietario_utente_id, spazio_id, conservazione, alimento_id, \
                      prodotto_alimentare_id, descrizione, quantita, unita_simbolo, scadenza, \
                      origine) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'manuale')",
                )
                .bind(utente_id)
                .bind(movimento.spazio_id)
                .bind(&movimento.conservazione)
                .bind(movimento.alimento_id)
                .bind(movimento.prodotto_alimentare_id)
                .bind(&movimento.descrizione)
                .bind(movimento.quantita)
                .bind(&movimento.unita_simbolo)
                .bind(&movimento.scadenza)
                .execute(&mut *tx)
                .await
                .context("Impossibile ricreare la scorta")?;
            }
        }
        sqlx::query(
            "UPDATE scorte_movimenti SET restituito_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
             WHERE id = ?",
        )
        .bind(movimento.id)
        .execute(&mut *tx)
        .await
        .context("Impossibile segnare il prelievo come restituito")?;
        rimesse += 1;
    }

    sqlx::query(
        "UPDATE planner_pasti SET scorte_scalate_il = NULL, scorte_scalate_automaticamente = 0, \
         scorte_mancanti = NULL WHERE id = ?",
    )
    .bind(pasto_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile riaprire lo scarico del pasto")?;
    tx.commit()
        .await
        .context("Impossibile salvare la restituzione")?;
    Ok(rimesse)
}

/// Scala da sole le scorte dei pasti ormai passati: orario superato (o
/// giorno finito, se il pasto non ha orario), non preparati, non saltati,
/// non ancora scaricati. Lo stato del pasto **non** cambia: segnarlo
/// consumato lo congelerebbe, e non si potrebbe più dire che è stato
/// saltato.
///
/// Non c'è uno scheduler nel bot: la funzione gira quando si apre una
/// schermata che mostra le scorte o il fabbisogno (lista della spesa,
/// scorte), che è l'unico momento in cui il risultato serve.
///
/// Ritorna i pasti scaricati, con quello che mancava in casa per ciascuno:
/// chi apre la schermata lo deve sapere, perché qui nessuno ha potuto
/// confermare l'eccezione.
pub async fn scala_pasti_scaduti(pool: &SqlitePool) -> anyhow::Result<Vec<PastoScaricato>> {
    let spazio_id = crate::identity::current_actor().spazio_id;
    let pasti: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT pp.id, pp.tipo_pasto, pp.ricetta_nome_snapshot FROM planner_pasti pp \
         JOIN planner_alimentari p ON p.id = pp.planner_id \
         WHERE p.spazio_id = ? AND p.archiviato = 0 \
           AND pp.stato = 'pianificato' AND pp.saltato_il IS NULL \
           AND pp.scorte_scalate_il IS NULL \
           AND ( date(pp.data_pasto) < date('now', 'localtime') \
              OR ( date(pp.data_pasto) = date('now', 'localtime') \
                   AND pp.orario IS NOT NULL \
                   AND pp.orario <= strftime('%H:%M', 'now', 'localtime') ) ) \
         ORDER BY pp.data_pasto, pp.id",
    )
    .bind(spazio_id)
    .fetch_all(pool)
    .await
    .context("Impossibile cercare i pasti passati")?;
    let mut scaricati = Vec::new();
    for (pasto_id, tipo, ricetta) in pasti {
        scala_scorte_per_pasto(pool, pasto_id, true).await?;
        let mancanti: Option<String> =
            sqlx::query_scalar("SELECT scorte_mancanti FROM planner_pasti WHERE id = ?")
                .bind(pasto_id)
                .fetch_one(pool)
                .await
                .context("Impossibile rileggere il pasto scaricato")?;
        scaricati.push(PastoScaricato {
            descrizione: format!("{} {}", etichetta_tipo_pasto(&tipo), ricetta),
            mancanti,
        });
    }
    Ok(scaricati)
}

/// Un pasto scaricato da solo a orario passato.
#[derive(Debug, Clone, PartialEq)]
pub struct PastoScaricato {
    pub descrizione: String,
    pub mancanti: Option<String>,
}

/// L'avviso per chi apre una schermata dopo uno scarico automatico in cui
/// mancava qualcosa. `None` se non mancava niente.
pub fn avviso_scarichi(scaricati: &[PastoScaricato]) -> Option<String> {
    let righe: Vec<String> = scaricati
        .iter()
        .filter_map(|pasto| {
            pasto
                .mancanti
                .as_ref()
                .map(|mancanti| format!("• {}: mancavano {mancanti}", pasto.descrizione))
        })
        .collect();
    (!righe.is_empty()).then(|| {
        format!(
            "⚠️ Pasti passati, ingredienti tolti dalle scorte da soli. In casa non c'era tutto:\n{}",
            righe.join("\n")
        )
    })
}

fn etichetta_tipo_pasto(tipo: &str) -> &'static str {
    match tipo {
        "colazione" => "Colazione ·",
        "spuntino_mattina" => "Spuntino ·",
        "pranzo" => "Pranzo ·",
        "spuntino_pomeriggio" => "Merenda ·",
        "cena" => "Cena ·",
        _ => "Pasto ·",
    }
}

/// Una ricetta con quanti dei suoi ingredienti ci sono già in casa.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct RicettaDisponibile {
    pub id: i64,
    pub nome: String,
    pub in_casa: i64,
    pub totali: i64,
}

/// Le ricette visibili, ordinate da quella per cui hai più ingredienti in
/// casa a quella per cui ne hai meno (chiesto da Alessio il 17 settembre
/// 2026). A parità, prima quelle a cui ne mancano meno, poi per nome.
///
/// Un ingrediente conta come "in casa" se c'è almeno una scorta dello stesso
/// alimento (o di un suo prodotto), qualunque quantità: la domanda è "cosa
/// posso cucinare con quello che ho", non "ne ho abbastanza per tutti".
/// Solo le ricette con almeno un ingrediente in casa.
pub async fn ricette_con_quello_che_ho(
    pool: &SqlitePool,
    limite: i64,
) -> anyhow::Result<Vec<RicettaDisponibile>> {
    let actor = crate::identity::current_actor();
    let utente_id = actor.utente_id.context("Utente non disponibile")?;
    sqlx::query_as(
        "WITH in_casa AS ( \
             SELECT DISTINCT COALESCE(s.alimento_id, p.alimento_id) AS alimento_id \
             FROM scorte s LEFT JOIN prodotti_alimentari p ON p.id = s.prodotto_alimentare_id \
             WHERE s.spazio_id = ?1 \
         ) \
         SELECT r.id, r.nome, \
                COUNT(DISTINCT CASE WHEN ic.alimento_id IS NOT NULL THEN ri.alimento_id END) \
                    AS in_casa, \
                COUNT(DISTINCT ri.alimento_id) AS totali \
         FROM ricette r \
         JOIN ricetta_ingredienti ri ON ri.ricetta_id = r.id \
         LEFT JOIN in_casa ic ON ic.alimento_id = ri.alimento_id \
         WHERE r.archiviata = 0 \
           AND ( r.catalogo_globale = 1 \
              OR r.proprietario_utente_id = ?2 \
              OR EXISTS (SELECT 1 FROM ricetta_spazi rs \
                         WHERE rs.ricetta_id = r.id AND rs.spazio_id = ?1) ) \
         GROUP BY r.id \
         HAVING in_casa > 0 \
         ORDER BY in_casa DESC, (totali - in_casa) ASC, r.nome COLLATE NOCASE, r.id \
         LIMIT ?3",
    )
    .bind(actor.spazio_id)
    .bind(utente_id)
    .bind(limite)
    .fetch_all(pool)
    .await
    .context("Impossibile cercare le ricette con quello che hai")
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
    bot.send_message(chat_id, "ℹ️ Questa scorta non c'è più. Riapri le scorte.")
        .reply_markup(nav_markup("dispensa:menu"))
        .await?;
    Ok(())
}

/// Messaggio per quante scorte sono entrate in casa, per luogo — usato anche
/// dalla lista della spesa dopo la chiusura.
pub fn riepilogo_ingresso(entrate: &[(Conservazione, usize)]) -> Option<String> {
    if entrate.is_empty() {
        return None;
    }
    Some(
        entrate
            .iter()
            .map(|(dove, quante)| format!("{quante} in {}", dove.etichetta()))
            .collect::<Vec<_>>()
            .join(", "),
    )
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

fn parse_id(raw: &str) -> Option<i64> {
    raw.parse::<i64>().ok().filter(|value| *value > 0)
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
    if data == "dispensa:legenda" {
        liste::cambia_legenda(pool).await;
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
            "✅ Da ora la spesa chiusa entra da sola in casa, ognuna al suo posto."
        };
        mostra_menu(bot, chat_id, pool, Some(avviso)).await?;
        return Ok(true);
    }
    // "Ricette con quello che ho": `:ricette` dalle scorte, `:ricette:r` dal
    // menù delle ricette -- cambia solo dove torna `⬅️ Indietro` (C3).
    if data == "dispensa:ricette" || data == "dispensa:ricette:r" {
        let indietro = if data.ends_with(":r") {
            "recipe:menu"
        } else {
            "dispensa:menu"
        };
        // Aprendo una ricetta da qui, `⬅️ Indietro` deve tornare qui
        // (18 settembre 2026), non all'elenco delle ricette.
        crate::modules::ricette::ricorda_provenienza_scorte(chat_id.0, data);
        mostra_ricette_disponibili(bot, chat_id, pool, indietro).await?;
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
    if let Some(raw_id) = data.strip_prefix("dispensa:gruppo:") {
        let Some(scorta_id) = parse_id(raw_id) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        mostra_gruppo(bot, chat_id, pool, scorta_id).await?;
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
            parti.next().and_then(parse_id),
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
            parti.next().and_then(parse_id),
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
        bot.send_message(chat_id, "📝 Scrivi cosa hai (es. \"Passata di pomodoro\").")
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
            "🔎 Scrivi il nome di un alimento (es. \"pasta\") o di un prodotto (es. \"de cecco\").",
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
        let Some(scorta_id) = parse_id(raw_id) else {
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
        let Some(scorta_id) = parse_id(raw_id) else {
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
        let Some(scorta_id) = parse_id(raw_id) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            DispensaConversationState::AwaitingScadenza { scorta_id },
        );
        bot.send_message(
            chat_id,
            "📅 Scrivi la scadenza come 31/12/2026.\n\nLa scadenza è facoltativa: puoi anche toglierla.",
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
            parti.next().and_then(parse_id),
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
        let Some(scorta_id) = parse_id(raw_id) else {
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
    if let Some(raw_id) = data.strip_prefix("dispensa:dest:") {
        let Some(scorta_id) = parse_id(raw_id) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let Ok(Some(scorta)) = scorta_per_id(pool, scorta_id).await else {
            expired(bot, chat_id).await?;
            return Ok(true);
        };
        let dove =
            Conservazione::da_token(&scorta.conservazione).unwrap_or(Conservazione::Dispensa);
        let avviso = match imposta_destinazione(
            pool,
            scorta.alimento_id,
            scorta.prodotto_alimentare_id,
            dove,
        )
        .await
        {
            Ok(()) => format!(
                "📌 D'ora in poi {} finisce in {} quando chiudi la spesa.",
                scorta.descrizione,
                dove.etichetta()
            ),
            Err(errore) => {
                tracing::warn!(?errore, scorta_id, "Salvataggio destinazione fallito");
                "⚠️ Non riesco a ricordare questo posto.".to_string()
            }
        };
        mostra_scorta(bot, chat_id, pool, scorta_id, Some(&avviso)).await?;
        return Ok(true);
    }
    // C16: un'eliminazione definitiva chiede sempre conferma esplicita.
    if let Some(raw_id) = data.strip_prefix("dispensa:del:ask:") {
        let Some(scorta_id) = parse_id(raw_id) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        bot.send_message(
            chat_id,
            "⚠️ Eliminare questa scorta definitivamente? Non si può recuperare.",
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
        let Some(scorta_id) = parse_id(raw_id) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let dove = scorta_per_id(pool, scorta_id)
            .await
            .ok()
            .flatten()
            .and_then(|scorta| Conservazione::da_token(&scorta.conservazione))
            .unwrap_or(Conservazione::Dispensa);
        let avviso = match rimuovi_scorta(pool, scorta_id).await {
            Ok(()) => "✅ Scorta eliminata.",
            Err(errore) => {
                tracing::warn!(?errore, scorta_id, "Eliminazione scorta fallita");
                "⚠️ Non riesco a eliminare la scorta."
            }
        };
        mostra_luogo(bot, chat_id, pool, dove, 0, Some(avviso)).await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("dispensa:scorta:") {
        let Some(scorta_id) = parse_id(raw_id) else {
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

/// Menù delle scorte: i tre luoghi con quante righe hanno (C7), le ricette
/// che si possono fare con quello che c'è, e la preferenza sull'ingresso
/// automatico dalla spesa.
async fn mostra_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    // Prima di contare, i pasti ormai passati si prendono la loro parte.
    let scarichi = scala_pasti_scaduti(pool).await.unwrap_or_else(|errore| {
        tracing::warn!(?errore, "Scarico dei pasti passati fallito");
        Vec::new()
    });

    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    if let Some(avviso) = avviso_scarichi(&scarichi) {
        testo.push_str(&avviso);
        testo.push_str("\n\n");
    }
    testo.push_str("🥫 Scorte\n\nQuello che hai già in casa.");

    let automatico = ingresso_automatico(pool).await;
    testo.push_str(if automatico {
        "\nChiudendo la spesa, la roba comprata entra da sola in casa, ognuna al suo posto."
    } else {
        "\nChiudendo la spesa, la roba comprata non entra da sola: la aggiungi tu."
    });

    // Legenda dei simboli (18 settembre 2026), accesa finché non la spegni.
    let legenda = liste::legenda_attiva(pool).await;
    if legenda {
        testo.push('\n');
        testo.push_str(&liste::blocco_legenda(liste::LEGENDA_SCORTE));
    }

    let mut rows = Vec::new();
    for dove in Conservazione::TUTTE {
        let totale = gruppi_del_luogo(pool, dove)
            .await
            .map(|gruppi| gruppi.len() as i64)
            .unwrap_or(0);
        rows.push(vec![button(
            liste::etichetta_con_conteggio(dove.etichetta(), totale),
            format!("dispensa:luogo:{}:0", dove.token()),
        )]);
    }
    rows.push(vec![button(
        "🍳 Ricette con quello che ho",
        "dispensa:ricette",
    )]);
    rows.push(vec![button(
        if automatico {
            "⚙️ Ingresso automatico: attivo"
        } else {
            "⚙️ Ingresso automatico: spento"
        },
        "dispensa:auto",
    )]);
    rows.push(vec![liste::pulsante_legenda(legenda, "dispensa:legenda")]);
    rows.push(nav_row("food:menu"));

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Elenco paginato di un luogo: una riga per alimento, con il totale.
async fn mostra_luogo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    dove: Conservazione,
    pagina: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let gruppi = gruppi_del_luogo(pool, dove).await.unwrap_or_default();
    let totale = gruppi.len() as i64;
    let pagina = liste::pagina_valida(pagina, totale);

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

    let mut rows: Vec<Vec<InlineKeyboardButton>> = gruppi
        .iter()
        .skip(liste::scarto(pagina) as usize)
        .take(liste::VOCI_PER_PAGINA)
        .map(|gruppo| {
            vec![button(
                etichetta_gruppo(gruppo),
                format!("dispensa:gruppo:{}", gruppo.id_rappresentante()),
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

/// Le confezioni di una riga: se è una sola si va dritti al suo dettaglio,
/// altrimenti si sceglie quale (ognuna con la sua scadenza).
async fn mostra_gruppo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    scorta_id: i64,
) -> ResponseResult<()> {
    let Ok(Some(scorta)) = scorta_per_id(pool, scorta_id).await else {
        expired(bot, chat_id).await?;
        return Ok(());
    };
    let dove = Conservazione::da_token(&scorta.conservazione).unwrap_or(Conservazione::Dispensa);
    let gruppo = gruppi_del_luogo(pool, dove)
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|gruppo| gruppo.contiene(scorta_id));
    let Some(gruppo) = gruppo else {
        expired(bot, chat_id).await?;
        return Ok(());
    };
    if gruppo.lotti.len() == 1 {
        return mostra_scorta(bot, chat_id, pool, scorta_id, None).await;
    }

    let testo = format!(
        "{}\n\n{} · {}\n{} confezioni con scadenze diverse: scegli quale guardare.",
        dove.etichetta(),
        gruppo.descrizione,
        formatta_quantita_leggibile(gruppo.quantita_base, &gruppo.unita_base),
        gruppo.lotti.len()
    );
    let mut rows: Vec<Vec<InlineKeyboardButton>> = gruppo
        .lotti
        .iter()
        .map(|lotto| {
            let scadenza = match lotto.scadenza.as_deref() {
                Some(data) => format!("scade {}", calendario::display_date(data)),
                None => "senza scadenza".to_string(),
            };
            vec![button(
                format!(
                    "{} {} · {scadenza}",
                    formatta_quantita(lotto.quantita),
                    lotto.unita_simbolo
                ),
                format!("dispensa:scorta:{}", lotto.id),
            )]
        })
        .collect();
    rows.push(nav_row(&format!("dispensa:luogo:{}:0", dove.token())));

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Dettaglio di una confezione: dov'è, quanta ce n'è, la scadenza, e cosa
/// farci.
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
            Some(data) => format!("📅 Scade {}", calendario::display_date(data)),
            None => "📅 Nessuna scadenza".to_string(),
        }
    ));
    // Lo stesso alimento può stare anche altrove: qui si vede, invece di
    // doverlo cercare in tre posti (23 settembre 2026).
    let altrove = altrove_in_casa(pool, scorta_id).await.unwrap_or_default();
    if !altrove.is_empty() {
        let righe: Vec<String> = altrove
            .iter()
            .map(|(dove, quantita, unita)| {
                format!(
                    "{} {}",
                    dove.etichetta(),
                    formatta_quantita_leggibile(*quantita, unita)
                )
            })
            .collect();
        testo.push_str(&format!("\n\n🏠 Ne hai anche in {}", righe.join(", ")));
    }

    let mut rows = vec![
        vec![button("✏️ Quantità", format!("dispensa:qty:{scorta_id}"))],
        vec![button("📅 Scadenza", format!("dispensa:exp:{scorta_id}"))],
        vec![button("🔀 Sposta", format!("dispensa:move:{scorta_id}"))],
    ];
    // Solo una scorta collegata al catalogo ha un "posto abituale": una
    // scritta a mano non viene mai dalla spesa.
    if scorta.alimento_id.is_some() || scorta.prodotto_alimentare_id.is_some() {
        rows.push(vec![button(
            "📌 Mettilo sempre qui",
            format!("dispensa:dest:{scorta_id}"),
        )]);
    }
    rows.push(vec![button(
        "🗑 Elimina",
        format!("dispensa:del:ask:{scorta_id}"),
    )]);
    rows.push(nav_row(&format!("dispensa:luogo:{}:0", dove.token())));

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Le ricette ordinate per quanti ingredienti hai già in casa. Il pulsante
/// porta al dettaglio della ricetta di sempre.
async fn mostra_ricette_disponibili(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    indietro: &str,
) -> ResponseResult<()> {
    if let Err(errore) = scala_pasti_scaduti(pool).await {
        tracing::warn!(?errore, "Scarico dei pasti passati fallito");
    }
    let ricette = ricette_con_quello_che_ho(pool, 20)
        .await
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Ricerca ricette con le scorte fallita");
            Vec::new()
        });

    let testo = if ricette.is_empty() {
        "🍳 Ricette con quello che ho\n\nNessuna ricetta usa quello che hai in casa.\nAggiungi qualche scorta, oppure crea una ricetta."
            .to_string()
    } else {
        "🍳 Ricette con quello che ho\n\nPrima quelle per cui hai più ingredienti. Il numero dice quanti ne hai in casa, su quanti ne servono."
            .to_string()
    };
    let mut rows: Vec<Vec<InlineKeyboardButton>> = ricette
        .iter()
        .map(|ricetta| {
            vec![button(
                format!(
                    "{} · {}/{}",
                    liste::tronca(&ricetta.nome, 32),
                    ricetta.in_casa,
                    ricetta.totali
                ),
                format!("recipe:detail:{}", ricetta.id),
            )]
        })
        .collect();
    rows.push(nav_row(indietro));

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::lista_spesa::FamigliaConversione;
    use sqlx::sqlite::SqlitePoolOptions;

    fn info(simbolo: &str) -> Option<InfoUnita> {
        match simbolo {
            "g" => Some(InfoUnita {
                famiglia: Some(FamigliaConversione::Massa),
                fattore_num: 1.0,
                fattore_den: 1.0,
            }),
            "kg" => Some(InfoUnita {
                famiglia: Some(FamigliaConversione::Massa),
                fattore_num: 1000.0,
                fattore_den: 1.0,
            }),
            _ => None,
        }
    }

    fn scorta(
        id: i64,
        alimento: Option<i64>,
        nome: &str,
        q: f64,
        u: &str,
        s: Option<&str>,
    ) -> Scorta {
        Scorta {
            id,
            conservazione: "dispensa".to_string(),
            alimento_id: alimento,
            prodotto_alimentare_id: None,
            descrizione: nome.to_string(),
            quantita: q,
            unita_simbolo: u.to_string(),
            scadenza: s.map(str::to_string),
        }
    }

    #[test]
    fn il_nome_decide_il_posto_quando_basta() {
        assert_eq!(
            conservazione_da_nome("🥬 Spinaci surgelati"),
            Some(Conservazione::Freezer)
        );
        assert_eq!(
            conservazione_da_nome("Frutti di bosco surgelati"),
            Some(Conservazione::Freezer)
        );
        assert_eq!(
            conservazione_da_nome("🥛 Latte UHT"),
            Some(Conservazione::Dispensa)
        );
        assert_eq!(
            conservazione_da_nome("🍎 Fichi secchi"),
            Some(Conservazione::Dispensa)
        );
        assert_eq!(
            conservazione_da_nome("🍎 Fragole"),
            Some(Conservazione::Frigo)
        );
        assert_eq!(
            conservazione_da_nome("🍎 Uva bianca"),
            Some(Conservazione::Frigo)
        );
        assert_eq!(
            conservazione_da_nome("🍎 Mele golden"),
            Some(Conservazione::Frigo)
        );
        assert_eq!(
            conservazione_da_nome("🥬 Patate"),
            Some(Conservazione::Dispensa)
        );
        assert_eq!(
            conservazione_da_nome("🥬 Cipolle rosse"),
            Some(Conservazione::Dispensa)
        );
        assert_eq!(
            conservazione_da_nome("🥬 Pomodori ciliegini"),
            Some(Conservazione::Dispensa)
        );
        assert_eq!(
            conservazione_da_nome("🍎 Ciliegie"),
            Some(Conservazione::Frigo)
        );
        // Il nome non dice niente: decide la categoria.
        assert_eq!(conservazione_da_nome("🍎 Banane"), None);
        assert_eq!(conservazione_da_nome("🥬 Zucchine"), None);
        assert_eq!(conservazione_da_nome("🥬 Cipollotti"), None);
        assert_eq!(conservazione_da_nome("🌾 Pasta"), None);
    }

    #[test]
    fn le_confezioni_dello_stesso_alimento_diventano_una_riga() {
        let scorte = vec![
            scorta(1, Some(7), "Pasta sfoglia", 500.0, "g", None),
            scorta(2, Some(7), "Pasta sfoglia", 0.5, "kg", Some("2026-10-01")),
            scorta(3, None, "Passata", 700.0, "g", None),
            scorta(4, Some(9), "Uova", 6.0, "pz", None),
        ];
        let gruppi = raggruppa_scorte(&scorte, info);
        assert_eq!(gruppi.len(), 3);
        // Prima il gruppo con una scadenza.
        assert_eq!(gruppi[0].descrizione, "Pasta sfoglia");
        assert_eq!(gruppi[0].quantita_base, 1000.0);
        assert_eq!(gruppi[0].unita_base, "g");
        // Dentro, prima la confezione che scade.
        assert_eq!(gruppi[0].lotti[0].id, 2);
        assert_eq!(gruppi[0].id_rappresentante(), 2);
        assert!(gruppi[0].contiene(1));
    }

    #[test]
    fn l_etichetta_manda_a_capo_la_scadenza_e_conta_le_confezioni() {
        let una = raggruppa_scorte(&[scorta(1, Some(7), "Pollo", 800.0, "g", None)], info);
        assert_eq!(etichetta_gruppo(&una[0]), "Pollo · 800 g");

        let due = raggruppa_scorte(
            &[
                scorta(1, Some(7), "Pasta", 1000.0, "g", Some("2026-12-31")),
                scorta(2, Some(7), "Pasta", 500.0, "g", None),
            ],
            info,
        );
        let etichetta = etichetta_gruppo(&due[0]);
        // Virgola, come si scrive in italiano (23 settembre 2026).
        assert!(etichetta.starts_with("Pasta · 1,5 kg\n📅 prima scadenza "));
        assert!(etichetta.ends_with("· 2 confezioni"));
    }

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
    fn token_dei_luoghi_e_riepilogo_dell_ingresso() {
        for dove in Conservazione::TUTTE {
            assert_eq!(Conservazione::da_token(dove.token()), Some(dove));
        }
        assert_eq!(Conservazione::da_token("armadio"), None);
        assert_eq!(riepilogo_ingresso(&[]), None);
        assert_eq!(
            riepilogo_ingresso(&[(Conservazione::Dispensa, 2), (Conservazione::Frigo, 1)])
                .as_deref(),
            Some("2 in 🧺 Dispensa, 1 in 🧊 Frigo")
        );
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

    async fn alimento(pool: &SqlitePool, nome: &str, categoria: &str) -> i64 {
        let normalizzato = normalizza_nome(nome);
        let id = match sqlx::query_scalar::<_, i64>(
            "SELECT id FROM alimenti WHERE nome_normalizzato = ? AND catalogo_globale = 1",
        )
        .bind(&normalizzato)
        .fetch_optional(pool)
        .await
        .expect("ricerca alimento")
        {
            Some(id) => id,
            None => sqlx::query(
                "INSERT INTO alimenti (nome, nome_normalizzato, catalogo_globale) VALUES (?, ?, 1)",
            )
            .bind(nome)
            .bind(&normalizzato)
            .execute(pool)
            .await
            .expect("alimento")
            .last_insert_rowid(),
        };
        sqlx::query("DELETE FROM alimento_categorie WHERE alimento_id = ?")
            .bind(id)
            .execute(pool)
            .await
            .expect("pulizia categorie");
        sqlx::query(
            "INSERT INTO alimento_categorie (alimento_id, categoria_id) \
             SELECT ?, id FROM categorie_alimento WHERE codice = ?",
        )
        .bind(id)
        .bind(categoria)
        .execute(pool)
        .await
        .expect("categoria");
        id
    }

    /// Un pasto pianificato oggi, con un ingrediente, nello spazio.
    async fn pasto_con(
        pool: &SqlitePool,
        user_id: i64,
        space_id: i64,
        alimento_id: i64,
        quantita: f64,
    ) -> i64 {
        let oggi: String = sqlx::query_scalar("SELECT date('now','localtime')")
            .fetch_one(pool)
            .await
            .expect("oggi");
        let planner_id = sqlx::query(
            "INSERT INTO planner_alimentari \
             (proprietario_utente_id, spazio_id, nome, nome_normalizzato, data_inizio, data_fine) \
             VALUES (?, ?, 'Settimana', 'settimana', ?, ?)",
        )
        .bind(user_id)
        .bind(space_id)
        .bind(&oggi)
        .bind(&oggi)
        .execute(pool)
        .await
        .expect("planner")
        .last_insert_rowid();
        let pasto_id = sqlx::query(
            "INSERT INTO planner_pasti \
             (planner_id, data_pasto, tipo_pasto, ricetta_nome_snapshot, \
              ricetta_porzione_base_snapshot, orario) \
             VALUES (?, ?, 'pranzo', 'Ricetta test', 1, '00:00')",
        )
        .bind(planner_id)
        .bind(&oggi)
        .execute(pool)
        .await
        .expect("pasto")
        .last_insert_rowid();
        sqlx::query(
            "INSERT INTO planner_pasto_ingredienti_snapshot \
             (pasto_id, alimento_id, alimento_nome_snapshot, unita_simbolo_snapshot, \
              quantita_base_snapshot, quantita_scalata_snapshot, tipo_override_snapshot, \
              quantita_finale_snapshot) \
             VALUES (?, ?, 'Pasta', 'g', 1, 1, 'nessuno', ?)",
        )
        .bind(pasto_id)
        .bind(alimento_id)
        .bind(quantita)
        .execute(pool)
        .await
        .expect("ingrediente");
        pasto_id
    }

    #[tokio::test]
    async fn una_scorta_si_aggiunge_si_somma_si_sposta_e_si_elimina() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let pasta = alimento(&pool, "Pasta", "cereali").await;
            let prima = aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(pasta)),
                "Pasta",
                500.0,
                "g",
            )
            .await
            .expect("prima");
            // Una seconda confezione uguale si somma alla prima.
            let seconda = aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(pasta)),
                "Pasta",
                500.0,
                "g",
            )
            .await
            .expect("seconda");
            assert_eq!(prima, seconda);
            let scorta = scorta_per_id(&pool, prima).await.unwrap().unwrap();
            assert_eq!(scorta.quantita, 1000.0);

            sposta_scorta(&pool, prima, Conservazione::Freezer)
                .await
                .expect("spostamento");
            assert!(gruppi_del_luogo(&pool, Conservazione::Dispensa)
                .await
                .unwrap()
                .is_empty());
            assert_eq!(
                gruppi_del_luogo(&pool, Conservazione::Freezer)
                    .await
                    .unwrap()
                    .len(),
                1
            );

            imposta_scadenza(&pool, prima, Some("2026-12-31"))
                .await
                .expect("scadenza");
            let scorta = scorta_per_id(&pool, prima).await.unwrap().unwrap();
            assert_eq!(scorta.scadenza.as_deref(), Some("2026-12-31"));

            rimuovi_scorta(&pool, prima).await.expect("eliminazione");
            assert!(scorta_per_id(&pool, prima).await.unwrap().is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn la_destinazione_segue_scelta_nome_e_categoria() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let pasta = alimento(&pool, "Pasta", "cereali").await;
            let latte = alimento(&pool, "Latte intero", "latticini").await;
            let spinaci = alimento(&pool, "Spinaci surgelati", "verdura").await;
            let mut conn = pool.acquire().await.unwrap();

            assert_eq!(
                destinazione_per(&mut conn, space_id, Some(pasta), None, "Pasta")
                    .await
                    .unwrap(),
                Conservazione::Dispensa
            );
            assert_eq!(
                destinazione_per(&mut conn, space_id, Some(latte), None, "Latte intero")
                    .await
                    .unwrap(),
                Conservazione::Frigo
            );
            // Il nome batte la categoria.
            assert_eq!(
                destinazione_per(
                    &mut conn,
                    space_id,
                    Some(spinaci),
                    None,
                    "Spinaci surgelati"
                )
                .await
                .unwrap(),
                Conservazione::Freezer
            );
            drop(conn);

            // E la scelta a mano batte tutto.
            imposta_destinazione(&pool, Some(pasta), None, Conservazione::Frigo)
                .await
                .expect("scelta");
            let mut conn = pool.acquire().await.unwrap();
            assert_eq!(
                destinazione_per(&mut conn, space_id, Some(pasta), None, "Pasta")
                    .await
                    .unwrap(),
                Conservazione::Frigo
            );
        })
        .await;
    }

    #[tokio::test]
    async fn un_pasto_scala_le_scorte_e_saltarlo_le_restituisce_com_erano() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let pasta = alimento(&pool, "Pasta", "cereali").await;
            // Due confezioni: quella che scade prima si usa per prima.
            let presto = aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(pasta)),
                "Pasta",
                100.0,
                "g",
            )
            .await
            .unwrap();
            imposta_scadenza(&pool, presto, Some("2026-10-01"))
                .await
                .unwrap();
            let tardi = aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(pasta)),
                "Pasta",
                1.0,
                "kg",
            )
            .await
            .unwrap();
            assert_ne!(presto, tardi);

            // Pasto oggi alle 00:00: l'orario è già passato.
            let pasto = pasto_con(&pool, user_id, space_id, pasta, 250.0).await;
            let scaricati = scala_pasti_scaduti(&pool).await.expect("scarico");
            assert_eq!(scaricati.len(), 1);
            // C'era tutto: nessun avviso.
            assert_eq!(scaricati[0].mancanti, None);
            assert_eq!(avviso_scarichi(&scaricati), None);

            // 100 g dalla confezione che scade (finita, eliminata), 150 g
            // dall'altra, che resta in kg.
            assert!(scorta_per_id(&pool, presto).await.unwrap().is_none());
            let resto = scorta_per_id(&pool, tardi).await.unwrap().unwrap();
            assert!((resto.quantita - 0.85).abs() < 1e-9);

            // Una volta sola.
            assert_eq!(scala_scorte_per_pasto(&pool, pasto, true).await.unwrap(), 0);

            // Saltato: tutto torna com'era, stessa scadenza compresa.
            let rimesse = restituisci_scorte_pasto(&pool, pasto, true)
                .await
                .expect("restituzione");
            assert_eq!(rimesse, 2);
            let resto = scorta_per_id(&pool, tardi).await.unwrap().unwrap();
            assert!((resto.quantita - 1.0).abs() < 1e-9);
            let gruppi = gruppi_del_luogo(&pool, Conservazione::Dispensa)
                .await
                .unwrap();
            assert_eq!(gruppi.len(), 1);
            assert_eq!(gruppi[0].quantita_base, 1100.0);
            assert_eq!(gruppi[0].prima_scadenza(), Some("2026-10-01"));
        })
        .await;
    }

    #[test]
    fn le_mancanze_si_leggono_in_due_modi() {
        let mancanze = vec![
            Mancanza {
                nome: "Pasta brisée".to_string(),
                serve: 200.0,
                in_casa: 0.0,
                unita: "g".to_string(),
            },
            Mancanza {
                nome: "Latte".to_string(),
                serve: 1500.0,
                in_casa: 1000.0,
                unita: "ml".to_string(),
            },
        ];
        assert_eq!(
            descrivi_mancanze(&mancanze),
            format!(
                "Pasta brisée {}, Latte {}",
                formatta_quantita_leggibile(200.0, "g"),
                formatta_quantita_leggibile(500.0, "ml")
            )
        );
        assert_eq!(
            riga_mancanza(&mancanze[0]),
            format!(
                "• Pasta brisée: servono {}, ne hai {}",
                formatta_quantita_leggibile(200.0, "g"),
                formatta_quantita_leggibile(0.0, "g")
            )
        );

        let scaricati = vec![
            PastoScaricato {
                descrizione: "Pranzo · Quiche".to_string(),
                mancanti: Some("Pasta brisée 200 g".to_string()),
            },
            PastoScaricato {
                descrizione: "Cena · Pasta".to_string(),
                mancanti: None,
            },
        ];
        let avviso = avviso_scarichi(&scaricati).expect("avviso");
        assert!(avviso.contains("• Pranzo · Quiche: mancavano Pasta brisée 200 g"));
        assert!(!avviso.contains("Cena"), "chi aveva tutto non compare");
    }

    #[tokio::test]
    async fn quel_che_manca_si_controlla_prima_e_si_ricorda_dopo_lo_scarico() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let pasta = alimento(&pool, "Pasta", "cereali").await;
            aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(pasta)),
                "Pasta",
                100.0,
                "g",
            )
            .await
            .unwrap();
            let pasto = pasto_con(&pool, user_id, space_id, pasta, 250.0).await;

            // Il controllo non tocca niente.
            let mancanze = mancanti_per_pasto(&pool, pasto).await.expect("controllo");
            assert_eq!(mancanze.len(), 1);
            assert_eq!(mancanze[0].serve, 250.0);
            assert_eq!(mancanze[0].in_casa, 100.0);
            assert_eq!(
                gruppi_del_luogo(&pool, Conservazione::Dispensa)
                    .await
                    .unwrap()[0]
                    .quantita_base,
                100.0
            );

            // Lo scarico automatico prende quello che c'è e annota il resto.
            let scaricati = scala_pasti_scaduti(&pool).await.expect("scarico");
            assert_eq!(scaricati.len(), 1);
            let mancanti = scaricati[0].mancanti.clone().expect("mancava qualcosa");
            assert!(mancanti.starts_with("Pasta "), "{mancanti}");
            assert!(avviso_scarichi(&scaricati).is_some());
            let annotato: Option<String> =
                sqlx::query_scalar("SELECT scorte_mancanti FROM planner_pasti WHERE id = ?")
                    .bind(pasto)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(annotato.as_deref(), Some(mancanti.as_str()));
            assert!(gruppi_del_luogo(&pool, Conservazione::Dispensa)
                .await
                .unwrap()
                .is_empty());
            // A scorte già prese non c'è più niente da controllare.
            assert!(mancanti_per_pasto(&pool, pasto).await.unwrap().is_empty());

            // Restituire le scorte cancella anche l'annotazione.
            restituisci_scorte_pasto(&pool, pasto, true)
                .await
                .expect("restituzione");
            let annotato: Option<String> =
                sqlx::query_scalar("SELECT scorte_mancanti FROM planner_pasti WHERE id = ?")
                    .bind(pasto)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(annotato, None);
        })
        .await;
    }

    /// Migration della consegna A: da saltato o consumato si torna a
    /// pianificato, ma il resto del pasto resta bloccato.
    #[tokio::test]
    async fn un_pasto_saltato_o_consumato_torna_pianificato_e_nient_altro() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;
        let pasta = alimento(&pool, "Pasta", "cereali").await;
        let pasto = pasto_con(&pool, user_id, space_id, pasta, 100.0).await;
        let esegui = |sql: &'static str| {
            let pool = pool.clone();
            async move { sqlx::query(sql).bind(pasto).execute(&pool).await }
        };
        const SALTA: &str =
            "UPDATE planner_pasti SET saltato_il = '2026-09-17T10:00:00Z' WHERE id = ?";
        const RIPRISTINA: &str = "UPDATE planner_pasti SET saltato_il = NULL WHERE id = ?";
        const CONSUMA: &str = "UPDATE planner_pasti SET stato = 'completato', \
             completato_il = '2026-09-17T10:00:00Z' WHERE id = ?";
        const RIAPRI: &str =
            "UPDATE planner_pasti SET stato = 'pianificato', completato_il = NULL WHERE id = ?";
        const CAMBIA: &str =
            "UPDATE planner_pasti SET ricetta_nome_snapshot = 'Altra ricetta' WHERE id = ?";

        esegui(SALTA).await.expect("saltato");
        assert!(
            esegui(CAMBIA).await.is_err(),
            "un saltato non cambia ricetta"
        );
        assert!(
            esegui("UPDATE planner_pasti SET saltato_il = '2026-09-18T10:00:00Z' WHERE id = ?")
                .await
                .is_err(),
            "la data del salto non si riscrive"
        );
        esegui(RIPRISTINA).await.expect("di nuovo pianificato");

        esegui(CONSUMA).await.expect("consumato");
        assert!(
            esegui(CAMBIA).await.is_err(),
            "un consumato non cambia ricetta"
        );
        esegui(RIAPRI).await.expect("di nuovo pianificato");
        esegui(CAMBIA).await.expect("un pianificato cambia ricetta");
    }

    /// "🔁 Sostituisci" su un pasto preparato, ingredienti usati o buttati
    /// (19 settembre 2026): le scorte restano tolte, ma il pasto torna libero
    /// di prendere quelle del piatto nuovo.
    #[tokio::test]
    async fn uno_scarico_dimenticato_lascia_le_scorte_tolte_e_libera_il_pasto() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let pasta = alimento(&pool, "Pasta", "cereali").await;
            aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(pasta)),
                "Pasta",
                500.0,
                "g",
            )
            .await
            .unwrap();
            let pasto = pasto_con(&pool, user_id, space_id, pasta, 200.0).await;
            assert!(scala_scorte_per_pasto(&pool, pasto, false).await.unwrap() > 0);
            sqlx::query(
                "UPDATE planner_pasti SET preparato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            )
            .bind(pasto)
            .execute(&pool)
            .await
            .unwrap();

            dimentica_scarico_pasto(&pool, pasto).await.expect("dimentica");

            let rimaste = gruppi_del_luogo(&pool, Conservazione::Dispensa).await.unwrap();
            assert_eq!(rimaste[0].quantita_base, 300.0, "la roba usata non torna");
            let (preparato, scalato): (Option<String>, Option<String>) = sqlx::query_as(
                "SELECT preparato_il, scorte_scalate_il FROM planner_pasti WHERE id = ?",
            )
            .bind(pasto)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!((preparato, scalato), (None, None));
            // Restituire adesso non rimette niente: i movimenti non sono più
            // di questo pasto.
            assert_eq!(restituisci_scorte_pasto(&pool, pasto, false).await.unwrap(), 0);
            // E il pasto può prendere di nuovo le sue scorte.
            assert!(scala_scorte_per_pasto(&pool, pasto, false).await.unwrap() > 0);
        })
        .await;
    }

    /// Due confezioni dello stesso alimento -- una generica e una di marca --
    /// stanno in una riga sola: su due righe separate sembra di averne meno
    /// (Alessio, collaudo del 25 settembre 2026).
    #[test]
    fn il_generico_e_quello_di_marca_stanno_nella_stessa_riga() {
        // Entrambe portano l'alimento 7: la seconda ha anche il prodotto, ed
        // e' la query a riempirle l'alimento con un COALESCE.
        let mut di_marca = scorta(2, Some(7), "Parmareggio Parmigiano", 200.0, "g", None);
        di_marca.prodotto_alimentare_id = Some(42);
        let gruppi = raggruppa_scorte(
            &[
                scorta(1, Some(7), "Parmigiano Reggiano", 220.0, "g", None),
                di_marca,
            ],
            info,
        );
        assert_eq!(gruppi.len(), 1, "una riga sola");
        assert_eq!(gruppi[0].quantita_base, 420.0);
        assert_eq!(gruppi[0].lotti.len(), 2, "le confezioni restano distinte");
    }

    /// Collaudo del 23 settembre 2026, punto 12: lo stesso alimento non deve
    /// sparpagliarsi. Se in casa ce n'è già, la roba nuova va dove sta
    /// quella, e dalla sua scheda si vede dov'è il resto.
    #[tokio::test]
    async fn la_roba_nuova_va_dove_sta_gia_quella_di_prima() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            // "Latte intero" per categoria andrebbe in frigo.
            let latte = alimento(&pool, "Latte intero", "latticini").await;
            let mut conn = pool.acquire().await.unwrap();
            assert_eq!(
                destinazione_per(&mut conn, space_id, Some(latte), None, "Latte intero")
                    .await
                    .unwrap(),
                Conservazione::Frigo
            );
            drop(conn);

            // Ma se una scorta è già in freezer, la roba nuova la raggiunge.
            let in_freezer = aggiungi_scorta(
                &pool,
                Conservazione::Freezer,
                Some(IdentitaCatalogo::Alimento(latte)),
                "Latte intero",
                1000.0,
                "ml",
            )
            .await
            .expect("scorta in freezer");
            let mut conn = pool.acquire().await.unwrap();
            assert_eq!(
                destinazione_per(&mut conn, space_id, Some(latte), None, "Latte intero")
                    .await
                    .unwrap(),
                Conservazione::Freezer,
                "va dove sta già"
            );
            drop(conn);

            // La scelta a mano resta più forte di tutto.
            imposta_destinazione(&pool, Some(latte), None, Conservazione::Dispensa)
                .await
                .expect("scelta");
            let mut conn = pool.acquire().await.unwrap();
            assert_eq!(
                destinazione_per(&mut conn, space_id, Some(latte), None, "Latte intero")
                    .await
                    .unwrap(),
                Conservazione::Dispensa
            );
            drop(conn);

            // La scheda di una scorta dice dov'è il resto.
            assert!(altrove_in_casa(&pool, in_freezer).await.unwrap().is_empty());
            aggiungi_scorta(
                &pool,
                Conservazione::Frigo,
                Some(IdentitaCatalogo::Alimento(latte)),
                "Latte intero",
                500.0,
                "ml",
            )
            .await
            .expect("scorta in frigo");
            let altrove = altrove_in_casa(&pool, in_freezer).await.unwrap();
            assert_eq!(altrove.len(), 1);
            assert_eq!(altrove[0].0, Conservazione::Frigo);
            assert_eq!(altrove[0].1, 500.0);
        })
        .await;
    }

    #[tokio::test]
    async fn uno_scarico_fatto_a_mano_non_si_restituisce_saltando() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let pasta = alimento(&pool, "Pasta", "cereali").await;
            let id = aggiungi_scorta(
                &pool,
                Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(pasta)),
                "Pasta",
                500.0,
                "g",
            )
            .await
            .unwrap();
            let pasto = pasto_con(&pool, user_id, space_id, pasta, 200.0).await;
            scala_scorte_per_pasto(&pool, pasto, false).await.unwrap();

            assert_eq!(
                restituisci_scorte_pasto(&pool, pasto, true).await.unwrap(),
                0
            );
            let scorta = scorta_per_id(&pool, id).await.unwrap().unwrap();
            assert_eq!(scorta.quantita, 300.0);

            // Un pasto sostituito invece restituisce tutto.
            assert_eq!(
                restituisci_scorte_pasto(&pool, pasto, false).await.unwrap(),
                1
            );
            let scorta = scorta_per_id(&pool, id).await.unwrap().unwrap();
            assert_eq!(scorta.quantita, 500.0);
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
