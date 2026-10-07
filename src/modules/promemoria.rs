//! ⏰ Promemoria (7 ottobre 2026, `docs/previsto/promemoria.md`).
//!
//! Il primo modulo in cui il bot scrive da solo, a un orario. Tre fonti:
//! i promemoria scritti a mano, i pasti del planner e le scorte che
//! scadono; un motore solo (`controlla`) che gira ogni trenta secondi e
//! manda quello che è arrivato il momento di mandare.
//!
//! Le ore sono testo in ora **locale**, `AAAA-MM-GG HH:MM`: il fuso del
//! telefono lo conosce solo SQLite (`'localtime'`), come nel resto del bot.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use anyhow::Context as _;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};
use sqlx::SqlitePool;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId},
};

use crate::modules::{calendario, liste};

type Bot = crate::context_bot::ContextBot;

/// Oltre questo ritardo un avviso dice che è in ritardo.
const RITARDO_DA_DIRE: i64 = 10;

/// Un pasto il cui avviso è in ritardo di più di così non si avvisa più:
/// il pasto è passato, o sta per esserlo.
const RITARDO_MASSIMO_PASTO: i64 = 30;

// ===========================================================================
// Dominio puro.
// ===========================================================================

/// Ogni quanto si ripete un promemoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ripetizione {
    Mai,
    Giorno,
    Feriali,
    Settimana,
    Mese,
    Anno,
}

pub const RIPETIZIONI: [Ripetizione; 6] = [
    Ripetizione::Mai,
    Ripetizione::Giorno,
    Ripetizione::Feriali,
    Ripetizione::Settimana,
    Ripetizione::Mese,
    Ripetizione::Anno,
];

impl Ripetizione {
    pub fn token(self) -> &'static str {
        match self {
            Ripetizione::Mai => "mai",
            Ripetizione::Giorno => "giorno",
            Ripetizione::Feriali => "feriali",
            Ripetizione::Settimana => "settimana",
            Ripetizione::Mese => "mese",
            Ripetizione::Anno => "anno",
        }
    }

    pub fn da_token(valore: &str) -> Option<Self> {
        RIPETIZIONI
            .iter()
            .copied()
            .find(|ripetizione| ripetizione.token() == valore)
    }

    /// Come la si dice, rispetto alla prima volta: "Ogni martedì", "Ogni
    /// mese, il giorno 7".
    pub fn etichetta(self, quando: NaiveDateTime, giorno_scelto: Option<u32>) -> String {
        let giorno = giorno_scelto.unwrap_or_else(|| quando.day());
        match self {
            Ripetizione::Mai => "Una volta sola".to_string(),
            Ripetizione::Giorno => "Ogni giorno".to_string(),
            Ripetizione::Feriali => "Dal lunedì al venerdì".to_string(),
            Ripetizione::Settimana => format!("Ogni {}", giorno_della_settimana(quando.weekday())),
            Ripetizione::Mese => format!("Ogni mese, il giorno {giorno}"),
            Ripetizione::Anno => format!(
                "Ogni anno, il {giorno} {}",
                calendario::month_name(quando.month()).to_lowercase()
            ),
        }
    }

    /// Il giorno da ricordare per le ripetizioni che ne hanno bisogno.
    fn giorno_scelto(self, quando: NaiveDateTime) -> Option<u32> {
        matches!(self, Ripetizione::Mese | Ripetizione::Anno).then(|| quando.day())
    }
}

fn giorno_della_settimana(giorno: Weekday) -> &'static str {
    match giorno {
        Weekday::Mon => "lunedì",
        Weekday::Tue => "martedì",
        Weekday::Wed => "mercoledì",
        Weekday::Thu => "giovedì",
        Weekday::Fri => "venerdì",
        Weekday::Sat => "sabato",
        Weekday::Sun => "domenica",
    }
}

fn e_feriale(data: NaiveDate) -> bool {
    !matches!(data.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Il giorno `giorno` di un mese, o l'ultimo se quel mese è più corto:
/// "ogni mese il 31" cade il 30 ad aprile.
fn giorno_nel_mese(anno: i32, mese: u32, giorno: u32) -> Option<NaiveDate> {
    let ultimo = calendario::days_in_month(anno, mese);
    NaiveDate::from_ymd_opt(anno, mese, giorno.min(ultimo))
}

/// La volta dopo `attuale`, una sola.
fn volta_dopo(
    ripetizione: Ripetizione,
    attuale: NaiveDateTime,
    giorno_scelto: Option<u32>,
) -> Option<NaiveDateTime> {
    let ora = attuale.time();
    let data = attuale.date();
    let giorno = giorno_scelto.unwrap_or_else(|| data.day());
    let prossima = match ripetizione {
        Ripetizione::Mai => return None,
        Ripetizione::Giorno => data.succ_opt()?,
        Ripetizione::Feriali => {
            let mut prossima = data.succ_opt()?;
            while !e_feriale(prossima) {
                prossima = prossima.succ_opt()?;
            }
            prossima
        }
        Ripetizione::Settimana => data.checked_add_days(chrono::Days::new(7))?,
        Ripetizione::Mese => {
            let (anno, mese) = calendario::shift_month(data.year(), data.month(), 1);
            giorno_nel_mese(anno, mese, giorno)?
        }
        Ripetizione::Anno => giorno_nel_mese(data.year() + 1, data.month(), giorno)?,
    };
    Some(prossima.and_time(ora))
}

/// La prima volta dopo `adesso`: le volte perse mentre il bot era spento si
/// saltano (tre giorni spento non sono tre avvisi di fila). `None` per un
/// promemoria che non si ripete.
pub fn prossima_volta(
    ripetizione: Ripetizione,
    attuale: NaiveDateTime,
    giorno_scelto: Option<u32>,
    adesso: NaiveDateTime,
) -> Option<NaiveDateTime> {
    let mut prossima = volta_dopo(ripetizione, attuale, giorno_scelto)?;
    // Un tetto, per sicurezza: un promemoria quotidiano fermo da dieci anni
    // sono 3650 passi, e non ne serve uno di più.
    for _ in 0..5000 {
        if prossima > adesso {
            return Some(prossima);
        }
        prossima = volta_dopo(ripetizione, prossima, giorno_scelto)?;
    }
    None
}

/// La prima volta che rispetta la ripetizione: un promemoria "dal lunedì
/// al venerdì" fissato di sabato parte lunedì.
pub fn allinea(ripetizione: Ripetizione, quando: NaiveDateTime) -> NaiveDateTime {
    if ripetizione != Ripetizione::Feriali {
        return quando;
    }
    let mut data = quando.date();
    while !e_feriale(data) {
        match data.succ_opt() {
            Some(dopo) => data = dopo,
            None => return quando,
        }
    }
    data.and_time(quando.time())
}

/// "oggi alle 09:00", "domani alle 21:30", "Mar 7 Ott alle 09:00".
pub fn quando_leggibile(quando: NaiveDateTime, adesso: NaiveDateTime) -> String {
    let ora = orario(quando);
    let oggi = adesso.date();
    if quando.date() == oggi {
        format!("oggi alle {ora}")
    } else if Some(quando.date()) == oggi.succ_opt() {
        format!("domani alle {ora}")
    } else if Some(quando.date()) == oggi.pred_opt() {
        format!("ieri alle {ora}")
    } else {
        let data = calendario::format_date(quando.date());
        format!(
            "{} alle {ora}",
            calendario::data_leggibile(&data, oggi.year())
        )
    }
}

/// `AAAA-MM-GG HH:MM`. A mano: `chrono` qui è senza le funzioni di formato
/// (vedi `Cargo.toml`).
pub fn leggi_ora_db(valore: &str) -> Option<NaiveDateTime> {
    let (data, ora) = valore.trim().split_once(' ')?;
    let data = calendario::parse_date(data)?;
    let (ore, minuti) = ora.split_once(':')?;
    if ore.len() != 2 || minuti.len() != 2 {
        return None;
    }
    data.and_hms_opt(ore.parse().ok()?, minuti.parse().ok()?, 0)
}

pub fn scrivi_ora_db(valore: NaiveDateTime) -> String {
    format!(
        "{} {}",
        calendario::format_date(valore.date()),
        orario(valore)
    )
}

/// `HH:MM`, come lo scrive `turni::valida_orario`.
fn leggi_hh_mm(valore: &str) -> Option<NaiveTime> {
    let (ore, minuti) = valore.split_once(':')?;
    NaiveTime::from_hms_opt(ore.parse().ok()?, minuti.parse().ok()?, 0)
}

fn orario(valore: NaiveDateTime) -> String {
    format!("{:02}:{:02}", valore.hour(), valore.minute())
}

const MESI: [(&str, u32); 24] = [
    ("gennaio", 1),
    ("febbraio", 2),
    ("marzo", 3),
    ("aprile", 4),
    ("maggio", 5),
    ("giugno", 6),
    ("luglio", 7),
    ("agosto", 8),
    ("settembre", 9),
    ("ottobre", 10),
    ("novembre", 11),
    ("dicembre", 12),
    ("gen", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("mag", 5),
    ("giu", 6),
    ("lug", 7),
    ("ago", 8),
    ("set", 9),
    ("ott", 10),
    ("nov", 11),
    ("dic", 12),
];

fn mese_scritto(parola: &str) -> Option<u32> {
    MESI.iter()
        .find(|(nome, _)| *nome == parola)
        .map(|(_, numero)| *numero)
}

fn giorno_scritto(parola: &str) -> Option<Weekday> {
    let parola = parola.trim_end_matches(['ì', 'i']);
    Some(match parola {
        "luned" | "lun" => Weekday::Mon,
        "marted" | "mar" => Weekday::Tue,
        "mercoled" | "mer" => Weekday::Wed,
        "gioved" | "gio" => Weekday::Thu,
        "venerd" | "ven" => Weekday::Fri,
        "sabato" | "sab" => Weekday::Sat,
        "domenica" | "dom" => Weekday::Sun,
        _ => return None,
    })
}

/// "fra 2 ore", "tra 10 minuti", "fra mezz'ora", "fra 3 giorni".
fn leggi_fra(resto: &str) -> Option<Duration> {
    let resto = resto.trim();
    match resto {
        "mezz'ora" | "mezzora" | "mezza ora" => return Some(Duration::minutes(30)),
        "un'ora" | "un ora" | "una ora" | "1 ora" => return Some(Duration::hours(1)),
        "un minuto" => return Some(Duration::minutes(1)),
        "un giorno" => return Some(Duration::days(1)),
        "una settimana" => return Some(Duration::weeks(1)),
        _ => {}
    }
    // Il numero, attaccato o no all'unità: "2 ore", "2ore", "10min", "2h".
    let cifre: String = resto.chars().take_while(|c| c.is_ascii_digit()).collect();
    let numero: i64 = cifre.parse().ok()?;
    let unita = resto[cifre.len()..].trim();
    let durata = match unita {
        "m" | "min" | "minuto" | "minuti" => Duration::minutes(numero),
        "h" | "ora" | "ore" => Duration::hours(numero),
        "g" | "giorno" | "giorni" => Duration::days(numero),
        "settimana" | "settimane" => Duration::weeks(numero),
        _ => return None,
    };
    (numero > 0).then_some(durata)
}

/// Un orario scritto: "9", "21:30", "9.30", "9h30".
fn leggi_orario(parola: &str) -> Option<NaiveTime> {
    let parola = parola.replace('h', ":");
    let parola = parola.trim_end_matches(':');
    let orario = crate::modules::turni::valida_orario(parola).ok()?;
    leggi_hh_mm(&orario)
}

/// Quando, scritto come lo scrive una persona (C20). Ritorna `None` se non
/// si capisce; un momento già passato si restituisce lo stesso, e lo dice
/// chi chiama ("è già passato"), invece di spostarlo da solo.
///
/// - `fra 2 ore`, `tra 10 minuti`, `fra mezz'ora`, `fra 3 giorni`;
/// - `domani alle 9`, `oggi 18:30`, `dopodomani`, `15/10 18:30`,
///   `15 ottobre alle 9`, `venerdì alle 20`, `lunedì prossimo`;
/// - `alle 21`, `21:30`: oggi se non è ancora passato, altrimenti domani;
/// - `stasera` (alle 20, e "stasera alle 9" sono le 21), `stamattina`,
///   `domattina` (alle 9), `mezzogiorno`, `mezzanotte`.
///
/// Senza un orario vale le 9 del mattino.
pub fn leggi_quando(testo: &str, adesso: NaiveDateTime) -> Option<NaiveDateTime> {
    let testo = testo
        .trim()
        .to_lowercase()
        .replace(['’', '`'], "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let adesso_al_minuto = adesso.with_second(0)?.with_nanosecond(0)?;
    for prefisso in ["fra ", "tra "] {
        if let Some(resto) = testo.strip_prefix(prefisso) {
            return Some(adesso_al_minuto + leggi_fra(resto)?);
        }
    }

    let oggi = adesso.date();
    let oggi_iso = calendario::format_date(oggi);
    let mut data: Option<NaiveDate> = None;
    let mut orario: Option<NaiveTime> = None;
    let mut orario_predefinito: Option<NaiveTime> = None;
    let mut di_sera = false;
    // "domani alle" senza l'ora è un messaggio partito a metà: non si
    // indovina.
    let mut manca_l_ora = false;
    let parole: Vec<&str> = testo
        .split(' ')
        .flat_map(|parola| parola.split_inclusive('\''))
        .map(|parola| parola.trim_end_matches(','))
        .filter(|parola| !parola.is_empty())
        .collect();
    let mut indice = 0;
    while indice < parole.len() {
        let parola = parole[indice];
        indice += 1;
        match parola {
            "alle" | "all'" | "ore" | "verso" => {
                manca_l_ora = true;
                continue;
            }
            "il" | "di" | "a" | "per" | "le" | "prossimo" | "prossima" | "h" => continue,
            "stasera" => {
                data = Some(oggi);
                orario_predefinito = Some(NaiveTime::from_hms_opt(20, 0, 0)?);
                di_sera = true;
                continue;
            }
            "stamattina" | "stamani" => {
                data = Some(oggi);
                orario_predefinito = Some(NaiveTime::from_hms_opt(9, 0, 0)?);
                continue;
            }
            "domattina" => {
                data = Some(oggi.succ_opt()?);
                orario_predefinito = Some(NaiveTime::from_hms_opt(9, 0, 0)?);
                continue;
            }
            "mezzogiorno" => {
                orario = Some(NaiveTime::from_hms_opt(12, 0, 0)?);
                manca_l_ora = false;
                continue;
            }
            "mezzanotte" => {
                orario = Some(NaiveTime::from_hms_opt(0, 0, 0)?);
                manca_l_ora = false;
                continue;
            }
            _ => {}
        }
        if let Some(giorno) = giorno_scritto(parola) {
            // Il prossimo, mai oggi: "venerdì" detto di venerdì è fra una
            // settimana.
            let mut candidato = oggi.succ_opt()?;
            while candidato.weekday() != giorno {
                candidato = candidato.succ_opt()?;
            }
            data = Some(candidato);
            continue;
        }
        // "15 ottobre", "15 ott 2027".
        if let (Ok(giorno), Some(mese)) = (
            parola.parse::<u32>(),
            parole.get(indice).and_then(|dopo| mese_scritto(dopo)),
        ) {
            indice += 1;
            let anno_scritto = parole
                .get(indice)
                .filter(|dopo| dopo.len() == 4)
                .and_then(|dopo| dopo.parse::<i32>().ok());
            if anno_scritto.is_some() {
                indice += 1;
            }
            let mut candidato =
                NaiveDate::from_ymd_opt(anno_scritto.unwrap_or(oggi.year()), mese, giorno)?;
            if anno_scritto.is_none() && candidato < oggi {
                candidato = NaiveDate::from_ymd_opt(oggi.year() + 1, mese, giorno)?;
            }
            data = Some(candidato);
            continue;
        }
        let sembra_data = parola.contains('/') || parola.contains('-');
        let parola_di_data = matches!(parola, "oggi" | "domani" | "dopodomani");
        if sembra_data || parola_di_data {
            let iso = calendario::leggi_data_scritta(parola, &oggi_iso, calendario::Verso::Futuro)?;
            data = Some(calendario::parse_date(&iso)?);
            continue;
        }
        // "15.10" è un orario se è da solo, una data se c'è già un orario
        // dopo ("15.10 alle 9").
        let c_e_un_orario_dopo = parole[indice..]
            .iter()
            .any(|dopo| leggi_orario(dopo).is_some());
        if parola.contains('.') && c_e_un_orario_dopo && data.is_none() {
            let iso = calendario::leggi_data_scritta(parola, &oggi_iso, calendario::Verso::Futuro)?;
            data = Some(calendario::parse_date(&iso)?);
            continue;
        }
        if orario.is_none() {
            if let Some(letto) = leggi_orario(parola) {
                orario = Some(letto);
                manca_l_ora = false;
                continue;
            }
        }
        return None;
    }

    if manca_l_ora {
        return None;
    }
    if data.is_none() && orario.is_none() && orario_predefinito.is_none() {
        return None;
    }
    let mut orario_scelto = orario
        .or(orario_predefinito)
        .unwrap_or(NaiveTime::from_hms_opt(9, 0, 0)?);
    // "stasera alle 9" sono le 21.
    if di_sera && orario_scelto.hour() < 12 {
        orario_scelto = orario_scelto.with_hour(orario_scelto.hour() + 12)?;
    }
    match data {
        Some(data) => Some(data.and_time(orario_scelto)),
        None => {
            let oggi_a_quell_ora = oggi.and_time(orario_scelto);
            if oggi_a_quell_ora > adesso {
                Some(oggi_a_quell_ora)
            } else {
                Some(oggi.succ_opt()?.and_time(orario_scelto))
            }
        }
    }
}

/// Le scelte rapide della domanda "Quando?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceltaRapida {
    FraUnOra,
    Stasera,
    DomaniMattina,
}

impl SceltaRapida {
    fn token(self) -> &'static str {
        match self {
            SceltaRapida::FraUnOra => "1h",
            SceltaRapida::Stasera => "stasera",
            SceltaRapida::DomaniMattina => "domani9",
        }
    }

    fn da_token(valore: &str) -> Option<Self> {
        [
            SceltaRapida::FraUnOra,
            SceltaRapida::Stasera,
            SceltaRapida::DomaniMattina,
        ]
        .into_iter()
        .find(|scelta| scelta.token() == valore)
    }

    fn etichetta(self) -> &'static str {
        match self {
            SceltaRapida::FraUnOra => "Fra 1 ora",
            SceltaRapida::Stasera => "Stasera alle 20",
            SceltaRapida::DomaniMattina => "Domani alle 9",
        }
    }

    fn quando(self, adesso: NaiveDateTime) -> Option<NaiveDateTime> {
        let adesso = adesso.with_second(0)?.with_nanosecond(0)?;
        match self {
            SceltaRapida::FraUnOra => Some(adesso + Duration::hours(1)),
            SceltaRapida::Stasera => Some(adesso.date().and_hms_opt(20, 0, 0)?),
            SceltaRapida::DomaniMattina => Some(adesso.date().succ_opt()?.and_hms_opt(9, 0, 0)?),
        }
    }

    /// "Stasera alle 20" dopo le 19 non ha senso.
    fn disponibili(adesso: NaiveDateTime) -> Vec<Self> {
        let mut scelte = vec![SceltaRapida::FraUnOra];
        if adesso.hour() < 19 {
            scelte.push(SceltaRapida::Stasera);
        }
        scelte.push(SceltaRapida::DomaniMattina);
        scelte
    }
}

/// Gli anticipi proposti per i pasti, in minuti.
const ANTICIPI_PASTI: [i64; 5] = [0, 15, 30, 60, 120];

fn anticipo_leggibile(minuti: i64) -> String {
    match minuti {
        0 => "all'ora del pasto".to_string(),
        60 => "1 ora prima".to_string(),
        m if m % 60 == 0 => format!("{} ore prima", m / 60),
        m => format!("{m} minuti prima"),
    }
}

/// "fra 30 minuti", "fra 1 ora e 15 minuti", "adesso".
fn fra_quanto(minuti: i64) -> String {
    match minuti {
        m if m <= 0 => "adesso".to_string(),
        m if m < 60 => format!("fra {m} minuti"),
        60 => "fra 1 ora".to_string(),
        m if m % 60 == 0 => format!("fra {} ore", m / 60),
        m if m < 120 => format!("fra 1 ora e {} minuti", m % 60),
        m => format!("fra {} ore e {} minuti", m / 60, m % 60),
    }
}

// ===========================================================================
// Attese di testo, una per chat.
// ===========================================================================

#[derive(Debug, Clone, PartialEq)]
enum Attesa {
    /// "Cosa ti devo ricordare?"
    NuovoTesto,
    /// "Quando?" per un promemoria nuovo.
    NuovoQuando { testo: String },
    /// "Si ripete?": solo pulsanti, ma la bozza deve restare.
    NuovoRipeti { testo: String, quando: String },
    /// Un testo nuovo per un promemoria che c'è già.
    Testo { id: i64 },
    /// Un quando nuovo per un promemoria che c'è già.
    Quando { id: i64 },
    /// L'ora del riepilogo delle scadenze, scritta a mano.
    OraScadenze,
}

fn attese() -> &'static Mutex<HashMap<i64, Attesa>> {
    static ATTESE: OnceLock<Mutex<HashMap<i64, Attesa>>> = OnceLock::new();
    ATTESE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn attesa(chat_id: i64) -> Option<Attesa> {
    attese()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&chat_id)
        .cloned()
}

fn aspetta(chat_id: i64, nuova: Attesa) {
    attese()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(chat_id, nuova);
}

/// Chiude l'attesa dei promemoria di questa chat: la chiama chi apre
/// un'altra schermata, così un testo scritto dopo non finisce qui.
pub fn chiudi_attesa(chat_id: i64) {
    attese()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&chat_id);
}

pub fn attesa_attiva(chat_id: i64) -> bool {
    attesa(chat_id).is_some()
}

// ===========================================================================
// Database.
// ===========================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
struct Riga {
    id: i64,
    testo: String,
    prossimo_il: String,
    ripetizione: String,
    giorno_scelto: Option<i64>,
    stato: String,
}

impl Riga {
    fn ripetizione(&self) -> Ripetizione {
        Ripetizione::da_token(&self.ripetizione).unwrap_or(Ripetizione::Mai)
    }

    fn quando(&self) -> Option<NaiveDateTime> {
        leggi_ora_db(&self.prossimo_il)
    }

    fn giorno_scelto(&self) -> Option<u32> {
        self.giorno_scelto
            .and_then(|giorno| u32::try_from(giorno).ok())
    }

    fn descrizione_ripetizione(&self) -> String {
        match self.quando() {
            Some(quando) => self.ripetizione().etichetta(quando, self.giorno_scelto()),
            None => String::new(),
        }
    }
}

const COLONNE: &str = "id, testo, prossimo_il, ripetizione, giorno_scelto, stato";

/// L'ora locale, al minuto.
pub async fn adesso_locale(pool: &SqlitePool) -> NaiveDateTime {
    let testo: Option<String> =
        sqlx::query_scalar("SELECT strftime('%Y-%m-%d %H:%M', 'now', 'localtime')")
            .fetch_one(pool)
            .await
            .ok();
    testo
        .as_deref()
        .and_then(leggi_ora_db)
        .unwrap_or_else(|| NaiveDate::MIN.and_time(NaiveTime::MIN))
}

fn utente_corrente() -> Option<(i64, Option<i64>)> {
    let attore = crate::identity::current_actor_opt()?;
    Some((attore.utente_id?, Some(attore.spazio_id)))
}

pub async fn crea(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: Option<i64>,
    testo: &str,
    quando: NaiveDateTime,
    ripetizione: Ripetizione,
    origine: &str,
) -> anyhow::Result<i64> {
    let quando = allinea(ripetizione, quando);
    let id = sqlx::query_scalar(
        "INSERT INTO promemoria_liberi (utente_id, spazio_id, testo, prossimo_il, ripetizione, \
                                 giorno_scelto, origine) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(utente_id)
    .bind(spazio_id)
    .bind(testo.trim())
    .bind(scrivi_ora_db(quando))
    .bind(ripetizione.token())
    .bind(ripetizione.giorno_scelto(quando).map(i64::from))
    .bind(origine)
    .fetch_one(pool)
    .await
    .context("Impossibile salvare il promemoria")?;
    Ok(id)
}

async fn leggi(pool: &SqlitePool, utente_id: i64, id: i64) -> Option<Riga> {
    sqlx::query_as(&format!(
        "SELECT {COLONNE} FROM promemoria_liberi WHERE id = ? AND utente_id = ? AND stato <> 'concluso'"
    ))
    .bind(id)
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn elenco(pool: &SqlitePool, utente_id: i64) -> Vec<Riga> {
    sqlx::query_as(&format!(
        "SELECT {COLONNE} FROM promemoria_liberi WHERE utente_id = ? AND stato <> 'concluso' \
         ORDER BY stato = 'sospeso', prossimo_il, id"
    ))
    .bind(utente_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Cambia quando, e riattiva: un promemoria con un quando nuovo è un
/// promemoria che si vuole ricevere.
async fn cambia_quando(
    pool: &SqlitePool,
    utente_id: i64,
    id: i64,
    quando: NaiveDateTime,
) -> anyhow::Result<()> {
    let Some(riga) = leggi(pool, utente_id, id).await else {
        anyhow::bail!("Promemoria non trovato");
    };
    let ripetizione = riga.ripetizione();
    let quando = allinea(ripetizione, quando);
    sqlx::query(
        "UPDATE promemoria_liberi SET prossimo_il = ?, giorno_scelto = ?, stato = 'attivo', \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND utente_id = ?",
    )
    .bind(scrivi_ora_db(quando))
    .bind(ripetizione.giorno_scelto(quando).map(i64::from))
    .bind(id)
    .bind(utente_id)
    .execute(pool)
    .await
    .context("Impossibile cambiare quando")?;
    Ok(())
}

async fn cambia_ripetizione(
    pool: &SqlitePool,
    utente_id: i64,
    id: i64,
    ripetizione: Ripetizione,
) -> anyhow::Result<()> {
    let Some(riga) = leggi(pool, utente_id, id).await else {
        anyhow::bail!("Promemoria non trovato");
    };
    let quando = riga.quando().context("Data del promemoria illeggibile")?;
    let quando = allinea(ripetizione, quando);
    sqlx::query(
        "UPDATE promemoria_liberi SET ripetizione = ?, prossimo_il = ?, giorno_scelto = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND utente_id = ?",
    )
    .bind(ripetizione.token())
    .bind(scrivi_ora_db(quando))
    .bind(ripetizione.giorno_scelto(quando).map(i64::from))
    .bind(id)
    .bind(utente_id)
    .execute(pool)
    .await
    .context("Impossibile cambiare la ripetizione")?;
    Ok(())
}

async fn cambia_testo(
    pool: &SqlitePool,
    utente_id: i64,
    id: i64,
    testo: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE promemoria_liberi SET testo = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND utente_id = ?",
    )
    .bind(testo.trim())
    .bind(id)
    .bind(utente_id)
    .execute(pool)
    .await
    .context("Impossibile cambiare il testo")?;
    Ok(())
}

async fn cambia_stato(pool: &SqlitePool, utente_id: i64, id: i64, stato: &str) -> bool {
    sqlx::query(
        "UPDATE promemoria_liberi SET stato = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND utente_id = ? AND stato <> 'concluso'",
    )
    .bind(stato)
    .bind(id)
    .bind(utente_id)
    .execute(pool)
    .await
    .map(|esito| esito.rows_affected() > 0)
    .unwrap_or(false)
}

async fn elimina(pool: &SqlitePool, utente_id: i64, id: i64) -> bool {
    sqlx::query("DELETE FROM promemoria_liberi WHERE id = ? AND utente_id = ?")
        .bind(id)
        .bind(utente_id)
        .execute(pool)
        .await
        .map(|esito| esito.rows_affected() > 0)
        .unwrap_or(false)
}

/// Le regole automatiche di un utente.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Regole {
    pub pasti_minuti_prima: Option<i64>,
    pub scadenze_ora: Option<String>,
    pub scadenze_giorni: i64,
}

pub async fn regole(pool: &SqlitePool, utente_id: i64) -> Regole {
    let riga: Option<(Option<i64>, Option<String>, i64)> = sqlx::query_as(
        "SELECT pasti_minuti_prima, scadenze_ora, scadenze_giorni \
         FROM promemoria_regole WHERE utente_id = ?",
    )
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    match riga {
        Some((pasti, ora, giorni)) => Regole {
            pasti_minuti_prima: pasti,
            scadenze_ora: ora,
            scadenze_giorni: giorni,
        },
        None => Regole {
            scadenze_giorni: 3,
            ..Regole::default()
        },
    }
}

async fn salva_regole(pool: &SqlitePool, utente_id: i64, regole: &Regole) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO promemoria_regole (utente_id, pasti_minuti_prima, scadenze_ora, scadenze_giorni) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT (utente_id) DO UPDATE SET pasti_minuti_prima = excluded.pasti_minuti_prima, \
             scadenze_ora = excluded.scadenze_ora, scadenze_giorni = excluded.scadenze_giorni, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(utente_id)
    .bind(regole.pasti_minuti_prima)
    .bind(&regole.scadenze_ora)
    .bind(regole.scadenze_giorni)
    .execute(pool)
    .await
    .context("Impossibile salvare le regole dei promemoria")?;
    Ok(())
}

/// L'eccezione di un pasto: `None` se non c'è (vale la regola), `Some(None)`
/// per "non ricordarmelo", `Some(Some(minuti))` per un altro anticipo.
async fn eccezione_pasto(pool: &SqlitePool, utente_id: i64, pasto_id: i64) -> Option<Option<i64>> {
    sqlx::query_scalar(
        "SELECT minuti_prima FROM promemoria_pasti WHERE utente_id = ? AND pasto_id = ?",
    )
    .bind(utente_id)
    .bind(pasto_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

// ===========================================================================
// Il motore: cosa mandare adesso.
// ===========================================================================

/// Un avviso pronto da mandare.
struct Avviso {
    utente_id: i64,
    chiave: String,
    /// Quello che si ripete se lo si rimanda.
    testo: String,
    /// Quello che arriva, con l'intestazione.
    messaggio: String,
}

/// Segna un avviso come mandato **prima** di mandarlo: se c'era già, non
/// parte. Restituisce l'id dell'invio se tocca a noi.
async fn prenota(pool: &SqlitePool, avviso: &Avviso) -> Option<i64> {
    sqlx::query_scalar(
        "INSERT INTO promemoria_invii (utente_id, chiave, testo) VALUES (?, ?, ?) \
         ON CONFLICT (utente_id, chiave) DO NOTHING RETURNING id",
    )
    .bind(avviso.utente_id)
    .bind(&avviso.chiave)
    .bind(&avviso.testo)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

fn tastiera_avviso(invio_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("✅ Fatto", format!("remind:act:{invio_id}:fatto"))],
        vec![
            button("⏰ 10 min", format!("remind:act:{invio_id}:10")),
            button("⏰ 1 ora", format!("remind:act:{invio_id}:60")),
            button("⏰ Domani", format!("remind:act:{invio_id}:domani")),
        ],
    ])
}

async fn chat_di(pool: &SqlitePool, utente_id: i64) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT chat_id FROM account_telegram WHERE utente_id = ? ORDER BY id LIMIT 1",
    )
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn manda(bot: &Bot, pool: &SqlitePool, avviso: Avviso) {
    let Some(invio_id) = prenota(pool, &avviso).await else {
        return;
    };
    let Some(chat_id) = chat_di(pool, avviso.utente_id).await else {
        tracing::warn!(
            utente_id = avviso.utente_id,
            "Promemoria senza una chat a cui mandarlo"
        );
        return;
    };
    match bot
        .manda_avviso(ChatId(chat_id), avviso.messaggio, tastiera_avviso(invio_id))
        .await
    {
        Ok(messaggio) => {
            let _ =
                sqlx::query("UPDATE promemoria_invii SET chat_id = ?, message_id = ? WHERE id = ?")
                    .bind(chat_id)
                    .bind(messaggio.id.0)
                    .bind(invio_id)
                    .execute(pool)
                    .await;
        }
        Err(errore) => {
            tracing::warn!(
                ?errore,
                utente_id = avviso.utente_id,
                "Promemoria non mandato"
            );
        }
    }
}

/// Se un utente ha la sezione accesa (e, per le fonti che ne dipendono,
/// la funzione da cui viene l'avviso).
async fn acceso(
    pool: &SqlitePool,
    utente_id: i64,
    anche: Option<crate::modules::impostazioni::Funzione>,
) -> bool {
    use crate::modules::impostazioni::Funzione;
    let funzioni = crate::modules::impostazioni::funzioni_di(pool, utente_id).await;
    let anche_quella = match anche {
        Some(funzione) => funzioni.attiva(funzione),
        None => true,
    };
    funzioni.attiva(Funzione::Promemoria) && anche_quella
}

/// Un giro del motore. `adesso` arriva da fuori, così le prove decidono che
/// ore sono.
pub async fn controlla(bot: &Bot, pool: &SqlitePool, adesso: NaiveDateTime) {
    if let Err(errore) = controlla_liberi(bot, pool, adesso).await {
        tracing::warn!(?errore, "Promemoria liberi non controllati");
    }
    if let Err(errore) = controlla_pasti(bot, pool, adesso).await {
        tracing::warn!(?errore, "Promemoria dei pasti non controllati");
    }
    if let Err(errore) = controlla_scadenze(bot, pool, adesso).await {
        tracing::warn!(?errore, "Promemoria delle scadenze non controllati");
    }
}

async fn controlla_liberi(
    bot: &Bot,
    pool: &SqlitePool,
    adesso: NaiveDateTime,
) -> anyhow::Result<()> {
    let righe: Vec<(i64, i64, String, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT id, utente_id, testo, prossimo_il, ripetizione, giorno_scelto FROM promemoria_liberi \
         WHERE stato = 'attivo' AND prossimo_il <= ? ORDER BY prossimo_il, id LIMIT 50",
    )
    .bind(scrivi_ora_db(adesso))
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i promemoria da mandare")?;
    for (id, utente_id, testo, prossimo_il, ripetizione, giorno_scelto) in righe {
        let ripetizione = Ripetizione::da_token(&ripetizione).unwrap_or(Ripetizione::Mai);
        let giorno_scelto = giorno_scelto.and_then(|giorno| u32::try_from(giorno).ok());
        let Some(previsto) = leggi_ora_db(&prossimo_il) else {
            continue;
        };
        // Con la sezione spenta il bot non scrive: la volta passa lo stesso,
        // invece di arrivare tutta insieme quando la si riaccende.
        if acceso(pool, utente_id, None).await {
            let mut messaggio = format!("⏰ {testo}");
            if (adesso - previsto).num_minutes() > RITARDO_DA_DIRE {
                messaggio.push_str(&format!(
                    "\n\n🕐 In ritardo: era per {}.",
                    quando_leggibile(previsto, adesso)
                ));
            }
            manda(
                bot,
                pool,
                Avviso {
                    utente_id,
                    chiave: format!("promemoria:{id}:{prossimo_il}"),
                    testo: testo.clone(),
                    messaggio,
                },
            )
            .await;
        }
        match prossima_volta(ripetizione, previsto, giorno_scelto, adesso) {
            Some(prossima) => {
                sqlx::query("UPDATE promemoria_liberi SET prossimo_il = ? WHERE id = ?")
                    .bind(scrivi_ora_db(prossima))
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
            None => {
                sqlx::query("UPDATE promemoria_liberi SET stato = 'concluso' WHERE id = ?")
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(())
}

async fn controlla_pasti(
    bot: &Bot,
    pool: &SqlitePool,
    adesso: NaiveDateTime,
) -> anyhow::Result<()> {
    use crate::modules::impostazioni::Funzione;
    let utenti: Vec<i64> = sqlx::query_scalar(
        "SELECT utente_id FROM promemoria_regole WHERE pasti_minuti_prima IS NOT NULL \
         UNION SELECT utente_id FROM promemoria_pasti WHERE minuti_prima IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere chi vuole i promemoria dei pasti")?;
    let oggi = calendario::format_date(adesso.date());
    let fino_a = calendario::format_date(adesso.date() + Duration::days(2));
    for utente_id in utenti {
        if !acceso(pool, utente_id, Some(Funzione::Planner)).await {
            continue;
        }
        let regola = regole(pool, utente_id).await.pasti_minuti_prima;
        let pasti: Vec<PastoDaRicordare> = sqlx::query_as(
            "SELECT pp.id AS pasto_id, pp.data_pasto AS data, pp.orario, pp.tipo_pasto AS tipo, \
                    pp.ricetta_nome_snapshot AS ricetta, e.minuti_prima AS eccezione, \
                    e.pasto_id IS NOT NULL AS ha_eccezione \
             FROM planner_pasti pp \
             JOIN planner_alimentari pa ON pa.id = pp.planner_id \
             LEFT JOIN promemoria_pasti e ON e.pasto_id = pp.id AND e.utente_id = ? \
             WHERE pa.spazio_id IN (SELECT spazio_id FROM membri_spazio WHERE utente_id = ?) \
               AND pa.archiviato = 0 \
               AND pp.stato = 'pianificato' AND pp.saltato_il IS NULL \
               AND pp.orario IS NOT NULL \
               AND pp.data_pasto BETWEEN ? AND ?",
        )
        .bind(utente_id)
        .bind(utente_id)
        .bind(&oggi)
        .bind(&fino_a)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere i pasti da ricordare")?;
        for pasto in pasti {
            let PastoDaRicordare {
                pasto_id,
                data,
                orario,
                tipo,
                ricetta,
                eccezione,
                ha_eccezione,
            } = pasto;
            let minuti = if ha_eccezione { eccezione } else { regola };
            let Some(minuti) = minuti else {
                continue;
            };
            let Some(ora_pasto) = leggi_ora_db(&format!("{data} {orario}")) else {
                continue;
            };
            let da_avvisare = ora_pasto - Duration::minutes(minuti);
            let ritardo = (adesso - da_avvisare).num_minutes();
            if !(0..=RITARDO_MASSIMO_PASTO).contains(&ritardo) {
                continue;
            }
            let tipo = crate::modules::planner_alimentare::MealType::from_token(&tipo)
                .map(|tipo| tipo.label())
                .unwrap_or("Pasto");
            let testo = format!("🍽️ {tipo} · {ricetta} alle {orario}");
            let mancano = (ora_pasto - adesso).num_minutes();
            manda(
                bot,
                pool,
                Avviso {
                    utente_id,
                    chiave: format!("pasto:{pasto_id}"),
                    messaggio: format!("{testo}\n\nÈ {}.", fra_quanto(mancano)),
                    testo,
                },
            )
            .await;
        }
    }
    Ok(())
}

/// Un pasto del planner che potrebbe meritare un avviso.
#[derive(sqlx::FromRow)]
struct PastoDaRicordare {
    pasto_id: i64,
    data: String,
    orario: String,
    tipo: String,
    ricetta: String,
    /// L'anticipo scelto per questo pasto; `None` con `ha_eccezione` vuol
    /// dire "non ricordarmelo".
    eccezione: Option<i64>,
    ha_eccezione: bool,
}

/// "oggi", "domani", "fra 3 giorni", "scaduto Lun 5 Ott".
fn quando_scade(scadenza: &str, oggi: NaiveDate) -> String {
    let Some(data) = calendario::parse_date(scadenza) else {
        return scadenza.to_string();
    };
    match (data - oggi).num_days() {
        0 => "scade oggi".to_string(),
        1 => "scade domani".to_string(),
        giorni if giorni > 1 => format!("scade fra {giorni} giorni"),
        _ => format!("scaduto {}", calendario::display_date(scadenza)),
    }
}

async fn controlla_scadenze(
    bot: &Bot,
    pool: &SqlitePool,
    adesso: NaiveDateTime,
) -> anyhow::Result<()> {
    use crate::modules::impostazioni::Funzione;
    let utenti: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT utente_id, scadenze_ora, scadenze_giorni FROM promemoria_regole \
         WHERE scadenze_ora IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere chi vuole le scadenze")?;
    let oggi = adesso.date();
    let oggi_iso = calendario::format_date(oggi);
    for (utente_id, ora, giorni) in utenti {
        let Some(ora) = leggi_hh_mm(&ora) else {
            continue;
        };
        if adesso.time() < ora {
            continue;
        }
        if !acceso(pool, utente_id, Some(Funzione::Scorte)).await {
            continue;
        }
        let chiave = format!("scadenze:{oggi_iso}");
        let gia_fatto: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM promemoria_invii WHERE utente_id = ? AND chiave = ?)",
        )
        .bind(utente_id)
        .bind(&chiave)
        .fetch_one(pool)
        .await
        .unwrap_or(true);
        if gia_fatto {
            continue;
        }
        let limite = calendario::format_date(oggi + Duration::days(giorni));
        let scorte: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT descrizione, conservazione, scadenza FROM scorte \
             WHERE spazio_id IN (SELECT spazio_id FROM membri_spazio WHERE utente_id = ?) \
               AND scadenza IS NOT NULL AND scadenza <= ? AND quantita > 0 \
             ORDER BY scadenza, descrizione",
        )
        .bind(utente_id)
        .bind(&limite)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere le scorte che scadono")?;
        if scorte.is_empty() {
            // Niente da dire oggi: si segna lo stesso, per non ricontrollare
            // ogni trenta secondi fino a mezzanotte.
            let _ = sqlx::query(
                "INSERT INTO promemoria_invii (utente_id, chiave) VALUES (?, ?) \
                 ON CONFLICT (utente_id, chiave) DO NOTHING",
            )
            .bind(utente_id)
            .bind(&chiave)
            .execute(pool)
            .await;
            continue;
        }
        let righe: Vec<String> = scorte
            .iter()
            .map(|(descrizione, conservazione, scadenza)| {
                let dove = crate::modules::dispensa::Conservazione::da_token(conservazione)
                    .map(|dove| dove.etichetta())
                    .unwrap_or("");
                format!(
                    "• {descrizione} · {dove} — {}",
                    quando_scade(scadenza, oggi)
                )
            })
            .collect();
        let testo = format!("🥫 Scorte che scadono\n\n{}", righe.join("\n"));
        manda(
            bot,
            pool,
            Avviso {
                utente_id,
                chiave,
                messaggio: testo.clone(),
                testo,
            },
        )
        .await;
    }
    Ok(())
}

/// I pulsanti di un avviso arrivato: `✅ Fatto` lo toglie, `⏰` lo rimanda
/// con un promemoria nuovo, una volta sola. Non passano dalla schermata
/// attiva: l'avviso non è una schermata, e i suoi pulsanti valgono anche
/// quando la schermata è cambiata.
pub async fn gestisci_avviso(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    message_id: MessageId,
    data: &str,
) -> ResponseResult<()> {
    let Some(resto) = data.strip_prefix("remind:act:") else {
        return Ok(());
    };
    let Some((invio, scelta)) = resto.split_once(':') else {
        return Ok(());
    };
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let Ok(invio) = invio.parse::<i64>() else {
        return Ok(());
    };
    let testo: Option<String> =
        sqlx::query_scalar("SELECT testo FROM promemoria_invii WHERE id = ? AND utente_id = ?")
            .bind(invio)
            .bind(utente_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let Some(testo) = testo else {
        let _ = bot.delete_message(chat_id, message_id).await;
        return Ok(());
    };
    let adesso = adesso_locale(pool).await;
    let quando = match scelta {
        "fatto" => None,
        "10" => Some(adesso + Duration::minutes(10)),
        "60" => Some(adesso + Duration::hours(1)),
        "domani" => Some(adesso + Duration::days(1)),
        _ => return Ok(()),
    };
    let esito = if quando.is_some() {
        "rimandato"
    } else {
        "fatto"
    };
    let _ = sqlx::query("UPDATE promemoria_invii SET esito = ?, esito_il = ? WHERE id = ?")
        .bind(esito)
        .bind(scrivi_ora_db(adesso))
        .bind(invio)
        .execute(pool)
        .await;
    if let Err(errore) = bot.delete_message(chat_id, message_id).await {
        tracing::debug!(?errore, "Avviso non eliminabile");
    }
    if let Some(quando) = quando {
        match crea(
            pool,
            utente_id,
            spazio_id,
            &testo,
            quando,
            Ripetizione::Mai,
            "rimandato",
        )
        .await
        {
            Ok(_) => {
                bot.avviso_che_sparisce(
                    chat_id,
                    &format!("⏰ Te lo ricordo {}.", quando_leggibile(quando, adesso)),
                    std::time::Duration::from_secs(4),
                )
                .await;
            }
            Err(errore) => {
                tracing::warn!(?errore, "Promemoria non rimandato");
                bot.avviso_che_sparisce(
                    chat_id,
                    "⚠️ Non sono riuscito a rimandarlo.",
                    std::time::Duration::from_secs(6),
                )
                .await;
            }
        }
    }
    Ok(())
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

fn annulla_row() -> Vec<InlineKeyboardButton> {
    vec![
        button("❌ Annulla", "remind:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]
}

/// Una riga di elenco: "oggi alle 09:00 — Chiama il medico 🔁".
fn riga_elenco(riga: &Riga, adesso: NaiveDateTime) -> String {
    let quando = riga
        .quando()
        .map(|quando| quando_leggibile(quando, adesso))
        .unwrap_or_default();
    let si_ripete = if riga.ripetizione() == Ripetizione::Mai {
        ""
    } else {
        " 🔁"
    };
    let sospeso = if riga.stato == "sospeso" { "⏸ " } else { "" };
    format!("• {sospeso}{quando} — {}{si_ripete}", riga.testo)
}

pub async fn mostra_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let adesso = adesso_locale(pool).await;
    let tutti = elenco(pool, utente_id).await;
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str("⏰ Promemoria");
    let in_arrivo: Vec<String> = tutti
        .iter()
        .filter(|riga| riga.stato == "attivo")
        .take(5)
        .map(|riga| riga_elenco(riga, adesso))
        .collect();
    if in_arrivo.is_empty() {
        testo.push_str("\n\nNessun promemoria in arrivo.\nCreane uno con ➕ Nuovo promemoria.");
    } else {
        testo.push_str(&format!("\n\nIn arrivo:\n{}", in_arrivo.join("\n")));
    }
    let regole = regole(pool, utente_id).await;
    let mut automatici = Vec::new();
    if let Some(minuti) = regole.pasti_minuti_prima {
        automatici.push(format!("pasti {}", anticipo_leggibile(minuti)));
    }
    if let Some(ora) = &regole.scadenze_ora {
        automatici.push(format!("scadenze alle {ora}"));
    }
    if !automatici.is_empty() {
        testo.push_str(&format!("\n\n🔁 Automatici: {}.", automatici.join(" · ")));
    }

    let mut rows = vec![vec![button("➕ Nuovo promemoria", "remind:new")]];
    if !tutti.is_empty() {
        rows.push(vec![button(
            liste::etichetta_con_conteggio("📋 Tutti i promemoria", tutti.len() as i64),
            "remind:list:0",
        )]);
    }
    rows.push(vec![button("🔁 Automatici", "remind:auto")]);
    rows.push(vec![button("🏠 Menù principale", "menu:main")]);
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_elenco(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    pagina: i64,
) -> ResponseResult<()> {
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let adesso = adesso_locale(pool).await;
    let tutti = elenco(pool, utente_id).await;
    if tutti.is_empty() {
        return mostra_menu(bot, chat_id, pool, None).await;
    }
    let totale = tutti.len() as i64;
    let pagina = liste::pagina_valida(pagina, totale);
    let testo = format!(
        "{}\n\n⏸ = sospeso, 🔁 = si ripete.",
        liste::intestazione("📋 Tutti i promemoria", totale, pagina)
    );
    let mut rows: Vec<Vec<InlineKeyboardButton>> = tutti
        .iter()
        .skip(liste::scarto(pagina) as usize)
        .take(liste::VOCI_PER_PAGINA)
        .map(|riga| {
            let quando = riga
                .quando()
                .map(|quando| quando_leggibile(quando, adesso))
                .unwrap_or_default();
            let segno = if riga.stato == "sospeso" {
                "⏸"
            } else {
                "⏰"
            };
            let si_ripete = if riga.ripetizione() == Ripetizione::Mai {
                ""
            } else {
                "🔁 "
            };
            vec![button(
                format!(
                    "{segno} {si_ripete}{} · {quando}",
                    liste::tronca(&riga.testo, 24)
                ),
                format!("remind:view:{}", riga.id),
            )]
        })
        .collect();
    if let Some(riga) = liste::riga_paginazione_da_totale(pagina, totale, "remind:noop", |p| {
        format!("remind:list:{p}")
    }) {
        rows.push(riga);
    }
    rows.push(nav_row("remind:menu"));
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// La scheda di un promemoria, con "Indietro" verso l'elenco.
async fn mostra_scheda(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    mostra_scheda_con_indietro(bot, chat_id, pool, id, avviso, "remind:list:0").await
}

async fn mostra_scheda_con_indietro(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
    indietro: &str,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let Some(riga) = leggi(pool, utente_id, id).await else {
        return mostra_menu(
            bot,
            chat_id,
            pool,
            Some("⚠️ Questo promemoria non c'è più."),
        )
        .await;
    };
    let adesso = adesso_locale(pool).await;
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    let quando = riga
        .quando()
        .map(|quando| quando_leggibile(quando, adesso))
        .unwrap_or_default();
    testo.push_str(&format!(
        "⏰ {}\n\n🕐 {}{}\n🔁 {}",
        riga.testo,
        primo_maiuscolo(&quando),
        if riga.stato == "sospeso" {
            " — ⏸ sospeso, non arriva"
        } else {
            ""
        },
        riga.descrizione_ripetizione()
    ));
    let pausa = if riga.stato == "sospeso" {
        button("▶️ Riattiva", format!("remind:resume:{id}"))
    } else {
        button("⏸ Sospendi", format!("remind:pause:{id}"))
    };
    let rows = vec![
        vec![
            button("✏️ Testo", format!("remind:edit:text:{id}")),
            button("🕐 Quando", format!("remind:edit:when:{id}")),
        ],
        vec![
            button("🔁 Ripetizione", format!("remind:edit:rep:{id}")),
            pausa,
        ],
        vec![button("🗑️ Elimina", format!("remind:del:ask:{id}"))],
        nav_row(indietro),
    ];
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

fn primo_maiuscolo(testo: &str) -> String {
    let mut lettere = testo.chars();
    match lettere.next() {
        Some(prima) => prima.to_uppercase().collect::<String>() + lettere.as_str(),
        None => String::new(),
    }
}

async fn chiedi_testo(bot: &Bot, chat_id: ChatId, avviso: Option<&str>) -> ResponseResult<()> {
    aspetta(chat_id.0, Attesa::NuovoTesto);
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str("➕ Nuovo promemoria\n\nCosa ti devo ricordare? Scrivilo qui sotto.");
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row()]))
        .await?;
    Ok(())
}

const ESEMPI_QUANDO: &str =
    "Scegli qui sotto, oppure scrivilo: domani alle 9, fra 2 ore, 15/10 18:30, venerdì alle 20.";

async fn chiedi_quando(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    di_cosa: &str,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let adesso = adesso_locale(pool).await;
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str(&format!(
        "🕐 Quando te lo ricordo?\n\n«{di_cosa}»\n\n{ESEMPI_QUANDO}"
    ));
    let scelte: Vec<InlineKeyboardButton> = SceltaRapida::disponibili(adesso)
        .into_iter()
        .map(|scelta| {
            button(
                scelta.etichetta(),
                format!("remind:when:{}", scelta.token()),
            )
        })
        .collect();
    let rows = vec![scelte, annulla_row()];
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn chiedi_ripetizione(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    di_cosa: &str,
    quando: NaiveDateTime,
    destinazione: &str,
) -> ResponseResult<()> {
    let adesso = adesso_locale(pool).await;
    let testo = format!(
        "🔁 Si ripete?\n\n«{di_cosa}», {}.",
        quando_leggibile(quando, adesso)
    );
    let mut rows: Vec<Vec<InlineKeyboardButton>> = RIPETIZIONI
        .iter()
        .map(|ripetizione| {
            vec![button(
                ripetizione.etichetta(quando, None),
                format!("remind:rep:{destinazione}:{}", ripetizione.token()),
            )]
        })
        .collect();
    rows.push(annulla_row());
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_automatici(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    use crate::modules::impostazioni::Funzione;
    chiudi_attesa(chat_id.0);
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let regole = regole(pool, utente_id).await;
    let funzioni = crate::modules::impostazioni::funzioni(pool).await;
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str("🔁 Promemoria automatici\n\n");
    testo.push_str(&match regole.pasti_minuti_prima {
        Some(minuti) => format!(
            "🍽️ Pasti del planner: {}, per i pasti con un orario.",
            anticipo_leggibile(minuti)
        ),
        None => "🍽️ Pasti del planner: spento.".to_string(),
    });
    if regole.pasti_minuti_prima.is_some() && !funzioni.attiva(Funzione::Planner) {
        testo.push_str("\n⚠️ Il Planner è spento nelle Impostazioni: non arriva niente.");
    }
    testo.push_str("\n\n");
    testo.push_str(&match &regole.scadenze_ora {
        Some(ora) => format!(
            "🥫 Scorte che scadono: ogni giorno alle {ora}, per quello che scade {}. Se non scade niente, non ti scrivo.",
            entro_giorni(regole.scadenze_giorni)
        ),
        None => "🥫 Scorte che scadono: spento.".to_string(),
    });
    if regole.scadenze_ora.is_some() && !funzioni.attiva(Funzione::Scorte) {
        testo.push_str("\n⚠️ Le Scorte sono spente nelle Impostazioni: non arriva niente.");
    }
    testo.push_str(
        "\n\nUn pasto alla volta si cambia dal suo dettaglio nel planner: ⏰ Promemoria.",
    );
    let rows = vec![
        vec![button("🍽️ Pasti: cambia", "remind:auto:pasti")],
        vec![button("🥫 Scadenze: cambia", "remind:auto:scad")],
        nav_row("remind:menu"),
    ];
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

fn entro_giorni(giorni: i64) -> String {
    match giorni {
        0 => "oggi".to_string(),
        1 => "entro domani".to_string(),
        n => format!("entro {n} giorni"),
    }
}

async fn chiedi_anticipo_pasti(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = ANTICIPI_PASTI
        .iter()
        .map(|minuti| {
            vec![button(
                primo_maiuscolo(&anticipo_leggibile(*minuti)),
                format!("remind:auto:pasti:{minuti}"),
            )]
        })
        .collect();
    rows.push(vec![button("🔕 Spento", "remind:auto:pasti:off")]);
    rows.push(nav_row("remind:auto"));
    bot.send_message(
        chat_id,
        "🍽️ Pasti del planner\n\nQuanto prima di ogni pasto ti scrivo? Vale per i pasti con un orario, ancora da mangiare.",
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

const ORE_SCADENZE: [&str; 4] = ["08:00", "09:00", "12:00", "18:00"];
const GIORNI_SCADENZE: [i64; 5] = [0, 1, 2, 3, 7];

async fn chiedi_ora_scadenze(
    bot: &Bot,
    chat_id: ChatId,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    aspetta(chat_id.0, Attesa::OraScadenze);
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str("🥫 Scorte che scadono\n\nA che ora ti mando il riepilogo? Scegli o scrivi l'ora (es. 7:30).");
    let ore: Vec<InlineKeyboardButton> = ORE_SCADENZE
        .iter()
        .map(|ora| {
            button(
                *ora,
                format!("remind:auto:scad:ora:{}", ora.replace(':', "")),
            )
        })
        .collect();
    let rows = vec![
        ore,
        vec![button("🔕 Spento", "remind:auto:scad:off")],
        nav_row("remind:auto"),
    ];
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn chiedi_giorni_scadenze(bot: &Bot, chat_id: ChatId, ora: &str) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let giorni: Vec<InlineKeyboardButton> = GIORNI_SCADENZE
        .iter()
        .map(|giorni| {
            button(
                primo_maiuscolo(&entro_giorni(*giorni)),
                format!("remind:auto:scad:giorni:{}:{giorni}", ora.replace(':', "")),
            )
        })
        .collect();
    bot.send_message(
        chat_id,
        format!("🥫 Scorte che scadono, alle {ora}\n\nDi quello che scade quando? Quello già scaduto c'è sempre."),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        giorni,
        nav_row("remind:auto:scad"),
    ]))
    .await?;
    Ok(())
}

/// Il promemoria di un pasto, dal suo dettaglio nel planner.
async fn mostra_pasto(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    pasto_id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let pasto: Option<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT pp.tipo_pasto, pp.ricetta_nome_snapshot, pp.orario, pp.data_pasto \
         FROM planner_pasti pp JOIN planner_alimentari pa ON pa.id = pp.planner_id \
         WHERE pp.id = ? AND pa.spazio_id IN (SELECT spazio_id FROM membri_spazio WHERE utente_id = ?)",
    )
    .bind(pasto_id)
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let Some((tipo, ricetta, orario, data)) = pasto else {
        return mostra_menu(bot, chat_id, pool, Some("⚠️ Questo pasto non c'è più.")).await;
    };
    let tipo = crate::modules::planner_alimentare::MealType::from_token(&tipo)
        .map(|tipo| tipo.label())
        .unwrap_or("Pasto");
    let indietro = format!("planner:view:{pasto_id}");
    let mut testo = String::new();
    if let Some(avviso) = avviso {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    testo.push_str(&format!(
        "⏰ Promemoria del pasto\n\n🍽️ {tipo} · {ricetta}\n📅 {}",
        calendario::display_date(&data)
    ));
    let Some(orario) = orario else {
        testo.push_str("\n\nQuesto pasto non ha un orario: dagliene uno con ✏️ Modifica, e potrò ricordartelo.");
        bot.send_message(chat_id, testo)
            .reply_markup(InlineKeyboardMarkup::new(vec![nav_row(&indietro)]))
            .await?;
        return Ok(());
    };
    testo.push_str(&format!(" alle {orario}"));
    let regola = regole(pool, utente_id).await.pasti_minuti_prima;
    let adesso = match eccezione_pasto(pool, utente_id, pasto_id).await {
        Some(Some(minuti)) => format!(
            "Adesso: {}, solo per questo pasto.",
            anticipo_leggibile(minuti)
        ),
        Some(None) => "Adesso: non te lo ricordo, solo per questo pasto.".to_string(),
        None => match regola {
            Some(minuti) => format!(
                "Adesso: {}, come tutti i pasti.",
                anticipo_leggibile(minuti)
            ),
            None => "Adesso: non te lo ricordo (i promemoria dei pasti sono spenti).".to_string(),
        },
    };
    testo.push_str(&format!("\n\n{adesso}"));
    let mut rows: Vec<Vec<InlineKeyboardButton>> = ANTICIPI_PASTI
        .iter()
        .map(|minuti| {
            vec![button(
                primo_maiuscolo(&anticipo_leggibile(*minuti)),
                format!("remind:meal:{pasto_id}:{minuti}"),
            )]
        })
        .collect();
    rows.push(vec![button(
        "🔕 Non ricordarmelo",
        format!("remind:meal:{pasto_id}:off"),
    )]);
    rows.push(vec![button(
        "↩️ Come tutti i pasti",
        format!("remind:meal:{pasto_id}:regola"),
    )]);
    rows.push(nav_row(&indietro));
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn cambia_eccezione_pasto(
    pool: &SqlitePool,
    utente_id: i64,
    pasto_id: i64,
    scelta: &str,
) -> anyhow::Result<()> {
    if scelta == "regola" {
        sqlx::query("DELETE FROM promemoria_pasti WHERE utente_id = ? AND pasto_id = ?")
            .bind(utente_id)
            .bind(pasto_id)
            .execute(pool)
            .await?;
        return Ok(());
    }
    let minuti: Option<i64> = if scelta == "off" {
        None
    } else {
        Some(scelta.parse().context("Anticipo non valido")?)
    };
    sqlx::query(
        "INSERT INTO promemoria_pasti (utente_id, pasto_id, minuti_prima) VALUES (?, ?, ?) \
         ON CONFLICT (utente_id, pasto_id) DO UPDATE SET minuti_prima = excluded.minuti_prima",
    )
    .bind(utente_id)
    .bind(pasto_id)
    .bind(minuti)
    .execute(pool)
    .await?;
    Ok(())
}

// ===========================================================================
// Ingressi.
// ===========================================================================

async fn salva_nuovo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    testo: &str,
    quando: NaiveDateTime,
    ripetizione: Ripetizione,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    match crea(
        pool,
        utente_id,
        spazio_id,
        testo,
        quando,
        ripetizione,
        "libero",
    )
    .await
    {
        Ok(id) => {
            let adesso = adesso_locale(pool).await;
            let quando = allinea(ripetizione, quando);
            let avviso = format!(
                "✅ Promemoria salvato: te lo ricordo {}.",
                quando_leggibile(quando, adesso)
            );
            // Appena creato si torna da dove si era partiti, il menù
            // (collaudo di bc29b7b).
            mostra_scheda_con_indietro(bot, chat_id, pool, id, Some(&avviso), "remind:menu").await
        }
        Err(errore) => {
            tracing::warn!(?errore, "Promemoria non salvato");
            mostra_menu(bot, chat_id, pool, Some("⚠️ Non sono riuscito a salvarlo.")).await
        }
    }
}

/// Un quando scelto (scritto o con un pulsante), per la bozza o per un
/// promemoria che c'è già.
async fn quando_scelto(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    quando: NaiveDateTime,
) -> ResponseResult<()> {
    let adesso = adesso_locale(pool).await;
    match attesa(chat_id.0) {
        Some(Attesa::NuovoQuando { testo }) => {
            if quando <= adesso {
                return chiedi_quando(
                    bot,
                    chat_id,
                    pool,
                    &testo,
                    Some("⚠️ È già passato: scegli un momento che deve ancora venire."),
                )
                .await;
            }
            aspetta(
                chat_id.0,
                Attesa::NuovoRipeti {
                    testo: testo.clone(),
                    quando: scrivi_ora_db(quando),
                },
            );
            chiedi_ripetizione(bot, chat_id, pool, &testo, quando, "new").await
        }
        Some(Attesa::Quando { id }) => {
            let Some((utente_id, _)) = utente_corrente() else {
                return Ok(());
            };
            let Some(riga) = leggi(pool, utente_id, id).await else {
                return mostra_menu(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Questo promemoria non c'è più."),
                )
                .await;
            };
            if quando <= adesso {
                return chiedi_quando(
                    bot,
                    chat_id,
                    pool,
                    &riga.testo,
                    Some("⚠️ È già passato: scegli un momento che deve ancora venire."),
                )
                .await;
            }
            match cambia_quando(pool, utente_id, id, quando).await {
                Ok(()) => {
                    let quando = allinea(riga.ripetizione(), quando);
                    let avviso = format!(
                        "✅ Fatto: te lo ricordo {}.",
                        quando_leggibile(quando, adesso)
                    );
                    mostra_scheda(bot, chat_id, pool, id, Some(&avviso)).await
                }
                Err(errore) => {
                    tracing::warn!(?errore, "Quando non cambiato");
                    mostra_scheda(
                        bot,
                        chat_id,
                        pool,
                        id,
                        Some("⚠️ Non sono riuscito a cambiarlo."),
                    )
                    .await
                }
            }
        }
        _ => mostra_menu(bot, chat_id, pool, None).await,
    }
}

/// Il testo scritto mentre i promemoria aspettano qualcosa. `false` se non
/// aspettano niente: il messaggio è di qualcun altro.
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
    let scritto = text.trim();
    match attesa_aperta {
        Attesa::NuovoTesto => {
            if scritto.is_empty() {
                chiedi_testo(bot, chat_id, Some("⚠️ Scrivi cosa ti devo ricordare.")).await?;
                return Ok(true);
            }
            aspetta(
                chat_id.0,
                Attesa::NuovoQuando {
                    testo: scritto.to_string(),
                },
            );
            chiedi_quando(bot, chat_id, pool, scritto, None).await?;
        }
        Attesa::NuovoQuando { testo } => {
            let adesso = adesso_locale(pool).await;
            match leggi_quando(scritto, adesso) {
                Some(quando) => quando_scelto(bot, chat_id, pool, quando).await?,
                None => {
                    chiedi_quando(bot, chat_id, pool, &testo, Some("⚠️ Non ho capito quando."))
                        .await?
                }
            }
        }
        Attesa::NuovoRipeti { testo, quando } => {
            let quando = leggi_ora_db(&quando).unwrap_or_default();
            bot.send_message(chat_id, "🔁 Scegli con i pulsanti se si ripete.")
                .await?;
            chiedi_ripetizione(bot, chat_id, pool, &testo, quando, "new").await?;
        }
        Attesa::Testo { id } => {
            let Some((utente_id, _)) = utente_corrente() else {
                return Ok(true);
            };
            if scritto.is_empty() {
                return Ok(true);
            }
            let avviso = match cambia_testo(pool, utente_id, id, scritto).await {
                Ok(()) => "✅ Testo cambiato.",
                Err(_) => "⚠️ Non sono riuscito a cambiarlo.",
            };
            mostra_scheda(bot, chat_id, pool, id, Some(avviso)).await?;
        }
        Attesa::Quando { id } => {
            let adesso = adesso_locale(pool).await;
            match leggi_quando(scritto, adesso) {
                Some(quando) => quando_scelto(bot, chat_id, pool, quando).await?,
                None => {
                    let Some((utente_id, _)) = utente_corrente() else {
                        return Ok(true);
                    };
                    let testo = leggi(pool, utente_id, id)
                        .await
                        .map(|riga| riga.testo)
                        .unwrap_or_default();
                    chiedi_quando(bot, chat_id, pool, &testo, Some("⚠️ Non ho capito quando."))
                        .await?
                }
            }
        }
        Attesa::OraScadenze => match crate::modules::turni::valida_orario(scritto) {
            Ok(ora) => chiedi_giorni_scadenze(bot, chat_id, &ora).await?,
            Err(_) => {
                chiedi_ora_scadenze(
                    bot,
                    chat_id,
                    Some("⚠️ Non è un'ora: scrivila così, 7:30 oppure 18."),
                )
                .await?
            }
        },
    }
    Ok(true)
}

fn ora_da_callback(valore: &str) -> Option<String> {
    (valore.len() == 4 && valore.bytes().all(|b| b.is_ascii_digit()))
        .then(|| format!("{}:{}", &valore[..2], &valore[2..]))
}

/// I pulsanti `remind:`. `false` se il callback non è di questo modulo.
pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    let Some(resto) = data.strip_prefix("remind:") else {
        return Ok(false);
    };
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(false);
    };
    match resto {
        "menu" => mostra_menu(bot, chat_id, pool, None).await?,
        "noop" => {}
        "new" => chiedi_testo(bot, chat_id, None).await?,
        "cancel" => {
            let torna_alla_scheda = match attesa(chat_id.0) {
                Some(Attesa::Testo { id }) | Some(Attesa::Quando { id }) => Some(id),
                _ => None,
            };
            chiudi_attesa(chat_id.0);
            match torna_alla_scheda {
                Some(id) => {
                    mostra_scheda(bot, chat_id, pool, id, Some("❌ Operazione annullata.")).await?
                }
                None => mostra_menu(bot, chat_id, pool, Some("❌ Operazione annullata.")).await?,
            }
        }
        "auto" => mostra_automatici(bot, chat_id, pool, None).await?,
        "auto:pasti" => chiedi_anticipo_pasti(bot, chat_id).await?,
        "auto:scad" => chiedi_ora_scadenze(bot, chat_id, None).await?,
        "auto:scad:off" => {
            let mut nuove = regole(pool, utente_id).await;
            nuove.scadenze_ora = None;
            let avviso = match salva_regole(pool, utente_id, &nuove).await {
                Ok(()) => "✅ Riepilogo delle scadenze spento.",
                Err(_) => "⚠️ Non sono riuscito a salvarlo.",
            };
            mostra_automatici(bot, chat_id, pool, Some(avviso)).await?;
        }
        _ => return gestisci_con_argomento(bot, chat_id, pool, utente_id, resto).await,
    }
    Ok(true)
}

async fn gestisci_con_argomento(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    utente_id: i64,
    resto: &str,
) -> ResponseResult<bool> {
    if let Some(scelta) = resto.strip_prefix("auto:pasti:") {
        let mut nuove = regole(pool, utente_id).await;
        nuove.pasti_minuti_prima = if scelta == "off" {
            None
        } else {
            match scelta.parse::<i64>() {
                Ok(minuti) => Some(minuti),
                Err(_) => return Ok(true),
            }
        };
        let avviso = match (
            salva_regole(pool, utente_id, &nuove).await,
            nuove.pasti_minuti_prima,
        ) {
            (Ok(()), Some(minuti)) => {
                format!("✅ Ti ricordo i pasti {}.", anticipo_leggibile(minuti))
            }
            (Ok(()), None) => "✅ Promemoria dei pasti spenti.".to_string(),
            (Err(_), _) => "⚠️ Non sono riuscito a salvarlo.".to_string(),
        };
        mostra_automatici(bot, chat_id, pool, Some(&avviso)).await?;
        return Ok(true);
    }
    if let Some(ora) = resto.strip_prefix("auto:scad:ora:") {
        if let Some(ora) = ora_da_callback(ora) {
            chiedi_giorni_scadenze(bot, chat_id, &ora).await?;
        }
        return Ok(true);
    }
    if let Some(valori) = resto.strip_prefix("auto:scad:giorni:") {
        let Some((ora, giorni)) = valori.split_once(':') else {
            return Ok(true);
        };
        let (Some(ora), Ok(giorni)) = (ora_da_callback(ora), giorni.parse::<i64>()) else {
            return Ok(true);
        };
        let mut nuove = regole(pool, utente_id).await;
        nuove.scadenze_ora = Some(ora.clone());
        nuove.scadenze_giorni = giorni;
        let avviso = match salva_regole(pool, utente_id, &nuove).await {
            Ok(()) => format!(
                "✅ Ogni giorno alle {ora} ti dico cosa scade {}.",
                entro_giorni(giorni)
            ),
            Err(_) => "⚠️ Non sono riuscito a salvarlo.".to_string(),
        };
        mostra_automatici(bot, chat_id, pool, Some(&avviso)).await?;
        return Ok(true);
    }
    if let Some(valori) = resto.strip_prefix("meal:") {
        let (pasto, scelta) = match valori.split_once(':') {
            Some((pasto, scelta)) => (pasto, Some(scelta)),
            None => (valori, None),
        };
        let Ok(pasto_id) = pasto.parse::<i64>() else {
            return Ok(true);
        };
        let avviso = match scelta {
            None => None,
            Some(scelta) => Some(
                match cambia_eccezione_pasto(pool, utente_id, pasto_id, scelta).await {
                    Ok(()) => "✅ Fatto.",
                    Err(_) => "⚠️ Non sono riuscito a salvarlo.",
                },
            ),
        };
        mostra_pasto(bot, chat_id, pool, pasto_id, avviso).await?;
        return Ok(true);
    }
    if let Some(scelta) = resto.strip_prefix("when:") {
        let adesso = adesso_locale(pool).await;
        if let Some(quando) =
            SceltaRapida::da_token(scelta).and_then(|scelta| scelta.quando(adesso))
        {
            quando_scelto(bot, chat_id, pool, quando).await?;
        }
        return Ok(true);
    }
    if let Some(valori) = resto.strip_prefix("rep:") {
        let Some((destinazione, token)) = valori.split_once(':') else {
            return Ok(true);
        };
        let Some(ripetizione) = Ripetizione::da_token(token) else {
            return Ok(true);
        };
        if destinazione == "new" {
            match attesa(chat_id.0) {
                Some(Attesa::NuovoRipeti { testo, quando }) => {
                    let quando = leggi_ora_db(&quando).unwrap_or_default();
                    salva_nuovo(bot, chat_id, pool, &testo, quando, ripetizione).await?;
                }
                _ => {
                    mostra_menu(
                        bot,
                        chat_id,
                        pool,
                        Some("⚠️ Questo promemoria non era più in preparazione: ricomincia da ➕ Nuovo promemoria."),
                    )
                    .await?
                }
            }
            return Ok(true);
        }
        let Ok(id) = destinazione.parse::<i64>() else {
            return Ok(true);
        };
        let avviso = match cambia_ripetizione(pool, utente_id, id, ripetizione).await {
            Ok(()) => "✅ Ripetizione cambiata.",
            Err(_) => "⚠️ Non sono riuscito a cambiarla.",
        };
        mostra_scheda(bot, chat_id, pool, id, Some(avviso)).await?;
        return Ok(true);
    }
    if let Some(pagina) = resto.strip_prefix("list:") {
        mostra_elenco(bot, chat_id, pool, pagina.parse().unwrap_or(0)).await?;
        return Ok(true);
    }
    let id_dopo = |prefisso: &str| -> Option<i64> { resto.strip_prefix(prefisso)?.parse().ok() };
    if let Some(id) = id_dopo("view:") {
        mostra_scheda(bot, chat_id, pool, id, None).await?;
    } else if let Some(id) = id_dopo("edit:text:") {
        let Some(riga) = leggi(pool, utente_id, id).await else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        aspetta(chat_id.0, Attesa::Testo { id });
        bot.send_message(
            chat_id,
            format!(
                "✏️ Testo del promemoria\n\nAdesso: «{}»\n\nScrivi quello nuovo.",
                riga.testo
            ),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![annulla_row()]))
        .await?;
    } else if let Some(id) = id_dopo("edit:when:") {
        let Some(riga) = leggi(pool, utente_id, id).await else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        aspetta(chat_id.0, Attesa::Quando { id });
        chiedi_quando(bot, chat_id, pool, &riga.testo, None).await?;
    } else if let Some(id) = id_dopo("edit:rep:") {
        let Some(riga) = leggi(pool, utente_id, id).await else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        let quando = riga.quando().unwrap_or_default();
        let mut rows: Vec<Vec<InlineKeyboardButton>> = RIPETIZIONI
            .iter()
            .map(|ripetizione| {
                let segno = if *ripetizione == riga.ripetizione() {
                    "✅ "
                } else {
                    ""
                };
                vec![button(
                    format!(
                        "{segno}{}",
                        ripetizione.etichetta(quando, riga.giorno_scelto())
                    ),
                    format!("remind:rep:{id}:{}", ripetizione.token()),
                )]
            })
            .collect();
        rows.push(nav_row(&format!("remind:view:{id}")));
        bot.send_message(chat_id, format!("🔁 Si ripete?\n\n«{}»", riga.testo))
            .reply_markup(InlineKeyboardMarkup::new(rows))
            .await?;
    } else if let Some(id) = id_dopo("pause:") {
        let avviso = if cambia_stato(pool, utente_id, id, "sospeso").await {
            "⏸ Sospeso: non arriva finché non lo riattivi."
        } else {
            "⚠️ Non sono riuscito a sospenderlo."
        };
        mostra_scheda(bot, chat_id, pool, id, Some(avviso)).await?;
    } else if let Some(id) = id_dopo("resume:") {
        riattiva(bot, chat_id, pool, utente_id, id).await?;
    } else if let Some(id) = id_dopo("del:ask:") {
        let Some(riga) = leggi(pool, utente_id, id).await else {
            return mostra_menu(bot, chat_id, pool, None).await.map(|()| true);
        };
        bot.send_message(
            chat_id,
            format!(
                "⚠️ Eliminare «{}» definitivamente? Non si può recuperare.",
                riga.testo
            ),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![button("✅ Sì, elimina", format!("remind:del:yes:{id}"))],
            vec![
                button("❌ Annulla", format!("remind:view:{id}")),
                button("🏠 Menù principale", "menu:main"),
            ],
        ]))
        .await?;
    } else if let Some(id) = id_dopo("del:yes:") {
        let avviso = if elimina(pool, utente_id, id).await {
            "✅ Promemoria eliminato."
        } else {
            "⚠️ Non sono riuscito a eliminarlo."
        };
        mostra_menu(bot, chat_id, pool, Some(avviso)).await?;
    } else {
        return Ok(false);
    }
    Ok(true)
}

/// Riattivare un promemoria la cui ora è passata mentre era sospeso: se si
/// ripete riparte dalla prossima volta, se no si chiede un quando nuovo.
async fn riattiva(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    utente_id: i64,
    id: i64,
) -> ResponseResult<()> {
    let Some(riga) = leggi(pool, utente_id, id).await else {
        return mostra_menu(bot, chat_id, pool, None).await;
    };
    let adesso = adesso_locale(pool).await;
    let quando = riga.quando().unwrap_or_default();
    if quando > adesso {
        cambia_stato(pool, utente_id, id, "attivo").await;
        return mostra_scheda(bot, chat_id, pool, id, Some("▶️ Riattivato.")).await;
    }
    match prossima_volta(riga.ripetizione(), quando, riga.giorno_scelto(), adesso) {
        Some(prossima) => {
            let _ = cambia_quando(pool, utente_id, id, prossima).await;
            let avviso = format!(
                "▶️ Riattivato: la prossima volta è {}.",
                quando_leggibile(prossima, adesso)
            );
            mostra_scheda(bot, chat_id, pool, id, Some(&avviso)).await
        }
        None => {
            aspetta(chat_id.0, Attesa::Quando { id });
            chiedi_quando(
                bot,
                chat_id,
                pool,
                &riga.testo,
                Some("L'ora di questo promemoria è passata mentre era sospeso."),
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alle(data: &str, ora: &str) -> NaiveDateTime {
        leggi_ora_db(&format!("{data} {ora}")).expect("data di prova")
    }

    /// Martedì 6 ottobre 2026, le 22:15: l'ora del collaudo di f0b373c.
    fn adesso() -> NaiveDateTime {
        alle("2026-10-06", "22:15")
    }

    #[test]
    fn si_capisce_quando_come_lo_scrive_una_persona() {
        let casi = [
            ("domani alle 9", alle("2026-10-07", "09:00")),
            ("Domani alle 9:30", alle("2026-10-07", "09:30")),
            ("domani 18.30", alle("2026-10-07", "18:30")),
            ("fra 2 ore", alle("2026-10-07", "00:15")),
            ("tra 10 minuti", alle("2026-10-06", "22:25")),
            ("fra 10min", alle("2026-10-06", "22:25")),
            ("fra mezz'ora", alle("2026-10-06", "22:45")),
            ("fra un'ora", alle("2026-10-06", "23:15")),
            ("fra 3 giorni", alle("2026-10-09", "22:15")),
            ("15/10 18:30", alle("2026-10-15", "18:30")),
            ("15/10 alle 18", alle("2026-10-15", "18:00")),
            ("15/10", alle("2026-10-15", "09:00")),
            ("15 ottobre alle 9", alle("2026-10-15", "09:00")),
            ("3 gen", alle("2027-01-03", "09:00")),
            ("venerdì alle 20", alle("2026-10-09", "20:00")),
            ("venerdi", alle("2026-10-09", "09:00")),
            ("martedì prossimo alle 8", alle("2026-10-13", "08:00")),
            // Un orario senza data: oggi se deve ancora venire, se no domani.
            ("alle 23", alle("2026-10-06", "23:00")),
            ("alle 21", alle("2026-10-07", "21:00")),
            ("7:30", alle("2026-10-07", "07:30")),
            ("domattina", alle("2026-10-07", "09:00")),
            ("dopodomani alle 12", alle("2026-10-08", "12:00")),
            ("domani a mezzogiorno", alle("2026-10-07", "12:00")),
        ];
        for (scritto, atteso) in casi {
            assert_eq!(leggi_quando(scritto, adesso()), Some(atteso), "«{scritto}»");
        }
    }

    #[test]
    fn stasera_e_sempre_di_sera() {
        let mattina = alle("2026-10-06", "10:00");
        assert_eq!(
            leggi_quando("stasera", mattina),
            Some(alle("2026-10-06", "20:00"))
        );
        assert_eq!(
            leggi_quando("stasera alle 9", mattina),
            Some(alle("2026-10-06", "21:00"))
        );
    }

    /// Quello che non si capisce non si indovina: si chiede di nuovo.
    #[test]
    fn quello_che_non_si_capisce_resta_non_capito() {
        for scritto in ["boh", "fra poco", "domani alle", "32/10", "alle 25", ""] {
            assert_eq!(leggi_quando(scritto, adesso()), None, "«{scritto}»");
        }
    }

    /// Un momento già passato si legge lo stesso: è chi chiama a dire "è
    /// già passato", invece di spostarlo da solo all'anno prossimo.
    #[test]
    fn una_data_passata_si_legge_e_si_riconosce() {
        let ieri = leggi_quando("ieri", adesso());
        assert_eq!(ieri, None, "\"ieri\" non è un quando per un promemoria");
        let oggi_prima = leggi_quando("oggi alle 8", adesso()).expect("letta");
        assert!(oggi_prima < adesso());
    }

    #[test]
    fn le_ripetizioni_saltano_le_volte_perse() {
        let attuale = alle("2026-10-01", "09:00");
        assert_eq!(
            prossima_volta(Ripetizione::Giorno, attuale, None, adesso()),
            Some(alle("2026-10-07", "09:00"))
        );
        assert_eq!(
            prossima_volta(Ripetizione::Mai, attuale, None, adesso()),
            None
        );
        // Giovedì 1 ottobre: la settimana dopo l'ultima persa.
        assert_eq!(
            prossima_volta(Ripetizione::Settimana, attuale, None, adesso()),
            Some(alle("2026-10-08", "09:00"))
        );
        // Venerdì 9 alle 9, poi lunedì 12.
        let venerdi = alle("2026-10-09", "09:00");
        assert_eq!(
            prossima_volta(Ripetizione::Feriali, venerdi, None, venerdi),
            Some(alle("2026-10-12", "09:00"))
        );
    }

    #[test]
    fn ogni_mese_il_31_torna_il_31() {
        let gennaio = alle("2027-01-31", "09:00");
        let febbraio = prossima_volta(Ripetizione::Mese, gennaio, Some(31), gennaio).unwrap();
        assert_eq!(febbraio, alle("2027-02-28", "09:00"));
        let marzo = prossima_volta(Ripetizione::Mese, febbraio, Some(31), febbraio).unwrap();
        assert_eq!(marzo, alle("2027-03-31", "09:00"));
        let bisestile = alle("2028-02-29", "09:00");
        assert_eq!(
            prossima_volta(Ripetizione::Anno, bisestile, Some(29), bisestile),
            Some(alle("2029-02-28", "09:00"))
        );
    }

    #[test]
    fn dal_lunedi_al_venerdi_fissato_di_sabato_parte_lunedi() {
        let sabato = alle("2026-10-10", "08:00");
        assert_eq!(
            allinea(Ripetizione::Feriali, sabato),
            alle("2026-10-12", "08:00")
        );
        assert_eq!(allinea(Ripetizione::Giorno, sabato), sabato);
    }

    #[test]
    fn le_ripetizioni_si_dicono_rispetto_alla_prima_volta() {
        let quando = alle("2026-10-06", "09:00");
        assert_eq!(
            Ripetizione::Settimana.etichetta(quando, None),
            "Ogni martedì"
        );
        assert_eq!(
            Ripetizione::Mese.etichetta(quando, None),
            "Ogni mese, il giorno 6"
        );
        assert_eq!(
            Ripetizione::Anno.etichetta(quando, None),
            "Ogni anno, il 6 ottobre"
        );
    }

    #[test]
    fn quando_si_dice_corto() {
        assert_eq!(
            quando_leggibile(alle("2026-10-06", "23:00"), adesso()),
            "oggi alle 23:00"
        );
        assert_eq!(
            quando_leggibile(alle("2026-10-07", "09:00"), adesso()),
            "domani alle 09:00"
        );
        assert_eq!(
            quando_leggibile(alle("2026-10-15", "18:30"), adesso()),
            "Gio 15 Ott alle 18:30"
        );
        assert_eq!(fra_quanto(30), "fra 30 minuti");
        assert_eq!(fra_quanto(75), "fra 1 ora e 15 minuti");
        assert_eq!(fra_quanto(0), "adesso");
        assert_eq!(anticipo_leggibile(0), "all'ora del pasto");
        assert_eq!(anticipo_leggibile(120), "2 ore prima");
    }
}
