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
}
