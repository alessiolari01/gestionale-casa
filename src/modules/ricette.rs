//! Step 7.2F.1 - Ricette operative Telegram.
//!
//! La ricetta mantiene separati alimento generico, prodotto commerciale
//! opzionale e formato di vendita. Il formato NON viene salvato nella ricetta:
//! sarà scelto in seguito dalla Lista spesa.
//!
//! Il procedimento è strutturato in step ordinati. Ogni step può avere zero o
//! più foto/video e può essere consultato sia come procedimento completo sia
//! in modalità guidata, uno step alla volta.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};
use teloxide::{
    net::Download,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile},
};
use tokio::fs::File;

use crate::{
    identity,
    resource_permissions::{self, ResourcePermission},
};

type Bot = crate::context_bot::ContextBot;

use crate::modules::liste;

const RECIPE_LIST_PAGE_SIZE: i64 = 5;
const FOOD_SEARCH_LIMIT: i64 = 8;
const RECIPE_SEARCH_LIMIT: i64 = 20;
const MEDIA_ROOT: &str = "data/media/ricette";
const DRAFT_MEDIA_ROOT: &str = "data/media/ricette/_draft";
const RECIPE_NAME_MAX: usize = 120;
const STEP_TEXT_MAX: usize = 3500;
const INGREDIENT_SEARCH_MAX: usize = 120;
const RESOURCE_TYPE_RECIPE: &str = "ricetta";

#[derive(Clone, Default)]
pub struct RecipeSessionStore {
    inner: Arc<Mutex<HashMap<i64, RecipeConversationState>>>,
}

impl RecipeSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.with_sessions(|sessions| {
            sessions.remove(&chat_id);
        });
    }

    pub fn has_active(&self, chat_id: i64) -> bool {
        self.with_sessions(|sessions| sessions.contains_key(&chat_id))
    }

    fn get(&self, chat_id: i64) -> Option<RecipeConversationState> {
        self.with_sessions(|sessions| sessions.get(&chat_id).cloned())
    }

    fn set(&self, chat_id: i64, state: RecipeConversationState) {
        self.with_sessions(|sessions| {
            sessions.insert(chat_id, state);
        });
    }

    fn with_sessions<T>(
        &self,
        f: impl FnOnce(&mut HashMap<i64, RecipeConversationState>) -> T,
    ) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

// Il wizard conserva una bozza completa tra un update Telegram e il successivo.
// Alcuni variant sono intenzionalmente più grandi degli altri: mantenerli
// inline evita una cascata di Box/unwrap in tutto il flusso conversazionale.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
enum RecipeConversationState {
    NewName,
    NewServings {
        draft: RecipeDraft,
    },
    IngredientHub {
        draft: RecipeDraft,
    },
    IngredientSearch {
        draft: RecipeDraft,
    },
    IngredientQuantityReady {
        draft: RecipeDraft,
        food: FoodChoice,
        product: Option<ProductChoice>,
        unit: UnitChoice,
    },
    StepText {
        draft: RecipeDraft,
    },
    StepMedia {
        draft: RecipeDraft,
        step_index: usize,
    },
    StepPhoto {
        draft: RecipeDraft,
        step_index: usize,
    },
    StepVideo {
        draft: RecipeDraft,
        step_index: usize,
    },
    AfterStep {
        draft: RecipeDraft,
    },
    Visibility {
        draft: RecipeDraft,
    },
    VisibilityChoose {
        draft: RecipeDraft,
        selected: Vec<i64>,
    },
    SearchName,
    IngredientFinder {
        selected: Vec<FoodChoice>,
    },
    IngredientFinderQuery {
        selected: Vec<FoodChoice>,
        category_filter: Option<CategoryChoice>,
    },
    EditName {
        recipe_id: i64,
    },
    EditServings {
        recipe_id: i64,
    },
    EditIngredientSearch {
        recipe_id: i64,
    },
    EditIngredientQuantityReady {
        recipe_id: i64,
        food: FoodChoice,
        product: Option<ProductChoice>,
        unit: UnitChoice,
    },
    EditStepText {
        recipe_id: i64,
        step_id: Option<i64>,
    },
    EditStepPhoto {
        recipe_id: i64,
        step_id: i64,
    },
    EditStepVideo {
        recipe_id: i64,
        step_id: i64,
    },
    EditVisibilityChoose {
        recipe_id: i64,
        selected: Vec<i64>,
    },
}

#[derive(Debug, Clone, Default)]
struct RecipeDraft {
    name: String,
    servings: i64,
    ingredients: Vec<DraftIngredient>,
    steps: Vec<DraftStep>,
    visible_spaces: Vec<i64>,
}

#[derive(Debug, Clone)]
struct DraftIngredient {
    food_id: i64,
    food_name: String,
    product_id: Option<i64>,
    product_label: Option<String>,
    quantity: f64,
    unit_id: i64,
    unit_symbol: String,
}

#[derive(Debug, Clone)]
struct DraftStep {
    text: String,
    media: Vec<DraftMedia>,
}

#[derive(Debug, Clone)]
struct DraftMedia {
    kind: String,
    temp_path: String,
    caption: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
/// Una riga della lista delle ricette.
///
/// Porzioni, numero di ingredienti e numero di step non ci sono piu': da
/// quando il testo non ripete le voci (C1) nessuno li mostrava, e costavano
/// due sotto-query correlate per riga — dieci `COUNT` a ogni apertura di una
/// pagina da cinque. Si leggono aprendo la ricetta, dove c'e' anche tutto il
/// resto.
struct RecipeListRecord {
    id: i64,
    name: String,
    owner: bool,
    shared: bool,
}

#[derive(Debug, Clone, FromRow)]
struct RecipeRecord {
    name: String,
    servings: i64,
    owner_user_id: Option<i64>,
    owner_name: Option<String>,
    global_catalog: i64,
}

#[derive(Debug, Clone, FromRow)]
struct IngredientRecord {
    id: i64,
    food_name: String,
    product_label: Option<String>,
    quantity: f64,
    unit_symbol: String,
    optional: i64,
    notes: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct StepRecord {
    id: i64,
    number: i64,
    text: String,
    photo_count: i64,
    video_count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct StepMediaRecord {
    id: i64,
    kind: String,
    path: String,
    caption: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct UnitChoice {
    id: i64,
    name: String,
    symbol: String,
}

#[derive(Debug, Clone, FromRow)]
struct CategoryChoice {
    id: i64,
    name: String,
    emoji: String,
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
struct FoodChoice {
    id: i64,
    name: String,
    default_unit_id: Option<i64>,
    default_unit_name: Option<String>,
    default_unit_symbol: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct RecipeProductChoice {
    pub product_id: i64,
    pub brand: String,
    pub product_name: String,
}

type ProductChoice = RecipeProductChoice;

#[derive(Debug, Clone, FromRow)]
pub struct RecipeIngredientMatch {
    pub recipe_id: i64,
    pub recipe_name: String,
    pub matched_ingredients: i64,
    pub total_ingredients: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct RecipeFoodCompatibility {
    pub label_code: String,
    pub label_name: String,
    pub label_emoji: String,
    pub status: String,
    pub total_ingredients: i64,
    pub incompatible_ingredients: i64,
    pub ingredients_to_check: i64,
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &RecipeSessionStore,
    text_hint: Option<&str>,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;
    let command = text_hint.and_then(first_command);

    if matches!(command, Some("/ricette") | Some("/ricetta")) {
        cancel_draft_media(chat_id).await;
        sessions.clear_chat(chat_id);
        show_menu(bot, msg.chat.id, pool).await?;
        return Ok(true);
    }

    if command == Some("/ricette_ingredienti") {
        cancel_draft_media(chat_id).await;
        sessions.set(
            chat_id,
            RecipeConversationState::IngredientFinderQuery {
                selected: Vec::new(),
                category_filter: None,
            },
        );
        show_ingredient_finder(bot, msg.chat.id, &[], None).await?;
        return Ok(true);
    }

    if command == Some("/ricetta_nuova") {
        cancel_draft_media(chat_id).await;
        sessions.set(chat_id, RecipeConversationState::NewName);
        bot.send_message(
            msg.chat.id,
            "🍳 Nuova ricetta\n\nScrivi il nome della ricetta.\n\nPuoi premere ❌ Annulla in qualsiasi momento.",
        )
        .reply_markup(flow_keyboard("recipe:menu"))
        .await?;
        return Ok(true);
    }

    if command == Some("/annulla") && sessions.has_active(chat_id) {
        cancel_draft_media(chat_id).await;
        sessions.clear_chat(chat_id);
        bot.send_message(msg.chat.id, "❌ Operazione Ricette annullata.")
            .reply_markup(recipe_menu_keyboard())
            .await?;
        return Ok(true);
    }

    if command.is_some() {
        return Ok(false);
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        RecipeConversationState::NewName => {
            let Some(text) = text_hint else {
                send_text_required(bot, msg.chat.id, "Scrivi il nome della ricetta.").await?;
                return Ok(true);
            };
            let Some(name) = clean_text(text, RECIPE_NAME_MAX) else {
                bot.send_message(
                    msg.chat.id,
                    format!("⚠️ Nome non valido. Usa da 1 a {RECIPE_NAME_MAX} caratteri."),
                )
                .reply_markup(flow_keyboard("recipe:menu"))
                .await?;
                return Ok(true);
            };
            let actor = identity::current_actor();
            let Some(user_id) = actor.utente_id else {
                bot.send_message(msg.chat.id, "⚠️ Identità utente non disponibile.")
                    .await?;
                return Ok(true);
            };
            if personal_recipe_name_exists(pool, user_id, &normalize_name(&name), None)
                .await
                .unwrap_or(false)
            {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Hai già una ricetta attiva con questo nome. Scegli un nome diverso.",
                )
                .reply_markup(flow_keyboard("recipe:menu"))
                .await?;
                return Ok(true);
            }
            let draft = RecipeDraft {
                name,
                servings: 1,
                ..RecipeDraft::default()
            };
            sessions.set(chat_id, RecipeConversationState::NewServings { draft });
            bot.send_message(
                msg.chat.id,
                "👥 Porzioni base\n\nPer quante porzioni è scritta la ricetta?\nInvia un numero intero positivo, ad esempio 2 o 4.",
            )
            .reply_markup(flow_keyboard("recipe:menu"))
            .await?;
            Ok(true)
        }
        RecipeConversationState::NewServings { mut draft } => {
            let Some(text) = text_hint else {
                send_text_required(bot, msg.chat.id, "Invia il numero di porzioni.").await?;
                return Ok(true);
            };
            let Some(servings) = parse_positive_i64(text) else {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Inserisci un numero intero positivo di porzioni.",
                )
                .reply_markup(flow_keyboard("recipe:menu"))
                .await?;
                return Ok(true);
            };
            draft.servings = servings;
            sessions.set(
                chat_id,
                RecipeConversationState::IngredientHub {
                    draft: draft.clone(),
                },
            );
            show_draft_ingredients(bot, msg.chat.id, &draft).await?;
            Ok(true)
        }
        RecipeConversationState::IngredientHub { draft } => {
            sessions.set(chat_id, RecipeConversationState::IngredientHub { draft });
            Ok(true)
        }
        RecipeConversationState::IngredientSearch { draft } => {
            let Some(query) = text_hint.and_then(|value| clean_text(value, INGREDIENT_SEARCH_MAX))
            else {
                send_text_required(bot, msg.chat.id, "Scrivi il nome dell'alimento da cercare.")
                    .await?;
                return Ok(true);
            };
            let foods = search_food_choices(pool, &query, FOOD_SEARCH_LIMIT)
                .await
                .unwrap_or_default();
            sessions.set(chat_id, RecipeConversationState::IngredientSearch { draft });
            show_food_search_results(
                bot,
                msg.chat.id,
                &query,
                &foods,
                "recipe:new:food",
                "recipe:new:ingredients",
            )
            .await?;
            Ok(true)
        }
        RecipeConversationState::IngredientQuantityReady {
            mut draft,
            food,
            product,
            unit,
        } => {
            let Some(text) = text_hint else {
                send_text_required(bot, msg.chat.id, "Inserisci la quantità necessaria.").await?;
                return Ok(true);
            };
            let Some(quantity) = parse_positive_number(text) else {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Quantità non valida. Inserisci un numero maggiore di zero.",
                )
                .reply_markup(ingredient_quantity_keyboard(
                    "recipe:new:quantity:changeunit",
                    "recipe:new:ingredients",
                ))
                .await?;
                return Ok(true);
            };
            draft.ingredients.push(DraftIngredient {
                food_id: food.id,
                food_name: food.name,
                product_id: product.as_ref().map(|value| value.product_id),
                product_label: product.as_ref().map(product_label),
                quantity,
                unit_id: unit.id,
                unit_symbol: unit.symbol,
            });
            sessions.set(
                chat_id,
                RecipeConversationState::IngredientHub {
                    draft: draft.clone(),
                },
            );
            show_draft_ingredients(bot, msg.chat.id, &draft).await?;
            Ok(true)
        }
        RecipeConversationState::StepText { mut draft } => {
            let Some(text) = text_hint.and_then(|value| clean_text(value, STEP_TEXT_MAX)) else {
                send_text_required(bot, msg.chat.id, "Scrivi il testo dello step.").await?;
                return Ok(true);
            };
            draft.steps.push(DraftStep {
                text,
                media: Vec::new(),
            });
            let step_index = draft.steps.len() - 1;
            sessions.set(
                chat_id,
                RecipeConversationState::StepMedia {
                    draft: draft.clone(),
                    step_index,
                },
            );
            show_step_media_menu(bot, msg.chat.id, &draft, step_index).await?;
            Ok(true)
        }
        RecipeConversationState::StepMedia { draft, step_index } => {
            sessions.set(
                chat_id,
                RecipeConversationState::StepMedia { draft, step_index },
            );
            Ok(true)
        }
        RecipeConversationState::StepPhoto {
            mut draft,
            step_index,
        } => {
            if msg.photo().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "📷 Invia una foto per questo step oppure usa ❌ Annulla allegato.",
                )
                .reply_markup(step_attachment_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            match save_draft_media(bot, msg, "foto", step_index + 1).await {
                Ok(media) => {
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    if let Some(step) = draft.steps.get_mut(step_index) {
                        step.media.push(media);
                    }
                    sessions.set(
                        chat_id,
                        RecipeConversationState::StepMedia {
                            draft: draft.clone(),
                            step_index,
                        },
                    );
                    bot.send_message(msg.chat.id, "✅ Foto aggiunta allo step.")
                        .await?;
                    show_step_media_menu(bot, msg.chat.id, &draft, step_index).await?;
                }
                Err(error) => {
                    tracing::warn!(?error, "Salvataggio foto step ricetta non riuscito");
                    bot.send_message(msg.chat.id, "⚠️ Non riesco a salvare questa foto. Riprova.")
                        .reply_markup(step_attachment_cancel_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::StepVideo {
            mut draft,
            step_index,
        } => {
            if msg.video().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "🎥 Invia un video per questo step oppure usa ❌ Annulla allegato.",
                )
                .reply_markup(step_attachment_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            match save_draft_media(bot, msg, "video", step_index + 1).await {
                Ok(media) => {
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    if let Some(step) = draft.steps.get_mut(step_index) {
                        step.media.push(media);
                    }
                    sessions.set(
                        chat_id,
                        RecipeConversationState::StepMedia {
                            draft: draft.clone(),
                            step_index,
                        },
                    );
                    bot.send_message(msg.chat.id, "✅ Video aggiunto allo step.")
                        .await?;
                    show_step_media_menu(bot, msg.chat.id, &draft, step_index).await?;
                }
                Err(error) => {
                    tracing::warn!(?error, "Salvataggio video step ricetta non riuscito");
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Non riesco a salvare questo video. Riprova.",
                    )
                    .reply_markup(step_attachment_cancel_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::AfterStep { draft } => {
            sessions.set(chat_id, RecipeConversationState::AfterStep { draft });
            Ok(true)
        }
        RecipeConversationState::Visibility { draft } => {
            sessions.set(chat_id, RecipeConversationState::Visibility { draft });
            Ok(true)
        }
        RecipeConversationState::VisibilityChoose { draft, selected } => {
            sessions.set(
                chat_id,
                RecipeConversationState::VisibilityChoose { draft, selected },
            );
            Ok(true)
        }
        RecipeConversationState::SearchName => {
            let Some(query) = text_hint.and_then(|value| clean_text(value, RECIPE_NAME_MAX)) else {
                send_text_required(bot, msg.chat.id, "Scrivi il nome della ricetta da cercare.")
                    .await?;
                return Ok(true);
            };
            sessions.clear_chat(chat_id);
            show_recipe_name_search(bot, msg.chat.id, pool, &query).await?;
            Ok(true)
        }
        RecipeConversationState::IngredientFinder { selected } => {
            sessions.set(
                chat_id,
                RecipeConversationState::IngredientFinder { selected },
            );
            Ok(true)
        }
        RecipeConversationState::IngredientFinderQuery {
            selected,
            category_filter,
        } => {
            let Some(query) = text_hint.and_then(|value| clean_text(value, INGREDIENT_SEARCH_MAX))
            else {
                send_text_required(
                    bot,
                    msg.chat.id,
                    "Scrivi un alimento da aggiungere alla ricerca.",
                )
                .await?;
                return Ok(true);
            };
            let foods = search_food_choices_filtered(
                pool,
                &query,
                FOOD_SEARCH_LIMIT,
                category_filter.as_ref().map(|category| category.id),
            )
            .await
            .unwrap_or_default();
            sessions.set(
                chat_id,
                RecipeConversationState::IngredientFinderQuery {
                    selected,
                    category_filter: category_filter.clone(),
                },
            );
            show_food_search_results(
                bot,
                msg.chat.id,
                &query,
                &foods,
                "recipe:find:food",
                "recipe:find:return",
            )
            .await?;
            Ok(true)
        }
        RecipeConversationState::EditName { recipe_id } => {
            let Some(name) = text_hint.and_then(|value| clean_text(value, RECIPE_NAME_MAX)) else {
                send_text_required(bot, msg.chat.id, "Scrivi il nuovo nome della ricetta.").await?;
                return Ok(true);
            };
            match update_recipe_name(pool, recipe_id, &name).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Nome ricetta aggiornato.")
                        .await?;
                    show_recipe_detail(bot, msg.chat.id, pool, recipe_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_keyboard(&format!("recipe:detail:{recipe_id}")))
                        .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::EditServings { recipe_id } => {
            let Some(text) = text_hint else {
                send_text_required(bot, msg.chat.id, "Invia il nuovo numero di porzioni.").await?;
                return Ok(true);
            };
            let Some(servings) = parse_positive_i64(text) else {
                bot.send_message(msg.chat.id, "⚠️ Inserisci un numero intero positivo.")
                    .reply_markup(flow_keyboard(&format!("recipe:detail:{recipe_id}")))
                    .await?;
                return Ok(true);
            };
            match update_recipe_servings(pool, recipe_id, servings).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Porzioni aggiornate.")
                        .await?;
                    show_recipe_detail(bot, msg.chat.id, pool, recipe_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_keyboard(&format!("recipe:detail:{recipe_id}")))
                        .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::EditIngredientSearch { recipe_id } => {
            let Some(query) = text_hint.and_then(|value| clean_text(value, INGREDIENT_SEARCH_MAX))
            else {
                send_text_required(bot, msg.chat.id, "Scrivi l'alimento da aggiungere.").await?;
                return Ok(true);
            };
            let foods = search_food_choices(pool, &query, FOOD_SEARCH_LIMIT)
                .await
                .unwrap_or_default();
            sessions.set(
                chat_id,
                RecipeConversationState::EditIngredientSearch { recipe_id },
            );
            show_food_search_results(
                bot,
                msg.chat.id,
                &query,
                &foods,
                &format!("recipe:edit:addfood:{recipe_id}"),
                &format!("recipe:edit:ingredients:{recipe_id}"),
            )
            .await?;
            Ok(true)
        }
        RecipeConversationState::EditIngredientQuantityReady {
            recipe_id,
            food,
            product,
            unit,
        } => {
            let Some(text) = text_hint else {
                send_text_required(bot, msg.chat.id, "Inserisci la quantità necessaria.").await?;
                return Ok(true);
            };
            let Some(quantity) = parse_positive_number(text) else {
                bot.send_message(msg.chat.id, "⚠️ Quantità non valida.")
                    .reply_markup(ingredient_quantity_keyboard(
                        &format!("recipe:edit:quantity:changeunit:{recipe_id}"),
                        &format!("recipe:edit:ingredients:{recipe_id}"),
                    ))
                    .await?;
                return Ok(true);
            };
            match insert_recipe_ingredient(
                pool,
                recipe_id,
                &food,
                product.as_ref(),
                quantity,
                &unit,
            )
            .await
            {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Ingrediente aggiunto.")
                        .await?;
                    show_manage_ingredients(bot, msg.chat.id, pool, recipe_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(back_home_keyboard(&format!(
                            "recipe:edit:ingredients:{recipe_id}"
                        )))
                        .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::EditStepText { recipe_id, step_id } => {
            let Some(text) = text_hint.and_then(|value| clean_text(value, STEP_TEXT_MAX)) else {
                send_text_required(bot, msg.chat.id, "Scrivi il testo dello step.").await?;
                return Ok(true);
            };
            let result = match step_id {
                Some(step_id) => update_step_text(pool, recipe_id, step_id, &text).await,
                None => add_recipe_step(pool, recipe_id, &text).await.map(|_| ()),
            };
            match result {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Procedimento aggiornato.")
                        .await?;
                    show_manage_steps(bot, msg.chat.id, pool, recipe_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_keyboard(&format!("recipe:edit:steps:{recipe_id}")))
                        .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::EditStepPhoto { recipe_id, step_id } => {
            if msg.photo().is_none() {
                bot.send_message(msg.chat.id, "📷 Invia la foto da associare allo step.")
                    .reply_markup(flow_keyboard(&format!(
                        "recipe:edit:step:{recipe_id}:{step_id}"
                    )))
                    .await?;
                return Ok(true);
            }
            match save_existing_step_media(bot, msg, pool, recipe_id, step_id, "foto").await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    bot.send_message(msg.chat.id, "✅ Foto aggiunta.").await?;
                    show_step_manage(bot, msg.chat.id, pool, recipe_id, step_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_keyboard(&format!(
                            "recipe:edit:step:{recipe_id}:{step_id}"
                        )))
                        .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::EditStepVideo { recipe_id, step_id } => {
            if msg.video().is_none() {
                bot.send_message(msg.chat.id, "🎥 Invia il video da associare allo step.")
                    .reply_markup(flow_keyboard(&format!(
                        "recipe:edit:step:{recipe_id}:{step_id}"
                    )))
                    .await?;
                return Ok(true);
            }
            match save_existing_step_media(bot, msg, pool, recipe_id, step_id, "video").await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    bot.send_message(msg.chat.id, "✅ Video aggiunto.").await?;
                    show_step_manage(bot, msg.chat.id, pool, recipe_id, step_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_keyboard(&format!(
                            "recipe:edit:step:{recipe_id}:{step_id}"
                        )))
                        .await?;
                }
            }
            Ok(true)
        }
        RecipeConversationState::EditVisibilityChoose {
            recipe_id,
            selected,
        } => {
            sessions.set(
                chat_id,
                RecipeConversationState::EditVisibilityChoose {
                    recipe_id,
                    selected,
                },
            );
            Ok(true)
        }
    }
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &RecipeSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    let gestisce_callback =
        data.starts_with("recipe:") || (data == "menu:main" && sessions.has_active(chat_id.0));
    if !gestisce_callback {
        return Ok(false);
    }

    if data == "menu:main" {
        cancel_draft_media(chat_id.0).await;
        sessions.clear_chat(chat_id.0);
        return Ok(false);
    }

    match data {
        "recipe:noop" => {}
        "recipe:menu" => {
            cancel_draft_media(chat_id.0).await;
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id, pool).await?;
        }
        "recipe:new" => {
            cancel_draft_media(chat_id.0).await;
            sessions.set(chat_id.0, RecipeConversationState::NewName);
            bot.send_message(
                chat_id,
                "🍳 Nuova ricetta\n\nScrivi il nome della ricetta.\n\nPuoi premere ❌ Annulla in qualsiasi momento.",
            )
            .reply_markup(flow_keyboard("recipe:menu"))
            .await?;
        }
        "recipe:new:cancel" => {
            cancel_draft_media(chat_id.0).await;
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, "❌ Creazione ricetta annullata.")
                .reply_markup(recipe_menu_keyboard())
                .await?;
        }
        "recipe:new:ingredients" => {
            if let Some(draft) = draft_from_state(sessions.get(chat_id.0)) {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::IngredientHub {
                        draft: draft.clone(),
                    },
                );
                show_draft_ingredients(bot, chat_id, &draft).await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:ingredient:add" => {
            if let Some(draft) = draft_from_state(sessions.get(chat_id.0)) {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::IngredientSearch { draft },
                );
                bot.send_message(
                    chat_id,
                    "🔎 Cerca alimento\n\nScrivi il nome dell'alimento da aggiungere. La ricerca considera anche marca e nome dei prodotti commerciali associati.",
                )
                .reply_markup(flow_keyboard("recipe:new:ingredients"))
                .await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:ingredients:done" => {
            if let Some(draft) = draft_from_state(sessions.get(chat_id.0)) {
                if draft.ingredients.is_empty() {
                    bot.send_message(
                        chat_id,
                        "⚠️ Aggiungi almeno un ingrediente prima di continuare.",
                    )
                    .reply_markup(draft_ingredients_keyboard(&draft))
                    .await?;
                } else {
                    sessions.set(chat_id.0, RecipeConversationState::StepText { draft });
                    bot.send_message(
                        chat_id,
                        "📝 Procedimento guidato · Step 1\n\nScrivi cosa bisogna fare in questo primo step. Foto e video potranno essere aggiunti subito dopo.",
                    )
                    .reply_markup(flow_keyboard("recipe:new:ingredients"))
                    .await?;
                }
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:step:photo" => {
            if let Some((draft, step_index)) = step_media_state(sessions.get(chat_id.0)) {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::StepPhoto { draft, step_index },
                );
                bot.send_message(
                    chat_id,
                    "📷 Invia una foto da associare a questo step. Puoi aggiungerne altre dopo.",
                )
                .reply_markup(step_attachment_cancel_keyboard())
                .await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:step:video" => {
            if let Some((draft, step_index)) = step_media_state(sessions.get(chat_id.0)) {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::StepVideo { draft, step_index },
                );
                bot.send_message(
                    chat_id,
                    "🎥 Invia un video da associare a questo step. Puoi aggiungerne altri dopo.",
                )
                .reply_markup(step_attachment_cancel_keyboard())
                .await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:step:attachment:cancel" => match sessions.get(chat_id.0) {
            Some(RecipeConversationState::StepPhoto { draft, step_index })
            | Some(RecipeConversationState::StepVideo { draft, step_index }) => {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::StepMedia {
                        draft: draft.clone(),
                        step_index,
                    },
                );
                show_step_media_menu(bot, chat_id, &draft, step_index).await?;
            }
            _ => show_expired_flow(bot, chat_id).await?,
        },
        "recipe:new:step:done" => {
            if let Some((draft, _)) = step_media_state(sessions.get(chat_id.0)) {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::AfterStep {
                        draft: draft.clone(),
                    },
                );
                show_after_step(bot, chat_id, &draft).await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:step:add" => {
            if let Some(draft) = draft_from_state(sessions.get(chat_id.0)) {
                let number = draft.steps.len() + 1;
                sessions.set(chat_id.0, RecipeConversationState::StepText { draft });
                bot.send_message(
                    chat_id,
                    format!("📝 Procedimento guidato · Step {number}\n\nScrivi il testo del nuovo step."),
                )
                .reply_markup(flow_keyboard("recipe:new:ingredients"))
                .await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:steps:done" => {
            if let Some(draft) = draft_from_state(sessions.get(chat_id.0)) {
                if draft.steps.is_empty() {
                    bot.send_message(chat_id, "⚠️ Inserisci almeno uno step del procedimento.")
                        .reply_markup(flow_keyboard("recipe:new:ingredients"))
                        .await?;
                } else {
                    sessions.set(
                        chat_id.0,
                        RecipeConversationState::Visibility {
                            draft: draft.clone(),
                        },
                    );
                    show_visibility_choice(bot, chat_id, &draft).await?;
                }
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:visibility:private" => {
            if let Some(mut draft) = draft_from_state(sessions.get(chat_id.0)) {
                draft.visible_spaces.clear();
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::Visibility {
                        draft: draft.clone(),
                    },
                );
                show_recipe_confirmation(bot, chat_id, &draft).await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:visibility:default" => {
            if let Some(mut draft) = draft_from_state(sessions.get(chat_id.0)) {
                draft.visible_spaces = vec![identity::current_actor().spazio_id];
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::Visibility {
                        draft: draft.clone(),
                    },
                );
                show_recipe_confirmation(bot, chat_id, &draft).await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:visibility:all" => {
            if let Some(mut draft) = draft_from_state(sessions.get(chat_id.0)) {
                let actor = identity::current_actor();
                let Some(user_id) = actor.utente_id else {
                    show_expired_flow(bot, chat_id).await?;
                    return Ok(true);
                };
                draft.visible_spaces = identity::list_user_spaces(pool, user_id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|space| {
                        matches!(
                            space.ruolo.as_str(),
                            "proprietario" | "amministratore" | "membro"
                        )
                    })
                    .map(|space| space.id)
                    .collect();
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::Visibility {
                        draft: draft.clone(),
                    },
                );
                show_recipe_confirmation(bot, chat_id, &draft).await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:visibility:choose" => {
            if let Some(draft) = draft_from_state(sessions.get(chat_id.0)) {
                let selected = draft.visible_spaces.clone();
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::VisibilityChoose {
                        draft: draft.clone(),
                        selected: selected.clone(),
                    },
                );
                show_space_picker(
                    bot,
                    chat_id,
                    pool,
                    &selected,
                    "recipe:new:visibility:toggle",
                    "recipe:new:visibility:done",
                    "recipe:new:steps:done",
                )
                .await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        "recipe:new:visibility:done" => match sessions.get(chat_id.0) {
            Some(RecipeConversationState::VisibilityChoose {
                mut draft,
                selected,
            }) => {
                draft.visible_spaces = selected;
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::Visibility {
                        draft: draft.clone(),
                    },
                );
                show_recipe_confirmation(bot, chat_id, &draft).await?;
            }
            _ => show_expired_flow(bot, chat_id).await?,
        },
        "recipe:new:save" => {
            let Some(draft) = draft_from_state(sessions.get(chat_id.0)) else {
                show_expired_flow(bot, chat_id).await?;
                return Ok(true);
            };
            match save_recipe(pool, chat_id.0, &draft).await {
                Ok(recipe_id) => {
                    if let Err(error) = finalize_draft_media(pool, chat_id.0, recipe_id).await {
                        tracing::error!(?error, recipe_id, "Finalizzazione media ricetta fallita");
                        bot.send_message(chat_id, "⚠️ Ricetta salvata, ma alcuni allegati non sono stati finalizzati correttamente.").await?;
                    }
                    sessions.clear_chat(chat_id.0);
                    bot.send_message(chat_id, format!("✅ Ricetta salvata: {}", draft.name))
                        .await?;
                    show_recipe_detail(bot, chat_id, pool, recipe_id).await?;
                }
                Err(error) => {
                    tracing::warn!(?error, "Salvataggio ricetta non riuscito");
                    bot.send_message(
                        chat_id,
                        format!("⚠️ Non riesco a salvare la ricetta: {error}"),
                    )
                    .reply_markup(recipe_confirmation_keyboard())
                    .await?;
                }
            }
        }
        "recipe:search" => {
            sessions.set(chat_id.0, RecipeConversationState::SearchName);
            bot.send_message(chat_id, "🔎 Cerca ricetta\n\nScrivi una parte del nome.")
                .reply_markup(flow_keyboard("recipe:menu"))
                .await?;
        }
        "recipe:find" => {
            let selected = Vec::new();
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: None,
                },
            );
            show_ingredient_finder(bot, chat_id, &selected, None).await?;
        }
        "recipe:find:return" => {
            let (selected, category_filter) = ingredient_query_state(sessions.get(chat_id.0));
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: category_filter.clone(),
                },
            );
            show_ingredient_finder(bot, chat_id, &selected, category_filter.as_ref()).await?;
        }
        "recipe:find:addmore" => {
            let (selected, category_filter) = ingredient_query_state(sessions.get(chat_id.0));
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: category_filter.clone(),
                },
            );
            bot.send_message(
                chat_id,
                "🥕 Aggiungi ingrediente alla ricerca

Scrivi il nome dell'alimento. Il filtro categoria, se attivo, viene mantenuto.",
            )
            .reply_markup(ingredient_finder_keyboard(
                &selected,
                category_filter.as_ref(),
            ))
            .await?;
        }
        "recipe:find:reset" => {
            let selected = Vec::new();
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: None,
                },
            );
            show_ingredient_finder(bot, chat_id, &selected, None).await?;
        }
        "recipe:find:run" => {
            let selected = selected_foods_from_state(sessions.get(chat_id.0)).unwrap_or_default();
            if selected.is_empty() {
                let category_filter = ingredient_filter_from_state(sessions.get(chat_id.0));
                bot.send_message(chat_id, "⚠️ Seleziona almeno un ingrediente.")
                    .reply_markup(ingredient_finder_keyboard(
                        &selected,
                        category_filter.as_ref(),
                    ))
                    .await?;
            } else {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::IngredientFinder {
                        selected: selected.clone(),
                    },
                );
                show_ingredient_search_results(bot, chat_id, pool, &selected).await?;
            }
        }
        _ if data.starts_with("recipe:list:") => {
            let page = data
                .strip_prefix("recipe:list:")
                .and_then(parse_nonnegative_i64)
                .unwrap_or(0);
            sessions.clear_chat(chat_id.0);
            show_recipe_list(bot, chat_id, pool, page).await?;
        }
        _ if data.starts_with("recipe:detail:") => {
            if let Some(recipe_id) = data
                .strip_prefix("recipe:detail:")
                .and_then(parse_positive_i64_str)
            {
                sessions.clear_chat(chat_id.0);
                show_recipe_detail(bot, chat_id, pool, recipe_id).await?;
            } else {
                show_invalid_action(bot, chat_id).await?;
            }
        }
        _ if data.starts_with("recipe:new:ingredient:remove:") => {
            let index = data
                .strip_prefix("recipe:new:ingredient:remove:")
                .and_then(|value| value.parse::<usize>().ok());
            if let (Some(mut draft), Some(index)) =
                (draft_from_state(sessions.get(chat_id.0)), index)
            {
                if index < draft.ingredients.len() {
                    draft.ingredients.remove(index);
                }
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::IngredientHub {
                        draft: draft.clone(),
                    },
                );
                show_draft_ingredients(bot, chat_id, &draft).await?;
            } else {
                show_expired_flow(bot, chat_id).await?;
            }
        }
        _ if data.starts_with("recipe:new:food:") => {
            let food_id = data
                .strip_prefix("recipe:new:food:")
                .and_then(parse_positive_i64_str);
            let Some(draft) = draft_from_state(sessions.get(chat_id.0)) else {
                show_expired_flow(bot, chat_id).await?;
                return Ok(true);
            };
            let Some(food_id) = food_id else {
                show_invalid_action(bot, chat_id).await?;
                return Ok(true);
            };
            if draft
                .ingredients
                .iter()
                .any(|ingredient| ingredient.food_id == food_id)
            {
                bot.send_message(chat_id, "⚠️ Questo alimento è già presente nella ricetta.")
                    .reply_markup(draft_ingredients_keyboard(&draft))
                    .await?;
                sessions.set(chat_id.0, RecipeConversationState::IngredientHub { draft });
                return Ok(true);
            }
            let Some(food) = visible_food_choice(pool, food_id).await.unwrap_or(None) else {
                bot.send_message(chat_id, "⚠️ Alimento non disponibile.")
                    .await?;
                return Ok(true);
            };
            let products = product_choices_for_food(pool, food.id)
                .await
                .unwrap_or_default();
            if products.is_empty() {
                begin_new_ingredient_quantity(bot, chat_id, pool, sessions, draft, food, None)
                    .await?;
            } else {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::IngredientSearch { draft },
                );
                show_product_choice(
                    bot,
                    chat_id,
                    &food,
                    &products,
                    "recipe:new:product:generic",
                    "recipe:new:product",
                    "recipe:new:ingredients",
                )
                .await?;
            }
        }
        _ if data.starts_with("recipe:new:product:generic:") => {
            let food_id = data
                .strip_prefix("recipe:new:product:generic:")
                .and_then(parse_positive_i64_str);
            let Some(draft) = draft_from_state(sessions.get(chat_id.0)) else {
                show_expired_flow(bot, chat_id).await?;
                return Ok(true);
            };
            let Some(food) = (match food_id {
                Some(id) => visible_food_choice(pool, id).await.unwrap_or(None),
                None => None,
            }) else {
                show_invalid_action(bot, chat_id).await?;
                return Ok(true);
            };
            begin_new_ingredient_quantity(bot, chat_id, pool, sessions, draft, food, None).await?;
        }
        _ if data.starts_with("recipe:new:product:") => {
            let product_id = data
                .strip_prefix("recipe:new:product:")
                .and_then(parse_positive_i64_str);
            let Some(draft) = draft_from_state(sessions.get(chat_id.0)) else {
                show_expired_flow(bot, chat_id).await?;
                return Ok(true);
            };
            let Some(product_id) = product_id else {
                show_invalid_action(bot, chat_id).await?;
                return Ok(true);
            };
            let Some((food, product)) = visible_product_choice(pool, product_id)
                .await
                .unwrap_or(None)
            else {
                bot.send_message(chat_id, "⚠️ Prodotto non disponibile.")
                    .await?;
                return Ok(true);
            };
            if draft
                .ingredients
                .iter()
                .any(|ingredient| ingredient.food_id == food.id)
            {
                bot.send_message(chat_id, "⚠️ Questo alimento è già presente nella ricetta.")
                    .await?;
                return Ok(true);
            }
            begin_new_ingredient_quantity(bot, chat_id, pool, sessions, draft, food, Some(product))
                .await?;
        }
        "recipe:new:quantity:changeunit" => match sessions.get(chat_id.0) {
            Some(RecipeConversationState::IngredientQuantityReady {
                draft,
                food,
                product,
                unit,
            }) => {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::IngredientQuantityReady {
                        draft,
                        food: food.clone(),
                        product,
                        unit,
                    },
                );
                show_unit_choice(
                    bot,
                    chat_id,
                    pool,
                    &food,
                    "recipe:new:quantity:unit",
                    "recipe:new:ingredients",
                )
                .await?;
            }
            _ => show_expired_flow(bot, chat_id).await?,
        },
        _ if data.starts_with("recipe:new:quantity:unit:") => {
            let unit_id = data
                .strip_prefix("recipe:new:quantity:unit:")
                .and_then(parse_positive_i64_str);
            let Some(unit_id) = unit_id else {
                show_invalid_action(bot, chat_id).await?;
                return Ok(true);
            };
            match sessions.get(chat_id.0) {
                Some(RecipeConversationState::IngredientQuantityReady {
                    draft,
                    food,
                    product,
                    ..
                }) => {
                    let Some(unit) = unit_by_id(pool, unit_id).await.unwrap_or(None) else {
                        bot.send_message(chat_id, "⚠️ Unità non disponibile.")
                            .await?;
                        return Ok(true);
                    };
                    sessions.set(
                        chat_id.0,
                        RecipeConversationState::IngredientQuantityReady {
                            draft,
                            food: food.clone(),
                            product: product.clone(),
                            unit: unit.clone(),
                        },
                    );
                    ask_ingredient_quantity_ready(
                        bot,
                        chat_id,
                        &food,
                        product.as_ref(),
                        &unit,
                        "recipe:new:quantity:changeunit",
                        "recipe:new:ingredients",
                    )
                    .await?;
                }
                _ => show_expired_flow(bot, chat_id).await?,
            }
        }
        _ if data.starts_with("recipe:new:visibility:toggle:") => {
            let space_id = data
                .strip_prefix("recipe:new:visibility:toggle:")
                .and_then(parse_positive_i64_str);
            match (sessions.get(chat_id.0), space_id) {
                (
                    Some(RecipeConversationState::VisibilityChoose {
                        draft,
                        mut selected,
                    }),
                    Some(space_id),
                ) => {
                    toggle_id(&mut selected, space_id);
                    sessions.set(
                        chat_id.0,
                        RecipeConversationState::VisibilityChoose {
                            draft,
                            selected: selected.clone(),
                        },
                    );
                    show_space_picker(
                        bot,
                        chat_id,
                        pool,
                        &selected,
                        "recipe:new:visibility:toggle",
                        "recipe:new:visibility:done",
                        "recipe:new:steps:done",
                    )
                    .await?;
                }
                _ => show_expired_flow(bot, chat_id).await?,
            }
        }
        "recipe:find:categories" => {
            let (selected, category_filter) = ingredient_query_state(sessions.get(chat_id.0));
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: category_filter.clone(),
                },
            );
            show_recipe_food_categories(bot, chat_id, pool, 0, &selected, category_filter.as_ref())
                .await?;
        }
        _ if data.starts_with("recipe:find:categories:") => {
            let page = data
                .strip_prefix("recipe:find:categories:")
                .and_then(parse_nonnegative_i64)
                .unwrap_or(0);
            let (selected, category_filter) = ingredient_query_state(sessions.get(chat_id.0));
            show_recipe_food_categories(
                bot,
                chat_id,
                pool,
                page,
                &selected,
                category_filter.as_ref(),
            )
            .await?;
        }
        "recipe:find:filter:clear" => {
            let selected = selected_foods_from_state(sessions.get(chat_id.0)).unwrap_or_default();
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: None,
                },
            );
            show_ingredient_finder(bot, chat_id, &selected, None).await?;
        }
        _ if data.starts_with("recipe:find:filter:") => {
            let category_id = data
                .strip_prefix("recipe:find:filter:")
                .and_then(parse_positive_i64_str);
            let Some(category_id) = category_id else {
                show_invalid_action(bot, chat_id).await?;
                return Ok(true);
            };
            let Some(category) = recipe_food_category_by_id(pool, category_id)
                .await
                .unwrap_or(None)
            else {
                show_invalid_action(bot, chat_id).await?;
                return Ok(true);
            };
            let selected = selected_foods_from_state(sessions.get(chat_id.0)).unwrap_or_default();
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: Some(category.clone()),
                },
            );
            show_ingredient_finder(bot, chat_id, &selected, Some(&category)).await?;
        }
        _ if data.starts_with("recipe:find:food:") => {
            let food_id = data
                .strip_prefix("recipe:find:food:")
                .and_then(parse_positive_i64_str);
            let (mut selected, category_filter) = ingredient_query_state(sessions.get(chat_id.0));
            if let Some(food_id) = food_id {
                if let Some(food) = visible_food_choice(pool, food_id).await.unwrap_or(None) {
                    if !selected.iter().any(|value| value.id == food.id) {
                        selected.push(food);
                    }
                }
            }
            sessions.set(
                chat_id.0,
                RecipeConversationState::IngredientFinderQuery {
                    selected: selected.clone(),
                    category_filter: category_filter.clone(),
                },
            );
            show_ingredient_finder(bot, chat_id, &selected, category_filter.as_ref()).await?;
        }
        _ if data.starts_with("recipe:complete:") => {
            if let Some(recipe_id) = data
                .strip_prefix("recipe:complete:")
                .and_then(parse_positive_i64_str)
            {
                sessions.clear_chat(chat_id.0);
                show_full_procedure(bot, chat_id, pool, recipe_id).await?;
            } else {
                show_invalid_action(bot, chat_id).await?;
            }
        }
        _ if data.starts_with("recipe:guided:finish:") => {
            if let Some(recipe_id) = data
                .strip_prefix("recipe:guided:finish:")
                .and_then(parse_positive_i64_str)
            {
                sessions.clear_chat(chat_id.0);
                show_guided_finished(bot, chat_id, pool, recipe_id).await?;
            } else {
                show_invalid_action(bot, chat_id).await?;
            }
        }
        _ if data.starts_with("recipe:guided:") => {
            let raw = data.strip_prefix("recipe:guided:").unwrap_or_default();
            if let Some((recipe_id, page)) = parse_recipe_page(raw) {
                sessions.clear_chat(chat_id.0);
                show_guided_step(bot, chat_id, pool, recipe_id, page).await?;
            } else {
                show_invalid_action(bot, chat_id).await?;
            }
        }
        _ if data.starts_with("recipe:step:media:") => {
            if let Some(step_id) = data
                .strip_prefix("recipe:step:media:")
                .and_then(parse_positive_i64_str)
            {
                show_step_media(bot, chat_id, pool, step_id).await?;
            } else {
                show_invalid_action(bot, chat_id).await?;
            }
        }
        _ if data.starts_with("recipe:media:item:") => {
            if let Some(media_id) = data
                .strip_prefix("recipe:media:item:")
                .and_then(parse_positive_i64_str)
            {
                show_media_item(bot, chat_id, pool, media_id).await?;
            } else {
                show_invalid_action(bot, chat_id).await?;
            }
        }
        _ if data.starts_with("recipe:edit:") => {
            handle_edit_callback(bot, chat_id, pool, sessions, data).await?;
        }
        _ if data.starts_with("recipe:invite:") => {
            handle_invite_callback(bot, chat_id, pool, data).await?;
        }
        _ => {
            show_invalid_action(bot, chat_id).await?;
        }
    }

    Ok(true)
}

async fn handle_edit_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &RecipeSessionStore,
    data: &str,
) -> ResponseResult<()> {
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:menu:")
        .and_then(parse_positive_i64_str)
    {
        sessions.clear_chat(chat_id.0);
        show_edit_menu(bot, chat_id, pool, recipe_id).await?;
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:name:")
        .and_then(parse_positive_i64_str)
    {
        if ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
            sessions.set(chat_id.0, RecipeConversationState::EditName { recipe_id });
            bot.send_message(chat_id, "✏️ Nome ricetta\n\nScrivi il nuovo nome.")
                .reply_markup(flow_keyboard(&format!("recipe:edit:menu:{recipe_id}")))
                .await?;
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:servings:")
        .and_then(parse_positive_i64_str)
    {
        if ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
            sessions.set(
                chat_id.0,
                RecipeConversationState::EditServings { recipe_id },
            );
            bot.send_message(
                chat_id,
                "👥 Porzioni base\n\nInvia il nuovo numero di porzioni.",
            )
            .reply_markup(flow_keyboard(&format!("recipe:edit:menu:{recipe_id}")))
            .await?;
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:ingredients:")
        .and_then(parse_positive_i64_str)
    {
        sessions.clear_chat(chat_id.0);
        show_manage_ingredients(bot, chat_id, pool, recipe_id).await?;
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:ingredient:add:")
        .and_then(parse_positive_i64_str)
    {
        if ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
            sessions.set(
                chat_id.0,
                RecipeConversationState::EditIngredientSearch { recipe_id },
            );
            bot.send_message(
                chat_id,
                "🔎 Aggiungi ingrediente\n\nScrivi il nome dell'alimento.",
            )
            .reply_markup(flow_keyboard(&format!(
                "recipe:edit:ingredients:{recipe_id}"
            )))
            .await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:irem:") {
        if let Some((recipe_id, ingredient_id)) = parse_two_positive_ids(raw) {
            match remove_recipe_ingredient(pool, recipe_id, ingredient_id).await {
                Ok(()) => {
                    bot.send_message(chat_id, "✅ Ingrediente rimosso.").await?;
                    show_manage_ingredients(bot, chat_id, pool, recipe_id).await?;
                }
                Err(error) => {
                    bot.send_message(chat_id, format!("⚠️ {error}"))
                        .reply_markup(back_home_keyboard(&format!(
                            "recipe:edit:ingredients:{recipe_id}"
                        )))
                        .await?;
                }
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:addfood:") {
        let mut parts = raw.split(':');
        let recipe_id = parts.next().and_then(parse_positive_i64_str);
        let food_id = parts.next().and_then(parse_positive_i64_str);
        if parts.next().is_some() {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        }
        let (Some(recipe_id), Some(food_id)) = (recipe_id, food_id) else {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        };
        if !ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
            return Ok(());
        }
        if recipe_has_food(pool, recipe_id, food_id)
            .await
            .unwrap_or(false)
        {
            bot.send_message(chat_id, "⚠️ Questo alimento è già presente nella ricetta.")
                .reply_markup(back_home_keyboard(&format!(
                    "recipe:edit:ingredients:{recipe_id}"
                )))
                .await?;
            return Ok(());
        }
        let Some(food) = visible_food_choice(pool, food_id).await.unwrap_or(None) else {
            bot.send_message(chat_id, "⚠️ Alimento non disponibile.")
                .await?;
            return Ok(());
        };
        let products = product_choices_for_food(pool, food.id)
            .await
            .unwrap_or_default();
        if products.is_empty() {
            begin_edit_ingredient_quantity(bot, chat_id, pool, sessions, recipe_id, food, None)
                .await?;
        } else {
            sessions.set(
                chat_id.0,
                RecipeConversationState::EditIngredientSearch { recipe_id },
            );
            show_product_choice(
                bot,
                chat_id,
                &food,
                &products,
                &format!("recipe:edit:pg:{recipe_id}"),
                &format!("recipe:edit:p:{recipe_id}"),
                &format!("recipe:edit:ingredients:{recipe_id}"),
            )
            .await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:pg:") {
        let mut parts = raw.split(':');
        let recipe_id = parts.next().and_then(parse_positive_i64_str);
        let food_id = parts.next().and_then(parse_positive_i64_str);
        if parts.next().is_some() {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        }
        let (Some(recipe_id), Some(food_id)) = (recipe_id, food_id) else {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        };
        let Some(food) = visible_food_choice(pool, food_id).await.unwrap_or(None) else {
            bot.send_message(chat_id, "⚠️ Alimento non disponibile.")
                .await?;
            return Ok(());
        };
        begin_edit_ingredient_quantity(bot, chat_id, pool, sessions, recipe_id, food, None).await?;
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:p:") {
        let mut parts = raw.split(':');
        let recipe_id = parts.next().and_then(parse_positive_i64_str);
        let product_id = parts.next().and_then(parse_positive_i64_str);
        if parts.next().is_some() {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        }
        let (Some(recipe_id), Some(product_id)) = (recipe_id, product_id) else {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        };
        let Some((food, product)) = visible_product_choice(pool, product_id)
            .await
            .unwrap_or(None)
        else {
            bot.send_message(chat_id, "⚠️ Prodotto non disponibile.")
                .await?;
            return Ok(());
        };
        begin_edit_ingredient_quantity(
            bot,
            chat_id,
            pool,
            sessions,
            recipe_id,
            food,
            Some(product),
        )
        .await?;
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:quantity:changeunit:")
        .and_then(parse_positive_i64_str)
    {
        match sessions.get(chat_id.0) {
            Some(RecipeConversationState::EditIngredientQuantityReady {
                recipe_id: state_recipe_id,
                food,
                product,
                unit,
            }) if state_recipe_id == recipe_id => {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::EditIngredientQuantityReady {
                        recipe_id,
                        food: food.clone(),
                        product,
                        unit,
                    },
                );
                show_unit_choice(
                    bot,
                    chat_id,
                    pool,
                    &food,
                    &format!("recipe:edit:quantity:unit:{recipe_id}"),
                    &format!("recipe:edit:ingredients:{recipe_id}"),
                )
                .await?;
            }
            _ => show_expired_flow(bot, chat_id).await?,
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:quantity:unit:") {
        let mut parts = raw.split(':');
        let recipe_id = parts.next().and_then(parse_positive_i64_str);
        let unit_id = parts.next().and_then(parse_positive_i64_str);
        if parts.next().is_some() {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        }
        let (Some(recipe_id), Some(unit_id)) = (recipe_id, unit_id) else {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        };
        match sessions.get(chat_id.0) {
            Some(RecipeConversationState::EditIngredientQuantityReady {
                recipe_id: state_recipe_id,
                food,
                product,
                ..
            }) if state_recipe_id == recipe_id => {
                let Some(unit) = unit_by_id(pool, unit_id).await.unwrap_or(None) else {
                    bot.send_message(chat_id, "⚠️ Unità non disponibile.")
                        .await?;
                    return Ok(());
                };
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::EditIngredientQuantityReady {
                        recipe_id,
                        food: food.clone(),
                        product: product.clone(),
                        unit: unit.clone(),
                    },
                );
                ask_ingredient_quantity_ready(
                    bot,
                    chat_id,
                    &food,
                    product.as_ref(),
                    &unit,
                    &format!("recipe:edit:quantity:changeunit:{recipe_id}"),
                    &format!("recipe:edit:ingredients:{recipe_id}"),
                )
                .await?;
            }
            _ => show_expired_flow(bot, chat_id).await?,
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:steps:")
        .and_then(parse_positive_i64_str)
    {
        sessions.clear_chat(chat_id.0);
        show_manage_steps(bot, chat_id, pool, recipe_id).await?;
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:step:add:")
        .and_then(parse_positive_i64_str)
    {
        if ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
            sessions.set(
                chat_id.0,
                RecipeConversationState::EditStepText {
                    recipe_id,
                    step_id: None,
                },
            );
            let next = recipe_step_count(pool, recipe_id).await.unwrap_or(0) + 1;
            bot.send_message(
                chat_id,
                format!("📝 Nuovo step {next}\n\nScrivi il testo dello step."),
            )
            .reply_markup(flow_keyboard(&format!("recipe:edit:steps:{recipe_id}")))
            .await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:step:text:") {
        if let Some((recipe_id, step_id)) = parse_two_positive_ids(raw) {
            if ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::EditStepText {
                        recipe_id,
                        step_id: Some(step_id),
                    },
                );
                bot.send_message(chat_id, "✏️ Modifica step\n\nScrivi il nuovo testo.")
                    .reply_markup(flow_keyboard(&format!(
                        "recipe:edit:step:{recipe_id}:{step_id}"
                    )))
                    .await?;
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:step:photo:") {
        if let Some((recipe_id, step_id)) = parse_two_positive_ids(raw) {
            if ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::EditStepPhoto { recipe_id, step_id },
                );
                bot.send_message(chat_id, "📷 Invia una foto da aggiungere allo step.")
                    .reply_markup(flow_keyboard(&format!(
                        "recipe:edit:step:{recipe_id}:{step_id}"
                    )))
                    .await?;
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:step:video:") {
        if let Some((recipe_id, step_id)) = parse_two_positive_ids(raw) {
            if ensure_recipe_edit_ui(bot, chat_id, pool, recipe_id).await? {
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::EditStepVideo { recipe_id, step_id },
                );
                bot.send_message(chat_id, "🎥 Invia un video da aggiungere allo step.")
                    .reply_markup(flow_keyboard(&format!(
                        "recipe:edit:step:{recipe_id}:{step_id}"
                    )))
                    .await?;
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:step:up:") {
        if let Some((recipe_id, step_id)) = parse_two_positive_ids(raw) {
            match move_recipe_step(pool, recipe_id, step_id, -1).await {
                Ok(()) => show_manage_steps(bot, chat_id, pool, recipe_id).await?,
                Err(error) => {
                    bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                }
            };
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:step:down:") {
        if let Some((recipe_id, step_id)) = parse_two_positive_ids(raw) {
            match move_recipe_step(pool, recipe_id, step_id, 1).await {
                Ok(()) => show_manage_steps(bot, chat_id, pool, recipe_id).await?,
                Err(error) => {
                    bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                }
            };
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:step:delete:") {
        if let Some((recipe_id, step_id)) = parse_two_positive_ids(raw) {
            match delete_recipe_step(pool, recipe_id, step_id).await {
                Ok(()) => {
                    bot.send_message(chat_id, "✅ Step eliminato e numerazione aggiornata.")
                        .await?;
                    show_manage_steps(bot, chat_id, pool, recipe_id).await?;
                }
                Err(error) => {
                    bot.send_message(chat_id, format!("⚠️ {error}"))
                        .reply_markup(back_home_keyboard(&format!(
                            "recipe:edit:steps:{recipe_id}"
                        )))
                        .await?;
                }
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:step:") {
        if let Some((recipe_id, step_id)) = parse_two_positive_ids(raw) {
            sessions.clear_chat(chat_id.0);
            show_step_manage(bot, chat_id, pool, recipe_id, step_id).await?;
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:md:") {
        if let Some((recipe_id, media_id)) = parse_two_positive_ids(raw) {
            match delete_step_media(pool, recipe_id, media_id).await {
                Ok(step_id) => {
                    bot.send_message(chat_id, "✅ Allegato rimosso.").await?;
                    show_step_manage(bot, chat_id, pool, recipe_id, step_id).await?;
                }
                Err(error) => {
                    bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                }
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:visibility:")
        .and_then(parse_positive_i64_str)
    {
        sessions.clear_chat(chat_id.0);
        show_edit_visibility(bot, chat_id, pool, recipe_id).await?;
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:vis:private:")
        .and_then(parse_positive_i64_str)
    {
        match set_recipe_spaces(pool, recipe_id, &[]).await {
            Ok(()) => {
                bot.send_message(chat_id, "✅ Ricetta impostata come privata.")
                    .await?;
                show_edit_menu(bot, chat_id, pool, recipe_id).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:vis:default:")
        .and_then(parse_positive_i64_str)
    {
        let space_id = identity::current_actor().spazio_id;
        match set_recipe_spaces(pool, recipe_id, &[space_id]).await {
            Ok(()) => {
                bot.send_message(chat_id, "✅ Visibilità aggiornata.")
                    .await?;
                show_edit_menu(bot, chat_id, pool, recipe_id).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:vis:all:")
        .and_then(parse_positive_i64_str)
    {
        let actor = identity::current_actor();
        let spaces = match actor.utente_id {
            Some(user_id) => identity::list_user_spaces(pool, user_id)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|space| {
                    matches!(
                        space.ruolo.as_str(),
                        "proprietario" | "amministratore" | "membro"
                    )
                })
                .map(|space| space.id)
                .collect::<Vec<_>>(),
            None => Vec::new(),
        };
        match set_recipe_spaces(pool, recipe_id, &spaces).await {
            Ok(()) => {
                bot.send_message(
                    chat_id,
                    "✅ Ricetta visibile in tutti i tuoi spazi modificabili.",
                )
                .await?;
                show_edit_menu(bot, chat_id, pool, recipe_id).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:vis:choose:")
        .and_then(parse_positive_i64_str)
    {
        if ensure_recipe_manage_ui(bot, chat_id, pool, recipe_id).await? {
            let selected = recipe_space_ids(pool, recipe_id).await.unwrap_or_default();
            sessions.set(
                chat_id.0,
                RecipeConversationState::EditVisibilityChoose {
                    recipe_id,
                    selected: selected.clone(),
                },
            );
            show_space_picker(
                bot,
                chat_id,
                pool,
                &selected,
                &format!("recipe:edit:vis:toggle:{recipe_id}"),
                &format!("recipe:edit:vis:done:{recipe_id}"),
                &format!("recipe:edit:visibility:{recipe_id}"),
            )
            .await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:vis:toggle:") {
        let mut parts = raw.split(':');
        let recipe_id = parts.next().and_then(parse_positive_i64_str);
        let space_id = parts.next().and_then(parse_positive_i64_str);
        if parts.next().is_some() {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        }
        match (sessions.get(chat_id.0), recipe_id, space_id) {
            (
                Some(RecipeConversationState::EditVisibilityChoose {
                    recipe_id: state_id,
                    mut selected,
                }),
                Some(recipe_id),
                Some(space_id),
            ) if state_id == recipe_id => {
                toggle_id(&mut selected, space_id);
                sessions.set(
                    chat_id.0,
                    RecipeConversationState::EditVisibilityChoose {
                        recipe_id,
                        selected: selected.clone(),
                    },
                );
                show_space_picker(
                    bot,
                    chat_id,
                    pool,
                    &selected,
                    &format!("recipe:edit:vis:toggle:{recipe_id}"),
                    &format!("recipe:edit:vis:done:{recipe_id}"),
                    &format!("recipe:edit:visibility:{recipe_id}"),
                )
                .await?;
            }
            _ => show_expired_flow(bot, chat_id).await?,
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:vis:done:")
        .and_then(parse_positive_i64_str)
    {
        match sessions.get(chat_id.0) {
            Some(RecipeConversationState::EditVisibilityChoose {
                recipe_id: state_id,
                selected,
            }) if state_id == recipe_id => {
                match set_recipe_spaces(pool, recipe_id, &selected).await {
                    Ok(()) => {
                        sessions.clear_chat(chat_id.0);
                        bot.send_message(chat_id, "✅ Visibilità aggiornata.")
                            .await?;
                        show_edit_menu(bot, chat_id, pool, recipe_id).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            _ => show_expired_flow(bot, chat_id).await?,
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:collaborators:")
        .and_then(parse_positive_i64_str)
    {
        sessions.clear_chat(chat_id.0);
        show_collaborators(bot, chat_id, pool, recipe_id).await?;
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:iu:") {
        let mut parts = raw.split(':');
        let recipe_id = parts.next().and_then(parse_positive_i64_str);
        let user_id = parts.next().and_then(parse_positive_i64_str);
        if parts.next().is_some() {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        }
        if let (Some(recipe_id), Some(user_id)) = (recipe_id, user_id) {
            if ensure_recipe_manage_ui(bot, chat_id, pool, recipe_id).await? {
                show_invite_permission_choice(bot, chat_id, recipe_id, user_id).await?;
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:ie:") {
        if let Some((recipe_id, user_id)) = parse_two_positive_ids(raw) {
            create_recipe_invite_ui(
                bot,
                chat_id,
                pool,
                recipe_id,
                user_id,
                ResourcePermission::Edit,
            )
            .await?;
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:im:") {
        if let Some((recipe_id, user_id)) = parse_two_positive_ids(raw) {
            create_recipe_invite_ui(
                bot,
                chat_id,
                pool,
                recipe_id,
                user_id,
                ResourcePermission::Manage,
            )
            .await?;
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(raw) = data.strip_prefix("recipe:edit:rev:") {
        if let Some((recipe_id, user_id)) = parse_two_positive_ids(raw) {
            if ensure_recipe_manage_ui(bot, chat_id, pool, recipe_id).await? {
                match resource_permissions::revoke_permission(
                    pool,
                    RESOURCE_TYPE_RECIPE,
                    recipe_id,
                    user_id,
                )
                .await
                {
                    Ok(()) => {
                        bot.send_message(chat_id, "✅ Permesso revocato.").await?;
                        show_collaborators(bot, chat_id, pool, recipe_id).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
        } else {
            show_invalid_action(bot, chat_id).await?;
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:delete:ask:")
        .and_then(parse_positive_i64_str)
    {
        if ensure_recipe_owner_ui(bot, chat_id, pool, recipe_id).await? {
            bot.send_message(
                chat_id,
                "⚠️ Eliminare definitivamente questa ricetta?\n\nIngredienti, procedimento, media, condivisioni e permessi collegati verranno eliminati. Questa operazione non è reversibile.",
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![button(
                    "🗑 Elimina definitivamente",
                    format!("recipe:edit:delete:yes:{recipe_id}"),
                )],
                vec![
                    button("❌ Annulla", format!("recipe:edit:menu:{recipe_id}")),
                    button("🏠 Menù principale", "menu:main"),
                ],
            ]))
            .await?;
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:delete:yes:")
        .and_then(parse_positive_i64_str)
    {
        match delete_recipe_permanently(pool, recipe_id).await {
            Ok(paths) => {
                sessions.clear_chat(chat_id.0);
                cleanup_recipe_media_files(recipe_id, &paths).await;
                bot.send_message(chat_id, "✅ Ricetta eliminata definitivamente.")
                    .reply_markup(recipe_menu_keyboard())
                    .await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:archive:ask:")
        .and_then(parse_positive_i64_str)
    {
        if ensure_recipe_owner_ui(bot, chat_id, pool, recipe_id).await? {
            bot.send_message(chat_id, "⚠️ Archiviare questa ricetta? Non comparirà più negli elenchi normali, ma i dati restano nel database.")
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![button("🗄 Archivia", format!("recipe:edit:archive:yes:{recipe_id}"))],
                    vec![button("❌ Annulla", format!("recipe:detail:{recipe_id}")), button("🏠 Menù principale", "menu:main")],
                ]))
                .await?;
        }
        return Ok(());
    }
    if let Some(recipe_id) = data
        .strip_prefix("recipe:edit:archive:yes:")
        .and_then(parse_positive_i64_str)
    {
        match archive_recipe(pool, recipe_id).await {
            Ok(()) => {
                sessions.clear_chat(chat_id.0);
                bot.send_message(chat_id, "✅ Ricetta archiviata.")
                    .reply_markup(recipe_menu_keyboard())
                    .await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(());
    }

    show_invalid_action(bot, chat_id).await?;
    Ok(())
}

async fn handle_invite_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<()> {
    if data == "recipe:invite:list" {
        show_pending_recipe_invites(bot, chat_id, pool).await?;
        return Ok(());
    }
    if let Some(invite_id) = data
        .strip_prefix("recipe:invite:accept:")
        .and_then(parse_positive_i64_str)
    {
        let actor = identity::current_actor();
        let Some(user_id) = actor.utente_id else {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        };
        match resource_permissions::accept_invite(pool, invite_id, user_id).await {
            Ok(invite) => {
                bot.send_message(chat_id, "✅ Invito ricetta accettato.")
                    .await?;
                show_recipe_detail(bot, chat_id, pool, invite.resource_id).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(recipe_menu_keyboard())
                    .await?;
            }
        };
        return Ok(());
    }
    if let Some(invite_id) = data
        .strip_prefix("recipe:invite:decline:")
        .and_then(parse_positive_i64_str)
    {
        let actor = identity::current_actor();
        let Some(user_id) = actor.utente_id else {
            show_invalid_action(bot, chat_id).await?;
            return Ok(());
        };
        match resource_permissions::decline_invite(pool, invite_id, user_id).await {
            Ok(()) => {
                bot.send_message(chat_id, "✅ Invito ricetta rifiutato.")
                    .await?;
                show_pending_recipe_invites(bot, chat_id, pool).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(recipe_menu_keyboard())
                    .await?;
            }
        };
        return Ok(());
    }
    show_invalid_action(bot, chat_id).await?;
    Ok(())
}

async fn show_menu(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let pending = pending_invite_count(pool).await.unwrap_or(0);
    let mut rows = vec![
        vec![
            button("📋 Elenco ricette", "recipe:list:0"),
            button("➕ Nuova ricetta", "recipe:new"),
        ],
        vec![
            button("🔎 Cerca per nome", "recipe:search"),
            button("🥕 Cerca per ingredienti", "recipe:find"),
        ],
    ];
    if pending > 0 {
        rows.push(vec![button(
            format!("📨 Inviti ricette ({pending})"),
            "recipe:invite:list",
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", "food:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(
        chat_id,
        "🍳 Ricette\n\nCrea, cerca e consulta ricette. Il procedimento può essere letto tutto insieme oppure seguito uno step alla volta.",
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_recipe_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
) -> ResponseResult<()> {
    let page = page.max(0);
    let total = count_visible_recipes(pool).await.unwrap_or(0);
    let pages = page_count(total, RECIPE_LIST_PAGE_SIZE);
    let safe_page = if pages == 0 { 0 } else { page.min(pages - 1) };
    let rows = list_visible_recipes(pool, safe_page, RECIPE_LIST_PAGE_SIZE)
        .await
        .unwrap_or_default();

    // C10: il nome della schermata coincide con quello del pulsante che ci
    // porta. C1: il testo non ripete i nomi, che stanno sui pulsanti; ne'
    // ingredienti, step e porzioni, che stanno nel dettaglio della ricetta.
    // L'ordinamento e' alfabetico, quindi C6 non chiede di dichiararlo.
    let mut text = liste::intestazione("📋 Elenco ricette", total, safe_page);
    if rows.is_empty() {
        text.push_str("\n\nNessuna ricetta visibile.");
    } else if rows.iter().any(|recipe| recipe.owner || recipe.shared) {
        text.push_str("\n\n👤 tua · 👥 condivisa");
    }

    let mut keyboard = rows
        .iter()
        .map(|recipe| {
            let suffix = if recipe.owner {
                " 👤"
            } else if recipe.shared {
                " 👥"
            } else {
                ""
            };
            vec![button(
                format!("🍳 {}{}", recipe.name, suffix),
                format!("recipe:detail:{}", recipe.id),
            )]
        })
        .collect::<Vec<_>>();
    if let Some(riga) = liste::riga_paginazione(safe_page, total, "recipe:noop", |pagina| {
        format!("recipe:list:{pagina}")
    }) {
        keyboard.push(riga);
    }
    // C6: sopra le 20 voci la ricerca viene prima della creazione.
    keyboard.push(if liste::si_cerca_invece_di_sfogliare(total) {
        vec![
            button("🔎 Cerca", "recipe:search"),
            button("➕ Nuova ricetta", "recipe:new"),
        ]
    } else {
        vec![
            button("➕ Nuova ricetta", "recipe:new"),
            button("🔎 Cerca", "recipe:search"),
        ]
    });
    keyboard.push(vec![
        button("⬅️ Indietro", "recipe:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;
    Ok(())
}

async fn show_recipe_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    let Some(recipe) = visible_recipe(pool, recipe_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, "⚠️ Ricetta non disponibile.")
            .reply_markup(back_home_keyboard("recipe:menu"))
            .await?;
        return Ok(());
    };
    let ingredients = list_recipe_ingredients(pool, recipe_id)
        .await
        .unwrap_or_default();
    let steps = list_recipe_steps(pool, recipe_id).await.unwrap_or_default();
    let spaces = recipe_space_names(pool, recipe_id)
        .await
        .unwrap_or_default();
    let compat = recipe_food_compatibility(pool, recipe_id)
        .await
        .unwrap_or_default();
    let actor = identity::current_actor();
    let user_id = actor.utente_id.unwrap_or_default();
    let can_edit = can_edit_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false);
    let can_manage = can_manage_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false);
    let owner = recipe.owner_user_id == Some(user_id);

    let mut text = format!(
        "🍳 {}\n\n👥 Porzioni base: {}",
        recipe.name, recipe.servings
    );
    if recipe.global_catalog == 1 {
        text.push_str("\n🌐 Catalogo globale");
    } else if let Some(owner_name) = &recipe.owner_name {
        text.push_str(&format!("\n👤 Proprietario: {owner_name}"));
    }
    if spaces.is_empty() {
        text.push_str("\n🔒 Visibilità: solo proprietario");
    } else {
        text.push_str(&format!("\n👁 Visibile in: {}", spaces.join(", ")));
    }

    text.push_str("\n\n🥕 Ingredienti");
    if ingredients.is_empty() {
        text.push_str("\n— Nessun ingrediente");
    } else {
        for ingredient in &ingredients {
            let product = ingredient
                .product_label
                .as_ref()
                .map(|label| format!(" · 🛒 {label}"))
                .unwrap_or_default();
            let optional = if ingredient.optional == 1 {
                " · opzionale"
            } else {
                ""
            };
            let notes = ingredient
                .notes
                .as_ref()
                .map(|note| format!(" · {note}"))
                .unwrap_or_default();
            text.push_str(&format!(
                "\n• {} {} {}{}{}{}",
                display_quantity(ingredient.quantity),
                ingredient.unit_symbol,
                ingredient.food_name,
                product,
                optional,
                notes
            ));
        }
    }
    text.push_str(&format!("\n\n📝 Procedimento: {} step", steps.len()));

    if !compat.is_empty() {
        let highlights = compat
            .iter()
            .filter(|row| {
                matches!(
                    row.label_code.as_str(),
                    "vegano" | "vegetariano" | "senza_glutine" | "senza_lattosio"
                )
            })
            .take(4)
            .map(|row| {
                let details = match row.status.as_str() {
                    "no" if row.incompatible_ingredients > 0 => format!(
                        " · {}/{} ingredienti incompatibili",
                        row.incompatible_ingredients, row.total_ingredients
                    ),
                    "da_verificare" if row.ingredients_to_check > 0 => format!(
                        " · {}/{} ingredienti da verificare",
                        row.ingredients_to_check, row.total_ingredients
                    ),
                    _ => String::new(),
                };
                format!(
                    "{} {}: {}{}",
                    row.label_emoji,
                    row.label_name,
                    compatibility_icon(&row.status),
                    details
                )
            })
            .collect::<Vec<_>>();
        if !highlights.is_empty() {
            text.push_str("\n\n🧭 Compatibilità\n");
            text.push_str(&highlights.join("\n"));
        }
    }

    let mut keyboard = vec![vec![
        button(
            "📖 Procedimento completo",
            format!("recipe:complete:{recipe_id}"),
        ),
        button(
            "👨‍🍳 Procedura guidata",
            format!("recipe:guided:{recipe_id}:0"),
        ),
    ]];
    if can_edit {
        keyboard.push(vec![button(
            "✏️ Modifica ricetta",
            format!("recipe:edit:menu:{recipe_id}"),
        )]);
    } else if can_manage {
        keyboard.push(vec![button(
            "👥 Collaboratori",
            format!("recipe:edit:collaborators:{recipe_id}"),
        )]);
    }
    if owner {
        keyboard.push(vec![button(
            "🗄 Archivia",
            format!("recipe:edit:archive:ask:{recipe_id}"),
        )]);
    }
    keyboard.push(vec![
        button("⬅️ Indietro", "recipe:list:0"),
        button("🏠 Menù principale", "menu:main"),
    ]);

    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;
    Ok(())
}

async fn show_full_procedure(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    let Some(recipe) = visible_recipe(pool, recipe_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, "⚠️ Ricetta non disponibile.")
            .reply_markup(back_home_keyboard("recipe:menu"))
            .await?;
        return Ok(());
    };
    let steps = list_recipe_steps(pool, recipe_id).await.unwrap_or_default();
    let chunks = full_procedure_text_chunks(&recipe.name, &steps);
    let mut keyboard = steps
        .iter()
        .filter(|step| step.photo_count + step.video_count > 0)
        .map(|step| {
            vec![button(
                format!("📎 Media step {}", step.number),
                format!("recipe:step:media:{}", step.id),
            )]
        })
        .collect::<Vec<_>>();
    if !steps.is_empty() {
        keyboard.push(vec![button(
            "👨‍🍳 Avvia procedura guidata",
            format!("recipe:guided:{recipe_id}:0"),
        )]);
    }
    keyboard.push(vec![
        button("⬅️ Indietro", format!("recipe:detail:{recipe_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    let markup = InlineKeyboardMarkup::new(keyboard);

    // Un procedimento può superare il limite di un singolo messaggio
    // Telegram. In quel caso inviamo più blocchi testuali e mettiamo i
    // comandi solo sull'ultimo, senza perdere alcuno step.
    for (index, chunk) in chunks.iter().enumerate() {
        let request = bot.send_message(chat_id, chunk.clone());
        if index + 1 == chunks.len() {
            request.reply_markup(markup.clone()).await?;
        } else {
            request.await?;
        }
    }
    Ok(())
}

async fn show_guided_step(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
    page: i64,
) -> ResponseResult<()> {
    let Some(recipe) = visible_recipe(pool, recipe_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, "⚠️ Ricetta non disponibile.")
            .reply_markup(back_home_keyboard("recipe:menu"))
            .await?;
        return Ok(());
    };
    let steps = list_recipe_steps(pool, recipe_id).await.unwrap_or_default();
    if steps.is_empty() {
        bot.send_message(
            chat_id,
            "⚠️ Questa ricetta non ha ancora un procedimento guidato.",
        )
        .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
        .await?;
        return Ok(());
    }
    let page = page.clamp(0, steps.len() as i64 - 1);
    let step = &steps[page as usize];
    let media = media_summary(step.photo_count, step.video_count);
    let mut text = format!(
        "👨‍🍳 Procedura guidata\n{}\n\nStep {}/{}\n\n{}",
        recipe.name,
        page + 1,
        steps.len(),
        step.text
    );
    if !media.is_empty() {
        text.push_str(&format!("\n\n{media}"));
    }

    let mut keyboard = Vec::new();
    if step.photo_count + step.video_count > 0 {
        keyboard.push(vec![button(
            "📎 Vedi foto/video dello step",
            format!("recipe:step:media:{}", step.id),
        )]);
    }
    let mut nav = Vec::new();
    if page > 0 {
        nav.push(button(
            "⬅️ Step precedente",
            format!("recipe:guided:{recipe_id}:{}", page - 1),
        ));
    }
    nav.push(button(
        format!("{}/{}", page + 1, steps.len()),
        "recipe:noop",
    ));
    if page + 1 < steps.len() as i64 {
        nav.push(button(
            "Step successivo ➡️",
            format!("recipe:guided:{recipe_id}:{}", page + 1),
        ));
    } else {
        nav.push(button(
            "✅ Termina",
            format!("recipe:guided:finish:{recipe_id}"),
        ));
    }
    keyboard.push(nav);
    keyboard.push(vec![button(
        "📖 Procedimento completo",
        format!("recipe:complete:{recipe_id}"),
    )]);
    keyboard.push(vec![
        button("⬅️ Indietro", format!("recipe:detail:{recipe_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;
    Ok(())
}

async fn show_guided_finished(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    let Some(recipe) = visible_recipe(pool, recipe_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, "⚠️ Ricetta non disponibile.")
            .reply_markup(recipe_menu_keyboard())
            .await?;
        return Ok(());
    };
    bot.send_message(
        chat_id,
        format!(
            "✅ Ricetta terminata\n\nHai completato tutti gli step di {}.",
            recipe.name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button(
            "🔁 Riparti dallo Step 1",
            format!("recipe:guided:{recipe_id}:0"),
        )],
        vec![button(
            "📖 Procedimento completo",
            format!("recipe:complete:{recipe_id}"),
        )],
        vec![button(
            "📄 Dettaglio ricetta",
            format!("recipe:detail:{recipe_id}"),
        )],
        vec![button("🏠 Menù principale", "menu:main")],
    ]))
    .await?;
    Ok(())
}

async fn show_step_media(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    step_id: i64,
) -> ResponseResult<()> {
    let Some((recipe_id, step_number)) = visible_step_context(pool, step_id).await.unwrap_or(None)
    else {
        bot.send_message(chat_id, "⚠️ Step non disponibile.")
            .reply_markup(recipe_menu_keyboard())
            .await?;
        return Ok(());
    };
    let media = list_step_media(pool, step_id).await.unwrap_or_default();
    let mut keyboard = media
        .iter()
        .map(|item| {
            let icon = if item.kind == "foto" { "📷" } else { "🎥" };
            vec![button(
                format!(
                    "{icon} {}",
                    item.caption
                        .clone()
                        .unwrap_or_else(|| format!("Allegato #{}", item.id))
                ),
                format!("recipe:media:item:{}", item.id),
            )]
        })
        .collect::<Vec<_>>();
    keyboard.push(vec![
        button(
            "⬅️ Indietro",
            format!("recipe:guided:{recipe_id}:{}", step_number - 1),
        ),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(
        chat_id,
        format!(
            "📎 Media · Step {step_number}\n\n{} allegati disponibili.",
            media.len()
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(keyboard))
    .await?;
    Ok(())
}

async fn show_media_item(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    media_id: i64,
) -> ResponseResult<()> {
    let Some((media, recipe_id, step_number)) = visible_media(pool, media_id).await.unwrap_or(None)
    else {
        bot.send_message(chat_id, "⚠️ Allegato non disponibile.")
            .reply_markup(recipe_menu_keyboard())
            .await?;
        return Ok(());
    };
    let path = PathBuf::from(&media.path);
    let caption = media
        .caption
        .clone()
        .unwrap_or_else(|| format!("Step {step_number}"));
    if path.exists() {
        if media.kind == "foto" {
            bot.send_photo(chat_id, InputFile::file(path))
                .caption(caption)
                .await?;
        } else {
            bot.send_video(chat_id, InputFile::file(path))
                .caption(caption)
                .await?;
        }
    } else {
        bot.send_message(chat_id, "⚠️ File allegato non trovato sul dispositivo.")
            .await?;
    }
    bot.send_message(chat_id, "📎 Allegato step")
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button(
                "⬅️ Indietro",
                format!("recipe:guided:{recipe_id}:{}", step_number - 1),
            ),
            button("🏠 Menù principale", "menu:main"),
        ]]))
        .await?;
    Ok(())
}

async fn show_recipe_name_search(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    query: &str,
) -> ResponseResult<()> {
    let rows = search_recipes_by_name(pool, query, RECIPE_SEARCH_LIMIT)
        .await
        .unwrap_or_default();
    let mut text = format!(
        "🔎 Risultati per: \"{query}\" · {}",
        result_label(rows.len() as i64)
    );
    if rows.is_empty() {
        text.push_str("\n\nNessuna ricetta trovata.");
    }
    let mut keyboard = rows
        .iter()
        .map(|recipe| {
            vec![button(
                format!("🍳 {}", recipe.name),
                format!("recipe:detail:{}", recipe.id),
            )]
        })
        .collect::<Vec<_>>();
    keyboard.push(vec![button("🔎 Nuova ricerca", "recipe:search")]);
    keyboard.push(vec![
        button("⬅️ Indietro", "recipe:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;
    Ok(())
}

async fn show_recipe_food_categories(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    requested_page: i64,
    selected: &[FoodChoice],
    current_filter: Option<&CategoryChoice>,
) -> ResponseResult<()> {
    let categories = list_recipe_food_categories(pool).await.unwrap_or_default();
    let total = categories.len() as i64;
    let pages = ((total + RECIPE_LIST_PAGE_SIZE - 1) / RECIPE_LIST_PAGE_SIZE).max(1);
    let page = requested_page.clamp(0, pages - 1);
    let start = (page * RECIPE_LIST_PAGE_SIZE) as usize;
    let end = (start + RECIPE_LIST_PAGE_SIZE as usize).min(categories.len());
    let page_categories = &categories[start..end];
    let mut rows = page_categories
        .iter()
        .map(|category| {
            let selected_mark = if current_filter.is_some_and(|value| value.id == category.id) {
                " ✅"
            } else {
                ""
            };
            vec![button(
                format!("{} {}{}", category.emoji, category.name, selected_mark),
                format!("recipe:find:filter:{}", category.id),
            )]
        })
        .collect::<Vec<_>>();
    if pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(button(
                "⬅️ Pagina precedente",
                format!("recipe:find:categories:{}", page - 1),
            ));
        }
        nav.push(button(format!("{}/{}", page + 1, pages), "recipe:noop"));
        if page + 1 < pages {
            nav.push(button(
                "Pagina successiva ➡️",
                format!("recipe:find:categories:{}", page + 1),
            ));
        }
        rows.push(nav);
    }
    if current_filter.is_some() {
        rows.push(vec![button(
            "🧹 Rimuovi filtro categoria",
            "recipe:find:filter:clear",
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", "recipe:find:return"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    let filter_line = current_filter
        .map(|category| format!("Filtro attuale: {} {}", category.emoji, category.name))
        .unwrap_or_else(|| "Nessun filtro categoria attivo.".to_string());
    bot.send_message(
        chat_id,
        format!(
            "🏷 Filtra ingredienti per categoria\n\n{filter_line}\n{}\nCategorie: {} · Pagina {}/{}\n\nLa categoria filtra gli alimenti restituiti quando scrivi il nome: non aggiunge direttamente un ingrediente.",
            if selected.is_empty() {
                "Nessun ingrediente selezionato.".to_string()
            } else {
                format!("{} ingredienti già selezionati.", selected.len())
            },
            total,
            page + 1,
            pages
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_ingredient_finder(
    bot: &Bot,
    chat_id: ChatId,
    selected: &[FoodChoice],
    category_filter: Option<&CategoryChoice>,
) -> ResponseResult<()> {
    let mut text = "🥕 Cerca ricette per ingredienti\n\nDigita direttamente in chat il nome di un alimento. Se vuoi restringere gli alimenti trovati, usa 🏷 Filtra categoria.\n\nLa ricerca ricette usa OR: basta che una ricetta contenga almeno uno degli ingredienti scelti. I risultati vengono ordinati per numero di corrispondenze.".to_string();
    if let Some(category) = category_filter {
        text.push_str(&format!(
            "\n\n🏷 Filtro categoria attivo: {} {}",
            category.emoji, category.name
        ));
    }
    if selected.is_empty() {
        text.push_str("\n\nNessun ingrediente selezionato.");
    } else {
        text.push_str("\n\nSelezionati:");
        for food in selected {
            text.push_str(&format!("\n• {}", food.name));
        }
    }
    bot.send_message(chat_id, text)
        .reply_markup(ingredient_finder_keyboard(selected, category_filter))
        .await?;
    Ok(())
}

async fn show_ingredient_search_results(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    selected: &[FoodChoice],
) -> ResponseResult<()> {
    let ids = selected.iter().map(|food| food.id).collect::<Vec<_>>();
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    let rows = search_by_ingredients(
        pool,
        &ids,
        user_id,
        actor.spazio_id,
        actor.view_all,
        RECIPE_SEARCH_LIMIT,
    )
    .await
    .unwrap_or_default();
    let selected_names = selected
        .iter()
        .map(|food| food.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let mut text = format!(
        "🥕 Ricerca per ingredienti\n{selected_names}\n\n{}",
        result_label(rows.len() as i64)
    );
    for row in &rows {
        text.push_str(&format!(
            "\n\n🍳 {}\n✅ {}/{} ingredienti selezionati presenti\n🥕 {} ingredienti totali nella ricetta",
            row.recipe_name,
            row.matched_ingredients,
            selected.len(),
            row.total_ingredients
        ));
    }
    let mut keyboard = rows
        .iter()
        .map(|row| {
            vec![button(
                format!(
                    "🍳 {} · {}/{}",
                    row.recipe_name,
                    row.matched_ingredients,
                    selected.len()
                ),
                format!("recipe:detail:{}", row.recipe_id),
            )]
        })
        .collect::<Vec<_>>();
    keyboard.push(vec![button("➕ Modifica ingredienti", "recipe:find")]);
    keyboard.push(vec![
        button("⬅️ Indietro", "recipe:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;
    Ok(())
}

async fn show_edit_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    let Some(recipe) = visible_recipe(pool, recipe_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, "⚠️ Ricetta non disponibile.")
            .reply_markup(recipe_menu_keyboard())
            .await?;
        return Ok(());
    };
    let can_edit = can_edit_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false);
    let can_manage = can_manage_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false);
    if !can_edit && !can_manage {
        bot.send_message(
            chat_id,
            "🔒 Non hai il permesso di modificare questa ricetta.",
        )
        .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
        .await?;
        return Ok(());
    }
    let mut rows = Vec::new();
    if can_edit {
        rows.push(vec![
            button("🏷 Nome", format!("recipe:edit:name:{recipe_id}")),
            button("👥 Porzioni", format!("recipe:edit:servings:{recipe_id}")),
        ]);
        rows.push(vec![
            button(
                "🥕 Ingredienti",
                format!("recipe:edit:ingredients:{recipe_id}"),
            ),
            button("📝 Procedimento", format!("recipe:edit:steps:{recipe_id}")),
        ]);
    }
    if can_manage {
        rows.push(vec![
            button(
                "👁 Visibilità",
                format!("recipe:edit:visibility:{recipe_id}"),
            ),
            button(
                "👥 Collaboratori",
                format!("recipe:edit:collaborators:{recipe_id}"),
            ),
        ]);
    }
    if recipe.owner_user_id == Some(user_id) {
        rows.push(vec![
            button("🗄 Archivia", format!("recipe:edit:archive:ask:{recipe_id}")),
            button("🗑 Elimina", format!("recipe:edit:delete:ask:{recipe_id}")),
        ]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("recipe:detail:{recipe_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, format!("✏️ Modifica ricetta\n{}", recipe.name))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_manage_ingredients(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    if !can_edit_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false)
    {
        bot.send_message(
            chat_id,
            "🔒 Non hai il permesso di modificare gli ingredienti.",
        )
        .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
        .await?;
        return Ok(());
    }
    let ingredients = list_recipe_ingredients(pool, recipe_id)
        .await
        .unwrap_or_default();
    let mut text = format!(
        "🥕 Ingredienti ricetta\n\n{}",
        result_label(ingredients.len() as i64)
    );
    for ingredient in &ingredients {
        let product = ingredient
            .product_label
            .as_ref()
            .map(|label| format!(" · 🛒 {label}"))
            .unwrap_or_default();
        text.push_str(&format!(
            "\n• {} {} {}{}",
            display_quantity(ingredient.quantity),
            ingredient.unit_symbol,
            ingredient.food_name,
            product
        ));
    }
    let mut rows = ingredients
        .iter()
        .map(|ingredient| {
            vec![button(
                format!("🗑 {}", ingredient.food_name),
                format!("recipe:edit:irem:{recipe_id}:{}", ingredient.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        "➕ Aggiungi ingrediente",
        format!("recipe:edit:ingredient:add:{recipe_id}"),
    )]);
    rows.push(vec![
        button("⬅️ Indietro", format!("recipe:edit:menu:{recipe_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_manage_steps(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    if !can_edit_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false)
    {
        bot.send_message(
            chat_id,
            "🔒 Non hai il permesso di modificare il procedimento.",
        )
        .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
        .await?;
        return Ok(());
    }
    let steps = list_recipe_steps(pool, recipe_id).await.unwrap_or_default();
    let mut text = format!("📝 Procedimento · {} step", steps.len());
    for step in &steps {
        let preview = truncate_chars(&step.text, 80);
        let media = media_summary(step.photo_count, step.video_count);
        text.push_str(&format!("\n\n{}. {}", step.number, preview));
        if !media.is_empty() {
            text.push_str(&format!("\n{media}"));
        }
    }
    let mut rows = steps
        .iter()
        .map(|step| {
            vec![button(
                format!("📝 Gestisci step {}", step.number),
                format!("recipe:edit:step:{recipe_id}:{}", step.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        "➕ Aggiungi step",
        format!("recipe:edit:step:add:{recipe_id}"),
    )]);
    rows.push(vec![
        button(
            "📖 Anteprima completa",
            format!("recipe:complete:{recipe_id}"),
        ),
        button("👨‍🍳 Prova guidata", format!("recipe:guided:{recipe_id}:0")),
    ]);
    rows.push(vec![
        button("⬅️ Indietro", format!("recipe:edit:menu:{recipe_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_step_manage(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
    step_id: i64,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    if !can_edit_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false)
    {
        bot.send_message(chat_id, "🔒 Non hai il permesso di modificare questo step.")
            .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
            .await?;
        return Ok(());
    }
    let Some(step) = step_for_recipe(pool, recipe_id, step_id)
        .await
        .unwrap_or(None)
    else {
        bot.send_message(chat_id, "⚠️ Step non disponibile.")
            .reply_markup(back_home_keyboard(&format!(
                "recipe:edit:steps:{recipe_id}"
            )))
            .await?;
        return Ok(());
    };
    let media = list_step_media(pool, step_id).await.unwrap_or_default();
    let mut text = format!("📝 Step {}\n\n{}", step.number, step.text);
    if media.is_empty() {
        text.push_str("\n\n📎 Nessun allegato.");
    } else {
        text.push_str("\n\n📎 Allegati:");
        for item in &media {
            let icon = if item.kind == "foto" { "📷" } else { "🎥" };
            text.push_str(&format!(
                "\n{icon} {}",
                item.caption
                    .clone()
                    .unwrap_or_else(|| format!("Allegato #{}", item.id))
            ));
        }
    }
    let mut rows = vec![
        vec![button(
            "✏️ Modifica testo",
            format!("recipe:edit:step:text:{recipe_id}:{step_id}"),
        )],
        vec![
            button(
                "📷 Aggiungi foto",
                format!("recipe:edit:step:photo:{recipe_id}:{step_id}"),
            ),
            button(
                "🎥 Aggiungi video",
                format!("recipe:edit:step:video:{recipe_id}:{step_id}"),
            ),
        ],
        vec![
            button(
                "⬆️ Sposta su",
                format!("recipe:edit:step:up:{recipe_id}:{step_id}"),
            ),
            button(
                "⬇️ Sposta giù",
                format!("recipe:edit:step:down:{recipe_id}:{step_id}"),
            ),
        ],
    ];
    for item in &media {
        let icon = if item.kind == "foto" { "📷" } else { "🎥" };
        rows.push(vec![
            button(
                format!("{icon} Apri"),
                format!("recipe:media:item:{}", item.id),
            ),
            button(
                "🗑 Rimuovi",
                format!("recipe:edit:md:{recipe_id}:{}", item.id),
            ),
        ]);
    }
    rows.push(vec![button(
        "🗑 Elimina step",
        format!("recipe:edit:step:delete:{recipe_id}:{step_id}"),
    )]);
    rows.push(vec![
        button("⬅️ Indietro", format!("recipe:edit:steps:{recipe_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_edit_visibility(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    if !ensure_recipe_manage_ui(bot, chat_id, pool, recipe_id).await? {
        return Ok(());
    }
    let spaces = recipe_space_names(pool, recipe_id)
        .await
        .unwrap_or_default();
    let current = if spaces.is_empty() {
        "🔒 Solo proprietario".to_string()
    } else {
        format!("👁 {}", spaces.join(", "))
    };
    bot.send_message(
        chat_id,
        format!("👁 Visibilità ricetta\n\nAttuale: {current}"),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button(
            "🔒 Solo mio",
            format!("recipe:edit:vis:private:{recipe_id}"),
        )],
        vec![button(
            "🎯 Spazio predefinito",
            format!("recipe:edit:vis:default:{recipe_id}"),
        )],
        vec![button(
            "🌐 Tutti i miei spazi",
            format!("recipe:edit:vis:all:{recipe_id}"),
        )],
        vec![button(
            "🎛 Scegli spazi",
            format!("recipe:edit:vis:choose:{recipe_id}"),
        )],
        vec![
            button("⬅️ Indietro", format!("recipe:edit:menu:{recipe_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ]))
    .await?;
    Ok(())
}

async fn show_collaborators(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<()> {
    if !ensure_recipe_manage_ui(bot, chat_id, pool, recipe_id).await? {
        return Ok(());
    }
    let current = list_recipe_permissions(pool, recipe_id)
        .await
        .unwrap_or_default();
    let eligible = eligible_collaborators(pool, recipe_id)
        .await
        .unwrap_or_default();
    let mut text = "👥 Collaboratori ricetta\n\nLa sola visibilità in uno spazio non concede modifica. I permessi sono espliciti.".to_string();
    if current.is_empty() {
        text.push_str("\n\nNessun collaboratore con permessi attivi.");
    } else {
        text.push_str("\n\nPermessi attivi:");
        for (user_id, name, can_edit, can_manage) in &current {
            let level = if *can_manage == 1 {
                "modifica + gestione"
            } else if *can_edit == 1 {
                "modifica"
            } else {
                "lettura"
            };
            text.push_str(&format!("\n• {name}: {level}"));
            let _ = user_id;
        }
    }
    let mut rows = current
        .iter()
        .map(|(user_id, name, _, _)| {
            vec![button(
                format!("🗑 Revoca · {name}"),
                format!("recipe:edit:rev:{recipe_id}:{user_id}"),
            )]
        })
        .collect::<Vec<_>>();
    for (user_id, name) in eligible {
        if !current
            .iter()
            .any(|(current_id, _, _, _)| *current_id == user_id)
        {
            rows.push(vec![button(
                format!("➕ {name}"),
                format!("recipe:edit:iu:{recipe_id}:{user_id}"),
            )]);
        }
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("recipe:edit:menu:{recipe_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_invite_permission_choice(
    bot: &Bot,
    chat_id: ChatId,
    recipe_id: i64,
    user_id: i64,
) -> ResponseResult<()> {
    bot.send_message(chat_id, "👥 Quale permesso vuoi proporre al collaboratore?")
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![button(
                "✏️ Può modificare",
                format!("recipe:edit:ie:{recipe_id}:{user_id}"),
            )],
            vec![button(
                "🛠 Modifica + gestisce permessi",
                format!("recipe:edit:im:{recipe_id}:{user_id}"),
            )],
            vec![
                button(
                    "⬅️ Indietro",
                    format!("recipe:edit:collaborators:{recipe_id}"),
                ),
                button("🏠 Menù principale", "menu:main"),
            ],
        ]))
        .await?;
    Ok(())
}

async fn create_recipe_invite_ui(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
    invited_user_id: i64,
    permission: ResourcePermission,
) -> ResponseResult<()> {
    if !ensure_recipe_manage_ui(bot, chat_id, pool, recipe_id).await? {
        return Ok(());
    }
    let actor = identity::current_actor();
    let Some(created_by) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    match resource_permissions::create_invite(
        pool,
        RESOURCE_TYPE_RECIPE,
        recipe_id,
        invited_user_id,
        created_by,
        permission,
    )
    .await
    {
        Ok(_) => {
            bot.send_message(
                chat_id,
                "✅ Invito creato. Il destinatario potrà accettarlo dalla sezione Ricette.",
            )
            .await?;
            show_collaborators(bot, chat_id, pool, recipe_id).await?;
        }
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(back_home_keyboard(&format!(
                    "recipe:edit:collaborators:{recipe_id}"
                )))
                .await?;
        }
    }
    Ok(())
}

async fn show_pending_recipe_invites(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    let rows = sqlx::query_as::<_, (i64, String, String, i64, i64)>(
        "SELECT i.id, r.nome, u.nome_visualizzato, i.puo_modificare, i.puo_gestire_permessi \
         FROM inviti_risorsa i \
         JOIN ricette r ON r.id = i.risorsa_id \
         JOIN utenti u ON u.id = i.creato_da_utente_id \
         WHERE i.tipo_risorsa = 'ricetta' AND i.invitato_utente_id = ? AND i.stato = 'pendente' \
         ORDER BY i.creato_il, i.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut text = format!("📨 Inviti ricette · {}", result_label(rows.len() as i64));
    let mut keyboard = Vec::new();
    for (invite_id, recipe_name, author, can_edit, can_manage) in rows {
        let level = if can_manage == 1 {
            "modifica + gestione"
        } else if can_edit == 1 {
            "modifica"
        } else {
            "lettura"
        };
        text.push_str(&format!(
            "\n\n🍳 {recipe_name}\nDa: {author}\nPermesso: {level}"
        ));
        keyboard.push(vec![
            button("✅ Accetta", format!("recipe:invite:accept:{invite_id}")),
            button("❌ Rifiuta", format!("recipe:invite:decline:{invite_id}")),
        ]);
    }
    keyboard.push(vec![
        button("⬅️ Indietro", "recipe:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;
    Ok(())
}

async fn show_draft_ingredients(
    bot: &Bot,
    chat_id: ChatId,
    draft: &RecipeDraft,
) -> ResponseResult<()> {
    let mut text = format!(
        "🥕 Ingredienti · {}\n👥 {} porzioni",
        draft.name, draft.servings
    );
    if draft.ingredients.is_empty() {
        text.push_str("\n\nNessun ingrediente aggiunto.");
    } else {
        for ingredient in &draft.ingredients {
            let product = ingredient
                .product_label
                .as_ref()
                .map(|label| format!(" · 🛒 {label}"))
                .unwrap_or_default();
            text.push_str(&format!(
                "\n\n• {} {} {}{}",
                display_quantity(ingredient.quantity),
                ingredient.unit_symbol,
                ingredient.food_name,
                product
            ));
        }
    }
    bot.send_message(chat_id, text)
        .reply_markup(draft_ingredients_keyboard(draft))
        .await?;
    Ok(())
}

async fn show_step_media_menu(
    bot: &Bot,
    chat_id: ChatId,
    draft: &RecipeDraft,
    step_index: usize,
) -> ResponseResult<()> {
    let Some(step) = draft.steps.get(step_index) else {
        show_expired_flow(bot, chat_id).await?;
        return Ok(());
    };
    let photos = step
        .media
        .iter()
        .filter(|media| media.kind == "foto")
        .count();
    let videos = step
        .media
        .iter()
        .filter(|media| media.kind == "video")
        .count();
    bot.send_message(
        chat_id,
        format!(
            "📝 Step {}\n\n{}\n\n📎 Allegati: 📷 {} · 🎥 {}\n\nFoto e video sono facoltativi e appartengono a questo specifico step.",
            step_index + 1,
            step.text,
            photos,
            videos
        ),
    )
    .reply_markup(step_media_keyboard())
    .await?;
    Ok(())
}

async fn show_after_step(bot: &Bot, chat_id: ChatId, draft: &RecipeDraft) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "✅ Step {} completato.\n\nVuoi aggiungerne un altro oppure terminare il procedimento?",
            draft.steps.len()
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![
            button("➕ Aggiungi step", "recipe:new:step:add"),
            button("✅ Fine procedimento", "recipe:new:steps:done"),
        ],
        vec![
            button("❌ Annulla", "recipe:new:cancel"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ]))
    .await?;
    Ok(())
}

async fn show_visibility_choice(
    bot: &Bot,
    chat_id: ChatId,
    draft: &RecipeDraft,
) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!("👁 Visibilità ricetta\n{}\n\nScegli dove rendere visibile la ricetta. La proprietà resta sempre separata dalla visibilità.", draft.name),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button("🔒 Solo mio", "recipe:new:visibility:private")],
        vec![button("🎯 Spazio predefinito", "recipe:new:visibility:default")],
        vec![button("🌐 Tutti i miei spazi", "recipe:new:visibility:all")],
        vec![button("🎛 Scegli spazi", "recipe:new:visibility:choose")],
        vec![button("⬅️ Indietro", "recipe:new:steps:done"), button("❌ Annulla", "recipe:new:cancel"), button("🏠 Menù principale", "menu:main")],
    ]))
    .await?;
    Ok(())
}

async fn show_recipe_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    draft: &RecipeDraft,
) -> ResponseResult<()> {
    let product_specific = draft
        .ingredients
        .iter()
        .filter(|ingredient| ingredient.product_id.is_some())
        .count();
    let media_count = draft
        .steps
        .iter()
        .map(|step| step.media.len())
        .sum::<usize>();
    let visibility = if draft.visible_spaces.is_empty() {
        "🔒 Solo proprietario".to_string()
    } else {
        format!("👁 {} spazio/i", draft.visible_spaces.len())
    };
    bot.send_message(
        chat_id,
        format!(
            "✅ Riepilogo ricetta\n\n🍳 {}\n👥 {} porzioni\n🥕 {} ingredienti ({} prodotti specifici)\n📝 {} step\n📎 {} foto/video\n{}\n\nIl formato di vendita NON viene salvato nella ricetta: sarà scelto dalla futura Lista spesa.",
            draft.name,
            draft.servings,
            draft.ingredients.len(),
            product_specific,
            draft.steps.len(),
            media_count,
            visibility
        ),
    )
    .reply_markup(recipe_confirmation_keyboard())
    .await?;
    Ok(())
}

async fn show_space_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    selected: &[i64],
    toggle_prefix: &str,
    done_callback: &str,
    back_callback: &str,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        show_invalid_action(bot, chat_id).await?;
        return Ok(());
    };
    let spaces = identity::list_user_spaces(pool, user_id)
        .await
        .unwrap_or_default();
    let mut rows = Vec::new();
    for space in spaces.into_iter().filter(|space| {
        matches!(
            space.ruolo.as_str(),
            "proprietario" | "amministratore" | "membro"
        )
    }) {
        let mark = if selected.contains(&space.id) {
            "✅"
        } else {
            "⬜"
        };
        rows.push(vec![button(
            format!("{mark} {}", space.nome),
            format!("{toggle_prefix}:{}", space.id),
        )]);
    }
    rows.push(vec![button("✅ Conferma spazi", done_callback)]);
    rows.push(vec![
        button("⬅️ Indietro", back_callback),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(
        chat_id,
        "🎛 Scegli gli spazi\n\nPuoi selezionarne più di uno.",
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_food_search_results(
    bot: &Bot,
    chat_id: ChatId,
    query: &str,
    foods: &[FoodChoice],
    callback_prefix: &str,
    back_callback: &str,
) -> ResponseResult<()> {
    let mut text = format!(
        "🔎 Alimenti per: \"{query}\" · {}",
        result_label(foods.len() as i64)
    );
    if foods.is_empty() {
        text.push_str("\n\nNessun alimento trovato.");
    }
    let mut rows = foods
        .iter()
        .map(|food| {
            vec![button(
                food.name.clone(),
                format!("{callback_prefix}:{}", food.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![
        button("⬅️ Indietro", back_callback),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, text)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn show_product_choice(
    bot: &Bot,
    chat_id: ChatId,
    food: &FoodChoice,
    products: &[ProductChoice],
    generic_prefix: &str,
    product_prefix: &str,
    back_callback: &str,
) -> ResponseResult<()> {
    let mut rows = vec![vec![button(
        "🌐 Usa alimento generico",
        format!("{generic_prefix}:{}", food.id),
    )]];
    for product in products {
        rows.push(vec![button(
            format!("🛒 {} · {}", product.brand, product.product_name),
            format!("{product_prefix}:{}", product.product_id),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", back_callback),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(
        chat_id,
        format!("🥕 {}\n\nSono disponibili prodotti commerciali. Vuoi usare l'alimento generico oppure un prodotto specifico?\n\nIl formato della confezione non viene scelto qui.", food.name),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn preferred_ingredient_unit(pool: &SqlitePool, food: &FoodChoice) -> Result<UnitChoice> {
    if let Some(unit_id) = food.default_unit_id {
        if let Some(unit) = unit_by_id(pool, unit_id).await? {
            return Ok(unit);
        }
    }
    list_units(pool)
        .await?
        .into_iter()
        .next()
        .context("Nessuna unità di misura disponibile")
}

async fn begin_new_ingredient_quantity(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &RecipeSessionStore,
    draft: RecipeDraft,
    food: FoodChoice,
    product: Option<ProductChoice>,
) -> ResponseResult<()> {
    let unit = match preferred_ingredient_unit(pool, &food).await {
        Ok(unit) => unit,
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(flow_keyboard("recipe:new:ingredients"))
                .await?;
            return Ok(());
        }
    };
    sessions.set(
        chat_id.0,
        RecipeConversationState::IngredientQuantityReady {
            draft,
            food: food.clone(),
            product: product.clone(),
            unit: unit.clone(),
        },
    );
    ask_ingredient_quantity_ready(
        bot,
        chat_id,
        &food,
        product.as_ref(),
        &unit,
        "recipe:new:quantity:changeunit",
        "recipe:new:ingredients",
    )
    .await
}

async fn begin_edit_ingredient_quantity(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &RecipeSessionStore,
    recipe_id: i64,
    food: FoodChoice,
    product: Option<ProductChoice>,
) -> ResponseResult<()> {
    let unit = match preferred_ingredient_unit(pool, &food).await {
        Ok(unit) => unit,
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(back_home_keyboard(&format!(
                    "recipe:edit:ingredients:{recipe_id}"
                )))
                .await?;
            return Ok(());
        }
    };
    sessions.set(
        chat_id.0,
        RecipeConversationState::EditIngredientQuantityReady {
            recipe_id,
            food: food.clone(),
            product: product.clone(),
            unit: unit.clone(),
        },
    );
    ask_ingredient_quantity_ready(
        bot,
        chat_id,
        &food,
        product.as_ref(),
        &unit,
        &format!("recipe:edit:quantity:changeunit:{recipe_id}"),
        &format!("recipe:edit:ingredients:{recipe_id}"),
    )
    .await
}

async fn ask_ingredient_quantity_ready(
    bot: &Bot,
    chat_id: ChatId,
    food: &FoodChoice,
    product: Option<&ProductChoice>,
    unit: &UnitChoice,
    change_unit_callback: &str,
    back_callback: &str,
) -> ResponseResult<()> {
    let product_text = product
        .map(|value| format!("\n🛒 {} · {}", value.brand, value.product_name))
        .unwrap_or_default();
    bot.send_message(
        chat_id,
        format!(
            "⚖️ Quantità ingrediente\n\n🥕 {}{}\n📏 Unità: {} ({})\n\nScrivi la quantità necessaria oppure cambia unità prima di inserirla.",
            food.name,
            product_text,
            plural_unit_name(&unit.name, &unit.symbol),
            unit.symbol
        ),
    )
    .reply_markup(ingredient_quantity_keyboard(change_unit_callback, back_callback))
    .await?;
    Ok(())
}

async fn show_unit_choice(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food: &FoodChoice,
    callback_prefix: &str,
    back_callback: &str,
) -> ResponseResult<()> {
    let units = list_units(pool).await.unwrap_or_default();
    let mut rows = Vec::new();
    for unit in units {
        let suggested = if food.default_unit_id == Some(unit.id) {
            " ✅"
        } else {
            ""
        };
        rows.push(vec![button(
            format!(
                "{} {} ({}){}",
                unit_icon(&unit.symbol),
                plural_unit_name(&unit.name, &unit.symbol),
                unit.symbol,
                suggested
            ),
            format!("{callback_prefix}:{}", unit.id),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", back_callback),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(
        chat_id,
        format!(
            "📏 Unità di misura\n🥕 {}\n\nScegli l'unità usata nella ricetta.",
            food.name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn save_recipe(pool: &SqlitePool, chat_id: i64, draft: &RecipeDraft) -> Result<i64> {
    validate_draft(draft)?;
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Ricetta non disponibile per un attore di sistema")?;

    for space_id in &draft.visible_spaces {
        identity::ensure_can_write_space(pool, *space_id).await?;
    }
    if personal_recipe_name_exists(pool, user_id, &normalize_name(&draft.name), None).await? {
        bail!("Hai già una ricetta attiva con questo nome");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la creazione ricetta")?;
    let result = sqlx::query(
        "INSERT INTO ricette (proprietario_utente_id, nome, nome_normalizzato, porzioni_base, catalogo_globale) \
         VALUES (?, ?, ?, ?, 0)",
    )
    .bind(user_id)
    .bind(&draft.name)
    .bind(normalize_name(&draft.name))
    .bind(draft.servings)
    .execute(&mut *tx)
    .await
    .context("Impossibile creare la ricetta")?;
    let recipe_id = result.last_insert_rowid();

    let mut unique_spaces = draft.visible_spaces.clone();
    unique_spaces.sort_unstable();
    unique_spaces.dedup();
    for space_id in unique_spaces {
        sqlx::query(
            "INSERT INTO ricetta_spazi (ricetta_id, spazio_id, condivisa_da_utente_id) VALUES (?, ?, ?)",
        )
        .bind(recipe_id)
        .bind(space_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile salvare la visibilità della ricetta")?;
    }

    for (index, ingredient) in draft.ingredients.iter().enumerate() {
        sqlx::query(
            "INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, prodotto_alimentare_id, quantita, unita_misura_id, ordinamento) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(recipe_id)
        .bind(ingredient.food_id)
        .bind(ingredient.product_id)
        .bind(ingredient.quantity)
        .bind(ingredient.unit_id)
        .bind(index as i64 + 1)
        .execute(&mut *tx)
        .await
        .context("Impossibile salvare un ingrediente della ricetta")?;
    }

    for (step_index, step) in draft.steps.iter().enumerate() {
        let result =
            sqlx::query("INSERT INTO ricetta_step (ricetta_id, numero, testo) VALUES (?, ?, ?)")
                .bind(recipe_id)
                .bind(step_index as i64 + 1)
                .bind(&step.text)
                .execute(&mut *tx)
                .await
                .context("Impossibile salvare uno step della ricetta")?;
        let step_id = result.last_insert_rowid();
        for (media_index, media) in step.media.iter().enumerate() {
            sqlx::query(
                "INSERT INTO ricetta_step_media (ricetta_step_id, tipo_media, percorso_file, descrizione, ordinamento) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(step_id)
            .bind(&media.kind)
            .bind(&media.temp_path)
            .bind(&media.caption)
            .bind(media_index as i64 + 1)
            .execute(&mut *tx)
            .await
            .context("Impossibile registrare un allegato dello step")?;
        }
    }

    tx.commit()
        .await
        .context("Impossibile completare la creazione ricetta")?;

    tracing::info!(
        recipe_id,
        chat_id,
        "Ricetta salvata con procedimento guidato"
    );
    Ok(recipe_id)
}

fn validate_draft(draft: &RecipeDraft) -> Result<()> {
    if clean_text(&draft.name, RECIPE_NAME_MAX).is_none() {
        bail!("Nome ricetta non valido");
    }
    if draft.servings <= 0 {
        bail!("Le porzioni devono essere maggiori di zero");
    }
    if draft.ingredients.is_empty() {
        bail!("La ricetta deve avere almeno un ingrediente");
    }
    if draft.steps.is_empty() {
        bail!("La ricetta deve avere almeno uno step");
    }
    let mut food_ids = HashSet::new();
    for ingredient in &draft.ingredients {
        if ingredient.food_id <= 0 || ingredient.quantity <= 0.0 || ingredient.unit_id <= 0 {
            bail!("Ingrediente non valido");
        }
        if !food_ids.insert(ingredient.food_id) {
            bail!("Lo stesso alimento non può comparire due volte nella ricetta");
        }
    }
    for step in &draft.steps {
        if clean_text(&step.text, STEP_TEXT_MAX).is_none() {
            bail!("Uno step contiene testo non valido");
        }
        if step
            .media
            .iter()
            .any(|media| !matches!(media.kind.as_str(), "foto" | "video"))
        {
            bail!("Tipo media step non valido");
        }
    }
    Ok(())
}

async fn finalize_draft_media(pool: &SqlitePool, chat_id: i64, recipe_id: i64) -> Result<()> {
    let rows = sqlx::query_as::<_, (i64, i64, String)>(
        "SELECT m.id, m.ricetta_step_id, m.percorso_file \
         FROM ricetta_step_media m \
         JOIN ricetta_step s ON s.id = m.ricetta_step_id \
         WHERE s.ricetta_id = ? ORDER BY s.numero, m.ordinamento, m.id",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli allegati appena creati")?;

    for (media_id, step_id, old_path) in rows {
        let source = PathBuf::from(&old_path);
        if !source.exists() {
            continue;
        }
        let filename = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("allegato.bin")
            .to_string();
        let target_dir = PathBuf::from(MEDIA_ROOT)
            .join(recipe_id.to_string())
            .join(step_id.to_string());
        tokio::fs::create_dir_all(&target_dir)
            .await
            .context("Impossibile creare la cartella media ricetta")?;
        let target = target_dir.join(filename);
        if tokio::fs::rename(&source, &target).await.is_err() {
            tokio::fs::copy(&source, &target)
                .await
                .context("Impossibile copiare un allegato ricetta")?;
            tokio::fs::remove_file(&source)
                .await
                .context("Impossibile rimuovere il file temporaneo ricetta")?;
        }
        sqlx::query("UPDATE ricetta_step_media SET percorso_file = ? WHERE id = ?")
            .bind(target.to_string_lossy().into_owned())
            .bind(media_id)
            .execute(pool)
            .await
            .context("Impossibile aggiornare il percorso allegato")?;
    }

    let draft_dir = PathBuf::from(DRAFT_MEDIA_ROOT).join(chat_id.to_string());
    if draft_dir.exists() {
        let _ = tokio::fs::remove_dir_all(draft_dir).await;
    }
    Ok(())
}

async fn cancel_draft_media(chat_id: i64) {
    let draft_dir = PathBuf::from(DRAFT_MEDIA_ROOT).join(chat_id.to_string());
    if draft_dir.exists() {
        let _ = tokio::fs::remove_dir_all(draft_dir).await;
    }
}

async fn save_draft_media(
    bot: &Bot,
    msg: &Message,
    kind: &str,
    step_number: usize,
) -> Result<DraftMedia> {
    let (file_id, caption) = telegram_media_from_message(msg, kind)?;
    let telegram_file = bot
        .get_file(file_id)
        .await
        .context("Impossibile leggere il file Telegram")?;
    let extension = safe_extension(&telegram_file.path, kind);
    let directory = PathBuf::from(DRAFT_MEDIA_ROOT)
        .join(msg.chat.id.0.to_string())
        .join(format!("step_{step_number}"));
    tokio::fs::create_dir_all(&directory)
        .await
        .context("Impossibile creare la cartella temporanea ricetta")?;
    let filename = format!("{}_{}_{}.{}", kind, msg.id.0, unique_suffix(), extension);
    let local_path = directory.join(filename);
    download_telegram_file(bot, &telegram_file.path, &local_path).await?;
    Ok(DraftMedia {
        kind: kind.to_string(),
        temp_path: local_path.to_string_lossy().into_owned(),
        caption,
    })
}

async fn save_existing_step_media(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    recipe_id: i64,
    step_id: i64,
    kind: &str,
) -> Result<()> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    if step_for_recipe(pool, recipe_id, step_id).await?.is_none() {
        bail!("Step non disponibile");
    }
    let (file_id, caption) = telegram_media_from_message(msg, kind)?;
    let telegram_file = bot
        .get_file(file_id)
        .await
        .context("Impossibile leggere il file Telegram")?;
    let extension = safe_extension(&telegram_file.path, kind);
    let directory = PathBuf::from(MEDIA_ROOT)
        .join(recipe_id.to_string())
        .join(step_id.to_string());
    tokio::fs::create_dir_all(&directory)
        .await
        .context("Impossibile creare la cartella allegati step")?;
    let filename = format!("{}_{}_{}.{}", kind, msg.id.0, unique_suffix(), extension);
    let local_path = directory.join(filename);
    download_telegram_file(bot, &telegram_file.path, &local_path).await?;
    let next_order: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(ordinamento), 0) + 1 FROM ricetta_step_media WHERE ricetta_step_id = ?",
    )
    .bind(step_id)
    .fetch_one(pool)
    .await
    .context("Impossibile determinare l'ordine allegato")?;
    if let Err(error) = sqlx::query(
        "INSERT INTO ricetta_step_media (ricetta_step_id, tipo_media, percorso_file, descrizione, ordinamento) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(step_id)
    .bind(kind)
    .bind(local_path.to_string_lossy().into_owned())
    .bind(caption)
    .bind(next_order)
    .execute(pool)
    .await
    {
        let _ = tokio::fs::remove_file(&local_path).await;
        return Err(error).context("Impossibile registrare l'allegato step");
    }
    Ok(())
}

fn telegram_media_from_message(
    msg: &Message,
    kind: &str,
) -> Result<(teloxide::types::FileId, Option<String>)> {
    let caption = msg
        .caption()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    match kind {
        "foto" => {
            let photo = msg
                .photo()
                .context("Foto Telegram non presente")?
                .iter()
                .max_by_key(|photo| u64::from(photo.width) * u64::from(photo.height))
                .context("Foto Telegram non leggibile")?;
            Ok((photo.file.id.clone(), caption))
        }
        "video" => {
            let video = msg.video().context("Video Telegram non presente")?;
            Ok((video.file.id.clone(), caption))
        }
        _ => bail!("Tipo media non supportato"),
    }
}

async fn download_telegram_file(bot: &Bot, telegram_path: &str, local_path: &Path) -> Result<()> {
    let mut destination = File::create(local_path)
        .await
        .context("Impossibile creare il file locale")?;
    if let Err(error) = bot.download_file(telegram_path, &mut destination).await {
        drop(destination);
        let _ = tokio::fs::remove_file(local_path).await;
        return Err(error).context("Download file Telegram fallito");
    }
    Ok(())
}

fn unique_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn safe_extension(path: &str, kind: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match (kind, extension.as_str()) {
        ("foto", "jpg" | "jpeg") => "jpg",
        ("foto", "png") => "png",
        ("foto", "webp") => "webp",
        ("video", "mp4") => "mp4",
        ("video", "mov") => "mov",
        ("video", "webm") => "webm",
        ("foto", _) => "jpg",
        ("video", _) => "mp4",
        _ => "bin",
    }
}

async fn count_visible_recipes(pool: &SqlitePool) -> Result<i64> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let (predicate, bind_space) = recipe_visibility_predicate("r", &actor);
    let sql = format!("SELECT COUNT(*) FROM ricette r WHERE r.archiviata = 0 AND ({predicate})");
    let mut query = sqlx::query_scalar::<_, i64>(&sql).bind(user_id);
    if bind_space {
        query = query.bind(actor.spazio_id);
    } else {
        query = query.bind(user_id);
    }
    query
        .fetch_one(pool)
        .await
        .context("Impossibile contare le ricette visibili")
}

async fn list_visible_recipes(
    pool: &SqlitePool,
    page: i64,
    limit: i64,
) -> Result<Vec<RecipeListRecord>> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let (predicate, bind_space) = recipe_visibility_predicate("r", &actor);
    let sql = format!(
        "SELECT r.id, r.nome AS name, \
                CASE WHEN r.proprietario_utente_id = ? THEN 1 ELSE 0 END AS owner, \
                CASE WHEN r.catalogo_globale = 0 AND r.proprietario_utente_id <> ? THEN 1 ELSE 0 END AS shared \
         FROM ricette r WHERE r.archiviata = 0 AND ({predicate}) \
         ORDER BY r.nome COLLATE NOCASE, r.id LIMIT ? OFFSET ?"
    );
    let mut query = sqlx::query_as::<_, RecipeListRecord>(&sql)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id);
    if bind_space {
        query = query.bind(actor.spazio_id);
    } else {
        query = query.bind(user_id);
    }
    query
        .bind(limit)
        .bind(page.max(0) * limit)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere l'elenco ricette")
}

async fn visible_recipe(pool: &SqlitePool, recipe_id: i64) -> Result<Option<RecipeRecord>> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(None);
    };
    let (predicate, bind_space) = recipe_visibility_predicate("r", &actor);
    let sql = format!(
        "SELECT r.nome AS name, r.porzioni_base AS servings, r.proprietario_utente_id AS owner_user_id, \
                u.nome_visualizzato AS owner_name, r.catalogo_globale AS global_catalog \
         FROM ricette r LEFT JOIN utenti u ON u.id = r.proprietario_utente_id \
         WHERE r.id = ? AND r.archiviata = 0 AND ({predicate})"
    );
    let mut query = sqlx::query_as::<_, RecipeRecord>(&sql)
        .bind(recipe_id)
        .bind(user_id);
    if bind_space {
        query = query.bind(actor.spazio_id);
    } else {
        query = query.bind(user_id);
    }
    query
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere la ricetta")
}

fn recipe_visibility_predicate(alias: &str, actor: &identity::AuditActor) -> (String, bool) {
    if actor.view_all {
        (
            format!(
                "{alias}.catalogo_globale = 1 OR {alias}.proprietario_utente_id = ? OR EXISTS (\
                    SELECT 1 FROM ricetta_spazi rvs JOIN membri_spazio rms ON rms.spazio_id = rvs.spazio_id \
                    WHERE rvs.ricetta_id = {alias}.id AND rms.utente_id = ?\
                 )"
            ),
            false,
        )
    } else {
        (
            format!(
                "{alias}.catalogo_globale = 1 OR {alias}.proprietario_utente_id = ? OR EXISTS (\
                    SELECT 1 FROM ricetta_spazi rvs \
                    WHERE rvs.ricetta_id = {alias}.id AND rvs.spazio_id = ?\
                 )"
            ),
            true,
        )
    }
}

async fn recipe_visible_to_user(pool: &SqlitePool, recipe_id: i64, user_id: i64) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM ricette r \
            WHERE r.id = ? AND r.archiviata = 0 AND (\
                r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS (\
                    SELECT 1 FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
                    WHERE rs.ricetta_id = r.id AND ms.utente_id = ?\
                )\
            )\
         )",
    )
    .bind(recipe_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare la visibilità ricetta")
}

async fn can_edit_recipe(pool: &SqlitePool, recipe_id: i64, user_id: i64) -> Result<bool> {
    let owner: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ricette WHERE id = ? AND archiviata = 0 AND proprietario_utente_id = ?)",
    )
    .bind(recipe_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare la proprietà ricetta")?;
    if owner {
        return Ok(true);
    }
    if !recipe_visible_to_user(pool, recipe_id, user_id).await? {
        return Ok(false);
    }
    resource_permissions::has_edit_permission(pool, RESOURCE_TYPE_RECIPE, recipe_id, user_id).await
}

async fn can_manage_recipe(pool: &SqlitePool, recipe_id: i64, user_id: i64) -> Result<bool> {
    let owner: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ricette WHERE id = ? AND archiviata = 0 AND proprietario_utente_id = ?)",
    )
    .bind(recipe_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare la proprietà ricetta")?;
    if owner {
        return Ok(true);
    }
    if !recipe_visible_to_user(pool, recipe_id, user_id).await? {
        return Ok(false);
    }
    resource_permissions::has_manage_permission(pool, RESOURCE_TYPE_RECIPE, recipe_id, user_id)
        .await
}

async fn ensure_recipe_edit_ui(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<bool> {
    let user_id = identity::current_actor().utente_id.unwrap_or_default();
    if can_edit_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false)
    {
        Ok(true)
    } else {
        bot.send_message(
            chat_id,
            "🔒 Non hai il permesso di modificare questa ricetta.",
        )
        .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
        .await?;
        Ok(false)
    }
}

async fn ensure_recipe_manage_ui(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<bool> {
    let user_id = identity::current_actor().utente_id.unwrap_or_default();
    if can_manage_recipe(pool, recipe_id, user_id)
        .await
        .unwrap_or(false)
    {
        Ok(true)
    } else {
        bot.send_message(
            chat_id,
            "🔒 Non hai il permesso di gestire visibilità o collaboratori.",
        )
        .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
        .await?;
        Ok(false)
    }
}

async fn ensure_recipe_owner_ui(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    recipe_id: i64,
) -> ResponseResult<bool> {
    let user_id = identity::current_actor().utente_id.unwrap_or_default();
    let owner: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ricette WHERE id = ? AND archiviata = 0 AND proprietario_utente_id = ?)",
    )
    .bind(recipe_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);
    if owner {
        Ok(true)
    } else {
        bot.send_message(
            chat_id,
            "🔒 Solo il proprietario può archiviare la ricetta.",
        )
        .reply_markup(back_home_keyboard(&format!("recipe:detail:{recipe_id}")))
        .await?;
        Ok(false)
    }
}

async fn list_recipe_ingredients(
    pool: &SqlitePool,
    recipe_id: i64,
) -> Result<Vec<IngredientRecord>> {
    sqlx::query_as::<_, IngredientRecord>(
        "SELECT ri.id, a.nome AS food_name, \
                CASE WHEN p.id IS NULL THEN NULL ELSE p.marca || ' · ' || p.nome_commerciale END AS product_label, \
                ri.quantita AS quantity, um.simbolo AS unit_symbol, \
                ri.opzionale AS optional, ri.note \
         FROM ricetta_ingredienti ri \
         JOIN alimenti a ON a.id = ri.alimento_id \
         JOIN unita_misura um ON um.id = ri.unita_misura_id \
         LEFT JOIN prodotti_alimentari p ON p.id = ri.prodotto_alimentare_id \
         WHERE ri.ricetta_id = ? \
         ORDER BY ri.ordinamento, ri.id",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli ingredienti della ricetta")
}

async fn list_recipe_steps(pool: &SqlitePool, recipe_id: i64) -> Result<Vec<StepRecord>> {
    sqlx::query_as::<_, StepRecord>(
        "SELECT s.id, s.numero AS number, s.testo AS text, \
                SUM(CASE WHEN m.tipo_media = 'foto' THEN 1 ELSE 0 END) AS photo_count, \
                SUM(CASE WHEN m.tipo_media = 'video' THEN 1 ELSE 0 END) AS video_count \
         FROM ricetta_step s \
         LEFT JOIN ricetta_step_media m ON m.ricetta_step_id = s.id \
         WHERE s.ricetta_id = ? \
         GROUP BY s.id \
         ORDER BY s.numero, s.id",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere il procedimento della ricetta")
}

async fn step_for_recipe(
    pool: &SqlitePool,
    recipe_id: i64,
    step_id: i64,
) -> Result<Option<StepRecord>> {
    sqlx::query_as::<_, StepRecord>(
        "SELECT s.id, s.numero AS number, s.testo AS text, \
                SUM(CASE WHEN m.tipo_media = 'foto' THEN 1 ELSE 0 END) AS photo_count, \
                SUM(CASE WHEN m.tipo_media = 'video' THEN 1 ELSE 0 END) AS video_count \
         FROM ricetta_step s \
         LEFT JOIN ricetta_step_media m ON m.ricetta_step_id = s.id \
         WHERE s.ricetta_id = ? AND s.id = ? GROUP BY s.id",
    )
    .bind(recipe_id)
    .bind(step_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere lo step")
}

async fn list_step_media(pool: &SqlitePool, step_id: i64) -> Result<Vec<StepMediaRecord>> {
    sqlx::query_as::<_, StepMediaRecord>(
        "SELECT id, tipo_media AS kind, percorso_file AS path, descrizione AS caption \
         FROM ricetta_step_media WHERE ricetta_step_id = ? ORDER BY ordinamento, id",
    )
    .bind(step_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli allegati dello step")
}

async fn visible_step_context(pool: &SqlitePool, step_id: i64) -> Result<Option<(i64, i64)>> {
    let row =
        sqlx::query_as::<_, (i64, i64)>("SELECT ricetta_id, numero FROM ricetta_step WHERE id = ?")
            .bind(step_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere lo step")?;
    let Some((recipe_id, number)) = row else {
        return Ok(None);
    };
    if visible_recipe(pool, recipe_id).await?.is_some() {
        Ok(Some((recipe_id, number)))
    } else {
        Ok(None)
    }
}

async fn visible_media(
    pool: &SqlitePool,
    media_id: i64,
) -> Result<Option<(StepMediaRecord, i64, i64)>> {
    let row = sqlx::query_as::<_, (i64, i64, String, String, Option<String>)>(
        "SELECT s.ricetta_id, s.numero, m.tipo_media, m.percorso_file, m.descrizione \
         FROM ricetta_step_media m JOIN ricetta_step s ON s.id = m.ricetta_step_id \
         WHERE m.id = ?",
    )
    .bind(media_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'allegato")?;
    let Some((recipe_id, step_number, kind, path, caption)) = row else {
        return Ok(None);
    };
    if visible_recipe(pool, recipe_id).await?.is_none() {
        return Ok(None);
    }
    Ok(Some((
        StepMediaRecord {
            id: media_id,
            kind,
            path,
            caption,
        },
        recipe_id,
        step_number,
    )))
}

async fn recipe_step_count(pool: &SqlitePool, recipe_id: i64) -> Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM ricetta_step WHERE ricetta_id = ?")
        .bind(recipe_id)
        .fetch_one(pool)
        .await
        .context("Impossibile contare gli step")
}

async fn search_recipes_by_name(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> Result<Vec<RecipeListRecord>> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let (predicate, bind_space) = recipe_visibility_predicate("r", &actor);
    let sql = format!(
        "SELECT r.id, r.nome AS name, \
                CASE WHEN r.proprietario_utente_id = ? THEN 1 ELSE 0 END AS owner, \
                CASE WHEN r.catalogo_globale = 0 AND r.proprietario_utente_id <> ? THEN 1 ELSE 0 END AS shared \
         FROM ricette r \
         WHERE r.archiviata = 0 AND r.nome_normalizzato LIKE ? AND ({predicate}) \
         ORDER BY CASE WHEN r.nome_normalizzato = ? THEN 0 ELSE 1 END, r.nome COLLATE NOCASE, r.id LIMIT ?"
    );
    let normalized = normalize_name(query);
    let like = format!("%{normalized}%");
    let mut q = sqlx::query_as::<_, RecipeListRecord>(&sql)
        .bind(user_id)
        .bind(user_id)
        .bind(&like)
        .bind(user_id);
    if bind_space {
        q = q.bind(actor.spazio_id);
    } else {
        q = q.bind(user_id);
    }
    q.bind(&normalized)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("Impossibile cercare le ricette per nome")
}

pub async fn search_by_ingredients(
    pool: &SqlitePool,
    ingredient_ids: &[i64],
    user_id: i64,
    current_space_id: i64,
    view_all_spaces: bool,
    limit: i64,
) -> Result<Vec<RecipeIngredientMatch>> {
    let mut ids = ingredient_ids
        .iter()
        .copied()
        .filter(|id| *id > 0)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() || limit <= 0 {
        return Ok(Vec::new());
    }
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT r.id AS recipe_id, r.nome AS recipe_name, \
                COUNT(DISTINCT ri.alimento_id) AS matched_ingredients, \
                (SELECT COUNT(*) FROM ricetta_ingredienti all_ri WHERE all_ri.ricetta_id = r.id) AS total_ingredients \
         FROM ricette r JOIN ricetta_ingredienti ri ON ri.ricetta_id = r.id \
         WHERE r.archiviata = 0 AND ri.alimento_id IN (",
    );
    {
        let mut separated = query.separated(", ");
        for ingredient_id in &ids {
            separated.push_bind(*ingredient_id);
        }
    }
    query.push(") AND (r.catalogo_globale = 1 OR r.proprietario_utente_id = ");
    query.push_bind(user_id);
    if view_all_spaces {
        query.push(
            " OR EXISTS (SELECT 1 FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
             WHERE rs.ricetta_id = r.id AND ms.utente_id = ",
        );
        query.push_bind(user_id);
        query.push(")");
    } else {
        query.push(" OR EXISTS (SELECT 1 FROM ricetta_spazi rs WHERE rs.ricetta_id = r.id AND rs.spazio_id = ");
        query.push_bind(current_space_id);
        query.push(")");
    }
    query.push(
        ") GROUP BY r.id, r.nome HAVING COUNT(DISTINCT ri.alimento_id) > 0 \
         ORDER BY matched_ingredients DESC, r.nome COLLATE NOCASE, r.id LIMIT ",
    );
    query.push_bind(limit);
    query
        .build_query_as::<RecipeIngredientMatch>()
        .fetch_all(pool)
        .await
        .context("Impossibile cercare le ricette per ingredienti")
}

pub async fn recipe_food_compatibility(
    pool: &SqlitePool,
    recipe_id: i64,
) -> Result<Vec<RecipeFoodCompatibility>> {
    sqlx::query_as::<_, RecipeFoodCompatibility>(
        "SELECT etichetta_codice AS label_code, etichetta_nome AS label_name, etichetta_emoji AS label_emoji, \
                stato AS status, ingredienti_totali AS total_ingredients, \
                ingredienti_non_compatibili AS incompatible_ingredients, ingredienti_da_verificare AS ingredients_to_check \
         FROM v_ricetta_compatibilita_alimentare WHERE ricetta_id = ? \
         ORDER BY ordinamento, etichetta_nome COLLATE NOCASE",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile calcolare la compatibilità alimentare della ricetta")
}

pub async fn product_choices_for_food(
    pool: &SqlitePool,
    food_id: i64,
) -> Result<Vec<RecipeProductChoice>> {
    sqlx::query_as::<_, RecipeProductChoice>(
        "SELECT p.id AS product_id, p.marca AS brand, p.nome_commerciale AS product_name \
         FROM prodotti_alimentari p \
         WHERE p.alimento_id = ? AND p.attivo = 1 \
         ORDER BY p.marca COLLATE NOCASE, p.nome_commerciale COLLATE NOCASE, p.id",
    )
    .bind(food_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i prodotti commerciali dell'ingrediente")
}

async fn search_food_choices(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> Result<Vec<FoodChoice>> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let normalized = normalize_name(query);
    let like = format!("%{normalized}%");
    let visibility = if actor.view_all {
        "a.catalogo_globale = 1 OR a.proprietario_utente_id = ? OR EXISTS (\
            SELECT 1 FROM alimento_spazi asp JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
            WHERE asp.alimento_id = a.id AND ms.utente_id = ?\
         )"
    } else {
        "a.catalogo_globale = 1 OR a.proprietario_utente_id = ? OR EXISTS (\
            SELECT 1 FROM alimento_spazi asp WHERE asp.alimento_id = a.id AND asp.spazio_id = ?\
         )"
    };
    let sql = format!(
        "SELECT DISTINCT a.id, a.nome AS name, a.unita_predefinita_id AS default_unit_id, \
                um.nome AS default_unit_name, um.simbolo AS default_unit_symbol \
         FROM alimenti a LEFT JOIN unita_misura um ON um.id = a.unita_predefinita_id \
         WHERE a.archiviato = 0 AND ({visibility}) AND (\
            a.nome_normalizzato LIKE ? OR EXISTS (SELECT 1 FROM alimento_alias aa WHERE aa.alimento_id = a.id AND aa.alias_normalizzato LIKE ?) \
            OR EXISTS (SELECT 1 FROM prodotti_alimentari p WHERE p.alimento_id = a.id AND p.attivo = 1 \
                AND (p.marca_normalizzata LIKE ? OR p.nome_commerciale_normalizzato LIKE ?))\
         ) \
         ORDER BY CASE WHEN a.nome_normalizzato = ? THEN 0 ELSE 1 END, a.nome COLLATE NOCASE, a.id LIMIT ?"
    );
    let mut q = sqlx::query_as::<_, FoodChoice>(&sql).bind(user_id);
    if actor.view_all {
        q = q.bind(user_id);
    } else {
        q = q.bind(actor.spazio_id);
    }
    q.bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(&normalized)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("Impossibile cercare gli alimenti per la ricetta")
}

async fn search_food_choices_filtered(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
    category_id: Option<i64>,
) -> Result<Vec<FoodChoice>> {
    let Some(category_id) = category_id else {
        return search_food_choices(pool, query, limit).await;
    };
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let normalized = normalize_name(query);
    let like_value = format!("%{normalized}%");
    let visibility = if actor.view_all {
        "a.catalogo_globale = 1 OR a.proprietario_utente_id = ? OR EXISTS (\
            SELECT 1 FROM alimento_spazi asp JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
            WHERE asp.alimento_id = a.id AND ms.utente_id = ?\
         )"
    } else {
        "a.catalogo_globale = 1 OR a.proprietario_utente_id = ? OR EXISTS (\
            SELECT 1 FROM alimento_spazi asp WHERE asp.alimento_id = a.id AND asp.spazio_id = ?\
         )"
    };
    let sql = format!(
        "SELECT DISTINCT a.id, a.nome AS name, a.unita_predefinita_id AS default_unit_id, \
                um.nome AS default_unit_name, um.simbolo AS default_unit_symbol \
         FROM alimenti a \
         JOIN alimento_categorie ac ON ac.alimento_id = a.id \
         LEFT JOIN unita_misura um ON um.id = a.unita_predefinita_id \
         WHERE a.archiviato = 0 AND ac.categoria_id = ? AND ({visibility}) AND (\
            a.nome_normalizzato LIKE ? OR EXISTS (SELECT 1 FROM alimento_alias aa WHERE aa.alimento_id = a.id AND aa.alias_normalizzato LIKE ?) \
            OR EXISTS (SELECT 1 FROM prodotti_alimentari p WHERE p.alimento_id = a.id AND p.attivo = 1 \
                AND (p.marca_normalizzata LIKE ? OR p.nome_commerciale_normalizzato LIKE ?))\
         ) \
         ORDER BY CASE WHEN a.nome_normalizzato = ? THEN 0 ELSE 1 END, a.nome COLLATE NOCASE, a.id LIMIT ?"
    );
    let mut q = sqlx::query_as::<_, FoodChoice>(&sql)
        .bind(category_id)
        .bind(user_id);
    if actor.view_all {
        q = q.bind(user_id);
    } else {
        q = q.bind(actor.spazio_id);
    }
    q.bind(&like_value)
        .bind(&like_value)
        .bind(&like_value)
        .bind(&like_value)
        .bind(&normalized)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("Impossibile cercare gli alimenti filtrati per categoria")
}

async fn list_recipe_food_categories(pool: &SqlitePool) -> Result<Vec<CategoryChoice>> {
    sqlx::query_as::<_, CategoryChoice>(
        "SELECT id, nome AS name, emoji FROM categorie_alimento WHERE attiva = 1 ORDER BY ordinamento, id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le categorie degli ingredienti")
}

async fn recipe_food_category_by_id(
    pool: &SqlitePool,
    category_id: i64,
) -> Result<Option<CategoryChoice>> {
    sqlx::query_as::<_, CategoryChoice>(
        "SELECT id, nome AS name, emoji FROM categorie_alimento WHERE id = ? AND attiva = 1",
    )
    .bind(category_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere la categoria ingrediente")
}

async fn visible_food_choice(pool: &SqlitePool, food_id: i64) -> Result<Option<FoodChoice>> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(None);
    };
    let visibility = if actor.view_all {
        "a.catalogo_globale = 1 OR a.proprietario_utente_id = ? OR EXISTS (\
            SELECT 1 FROM alimento_spazi asp JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
            WHERE asp.alimento_id = a.id AND ms.utente_id = ?\
         )"
    } else {
        "a.catalogo_globale = 1 OR a.proprietario_utente_id = ? OR EXISTS (\
            SELECT 1 FROM alimento_spazi asp WHERE asp.alimento_id = a.id AND asp.spazio_id = ?\
         )"
    };
    let sql = format!(
        "SELECT a.id, a.nome AS name, a.unita_predefinita_id AS default_unit_id, \
                um.nome AS default_unit_name, um.simbolo AS default_unit_symbol \
         FROM alimenti a LEFT JOIN unita_misura um ON um.id = a.unita_predefinita_id \
         WHERE a.id = ? AND a.archiviato = 0 AND ({visibility})"
    );
    let mut q = sqlx::query_as::<_, FoodChoice>(&sql)
        .bind(food_id)
        .bind(user_id);
    if actor.view_all {
        q = q.bind(user_id);
    } else {
        q = q.bind(actor.spazio_id);
    }
    q.fetch_optional(pool)
        .await
        .context("Impossibile leggere l'alimento")
}

async fn visible_product_choice(
    pool: &SqlitePool,
    product_id: i64,
) -> Result<Option<(FoodChoice, ProductChoice)>> {
    let row = sqlx::query_as::<_, (i64, i64, String, String)>(
        "SELECT p.id, p.alimento_id, p.marca, p.nome_commerciale FROM prodotti_alimentari p WHERE p.id = ? AND p.attivo = 1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il prodotto commerciale")?;
    let Some((product_id, food_id, brand, product_name)) = row else {
        return Ok(None);
    };
    let Some(food) = visible_food_choice(pool, food_id).await? else {
        return Ok(None);
    };
    Ok(Some((
        food,
        ProductChoice {
            product_id,
            brand,
            product_name,
        },
    )))
}

async fn list_units(pool: &SqlitePool) -> Result<Vec<UnitChoice>> {
    sqlx::query_as::<_, UnitChoice>(
        "SELECT id, nome AS name, simbolo AS symbol FROM unita_misura WHERE attiva = 1 ORDER BY ordinamento, id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le unità di misura")
}

async fn unit_by_id(pool: &SqlitePool, unit_id: i64) -> Result<Option<UnitChoice>> {
    sqlx::query_as::<_, UnitChoice>(
        "SELECT id, nome AS name, simbolo AS symbol FROM unita_misura WHERE id = ? AND attiva = 1",
    )
    .bind(unit_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'unità di misura")
}

async fn personal_recipe_name_exists(
    pool: &SqlitePool,
    user_id: i64,
    normalized_name: &str,
    exclude_recipe_id: Option<i64>,
) -> Result<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ricette \
         WHERE proprietario_utente_id = ? AND nome_normalizzato = ? AND archiviata = 0 \
           AND (? IS NULL OR id <> ?))",
    )
    .bind(user_id)
    .bind(normalized_name)
    .bind(exclude_recipe_id)
    .bind(exclude_recipe_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il nome ricetta")?;
    Ok(exists)
}

async fn update_recipe_name(pool: &SqlitePool, recipe_id: i64, name: &str) -> Result<()> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    let normalized = normalize_name(name);
    let owner_id: Option<i64> =
        sqlx::query_scalar("SELECT proprietario_utente_id FROM ricette WHERE id = ?")
            .bind(recipe_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere il proprietario ricetta")?
            .flatten();
    if let Some(owner_id) = owner_id {
        if personal_recipe_name_exists(pool, owner_id, &normalized, Some(recipe_id)).await? {
            bail!("Esiste già una ricetta attiva con questo nome");
        }
    }
    sqlx::query(
        "UPDATE ricette SET nome = ?, nome_normalizzato = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND archiviata = 0",
    )
    .bind(name)
    .bind(normalized)
    .bind(recipe_id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare il nome ricetta")?;
    Ok(())
}

async fn update_recipe_servings(pool: &SqlitePool, recipe_id: i64, servings: i64) -> Result<()> {
    if servings <= 0 {
        bail!("Le porzioni devono essere maggiori di zero");
    }
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    sqlx::query(
        "UPDATE ricette SET porzioni_base = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND archiviata = 0",
    )
    .bind(servings)
    .bind(recipe_id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare le porzioni")?;
    Ok(())
}

async fn recipe_has_food(pool: &SqlitePool, recipe_id: i64, food_id: i64) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ricetta_ingredienti WHERE ricetta_id = ? AND alimento_id = ?)",
    )
    .bind(recipe_id)
    .bind(food_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare l'ingrediente")
}

async fn insert_recipe_ingredient(
    pool: &SqlitePool,
    recipe_id: i64,
    food: &FoodChoice,
    product: Option<&ProductChoice>,
    quantity: f64,
    unit: &UnitChoice,
) -> Result<()> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    if quantity <= 0.0 {
        bail!("Quantità non valida");
    }
    if recipe_has_food(pool, recipe_id, food.id).await? {
        bail!("Questo alimento è già presente nella ricetta");
    }
    if let Some(product) = product {
        let matches_food: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM prodotti_alimentari WHERE id = ? AND alimento_id = ? AND attivo = 1)",
        )
        .bind(product.product_id)
        .bind(food.id)
        .fetch_one(pool)
        .await
        .context("Impossibile verificare il prodotto specifico")?;
        if !matches_food {
            bail!("Il prodotto specifico non appartiene all'alimento selezionato");
        }
    }
    let order: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(ordinamento), 0) + 1 FROM ricetta_ingredienti WHERE ricetta_id = ?",
    )
    .bind(recipe_id)
    .fetch_one(pool)
    .await
    .context("Impossibile determinare l'ordine ingrediente")?;
    sqlx::query(
        "INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, prodotto_alimentare_id, quantita, unita_misura_id, ordinamento) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(recipe_id)
    .bind(food.id)
    .bind(product.map(|value| value.product_id))
    .bind(quantity)
    .bind(unit.id)
    .bind(order)
    .execute(pool)
    .await
    .context("Impossibile aggiungere l'ingrediente")?;
    Ok(())
}

async fn remove_recipe_ingredient(
    pool: &SqlitePool,
    recipe_id: i64,
    ingredient_id: i64,
) -> Result<()> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ricetta_ingredienti WHERE ricetta_id = ?")
            .bind(recipe_id)
            .fetch_one(pool)
            .await
            .context("Impossibile contare gli ingredienti")?;
    if count <= 1 {
        bail!("Una ricetta deve mantenere almeno un ingrediente");
    }
    let result = sqlx::query("DELETE FROM ricetta_ingredienti WHERE id = ? AND ricetta_id = ?")
        .bind(ingredient_id)
        .bind(recipe_id)
        .execute(pool)
        .await
        .context("Impossibile rimuovere l'ingrediente")?;
    if result.rows_affected() != 1 {
        bail!("Ingrediente non disponibile");
    }
    Ok(())
}

async fn add_recipe_step(pool: &SqlitePool, recipe_id: i64, text: &str) -> Result<i64> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    let number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(numero), 0) + 1 FROM ricetta_step WHERE ricetta_id = ?",
    )
    .bind(recipe_id)
    .fetch_one(pool)
    .await
    .context("Impossibile determinare il numero del nuovo step")?;
    let result =
        sqlx::query("INSERT INTO ricetta_step (ricetta_id, numero, testo) VALUES (?, ?, ?)")
            .bind(recipe_id)
            .bind(number)
            .bind(text)
            .execute(pool)
            .await
            .context("Impossibile aggiungere lo step")?;
    Ok(result.last_insert_rowid())
}

async fn update_step_text(
    pool: &SqlitePool,
    recipe_id: i64,
    step_id: i64,
    text: &str,
) -> Result<()> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    let result = sqlx::query(
        "UPDATE ricetta_step SET testo = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND ricetta_id = ?",
    )
    .bind(text)
    .bind(step_id)
    .bind(recipe_id)
    .execute(pool)
    .await
    .context("Impossibile modificare lo step")?;
    if result.rows_affected() != 1 {
        bail!("Step non disponibile");
    }
    Ok(())
}

async fn move_recipe_step(
    pool: &SqlitePool,
    recipe_id: i64,
    step_id: i64,
    direction: i64,
) -> Result<()> {
    if !matches!(direction, -1 | 1) {
        bail!("Direzione spostamento non valida");
    }
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    let current: Option<i64> =
        sqlx::query_scalar("SELECT numero FROM ricetta_step WHERE id = ? AND ricetta_id = ?")
            .bind(step_id)
            .bind(recipe_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere lo step")?;
    let current = current.context("Step non disponibile")?;
    let target_number = current + direction;
    if target_number <= 0 {
        return Ok(());
    }
    let target_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM ricetta_step WHERE ricetta_id = ? AND numero = ?")
            .bind(recipe_id)
            .bind(target_number)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere lo step adiacente")?;
    let Some(target_id) = target_id else {
        return Ok(());
    };
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire lo spostamento step")?;
    let temporary_number = 1_000_000_000_i64.saturating_add(step_id);
    sqlx::query("UPDATE ricetta_step SET numero = ? WHERE id = ? AND ricetta_id = ?")
        .bind(temporary_number)
        .bind(step_id)
        .bind(recipe_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile preparare lo spostamento step")?;
    sqlx::query("UPDATE ricetta_step SET numero = ? WHERE id = ? AND ricetta_id = ?")
        .bind(current)
        .bind(target_id)
        .bind(recipe_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile spostare lo step adiacente")?;
    sqlx::query("UPDATE ricetta_step SET numero = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND ricetta_id = ?")
        .bind(target_number)
        .bind(step_id)
        .bind(recipe_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile completare lo spostamento step")?;
    tx.commit()
        .await
        .context("Impossibile salvare lo spostamento step")?;
    Ok(())
}

async fn delete_recipe_step(pool: &SqlitePool, recipe_id: i64, step_id: i64) -> Result<()> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    let steps = list_recipe_steps(pool, recipe_id).await?;
    if steps.len() <= 1 {
        bail!("Una ricetta deve mantenere almeno uno step del procedimento");
    }
    let step = steps
        .iter()
        .find(|step| step.id == step_id)
        .cloned()
        .context("Step non disponibile")?;
    let media = list_step_media(pool, step_id).await.unwrap_or_default();
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire l'eliminazione step")?;
    sqlx::query("DELETE FROM ricetta_step WHERE id = ? AND ricetta_id = ?")
        .bind(step_id)
        .bind(recipe_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile eliminare lo step")?;
    // Due fasi per non violare UNIQUE (ricetta_id, numero) durante una
    // rinumerazione multipla (es. eliminazione dello step 2 su 4 step).
    const RENUMBER_OFFSET: i64 = 1_000_000_000;
    sqlx::query("UPDATE ricetta_step SET numero = numero + ? WHERE ricetta_id = ? AND numero > ?")
        .bind(RENUMBER_OFFSET)
        .bind(recipe_id)
        .bind(step.number)
        .execute(&mut *tx)
        .await
        .context("Impossibile preparare la rinumerazione degli step")?;
    sqlx::query("UPDATE ricetta_step SET numero = numero - ? WHERE ricetta_id = ? AND numero > ?")
        .bind(RENUMBER_OFFSET + 1)
        .bind(recipe_id)
        .bind(RENUMBER_OFFSET + step.number)
        .execute(&mut *tx)
        .await
        .context("Impossibile rinumerare gli step")?;
    tx.commit()
        .await
        .context("Impossibile completare l'eliminazione step")?;
    for item in media {
        let path = PathBuf::from(item.path);
        if path.exists() {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
    Ok(())
}

async fn delete_step_media(pool: &SqlitePool, recipe_id: i64, media_id: i64) -> Result<i64> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if !can_edit_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di modificare questa ricetta");
    }
    let row = sqlx::query_as::<_, (i64, String)>(
        "SELECT m.ricetta_step_id, m.percorso_file FROM ricetta_step_media m \
         JOIN ricetta_step s ON s.id = m.ricetta_step_id WHERE m.id = ? AND s.ricetta_id = ?",
    )
    .bind(media_id)
    .bind(recipe_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'allegato")?
    .context("Allegato non disponibile")?;
    sqlx::query("DELETE FROM ricetta_step_media WHERE id = ?")
        .bind(media_id)
        .execute(pool)
        .await
        .context("Impossibile rimuovere l'allegato")?;
    let path = PathBuf::from(row.1);
    if path.exists() {
        let _ = tokio::fs::remove_file(path).await;
    }
    Ok(row.0)
}

async fn set_recipe_spaces(pool: &SqlitePool, recipe_id: i64, spaces: &[i64]) -> Result<()> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    if !can_manage_recipe(pool, recipe_id, user_id).await? {
        bail!("Non hai il permesso di gestire la visibilità della ricetta");
    }
    let mut unique = spaces
        .iter()
        .copied()
        .filter(|id| *id > 0)
        .collect::<Vec<_>>();
    unique.sort_unstable();
    unique.dedup();
    for space_id in &unique {
        identity::ensure_can_write_space(pool, *space_id).await?;
    }
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire l'aggiornamento visibilità")?;
    // Prima aggiungiamo i nuovi spazi e solo dopo rimuoviamo quelli non più
    // selezionati. È importante per un collaboratore con permesso Manage:
    // durante gli INSERT deve esistere ancora almeno uno spazio che gli dia
    // visibilità, come richiesto dai trigger fail-closed della ricetta.
    for space_id in &unique {
        sqlx::query("INSERT OR IGNORE INTO ricetta_spazi (ricetta_id, spazio_id, condivisa_da_utente_id) VALUES (?, ?, ?)")
            .bind(recipe_id)
            .bind(*space_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile aggiungere uno spazio alla ricetta")?;
    }
    if unique.is_empty() {
        sqlx::query("DELETE FROM ricetta_spazi WHERE ricetta_id = ?")
            .bind(recipe_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile rendere privata la ricetta")?;
    } else {
        let placeholders = std::iter::repeat_n("?", unique.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "DELETE FROM ricetta_spazi WHERE ricetta_id = ? AND spazio_id NOT IN ({placeholders})"
        );
        let mut query = sqlx::query(&sql).bind(recipe_id);
        for space_id in &unique {
            query = query.bind(*space_id);
        }
        query
            .execute(&mut *tx)
            .await
            .context("Impossibile restringere la visibilità ricetta")?;
    }
    // Visibilità e permesso restano separati: se l'utente non vede più la
    // ricetta attraverso alcuno spazio, il suo permesso esplicito viene tolto.
    sqlx::query(
        "DELETE FROM permessi_risorsa WHERE tipo_risorsa = 'ricetta' AND risorsa_id = ? AND utente_id NOT IN (\
            SELECT DISTINCT ms.utente_id FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
            WHERE rs.ricetta_id = ?\
         )",
    )
    .bind(recipe_id)
    .bind(recipe_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile riallineare i permessi ricetta")?;
    sqlx::query(
        "UPDATE inviti_risorsa SET stato = 'revocato', risposto_il = COALESCE(risposto_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE tipo_risorsa = 'ricetta' AND risorsa_id = ? AND stato = 'pendente' AND invitato_utente_id NOT IN (\
            SELECT DISTINCT ms.utente_id FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
            WHERE rs.ricetta_id = ?\
         )",
    )
    .bind(recipe_id)
    .bind(recipe_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile riallineare gli inviti ricetta")?;
    tx.commit()
        .await
        .context("Impossibile salvare la visibilità ricetta")?;
    Ok(())
}

async fn recipe_space_ids(pool: &SqlitePool, recipe_id: i64) -> Result<Vec<i64>> {
    sqlx::query_scalar(
        "SELECT spazio_id FROM ricetta_spazi WHERE ricetta_id = ? ORDER BY spazio_id",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi della ricetta")
}

async fn recipe_space_names(pool: &SqlitePool, recipe_id: i64) -> Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT s.nome FROM ricetta_spazi rs JOIN spazi s ON s.id = rs.spazio_id WHERE rs.ricetta_id = ? ORDER BY s.nome COLLATE NOCASE, s.id",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i nomi degli spazi ricetta")
}

async fn eligible_collaborators(pool: &SqlitePool, recipe_id: i64) -> Result<Vec<(i64, String)>> {
    let owner_id: Option<i64> =
        sqlx::query_scalar("SELECT proprietario_utente_id FROM ricette WHERE id = ?")
            .bind(recipe_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere il proprietario ricetta")?
            .flatten();
    sqlx::query_as::<_, (i64, String)>(
        "SELECT DISTINCT u.id, u.nome_visualizzato FROM ricetta_spazi rs \
         JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
         JOIN utenti u ON u.id = ms.utente_id AND u.stato = 'attivo' \
         WHERE rs.ricetta_id = ? AND (? IS NULL OR u.id <> ?) \
         ORDER BY u.nome_visualizzato COLLATE NOCASE, u.id",
    )
    .bind(recipe_id)
    .bind(owner_id)
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i collaboratori candidati")
}

async fn list_recipe_permissions(
    pool: &SqlitePool,
    recipe_id: i64,
) -> Result<Vec<(i64, String, i64, i64)>> {
    sqlx::query_as::<_, (i64, String, i64, i64)>(
        "SELECT p.utente_id, u.nome_visualizzato, p.puo_modificare, p.puo_gestire_permessi \
         FROM permessi_risorsa p JOIN utenti u ON u.id = p.utente_id \
         WHERE p.tipo_risorsa = 'ricetta' AND p.risorsa_id = ? \
         ORDER BY u.nome_visualizzato COLLATE NOCASE, u.id",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i permessi ricetta")
}

async fn pending_invite_count(pool: &SqlitePool) -> Result<i64> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM inviti_risorsa WHERE tipo_risorsa = 'ricetta' AND invitato_utente_id = ? AND stato = 'pendente'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile contare gli inviti ricetta")
}

async fn delete_recipe_permanently(pool: &SqlitePool, recipe_id: i64) -> Result<Vec<String>> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    let owner: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ricette WHERE id = ? AND proprietario_utente_id = ?)",
    )
    .bind(recipe_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il proprietario della ricetta")?;
    if !owner {
        bail!("Solo il proprietario può eliminare definitivamente la ricetta");
    }
    let paths = sqlx::query_scalar::<_, String>(
        "SELECT m.percorso_file FROM ricetta_step_media m \
         JOIN ricetta_step s ON s.id = m.ricetta_step_id WHERE s.ricetta_id = ?",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i media della ricetta")?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare l'eliminazione")?;
    sqlx::query("DELETE FROM inviti_risorsa WHERE tipo_risorsa = 'ricetta' AND risorsa_id = ?")
        .bind(recipe_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile eliminare gli inviti della ricetta")?;
    sqlx::query("DELETE FROM permessi_risorsa WHERE tipo_risorsa = 'ricetta' AND risorsa_id = ?")
        .bind(recipe_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile eliminare i permessi della ricetta")?;
    let deleted = sqlx::query("DELETE FROM ricette WHERE id = ? AND proprietario_utente_id = ?")
        .bind(recipe_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile eliminare la ricetta")?;
    if deleted.rows_affected() != 1 {
        bail!("Ricetta non disponibile o non di tua proprietà");
    }
    tx.commit()
        .await
        .context("Impossibile completare l'eliminazione")?;
    Ok(paths)
}

async fn cleanup_recipe_media_files(recipe_id: i64, paths: &[String]) {
    for path in paths {
        let _ = tokio::fs::remove_file(path).await;
    }
    let _ = tokio::fs::remove_dir_all(PathBuf::from(MEDIA_ROOT).join(recipe_id.to_string())).await;
}

async fn archive_recipe(pool: &SqlitePool, recipe_id: i64) -> Result<()> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    let result = sqlx::query(
        "UPDATE ricette SET archiviata = 1, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND proprietario_utente_id = ? AND archiviata = 0",
    )
    .bind(recipe_id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile archiviare la ricetta")?;
    if result.rows_affected() != 1 {
        bail!("Ricetta non disponibile o non di tua proprietà");
    }
    Ok(())
}

fn draft_from_state(state: Option<RecipeConversationState>) -> Option<RecipeDraft> {
    match state? {
        RecipeConversationState::NewServings { draft }
        | RecipeConversationState::IngredientHub { draft }
        | RecipeConversationState::IngredientSearch { draft }
        | RecipeConversationState::IngredientQuantityReady { draft, .. }
        | RecipeConversationState::StepText { draft }
        | RecipeConversationState::StepMedia { draft, .. }
        | RecipeConversationState::StepPhoto { draft, .. }
        | RecipeConversationState::StepVideo { draft, .. }
        | RecipeConversationState::AfterStep { draft }
        | RecipeConversationState::Visibility { draft }
        | RecipeConversationState::VisibilityChoose { draft, .. } => Some(draft),
        _ => None,
    }
}

fn step_media_state(state: Option<RecipeConversationState>) -> Option<(RecipeDraft, usize)> {
    match state? {
        RecipeConversationState::StepMedia { draft, step_index }
        | RecipeConversationState::StepPhoto { draft, step_index }
        | RecipeConversationState::StepVideo { draft, step_index } => Some((draft, step_index)),
        _ => None,
    }
}

fn selected_foods_from_state(state: Option<RecipeConversationState>) -> Option<Vec<FoodChoice>> {
    match state? {
        RecipeConversationState::IngredientFinder { selected }
        | RecipeConversationState::IngredientFinderQuery { selected, .. } => Some(selected),
        _ => None,
    }
}

fn ingredient_filter_from_state(state: Option<RecipeConversationState>) -> Option<CategoryChoice> {
    match state? {
        RecipeConversationState::IngredientFinderQuery {
            category_filter, ..
        } => category_filter,
        _ => None,
    }
}

fn ingredient_query_state(
    state: Option<RecipeConversationState>,
) -> (Vec<FoodChoice>, Option<CategoryChoice>) {
    match state {
        Some(RecipeConversationState::IngredientFinder { selected }) => (selected, None),
        Some(RecipeConversationState::IngredientFinderQuery {
            selected,
            category_filter,
        }) => (selected, category_filter),
        _ => (Vec::new(), None),
    }
}

fn recipe_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("📋 Elenco ricette", "recipe:list:0"),
            button("➕ Nuova ricetta", "recipe:new"),
        ],
        vec![
            button("🔎 Cerca per nome", "recipe:search"),
            button("🥕 Cerca per ingredienti", "recipe:find"),
        ],
        vec![
            button("⬅️ Indietro", "food:menu"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn draft_ingredients_keyboard(draft: &RecipeDraft) -> InlineKeyboardMarkup {
    let mut rows = draft
        .ingredients
        .iter()
        .enumerate()
        .map(|(index, ingredient)| {
            vec![button(
                format!("🗑 {}", ingredient.food_name),
                format!("recipe:new:ingredient:remove:{index}"),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        "➕ Aggiungi ingrediente",
        "recipe:new:ingredient:add",
    )]);
    if !draft.ingredients.is_empty() {
        rows.push(vec![button(
            "✅ Ingredienti completati",
            "recipe:new:ingredients:done",
        )]);
    }
    rows.push(vec![
        button("❌ Annulla", "recipe:new:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn step_media_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("📷 Aggiungi foto", "recipe:new:step:photo"),
            button("🎥 Aggiungi video", "recipe:new:step:video"),
        ],
        vec![button("✅ Completa step", "recipe:new:step:done")],
        vec![
            button("❌ Annulla", "recipe:new:cancel"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn step_attachment_cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "❌ Annulla allegato",
            "recipe:new:step:attachment:cancel",
        )],
        vec![button("🏠 Menù principale", "menu:main")],
    ])
}

fn recipe_confirmation_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("✅ Salva ricetta", "recipe:new:save")],
        vec![
            button("⬅️ Visibilità", "recipe:new:steps:done"),
            button("❌ Annulla", "recipe:new:cancel"),
        ],
        vec![button("🏠 Menù principale", "menu:main")],
    ])
}

fn ingredient_finder_keyboard(
    selected: &[FoodChoice],
    category_filter: Option<&CategoryChoice>,
) -> InlineKeyboardMarkup {
    let category_label = category_filter
        .map(|category| format!("🏷 {} {}", category.emoji, category.name))
        .unwrap_or_else(|| "🏷 Filtra categoria".to_string());
    let mut rows = vec![vec![button(category_label, "recipe:find:categories")]];
    if category_filter.is_some() {
        rows.push(vec![button(
            "🧹 Rimuovi filtro categoria",
            "recipe:find:filter:clear",
        )]);
    }
    if !selected.is_empty() {
        rows.push(vec![button(
            "➕ Aggiungi ingrediente",
            "recipe:find:addmore",
        )]);
        rows.push(vec![
            button("🔎 Cerca ricette", "recipe:find:run"),
            button("🧹 Azzera", "recipe:find:reset"),
        ]);
    }
    rows.push(vec![
        button("⬅️ Indietro", "recipe:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn ingredient_quantity_keyboard(
    change_unit_callback: &str,
    back_callback: &str,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("📏 Cambia unità", change_unit_callback)],
        vec![
            button("⬅️ Indietro", back_callback),
            button("❌ Annulla", "recipe:new:cancel"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn flow_keyboard(back_callback: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", back_callback),
        button("❌ Annulla", "recipe:new:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn back_home_keyboard(back_callback: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", back_callback),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn button(text: impl Into<String>, callback: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), callback.into())
}

async fn send_text_required(bot: &Bot, chat_id: ChatId, message: &str) -> ResponseResult<()> {
    bot.send_message(chat_id, format!("⚠️ {message}"))
        .reply_markup(flow_keyboard("recipe:menu"))
        .await?;
    Ok(())
}

async fn show_expired_flow(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "⚠️ Questa operazione Ricette non è più attiva. Riapri la sezione e riprova.",
    )
    .reply_markup(recipe_menu_keyboard())
    .await?;
    Ok(())
}

async fn show_invalid_action(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "⚠️ Azione Ricette non disponibile.")
        .reply_markup(recipe_menu_keyboard())
        .await?;
    Ok(())
}

fn first_command(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let token = trimmed.split_whitespace().next()?;
    token.split('@').next()
}

fn clean_text(raw: &str, max_chars: usize) -> Option<String> {
    let clean = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = clean.chars().count();
    if count == 0 || count > max_chars {
        None
    } else {
        Some(clean)
    }
}

fn normalize_name(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn parse_positive_i64(raw: &str) -> Option<i64> {
    raw.trim().parse::<i64>().ok().filter(|value| *value > 0)
}

fn parse_positive_i64_str(raw: &str) -> Option<i64> {
    raw.parse::<i64>().ok().filter(|value| *value > 0)
}

fn parse_nonnegative_i64(raw: &str) -> Option<i64> {
    raw.parse::<i64>().ok().filter(|value| *value >= 0)
}

fn parse_positive_number(raw: &str) -> Option<f64> {
    let normalized = raw.trim().replace(',', ".");
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn parse_two_positive_ids(raw: &str) -> Option<(i64, i64)> {
    let mut parts = raw.split(':');
    let first = parts.next().and_then(parse_positive_i64_str)?;
    let second = parts.next().and_then(parse_positive_i64_str)?;
    if parts.next().is_some() {
        None
    } else {
        Some((first, second))
    }
}

fn parse_recipe_page(raw: &str) -> Option<(i64, i64)> {
    let mut parts = raw.split(':');
    let recipe_id = parts.next().and_then(parse_positive_i64_str)?;
    let page = parts.next().and_then(parse_nonnegative_i64)?;
    if parts.next().is_some() {
        None
    } else {
        Some((recipe_id, page))
    }
}

fn toggle_id(values: &mut Vec<i64>, target: i64) {
    if let Some(index) = values.iter().position(|value| *value == target) {
        values.remove(index);
    } else {
        values.push(target);
        values.sort_unstable();
        values.dedup();
    }
}

fn page_count(total: i64, page_size: i64) -> i64 {
    if total <= 0 || page_size <= 0 {
        0
    } else {
        (total + page_size - 1) / page_size
    }
}

fn result_label(total: i64) -> String {
    if total == 1 {
        "1 risultato".to_string()
    } else {
        format!("{total} risultati")
    }
}

fn product_label(product: &ProductChoice) -> String {
    format!("{} · {}", product.brand, product.product_name)
}

fn compatibility_icon(status: &str) -> &'static str {
    match status {
        "si" | "sì" => "✅",
        "no" => "❌",
        _ => "⚠️",
    }
}

fn full_procedure_text_chunks(recipe_name: &str, steps: &[StepRecord]) -> Vec<String> {
    // Lasciamo margine rispetto al limite Telegram di 4096 caratteri per
    // header, emoji e possibili differenze di conteggio Unicode.
    const MAX_CHARS: usize = 3_800;
    let first_header = format!("📖 Procedimento completo\n{recipe_name}");
    if steps.is_empty() {
        return vec![format!("{first_header}\n\nNessuno step disponibile.")];
    }

    let continuation_header = format!("📖 Procedimento completo · continua\n{recipe_name}");
    let mut chunks = Vec::new();
    let mut current = first_header.clone();

    for step in steps {
        let mut block = format!("\n\n{}. {}", step.number, step.text);
        let media = media_summary(step.photo_count, step.video_count);
        if !media.is_empty() {
            block.push_str(&format!("\n{media}"));
        }

        let header_len = if chunks.is_empty() {
            first_header.chars().count()
        } else {
            continuation_header.chars().count()
        };
        if current.chars().count() + block.chars().count() > MAX_CHARS
            && current.chars().count() > header_len
        {
            chunks.push(current);
            current = continuation_header.clone();
        }
        current.push_str(&block);
    }
    chunks.push(current);
    chunks
}

fn media_summary(photos: i64, videos: i64) -> String {
    let mut parts = Vec::new();
    if photos > 0 {
        parts.push(format!("📷 {photos}"));
    }
    if videos > 0 {
        parts.push(format!("🎥 {videos}"));
    }
    parts.join(" · ")
}

fn unit_icon(symbol: &str) -> &'static str {
    match symbol {
        "g" | "kg" => "⚖️",
        "ml" | "l" => "🥤",
        "pz" => "🔢",
        "cucchiaio" | "cucchiaino" => "🥄",
        _ => "📏",
    }
}

fn plural_unit_name(name: &str, symbol: &str) -> String {
    match symbol {
        "g" => "grammi".to_string(),
        "kg" => "chilogrammi".to_string(),
        "ml" => "millilitri".to_string(),
        "l" => "litri".to_string(),
        "pz" => "pezzi".to_string(),
        "cucchiaio" => "cucchiai".to_string(),
        "cucchiaino" => "cucchiaini".to_string(),
        _ => name.to_string(),
    }
}

fn display_quantity(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        let mut text = format!("{value:.3}");
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let mut output = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        output.push('…');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("db test");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration test");
        pool
    }

    async fn create_user(pool: &SqlitePool, name: &str, space_id: i64) -> i64 {
        let result = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("utente");
        let user_id = result.last_insert_rowid();
        if space_id != 1 {
            sqlx::query("INSERT OR IGNORE INTO spazi (id, nome, tipo, creato_da_utente_id) VALUES (?, ?, 'personale', ?)")
                .bind(space_id)
                .bind(format!("Spazio {name}"))
                .bind(user_id)
                .execute(pool)
                .await
                .expect("spazio");
        }
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
        user_id
    }

    fn actor(user_id: i64, space_id: i64, name: &str, view_all: bool) -> identity::AuditActor {
        identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: name.to_string(),
            spazio_id: space_id,
            spazio_nome_snapshot: format!("Spazio {space_id}"),
            view_all,
            origine: "test",
            telegram_user_id: None,
            telegram_username: None,
        }
    }

    async fn base_food(pool: &SqlitePool, offset: i64) -> FoodChoice {
        sqlx::query_as::<_, FoodChoice>(
            "SELECT a.id, a.nome AS name, a.unita_predefinita_id AS default_unit_id, \
                    um.nome AS default_unit_name, um.simbolo AS default_unit_symbol \
             FROM alimenti a LEFT JOIN unita_misura um ON um.id = a.unita_predefinita_id \
             WHERE a.catalogo_globale = 1 AND a.archiviato = 0 ORDER BY a.id LIMIT 1 OFFSET ?",
        )
        .bind(offset)
        .fetch_one(pool)
        .await
        .expect("alimento base")
    }

    async fn unit(pool: &SqlitePool, symbol: &str) -> UnitChoice {
        sqlx::query_as::<_, UnitChoice>(
            "SELECT id, nome AS name, simbolo AS symbol FROM unita_misura WHERE simbolo = ?",
        )
        .bind(symbol)
        .fetch_one(pool)
        .await
        .expect("unità")
    }

    fn ingredient(food: &FoodChoice, unit: &UnitChoice, quantity: f64) -> DraftIngredient {
        DraftIngredient {
            food_id: food.id,
            food_name: food.name.clone(),
            product_id: None,
            product_label: None,
            quantity,
            unit_id: unit.id,
            unit_symbol: unit.symbol.clone(),
        }
    }

    #[tokio::test]
    async fn ricetta_salva_ingredienti_step_e_media_strutturati() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Chef", 1).await;
        let food = base_food(&pool, 0).await;
        let grams = unit(&pool, "g").await;
        let draft = RecipeDraft {
            name: "Ricetta guidata test".to_string(),
            servings: 2,
            ingredients: vec![ingredient(&food, &grams, 150.0)],
            steps: vec![
                DraftStep {
                    text: "Prepara gli ingredienti".to_string(),
                    media: Vec::new(),
                },
                DraftStep {
                    text: "Cuoci tutto".to_string(),
                    media: Vec::new(),
                },
            ],
            visible_spaces: Vec::new(),
        };
        let recipe_id = identity::with_actor(actor(user_id, 1, "Chef", false), async {
            save_recipe(&pool, 99, &draft).await.expect("salvataggio")
        })
        .await;
        let counts: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM ricetta_ingredienti WHERE ricetta_id = ?), \
                    (SELECT COUNT(*) FROM ricetta_step WHERE ricetta_id = ?)",
        )
        .bind(recipe_id)
        .bind(recipe_id)
        .fetch_one(&pool)
        .await
        .expect("conteggi");
        assert_eq!(counts, (1, 2));
        let numbers: Vec<i64> = sqlx::query_scalar(
            "SELECT numero FROM ricetta_step WHERE ricetta_id = ? ORDER BY numero",
        )
        .bind(recipe_id)
        .fetch_all(&pool)
        .await
        .expect("step");
        assert_eq!(numbers, vec![1, 2]);
    }

    #[tokio::test]
    async fn prodotto_ricetta_non_duplica_i_formati_di_vendita() {
        let pool = test_pool().await;
        let food = base_food(&pool, 0).await;
        let grams = unit(&pool, "g").await;
        let product = sqlx::query(
            "INSERT INTO prodotti_alimentari (alimento_id, marca, marca_normalizzata, nome_commerciale, nome_commerciale_normalizzato, quantita_confezione, unita_confezione_id) \
             VALUES (?, 'Marca Test', 'marca test', 'Original', 'original', 100, ?)",
        )
        .bind(food.id)
        .bind(grams.id)
        .execute(&pool)
        .await
        .expect("prodotto")
        .last_insert_rowid();
        for quantity in [100.0, 200.0, 350.0] {
            sqlx::query(
                "INSERT OR IGNORE INTO formati_prodotto_alimentare (prodotto_alimentare_id, quantita_confezione, unita_confezione_id) VALUES (?, ?, ?)",
            )
            .bind(product)
            .bind(quantity)
            .bind(grams.id)
            .execute(&pool)
            .await
            .expect("formato");
        }
        let choices = product_choices_for_food(&pool, food.id)
            .await
            .expect("prodotti ricetta");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].product_id, product);
        assert_eq!(choices[0].brand, "Marca Test");
    }

    #[tokio::test]
    async fn ricerca_ingredienti_usa_or_e_ordina_per_corrispondenze() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Ricercatore", 1).await;
        let food_a = base_food(&pool, 0).await;
        let food_b = base_food(&pool, 1).await;
        let grams = unit(&pool, "g").await;
        let actor = actor(user_id, 1, "Ricercatore", false);
        let recipe_a = identity::with_actor(actor.clone(), async {
            save_recipe(
                &pool,
                1,
                &RecipeDraft {
                    name: "Due ingredienti".to_string(),
                    servings: 1,
                    ingredients: vec![
                        ingredient(&food_a, &grams, 10.0),
                        ingredient(&food_b, &grams, 20.0),
                    ],
                    steps: vec![DraftStep {
                        text: "Step A".to_string(),
                        media: Vec::new(),
                    }],
                    visible_spaces: Vec::new(),
                },
            )
            .await
            .expect("ricetta A")
        })
        .await;
        let recipe_b = identity::with_actor(actor.clone(), async {
            save_recipe(
                &pool,
                1,
                &RecipeDraft {
                    name: "Un ingrediente".to_string(),
                    servings: 1,
                    ingredients: vec![ingredient(&food_a, &grams, 10.0)],
                    steps: vec![DraftStep {
                        text: "Step B".to_string(),
                        media: Vec::new(),
                    }],
                    visible_spaces: Vec::new(),
                },
            )
            .await
            .expect("ricetta B")
        })
        .await;
        let rows = search_by_ingredients(&pool, &[food_a.id, food_b.id], user_id, 1, false, 10)
            .await
            .expect("ricerca OR");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].recipe_id, recipe_a);
        assert_eq!(rows[0].matched_ingredients, 2);
        assert_eq!(rows[1].recipe_id, recipe_b);
        assert_eq!(rows[1].matched_ingredients, 1);
    }

    #[tokio::test]
    async fn step_si_possono_riordinare_ed_eliminare_con_rinumerazione() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Editor", 1).await;
        let food = base_food(&pool, 0).await;
        let grams = unit(&pool, "g").await;
        let actor = actor(user_id, 1, "Editor", false);
        let recipe_id = identity::with_actor(actor.clone(), async {
            save_recipe(
                &pool,
                1,
                &RecipeDraft {
                    name: "Ordine step".to_string(),
                    servings: 1,
                    ingredients: vec![ingredient(&food, &grams, 1.0)],
                    steps: vec![
                        DraftStep {
                            text: "Uno".to_string(),
                            media: Vec::new(),
                        },
                        DraftStep {
                            text: "Due".to_string(),
                            media: Vec::new(),
                        },
                        DraftStep {
                            text: "Tre".to_string(),
                            media: Vec::new(),
                        },
                    ],
                    visible_spaces: Vec::new(),
                },
            )
            .await
            .expect("ricetta")
        })
        .await;
        let steps = list_recipe_steps(&pool, recipe_id).await.expect("step");
        let step_two = steps[1].id;
        identity::with_actor(actor.clone(), async {
            move_recipe_step(&pool, recipe_id, step_two, -1)
                .await
                .expect("sposta");
        })
        .await;
        let texts: Vec<String> = sqlx::query_scalar(
            "SELECT testo FROM ricetta_step WHERE ricetta_id = ? ORDER BY numero",
        )
        .bind(recipe_id)
        .fetch_all(&pool)
        .await
        .expect("ordine");
        assert_eq!(texts, vec!["Due", "Uno", "Tre"]);
        let last_id: i64 =
            sqlx::query_scalar("SELECT id FROM ricetta_step WHERE ricetta_id = ? AND numero = 3")
                .bind(recipe_id)
                .fetch_one(&pool)
                .await
                .expect("ultimo");
        identity::with_actor(actor, async {
            delete_recipe_step(&pool, recipe_id, last_id)
                .await
                .expect("elimina");
        })
        .await;
        let numbers: Vec<i64> = sqlx::query_scalar(
            "SELECT numero FROM ricetta_step WHERE ricetta_id = ? ORDER BY numero",
        )
        .bind(recipe_id)
        .fetch_all(&pool)
        .await
        .expect("numeri");
        assert_eq!(numbers, vec![1, 2]);
    }

    #[tokio::test]
    async fn visibilita_non_concede_modifica_senza_permesso_esplicito() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner", 1).await;
        let other_id = create_user(&pool, "Guest", 2).await;
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (1, ?, 'membro')",
        )
        .bind(other_id)
        .execute(&pool)
        .await
        .expect("membership condivisa");
        let food = base_food(&pool, 0).await;
        let grams = unit(&pool, "g").await;
        let recipe_id = identity::with_actor(actor(owner_id, 1, "Owner", false), async {
            save_recipe(
                &pool,
                1,
                &RecipeDraft {
                    name: "Condivisa".to_string(),
                    servings: 1,
                    ingredients: vec![ingredient(&food, &grams, 1.0)],
                    steps: vec![DraftStep {
                        text: "Step".to_string(),
                        media: Vec::new(),
                    }],
                    visible_spaces: vec![1],
                },
            )
            .await
            .expect("ricetta")
        })
        .await;
        assert!(recipe_visible_to_user(&pool, recipe_id, other_id)
            .await
            .expect("visible"));
        assert!(!can_edit_recipe(&pool, recipe_id, other_id)
            .await
            .expect("edit"));
        let invite_id = resource_permissions::create_invite(
            &pool,
            RESOURCE_TYPE_RECIPE,
            recipe_id,
            other_id,
            owner_id,
            ResourcePermission::Edit,
        )
        .await
        .expect("invito");
        resource_permissions::accept_invite(&pool, invite_id, other_id)
            .await
            .expect("accetta");
        assert!(can_edit_recipe(&pool, recipe_id, other_id)
            .await
            .expect("edit after"));
    }

    #[tokio::test]
    async fn eliminare_uno_step_intermedio_rinumera_senza_violare_unique() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Editor middle", 1).await;
        let food = base_food(&pool, 0).await;
        let grams = unit(&pool, "g").await;
        let actor = actor(user_id, 1, "Editor middle", false);
        let recipe_id = identity::with_actor(actor.clone(), async {
            save_recipe(
                &pool,
                1,
                &RecipeDraft {
                    name: "Elimina step intermedio".to_string(),
                    servings: 1,
                    ingredients: vec![ingredient(&food, &grams, 1.0)],
                    steps: vec![
                        DraftStep {
                            text: "Uno".to_string(),
                            media: Vec::new(),
                        },
                        DraftStep {
                            text: "Due".to_string(),
                            media: Vec::new(),
                        },
                        DraftStep {
                            text: "Tre".to_string(),
                            media: Vec::new(),
                        },
                        DraftStep {
                            text: "Quattro".to_string(),
                            media: Vec::new(),
                        },
                    ],
                    visible_spaces: Vec::new(),
                },
            )
            .await
            .expect("ricetta")
        })
        .await;
        let step_two: i64 =
            sqlx::query_scalar("SELECT id FROM ricetta_step WHERE ricetta_id = ? AND numero = 2")
                .bind(recipe_id)
                .fetch_one(&pool)
                .await
                .expect("step due");
        identity::with_actor(actor, async {
            delete_recipe_step(&pool, recipe_id, step_two)
                .await
                .expect("elimina step intermedio");
        })
        .await;
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT numero, testo FROM ricetta_step WHERE ricetta_id = ? ORDER BY numero",
        )
        .bind(recipe_id)
        .fetch_all(&pool)
        .await
        .expect("step rinumerati");
        assert_eq!(
            rows,
            vec![
                (1, "Uno".to_string()),
                (2, "Tre".to_string()),
                (3, "Quattro".to_string()),
            ]
        );
    }

    #[test]
    fn procedimento_completo_viene_diviso_senza_perdere_step_lunghi() {
        let long = "x".repeat(3_400);
        let steps = vec![
            StepRecord {
                id: 1,
                number: 1,
                text: long.clone(),
                photo_count: 1,
                video_count: 0,
            },
            StepRecord {
                id: 2,
                number: 2,
                text: long,
                photo_count: 0,
                video_count: 1,
            },
        ];
        let chunks = full_procedure_text_chunks("Ricetta lunga", &steps);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 3_800));
        let joined = chunks.join("\n");
        assert!(joined.contains("1. "));
        assert!(joined.contains("2. "));
        assert!(joined.contains("📷 1"));
        assert!(joined.contains("🎥 1"));
    }

    #[test]
    fn callback_ricette_restano_sotto_il_limite_telegram() {
        let id = i64::MAX;
        let callbacks = [
            format!("recipe:edit:irem:{id}:{id}"),
            format!("recipe:edit:pg:{id}:{id}"),
            format!("recipe:edit:p:{id}:{id}"),
            format!("recipe:edit:iu:{id}:{id}"),
            format!("recipe:edit:ie:{id}:{id}"),
            format!("recipe:edit:im:{id}:{id}"),
            format!("recipe:edit:rev:{id}:{id}"),
            format!("recipe:edit:md:{id}:{id}"),
            format!("recipe:edit:step:photo:{id}:{id}"),
            format!("recipe:edit:vis:toggle:{id}:{id}"),
        ];
        for callback in callbacks {
            assert!(
                callback.len() <= 64,
                "callback troppo lungo: {} ({})",
                callback,
                callback.len()
            );
        }
    }

    #[test]
    fn paginazione_ricette_usa_cinque_elementi() {
        assert_eq!(RECIPE_LIST_PAGE_SIZE, 5);
        assert_eq!(page_count(0, RECIPE_LIST_PAGE_SIZE), 0);
        assert_eq!(page_count(1, RECIPE_LIST_PAGE_SIZE), 1);
        assert_eq!(page_count(6, RECIPE_LIST_PAGE_SIZE), 2);
    }

    #[test]
    fn quantita_accetta_formato_italiano() {
        assert_eq!(parse_positive_number("12,5"), Some(12.5));
        assert_eq!(parse_positive_number("0"), None);
        assert_eq!(parse_positive_number("-1"), None);
    }
}
