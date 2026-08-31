//! Step 7.2I.0 - Fondazioni Porzioni e override.
//!
//! Questo modulo contiene esclusivamente la logica di dominio necessaria a
//! calcolare le quantità personali a partire dai valori base di una ricetta.
//! Non contiene ancora handler Telegram, planner o lista della spesa.
//!
//! Le API sono predisposte per i sottostep successivi di 7.2I.
#![allow(dead_code)]

use std::{error::Error, fmt};

/// Personalizzazione applicata a una specifica riga ingrediente della ricetta.
///
/// `Excluded` non viene rappresentato come quantità `0`: l'assenza
/// dell'ingrediente è un'informazione di dominio distinta da una quantità.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IngredientOverride {
    None,
    Quantity(f64),
    Excluded,
}

/// Risultato delle tre fasi di calcolo:
/// quantità base per una porzione, quantità scalata per il profilo e quantità
/// finale dopo l'eventuale override.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortionCalculation {
    pub base_quantity: f64,
    pub scaled_quantity: f64,
    pub final_quantity: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortionError {
    Servings,
    RecipeQuantity,
    PortionFactor,
    OverrideQuantity,
}

impl fmt::Display for PortionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Servings => "il numero di porzioni base deve essere positivo",
            Self::RecipeQuantity => {
                "la quantità totale dell'ingrediente deve essere positiva e finita"
            }
            Self::PortionFactor => {
                "il fattore di porzione del profilo deve essere positivo e finito"
            }
            Self::OverrideQuantity => "la quantità di override deve essere positiva e finita",
        };
        f.write_str(message)
    }
}

impl Error for PortionError {}

/// Calcola la quantità standard per una singola porzione.
///
/// Esempio: 400 g su 4 porzioni => 100 g.
pub fn base_quantity(
    recipe_total_quantity: f64,
    recipe_servings: i64,
) -> Result<f64, PortionError> {
    if recipe_servings <= 0 {
        return Err(PortionError::Servings);
    }
    if !is_positive_finite(recipe_total_quantity) {
        return Err(PortionError::RecipeQuantity);
    }

    Ok(recipe_total_quantity / recipe_servings as f64)
}

/// Calcola la quantità di un ingrediente per un profilo.
///
/// Ordine delle regole:
/// 1. quantità totale della ricetta / porzioni base;
/// 2. moltiplicazione per il fattore personale;
/// 3. applicazione dell'eventuale override dell'ingrediente.
///
/// `final_quantity == None` significa che l'ingrediente è escluso per quel
/// profilo e non deve partecipare al futuro calcolo di planner/spesa.
pub fn calculate_profile_quantity(
    recipe_total_quantity: f64,
    recipe_servings: i64,
    portion_factor: f64,
    ingredient_override: IngredientOverride,
) -> Result<PortionCalculation, PortionError> {
    if !is_positive_finite(portion_factor) {
        return Err(PortionError::PortionFactor);
    }

    let base_quantity = base_quantity(recipe_total_quantity, recipe_servings)?;
    let scaled_quantity = base_quantity * portion_factor;

    let final_quantity = match ingredient_override {
        IngredientOverride::None => Some(scaled_quantity),
        IngredientOverride::Quantity(quantity) => {
            if !is_positive_finite(quantity) {
                return Err(PortionError::OverrideQuantity);
            }
            Some(quantity)
        }
        IngredientOverride::Excluded => None,
    };

    Ok(PortionCalculation {
        base_quantity,
        scaled_quantity,
        final_quantity,
    })
}

/// Contributo di un singolo profilo a una riga ingrediente.
///
/// È il mattone usato dai futuri planner e lista della spesa: ogni profilo
/// viene calcolato con le stesse regole di `calculate_profile_quantity`, poi
/// i contributi presenti vengono sommati.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfileIngredientContribution {
    pub profile_id: i64,
    pub portion_factor: f64,
    pub ingredient_override: IngredientOverride,
}

/// Risultato del calcolo multi-profilo per una singola riga ingrediente.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiProfileIngredientCalculation {
    pub total_quantity: f64,
    pub included_profiles: Vec<i64>,
    pub excluded_profiles: Vec<i64>,
}

/// Calcola la quantità complessiva necessaria per più profili.
///
/// L'override di quantità resta assoluto per il singolo profilo e quindi
/// prevale sulla sua percentuale. Un profilo che esclude l'ingrediente non
/// contribuisce al totale, ma viene mantenuto in `excluded_profiles`.
pub fn calculate_multi_profile_ingredient(
    recipe_total_quantity: f64,
    recipe_servings: i64,
    profiles: &[ProfileIngredientContribution],
) -> Result<MultiProfileIngredientCalculation, PortionError> {
    let mut total_quantity = 0.0;
    let mut included_profiles = Vec::new();
    let mut excluded_profiles = Vec::new();

    for profile in profiles {
        let calculation = calculate_profile_quantity(
            recipe_total_quantity,
            recipe_servings,
            profile.portion_factor,
            profile.ingredient_override,
        )?;

        match calculation.final_quantity {
            Some(quantity) => {
                total_quantity += quantity;
                included_profiles.push(profile.profile_id);
            }
            None => excluded_profiles.push(profile.profile_id),
        }
    }

    Ok(MultiProfileIngredientCalculation {
        total_quantity,
        included_profiles,
        excluded_profiles,
    })
}

fn is_positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-9;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "atteso {expected}, ottenuto {actual}"
        );
    }

    #[test]
    fn base_quantity_divides_recipe_total_by_servings() {
        let quantity = base_quantity(400.0, 4).expect("quantità valida");
        assert_close(quantity, 100.0);
    }

    #[test]
    fn standard_profile_keeps_single_serving_quantity() {
        let result = calculate_profile_quantity(400.0, 4, 1.0, IngredientOverride::None)
            .expect("calcolo valido");

        assert_close(result.base_quantity, 100.0);
        assert_close(result.scaled_quantity, 100.0);
        assert_close(result.final_quantity.expect("ingrediente presente"), 100.0);
    }

    #[test]
    fn larger_profile_scales_recipe_quantity() {
        let result = calculate_profile_quantity(400.0, 4, 1.2, IngredientOverride::None)
            .expect("calcolo valido");

        assert_close(result.final_quantity.expect("ingrediente presente"), 120.0);
    }

    #[test]
    fn smaller_profile_scales_recipe_quantity() {
        let result = calculate_profile_quantity(400.0, 4, 0.8, IngredientOverride::None)
            .expect("calcolo valido");

        assert_close(result.final_quantity.expect("ingrediente presente"), 80.0);
    }

    #[test]
    fn ingredient_quantity_override_wins_after_profile_scaling() {
        let result = calculate_profile_quantity(400.0, 4, 1.2, IngredientOverride::Quantity(90.0))
            .expect("calcolo valido");

        assert_close(result.scaled_quantity, 120.0);
        assert_close(result.final_quantity.expect("ingrediente presente"), 90.0);
    }

    #[test]
    fn excluded_ingredient_has_no_final_quantity() {
        let result = calculate_profile_quantity(400.0, 4, 1.2, IngredientOverride::Excluded)
            .expect("calcolo valido");

        assert_close(result.scaled_quantity, 120.0);
        assert_eq!(result.final_quantity, None);
    }

    #[test]
    fn rejects_non_positive_servings() {
        assert_eq!(base_quantity(400.0, 0), Err(PortionError::Servings));
    }

    #[test]
    fn rejects_non_positive_or_non_finite_recipe_quantity() {
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(base_quantity(value, 4), Err(PortionError::RecipeQuantity));
        }
    }

    #[test]
    fn rejects_non_positive_or_non_finite_profile_factor() {
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                calculate_profile_quantity(400.0, 4, value, IngredientOverride::None),
                Err(PortionError::PortionFactor)
            );
        }
    }

    #[test]
    fn rejects_invalid_quantity_override() {
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                calculate_profile_quantity(400.0, 4, 1.0, IngredientOverride::Quantity(value)),
                Err(PortionError::OverrideQuantity)
            );
        }
    }

    #[test]
    fn multi_profile_sums_scaled_and_absolute_overrides() {
        let profiles = [
            ProfileIngredientContribution {
                profile_id: 10,
                portion_factor: 1.2,
                ingredient_override: IngredientOverride::None,
            },
            ProfileIngredientContribution {
                profile_id: 20,
                portion_factor: 0.8,
                ingredient_override: IngredientOverride::Quantity(90.0),
            },
        ];

        let result =
            calculate_multi_profile_ingredient(400.0, 4, &profiles).expect("calcolo valido");

        assert_close(result.total_quantity, 210.0);
        assert_eq!(result.included_profiles, vec![10, 20]);
        assert!(result.excluded_profiles.is_empty());
    }

    #[test]
    fn multi_profile_tracks_exclusions_without_adding_them() {
        let profiles = [
            ProfileIngredientContribution {
                profile_id: 10,
                portion_factor: 1.5,
                ingredient_override: IngredientOverride::None,
            },
            ProfileIngredientContribution {
                profile_id: 20,
                portion_factor: 1.0,
                ingredient_override: IngredientOverride::Excluded,
            },
        ];

        let result =
            calculate_multi_profile_ingredient(400.0, 4, &profiles).expect("calcolo valido");

        assert_close(result.total_quantity, 150.0);
        assert_eq!(result.included_profiles, vec![10]);
        assert_eq!(result.excluded_profiles, vec![20]);
    }

    #[test]
    fn multi_profile_empty_selection_has_zero_total() {
        let result = calculate_multi_profile_ingredient(400.0, 4, &[]).expect("calcolo valido");

        assert_close(result.total_quantity, 0.0);
        assert!(result.included_profiles.is_empty());
        assert!(result.excluded_profiles.is_empty());
    }
}
