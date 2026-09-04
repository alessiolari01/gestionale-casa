//! Sotto-step 3/5 del punto 6 del ciclo di automazione
//! (`docs/previsto/automazione-ciclo-sviluppo.md`): schermata admin
//! `🛠️ Amministrazione → 🚀 Distribuzione` per configurare il default di
//! tipo/orario della manutenzione proposto a ogni deploy automatico.
//!
//! Le colonne `scelta_puntuale_*` esistono già nello schema
//! (`impostazioni_distribuzione`) ma non hanno ancora una UI: la scelta
//! puntuale per il singolo deploy arriva con il sotto-step 5, quando esiste
//! davvero un deploy che la offre.

use sqlx::SqlitePool;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

/// Minuti ammessi per il countdown: oltre i 180 non è più "a downtime
/// minimo", sotto 1 non è un countdown.
const MINUTI_MIN: i64 = 1;
const MINUTI_MAX: i64 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoManutenzione {
    Subito,
    Countdown,
    Programmato,
}

impl TipoManutenzione {
    fn as_db(self) -> &'static str {
        match self {
            Self::Subito => "subito",
            Self::Countdown => "countdown",
            Self::Programmato => "programmato",
        }
    }

    fn from_db(value: &str) -> Option<Self> {
        match value {
            "subito" => Some(Self::Subito),
            "countdown" => Some(Self::Countdown),
            "programmato" => Some(Self::Programmato),
            _ => None,
        }
    }

    pub fn etichetta(self) -> &'static str {
        match self {
            Self::Subito => "Subito",
            Self::Countdown => "Countdown standard",
            Self::Programmato => "Programma orario",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpostazioniDistribuzione {
    pub tipo_default: TipoManutenzione,
    pub minuti_countdown_default: Option<i64>,
    pub orario_programmato_default: Option<String>,
}

/// Legge il default attuale. La riga esiste sempre: la migration la crea
/// già valorizzata, e nessun percorso di codice la elimina.
pub async fn leggi(pool: &SqlitePool) -> sqlx::Result<ImpostazioniDistribuzione> {
    let row: (String, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT tipo_default, minuti_countdown_default, orario_programmato_default
           FROM impostazioni_distribuzione
          WHERE id = 1",
    )
    .fetch_one(pool)
    .await?;

    let tipo_default = TipoManutenzione::from_db(&row.0).unwrap_or(TipoManutenzione::Countdown);

    Ok(ImpostazioniDistribuzione {
        tipo_default,
        minuti_countdown_default: row.1,
        orario_programmato_default: row.2,
    })
}

pub async fn imposta_subito(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE impostazioni_distribuzione
            SET tipo_default = ?1,
                minuti_countdown_default = NULL,
                orario_programmato_default = NULL,
                aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = 1",
    )
    .bind(TipoManutenzione::Subito.as_db())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn imposta_countdown(pool: &SqlitePool, minuti: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE impostazioni_distribuzione
            SET tipo_default = ?1,
                minuti_countdown_default = ?2,
                orario_programmato_default = NULL,
                aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = 1",
    )
    .bind(TipoManutenzione::Countdown.as_db())
    .bind(minuti)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn imposta_programmato(pool: &SqlitePool, orario: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE impostazioni_distribuzione
            SET tipo_default = ?1,
                minuti_countdown_default = NULL,
                orario_programmato_default = ?2,
                aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = 1",
    )
    .bind(TipoManutenzione::Programmato.as_db())
    .bind(orario)
    .execute(pool)
    .await?;
    Ok(())
}

/// Valida i minuti scritti a mano. Pubblica e pura per essere testabile
/// senza database, stessa scelta di `porzioni.rs` e `calendario.rs`.
pub fn valida_minuti(testo: &str) -> Result<i64, &'static str> {
    let valore: i64 = testo
        .trim()
        .parse()
        .map_err(|_| "Scrivi solo un numero di minuti, es. 7.")?;
    if !(MINUTI_MIN..=MINUTI_MAX).contains(&valore) {
        return Err("I minuti devono essere tra 1 e 180.");
    }
    Ok(valore)
}

/// Valida un orario scritto a mano nel formato `HH:MM` e lo normalizza a
/// due cifre per ciascuna parte (es. "9:5" diventa "09:05").
pub fn valida_orario(testo: &str) -> Result<String, &'static str> {
    let testo = testo.trim();
    let (ore_testo, minuti_testo) = testo
        .split_once(':')
        .ok_or("Scrivi l'orario come HH:MM, es. 03:00.")?;
    let ore: u32 = ore_testo
        .trim()
        .parse()
        .map_err(|_| "Scrivi l'orario come HH:MM, es. 03:00.")?;
    let minuti: u32 = minuti_testo
        .trim()
        .parse()
        .map_err(|_| "Scrivi l'orario come HH:MM, es. 03:00.")?;
    if ore > 23 || minuti > 59 {
        return Err("L'ora deve essere tra 00 e 23, i minuti tra 00 e 59.");
    }
    Ok(format!("{ore:02}:{minuti:02}"))
}

/// Decodifica il token `HHMM` usato nei bottoni preimpostati (es. "0300")
/// nel formato `HH:MM` salvato a database. I bottoni sono generati da
/// `scelta_orario_keyboard`, quindi l'input è sempre valido: la funzione
/// resta comunque difensiva perché il valore arriva da un callback esterno.
pub fn orario_da_callback(token: &str) -> Option<String> {
    if token.len() != 4 || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ore: u32 = token[0..2].parse().ok()?;
    let minuti: u32 = token[2..4].parse().ok()?;
    if ore > 23 || minuti > 59 {
        return None;
    }
    Some(format!("{ore:02}:{minuti:02}"))
}

fn parse_hhmm(valore: &str) -> Option<(i64, i64)> {
    let (ore, minuti) = valore.split_once(':')?;
    Some((ore.parse().ok()?, minuti.parse().ok()?))
}

fn formatta_durata(minuti_totali: i64) -> String {
    let ore = minuti_totali / 60;
    let minuti = minuti_totali % 60;
    if ore > 0 {
        format!("{ore}h {minuti:02}m")
    } else {
        format!("{minuti}m")
    }
}

/// Minuti mancanti dall'ora attuale al prossimo orario `HH:MM` che
/// capita — oggi se non è ancora passato, altrimenti domani. Pura, testata
/// senza database: chi la chiama legge l'ora attuale da SQLite (vedi
/// `tempo_rimanente`), perché è l'unico posto che conosce davvero il fuso
/// orario del telefono (stessa scelta già presa per il calendario, vedi
/// `STATO.md`).
pub fn calcola_tempo_rimanente(ora_attuale: &str, orario_target: &str) -> Option<String> {
    let (h1, m1) = parse_hhmm(ora_attuale)?;
    let (h2, m2) = parse_hhmm(orario_target)?;
    let minuti_attuali = h1 * 60 + m1;
    let minuti_target = h2 * 60 + m2;
    let mut differenza = minuti_target - minuti_attuali;
    if differenza < 0 {
        differenza += 24 * 60;
    }
    Some(formatta_durata(differenza))
}

/// Tempo rimanente dall'ora attuale del telefono fino a `orario_target`.
/// `None` solo se `orario_target` non fosse nel formato atteso (non
/// dovrebbe succedere: è scritto solo da `imposta_programmato`, già
/// validato).
pub async fn tempo_rimanente(
    pool: &SqlitePool,
    orario_target: &str,
) -> sqlx::Result<Option<String>> {
    let ora_attuale: String = sqlx::query_scalar("SELECT strftime('%H:%M', 'now', 'localtime')")
        .fetch_one(pool)
        .await?;
    Ok(calcola_tempo_rimanente(&ora_attuale, orario_target))
}

pub fn testo_schermata_principale(
    impostazioni: &ImpostazioniDistribuzione,
    tempo_rimanente: Option<&str>,
) -> String {
    let dettaglio = match impostazioni.tipo_default {
        TipoManutenzione::Subito => TipoManutenzione::Subito.etichetta().to_string(),
        TipoManutenzione::Countdown => format!(
            "{}, {} minuti",
            TipoManutenzione::Countdown.etichetta(),
            impostazioni.minuti_countdown_default.unwrap_or(0)
        ),
        TipoManutenzione::Programmato => {
            let orario = impostazioni
                .orario_programmato_default
                .as_deref()
                .unwrap_or("--:--");
            match tempo_rimanente {
                Some(rimanente) => format!(
                    "{}, alle {orario} (tra {rimanente})",
                    TipoManutenzione::Programmato.etichetta()
                ),
                None => format!(
                    "{}, alle {orario}",
                    TipoManutenzione::Programmato.etichetta()
                ),
            }
        }
    };
    format!(
        "🚀 Distribuzione\n\nDefault attuale: {dettaglio}.\n\nQuesto valore viene proposto a ogni deploy automatico."
    )
}

pub fn testo_scelta_tipo() -> &'static str {
    "✏️ Cambia default\n\nScegli il tipo di manutenzione da proporre a ogni deploy."
}

pub fn testo_scelta_minuti() -> &'static str {
    "⏱️ Countdown standard\n\nScegli i minuti, oppure scrivili in chat (un numero tra 1 e 180)."
}

pub fn testo_scelta_orario() -> &'static str {
    "🕒 Programma orario\n\nScegli l'orario, oppure scrivilo in chat (formato HH:MM)."
}

pub fn schermata_principale_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✏️ Cambia default".to_string(),
            "admin:distribuzione:cambia".to_string(),
        )],
        vec![
            InlineKeyboardButton::callback(
                "⬅️ Amministrazione".to_string(),
                "admin:menu".to_string(),
            ),
            InlineKeyboardButton::callback(
                "🏠 Menù principale".to_string(),
                "menu:main".to_string(),
            ),
        ],
    ])
}

pub fn scelta_tipo_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "Subito".to_string(),
            "admin:distribuzione:tipo:subito".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "Countdown standard".to_string(),
            "admin:distribuzione:tipo:countdown".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "Programma orario".to_string(),
            "admin:distribuzione:tipo:programmato".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "⬅️ Distribuzione".to_string(),
            "admin:distribuzione".to_string(),
        )],
    ])
}

pub fn scelta_minuti_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "3 min".to_string(),
                "admin:distribuzione:minuti:3".to_string(),
            ),
            InlineKeyboardButton::callback(
                "5 min".to_string(),
                "admin:distribuzione:minuti:5".to_string(),
            ),
            InlineKeyboardButton::callback(
                "10 min".to_string(),
                "admin:distribuzione:minuti:10".to_string(),
            ),
            InlineKeyboardButton::callback(
                "15 min".to_string(),
                "admin:distribuzione:minuti:15".to_string(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "✏️ Altro valore".to_string(),
            "admin:distribuzione:minuti:altro".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "⬅️ Distribuzione".to_string(),
            "admin:distribuzione".to_string(),
        )],
    ])
}

pub fn scelta_orario_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "02:00".to_string(),
                "admin:distribuzione:orario:0200".to_string(),
            ),
            InlineKeyboardButton::callback(
                "03:00".to_string(),
                "admin:distribuzione:orario:0300".to_string(),
            ),
            InlineKeyboardButton::callback(
                "04:00".to_string(),
                "admin:distribuzione:orario:0400".to_string(),
            ),
            InlineKeyboardButton::callback(
                "05:00".to_string(),
                "admin:distribuzione:orario:0500".to_string(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "✏️ Altro orario".to_string(),
            "admin:distribuzione:orario:altro".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "⬅️ Distribuzione".to_string(),
            "admin:distribuzione".to_string(),
        )],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_minuti_accetta_intervallo() {
        assert_eq!(valida_minuti("5"), Ok(5));
        assert_eq!(valida_minuti(" 180 "), Ok(180));
        assert_eq!(valida_minuti("1"), Ok(1));
    }

    #[test]
    fn valida_minuti_rifiuta_fuori_intervallo_o_non_numero() {
        assert!(valida_minuti("0").is_err());
        assert!(valida_minuti("181").is_err());
        assert!(valida_minuti("-3").is_err());
        assert!(valida_minuti("abc").is_err());
        assert!(valida_minuti("5 minuti").is_err());
    }

    #[test]
    fn valida_orario_normalizza_le_cifre_singole() {
        assert_eq!(valida_orario("9:5"), Ok("09:05".to_string()));
        assert_eq!(valida_orario(" 23:59 "), Ok("23:59".to_string()));
        assert_eq!(valida_orario("00:00"), Ok("00:00".to_string()));
    }

    #[test]
    fn valida_orario_rifiuta_valori_fuori_range_o_malformati() {
        assert!(valida_orario("24:00").is_err());
        assert!(valida_orario("10:60").is_err());
        assert!(valida_orario("senza due punti").is_err());
        assert!(valida_orario("10-30").is_err());
    }

    #[test]
    fn orario_da_callback_decodifica_token_valido() {
        assert_eq!(orario_da_callback("0300"), Some("03:00".to_string()));
        assert_eq!(orario_da_callback("2359"), Some("23:59".to_string()));
    }

    #[test]
    fn orario_da_callback_rifiuta_token_malformato() {
        assert_eq!(orario_da_callback("2400"), None);
        assert_eq!(orario_da_callback("999"), None);
        assert_eq!(orario_da_callback("ab00"), None);
    }

    #[test]
    fn tempo_rimanente_stesso_giorno() {
        assert_eq!(
            calcola_tempo_rimanente("01:00", "03:00"),
            Some("2h 00m".to_string())
        );
        assert_eq!(
            calcola_tempo_rimanente("02:50", "03:00"),
            Some("10m".to_string())
        );
    }

    #[test]
    fn tempo_rimanente_attraversa_la_mezzanotte() {
        assert_eq!(
            calcola_tempo_rimanente("23:30", "03:00"),
            Some("3h 30m".to_string())
        );
    }

    #[test]
    fn tempo_rimanente_orario_gia_arrivato_torna_zero() {
        assert_eq!(
            calcola_tempo_rimanente("03:00", "03:00"),
            Some("0m".to_string())
        );
    }
}
