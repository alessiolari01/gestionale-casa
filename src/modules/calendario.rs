//! Griglia del mese e regole di calendario, in un posto solo.
//!
//! Nasce da due implementazioni parallele delle stesse regole: la congruenza di
//! Zeller e gli anni bisestili scritti a mano in `spazi_membri.rs` per il
//! calendario delle scadenze degli inviti, e l'aritmetica basata su `chrono`
//! introdotta nel planner. Due copie delle stesse regole in due moduli sono due
//! occasioni di divergere.
//!
//! Qui stanno le primitive sulle date e la tastiera a griglia che le usa.
//! `chrono` era già nel grafo delle dipendenze tramite `teloxide-core`, con
//! `default-features = false`: dichiararlo diretto non ha aggiunto nulla al
//! binario.

use chrono::{Datelike, Days, NaiveDate, Weekday};
use teloxide::types::InlineKeyboardButton;

// ===========================================================================
// Primitive sulle date
// ===========================================================================

/// Interpreta una data ISO `AAAA-MM-GG`.
///
/// Più severa di un controllo di sola forma: `2026-02-30` ha la forma giusta ma
/// non esiste, e va rifiutata dove entra — cioè nei callback di Telegram.
pub fn parse_date(value: &str) -> Option<NaiveDate> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return None;
    }
    let year: i32 = value[..4].parse().ok()?;
    let month: u32 = value[5..7].parse().ok()?;
    let day: u32 = value[8..10].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Formatta in ISO `AAAA-MM-GG`, solo dentro l'anno a quattro cifre.
///
/// Una data che il nostro stesso parser non saprebbe rileggere non deve uscire
/// da qui: finirebbe in un callback e tornerebbe indietro rotta.
pub fn format_iso(date: NaiveDate) -> Option<String> {
    let year = date.year();
    (0..=9999)
        .contains(&year)
        .then(|| format!("{year:04}-{:02}-{:02}", date.month(), date.day()))
}

pub fn valid_date(value: &str) -> bool {
    parse_date(value).is_some()
}

/// Sposta una data di `days` giorni.
pub fn shift_date(date: &str, days: i64) -> Option<String> {
    let parsed = parse_date(date)?;
    let passo = Days::new(days.unsigned_abs());
    let shifted = if days >= 0 {
        parsed.checked_add_days(passo)?
    } else {
        parsed.checked_sub_days(passo)?
    };
    format_iso(shifted)
}

/// Lunedì della settimana che contiene la data.
pub fn week_start_for_date(date: &str) -> Option<String> {
    let parsed = parse_date(date)?;
    let indietro = u64::from(parsed.weekday().num_days_from_monday());
    format_iso(parsed.checked_sub_days(Days::new(indietro))?)
}

pub fn weekday_name(date: &str) -> &'static str {
    match parse_date(date).map(|parsed| parsed.weekday()) {
        Some(Weekday::Mon) => "Lunedì",
        Some(Weekday::Tue) => "Martedì",
        Some(Weekday::Wed) => "Mercoledì",
        Some(Weekday::Thu) => "Giovedì",
        Some(Weekday::Fri) => "Venerdì",
        Some(Weekday::Sat) => "Sabato",
        Some(Weekday::Sun) => "Domenica",
        None => "Giorno",
    }
}

pub fn weekday_short(date: &str) -> &'static str {
    match weekday_name(date) {
        "Lunedì" => "Lun",
        "Martedì" => "Mar",
        "Mercoledì" => "Mer",
        "Giovedì" => "Gio",
        "Venerdì" => "Ven",
        "Sabato" => "Sab",
        "Domenica" => "Dom",
        _ => "Giorno",
    }
}

/// `GG/MM/AAAA`, la forma con cui le date si leggono in italiano.
pub fn display_date(value: &str) -> String {
    if valid_date(value) {
        format!("{}/{}/{}", &value[8..10], &value[5..7], &value[..4])
    } else {
        value.to_string()
    }
}

/// `GG/MM`. Dentro una settimana o un mese l'anno è lo stesso su tutte le
/// righe: ripeterlo costa spazio e non distingue niente.
pub fn display_day_month(value: &str) -> String {
    if valid_date(value) {
        format!("{}/{}", &value[8..10], &value[5..7])
    } else {
        value.to_string()
    }
}

pub fn month_name(month: u32) -> &'static str {
    match month {
        1 => "Gennaio",
        2 => "Febbraio",
        3 => "Marzo",
        4 => "Aprile",
        5 => "Maggio",
        6 => "Giugno",
        7 => "Luglio",
        8 => "Agosto",
        9 => "Settembre",
        10 => "Ottobre",
        11 => "Novembre",
        12 => "Dicembre",
        _ => "Mese",
    }
}

/// Primo giorno del mese, come data ISO.
pub fn month_start(year: i32, month: u32) -> Option<String> {
    format_iso(NaiveDate::from_ymd_opt(year, month, 1)?)
}

/// Sposta di `delta` mesi restando su un primo del mese valido.
pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + month as i32 - 1 + delta;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (anno_dopo, mese_dopo) = shift_month(year, month, 1);
    let Some(inizio) = NaiveDate::from_ymd_opt(year, month, 1) else {
        return 0;
    };
    let Some(prossimo) = NaiveDate::from_ymd_opt(anno_dopo, mese_dopo, 1) else {
        return 0;
    };
    (prossimo - inizio).num_days().max(0) as u32
}

/// Posizione del giorno nella settimana, con lunedì a zero.
pub fn weekday_monday_zero(year: i32, month: u32, day: u32) -> usize {
    NaiveDate::from_ymd_opt(year, month, day)
        .map(|date| date.weekday().num_days_from_monday() as usize)
        .unwrap_or(0)
}

// ===========================================================================
// La griglia del mese
// ===========================================================================

/// Come si comporta una cella del calendario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GiornoStato {
    /// Selezionabile.
    Libero,
    /// Visibile ma non premibile, perché fuori dai limiti consentiti.
    Bloccato,
}

/// Cosa il chiamante vuole dire su un singolo giorno.
///
/// `marcatore` compare accanto al numero: serve a mostrare che quel giorno ha
/// già qualcosa dentro senza doverlo scrivere in un elenco a parte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Giorno {
    pub stato: GiornoStato,
    pub marcatore: Option<&'static str>,
}

impl Default for Giorno {
    fn default() -> Self {
        Self {
            stato: GiornoStato::Libero,
            marcatore: None,
        }
    }
}

/// Configurazione di una griglia mensile.
pub struct Calendario<'a> {
    pub year: i32,
    pub month: u32,
    /// Data di oggi in ISO: viene marcata, ed è il riferimento principale di
    /// chi guarda un calendario.
    pub oggi: &'a str,
    // I `+ Sync` non sono decorativi: senza, `&dyn Fn` non e' `Send`, e una
    // schermata che tiene la configurazione a cavallo di un `.await` produce un
    // future non-`Send` che teloxide rifiuta con un errore che parla di
    // `Injectable` e non dice mai la parola «Send».
    /// Callback per un giorno selezionabile; riceve la data ISO.
    pub callback_giorno: &'a (dyn Fn(&str) -> String + Sync),
    /// Callback per cambiare mese; riceve anno e mese.
    pub callback_mese: &'a (dyn Fn(i32, u32) -> String + Sync),
    /// Callback usato da tutte le celle inerti.
    pub callback_inerte: &'a str,
    /// Decide stato e marcatore di ogni giorno.
    pub giorno: &'a (dyn Fn(&str) -> Giorno + Sync),
    /// Mese minimo raggiungibile all'indietro, se esiste un limite.
    pub mese_minimo: Option<(i32, u32)>,
}

fn inerte(text: impl Into<String>, callback: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), callback.to_string())
}

/// Costruisce le righe della griglia: intestazione con mese e frecce, riga dei
/// giorni della settimana, poi le settimane del mese.
///
/// Non aggiunge la riga di navigazione: quella la mette il chiamante, che sa
/// dove si torna (convenzione C3).
pub fn righe(config: &Calendario<'_>) -> Vec<Vec<InlineKeyboardButton>> {
    let mut rows = Vec::new();
    let (anno_prec, mese_prec) = shift_month(config.year, config.month, -1);
    let (anno_succ, mese_succ) = shift_month(config.year, config.month, 1);

    let indietro_permesso = config
        .mese_minimo
        .map(|minimo| (anno_prec, mese_prec) >= minimo)
        .unwrap_or(true);

    rows.push(vec![
        if indietro_permesso {
            InlineKeyboardButton::callback(
                "⬅️".to_string(),
                (config.callback_mese)(anno_prec, mese_prec),
            )
        } else {
            // Una freccia che non porta da nessuna parte non si etichetta con
            // una croce: si spegne e basta, così non sembra rotta.
            inerte(" ", config.callback_inerte)
        },
        inerte(
            format!("{} {}", month_name(config.month), config.year),
            config.callback_inerte,
        ),
        InlineKeyboardButton::callback(
            "➡️".to_string(),
            (config.callback_mese)(anno_succ, mese_succ),
        ),
    ]);

    rows.push(
        ["Lun", "Mar", "Mer", "Gio", "Ven", "Sab", "Dom"]
            .into_iter()
            .map(|label| inerte(label, config.callback_inerte))
            .collect(),
    );

    let primo = weekday_monday_zero(config.year, config.month, 1);
    let totale = days_in_month(config.year, config.month);
    let mut giorno = 1_u32;
    for settimana in 0..6 {
        let mut row = Vec::new();
        for colonna in 0..7 {
            let indice = settimana * 7 + colonna;
            if indice < primo || giorno > totale {
                row.push(inerte("·", config.callback_inerte));
                continue;
            }
            let data = format!("{:04}-{:02}-{:02}", config.year, config.month, giorno);
            let descrizione = (config.giorno)(&data);
            // Oggi si racchiude fra parentesi quadre invece di usare un
            // simbolo: in una griglia di sette colonne un'emoji allarga la
            // cella, e `·` e' gia' il riempitivo dei giorni fuori dal mese
            // (convenzione C4: un simbolo, un significato).
            let numero = if data == config.oggi {
                format!("[{giorno}]")
            } else {
                giorno.to_string()
            };
            let etichetta = match descrizione.marcatore {
                Some(marcatore) => format!("{numero} {marcatore}"),
                None => numero,
            };
            row.push(match descrizione.stato {
                GiornoStato::Libero => {
                    InlineKeyboardButton::callback(etichetta, (config.callback_giorno)(&data))
                }
                GiornoStato::Bloccato => inerte(etichetta, config.callback_inerte),
            });
            giorno += 1;
        }
        rows.push(row);
        if giorno > totale {
            break;
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_base<'a>(
        oggi: &'a str,
        giorno: &'a (dyn Fn(&str) -> Giorno + Sync),
        vuoto: &'a (dyn Fn(&str) -> String + Sync),
        mese: &'a (dyn Fn(i32, u32) -> String + Sync),
    ) -> Calendario<'a> {
        Calendario {
            year: 2026,
            month: 9,
            oggi,
            callback_giorno: vuoto,
            callback_mese: mese,
            callback_inerte: "noop",
            giorno,
            mese_minimo: None,
        }
    }

    fn etichette(rows: &[Vec<InlineKeyboardButton>]) -> Vec<Vec<String>> {
        rows.iter()
            .map(|row| row.iter().map(|b| b.text.clone()).collect())
            .collect()
    }

    #[test]
    fn giorni_del_mese_seguono_i_bisestili() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2028, 2), 29);
        // Le eccezioni secolari: 2000 bisestile, 1900 no.
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn spostamento_mese_attraversa_l_anno() {
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 9, 0), (2026, 9));
        assert_eq!(shift_month(2026, 1, -13), (2024, 12));
    }

    #[test]
    fn primo_giorno_della_settimana_con_lunedi_a_zero() {
        // 31/08/2026 è un lunedì, 06/09/2026 una domenica.
        assert_eq!(weekday_monday_zero(2026, 8, 31), 0);
        assert_eq!(weekday_monday_zero(2026, 9, 6), 6);
        assert_eq!(weekday_monday_zero(2026, 9, 1), 1);
    }

    /// La griglia deve avere sempre sette colonne, altrimenti i giorni non
    /// cadono sotto la loro intestazione.
    #[test]
    fn ogni_settimana_ha_sette_colonne() {
        let giorno = |_: &str| Giorno::default();
        let vuoto = |data: &str| format!("g:{data}");
        let mese = |a: i32, m: u32| format!("m:{a}:{m}");
        let config = config_base("2026-09-01", &giorno, &vuoto, &mese);
        let rows = righe(&config);
        for row in rows.iter().skip(1) {
            assert_eq!(row.len(), 7, "riga con {} celle", row.len());
        }
        // Intestazione: freccia, mese, freccia.
        assert_eq!(rows[0].len(), 3);
    }

    #[test]
    fn oggi_viene_marcato_e_una_sola_volta() {
        let giorno = |_: &str| Giorno::default();
        let vuoto = |data: &str| format!("g:{data}");
        let mese = |a: i32, m: u32| format!("m:{a}:{m}");
        let config = config_base("2026-09-01", &giorno, &vuoto, &mese);
        let testo = etichette(&righe(&config));
        let marcati: Vec<&String> = testo
            .iter()
            .flatten()
            .filter(|label| label.starts_with('[') && label.ends_with(']'))
            .collect();
        assert_eq!(marcati.len(), 1, "marcati: {marcati:?}");
        assert_eq!(marcati[0], "[1]");
    }

    #[test]
    fn un_giorno_bloccato_non_e_premibile() {
        let giorno = |data: &str| {
            if data < "2026-09-10" {
                Giorno {
                    stato: GiornoStato::Bloccato,
                    marcatore: None,
                }
            } else {
                Giorno::default()
            }
        };
        let scelta = |data: &str| format!("g:{data}");
        let mese = |a: i32, m: u32| format!("m:{a}:{m}");
        let config = config_base("2026-09-15", &giorno, &scelta, &mese);
        let rows = righe(&config);
        let premibili: Vec<String> = rows
            .iter()
            .flatten()
            .filter_map(|b| match &b.kind {
                teloxide::types::InlineKeyboardButtonKind::CallbackData(d) if d != "noop" => {
                    Some(d.clone())
                }
                _ => None,
            })
            .collect();
        assert!(premibili.contains(&"g:2026-09-10".to_string()));
        assert!(!premibili.contains(&"g:2026-09-09".to_string()));
    }

    #[test]
    fn il_marcatore_compare_accanto_al_numero() {
        let giorno = |data: &str| Giorno {
            stato: GiornoStato::Libero,
            marcatore: if data == "2026-09-03" {
                Some("•")
            } else {
                None
            },
        };
        let scelta = |data: &str| format!("g:{data}");
        let mese = |a: i32, m: u32| format!("m:{a}:{m}");
        let config = config_base("2026-09-01", &giorno, &scelta, &mese);
        let testo: Vec<String> = etichette(&righe(&config)).into_iter().flatten().collect();
        assert!(testo.contains(&"3 •".to_string()), "{testo:?}");
    }

    #[test]
    fn oggi_e_marcatore_convivono() {
        let giorno = |_: &str| Giorno {
            stato: GiornoStato::Libero,
            marcatore: Some("•"),
        };
        let scelta = |data: &str| format!("g:{data}");
        let mese = |a: i32, m: u32| format!("m:{a}:{m}");
        let config = config_base("2026-09-01", &giorno, &scelta, &mese);
        let testo: Vec<String> = etichette(&righe(&config)).into_iter().flatten().collect();
        assert!(testo.contains(&"[1] •".to_string()), "{testo:?}");
    }

    /// Il mese minimo spegne la freccia indietro invece di lasciarla premibile
    /// verso un mese che non si può usare.
    #[test]
    fn la_freccia_indietro_si_spegne_al_limite() {
        let giorno = |_: &str| Giorno::default();
        let scelta = |data: &str| format!("g:{data}");
        let mese = |a: i32, m: u32| format!("m:{a}:{m}");
        let mut config = config_base("2026-09-01", &giorno, &scelta, &mese);
        config.mese_minimo = Some((2026, 9));
        let rows = righe(&config);
        assert_eq!(rows[0][0].text, " ");
        assert!(matches!(
            &rows[0][0].kind,
            teloxide::types::InlineKeyboardButtonKind::CallbackData(d) if d == "noop"
        ));
        // Il mese precedente resta raggiungibile se il limite lo consente.
        config.mese_minimo = Some((2026, 1));
        let rows = righe(&config);
        assert_eq!(rows[0][0].text, "⬅️");
    }

    #[test]
    fn date_iso_valide_e_non() {
        assert!(valid_date("2026-08-31"));
        for value in ["2026-02-30", "2026-13-01", "2026-00-10", "31/08/2026", ""] {
            assert!(!valid_date(value), "{value}");
        }
        assert_eq!(display_date("2026-08-31"), "31/08/2026");
        assert_eq!(display_day_month("2026-08-31"), "31/08");
        assert_eq!(shift_date("2026-12-31", 1).as_deref(), Some("2027-01-01"));
        assert_eq!(
            week_start_for_date("2026-09-06").as_deref(),
            Some("2026-08-31")
        );
        assert_eq!(weekday_short("2026-08-31"), "Lun");
        assert_eq!(month_start(2026, 9).as_deref(), Some("2026-09-01"));
    }
}
