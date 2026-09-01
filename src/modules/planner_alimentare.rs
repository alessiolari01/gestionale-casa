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

// ===== Step 7.3B · Planner Telegram operativo =====

use anyhow::Context as _;
use chrono::{Datelike, Days, NaiveDate, Weekday};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

type PlannerBot = crate::context_bot::ContextBot;
const PLANNER_PAGE_SIZE: i64 = 5;

impl MealType {
    fn emoji(self) -> &'static str {
        match self {
            Self::Breakfast => "☕",
            Self::MorningSnack => "🍎",
            Self::Lunch => "🍝",
            Self::AfternoonSnack => "🥪",
            Self::Dinner => "🍽️",
            Self::Other => "🍴",
        }
    }
}

#[derive(Debug, Clone)]
struct PlannerDraft {
    meal_id: Option<i64>,
    date: String,
    meal_type: Option<MealType>,
    recipe_id: Option<i64>,
    selected_profiles: Vec<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct PlannerRecipeChoice {
    id: i64,
    name: String,
    servings: i64,
    updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct PlannerProfileChoice {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, FromRow)]
struct PlannerMealRow {
    id: i64,
    date: String,
    meal_type: String,
    recipe_name: String,
    state: String,
    skipped_at: Option<String>,
    recipe_snapshot_version: Option<String>,
    current_recipe_version: Option<String>,
}

impl PlannerMealRow {
    /// La ricetta e' cambiata dopo la pianificazione di questo pasto.
    ///
    /// Stessa regola del dettaglio: un pasto completato o saltato e' congelato
    /// e non va mai segnalato come aggiornabile.
    fn needs_update(&self, oggi: &str) -> bool {
        self.skipped_at.is_none()
            && !meal_is_past(&self.date, oggi)
            && recipe_update_available(
                if self.state == "completato" {
                    PlannedMealState::Completed
                } else {
                    PlannedMealState::Planned
                },
                self.recipe_snapshot_version.as_deref(),
                self.current_recipe_version.as_deref(),
            )
    }

    /// Simbolo di stato, con le stesse convenzioni del dettaglio del pasto:
    /// consumata, saltata, da aggiornare, pianificata.
    fn marker(&self, oggi: &str) -> &'static str {
        if self.state == "completato" {
            "✅"
        } else if self.skipped_at.is_some() {
            "⏭"
        } else if self.needs_update(oggi) {
            "🔄"
        } else {
            "○"
        }
    }
}

/// Un pasto e' passato quando la sua data precede oggi.
///
/// Le date sono ISO `AAAA-MM-GG`, quindi il confronto fra stringhe coincide con
/// quello cronologico. Su un pasto passato non ha senso proporre di riallineare
/// la ricetta: e' gia' stato, o non e' stato, e in entrambi i casi cambiarne le
/// quantita' riscriverebbe la storia.
fn meal_is_past(data_pasto: &str, oggi: &str) -> bool {
    data_pasto < oggi
}

#[derive(Debug, Clone, FromRow)]
struct PlannerMealDetail {
    id: i64,
    date: String,
    meal_type: String,
    recipe_id: Option<i64>,
    recipe_name: String,
    state: String,
    skipped_at: Option<String>,
    recipe_snapshot_version: Option<String>,
    current_recipe_version: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct PlannerMealDetailRow {
    id: i64,
    date: String,
    meal_type: String,
    recipe_id: Option<i64>,
    recipe_name: String,
    state: String,
    skipped_at: Option<String>,
    recipe_snapshot_version: Option<String>,
    current_recipe_version: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct PlannerIngredientSource {
    ingredient_id: i64,
    food_id: i64,
    food_name: String,
    unit: String,
    recipe_quantity: f64,
    servings: i64,
    factor: f64,
    override_kind: Option<String>,
    override_quantity: Option<f64>,
}

static PLANNER_DRAFTS: OnceLock<Mutex<HashMap<i64, PlannerDraft>>> = OnceLock::new();

fn planner_drafts() -> &'static Mutex<HashMap<i64, PlannerDraft>> {
    PLANNER_DRAFTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn planner_set_draft(chat_id: i64, draft: PlannerDraft) {
    planner_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(chat_id, draft);
}

fn planner_get_draft(chat_id: i64) -> Option<PlannerDraft> {
    planner_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&chat_id)
        .cloned()
}

fn planner_clear_draft(chat_id: i64) {
    planner_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&chat_id);
}

pub async fn handle_message(
    bot: &PlannerBot,
    msg: &Message,
    pool: &SqlitePool,
    text: &str,
) -> ResponseResult<bool> {
    match text.split_whitespace().next().unwrap_or_default() {
        "/planner" | "/planner_alimentare" => {
            planner_clear_draft(msg.chat.id.0);
            planner_show_menu(bot, msg.chat.id, pool).await?;
            Ok(true)
        }
        "/annulla" if planner_get_draft(msg.chat.id.0).is_some() => {
            let date = planner_get_draft(msg.chat.id.0).map(|draft| draft.date);
            planner_clear_draft(msg.chat.id.0);
            if let Some(date) = date {
                planner_show_day(
                    bot,
                    msg.chat.id,
                    pool,
                    &date,
                    Some("❌ Modifica annullata."),
                )
                .await?;
            } else {
                planner_show_menu(bot, msg.chat.id, pool).await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub async fn handle_callback(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    if data == "planner:noop" {
        return Ok(true);
    }
    if data == "planner:menu" {
        planner_clear_draft(chat_id.0);
        planner_show_menu(bot, chat_id, pool).await?;
        return Ok(true);
    }
    if let Some(week) = data.strip_prefix("planner:week:") {
        planner_clear_draft(chat_id.0);
        if planner_valid_date(week) {
            planner_show_week(bot, chat_id, pool, week).await?;
        } else {
            planner_invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(date) = data.strip_prefix("planner:day:") {
        planner_clear_draft(chat_id.0);
        if planner_valid_date(date) {
            planner_show_day(bot, chat_id, pool, date, None).await?;
        } else {
            planner_invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(date) = data.strip_prefix("planner:add:") {
        if !planner_valid_date(date) {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        }
        planner_set_draft(
            chat_id.0,
            PlannerDraft {
                meal_id: None,
                date: date.to_string(),
                meal_type: None,
                recipe_id: None,
                selected_profiles: Vec::new(),
            },
        );
        planner_show_type_picker(bot, chat_id, date, None).await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:edit:") {
        let Some(meal_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match planner_load_edit_draft(pool, meal_id).await {
            Ok(Some(draft)) => {
                let date = draft.date.clone();
                planner_set_draft(chat_id.0, draft);
                planner_show_type_picker(bot, chat_id, &date, Some("✏️ Modifica pasto")).await?;
            }
            Ok(None) => {
                bot.send_message(
                    chat_id,
                    "⚠️ Pasto non disponibile, già consumato o saltato.",
                )
                .reply_markup(planner_nav_markup("planner:menu"))
                .await?;
            }
            Err(error) => {
                tracing::warn!(?error, meal_id, "Apertura modifica Planner fallita");
                bot.send_message(chat_id, "⚠️ Non riesco ad aprire questo pasto.")
                    .reply_markup(planner_nav_markup("planner:menu"))
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:refresh:yes:") {
        let Some(meal_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match planner_refresh_meal(pool, meal_id).await {
            Ok(recipe_name) => {
                planner_show_meal_detail(
                    bot,
                    chat_id,
                    pool,
                    meal_id,
                    Some(&format!("🔄 Pasto aggiornato a «{recipe_name}».")),
                )
                .await?;
            }
            Err(error) => {
                tracing::warn!(?error, meal_id, "Aggiornamento pasto Planner fallito");
                planner_show_meal_detail(bot, chat_id, pool, meal_id, Some(&format!("⚠️ {error}")))
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:refresh:ask:") {
        let Some(meal_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        planner_show_refresh_confirmation(bot, chat_id, pool, meal_id).await?;
        return Ok(true);
    }
    if let Some(token) = data.strip_prefix("planner:type:") {
        let Some(meal_type) = MealType::from_token(token) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let Some(mut draft) = planner_get_draft(chat_id.0) else {
            planner_expired(bot, chat_id).await?;
            return Ok(true);
        };
        draft.meal_type = Some(meal_type);
        draft.recipe_id = None;
        draft.selected_profiles.clear();
        planner_set_draft(chat_id.0, draft);
        planner_show_recipe_picker(bot, chat_id, pool, 0).await?;
        return Ok(true);
    }
    if let Some(raw_page) = data.strip_prefix("planner:recipes:") {
        let Some(page) = raw_page.parse::<i64>().ok().filter(|value| *value >= 0) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        if planner_get_draft(chat_id.0).is_some() {
            planner_show_recipe_picker(bot, chat_id, pool, page).await?;
        } else {
            planner_expired(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:recipe:") {
        let Some(recipe_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let Some(mut draft) = planner_get_draft(chat_id.0) else {
            planner_expired(bot, chat_id).await?;
            return Ok(true);
        };
        match planner_visible_recipe(pool, recipe_id).await {
            Ok(Some(_)) => {
                draft.recipe_id = Some(recipe_id);
                draft.selected_profiles.clear();
                planner_set_draft(chat_id.0, draft);
                planner_show_profile_picker(bot, chat_id, pool, 0).await?;
            }
            Ok(None) => {
                bot.send_message(chat_id, "⚠️ Ricetta non disponibile.")
                    .reply_markup(planner_nav_markup("planner:menu"))
                    .await?;
            }
            Err(error) => {
                tracing::warn!(?error, recipe_id, "Verifica ricetta Planner fallita");
                bot.send_message(chat_id, "⚠️ Non riesco a verificare la ricetta.")
                    .reply_markup(planner_nav_markup("planner:menu"))
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_page) = data.strip_prefix("planner:profiles:") {
        let Some(page) = raw_page.parse::<i64>().ok().filter(|value| *value >= 0) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        if planner_get_draft(chat_id.0).is_some() {
            planner_show_profile_picker(bot, chat_id, pool, page).await?;
        } else {
            planner_expired(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:profile:") {
        let Some(profile_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let Some(mut draft) = planner_get_draft(chat_id.0) else {
            planner_expired(bot, chat_id).await?;
            return Ok(true);
        };
        if !planner_visible_profile_exists(pool, profile_id)
            .await
            .unwrap_or(false)
        {
            bot.send_message(chat_id, "⚠️ Profilo non disponibile.")
                .reply_markup(planner_nav_markup("planner:menu"))
                .await?;
            return Ok(true);
        }
        if let Some(index) = draft
            .selected_profiles
            .iter()
            .position(|value| *value == profile_id)
        {
            draft.selected_profiles.remove(index);
        } else {
            draft.selected_profiles.push(profile_id);
        }
        planner_set_draft(chat_id.0, draft);
        planner_show_profile_picker(bot, chat_id, pool, 0).await?;
        return Ok(true);
    }
    if data == "planner:save" {
        let Some(draft) = planner_get_draft(chat_id.0) else {
            planner_expired(bot, chat_id).await?;
            return Ok(true);
        };
        if draft.meal_type.is_none()
            || draft.recipe_id.is_none()
            || draft.selected_profiles.is_empty()
        {
            bot.send_message(
                chat_id,
                "⚠️ Scegli tipo di pasto, ricetta e almeno un profilo partecipante.",
            )
            .reply_markup(planner_nav_markup("planner:menu"))
            .await?;
            return Ok(true);
        }
        match planner_save_draft(pool, &draft).await {
            Ok(_meal_id) => {
                planner_clear_draft(chat_id.0);
                let notice = if draft.meal_id.is_some() {
                    "✅ Pasto aggiornato."
                } else {
                    "✅ Pasto aggiunto al Planner."
                };
                planner_show_day(bot, chat_id, pool, &draft.date, Some(notice)).await?;
            }
            Err(error) => {
                tracing::warn!(?error, "Salvataggio Planner fallito");
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(planner_nav_markup("planner:menu"))
                    .await?;
            }
        }
        return Ok(true);
    }
    if data == "planner:cancel" {
        let date = planner_get_draft(chat_id.0).map(|draft| draft.date);
        planner_clear_draft(chat_id.0);
        if let Some(date) = date {
            planner_show_day(bot, chat_id, pool, &date, Some("❌ Modifica annullata.")).await?;
        } else {
            planner_show_menu(bot, chat_id, pool).await?;
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:view:") {
        if let Some(meal_id) = planner_positive_i64(raw_id) {
            planner_show_meal_detail(bot, chat_id, pool, meal_id, None).await?;
        } else {
            planner_invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:delete:ask:") {
        if let Some(meal_id) = planner_positive_i64(raw_id) {
            planner_show_delete_confirmation(bot, chat_id, pool, meal_id).await?;
        } else {
            planner_invalid(bot, chat_id).await?;
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:delete:yes:") {
        let Some(meal_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match planner_delete_meal(pool, meal_id).await {
            Ok(date) => {
                planner_show_day(
                    bot,
                    chat_id,
                    pool,
                    &date,
                    Some("✅ Pasto rimosso dal Planner."),
                )
                .await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(planner_nav_markup("planner:menu"))
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:skip:") {
        let Some(meal_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match planner_skip_meal(pool, meal_id).await {
            Ok(()) => {
                planner_show_meal_detail(
                    bot,
                    chat_id,
                    pool,
                    meal_id,
                    Some("⏭ Pasto segnato come saltato."),
                )
                .await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(planner_nav_markup("planner:menu"))
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("planner:complete:") {
        let Some(meal_id) = planner_positive_i64(raw_id) else {
            planner_invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match planner_complete_meal(pool, meal_id).await {
            Ok(()) => {
                planner_show_meal_detail(
                    bot,
                    chat_id,
                    pool,
                    meal_id,
                    Some("✅ Pasto segnato come consumato e congelato."),
                )
                .await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(planner_nav_markup("planner:menu"))
                    .await?;
            }
        }
        return Ok(true);
    }

    Ok(false)
}

async fn planner_show_menu(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
) -> ResponseResult<()> {
    let week = planner_current_week_start(pool).await;
    planner_show_week(bot, chat_id, pool, &week).await
}

async fn planner_show_week(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    week_start: &str,
) -> ResponseResult<()> {
    let week_end = planner_shift_date(week_start, 6).unwrap_or_else(|| week_start.to_string());
    let previous = planner_shift_date(week_start, -7).unwrap_or_else(|| week_start.to_string());
    let next = planner_shift_date(week_start, 7).unwrap_or_else(|| week_start.to_string());

    let mut rows = Vec::new();
    let oggi = planner_today(pool).await;
    let mut settimana_da_aggiornare = false;
    let mut text = format!(
        "📅 Planner alimentare\n\nSettimana {} → {}\n\n",
        planner_display_date(week_start),
        planner_display_date(&week_end)
    );

    for offset in 0..7 {
        let date = planner_shift_date(week_start, offset).unwrap_or_else(|| week_start.to_string());
        // Una sola lettura per giorno: da qui ricaviamo conteggio, nomi e
        // pasti da aggiornare, invece di interrogare il database tre volte.
        let meals = planner_load_meals(pool, &date).await.unwrap_or_default();
        let count = meals.len();
        let weekday = planner_weekday(&date);
        let da_aggiornare = meals.iter().any(|meal| meal.needs_update(&oggi));
        if da_aggiornare {
            settimana_da_aggiornare = true;
        }
        rows.push(vec![planner_button(
            format!(
                "{weekday} {} · {count} {}{}",
                planner_display_date(&date),
                if count == 1 { "pasto" } else { "pasti" },
                if da_aggiornare { " 🔄" } else { "" }
            ),
            format!("planner:day:{date}"),
        )]);
        if !meals.is_empty() {
            let names: Vec<String> = meals
                .iter()
                .map(|meal| format!("{} {}", meal.marker(&oggi), meal.recipe_name))
                .collect();
            text.push_str(&format!(
                "{}: {}\n",
                planner_display_date(&date),
                names.join(" · ")
            ));
        }
    }

    if settimana_da_aggiornare {
        text.push_str(
            "\n🔄 In questa settimana c'è almeno una ricetta cambiata dopo la pianificazione.\n",
        );
    }

    rows.push(vec![
        planner_button(
            "⬅️ Settimana precedente",
            format!("planner:week:{previous}"),
        ),
        planner_button("Settimana successiva ➡️", format!("planner:week:{next}")),
    ]);
    rows.push(planner_global_nav("food:menu"));

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn planner_show_day(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    date: &str,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let meals = planner_load_meals(pool, date).await.unwrap_or_default();
    let oggi = planner_today(pool).await;
    let weekday = planner_weekday(date);
    let mut rows = Vec::new();
    let mut text = format!(
        "{}📅 {weekday} · {}\n\n",
        notice
            .map(|value| format!("{value}\n\n"))
            .unwrap_or_default(),
        planner_display_date(date)
    );

    if meals.is_empty() {
        text.push_str("Nessun pasto pianificato.");
    } else {
        for meal in &meals {
            let meal_type = MealType::from_token(&meal.meal_type);
            let label = meal_type
                .map(|value| format!("{} {}", value.emoji(), value.label()))
                .unwrap_or_else(|| "🍴 Pasto".to_string());
            let marker = meal.marker(&oggi);
            text.push_str(&format!("{marker} {label}: {}\n", meal.recipe_name));
            rows.push(vec![planner_button(
                format!("{marker} {label} · {}", meal.recipe_name),
                format!("planner:view:{}", meal.id),
            )]);
        }
    }

    if meals.iter().any(|meal| meal.needs_update(&oggi)) {
        text.push_str(
            "\n🔄 Per i pasti segnati, la ricetta è cambiata dopo la pianificazione.\nApri il pasto per vedere cosa fare.",
        );
    }

    rows.push(vec![planner_button(
        "➕ Aggiungi pasto",
        format!("planner:add:{date}"),
    )]);
    let week = planner_week_start_for_date(date).unwrap_or_else(|| date.to_string());
    rows.push(planner_global_nav(&format!("planner:week:{week}")));

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn planner_show_type_picker(
    bot: &PlannerBot,
    chat_id: ChatId,
    date: &str,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let values = [
        MealType::Breakfast,
        MealType::MorningSnack,
        MealType::Lunch,
        MealType::AfternoonSnack,
        MealType::Dinner,
        MealType::Other,
    ];
    let mut rows = Vec::new();
    for pair in values.chunks(2) {
        rows.push(
            pair.iter()
                .map(|meal| {
                    planner_button(
                        format!("{} {}", meal.emoji(), meal.label()),
                        format!("planner:type:{}", meal.token()),
                    )
                })
                .collect(),
        );
    }
    rows.push(planner_global_nav(&format!("planner:day:{date}")));
    bot.send_message(
        chat_id,
        format!(
            "{}➕ Pasto · {}\n\nScegli il tipo di pasto.",
            notice
                .map(|value| format!("{value}\n\n"))
                .unwrap_or_default(),
            planner_display_date(date)
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn planner_show_recipe_picker(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    requested_page: i64,
) -> ResponseResult<()> {
    let Some(draft) = planner_get_draft(chat_id.0) else {
        planner_expired(bot, chat_id).await?;
        return Ok(());
    };
    let total = planner_count_visible_recipes(pool).await.unwrap_or(0);
    let pages = planner_page_count(total);
    let page = requested_page.clamp(0, pages - 1);
    let recipes = planner_visible_recipes_page(pool, page)
        .await
        .unwrap_or_default();

    let mut rows: Vec<Vec<InlineKeyboardButton>> = recipes
        .iter()
        .map(|recipe| {
            vec![planner_button(
                format!("🍳 {} · {} porzioni", recipe.name, recipe.servings),
                format!("planner:recipe:{}", recipe.id),
            )]
        })
        .collect();
    rows.push(planner_pagination("planner:recipes", page, pages));
    rows.push(planner_global_nav(&format!("planner:add:{}", draft.date)));

    bot.send_message(
        chat_id,
        format!(
            "🍳 Scegli ricetta\n\n📅 {}\n🍴 {}\n\nTotale: {total}\nPagina {}/{}",
            planner_display_date(&draft.date),
            draft.meal_type.map(MealType::label).unwrap_or("Pasto"),
            page + 1,
            pages
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn planner_show_profile_picker(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    requested_page: i64,
) -> ResponseResult<()> {
    let Some(draft) = planner_get_draft(chat_id.0) else {
        planner_expired(bot, chat_id).await?;
        return Ok(());
    };
    let total = planner_count_visible_profiles(pool).await.unwrap_or(0);
    let pages = planner_page_count(total);
    let page = requested_page.clamp(0, pages - 1);
    let profiles = planner_visible_profiles_page(pool, page)
        .await
        .unwrap_or_default();

    let mut rows = Vec::new();
    for profile in profiles {
        let checked = draft.selected_profiles.contains(&profile.id);
        rows.push(vec![planner_button(
            format!("{} {}", if checked { "☑️" } else { "⬜" }, profile.name),
            format!("planner:profile:{}", profile.id),
        )]);
    }
    rows.push(planner_pagination("planner:profiles", page, pages));
    rows.push(vec![planner_button(
        format!(
            "✅ Salva pasto · {} profil{}",
            draft.selected_profiles.len(),
            if draft.selected_profiles.len() == 1 {
                "o"
            } else {
                "i"
            }
        ),
        "planner:save",
    )]);
    rows.push(planner_global_nav("planner:recipes:0"));

    bot.send_message(
        chat_id,
        format!(
            "👥 Profili partecipanti\n\nSeleziona uno o più profili.\n\nSelezionati: {}\nPagina {}/{}",
            draft.selected_profiles.len(),
            page + 1,
            pages
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn planner_show_meal_detail(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    meal_id: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let meal = match planner_load_meal_detail(pool, meal_id).await {
        Ok(Some(meal)) => meal,
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Pasto non disponibile.")
                .reply_markup(planner_nav_markup("planner:menu"))
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::warn!(?error, meal_id, "Dettaglio Planner non leggibile");
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a leggere il dettaglio di questo pasto.",
            )
            .reply_markup(planner_nav_markup("planner:menu"))
            .await?;
            return Ok(());
        }
    };

    let profiles = planner_meal_profiles(pool, meal_id)
        .await
        .unwrap_or_default();
    let meal_type = MealType::from_token(&meal.meal_type);
    let oggi = planner_today(pool).await;
    let changed = meal.skipped_at.is_none()
        && !meal_is_past(&meal.date, &oggi)
        && recipe_update_available(
            if meal.state == "completato" {
                PlannedMealState::Completed
            } else {
                PlannedMealState::Planned
            },
            meal.recipe_snapshot_version.as_deref(),
            meal.current_recipe_version.as_deref(),
        );

    let mut text = format!(
        "{}🍽️ Dettaglio pasto\n\n📅 {} · {}\n🍴 {} {}\n🍳 Ricetta: {}\n👥 Profili: {}\n📌 Stato: {}",
        notice
            .map(|value| format!("{value}\n\n"))
            .unwrap_or_default(),
        planner_weekday(&meal.date),
        planner_display_date(&meal.date),
        meal_type.map(MealType::emoji).unwrap_or("🍴"),
        meal_type.map(MealType::label).unwrap_or("Pasto"),
        meal.recipe_name,
        if profiles.is_empty() {
            "nessuno".to_string()
        } else {
            profiles.join(", ")
        },
        if meal.state == "completato" {
            "✅ consumata"
        } else if meal.skipped_at.is_some() {
            "⏭ saltata"
        } else {
            "📅 pianificata"
        }
    );

    if changed {
        text.push_str(
            "\n\n🔄 La ricetta è cambiata dopo la pianificazione. Il pasto è rimasto invariato.\nPuoi allinearlo alla ricetta di oggi con 🔄 Aggiorna.",
        );
    }

    let totals = planner_snapshot_totals(pool, meal_id)
        .await
        .unwrap_or_default();
    if !totals.is_empty() {
        text.push_str("\n\n📐 Quantità totali:\n");
        text.push_str(&totals.join("\n"));
    }

    let mut rows = Vec::new();
    if changed {
        rows.push(vec![planner_button(
            "🔄 Aggiorna alla ricetta attuale",
            format!("planner:refresh:ask:{}", meal.id),
        )]);
    }
    if meal.state == "pianificato" && meal.skipped_at.is_none() {
        rows.push(vec![
            planner_button("✏️ Modifica", format!("planner:edit:{}", meal.id)),
            planner_button(
                "✅ Segna come consumata",
                format!("planner:complete:{}", meal.id),
            ),
        ]);
        rows.push(vec![planner_button(
            "⏭ Segna come saltata",
            format!("planner:skip:{}", meal.id),
        )]);
        rows.push(vec![planner_button(
            "🗑️ Rimuovi pasto",
            format!("planner:delete:ask:{}", meal.id),
        )]);
    }
    rows.push(planner_global_nav(&format!("planner:day:{}", meal.date)));

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn planner_show_delete_confirmation(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    meal_id: i64,
) -> ResponseResult<()> {
    let Some(meal) = planner_load_meal_detail(pool, meal_id)
        .await
        .unwrap_or(None)
    else {
        planner_invalid(bot, chat_id).await?;
        return Ok(());
    };
    bot.send_message(
        chat_id,
        format!(
            "🗑️ Rimuovere questo pasto?\n\n🍳 {}\n📅 {}",
            meal.recipe_name,
            planner_display_date(&meal.date)
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![planner_button(
            "✅ Conferma rimozione",
            format!("planner:delete:yes:{}", meal.id),
        )],
        planner_global_nav(&format!("planner:view:{}", meal.id)),
    ]))
    .await?;
    Ok(())
}

/// Data odierna secondo il fuso del dispositivo.
/// Data usata quando SQLite non risponde: lontana nel futuro, cosi' nessun
/// pasto risulta passato e nessuna segnalazione parte a vuoto.
const PLANNER_DATA_SCONOSCIUTA: &str = "9999-12-31";

/// Settimana mostrata quando nemmeno la data di oggi e' leggibile.
///
/// E' un lunedi': meglio una settimana palesemente vuota che spostare l'utente
/// nell'anno 9999 per un errore di lettura.
const PLANNER_SETTIMANA_DI_RIPIEGO: &str = "1970-01-05";

async fn planner_today(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(pool)
        .await
        // Se la data non e' leggibile non segnaliamo nulla, invece di segnalare
        // tutto: una data futura irraggiungibile rende ogni pasto "passato".
        .unwrap_or_else(|_| PLANNER_DATA_SCONOSCIUTA.to_string())
}

/// Chiede conferma prima di riallineare il pasto alla ricetta viva.
///
/// L'aggiornamento non e' mai automatico: e' l'utente a decidere quando una
/// modifica della ricetta deve entrare in un pasto gia' pianificato.
async fn planner_show_refresh_confirmation(
    bot: &PlannerBot,
    chat_id: ChatId,
    pool: &SqlitePool,
    meal_id: i64,
) -> ResponseResult<()> {
    let Some(meal) = planner_load_meal_detail(pool, meal_id)
        .await
        .unwrap_or(None)
    else {
        planner_invalid(bot, chat_id).await?;
        return Ok(());
    };
    let profiles = planner_meal_profiles(pool, meal_id)
        .await
        .unwrap_or_default();

    bot.send_message(
        chat_id,
        format!(
            "🔄 Aggiornare questo pasto?\n\n🍳 {}\n📅 {}\n👥 {}\n\n\
             Le quantità vengono ricalcolate con la ricetta di adesso.\n\
             I partecipanti e le loro percentuali personali restano quelli che hai scelto.\n\
             Gli altri pasti non vengono toccati.",
            meal.recipe_name,
            planner_display_date(&meal.date),
            if profiles.is_empty() {
                "nessun profilo".to_string()
            } else {
                profiles.join(", ")
            }
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![planner_button(
            "✅ Sì, aggiorna",
            format!("planner:refresh:yes:{}", meal.id),
        )],
        planner_global_nav(&format!("planner:view:{}", meal.id)),
    ]))
    .await?;
    Ok(())
}

/// Riallinea il pasto alla ricetta viva riusando lo stesso percorso della
/// modifica: cosi' il ricalcolo degli snapshot resta scritto in un posto solo.
async fn planner_refresh_meal(pool: &SqlitePool, meal_id: i64) -> anyhow::Result<String> {
    let Some(meal) = planner_load_meal_detail(pool, meal_id).await? else {
        anyhow::bail!("Pasto non disponibile");
    };
    let oggi = planner_today(pool).await;
    if meal_is_past(&meal.date, &oggi) {
        anyhow::bail!("Un pasto già passato non si aggiorna");
    }
    let Some(draft) = planner_load_edit_draft(pool, meal_id).await? else {
        anyhow::bail!("Il pasto non è più aggiornabile");
    };
    if draft.recipe_id.is_none() {
        anyhow::bail!("La ricetta di questo pasto non esiste più");
    }
    planner_save_draft(pool, &draft).await?;
    Ok(meal.recipe_name)
}

// ===== Aritmetica di calendario =====
//
// Fino allo Step 7.3B ognuna di queste operazioni era una query a SQLite.
// `planner_show_week` ne eseguiva diciassette per soli conti di calendario,
// a ogni apertura della schermata: su un telefono che fa da server sono
// diciassette round-trip che non leggono alcun dato.
//
// `chrono` era gia' nel grafo delle dipendenze — lo usa `teloxide-core`, con
// `default-features = false` — quindi dichiararlo diretto non aggiunge nulla
// al binario e ci evita di riscrivere a mano l'aritmetica gregoriana.

/// Interpreta una data ISO `YYYY-MM-DD`.
///
/// Piu' severa del controllo di sola forma che sostituisce: `2026-02-30` ha la
/// forma giusta ma non esiste, e prima veniva accettata.
fn planner_parse_date(value: &str) -> Option<NaiveDate> {
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

/// Formatta una data in ISO `YYYY-MM-DD`.
///
/// Restituisce `None` fuori dall'intervallo a quattro cifre: una data che il
/// nostro stesso parser non saprebbe rileggere non deve mai uscire da qui,
/// altrimenti finirebbe in un callback Telegram e tornerebbe indietro rotta.
fn planner_format_iso(date: NaiveDate) -> Option<String> {
    let year = date.year();
    (0..=9999)
        .contains(&year)
        .then(|| format!("{year:04}-{:02}-{:02}", date.month(), date.day()))
}

/// Sposta una data di `days` giorni.
///
/// Restituisce `None` se la data non e' valida o se il risultato uscirebbe
/// dall'intervallo rappresentabile: chi chiama decide come degradare.
fn planner_shift_date(date: &str, days: i64) -> Option<String> {
    let parsed = planner_parse_date(date)?;
    let passo = Days::new(days.unsigned_abs());
    let shifted = if days >= 0 {
        parsed.checked_add_days(passo)?
    } else {
        parsed.checked_sub_days(passo)?
    };
    planner_format_iso(shifted)
}

/// Lunedi' della settimana che contiene la data.
fn planner_week_start_for_date(date: &str) -> Option<String> {
    let parsed = planner_parse_date(date)?;
    let indietro = u64::from(parsed.weekday().num_days_from_monday());
    planner_format_iso(parsed.checked_sub_days(Days::new(indietro))?)
}

/// Inizio della settimana corrente.
///
/// Resta l'unica lettura al database di questo gruppo: la data di oggi dipende
/// dal fuso orario del telefono, che solo SQLite conosce con `localtime`.
async fn planner_current_week_start(pool: &SqlitePool) -> String {
    let oggi = planner_today(pool).await;
    if oggi == PLANNER_DATA_SCONOSCIUTA {
        return PLANNER_SETTIMANA_DI_RIPIEGO.to_string();
    }
    planner_week_start_for_date(&oggi).unwrap_or(oggi)
}

fn planner_weekday(date: &str) -> &'static str {
    match planner_parse_date(date).map(|parsed| parsed.weekday()) {
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

fn planner_valid_date(value: &str) -> bool {
    planner_parse_date(value).is_some()
}

fn planner_display_date(value: &str) -> String {
    if planner_valid_date(value) {
        format!("{}/{}/{}", &value[8..10], &value[5..7], &value[..4])
    } else {
        value.to_string()
    }
}

fn planner_positive_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|number| *number > 0)
}

fn planner_page_count(total: i64) -> i64 {
    ((total.max(0) + PLANNER_PAGE_SIZE - 1) / PLANNER_PAGE_SIZE).max(1)
}

fn planner_pagination(prefix: &str, page: i64, pages: i64) -> Vec<InlineKeyboardButton> {
    let mut row = Vec::new();
    if page > 0 {
        row.push(planner_button(
            "⬅️ Pagina precedente",
            format!("{prefix}:{}", page - 1),
        ));
    }
    row.push(planner_button(
        format!("{}/{}", page + 1, pages),
        "planner:noop",
    ));
    if page + 1 < pages {
        row.push(planner_button(
            "Pagina successiva ➡️",
            format!("{prefix}:{}", page + 1),
        ));
    }
    row
}

fn planner_global_nav(back: &str) -> Vec<InlineKeyboardButton> {
    vec![
        planner_button("⬅️ Indietro", back.to_string()),
        planner_button("🏠 Menù principale", "menu:main"),
    ]
}

fn planner_nav_markup(back: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![planner_global_nav(back)])
}

fn planner_button(label: impl Into<String>, callback: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.into(), callback.into())
}

async fn planner_invalid(bot: &PlannerBot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "⚠️ Pulsante Planner non valido o non più disponibile.",
    )
    .reply_markup(planner_nav_markup("planner:menu"))
    .await?;
    Ok(())
}

async fn planner_expired(bot: &PlannerBot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "ℹ️ La modifica del pasto non è più attiva. Riapri il Planner.",
    )
    .reply_markup(planner_nav_markup("planner:menu"))
    .await?;
    Ok(())
}

async fn planner_find_for_date(pool: &SqlitePool, date: &str) -> anyhow::Result<Option<i64>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    sqlx::query_scalar(
        "SELECT p.id FROM planner_alimentari p \
         JOIN membri_spazio ms ON ms.spazio_id = p.spazio_id AND ms.utente_id = ? \
         WHERE p.spazio_id = ? AND p.archiviato = 0 \
           AND date(?) BETWEEN date(p.data_inizio) AND date(p.data_fine) \
         ORDER BY p.id LIMIT 1",
    )
    .bind(user_id)
    .bind(actor.spazio_id)
    .bind(date)
    .fetch_optional(pool)
    .await
    .context("Impossibile cercare il Planner")
}

async fn planner_load_meals(pool: &SqlitePool, date: &str) -> anyhow::Result<Vec<PlannerMealRow>> {
    let Some(planner_id) = planner_find_for_date(pool, date).await? else {
        return Ok(Vec::new());
    };
    sqlx::query_as(
        "SELECT pp.id, pp.data_pasto AS date, pp.tipo_pasto AS meal_type, \
                pp.ricetta_nome_snapshot AS recipe_name, pp.stato AS state, \
                pp.saltato_il AS skipped_at, \
                pp.ricetta_aggiornato_il_snapshot AS recipe_snapshot_version, \
                (SELECT r.aggiornato_il FROM ricette r WHERE r.id = pp.ricetta_id) \
                    AS current_recipe_version \
         FROM planner_pasti pp WHERE pp.planner_id = ? AND pp.data_pasto = ? \
         ORDER BY CASE pp.tipo_pasto \
           WHEN 'colazione' THEN 1 WHEN 'spuntino_mattina' THEN 2 \
           WHEN 'pranzo' THEN 3 WHEN 'spuntino_pomeriggio' THEN 4 \
           WHEN 'cena' THEN 5 ELSE 6 END, pp.ordinamento, pp.id",
    )
    .bind(planner_id)
    .bind(date)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i pasti")
}

async fn planner_ensure_week_conn(
    conn: &mut SqliteConnection,
    week_start: &str,
) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let week_end =
        planner_shift_date(week_start, 6).context("Impossibile calcolare fine settimana")?;

    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM planner_alimentari \
         WHERE spazio_id = ? AND archiviato = 0 AND data_inizio = ? AND data_fine = ? \
         ORDER BY id LIMIT 1",
    )
    .bind(actor.spazio_id)
    .bind(week_start)
    .bind(&week_end)
    .fetch_optional(&mut *conn)
    .await
    .context("Impossibile leggere il Planner")?
    {
        return Ok(id);
    }

    let name = format!("Settimana {}", planner_display_date(week_start));
    let id = sqlx::query(
        "INSERT INTO planner_alimentari \
         (proprietario_utente_id, spazio_id, nome, nome_normalizzato, data_inizio, data_fine) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(actor.spazio_id)
    .bind(&name)
    .bind(name.to_lowercase())
    .bind(week_start)
    .bind(&week_end)
    .execute(&mut *conn)
    .await
    .context("Impossibile creare il Planner della settimana")?
    .last_insert_rowid();
    Ok(id)
}

async fn planner_count_visible_recipes(pool: &SqlitePool) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    if actor.view_all {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM ricette r WHERE r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS (\
               SELECT 1 FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
               WHERE rs.ricetta_id = r.id AND ms.utente_id = ?))",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .context("Impossibile contare le ricette")
    } else {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM ricette r WHERE r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS (\
               SELECT 1 FROM ricetta_spazi rs WHERE rs.ricetta_id = r.id AND rs.spazio_id = ?))",
        )
        .bind(user_id)
        .bind(actor.spazio_id)
        .fetch_one(pool)
        .await
        .context("Impossibile contare le ricette")
    }
}

async fn planner_visible_recipes_page(
    pool: &SqlitePool,
    page: i64,
) -> anyhow::Result<Vec<PlannerRecipeChoice>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let offset = page.max(0) * PLANNER_PAGE_SIZE;
    if actor.view_all {
        sqlx::query_as(
            "SELECT r.id, r.nome AS name, r.porzioni_base AS servings, r.aggiornato_il AS updated_at \
             FROM ricette r WHERE r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS (\
               SELECT 1 FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
               WHERE rs.ricetta_id = r.id AND ms.utente_id = ?)) \
             ORDER BY r.nome COLLATE NOCASE, r.id LIMIT ? OFFSET ?",
        )
        .bind(user_id)
        .bind(user_id)
        .bind(PLANNER_PAGE_SIZE)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere le ricette")
    } else {
        sqlx::query_as(
            "SELECT r.id, r.nome AS name, r.porzioni_base AS servings, r.aggiornato_il AS updated_at \
             FROM ricette r WHERE r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS (\
               SELECT 1 FROM ricetta_spazi rs WHERE rs.ricetta_id = r.id AND rs.spazio_id = ?)) \
             ORDER BY r.nome COLLATE NOCASE, r.id LIMIT ? OFFSET ?",
        )
        .bind(user_id)
        .bind(actor.spazio_id)
        .bind(PLANNER_PAGE_SIZE)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere le ricette")
    }
}

async fn planner_visible_recipe(
    pool: &SqlitePool,
    recipe_id: i64,
) -> anyhow::Result<Option<PlannerRecipeChoice>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    if actor.view_all {
        sqlx::query_as(
            "SELECT r.id, r.nome AS name, r.porzioni_base AS servings, r.aggiornato_il AS updated_at \
             FROM ricette r WHERE r.id = ? AND r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS (\
               SELECT 1 FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
               WHERE rs.ricetta_id = r.id AND ms.utente_id = ?))",
        )
        .bind(recipe_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Impossibile verificare la ricetta")
    } else {
        sqlx::query_as(
            "SELECT r.id, r.nome AS name, r.porzioni_base AS servings, r.aggiornato_il AS updated_at \
             FROM ricette r WHERE r.id = ? AND r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS (\
               SELECT 1 FROM ricetta_spazi rs WHERE rs.ricetta_id = r.id AND rs.spazio_id = ?))",
        )
        .bind(recipe_id)
        .bind(user_id)
        .bind(actor.spazio_id)
        .fetch_optional(pool)
        .await
        .context("Impossibile verificare la ricetta")
    }
}

async fn planner_count_visible_profiles(pool: &SqlitePool) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let view_all = if actor.view_all { 1_i64 } else { 0_i64 };
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

async fn planner_visible_profiles_page(
    pool: &SqlitePool,
    page: i64,
) -> anyhow::Result<Vec<PlannerProfileChoice>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let view_all = if actor.view_all { 1_i64 } else { 0_i64 };
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
    .bind(PLANNER_PAGE_SIZE)
    .bind(page.max(0) * PLANNER_PAGE_SIZE)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i profili")
}

async fn planner_visible_profile_exists(
    pool: &SqlitePool,
    profile_id: i64,
) -> anyhow::Result<bool> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let view_all = if actor.view_all { 1_i64 } else { 0_i64 };
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM profili_alimentari pa WHERE pa.id = ? AND pa.archiviato = 0 AND \
         (pa.gestore_utente_id = ? OR pa.utente_collegato_id = ? OR EXISTS (\
           SELECT 1 FROM profilo_alimentare_spazi pas JOIN membri_spazio ms ON ms.spazio_id = pas.spazio_id \
           WHERE pas.profilo_alimentare_id = pa.id AND ms.utente_id = ? AND (? = 1 OR pas.spazio_id = ?))))",
    )
    .bind(profile_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il profilo")
}

async fn planner_save_draft(pool: &SqlitePool, draft: &PlannerDraft) -> anyhow::Result<i64> {
    let recipe_id = draft.recipe_id.context("Scegli una ricetta")?;
    let meal_type = draft.meal_type.context("Scegli il tipo di pasto")?;
    if draft.selected_profiles.is_empty() {
        anyhow::bail!("Seleziona almeno un profilo");
    }

    let recipe = planner_visible_recipe(pool, recipe_id)
        .await?
        .context("Ricetta non disponibile")?;
    for profile_id in &draft.selected_profiles {
        if !planner_visible_profile_exists(pool, *profile_id).await? {
            anyhow::bail!("Uno dei profili selezionati non è più disponibile");
        }
    }

    let week_start =
        planner_week_start_for_date(&draft.date).context("Data del pasto non valida")?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione Planner")?;
    let planner_id = planner_ensure_week_conn(&mut tx, &week_start).await?;

    let meal_id = if let Some(meal_id) = draft.meal_id {
        planner_ensure_editable_conn(&mut tx, meal_id, planner_id).await?;
        sqlx::query("DELETE FROM planner_pasto_ingredienti_snapshot WHERE pasto_id = ?")
            .bind(meal_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile aggiornare gli snapshot")?;
        sqlx::query("DELETE FROM planner_pasto_profili WHERE pasto_id = ?")
            .bind(meal_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile aggiornare i profili")?;
        sqlx::query(
            "UPDATE planner_pasti SET tipo_pasto = ?, ricetta_id = ?, \
             ricetta_nome_snapshot = ?, ricetta_porzione_base_snapshot = ?, \
             ricetta_aggiornato_il_snapshot = ?, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
             WHERE id = ? AND stato = 'pianificato'",
        )
        .bind(meal_type.token())
        .bind(recipe.id)
        .bind(&recipe.name)
        .bind(recipe.servings)
        .bind(&recipe.updated_at)
        .bind(meal_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile aggiornare il pasto")?;
        meal_id
    } else {
        sqlx::query(
            "INSERT INTO planner_pasti \
             (planner_id, data_pasto, tipo_pasto, ricetta_id, ricetta_nome_snapshot, \
              ricetta_porzione_base_snapshot, ricetta_aggiornato_il_snapshot) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(planner_id)
        .bind(&draft.date)
        .bind(meal_type.token())
        .bind(recipe.id)
        .bind(&recipe.name)
        .bind(recipe.servings)
        .bind(&recipe.updated_at)
        .execute(&mut *tx)
        .await
        .context("Impossibile creare il pasto")?
        .last_insert_rowid()
    };

    for profile_id in &draft.selected_profiles {
        planner_snapshot_profile(&mut tx, meal_id, recipe.id, *profile_id).await?;
    }

    tx.commit()
        .await
        .context("Impossibile completare il salvataggio del pasto")?;
    Ok(meal_id)
}

async fn planner_snapshot_profile(
    conn: &mut SqliteConnection,
    meal_id: i64,
    recipe_id: i64,
    profile_id: i64,
) -> anyhow::Result<()> {
    let profile_name: String =
        sqlx::query_scalar("SELECT nome FROM profili_alimentari WHERE id = ? AND archiviato = 0")
            .bind(profile_id)
            .fetch_one(&mut *conn)
            .await
            .context("Impossibile leggere il profilo")?;

    let factor: f64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT fattore_porzione FROM profilo_ricetta_porzioni \
         WHERE profilo_alimentare_id = ? AND ricetta_id = ?), 1.0)",
    )
    .bind(profile_id)
    .bind(recipe_id)
    .fetch_one(&mut *conn)
    .await
    .context("Impossibile leggere la porzione")?;

    sqlx::query(
        "INSERT INTO planner_pasto_profili \
         (pasto_id, profilo_alimentare_id, profilo_nome_snapshot, fattore_porzione_snapshot) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(meal_id)
    .bind(profile_id)
    .bind(&profile_name)
    .bind(factor)
    .execute(&mut *conn)
    .await
    .context("Impossibile salvare il profilo")?;

    let ingredients: Vec<PlannerIngredientSource> = sqlx::query_as(
        "SELECT ri.id AS ingredient_id, ri.alimento_id AS food_id, a.nome AS food_name, \
                um.simbolo AS unit, ri.quantita AS recipe_quantity, r.porzioni_base AS servings, \
                ? AS factor, o.tipo_override AS override_kind, o.quantita_override AS override_quantity \
         FROM ricetta_ingredienti ri \
         JOIN ricette r ON r.id = ri.ricetta_id \
         JOIN alimenti a ON a.id = ri.alimento_id \
         JOIN unita_misura um ON um.id = ri.unita_misura_id \
         LEFT JOIN profilo_ricetta_ingredienti_override o \
           ON o.ricetta_ingrediente_id = ri.id AND o.profilo_alimentare_id = ? \
         WHERE ri.ricetta_id = ? ORDER BY ri.ordinamento, ri.id",
    )
    .bind(factor)
    .bind(profile_id)
    .bind(recipe_id)
    .fetch_all(&mut *conn)
    .await
    .context("Impossibile calcolare gli ingredienti")?;

    for ingredient in ingredients {
        let base = ingredient.recipe_quantity / ingredient.servings as f64;
        let scaled = base * ingredient.factor;
        let (kind, final_quantity) = match ingredient.override_kind.as_deref() {
            Some("escluso") => ("escluso", None),
            Some("quantita") => (
                "quantita",
                Some(
                    ingredient
                        .override_quantity
                        .context("Override quantità privo di valore")?,
                ),
            ),
            _ => ("nessuno", Some(scaled)),
        };
        let snapshot = build_ingredient_snapshot(
            profile_id,
            &profile_name,
            &ingredient.food_name,
            &ingredient.unit,
            base,
            scaled,
            final_quantity,
        )
        .map_err(anyhow::Error::new)?;

        sqlx::query(
            "INSERT INTO planner_pasto_ingredienti_snapshot \
             (pasto_id, profilo_alimentare_id, ricetta_ingrediente_id, alimento_id, \
              alimento_nome_snapshot, unita_simbolo_snapshot, quantita_base_snapshot, \
              quantita_scalata_snapshot, tipo_override_snapshot, quantita_finale_snapshot) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(meal_id)
        .bind(profile_id)
        .bind(ingredient.ingredient_id)
        .bind(ingredient.food_id)
        .bind(snapshot.food_name)
        .bind(snapshot.unit)
        .bind(snapshot.base_quantity)
        .bind(snapshot.scaled_quantity)
        .bind(kind)
        .bind(snapshot.final_quantity)
        .execute(&mut *conn)
        .await
        .context("Impossibile salvare lo snapshot ingrediente")?;
    }
    Ok(())
}

async fn planner_ensure_editable_conn(
    conn: &mut SqliteConnection,
    meal_id: i64,
    planner_id: i64,
) -> anyhow::Result<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM planner_pasti \
         WHERE id = ? AND planner_id = ? AND stato = 'pianificato')",
    )
    .bind(meal_id)
    .bind(planner_id)
    .fetch_one(&mut *conn)
    .await
    .context("Impossibile verificare il pasto")?;
    if !exists {
        anyhow::bail!("Il pasto non è più modificabile");
    }
    Ok(())
}

async fn planner_load_edit_draft(
    pool: &SqlitePool,
    meal_id: i64,
) -> anyhow::Result<Option<PlannerDraft>> {
    let Some(meal) = planner_load_meal_detail(pool, meal_id).await? else {
        return Ok(None);
    };
    if meal.state != "pianificato" || meal.skipped_at.is_some() {
        return Ok(None);
    }
    let selected_profiles: Vec<i64> = sqlx::query_scalar(
        "SELECT profilo_alimentare_id FROM planner_pasto_profili \
         WHERE pasto_id = ? AND profilo_alimentare_id IS NOT NULL \
         ORDER BY profilo_nome_snapshot",
    )
    .bind(meal_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i partecipanti")?;

    Ok(Some(PlannerDraft {
        meal_id: Some(meal_id),
        date: meal.date,
        meal_type: MealType::from_token(&meal.meal_type),
        recipe_id: meal.recipe_id,
        selected_profiles,
    }))
}

async fn planner_load_meal_detail(
    pool: &SqlitePool,
    meal_id: i64,
) -> anyhow::Result<Option<PlannerMealDetail>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;

    let row: Option<PlannerMealDetailRow> = sqlx::query_as(
        "SELECT pp.id, pp.data_pasto AS date, pp.tipo_pasto AS meal_type, pp.ricetta_id AS recipe_id,                 pp.ricetta_nome_snapshot AS recipe_name, pp.stato AS state, pp.saltato_il AS skipped_at,                 pp.ricetta_aggiornato_il_snapshot AS recipe_snapshot_version,                 (SELECT r.aggiornato_il FROM ricette r WHERE r.id = pp.ricetta_id) AS current_recipe_version          FROM planner_pasti pp          JOIN planner_alimentari p ON p.id = pp.planner_id          WHERE pp.id = ?            AND p.spazio_id = ?            AND p.archiviato = 0            AND EXISTS (                SELECT 1 FROM membri_spazio ms                WHERE ms.spazio_id = p.spazio_id AND ms.utente_id = ?            )",
    )
    .bind(meal_id)
    .bind(actor.spazio_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il dettaglio pasto")?;

    Ok(row.map(|row| PlannerMealDetail {
        id: row.id,
        date: row.date,
        meal_type: row.meal_type,
        recipe_id: row.recipe_id,
        recipe_name: row.recipe_name,
        state: row.state,
        skipped_at: row.skipped_at,
        recipe_snapshot_version: row.recipe_snapshot_version,
        current_recipe_version: row.current_recipe_version,
    }))
}

async fn planner_meal_profiles(pool: &SqlitePool, meal_id: i64) -> anyhow::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT profilo_nome_snapshot FROM planner_pasto_profili \
         WHERE pasto_id = ? ORDER BY profilo_nome_snapshot COLLATE NOCASE",
    )
    .bind(meal_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i profili del pasto")
}

async fn planner_snapshot_totals(pool: &SqlitePool, meal_id: i64) -> anyhow::Result<Vec<String>> {
    #[derive(FromRow)]
    struct Total {
        name: String,
        unit: String,
        quantity: f64,
    }
    let rows: Vec<Total> = sqlx::query_as(
        "SELECT alimento_nome_snapshot AS name, unita_simbolo_snapshot AS unit, \
                SUM(quantita_finale_snapshot) AS quantity \
         FROM planner_pasto_ingredienti_snapshot \
         WHERE pasto_id = ? AND quantita_finale_snapshot IS NOT NULL \
         GROUP BY alimento_nome_snapshot, unita_simbolo_snapshot \
         ORDER BY alimento_nome_snapshot COLLATE NOCASE",
    )
    .bind(meal_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le quantità del pasto")?;

    Ok(rows
        .into_iter()
        .map(|row| {
            format!(
                "• {}: {} {}",
                row.name,
                planner_format_quantity(row.quantity),
                row.unit
            )
        })
        .collect())
}

async fn planner_delete_meal(pool: &SqlitePool, meal_id: i64) -> anyhow::Result<String> {
    let meal = planner_load_meal_detail(pool, meal_id)
        .await?
        .context("Pasto non disponibile")?;
    if meal.state != "pianificato" {
        anyhow::bail!("Un pasto consumato o saltato non può essere rimosso");
    }
    let result = sqlx::query(
        "DELETE FROM planner_pasti WHERE id = ? AND stato = 'pianificato' AND saltato_il IS NULL",
    )
    .bind(meal_id)
    .execute(pool)
    .await
    .context("Impossibile rimuovere il pasto")?;
    if result.rows_affected() != 1 {
        anyhow::bail!("Pasto non più disponibile");
    }
    Ok(meal.date)
}

async fn planner_complete_meal(pool: &SqlitePool, meal_id: i64) -> anyhow::Result<()> {
    let meal = planner_load_meal_detail(pool, meal_id)
        .await?
        .context("Pasto non disponibile")?;
    if meal.state != "pianificato" || meal.skipped_at.is_some() {
        anyhow::bail!("Il pasto è già consumato o saltato");
    }
    let result = sqlx::query(
        "UPDATE planner_pasti SET stato = 'completato', \
         completato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE id = ? AND stato = 'pianificato' AND saltato_il IS NULL",
    )
    .bind(meal_id)
    .execute(pool)
    .await
    .context("Impossibile segnare il pasto come consumato")?;
    if result.rows_affected() != 1 {
        anyhow::bail!("Pasto non più disponibile");
    }
    Ok(())
}

async fn planner_skip_meal(pool: &SqlitePool, meal_id: i64) -> anyhow::Result<()> {
    let meal = planner_load_meal_detail(pool, meal_id)
        .await?
        .context("Pasto non disponibile")?;
    if meal.state != "pianificato" || meal.skipped_at.is_some() {
        anyhow::bail!("Il pasto è già consumato o saltato");
    }
    let result = sqlx::query(
        "UPDATE planner_pasti SET          saltato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now'),          aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now')          WHERE id = ? AND stato = 'pianificato' AND saltato_il IS NULL",
    )
    .bind(meal_id)
    .execute(pool)
    .await
    .context("Impossibile segnare il pasto come saltato")?;
    if result.rows_affected() != 1 {
        anyhow::bail!("Pasto non più disponibile");
    }
    Ok(())
}

fn planner_format_quantity(value: f64) -> String {
    if value.fract().abs() < 0.000_001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .replace('.', ",")
    }
}

#[cfg(test)]
mod telegram_tests {
    use super::*;

    #[test]
    fn callback_data_resta_sotto_limite_telegram() {
        for value in [
            "planner:day:2026-08-31",
            "planner:week:2026-08-31",
            "planner:delete:ask:999999999",
            "planner:profile:999999999",
        ] {
            assert!(value.len() <= 64);
        }
    }

    #[test]
    fn data_iso_e_formattazione_italiana() {
        assert!(planner_valid_date("2026-08-31"));
        assert!(!planner_valid_date("31/08/2026"));
        assert_eq!(planner_display_date("2026-08-31"), "31/08/2026");
    }

    /// La validazione ora e' semantica, non piu' di sola forma: prima queste
    /// stringhe passavano e arrivavano fino a SQLite.
    #[test]
    fn data_con_forma_giusta_ma_inesistente_viene_rifiutata() {
        for value in [
            "2026-02-30", // febbraio non ha 30 giorni
            "2026-13-01", // mese inesistente
            "2026-00-10", // mese zero
            "2026-04-31", // aprile ha 30 giorni
            "2026-08-00", // giorno zero
        ] {
            assert!(
                !planner_valid_date(value),
                "{value} non dovrebbe essere valida"
            );
        }
        assert!(!planner_valid_date("2026-8-31"));
        assert!(!planner_valid_date("2026-08-311"));
        assert!(!planner_valid_date(""));
    }

    #[test]
    fn spostamento_data_attraversa_mese_e_anno() {
        assert_eq!(
            planner_shift_date("2026-08-31", 1).as_deref(),
            Some("2026-09-01")
        );
        assert_eq!(
            planner_shift_date("2026-09-01", -1).as_deref(),
            Some("2026-08-31")
        );
        assert_eq!(
            planner_shift_date("2026-12-31", 1).as_deref(),
            Some("2027-01-01")
        );
        assert_eq!(
            planner_shift_date("2026-01-01", -1).as_deref(),
            Some("2025-12-31")
        );
        assert_eq!(
            planner_shift_date("2026-08-31", 0).as_deref(),
            Some("2026-08-31")
        );
    }

    /// Le regole dei bisestili, comprese le eccezioni secolari: e' il caso in
    /// cui un'aritmetica scritta a mano sbaglia piu' facilmente.
    #[test]
    fn spostamento_data_rispetta_gli_anni_bisestili() {
        assert_eq!(
            planner_shift_date("2028-02-28", 1).as_deref(),
            Some("2028-02-29")
        );
        assert_eq!(
            planner_shift_date("2026-02-28", 1).as_deref(),
            Some("2026-03-01")
        );
        // 2000 e' bisestile (divisibile per 400), 1900 no (divisibile per 100).
        assert_eq!(
            planner_shift_date("2000-02-28", 1).as_deref(),
            Some("2000-02-29")
        );
        assert_eq!(
            planner_shift_date("1900-02-28", 1).as_deref(),
            Some("1900-03-01")
        );
    }

    #[test]
    fn spostamento_data_e_reversibile_su_tutta_la_settimana() {
        let mut data = "2026-01-01".to_string();
        for _ in 0..400 {
            let avanti = planner_shift_date(&data, 7).expect("data valida");
            let indietro = planner_shift_date(&avanti, -7).expect("data valida");
            assert_eq!(indietro, data);
            data = avanti;
        }
    }

    #[test]
    fn spostamento_data_rifiuta_input_non_validi() {
        assert_eq!(planner_shift_date("2026-02-30", 1), None);
        assert_eq!(planner_shift_date("oggi", 1), None);
        // Oltre l'anno a quattro cifre non produciamo una data che il nostro
        // stesso parser rifiuterebbe.
        assert_eq!(planner_shift_date("9999-12-31", 1), None);
        assert_eq!(planner_shift_date("0000-01-01", -1), None);
    }

    #[test]
    fn la_settimana_comincia_di_lunedi() {
        // 31/08/2026 e' un lunedi': tutti i giorni fino a domenica devono
        // ricadere sulla stessa settimana.
        for (data, atteso) in [
            ("2026-08-31", "2026-08-31"),
            ("2026-09-01", "2026-08-31"),
            ("2026-09-04", "2026-08-31"),
            ("2026-09-06", "2026-08-31"),
            ("2026-09-07", "2026-09-07"),
        ] {
            assert_eq!(planner_week_start_for_date(data).as_deref(), Some(atteso));
        }
        assert_eq!(planner_week_start_for_date("2026-02-30"), None);
    }

    /// Il ripiego deve essere un lunedi' reale, altrimenti la schermata
    /// settimana partirebbe da meta' settimana.
    #[test]
    fn la_settimana_di_ripiego_e_un_lunedi() {
        assert_eq!(planner_weekday(PLANNER_SETTIMANA_DI_RIPIEGO), "Lunedì");
        assert!(planner_valid_date(PLANNER_DATA_SCONOSCIUTA));
        assert_eq!(
            planner_week_start_for_date(PLANNER_SETTIMANA_DI_RIPIEGO).as_deref(),
            Some(PLANNER_SETTIMANA_DI_RIPIEGO)
        );
    }

    #[test]
    fn settimana_e_giorno_restano_coerenti() {
        let inizio = planner_week_start_for_date("2026-09-03").expect("settimana valida");
        let fine = planner_shift_date(&inizio, 6).expect("data valida");
        assert_eq!(planner_weekday(&inizio), "Lunedì");
        assert_eq!(planner_weekday(&fine), "Domenica");
    }

    #[test]
    fn nomi_dei_giorni_in_italiano() {
        let attesi = [
            ("2026-08-31", "Lunedì"),
            ("2026-09-01", "Martedì"),
            ("2026-09-02", "Mercoledì"),
            ("2026-09-03", "Giovedì"),
            ("2026-09-04", "Venerdì"),
            ("2026-09-05", "Sabato"),
            ("2026-09-06", "Domenica"),
        ];
        for (data, atteso) in attesi {
            assert_eq!(planner_weekday(data), atteso);
        }
        // Una data illeggibile non deve rompere la schermata.
        assert_eq!(planner_weekday("2026-02-30"), "Giorno");
    }

    #[test]
    fn paginazione_massimo_cinque() {
        assert_eq!(PLANNER_PAGE_SIZE, 5);
        assert_eq!(planner_page_count(0), 1);
        assert_eq!(planner_page_count(6), 2);
    }

    #[test]
    fn quantita_usa_virgola_italiana() {
        assert_eq!(planner_format_quantity(90.5), "90,5");
        assert_eq!(planner_format_quantity(100.0), "100");
    }

    const OGGI: &str = "2026-09-01";
    const PRIMA: &str = "2026-08-31T10:00:00Z";
    const DOPO: &str = "2026-08-31T11:00:00Z";

    fn riga_pasto(
        date: &str,
        state: &str,
        skipped: Option<&str>,
        snapshot: Option<&str>,
        corrente: Option<&str>,
    ) -> PlannerMealRow {
        PlannerMealRow {
            id: 1,
            date: date.to_string(),
            meal_type: "pranzo".to_string(),
            recipe_name: "Pasta al pesto".to_string(),
            state: state.to_string(),
            skipped_at: skipped.map(str::to_string),
            recipe_snapshot_version: snapshot.map(str::to_string),
            current_recipe_version: corrente.map(str::to_string),
        }
    }

    /// Pasto di oggi, pianificato, con la ricetta cambiata dopo: e' il caso in
    /// cui l'aggiornamento ha senso.
    fn pasto_aggiornabile(date: &str) -> PlannerMealRow {
        riga_pasto(date, "pianificato", None, Some(PRIMA), Some(DOPO))
    }

    #[test]
    fn confronto_fra_date_iso_segue_il_calendario() {
        assert!(meal_is_past("2026-08-31", OGGI));
        assert!(!meal_is_past(OGGI, OGGI));
        assert!(!meal_is_past("2026-09-02", OGGI));
        // Cambi di mese e di anno, dove un confronto ingenuo sbaglierebbe.
        assert!(meal_is_past("2025-12-31", "2026-01-01"));
        assert!(!meal_is_past("2026-10-01", "2026-09-30"));
    }

    #[test]
    fn pasto_di_oggi_con_ricetta_cambiata_e_da_aggiornare() {
        let pasto = pasto_aggiornabile(OGGI);
        assert!(pasto.needs_update(OGGI));
        assert_eq!(pasto.marker(OGGI), "🔄");
    }

    #[test]
    fn pasto_futuro_con_ricetta_cambiata_e_da_aggiornare() {
        let pasto = pasto_aggiornabile("2026-09-05");
        assert!(pasto.needs_update(OGGI));
        assert_eq!(pasto.marker(OGGI), "🔄");
    }

    #[test]
    fn pasto_passato_non_viene_segnalato_anche_se_la_ricetta_e_cambiata() {
        // Riscrivere le quantita' di un pasto gia' passato significherebbe
        // riscrivere la storia: resta neutro.
        let pasto = pasto_aggiornabile("2026-08-30");
        assert!(!pasto.needs_update(OGGI));
        assert_eq!(pasto.marker(OGGI), "○");
    }

    #[test]
    fn pasto_con_ricetta_invariata_resta_neutro() {
        let pasto = riga_pasto(OGGI, "pianificato", None, Some(PRIMA), Some(PRIMA));
        assert!(!pasto.needs_update(OGGI));
        assert_eq!(pasto.marker(OGGI), "○");
    }

    #[test]
    fn pasto_completato_resta_congelato_anche_se_la_ricetta_cambia() {
        let pasto = riga_pasto(OGGI, "completato", None, Some(PRIMA), Some(DOPO));
        assert!(!pasto.needs_update(OGGI));
        assert_eq!(pasto.marker(OGGI), "✅");
    }

    #[test]
    fn pasto_saltato_non_viene_mai_segnalato_da_aggiornare() {
        let pasto = riga_pasto(OGGI, "pianificato", Some(DOPO), Some(PRIMA), Some(DOPO));
        assert!(!pasto.needs_update(OGGI));
        assert_eq!(pasto.marker(OGGI), "⏭");
    }

    #[test]
    fn ricetta_eliminata_non_produce_falsi_aggiornamenti() {
        // Senza ricetta viva non c'e' versione corrente da confrontare: il pasto
        // sopravvive con il solo snapshot e non deve lampeggiare.
        let pasto = riga_pasto(OGGI, "pianificato", None, Some(PRIMA), None);
        assert!(!pasto.needs_update(OGGI));
        assert_eq!(pasto.marker(OGGI), "○");
    }

    #[test]
    fn callback_di_aggiornamento_restano_sotto_il_limite_telegram() {
        for callback in [
            format!("planner:refresh:ask:{}", i64::MAX),
            format!("planner:refresh:yes:{}", i64::MAX),
        ] {
            assert!(callback.len() <= 64, "callback troppo lunga: {callback}");
        }
    }

    #[test]
    fn navigazione_globale_ha_indietro_e_menu_principale() {
        // `💡 Migliora` non viene aggiunto qui: lo inserisce il ContextBot
        // subito prima di `🏠 Menù principale` quando la riga ha meno di tre
        // pulsanti. La riga che l'utente vede è quindi
        // `⬅️ Indietro | 💡 Migliora | 🏠 Menù principale`, come da specifica.
        // Il test verifica ciò che questa funzione deve davvero garantire:
        // due pulsanti, con `menu:main` in ultima posizione, altrimenti
        // l'inserimento del ContextBot finirebbe nel punto sbagliato.
        let row = planner_global_nav("planner:menu");
        assert_eq!(row.len(), 2);
        assert!(matches!(
            &row[1].kind,
            teloxide::types::InlineKeyboardButtonKind::CallbackData(data) if data == "menu:main"
        ));
    }
}
