//! Step 7.3A - Fondazioni del Planner alimentare.
//!
//! Questo modulo contiene il dominio minimo indipendente dalla UI Telegram.
//! Il planner operativo e la navigazione settimanale arrivano in 7.3B.
#![allow(dead_code)]

use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MealType {
    Breakfast,
    MorningSnack,
    Lunch,
    AfternoonSnack,
    Dinner,
    Other,
}

impl MealType {
    pub fn token(self) -> &'static str {
        match self {
            Self::Breakfast => "colazione",
            Self::MorningSnack => "spuntino_mattina",
            Self::Lunch => "pranzo",
            Self::AfternoonSnack => "spuntino_pomeriggio",
            Self::Dinner => "cena",
            Self::Other => "altro",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Breakfast => "Colazione",
            Self::MorningSnack => "Spuntino mattina",
            Self::Lunch => "Pranzo",
            Self::AfternoonSnack => "Spuntino pomeriggio",
            Self::Dinner => "Cena",
            Self::Other => "Altro",
        }
    }

    pub fn from_token(value: &str) -> Option<Self> {
        match value {
            "colazione" => Some(Self::Breakfast),
            "spuntino_mattina" => Some(Self::MorningSnack),
            "pranzo" => Some(Self::Lunch),
            "spuntino_pomeriggio" => Some(Self::AfternoonSnack),
            "cena" => Some(Self::Dinner),
            "altro" => Some(Self::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedMealState {
    Planned,
    Completed,
}

impl PlannedMealState {
    pub fn token(self) -> &'static str {
        match self {
            Self::Planned => "pianificato",
            Self::Completed => "completato",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedProfileIngredient {
    pub profile_id: i64,
    pub profile_name: String,
    pub food_name: String,
    pub unit: String,
    pub base_quantity: f64,
    pub scaled_quantity: f64,
    pub final_quantity: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerError {
    InvalidProfile,
    EmptyProfileName,
    EmptyFoodName,
    EmptyUnit,
    InvalidQuantity,
}

impl fmt::Display for PlannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidProfile => "il profilo deve avere un identificativo valido",
            Self::EmptyProfileName => "il nome del profilo non può essere vuoto",
            Self::EmptyFoodName => "il nome dell'alimento non può essere vuoto",
            Self::EmptyUnit => "l'unità di misura non può essere vuota",
            Self::InvalidQuantity => "le quantità devono essere positive e finite",
        };
        f.write_str(message)
    }
}

impl Error for PlannerError {}

/// Converte un calcolo già risolto da `porzioni` in uno snapshot immutabile
/// adatto a un pasto del planner.
///
/// `final_quantity == None` rappresenta un ingrediente escluso e resta distinto
/// da una quantità zero.
pub fn build_ingredient_snapshot(
    profile_id: i64,
    profile_name: &str,
    food_name: &str,
    unit: &str,
    base_quantity: f64,
    scaled_quantity: f64,
    final_quantity: Option<f64>,
) -> Result<PlannedProfileIngredient, PlannerError> {
    if profile_id <= 0 {
        return Err(PlannerError::InvalidProfile);
    }
    if profile_name.trim().is_empty() {
        return Err(PlannerError::EmptyProfileName);
    }
    if food_name.trim().is_empty() {
        return Err(PlannerError::EmptyFoodName);
    }
    if unit.trim().is_empty() {
        return Err(PlannerError::EmptyUnit);
    }
    if !positive_finite(base_quantity)
        || !positive_finite(scaled_quantity)
        || final_quantity.is_some_and(|value| !positive_finite(value))
    {
        return Err(PlannerError::InvalidQuantity);
    }

    Ok(PlannedProfileIngredient {
        profile_id,
        profile_name: profile_name.trim().to_string(),
        food_name: food_name.trim().to_string(),
        unit: unit.trim().to_string(),
        base_quantity,
        scaled_quantity,
        final_quantity,
    })
}

/// Indica se il planner deve proporre esplicitamente un aggiornamento.
///
/// Un pasto completato non viene mai marcato da aggiornare: è congelato.
/// Per un pasto pianificato basta che la versione/timestamp corrente della
/// ricetta sia diversa da quella salvata nello snapshot.
pub fn recipe_update_available(
    state: PlannedMealState,
    recipe_snapshot_version: Option<&str>,
    current_recipe_version: Option<&str>,
) -> bool {
    state == PlannedMealState::Planned
        && recipe_snapshot_version.is_some()
        && current_recipe_version.is_some()
        && recipe_snapshot_version != current_recipe_version
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

// ---------------------------------------------------------------------------
// Step 7.3B.1 - Aritmetica delle date del planner.
//
// Il progetto non dipende da una libreria di date: le poche operazioni servite
// sono implementate qui, in un modulo di dominio testabile, invece che dentro
// gli handler Telegram. Le date sono sempre stringhe ISO `AAAA-MM-GG`, la stessa
// forma usata da SQLite e dalle colonne `data_inizio`/`data_fine`.
// ---------------------------------------------------------------------------

/// Giorni trascorsi dal 1970-01-01 per una data del calendario gregoriano
/// (algoritmo di Howard Hinnant, valido anche per le date precedenti).
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(if month <= 2 { year - 1 } else { year });
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = (i64::from(month) + 9) % 12;
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Inversa di `days_from_civil`.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (year as i32, month, day)
}

/// Interpreta una data ISO `AAAA-MM-GG`, rifiutando i giorni inesistenti come
/// il 31 febbraio: la conversione di andata e ritorno deve coincidere.
pub fn parse_iso_date(value: &str) -> Option<(i32, u32, u32)> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year: i32 = value.get(0..4)?.parse().ok()?;
    let month: u32 = value.get(5..7)?.parse().ok()?;
    let day: u32 = value.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if civil_from_days(days_from_civil(year, month, day)) != (year, month, day) {
        return None;
    }
    Some((year, month, day))
}

/// Formatta una data ISO a partire dai suoi componenti.
pub fn format_iso_date(year: i32, month: u32, day: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

/// Sposta una data ISO di `delta` giorni, in avanti o indietro.
pub fn add_days(date: &str, delta: i64) -> Option<String> {
    let (year, month, day) = parse_iso_date(date)?;
    let (year, month, day) = civil_from_days(days_from_civil(year, month, day) + delta);
    Some(format_iso_date(year, month, day))
}

/// Differenza in giorni fra due date ISO.
pub fn days_between(from: &str, to: &str) -> Option<i64> {
    let (fy, fm, fd) = parse_iso_date(from)?;
    let (ty, tm, td) = parse_iso_date(to)?;
    Some(days_from_civil(ty, tm, td) - days_from_civil(fy, fm, fd))
}

/// Giorno della settimana con lunedì = 0.
pub fn weekday_monday_zero(date: &str) -> Option<u32> {
    let (year, month, day) = parse_iso_date(date)?;
    Some((days_from_civil(year, month, day) + 3).rem_euclid(7) as u32)
}

/// Lunedì della settimana che contiene la data indicata.
pub fn week_start(date: &str) -> Option<String> {
    let offset = i64::from(weekday_monday_zero(date)?);
    add_days(date, -offset)
}

/// Lunedì e domenica della settimana che contiene la data indicata.
pub fn week_range(date: &str) -> Option<(String, String)> {
    let start = week_start(date)?;
    let end = add_days(&start, 6)?;
    Some((start, end))
}

/// Primo e ultimo giorno del mese che contiene la data indicata.
pub fn month_range(date: &str) -> Option<(String, String)> {
    let (year, month, _) = parse_iso_date(date)?;
    let start = format_iso_date(year, month, 1);
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = add_days(&format_iso_date(next_year, next_month, 1), -1)?;
    Some((start, end))
}

/// Data leggibile per l'utente: `31/08/2026`. Se il valore non è una data
/// valida viene restituito invariato, per non nascondere un dato sporco.
pub fn format_human_date(date: &str) -> String {
    match parse_iso_date(date) {
        Some((year, month, day)) => format!("{day:02}/{month:02}/{year:04}"),
        None => date.to_string(),
    }
}

/// Abbreviazione italiana del giorno della settimana.
pub fn weekday_short(date: &str) -> Option<&'static str> {
    const GIORNI: [&str; 7] = ["Lun", "Mar", "Mer", "Gio", "Ven", "Sab", "Dom"];
    Some(GIORNI[weekday_monday_zero(date)? as usize])
}

/// Periodo leggibile: `31/08/2026 – 06/09/2026`, oppure la sola data quando il
/// periodo dura un giorno solo.
pub fn format_human_range(start: &str, end: &str) -> String {
    if start == end {
        format_human_date(start)
    } else {
        format!("{} – {}", format_human_date(start), format_human_date(end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meal_type_roundtrip() {
        let values = [
            MealType::Breakfast,
            MealType::MorningSnack,
            MealType::Lunch,
            MealType::AfternoonSnack,
            MealType::Dinner,
            MealType::Other,
        ];

        for value in values {
            assert_eq!(MealType::from_token(value.token()), Some(value));
            assert!(!value.label().is_empty());
        }
    }

    #[test]
    fn snapshot_keeps_exclusion_distinct_from_zero() {
        let snapshot = build_ingredient_snapshot(1, "Giorgia", "Pasta", "g", 100.0, 120.0, None)
            .expect("snapshot valido");

        assert_eq!(snapshot.final_quantity, None);
        assert_eq!(snapshot.scaled_quantity, 120.0);
    }

    #[test]
    fn snapshot_keeps_absolute_override() {
        let snapshot =
            build_ingredient_snapshot(1, "Giorgia", "Pasta", "g", 100.0, 120.0, Some(90.5))
                .expect("snapshot valido");

        assert_eq!(snapshot.final_quantity, Some(90.5));
    }

    #[test]
    fn snapshot_rejects_zero_and_non_finite_quantities() {
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                build_ingredient_snapshot(1, "Giorgia", "Pasta", "g", value, 100.0, Some(100.0)),
                Err(PlannerError::InvalidQuantity)
            );
        }
    }

    #[test]
    fn planned_meal_detects_recipe_change() {
        assert!(recipe_update_available(
            PlannedMealState::Planned,
            Some("2026-08-31T10:00:00Z"),
            Some("2026-08-31T11:00:00Z"),
        ));
    }

    #[test]
    fn planned_meal_does_not_request_update_when_recipe_is_unchanged() {
        assert!(!recipe_update_available(
            PlannedMealState::Planned,
            Some("2026-08-31T10:00:00Z"),
            Some("2026-08-31T10:00:00Z"),
        ));
    }

    #[test]
    fn completed_meal_stays_frozen_even_if_recipe_changes() {
        assert!(!recipe_update_available(
            PlannedMealState::Completed,
            Some("2026-08-31T10:00:00Z"),
            Some("2026-08-31T11:00:00Z"),
        ));
    }

    #[test]
    fn date_valide_e_invalide() {
        assert_eq!(parse_iso_date("2026-08-31"), Some((2026, 8, 31)));
        assert_eq!(parse_iso_date("2024-02-29"), Some((2024, 2, 29)));
        for invalida in [
            "2026-02-31",
            "2026-13-01",
            "2026-00-10",
            "2026-08-00",
            "2025-02-29",
            "31/08/2026",
            "2026-8-31",
            "",
        ] {
            assert_eq!(
                parse_iso_date(invalida),
                None,
                "doveva essere rifiutata: {invalida}"
            );
        }
    }

    #[test]
    fn spostamento_giorni_attraversa_mesi_e_anni() {
        assert_eq!(add_days("2026-08-31", 1).as_deref(), Some("2026-09-01"));
        assert_eq!(add_days("2026-01-01", -1).as_deref(), Some("2025-12-31"));
        assert_eq!(add_days("2024-02-28", 1).as_deref(), Some("2024-02-29"));
        assert_eq!(add_days("2025-02-28", 1).as_deref(), Some("2025-03-01"));
    }

    #[test]
    fn giorno_della_settimana_con_lunedi_zero() {
        assert_eq!(weekday_monday_zero("2026-08-31"), Some(0));
        assert_eq!(weekday_monday_zero("2026-09-06"), Some(6));
        assert_eq!(weekday_short("2026-08-31"), Some("Lun"));
        assert_eq!(weekday_short("2026-09-06"), Some("Dom"));
    }

    #[test]
    fn settimana_parte_sempre_da_lunedi() {
        for giorno in ["2026-08-31", "2026-09-01", "2026-09-03", "2026-09-06"] {
            assert_eq!(
                week_range(giorno),
                Some(("2026-08-31".to_string(), "2026-09-06".to_string())),
                "settimana sbagliata per {giorno}"
            );
        }
    }

    #[test]
    fn mese_copre_tutti_i_giorni_reali() {
        assert_eq!(
            month_range("2026-02-10"),
            Some(("2026-02-01".to_string(), "2026-02-28".to_string()))
        );
        assert_eq!(
            month_range("2024-02-10"),
            Some(("2024-02-01".to_string(), "2024-02-29".to_string()))
        );
        assert_eq!(
            month_range("2026-12-05"),
            Some(("2026-12-01".to_string(), "2026-12-31".to_string()))
        );
    }

    #[test]
    fn differenza_fra_date() {
        assert_eq!(days_between("2026-08-31", "2026-09-06"), Some(6));
        assert_eq!(days_between("2026-09-06", "2026-08-31"), Some(-6));
        assert_eq!(days_between("2026-08-31", "2026-08-31"), Some(0));
    }

    #[test]
    fn periodo_leggibile() {
        assert_eq!(
            format_human_range("2026-08-31", "2026-09-06"),
            "31/08/2026 – 06/09/2026"
        );
        assert_eq!(format_human_range("2026-08-31", "2026-08-31"), "31/08/2026");
        assert_eq!(format_human_date("non-una-data"), "non-una-data");
    }
}
