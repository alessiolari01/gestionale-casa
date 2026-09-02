//! Step 7.2D.0.4.1 - rifiniture catalogo, audit prodotti e navigazione.
//!
//! Proprietà e visibilità sono separate:
//! - l'alimento appartiene all'utente che lo crea;
//! - può essere condiviso con zero, uno o più spazi;
//! - perdere una membership non fa perdere gli alimenti posseduti;
//! - gli alimenti altrui condivisi in spazi comuni vengono letti dal DB
//!   centrale e quindi si sincronizzano senza creare copie.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use super::storico::{self, NewFieldChange, NewHistoryEvent};
use crate::identity;

type Bot = crate::context_bot::ContextBot;

use crate::modules::liste;

const FOOD_PAGE_SIZE: usize = 5;
const FOOD_PAGE_FETCH: i64 = FOOD_PAGE_SIZE as i64 + 1;
const FOOD_NAME_MAX_CHARS: usize = 120;
const PRODUCT_TEXT_MAX_CHARS: usize = 120;

#[derive(Clone, Default)]
pub struct FoodSessionStore {
    inner: Arc<Mutex<HashMap<i64, FoodConversationState>>>,
}

#[derive(Debug, Clone)]
enum FoodConversationState {
    Name,
    Unit {
        name: String,
    },
    PendingCategory {
        name: String,
        unit_id: Option<i64>,
    },
    Visibility {
        name: String,
        unit_id: Option<i64>,
        category_id: i64,
    },
    Spaces {
        name: String,
        unit_id: Option<i64>,
        category_id: i64,
        selected: Vec<i64>,
    },
    Search,
    SearchResults {
        query: String,
    },
    FilterCategories {
        selected: Vec<i64>,
    },
    EditFoodName {
        food_id: i64,
    },
    EditFoodSpaces {
        food_id: i64,
        selected: Vec<i64>,
    },
    ProductBrand {
        food_id: i64,
    },
    ProductName {
        food_id: i64,
        brand: String,
    },
    ProductQuantity {
        food_id: i64,
        brand: String,
        product_name: String,
        unit_id: i64,
    },
    ProductNutritionInput {
        product_id: i64,
        reference_unit_id: i64,
    },
    ProductNutritionConfirm {
        product_id: i64,
        reference_unit_id: i64,
        values: [Option<f64>; 9],
    },
    ProductEditBrand {
        product_id: i64,
    },
    ProductEditName {
        product_id: i64,
    },
    ProductFormatQuantity {
        product_id: i64,
        unit_id: i64,
    },
    ProductFormatEditQuantity {
        product_id: i64,
        format_id: i64,
        unit_id: i64,
    },
    ProductFormatEditEan {
        product_id: i64,
        format_id: i64,
    },
}

impl FoodSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, chat_id: i64) -> Option<FoodConversationState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&chat_id)
            .cloned()
    }

    fn set(&self, chat_id: i64, state: FoodConversationState) {
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
}

#[derive(Debug, Clone, FromRow)]
struct UnitRecord {
    id: i64,
    nome: String,
    simbolo: String,
}

#[derive(Debug, Clone, FromRow)]
struct SpaceRecord {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, FromRow)]
struct CategoryRecord {
    id: i64,
    name: String,
    emoji: String,
}

#[derive(Debug, Clone, FromRow)]
struct FoodRecord {
    id: i64,
    name: String,
    description: Option<String>,
    unit_code: Option<String>,
    unit_symbol: Option<String>,
    owner_user_id: Option<i64>,
    owner_name: Option<String>,
    global_catalog: i64,
}

#[derive(Debug, Clone, FromRow)]
struct ProductRecord {
    id: i64,
    food_id: i64,
    brand: String,
    product_name: String,
    format_count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct ProductFormatRecord {
    id: i64,
    product_id: i64,
    package_quantity: f64,
    package_unit_id: i64,
    package_unit_symbol: String,
    ean: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct NutritionRecord {
    reference_unit_symbol: String,
    energy_kcal: Option<f64>,
    energy_kj: Option<f64>,
    fat_g: Option<f64>,
    saturated_fat_g: Option<f64>,
    carbohydrates_g: Option<f64>,
    sugars_g: Option<f64>,
    fibre_g: Option<f64>,
    protein_g: Option<f64>,
    salt_g: Option<f64>,
}

pub async fn show_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🍽️ Alimentazione\n\nScegli alimenti, ricette, profili alimentari oppure il Planner.",
    )
    .reply_markup(alimentation_menu_keyboard())
    .await?;
    Ok(())
}

async fn show_foods_menu(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    // C7: il conteggio sta sul pulsante, non in una riga di testo sopra. Se
    // il conteggio non si riesce a leggere si mostra comunque il menu' senza
    // numeri: una sezione raggiungibile vale piu' di un numero esatto.
    let total = count_foods(pool, None, None).await.ok().map(|t| t as i64);
    bot.send_message(chat_id, "🥕 Alimenti")
        .reply_markup(food_menu_keyboard_con_conteggio(total))
        .await?;
    Ok(())
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &FoodSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if let Some((command, args)) = parse_command(text) {
        match command {
            "/alimentazione" => {
                sessions.clear_chat(chat_id);
                show_menu(bot, msg.chat.id).await?;
                return Ok(true);
            }
            "/alimenti" => {
                sessions.clear_chat(chat_id);
                show_foods_menu(bot, msg.chat.id, pool).await?;
                return Ok(true);
            }
            "/alimento_nuovo" => {
                if args.is_empty() {
                    sessions.set(chat_id, FoodConversationState::Name);
                    bot.send_message(
                        msg.chat.id,
                        "➕ Nuovo alimento\n\n\
                         Scrivi il nome dell'alimento.\n\
                         Esempio: Pollo\n\n\
                         Premi ❌ Annulla per uscire.",
                    )
                    .reply_markup(cancel_keyboard())
                    .await?;
                } else {
                    start_unit_choice(bot, msg.chat.id, chat_id, pool, sessions, args).await?;
                }
                return Ok(true);
            }
            "/alimenti_lista" => {
                sessions.clear_chat(chat_id);
                send_food_list(bot, msg.chat.id, pool, 0).await?;
                return Ok(true);
            }
            "/alimento_cerca" => {
                if args.is_empty() {
                    sessions.set(chat_id, FoodConversationState::Search);
                    bot.send_message(
                        msg.chat.id,
                        "🔎 Cerca alimento\n\n\
                         Scrivi il nome o un alias da cercare.\n\n\
                         Premi ❌ Annulla per uscire.",
                    )
                    .reply_markup(cancel_keyboard())
                    .await?;
                } else {
                    sessions.set(
                        chat_id,
                        FoodConversationState::SearchResults {
                            query: args.to_string(),
                        },
                    );
                    send_search_results(bot, msg.chat.id, pool, args, 0).await?;
                }
                return Ok(true);
            }
            "/alimento" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_food_detail(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Apri un alimento da 📋 Elenco alimenti oppure usa 🔎 Cerca.",
                    )
                    .reply_markup(food_menu_keyboard())
                    .await?;
                }
                return Ok(true);
            }
            "/annulla" => {
                if sessions.has_active(chat_id) {
                    sessions.clear_chat(chat_id);
                    bot.send_message(
                        msg.chat.id,
                        "❌ Operazione annullata. Nessuna modifica pendente è stata salvata.",
                    )
                    .await?;
                    show_menu(bot, msg.chat.id).await?;
                    return Ok(true);
                }
                return Ok(false);
            }
            _ => {
                if sessions.get(chat_id).is_some() {
                    sessions.clear_chat(chat_id);
                }
                return Ok(false);
            }
        }
    }

    match sessions.get(chat_id) {
        Some(FoodConversationState::Name) => {
            start_unit_choice(bot, msg.chat.id, chat_id, pool, sessions, text).await?;
            Ok(true)
        }
        Some(FoodConversationState::Unit { name }) => {
            match find_unit_by_text(pool, text).await {
                Ok(Some(unit)) => {
                    start_category_choice(
                        bot,
                        msg.chat.id,
                        chat_id,
                        pool,
                        sessions,
                        name,
                        Some(unit.id),
                    )
                    .await?;
                }
                Ok(None) => {
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Unità non riconosciuta.\n\n\
                         Scrivi uno tra: g, kg, ml, l, pz, cucchiaio, \
                         cucchiaino, qb.\n\
                         Scegli un'unità per continuare.",
                    )
                    .reply_markup(
                        unit_keyboard_from_db(pool)
                            .await
                            .unwrap_or_else(|_| cancel_keyboard()),
                    )
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore ricerca unità alimento");
                    bot.send_message(msg.chat.id, "⚠️ Non riesco a leggere le unità di misura.")
                        .reply_markup(cancel_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::PendingCategory { .. }) => {
            bot.send_message(
                msg.chat.id,
                "Usa i pulsanti per scegliere la categoria dell'alimento, oppure premi ❌ Annulla.",
            )
            .await?;
            Ok(true)
        }
        Some(FoodConversationState::Visibility { .. })
        | Some(FoodConversationState::Spaces { .. })
        | Some(FoodConversationState::EditFoodSpaces { .. }) => {
            bot.send_message(
                msg.chat.id,
                "Usa i pulsanti della schermata corrente oppure premi ❌ Annulla.",
            )
            .await?;
            Ok(true)
        }
        Some(FoodConversationState::EditFoodName { food_id }) => {
            sessions.clear_chat(chat_id);
            match update_food_name(pool, food_id, text).await {
                Ok(()) => {
                    bot.send_message(msg.chat.id, "✅ Nome alimento aggiornato.")
                        .await?;
                    send_food_detail(bot, msg.chat.id, pool, food_id).await?;
                }
                Err(error) => {
                    tracing::warn!(?error, food_id, "Modifica nome alimento non riuscita");
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductBrand { food_id }) => {
            let Some(brand) = clean_product_text(text) else {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Marca non valida. Scrivi un nome breve, ad esempio: Philadelphia.",
                )
                .reply_markup(product_cancel_keyboard(food_id))
                .await?;
                return Ok(true);
            };
            sessions.set(
                chat_id,
                FoodConversationState::ProductName { food_id, brand },
            );
            bot.send_message(
                msg.chat.id,
                "🛒 Nome commerciale\n\nScrivi il nome del prodotto.\nEsempio: Original.",
            )
            .reply_markup(product_cancel_keyboard(food_id))
            .await?;
            Ok(true)
        }
        Some(FoodConversationState::ProductName { food_id, brand }) => {
            let Some(product_name) = clean_product_text(text) else {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Nome prodotto non valido. Riprova con un nome breve.",
                )
                .reply_markup(product_cancel_keyboard(food_id))
                .await?;
                return Ok(true);
            };
            match default_product_package_unit(pool, food_id).await {
                Ok(unit) => {
                    sessions.set(
                        chat_id,
                        FoodConversationState::ProductQuantity {
                            food_id,
                            brand,
                            product_name,
                            unit_id: unit.id,
                        },
                    );
                    send_product_quantity_prompt(bot, msg.chat.id, food_id, &unit).await?;
                }
                Err(error) => {
                    tracing::error!(?error, food_id, "Errore unità predefinita prodotto");
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Non riesco a determinare l'unità della confezione.",
                    )
                    .reply_markup(product_cancel_keyboard(food_id))
                    .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductQuantity {
            food_id,
            brand,
            product_name,
            unit_id,
        }) => {
            let normalized = text.trim().replace(',', ".");
            let quantity = normalized.parse::<f64>().ok().filter(|value| *value > 0.0);
            let Some(quantity) = quantity else {
                match get_unit_by_id(pool, unit_id).await {
                    Ok(Some(unit)) => {
                        bot.send_message(
                            msg.chat.id,
                            "⚠️ Quantità non valida. Scrivi un numero maggiore di zero, ad esempio 200.",
                        )
                        .reply_markup(product_quantity_keyboard(food_id))
                        .await?;
                        send_product_quantity_prompt(bot, msg.chat.id, food_id, &unit).await?;
                    }
                    _ => {
                        bot.send_message(msg.chat.id, "⚠️ Unità confezione non disponibile.")
                            .reply_markup(product_cancel_keyboard(food_id))
                            .await?;
                    }
                }
                return Ok(true);
            };

            match create_product_association(
                pool,
                food_id,
                &brand,
                &product_name,
                quantity,
                unit_id,
            )
            .await
            {
                Ok(product_id) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Prodotto associato all'alimento.")
                        .await?;
                    send_product_detail(bot, msg.chat.id, pool, product_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(product_quantity_keyboard(food_id))
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductNutritionInput {
            product_id,
            reference_unit_id,
        }) => {
            match parse_nutrition_values(text) {
                Ok(values) => match get_unit_by_id(pool, reference_unit_id).await {
                    Ok(Some(unit)) => {
                        sessions.set(
                            chat_id,
                            FoodConversationState::ProductNutritionConfirm {
                                product_id,
                                reference_unit_id,
                                values,
                            },
                        );
                        send_nutrition_confirmation(bot, msg.chat.id, product_id, &unit, &values)
                            .await?;
                    }
                    _ => {
                        bot.send_message(msg.chat.id, "⚠️ Unità nutrizionale non disponibile.")
                            .reply_markup(nutrition_input_keyboard(product_id))
                            .await?;
                    }
                },
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(nutrition_input_keyboard(product_id))
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductNutritionConfirm { product_id, .. }) => {
            bot.send_message(
                msg.chat.id,
                "Usa ✅ Conferma per salvare, ✏️ Modifica per correggere oppure ❌ Annulla.",
            )
            .reply_markup(nutrition_confirmation_keyboard(product_id))
            .await?;
            Ok(true)
        }
        Some(FoodConversationState::ProductEditBrand { product_id }) => {
            let Some(brand) = clean_product_text(text) else {
                bot.send_message(msg.chat.id, "⚠️ Marca non valida. Riprova.")
                    .reply_markup(product_edit_cancel_keyboard(product_id))
                    .await?;
                return Ok(true);
            };
            sessions.clear_chat(chat_id);
            match update_product_brand(pool, product_id, &brand).await {
                Ok(()) => {
                    bot.send_message(msg.chat.id, "✅ Marca aggiornata.")
                        .await?;
                    send_product_detail(bot, msg.chat.id, pool, product_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(product_edit_cancel_keyboard(product_id))
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductEditName { product_id }) => {
            let Some(product_name) = clean_product_text(text) else {
                bot.send_message(msg.chat.id, "⚠️ Nome commerciale non valido. Riprova.")
                    .reply_markup(product_edit_cancel_keyboard(product_id))
                    .await?;
                return Ok(true);
            };
            sessions.clear_chat(chat_id);
            match update_product_name(pool, product_id, &product_name).await {
                Ok(()) => {
                    bot.send_message(msg.chat.id, "✅ Nome commerciale aggiornato.")
                        .await?;
                    send_product_detail(bot, msg.chat.id, pool, product_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(product_edit_cancel_keyboard(product_id))
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductFormatQuantity {
            product_id,
            unit_id,
        }) => {
            let normalized = text.trim().replace(',', ".");
            let quantity = normalized
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value > 0.0);
            let Some(quantity) = quantity else {
                match get_unit_by_id(pool, unit_id).await {
                    Ok(Some(unit)) => {
                        bot.send_message(
                            msg.chat.id,
                            "⚠️ Quantità non valida. Scrivi un numero maggiore di zero.",
                        )
                        .reply_markup(product_format_quantity_keyboard(product_id))
                        .await?;
                        send_product_format_quantity_prompt(bot, msg.chat.id, product_id, &unit)
                            .await?;
                    }
                    _ => {
                        bot.send_message(msg.chat.id, "⚠️ Unità formato non disponibile.")
                            .reply_markup(product_format_cancel_keyboard(product_id))
                            .await?;
                    }
                }
                return Ok(true);
            };
            sessions.clear_chat(chat_id);
            match create_product_format(pool, product_id, quantity, unit_id).await {
                Ok(_) => {
                    bot.send_message(msg.chat.id, "✅ Formato aggiunto.")
                        .await?;
                    send_product_formats(bot, msg.chat.id, pool, product_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(product_format_cancel_keyboard(product_id))
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductFormatEditQuantity {
            product_id,
            format_id,
            unit_id,
        }) => {
            let normalized = text.trim().replace(',', ".");
            let quantity = normalized
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value > 0.0);
            let Some(quantity) = quantity else {
                match get_unit_by_id(pool, unit_id).await {
                    Ok(Some(unit)) => {
                        bot.send_message(
                            msg.chat.id,
                            "⚠️ Quantità non valida. Scrivi un numero maggiore di zero.",
                        )
                        .reply_markup(product_format_edit_quantity_keyboard(format_id))
                        .await?;
                        send_product_format_edit_quantity_prompt(
                            bot,
                            msg.chat.id,
                            format_id,
                            &unit,
                        )
                        .await?;
                    }
                    _ => {
                        bot.send_message(msg.chat.id, "⚠️ Unità formato non disponibile.")
                            .reply_markup(product_format_detail_keyboard(
                                format_id, product_id, true,
                            ))
                            .await?;
                    }
                }
                return Ok(true);
            };
            sessions.clear_chat(chat_id);
            match update_product_format_quantity(pool, format_id, quantity, unit_id).await {
                Ok(()) => {
                    bot.send_message(msg.chat.id, "✅ Formato aggiornato.")
                        .await?;
                    send_product_format_detail(bot, msg.chat.id, pool, format_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(product_format_detail_keyboard(format_id, product_id, true))
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::ProductFormatEditEan {
            product_id,
            format_id,
        }) => {
            let ean = match parse_ean_input(text) {
                Ok(ean) => ean,
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(product_format_detail_keyboard(format_id, product_id, true))
                        .await?;
                    return Ok(true);
                }
            };
            sessions.clear_chat(chat_id);
            match update_product_format_ean(pool, format_id, ean.as_deref()).await {
                Ok(()) => {
                    bot.send_message(msg.chat.id, "✅ Barcode / EAN aggiornato.")
                        .await?;
                    send_product_format_detail(bot, msg.chat.id, pool, format_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(product_format_detail_keyboard(format_id, product_id, true))
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::FilterCategories { .. })
        | Some(FoodConversationState::SearchResults { .. }) => {
            bot.send_message(
                msg.chat.id,
                "Usa i pulsanti della schermata corrente oppure avvia una nuova ricerca.",
            )
            .await?;
            Ok(true)
        }
        Some(FoodConversationState::Search) => {
            sessions.set(
                chat_id,
                FoodConversationState::SearchResults {
                    query: text.to_string(),
                },
            );
            send_search_results(bot, msg.chat.id, pool, text, 0).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &FoodSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    match data {
        "food:noop" => Ok(true),
        "food:menu" => {
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id).await?;
            Ok(true)
        }
        "food:foods" => {
            sessions.clear_chat(chat_id.0);
            show_foods_menu(bot, chat_id, pool).await?;
            Ok(true)
        }
        "food:new" => {
            sessions.set(chat_id.0, FoodConversationState::Name);
            bot.send_message(
                chat_id,
                "➕ Nuovo alimento\n\n\
                 Scrivi il nome dell'alimento.\n\
                 Esempio: Pollo\n\n\
                 Premi ❌ Annulla per uscire.",
            )
            .reply_markup(cancel_keyboard())
            .await?;
            Ok(true)
        }
        "food:list" => {
            sessions.clear_chat(chat_id.0);
            send_food_list(bot, chat_id, pool, 0).await?;
            Ok(true)
        }
        "food:search" => {
            sessions.set(chat_id.0, FoodConversationState::Search);
            bot.send_message(
                chat_id,
                "🔎 Cerca alimento\n\n\
                 Scrivi il nome o un alias da cercare.\n\n\
                 Premi ❌ Annulla per uscire.",
            )
            .reply_markup(cancel_keyboard())
            .await?;
            Ok(true)
        }
        "food:search:list" => {
            sessions.set(chat_id.0, FoodConversationState::Search);
            bot.send_message(
                chat_id,
                "🔎 Cerca alimento\n\nScrivi il nome o un alias da cercare.\n\nPremi ❌ Annulla per uscire.",
            )
            .reply_markup(search_from_list_keyboard())
            .await?;
            Ok(true)
        }
        "food:filter" => {
            let selected = match sessions.get(chat_id.0) {
                Some(FoodConversationState::FilterCategories { selected }) => selected,
                _ => Vec::new(),
            };
            sessions.set(
                chat_id.0,
                FoodConversationState::FilterCategories {
                    selected: selected.clone(),
                },
            );
            send_food_filter_menu(bot, chat_id, pool, &selected).await?;
            Ok(true)
        }
        "food:filter:all" => {
            sessions.clear_chat(chat_id.0);
            send_food_list(bot, chat_id, pool, 0).await?;
            Ok(true)
        }
        "food:filter:clear" => {
            sessions.set(
                chat_id.0,
                FoodConversationState::FilterCategories {
                    selected: Vec::new(),
                },
            );
            send_food_filter_menu(bot, chat_id, pool, &[]).await?;
            Ok(true)
        }
        "food:filter:apply" => {
            let selected = match sessions.get(chat_id.0) {
                Some(FoodConversationState::FilterCategories { selected }) => selected,
                _ => Vec::new(),
            };

            if selected.is_empty() {
                sessions.clear_chat(chat_id.0);
                send_food_list(bot, chat_id, pool, 0).await?;
            } else {
                let page = 0;
                sessions.set(
                    chat_id.0,
                    FoodConversationState::FilterCategories {
                        selected: selected.clone(),
                    },
                );
                send_filtered_food_list(bot, chat_id, pool, &selected, page).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:filter:toggle:") => {
            let category_id = data
                .strip_prefix("food:filter:toggle:")
                .and_then(|raw| raw.parse::<i64>().ok())
                .filter(|id| *id > 0);

            let Some(category_id) = category_id else {
                bot.send_message(chat_id, "Filtro categoria non valido.")
                    .reply_markup(food_menu_keyboard())
                    .await?;
                return Ok(true);
            };

            match list_categories(pool).await {
                Ok(categories) if categories.iter().any(|category| category.id == category_id) => {
                    let mut selected = match sessions.get(chat_id.0) {
                        Some(FoodConversationState::FilterCategories { selected }) => selected,
                        _ => Vec::new(),
                    };

                    if let Some(index) = selected.iter().position(|id| *id == category_id) {
                        selected.remove(index);
                    } else {
                        selected.push(category_id);
                        selected.sort_unstable();
                        selected.dedup();
                    }

                    sessions.set(
                        chat_id.0,
                        FoodConversationState::FilterCategories {
                            selected: selected.clone(),
                        },
                    );
                    send_food_filter_menu(bot, chat_id, pool, &selected).await?;
                }
                Ok(_) => {
                    bot.send_message(chat_id, "Categoria non disponibile.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore verifica categoria filtro multiplo");
                    bot.send_message(chat_id, "⚠️ Non riesco a leggere le categorie.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }

        _ if data.starts_with("food:list:page:") => {
            let page = data
                .strip_prefix("food:list:page:")
                .and_then(parse_nonnegative_page);
            if let Some(page) = page {
                sessions.clear_chat(chat_id.0);
                send_food_list(bot, chat_id, pool, page).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:search:page:") => {
            let page = data
                .strip_prefix("food:search:page:")
                .and_then(parse_nonnegative_page);
            let state = sessions.get(chat_id.0);
            match (page, state) {
                (Some(page), Some(FoodConversationState::SearchResults { query, .. })) => {
                    sessions.set(
                        chat_id.0,
                        FoodConversationState::SearchResults {
                            query: query.clone(),
                        },
                    );
                    send_search_results(bot, chat_id, pool, &query, page).await?;
                }
                _ => {
                    bot.send_message(chat_id, "La ricerca non è più attiva. Avviane una nuova.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:filter:page:") => {
            let page = data
                .strip_prefix("food:filter:page:")
                .and_then(parse_nonnegative_page);
            let state = sessions.get(chat_id.0);
            match (page, state) {
                (Some(page), Some(FoodConversationState::FilterCategories { selected }))
                    if !selected.is_empty() =>
                {
                    sessions.set(
                        chat_id.0,
                        FoodConversationState::FilterCategories {
                            selected: selected.clone(),
                        },
                    );
                    send_filtered_food_list(bot, chat_id, pool, &selected, page).await?;
                }
                _ => {
                    bot.send_message(chat_id, "Il filtro non è più attivo.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:new:category:") => {
            let category_id = data
                .strip_prefix("food:new:category:")
                .and_then(parse_positive_id);
            let Some(category_id) = category_id else {
                bot.send_message(chat_id, "Categoria non valida.").await?;
                return Ok(true);
            };
            let Some(FoodConversationState::PendingCategory { name, unit_id }) =
                sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };
            let valid = list_categories(pool)
                .await
                .map(|categories| categories.iter().any(|category| category.id == category_id))
                .unwrap_or(false);
            if !valid {
                bot.send_message(chat_id, "Categoria non disponibile.")
                    .await?;
                return Ok(true);
            }
            start_visibility_choice(
                bot,
                chat_id,
                chat_id.0,
                sessions,
                name,
                unit_id,
                category_id,
            )
            .await?;
            Ok(true)
        }
        _ if data.starts_with("food:edit:menu:") => {
            let food_id = data
                .strip_prefix("food:edit:menu:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                sessions.clear_chat(chat_id.0);
                send_food_edit_menu(bot, chat_id, pool, food_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:name:") => {
            let food_id = data
                .strip_prefix("food:edit:name:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                if can_edit_food_current(pool, food_id).await.unwrap_or(false) {
                    sessions.set(chat_id.0, FoodConversationState::EditFoodName { food_id });
                    bot.send_message(
                        chat_id,
                        "📝 Nuovo nome\n\nScrivi il nuovo nome dell'alimento.",
                    )
                    .reply_markup(edit_text_keyboard(food_id))
                    .await?;
                } else {
                    bot.send_message(
                        chat_id,
                        "⚠️ Non hai il permesso di modificare questo alimento.",
                    )
                    .reply_markup(food_menu_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:unit:") => {
            let food_id = data
                .strip_prefix("food:edit:unit:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                send_edit_unit_menu(bot, chat_id, pool, food_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:setunit:") => {
            let ids = data
                .strip_prefix("food:edit:setunit:")
                .and_then(parse_two_positive_ids);
            if let Some((food_id, unit_id)) = ids {
                match update_food_unit(pool, food_id, unit_id).await {
                    Ok(()) => {
                        bot.send_message(chat_id, "✅ Unità alimento aggiornata.")
                            .await?;
                        send_food_detail(bot, chat_id, pool, food_id).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}"))
                            .reply_markup(food_menu_keyboard())
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:visibility:") => {
            let food_id = data
                .strip_prefix("food:edit:visibility:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                send_edit_visibility_menu(bot, chat_id, pool, food_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:vis:private:") => {
            let food_id = data
                .strip_prefix("food:edit:vis:private:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                match replace_food_shares(pool, food_id, &[]).await {
                    Ok(()) => send_food_detail(bot, chat_id, pool, food_id).await?,
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:vis:default:") => {
            let food_id = data
                .strip_prefix("food:edit:vis:default:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                let actor = identity::current_actor();
                match replace_food_shares(pool, food_id, &[actor.spazio_id]).await {
                    Ok(()) => send_food_detail(bot, chat_id, pool, food_id).await?,
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:vis:all:") => {
            let food_id = data
                .strip_prefix("food:edit:vis:all:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                match list_shareable_spaces(pool).await {
                    Ok(spaces) => {
                        let ids: Vec<i64> = spaces.iter().map(|space| space.id).collect();
                        match replace_food_shares(pool, food_id, &ids).await {
                            Ok(()) => send_food_detail(bot, chat_id, pool, food_id).await?,
                            Err(error) => {
                                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                            }
                        }
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:vis:choose:") => {
            let food_id = data
                .strip_prefix("food:edit:vis:choose:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                if !can_manage_food_current(pool, food_id)
                    .await
                    .unwrap_or(false)
                {
                    bot.send_message(chat_id, "⚠️ Non hai il permesso di gestire la visibilità.")
                        .await?;
                    return Ok(true);
                }
                match (
                    list_shareable_spaces(pool).await,
                    current_food_share_ids(pool, food_id).await,
                ) {
                    (Ok(spaces), Ok(selected)) => {
                        sessions.set(
                            chat_id.0,
                            FoodConversationState::EditFoodSpaces {
                                food_id,
                                selected: selected.clone(),
                            },
                        );
                        bot.send_message(
                            chat_id,
                            "🎛 Visibilità alimento\n\nSeleziona gli spazi e premi ✅ Salva.",
                        )
                        .reply_markup(edit_space_selection_keyboard(food_id, &spaces, &selected))
                        .await?;
                    }
                    _ => {
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere gli spazi.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:space:") => {
            let ids = data
                .strip_prefix("food:edit:space:")
                .and_then(parse_two_positive_ids);
            if let Some((food_id, space_id)) = ids {
                let Some(FoodConversationState::EditFoodSpaces {
                    food_id: state_food_id,
                    mut selected,
                }) = sessions.get(chat_id.0)
                else {
                    bot.send_message(chat_id, "Selezione spazi scaduta.")
                        .await?;
                    return Ok(true);
                };
                if state_food_id != food_id {
                    bot.send_message(chat_id, "Selezione spazi non valida.")
                        .await?;
                    return Ok(true);
                }
                match list_shareable_spaces(pool).await {
                    Ok(spaces) if spaces.iter().any(|space| space.id == space_id) => {
                        if let Some(index) = selected.iter().position(|id| *id == space_id) {
                            selected.remove(index);
                        } else {
                            selected.push(space_id);
                            selected.sort_unstable();
                            selected.dedup();
                        }
                        sessions.set(
                            chat_id.0,
                            FoodConversationState::EditFoodSpaces {
                                food_id,
                                selected: selected.clone(),
                            },
                        );
                        bot.send_message(chat_id, "🎛 Selezione aggiornata.")
                            .reply_markup(edit_space_selection_keyboard(
                                food_id, &spaces, &selected,
                            ))
                            .await?;
                    }
                    _ => {
                        bot.send_message(chat_id, "Spazio non disponibile.").await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:spaces:save:") => {
            let food_id = data
                .strip_prefix("food:edit:spaces:save:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                let Some(FoodConversationState::EditFoodSpaces {
                    food_id: state_food_id,
                    selected,
                }) = sessions.get(chat_id.0)
                else {
                    bot.send_message(chat_id, "Selezione spazi scaduta.")
                        .await?;
                    return Ok(true);
                };
                if state_food_id != food_id {
                    bot.send_message(chat_id, "Selezione spazi non valida.")
                        .await?;
                    return Ok(true);
                }
                sessions.clear_chat(chat_id.0);
                match replace_food_shares(pool, food_id, &selected).await {
                    Ok(()) => send_food_detail(bot, chat_id, pool, food_id).await?,
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:edit:cancel:") => {
            let food_id = data
                .strip_prefix("food:edit:cancel:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(food_id) = food_id {
                bot.send_message(chat_id, "Modifica annullata.").await?;
                send_food_detail(bot, chat_id, pool, food_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:permissions:") => {
            let food_id = data
                .strip_prefix("food:permissions:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                send_food_permissions_menu(bot, chat_id, pool, food_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:perm:choose:") => {
            let ids = data
                .strip_prefix("food:perm:choose:")
                .and_then(parse_two_positive_ids);
            if let Some((food_id, user_id)) = ids {
                send_permission_level_menu(bot, chat_id, pool, food_id, user_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:perm:send:") => {
            let raw = data.strip_prefix("food:perm:send:").unwrap_or_default();
            let parts: Vec<&str> = raw.split(':').collect();
            if parts.len() == 3 {
                let food_id = parts[0].parse::<i64>().ok().filter(|id| *id > 0);
                let user_id = parts[1].parse::<i64>().ok().filter(|id| *id > 0);
                let permission = match parts[2] {
                    "manage" => Some(crate::resource_permissions::ResourcePermission::Manage),
                    "edit" => Some(crate::resource_permissions::ResourcePermission::Edit),
                    _ => None,
                };
                if let (Some(food_id), Some(user_id), Some(permission)) =
                    (food_id, user_id, permission)
                {
                    match create_and_send_food_invite(bot, pool, food_id, user_id, permission).await
                    {
                        Ok(()) => {
                            bot.send_message(chat_id, "✅ Invito inviato.").await?;
                            send_food_permissions_menu(bot, chat_id, pool, food_id).await?;
                        }
                        Err(error) => {
                            bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                        }
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:perm:revoke:") => {
            let ids = data
                .strip_prefix("food:perm:revoke:")
                .and_then(parse_two_positive_ids);
            if let Some((food_id, user_id)) = ids {
                match revoke_food_permission(pool, food_id, user_id).await {
                    Ok(()) => {
                        bot.send_message(chat_id, "✅ Permesso revocato.").await?;
                        send_food_permissions_menu(bot, chat_id, pool, food_id).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:invite:accept:") => {
            let invite_id = data
                .strip_prefix("food:invite:accept:")
                .and_then(parse_positive_id);
            let actor = identity::current_actor();
            if let (Some(invite_id), Some(user_id)) = (invite_id, actor.utente_id) {
                match crate::resource_permissions::accept_invite(pool, invite_id, user_id).await {
                    Ok(invite) if invite.resource_type == "alimento" => {
                        bot.send_message(chat_id, "✅ Invito accettato.").await?;
                        send_food_detail(bot, chat_id, pool, invite.resource_id).await?;
                    }
                    Ok(_) => {
                        bot.send_message(chat_id, "✅ Invito accettato.").await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:invite:decline:") => {
            let invite_id = data
                .strip_prefix("food:invite:decline:")
                .and_then(parse_positive_id);
            let actor = identity::current_actor();
            if let (Some(invite_id), Some(user_id)) = (invite_id, actor.utente_id) {
                match crate::resource_permissions::decline_invite(pool, invite_id, user_id).await {
                    Ok(()) => {
                        bot.send_message(chat_id, "Invito rifiutato.")
                            .reply_markup(food_menu_keyboard())
                            .await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:category:") => {
            let food_id = data
                .strip_prefix("food:category:")
                .and_then(|raw| raw.parse::<i64>().ok())
                .filter(|id| *id > 0);

            if let Some(food_id) = food_id {
                sessions.clear_chat(chat_id.0);
                send_food_category_menu(bot, chat_id, pool, food_id).await?;
            } else {
                bot.send_message(chat_id, "Alimento non valido.")
                    .reply_markup(food_menu_keyboard())
                    .await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:setcat:") => {
            let ids = data
                .strip_prefix("food:setcat:")
                .and_then(parse_two_positive_ids);

            if let Some((food_id, category_id)) = ids {
                match set_food_category(pool, food_id, category_id).await {
                    Ok(()) => {
                        bot.send_message(chat_id, "✅ Categoria alimento aggiornata.")
                            .await?;
                        send_food_detail(bot, chat_id, pool, food_id).await?;
                    }
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            food_id,
                            category_id,
                            "Cambio categoria alimento non riuscito"
                        );
                        bot.send_message(chat_id, format!("⚠️ {error}"))
                            .reply_markup(food_menu_keyboard())
                            .await?;
                    }
                }
            } else {
                bot.send_message(chat_id, "Categoria alimento non valida.")
                    .reply_markup(food_menu_keyboard())
                    .await?;
            }
            Ok(true)
        }
        "food:cancel" => {
            let had_active_operation = sessions.has_active(chat_id.0);
            sessions.clear_chat(chat_id.0);
            if had_active_operation {
                bot.send_message(
                    chat_id,
                    "❌ Operazione annullata. Nessuna modifica pendente è stata salvata.",
                )
                .await?;
            }
            show_menu(bot, chat_id).await?;
            Ok(true)
        }
        "menu:main" => {
            if sessions.has_active(chat_id.0) {
                sessions.clear_chat(chat_id.0);
                bot.send_message(
                    chat_id,
                    "❌ Operazione Alimentazione annullata. Nessuna modifica pendente è stata salvata.",
                )
                .await?;
            }
            Ok(false)
        }
        "food:back" => {
            sessions.clear_chat(chat_id.0);
            show_foods_menu(bot, chat_id, pool).await?;
            Ok(true)
        }
        "food:new:back:name" => {
            sessions.set(chat_id.0, FoodConversationState::Name);
            bot.send_message(
                chat_id,
                "➕ Nuovo alimento\n\nScrivi il nome dell'alimento.\nEsempio: Pollo\n\nPremi ❌ Annulla per uscire.",
            )
            .reply_markup(cancel_keyboard())
            .await?;
            Ok(true)
        }
        "food:new:back:unit" => {
            let Some(FoodConversationState::PendingCategory { name, .. }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            match unit_keyboard_from_db(pool).await {
                Ok(keyboard) => {
                    sessions.set(
                        chat_id.0,
                        FoodConversationState::Unit { name: name.clone() },
                    );
                    bot.send_message(
                        chat_id,
                        format!("🥕 {name}\n\nScegli di nuovo l'unità predefinita."),
                    )
                    .reply_markup(keyboard)
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore ritorno scelta unità");
                    bot.send_message(chat_id, "⚠️ Non riesco a leggere le unità di misura.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        "food:new:back:category" => {
            let Some(FoodConversationState::Visibility { name, unit_id, .. }) =
                sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };
            start_category_choice(bot, chat_id, chat_id.0, pool, sessions, name, unit_id).await?;
            Ok(true)
        }
        "food:new:back:visibility" => {
            let Some(FoodConversationState::Spaces {
                name,
                unit_id,
                category_id,
                ..
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            sessions.set(
                chat_id.0,
                FoodConversationState::Visibility {
                    name: name.clone(),
                    unit_id,
                    category_id,
                },
            );
            bot.send_message(
                chat_id,
                format!(
                    "👁 Visibilità di {name}\n\nL'alimento resterà sempre di tua proprietà.\nScegli dove renderlo visibile:"
                ),
            )
            .reply_markup(visibility_keyboard())
            .await?;
            Ok(true)
        }
        "food:new:visibility:private" => {
            let Some(FoodConversationState::Visibility {
                name,
                unit_id,
                category_id,
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };
            finish_creation(
                bot,
                chat_id,
                chat_id.0,
                pool,
                sessions,
                &name,
                unit_id,
                category_id,
                &[],
            )
            .await?;
            Ok(true)
        }
        "food:new:visibility:default" => {
            let Some(FoodConversationState::Visibility {
                name,
                unit_id,
                category_id,
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };
            let actor = identity::current_actor();
            match ensure_shareable_space(pool, actor.spazio_id).await {
                Ok(true) => {
                    finish_creation(
                        bot,
                        chat_id,
                        chat_id.0,
                        pool,
                        sessions,
                        &name,
                        unit_id,
                        category_id,
                        &[actor.spazio_id],
                    )
                    .await?;
                }
                Ok(false) => {
                    bot.send_message(
                        chat_id,
                        "⚠️ Non hai diritto di scrittura nello spazio predefinito. \
                         Puoi comunque salvare l'alimento come personale.",
                    )
                    .reply_markup(visibility_keyboard())
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore verifica spazio predefinito");
                    bot.send_message(chat_id, "⚠️ Non riesco a verificare lo spazio.")
                        .reply_markup(visibility_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        "food:new:visibility:all" => {
            let Some(FoodConversationState::Visibility {
                name,
                unit_id,
                category_id,
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };
            match list_shareable_spaces(pool).await {
                Ok(spaces) => {
                    let ids: Vec<i64> = spaces.iter().map(|space| space.id).collect();
                    finish_creation(
                        bot,
                        chat_id,
                        chat_id.0,
                        pool,
                        sessions,
                        &name,
                        unit_id,
                        category_id,
                        &ids,
                    )
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore elenco spazi condivisibili");
                    bot.send_message(chat_id, "⚠️ Non riesco a leggere i tuoi spazi.")
                        .reply_markup(visibility_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        "food:new:visibility:choose" => {
            let Some(FoodConversationState::Visibility {
                name,
                unit_id,
                category_id,
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            match list_shareable_spaces(pool).await {
                Ok(spaces) if spaces.is_empty() => {
                    bot.send_message(
                        chat_id,
                        "Non hai spazi nei quali puoi condividere alimenti. \
                         Puoi salvarlo come personale.",
                    )
                    .reply_markup(visibility_keyboard())
                    .await?;
                }
                Ok(spaces) => {
                    sessions.set(
                        chat_id.0,
                        FoodConversationState::Spaces {
                            name,
                            unit_id,
                            category_id,
                            selected: Vec::new(),
                        },
                    );
                    bot.send_message(
                        chat_id,
                        "🎛 Scegli gli spazi\n\n\
                         Tocca gli spazi nei quali vuoi rendere visibile \
                         l'alimento, poi premi ✅ Salva.",
                    )
                    .reply_markup(space_selection_keyboard(&spaces, &[]))
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore elenco spazi condivisibili");
                    bot.send_message(chat_id, "⚠️ Non riesco a leggere i tuoi spazi.")
                        .reply_markup(visibility_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        "food:new:spaces:save" => {
            let Some(FoodConversationState::Spaces {
                name,
                unit_id,
                category_id,
                selected,
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            finish_creation(
                bot,
                chat_id,
                chat_id.0,
                pool,
                sessions,
                &name,
                unit_id,
                category_id,
                &selected,
            )
            .await?;
            Ok(true)
        }
        _ if data.starts_with("food:new:space:") => {
            let Some(space_id) = data
                .strip_prefix("food:new:space:")
                .and_then(|raw| raw.parse::<i64>().ok())
                .filter(|id| *id > 0)
            else {
                bot.send_message(chat_id, "Pulsante spazio non valido.")
                    .await?;
                return Ok(true);
            };

            let Some(FoodConversationState::Spaces {
                name,
                unit_id,
                category_id,
                mut selected,
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            match list_shareable_spaces(pool).await {
                Ok(spaces) if spaces.iter().any(|space| space.id == space_id) => {
                    if let Some(index) = selected.iter().position(|id| *id == space_id) {
                        selected.remove(index);
                    } else {
                        selected.push(space_id);
                        selected.sort_unstable();
                        selected.dedup();
                    }

                    sessions.set(
                        chat_id.0,
                        FoodConversationState::Spaces {
                            name,
                            unit_id,
                            category_id,
                            selected: selected.clone(),
                        },
                    );

                    bot.send_message(
                        chat_id,
                        "🎛 Selezione aggiornata\n\n\
                         Tocca altri spazi oppure premi ✅ Salva.",
                    )
                    .reply_markup(space_selection_keyboard(&spaces, &selected))
                    .await?;
                }
                Ok(_) => {
                    bot.send_message(
                        chat_id,
                        "⚠️ Non puoi condividere alimenti in quello spazio.",
                    )
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore verifica spazio selezionato");
                    bot.send_message(chat_id, "⚠️ Non riesco a verificare lo spazio.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:new:unit:") => {
            let unit_id = data
                .strip_prefix("food:new:unit:")
                .and_then(|raw| raw.parse::<i64>().ok())
                .filter(|id| *id > 0);

            let Some(unit_id) = unit_id else {
                bot.send_message(chat_id, "Pulsante unità non valido.")
                    .reply_markup(food_menu_keyboard())
                    .await?;
                return Ok(true);
            };

            let Some(FoodConversationState::Unit { name }) = sessions.get(chat_id.0) else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            start_category_choice(bot, chat_id, chat_id.0, pool, sessions, name, Some(unit_id))
                .await?;
            Ok(true)
        }
        _ if data.starts_with("food:products:") => {
            let food_id = data
                .strip_prefix("food:products:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                sessions.clear_chat(chat_id.0);
                send_food_products(bot, chat_id, pool, food_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:new:") => {
            let food_id = data
                .strip_prefix("food:product:new:")
                .and_then(parse_positive_id);
            if let Some(food_id) = food_id {
                if can_edit_food_current(pool, food_id).await.unwrap_or(false) {
                    sessions.set(chat_id.0, FoodConversationState::ProductBrand { food_id });
                    bot.send_message(
                        chat_id,
                        "🏷 Marca del prodotto\n\nScrivi la marca.\nEsempio: Philadelphia",
                    )
                    .reply_markup(product_cancel_keyboard(food_id))
                    .await?;
                } else {
                    bot.send_message(
                        chat_id,
                        "⚠️ Non hai il permesso di associare prodotti a questo alimento.",
                    )
                    .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:changeunit:") => {
            let food_id = data
                .strip_prefix("food:product:changeunit:")
                .and_then(parse_positive_id);
            match (food_id, sessions.get(chat_id.0)) {
                (
                    Some(food_id),
                    Some(FoodConversationState::ProductQuantity {
                        food_id: state_food_id,
                        ..
                    }),
                ) if food_id == state_food_id => {
                    send_product_unit_choice(bot, chat_id, pool, food_id).await?;
                }
                _ => {
                    bot.send_message(chat_id, "Associazione prodotto non più attiva.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:quantity:") => {
            let food_id = data
                .strip_prefix("food:product:quantity:")
                .and_then(parse_positive_id);
            match (food_id, sessions.get(chat_id.0)) {
                (
                    Some(food_id),
                    Some(FoodConversationState::ProductQuantity {
                        food_id: state_food_id,
                        unit_id,
                        ..
                    }),
                ) if food_id == state_food_id => match get_unit_by_id(pool, unit_id).await {
                    Ok(Some(unit)) => {
                        send_product_quantity_prompt(bot, chat_id, food_id, &unit).await?;
                    }
                    _ => {
                        bot.send_message(chat_id, "⚠️ Unità confezione non disponibile.")
                            .reply_markup(product_cancel_keyboard(food_id))
                            .await?;
                    }
                },
                _ => {
                    bot.send_message(chat_id, "Associazione prodotto non più attiva.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:unit:") => {
            let unit_id = data
                .strip_prefix("food:product:unit:")
                .and_then(parse_positive_id);
            let state = sessions.get(chat_id.0);
            match (unit_id, state) {
                (
                    Some(unit_id),
                    Some(FoodConversationState::ProductQuantity {
                        food_id,
                        brand,
                        product_name,
                        ..
                    }),
                ) => match get_product_package_unit(pool, unit_id).await {
                    Ok(Some(unit)) => {
                        sessions.set(
                            chat_id.0,
                            FoodConversationState::ProductQuantity {
                                food_id,
                                brand,
                                product_name,
                                unit_id,
                            },
                        );
                        send_product_quantity_prompt(bot, chat_id, food_id, &unit).await?;
                    }
                    _ => {
                        bot.send_message(chat_id, "⚠️ Unità confezione non disponibile.")
                            .reply_markup(product_cancel_keyboard(food_id))
                            .await?;
                    }
                },
                _ => {
                    bot.send_message(chat_id, "Associazione prodotto non più attiva.")
                        .reply_markup(food_menu_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:view:") => {
            let product_id = data
                .strip_prefix("food:product:view:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(product_id) = product_id {
                send_product_detail(bot, chat_id, pool, product_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:formats:") => {
            let product_id = data
                .strip_prefix("food:product:formats:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(product_id) = product_id {
                send_product_formats(bot, chat_id, pool, product_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:format:new:") => {
            let product_id = data
                .strip_prefix("food:product:format:new:")
                .and_then(parse_positive_id);
            if let Some(product_id) = product_id {
                match get_product(pool, product_id).await {
                    Ok(Some(product))
                        if can_edit_food_current(pool, product.food_id)
                            .await
                            .unwrap_or(false) =>
                    {
                        match default_product_format_unit(pool, product_id).await {
                            Ok(unit) => {
                                sessions.set(
                                    chat_id.0,
                                    FoodConversationState::ProductFormatQuantity {
                                        product_id,
                                        unit_id: unit.id,
                                    },
                                );
                                send_product_format_quantity_prompt(
                                    bot, chat_id, product_id, &unit,
                                )
                                .await?;
                            }
                            Err(error) => {
                                tracing::error!(?error, product_id, "Errore unità nuovo formato");
                                bot.send_message(
                                    chat_id,
                                    "⚠️ Non riesco a preparare il nuovo formato.",
                                )
                                .await?;
                            }
                        }
                    }
                    Ok(Some(_)) => {
                        bot.send_message(
                            chat_id,
                            "⚠️ Non hai il permesso di aggiungere formati a questo prodotto.",
                        )
                        .await?;
                    }
                    Ok(None) => {
                        bot.send_message(chat_id, "Prodotto non disponibile.")
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, product_id, "Errore prodotto per nuovo formato");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:format:changeunit:") => {
            let product_id = data
                .strip_prefix("food:product:format:changeunit:")
                .and_then(parse_positive_id);
            match (product_id, sessions.get(chat_id.0)) {
                (
                    Some(product_id),
                    Some(FoodConversationState::ProductFormatQuantity {
                        product_id: state_product_id,
                        ..
                    }),
                ) if product_id == state_product_id => {
                    send_product_format_unit_choice(bot, chat_id, pool, product_id).await?;
                }
                _ => {
                    bot.send_message(chat_id, "Inserimento formato non più attivo.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:format:unit:") => {
            let raw = data
                .strip_prefix("food:product:format:unit:")
                .unwrap_or_default();
            let mut parts = raw.split(':');
            let product_id = parts.next().and_then(parse_positive_id);
            let unit_id = parts.next().and_then(parse_positive_id);
            if parts.next().is_some() {
                return Ok(true);
            }
            match (product_id, unit_id, sessions.get(chat_id.0)) {
                (
                    Some(product_id),
                    Some(unit_id),
                    Some(FoodConversationState::ProductFormatQuantity {
                        product_id: state_product_id,
                        ..
                    }),
                ) if product_id == state_product_id => {
                    match get_product_package_unit(pool, unit_id).await {
                        Ok(Some(unit)) => {
                            sessions.set(
                                chat_id.0,
                                FoodConversationState::ProductFormatQuantity {
                                    product_id,
                                    unit_id,
                                },
                            );
                            send_product_format_quantity_prompt(bot, chat_id, product_id, &unit)
                                .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, "⚠️ Unità formato non disponibile.")
                                .await?;
                        }
                    }
                }
                _ => {
                    bot.send_message(chat_id, "Inserimento formato non più attivo.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:format:view:") => {
            let format_id = data
                .strip_prefix("food:format:view:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(format_id) = format_id {
                send_product_format_detail(bot, chat_id, pool, format_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:format:delete:ask:") => {
            let format_id = data
                .strip_prefix("food:format:delete:ask:")
                .and_then(parse_positive_id);
            if let Some(format_id) = format_id {
                match get_product_format(pool, format_id).await {
                    Ok(Some(format)) => match get_product(pool, format.product_id).await {
                        Ok(Some(product))
                            if can_edit_food_current(pool, product.food_id)
                                .await
                                .unwrap_or(false) =>
                        {
                            bot.send_message(
                                chat_id,
                                format!(
                                    "🗑 Elimina formato\n\n{} · {}\nConfezione: {} {}\n\nIl formato verrà rimosso dalle opzioni attive. Il prodotto commerciale resterà disponibile.",
                                    product.brand,
                                    product.product_name,
                                    display_quantity(format.package_quantity),
                                    format.package_unit_symbol,
                                ),
                            )
                            .reply_markup(InlineKeyboardMarkup::new(vec![
                                vec![button(
                                    "🗑 Elimina formato",
                                    format!("food:format:delete:yes:{format_id}"),
                                )],
                                vec![
                                    button("❌ Annulla", format!("food:format:view:{format_id}")),
                                    button("🏠 Menù principale", "menu:main"),
                                ],
                            ]))
                            .await?;
                        }
                        Ok(Some(_)) => {
                            bot.send_message(
                                chat_id,
                                "⚠️ Non hai il permesso di eliminare questo formato.",
                            )
                            .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, "Prodotto non disponibile.")
                                .await?;
                        }
                    },
                    Ok(None) => {
                        bot.send_message(chat_id, "Formato non disponibile.")
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, format_id, "Errore eliminazione formato");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere questo formato.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:format:delete:yes:") => {
            let format_id = data
                .strip_prefix("food:format:delete:yes:")
                .and_then(parse_positive_id);
            if let Some(format_id) = format_id {
                match delete_product_format(pool, format_id).await {
                    Ok(product_id) => {
                        sessions.clear_chat(chat_id.0);
                        bot.send_message(chat_id, "✅ Formato eliminato.").await?;
                        send_product_formats(bot, chat_id, pool, product_id).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:format:edit:quantity:") => {
            let format_id = data
                .strip_prefix("food:format:edit:quantity:")
                .and_then(parse_positive_id);
            if let Some(format_id) = format_id {
                match get_product_format(pool, format_id).await {
                    Ok(Some(format)) => match get_product(pool, format.product_id).await {
                        Ok(Some(product))
                            if can_edit_food_current(pool, product.food_id)
                                .await
                                .unwrap_or(false) =>
                        {
                            match get_unit_by_id(pool, format.package_unit_id).await {
                                Ok(Some(unit)) => {
                                    sessions.set(
                                        chat_id.0,
                                        FoodConversationState::ProductFormatEditQuantity {
                                            product_id: product.id,
                                            format_id,
                                            unit_id: unit.id,
                                        },
                                    );
                                    send_product_format_edit_quantity_prompt(
                                        bot, chat_id, format_id, &unit,
                                    )
                                    .await?;
                                }
                                _ => {
                                    bot.send_message(chat_id, "⚠️ Unità formato non disponibile.")
                                        .await?;
                                }
                            }
                        }
                        Ok(Some(_)) => {
                            bot.send_message(
                                chat_id,
                                "⚠️ Non hai il permesso di modificare questo formato.",
                            )
                            .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, "Prodotto non disponibile.")
                                .await?;
                        }
                    },
                    Ok(None) => {
                        bot.send_message(chat_id, "Formato non disponibile.")
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, format_id, "Errore modifica formato");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere questo formato.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:format:edit:changeunit:") => {
            let format_id = data
                .strip_prefix("food:format:edit:changeunit:")
                .and_then(parse_positive_id);
            match (format_id, sessions.get(chat_id.0)) {
                (
                    Some(format_id),
                    Some(FoodConversationState::ProductFormatEditQuantity {
                        format_id: state_format_id,
                        ..
                    }),
                ) if format_id == state_format_id => {
                    send_product_format_edit_unit_choice(bot, chat_id, pool, format_id).await?;
                }
                _ => {
                    bot.send_message(chat_id, "Modifica formato non più attiva.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:format:edit:unit:") => {
            let raw = data
                .strip_prefix("food:format:edit:unit:")
                .unwrap_or_default();
            let mut parts = raw.split(':');
            let format_id = parts.next().and_then(parse_positive_id);
            let unit_id = parts.next().and_then(parse_positive_id);
            if parts.next().is_some() {
                return Ok(true);
            }
            match (format_id, unit_id, sessions.get(chat_id.0)) {
                (
                    Some(format_id),
                    Some(unit_id),
                    Some(FoodConversationState::ProductFormatEditQuantity {
                        product_id,
                        format_id: state_format_id,
                        ..
                    }),
                ) if format_id == state_format_id => {
                    match get_product_package_unit(pool, unit_id).await {
                        Ok(Some(unit)) => {
                            sessions.set(
                                chat_id.0,
                                FoodConversationState::ProductFormatEditQuantity {
                                    product_id,
                                    format_id,
                                    unit_id,
                                },
                            );
                            send_product_format_edit_quantity_prompt(
                                bot, chat_id, format_id, &unit,
                            )
                            .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, "⚠️ Unità formato non disponibile.")
                                .await?;
                        }
                    }
                }
                _ => {
                    bot.send_message(chat_id, "Modifica formato non più attiva.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:format:edit:ean:") => {
            let format_id = data
                .strip_prefix("food:format:edit:ean:")
                .and_then(parse_positive_id);
            if let Some(format_id) = format_id {
                match get_product_format(pool, format_id).await {
                    Ok(Some(format)) => match get_product(pool, format.product_id).await {
                        Ok(Some(product))
                            if can_edit_food_current(pool, product.food_id)
                                .await
                                .unwrap_or(false) =>
                        {
                            sessions.set(
                                chat_id.0,
                                FoodConversationState::ProductFormatEditEan {
                                    product_id: product.id,
                                    format_id,
                                },
                            );
                            let current = format.ean.as_deref().unwrap_or("non impostato");
                            bot.send_message(
                                chat_id,
                                format!(
                                    "🔢 Barcode / EAN del formato\n\nValore attuale: {current}\n\nScrivi il nuovo codice oppure - per rimuoverlo."
                                ),
                            )
                            .reply_markup(product_format_detail_keyboard(
                                format_id,
                                product.id,
                                true,
                            ))
                            .await?;
                        }
                        Ok(Some(_)) => {
                            bot.send_message(
                                chat_id,
                                "⚠️ Non hai il permesso di modificare questo formato.",
                            )
                            .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, "Prodotto non disponibile.")
                                .await?;
                        }
                    },
                    Ok(None) => {
                        bot.send_message(chat_id, "Formato non disponibile.")
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, format_id, "Errore EAN formato");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere questo formato.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:edit:brand:") => {
            let product_id = data
                .strip_prefix("food:product:edit:brand:")
                .and_then(parse_positive_id);
            if let Some(product_id) = product_id {
                match get_product(pool, product_id).await {
                    Ok(Some(product))
                        if can_edit_food_current(pool, product.food_id)
                            .await
                            .unwrap_or(false) =>
                    {
                        sessions.set(
                            chat_id.0,
                            FoodConversationState::ProductEditBrand { product_id },
                        );
                        bot.send_message(
                            chat_id,
                            format!(
                                "🏷 Modifica marca\n\nMarca attuale: {}\n\nScrivi la nuova marca.",
                                product.brand
                            ),
                        )
                        .reply_markup(product_edit_cancel_keyboard(product_id))
                        .await?;
                    }
                    Ok(Some(_)) => {
                        bot.send_message(
                            chat_id,
                            "⚠️ Non hai il permesso di modificare questo prodotto.",
                        )
                        .await?;
                    }
                    Ok(None) => {
                        bot.send_message(chat_id, "Prodotto non disponibile.")
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, product_id, "Errore modifica marca prodotto");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:edit:name:") => {
            let product_id = data
                .strip_prefix("food:product:edit:name:")
                .and_then(parse_positive_id);
            if let Some(product_id) = product_id {
                match get_product(pool, product_id).await {
                    Ok(Some(product))
                        if can_edit_food_current(pool, product.food_id)
                            .await
                            .unwrap_or(false) =>
                    {
                        sessions.set(
                            chat_id.0,
                            FoodConversationState::ProductEditName { product_id },
                        );
                        bot.send_message(
                            chat_id,
                            format!(
                                "🛒 Modifica nome commerciale\n\nNome attuale: {}\n\nScrivi il nuovo nome.",
                                product.product_name
                            ),
                        )
                        .reply_markup(product_edit_cancel_keyboard(product_id))
                        .await?;
                    }
                    Ok(Some(_)) => {
                        bot.send_message(
                            chat_id,
                            "⚠️ Non hai il permesso di modificare questo prodotto.",
                        )
                        .await?;
                    }
                    Ok(None) => {
                        bot.send_message(chat_id, "Prodotto non disponibile.")
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, product_id, "Errore modifica nome prodotto");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:edit:quantity:") => {
            let product_id = data
                .strip_prefix("food:product:edit:quantity:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(product_id) = product_id {
                bot.send_message(
                    chat_id,
                    "📦 Quantità, unità e barcode ora appartengono ai singoli formati.",
                )
                .await?;
                send_product_formats(bot, chat_id, pool, product_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:edit:ean:") => {
            let product_id = data
                .strip_prefix("food:product:edit:ean:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(product_id) = product_id {
                bot.send_message(
                    chat_id,
                    "🔢 Il barcode / EAN ora appartiene al singolo formato.",
                )
                .await?;
                send_product_formats(bot, chat_id, pool, product_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:edit:") => {
            let product_id = data
                .strip_prefix("food:product:edit:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(product_id) = product_id {
                send_product_edit_menu(bot, chat_id, pool, product_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:nutrition:edit:") => {
            let product_id = data
                .strip_prefix("food:product:nutrition:edit:")
                .and_then(parse_positive_id);
            if let Some(product_id) = product_id {
                match get_product(pool, product_id).await {
                    Ok(Some(product))
                        if can_edit_food_current(pool, product.food_id)
                            .await
                            .unwrap_or(false) =>
                    {
                        match default_nutrition_reference_unit(pool, &product).await {
                            Ok(unit) => {
                                sessions.set(
                                    chat_id.0,
                                    FoodConversationState::ProductNutritionInput {
                                        product_id,
                                        reference_unit_id: unit.id,
                                    },
                                );
                                send_nutrition_input_prompt(bot, chat_id, product_id, &unit)
                                    .await?;
                            }
                            Err(error) => {
                                tracing::error!(
                                    ?error,
                                    product_id,
                                    "Errore unità riferimento nutrizionale"
                                );
                                bot.send_message(
                                    chat_id,
                                    "⚠️ Non riesco a preparare l'inserimento nutrizionale.",
                                )
                                .await?;
                            }
                        }
                    }
                    Ok(Some(_)) => {
                        bot.send_message(
                            chat_id,
                            "⚠️ Non hai il permesso di modificare questo prodotto.",
                        )
                        .await?;
                    }
                    Ok(None) => {
                        bot.send_message(chat_id, "Prodotto non disponibile.")
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, product_id, "Errore prodotto per nutrizione");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                            .await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:nutrition:ref:") => {
            let raw = data
                .strip_prefix("food:product:nutrition:ref:")
                .unwrap_or_default();
            let mut parts = raw.split(':');
            let product_id = parts.next().and_then(parse_positive_id);
            let symbol = parts.next();
            if parts.next().is_some() {
                return Ok(true);
            }
            match (product_id, symbol, sessions.get(chat_id.0)) {
                (
                    Some(product_id),
                    Some(symbol @ ("g" | "ml")),
                    Some(FoodConversationState::ProductNutritionInput {
                        product_id: state_product_id,
                        ..
                    }),
                ) if product_id == state_product_id => {
                    match find_unit_by_text(pool, symbol).await {
                        Ok(Some(unit)) => {
                            sessions.set(
                                chat_id.0,
                                FoodConversationState::ProductNutritionInput {
                                    product_id,
                                    reference_unit_id: unit.id,
                                },
                            );
                            send_nutrition_input_prompt(bot, chat_id, product_id, &unit).await?;
                        }
                        _ => {
                            bot.send_message(chat_id, "⚠️ Unità nutrizionale non disponibile.")
                                .await?;
                        }
                    }
                }
                _ => {
                    bot.send_message(chat_id, "Inserimento nutrizionale non più attivo.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:nutrition:confirm:") => {
            let product_id = data
                .strip_prefix("food:product:nutrition:confirm:")
                .and_then(parse_positive_id);
            match (product_id, sessions.get(chat_id.0)) {
                (
                    Some(product_id),
                    Some(FoodConversationState::ProductNutritionConfirm {
                        product_id: state_product_id,
                        reference_unit_id,
                        values,
                    }),
                ) if product_id == state_product_id => {
                    match save_product_nutrition(pool, product_id, reference_unit_id, &values).await
                    {
                        Ok(()) => {
                            sessions.clear_chat(chat_id.0);
                            bot.send_message(chat_id, "✅ Valori nutrizionali aggiornati.")
                                .await?;
                            send_product_nutrition(bot, chat_id, pool, product_id).await?;
                        }
                        Err(error) => {
                            bot.send_message(chat_id, format!("⚠️ {error}"))
                                .reply_markup(nutrition_confirmation_keyboard(product_id))
                                .await?;
                        }
                    }
                }
                _ => {
                    bot.send_message(chat_id, "Conferma nutrizionale non più attiva.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:nutrition:modify:") => {
            let product_id = data
                .strip_prefix("food:product:nutrition:modify:")
                .and_then(parse_positive_id);
            match (product_id, sessions.get(chat_id.0)) {
                (
                    Some(product_id),
                    Some(FoodConversationState::ProductNutritionConfirm {
                        product_id: state_product_id,
                        reference_unit_id,
                        ..
                    }),
                ) if product_id == state_product_id => {
                    match get_unit_by_id(pool, reference_unit_id).await {
                        Ok(Some(unit)) => {
                            sessions.set(
                                chat_id.0,
                                FoodConversationState::ProductNutritionInput {
                                    product_id,
                                    reference_unit_id,
                                },
                            );
                            send_nutrition_input_prompt(bot, chat_id, product_id, &unit).await?;
                        }
                        _ => {
                            bot.send_message(chat_id, "⚠️ Unità nutrizionale non disponibile.")
                                .await?;
                        }
                    }
                }
                _ => {
                    bot.send_message(chat_id, "Conferma nutrizionale non più attiva.")
                        .await?;
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:nutrition:cancel:") => {
            let product_id = data
                .strip_prefix("food:product:nutrition:cancel:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(product_id) = product_id {
                send_product_nutrition(bot, chat_id, pool, product_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:nutrition:remove:") => {
            let product_id = data
                .strip_prefix("food:product:nutrition:remove:")
                .and_then(parse_positive_id);
            if let Some(product_id) = product_id {
                match remove_product_nutrition(pool, product_id).await {
                    Ok(()) => {
                        sessions.clear_chat(chat_id.0);
                        bot.send_message(chat_id, "✅ Valori nutrizionali rimossi.")
                            .await?;
                        send_product_nutrition(bot, chat_id, pool, product_id).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:nutrition:") => {
            let product_id = data
                .strip_prefix("food:product:nutrition:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(product_id) = product_id {
                send_product_nutrition(bot, chat_id, pool, product_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:product:cancel:") => {
            let food_id = data
                .strip_prefix("food:product:cancel:")
                .and_then(parse_positive_id);
            sessions.clear_chat(chat_id.0);
            if let Some(food_id) = food_id {
                send_food_products(bot, chat_id, pool, food_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("food:view:") => {
            let id = data
                .strip_prefix("food:view:")
                .and_then(|raw| raw.parse::<i64>().ok())
                .filter(|id| *id > 0);
            if let Some(id) = id {
                sessions.clear_chat(chat_id.0);
                send_food_detail(bot, chat_id, pool, id).await?;
            } else {
                bot.send_message(chat_id, "Pulsante alimento non valido.")
                    .reply_markup(food_menu_keyboard())
                    .await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn expired_creation(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "La creazione alimento non è più attiva.")
        .reply_markup(food_menu_keyboard())
        .await?;
    Ok(())
}

async fn start_unit_choice(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &FoodSessionStore,
    raw_name: &str,
) -> ResponseResult<()> {
    let Some(name) = clean_food_name(raw_name) else {
        sessions.set(raw_chat_id, FoodConversationState::Name);
        bot.send_message(
            chat_id,
            format!(
                "⚠️ Il nome deve contenere da 1 a {FOOD_NAME_MAX_CHARS} caratteri.\n\
                 Riprova oppure premi ❌ Annulla."
            ),
        )
        .reply_markup(cancel_keyboard())
        .await?;
        return Ok(());
    };

    let keyboard = match unit_keyboard_from_db(pool).await {
        Ok(keyboard) => keyboard,
        Err(error) => {
            tracing::error!(?error, "Errore caricamento unità alimento");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le unità di misura.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
    };

    sessions.set(
        raw_chat_id,
        FoodConversationState::Unit { name: name.clone() },
    );
    bot.send_message(
        chat_id,
        format!(
            "🥕 {name}\n\n\
             Scegli l'unità predefinita.\n\
             Puoi anche scrivere g, kg, ml, l, pz, cucchiaio, \
             cucchiaino o qb.\n\
             L'unità è obbligatoria per salvare l'alimento."
        ),
    )
    .reply_markup(keyboard)
    .await?;
    Ok(())
}

async fn start_category_choice(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &FoodSessionStore,
    name: String,
    unit_id: Option<i64>,
) -> ResponseResult<()> {
    match list_categories(pool).await {
        Ok(categories) if !categories.is_empty() => {
            sessions.set(
                raw_chat_id,
                FoodConversationState::PendingCategory {
                    name: name.clone(),
                    unit_id,
                },
            );
            bot.send_message(
                chat_id,
                format!("🏷 Categoria di {name}\n\nScegli la categoria prima di continuare."),
            )
            .reply_markup(category_before_save_keyboard(&categories))
            .await?;
        }
        Ok(_) => {
            bot.send_message(chat_id, "⚠️ Nessuna categoria alimentare disponibile.")
                .reply_markup(food_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore categorie durante la creazione alimento");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le categorie.")
                .reply_markup(food_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn start_visibility_choice(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &FoodSessionStore,
    name: String,
    unit_id: Option<i64>,
    category_id: i64,
) -> ResponseResult<()> {
    sessions.set(
        raw_chat_id,
        FoodConversationState::Visibility {
            name: name.clone(),
            unit_id,
            category_id,
        },
    );

    bot.send_message(
        chat_id,
        format!(
            "👁 Visibilità di {name}\n\n\
             L'alimento resterà sempre di tua proprietà.\n\
             Scegli dove renderlo visibile:"
        ),
    )
    .reply_markup(visibility_keyboard())
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn finish_creation(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &FoodSessionStore,
    name: &str,
    unit_id: Option<i64>,
    category_id: i64,
    shared_spaces: &[i64],
) -> ResponseResult<()> {
    match create_food_with_category(pool, name, unit_id, Some(category_id), shared_spaces).await {
        Ok(id) => {
            sessions.clear_chat(raw_chat_id);
            let visibility = if shared_spaces.is_empty() {
                "🔒 Solo personale".to_string()
            } else {
                format!("👥 Condiviso con {} spazio/i", shared_spaces.len())
            };
            bot.send_message(chat_id, format!("✅ Alimento creato: {name}\n{visibility}"))
                .reply_markup(food_created_keyboard(id))
                .await?;
        }
        Err(error) => {
            sessions.clear_chat(raw_chat_id);
            tracing::warn!(?error, food_name = name, "Creazione alimento non riuscita");
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(food_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn send_food_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    tracing::info!(
        chat_id = chat_id.0,
        user_id = ?actor.utente_id,
        space_id = actor.spazio_id,
        view_all = actor.view_all,
        page,
        "Alimentazione: preparazione elenco"
    );

    let total = match count_foods(pool, None, None).await {
        Ok(total) => total,
        Err(error) => {
            tracing::error!(?error, "Errore conteggio alimenti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere gli alimenti.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
    };
    if total == 0 {
        let title = "📋 Alimenti";
        bot.send_message(
            chat_id,
            format!("{title}\n\nNessun alimento disponibile nella vista corrente."),
        )
        .reply_markup(food_menu_keyboard())
        .await?;
        return Ok(());
    }

    let page = clamp_page(page, total);
    match list_foods_with_offset(pool, None, None, FOOD_PAGE_FETCH, page_offset(page)).await {
        Ok(rows) => {
            let (foods, _has_next) = split_food_page(rows);
            // C10: il nome della schermata coincide con quello del pulsante
            // che ci porta, cioe' `📋 Elenco alimenti`.
            let title = "📋 Elenco alimenti";
            let current_user = actor.utente_id;
            let mut text = liste::intestazione(title, total as i64, page);
            text.push_str(&note_elenco_alimenti(&foods, current_user));
            tracing::info!(
                chat_id = chat_id.0,
                user_id = ?actor.utente_id,
                page,
                rows = foods.len(),
                total,
                "Alimentazione: invio elenco"
            );
            bot.send_message(chat_id, text)
                .reply_markup(food_results_keyboard(&foods, page, total, current_user))
                .await?;
            tracing::info!(
                chat_id = chat_id.0,
                user_id = ?actor.utente_id,
                page,
                "Alimentazione: elenco inviato"
            );
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco alimenti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere gli alimenti.")
                .reply_markup(food_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn send_search_results(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    raw_query: &str,
    page: i64,
) -> ResponseResult<()> {
    let normalized = normalize_name(raw_query);
    if normalized.is_empty() {
        bot.send_message(chat_id, "⚠️ Scrivi almeno un termine da cercare.")
            .reply_markup(food_menu_keyboard())
            .await?;
        return Ok(());
    }

    let total = match count_foods(pool, Some(&normalized), None).await {
        Ok(total) => total,
        Err(error) => {
            tracing::error!(?error, "Errore conteggio ricerca alimenti");
            bot.send_message(chat_id, "⚠️ Non riesco a cercare gli alimenti.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
    };
    if total == 0 {
        bot.send_message(
            chat_id,
            format!("🔎 Nessun alimento trovato per: \"{raw_query}\""),
        )
        .reply_markup(food_menu_keyboard())
        .await?;
        return Ok(());
    }

    let page = clamp_page(page, total);
    match list_foods_with_offset(
        pool,
        Some(&normalized),
        None,
        FOOD_PAGE_FETCH,
        page_offset(page),
    )
    .await
    {
        Ok(rows) => {
            let (foods, _has_next) = split_food_page(rows);
            let current_user = identity::current_actor().utente_id;
            let mut text = liste::intestazione(
                &format!("🔎 Risultati per: \"{raw_query}\""),
                total as i64,
                page,
            );
            append_search_food_lines(pool, &mut text, &foods, current_user, &normalized).await;
            bot.send_message(chat_id, text)
                .reply_markup(food_search_results_keyboard(
                    &foods,
                    page,
                    total,
                    current_user,
                ))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore ricerca alimenti");
            bot.send_message(chat_id, "⚠️ Non riesco a cercare gli alimenti.")
                .reply_markup(food_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn send_food_filter_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    selected: &[i64],
) -> ResponseResult<()> {
    match list_categories(pool).await {
        Ok(categories) => {
            let selected_count = selected.len();
            bot.send_message(
                chat_id,
                format!(
                    "🏷 Filtra alimenti per categoria\n\n\
                     Seleziona una o più categorie e poi premi ✅ Applica.\n\
                     Con più categorie il filtro è inclusivo: ad esempio \
                     🥩 Carne + 🥬 Verdure mostra gli alimenti che appartengono \
                     ad almeno una delle due categorie.\n\n\
                     Selezionate: {selected_count}"
                ),
            )
            .reply_markup(category_filter_keyboard(&categories, selected))
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco categorie alimenti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le categorie.")
                .reply_markup(food_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn send_filtered_food_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    category_ids: &[i64],
    page: i64,
) -> ResponseResult<()> {
    if category_ids.is_empty() {
        send_food_list(bot, chat_id, pool, 0).await?;
        return Ok(());
    }

    let categories = match list_categories(pool).await {
        Ok(categories) => categories,
        Err(error) => {
            tracing::error!(?error, "Errore categorie filtro multiplo");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le categorie.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
    };

    let selected_categories: Vec<&CategoryRecord> = categories
        .iter()
        .filter(|category| category_ids.contains(&category.id))
        .collect();

    if selected_categories.is_empty() {
        bot.send_message(chat_id, "Nessuna categoria selezionata è disponibile.")
            .reply_markup(food_menu_keyboard())
            .await?;
        return Ok(());
    }

    let labels = selected_categories
        .iter()
        .map(|category| format!("{} {}", category.emoji, category.name))
        .collect::<Vec<_>>()
        .join(" + ");

    match list_foods_multi_categories_page(pool, category_ids, page).await {
        Ok((foods, _has_next, total, page)) => {
            if foods.is_empty() {
                bot.send_message(
                    chat_id,
                    format!("{labels}\n\nNessun alimento nella vista corrente."),
                )
                .reply_markup(empty_filtered_keyboard())
                .await?;
                return Ok(());
            }

            let current_user = identity::current_actor().utente_id;
            let mut text = liste::intestazione(&labels, total as i64, page);
            text.push_str(&note_elenco_alimenti(&foods, current_user));
            bot.send_message(chat_id, text)
                .reply_markup(food_filtered_results_keyboard(
                    &foods,
                    page,
                    total,
                    current_user,
                ))
                .await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                ?category_ids,
                "Errore filtro multi-categoria alimenti"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a filtrare gli alimenti.")
                .reply_markup(food_menu_keyboard())
                .await?;
        }
    }

    Ok(())
}

async fn list_foods_multi_categories_all(
    pool: &SqlitePool,
    category_ids: &[i64],
) -> Result<Vec<FoodRecord>> {
    let mut seen = HashSet::new();
    let mut foods = Vec::new();

    for category_id in category_ids {
        for food in list_foods(pool, None, Some(*category_id), 100_000).await? {
            if seen.insert(food.id) {
                foods.push(food);
            }
        }
    }

    foods.sort_by(|left, right| {
        food_origin_rank(left)
            .cmp(&food_origin_rank(right))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(foods)
}

async fn list_foods_multi_categories_page(
    pool: &SqlitePool,
    category_ids: &[i64],
    page: i64,
) -> Result<(Vec<FoodRecord>, bool, usize, i64)> {
    let foods = list_foods_multi_categories_all(pool, category_ids).await?;
    let total = foods.len();
    if total == 0 {
        return Ok((Vec::new(), false, 0, 0));
    }
    let page = clamp_page(page, total);
    let offset = page_offset(page) as usize;
    let mut page_rows = foods
        .into_iter()
        .skip(offset)
        .take(FOOD_PAGE_SIZE + 1)
        .collect::<Vec<_>>();
    let has_next = page_rows.len() > FOOD_PAGE_SIZE;
    page_rows.truncate(FOOD_PAGE_SIZE);
    Ok((page_rows, has_next, total, page))
}

async fn food_visible_to_user(pool: &SqlitePool, food_id: i64, user_id: i64) -> Result<bool> {
    let visible: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM alimenti a \
            WHERE a.id = ? AND a.archiviato = 0 \
              AND (\
                    a.proprietario_utente_id = ? \
                    OR EXISTS (\
                        SELECT 1 \
                        FROM alimento_spazi asp \
                        JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
                        WHERE asp.alimento_id = a.id \
                          AND ms.utente_id = ?\
                    )\
              )\
         )",
    )
    .bind(food_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare la visibilità alimento")?;
    Ok(visible)
}

async fn is_system_admin_user(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM utenti \
            WHERE id = ? \
              AND stato = 'attivo' \
              AND ruolo_sistema = 'admin'\
         )",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il ruolo di sistema")
}

async fn food_is_global_catalog(pool: &SqlitePool, food_id: i64) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM alimenti \
            WHERE id = ? AND archiviato = 0 AND catalogo_globale = 1\
         )",
    )
    .bind(food_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il catalogo globale")
}

async fn can_edit_food(pool: &SqlitePool, food_id: i64, user_id: i64) -> Result<bool> {
    if food_is_global_catalog(pool, food_id).await? {
        // Il catalogo base e' visibile a tutti, ma modificabile solo
        // dall'amministratore di sistema. Non viene trasformato in una
        // risorsa personale e non richiede permessi_risorsa.
        return is_system_admin_user(pool, user_id).await;
    }

    let owner: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM alimenti \
            WHERE id = ? AND archiviato = 0 \
              AND proprietario_utente_id = ?\
         )",
    )
    .bind(food_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il proprietario alimento")?;

    if owner {
        return Ok(true);
    }
    if !food_visible_to_user(pool, food_id, user_id).await? {
        return Ok(false);
    }
    crate::resource_permissions::has_edit_permission(pool, "alimento", food_id, user_id).await
}

async fn can_manage_food(pool: &SqlitePool, food_id: i64, user_id: i64) -> Result<bool> {
    if food_is_global_catalog(pool, food_id).await? {
        // Gli alimenti base sono globali per definizione: non hanno una
        // visibilita' per-spazio o collaboratori da amministrare.
        return Ok(false);
    }

    let owner: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM alimenti \
            WHERE id = ? AND archiviato = 0 \
              AND proprietario_utente_id = ?\
         )",
    )
    .bind(food_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il proprietario alimento")?;
    if owner {
        return Ok(true);
    }
    if !food_visible_to_user(pool, food_id, user_id).await? {
        return Ok(false);
    }
    crate::resource_permissions::has_manage_permission(pool, "alimento", food_id, user_id).await
}

async fn can_edit_food_current(pool: &SqlitePool, food_id: i64) -> Result<bool> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")?;
    can_edit_food(pool, food_id, user_id).await
}

async fn can_manage_food_current(pool: &SqlitePool, food_id: i64) -> Result<bool> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")?;
    can_manage_food(pool, food_id, user_id).await
}

async fn ensure_food_edit(pool: &SqlitePool, food_id: i64) -> Result<i64> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")?;
    if !can_edit_food(pool, food_id, user_id).await? {
        bail!("Non hai il permesso di modificare questo alimento");
    }
    Ok(user_id)
}

async fn ensure_food_manage(pool: &SqlitePool, food_id: i64) -> Result<i64> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")?;
    if !can_manage_food(pool, food_id, user_id).await? {
        bail!("Non hai il permesso di gestire visibilità e collaboratori di questo alimento");
    }
    Ok(user_id)
}

async fn update_food_name(pool: &SqlitePool, food_id: i64, raw_name: &str) -> Result<()> {
    ensure_food_edit(pool, food_id).await?;
    let name = clean_food_name(raw_name).context("Il nome dell'alimento non è valido")?;
    let normalized = normalize_name(&name);

    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM alimenti other \
            JOIN alimenti current ON current.id = ? \
            WHERE other.id <> current.id \
              AND other.archiviato = 0 \
              AND other.nome_normalizzato = ? \
              AND (\
                    other.catalogo_globale = 1 \
                    OR (current.proprietario_utente_id IS NOT NULL \
                        AND other.proprietario_utente_id = current.proprietario_utente_id) \
                    OR EXISTS (\
                        SELECT 1 \
                        FROM alimento_spazi mine \
                        JOIN alimento_spazi theirs ON theirs.spazio_id = mine.spazio_id \
                        WHERE mine.alimento_id = current.id \
                          AND theirs.alimento_id = other.id\
                    )\
              )\
         )",
    )
    .bind(food_id)
    .bind(&normalized)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare i nomi alimento")?;
    if duplicate {
        bail!("Esiste già un alimento con questo nome nel catalogo coinvolto");
    }

    sqlx::query("UPDATE alimenti SET nome = ?, nome_normalizzato = ? WHERE id = ?")
        .bind(name)
        .bind(normalized)
        .bind(food_id)
        .execute(pool)
        .await
        .context("Impossibile aggiornare il nome alimento")?;
    Ok(())
}

async fn update_food_unit(pool: &SqlitePool, food_id: i64, unit_id: i64) -> Result<()> {
    ensure_food_edit(pool, food_id).await?;
    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM unita_misura WHERE id = ? AND attiva = 1)")
            .bind(unit_id)
            .fetch_one(pool)
            .await?;
    if !valid {
        bail!("Unità di misura non disponibile");
    }
    sqlx::query("UPDATE alimenti SET unita_predefinita_id = ? WHERE id = ?")
        .bind(unit_id)
        .bind(food_id)
        .execute(pool)
        .await
        .context("Impossibile aggiornare l'unità alimento")?;
    Ok(())
}

async fn current_food_share_ids(pool: &SqlitePool, food_id: i64) -> Result<Vec<i64>> {
    sqlx::query_scalar::<_, i64>(
        "SELECT spazio_id FROM alimento_spazi WHERE alimento_id = ? ORDER BY spazio_id",
    )
    .bind(food_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi alimento")
}

async fn replace_food_shares(pool: &SqlitePool, food_id: i64, spaces: &[i64]) -> Result<()> {
    let user_id = ensure_food_manage(pool, food_id).await?;
    let mut selected = spaces.to_vec();
    selected.sort_unstable();
    selected.dedup();

    for space_id in &selected {
        if !user_can_share_to_space(pool, user_id, *space_id).await? {
            bail!("Non hai diritto di condividere l'alimento in uno degli spazi selezionati");
        }
    }

    let normalized: String = sqlx::query_scalar(
        "SELECT nome_normalizzato FROM alimenti WHERE id = ? AND archiviato = 0",
    )
    .bind(food_id)
    .fetch_optional(pool)
    .await?
    .context("Alimento non disponibile")?;

    for space_id in &selected {
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(\
                SELECT 1 FROM alimento_spazi asp \
                JOIN alimenti a ON a.id = asp.alimento_id \
                WHERE asp.spazio_id = ? AND asp.alimento_id <> ? \
                  AND a.archiviato = 0 AND a.nome_normalizzato = ?\
             )",
        )
        .bind(space_id)
        .bind(food_id)
        .bind(&normalized)
        .fetch_one(pool)
        .await?;
        if duplicate {
            bail!("Uno degli spazi selezionati possiede già un alimento con questo nome");
        }
    }

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM alimento_spazi WHERE alimento_id = ?")
        .bind(food_id)
        .execute(&mut *tx)
        .await?;
    for space_id in selected {
        sqlx::query(
            "INSERT INTO alimento_spazi (alimento_id, spazio_id, condiviso_da_utente_id) VALUES (?, ?, ?)",
        )
        .bind(food_id)
        .bind(space_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn send_food_edit_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
) -> ResponseResult<()> {
    let food = match get_food(pool, food_id).await {
        Ok(Some(food)) => food,
        _ => {
            bot.send_message(chat_id, "Alimento non disponibile.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
    };
    if !can_edit_food_current(pool, food_id).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ Non hai il permesso di modificare questo alimento.",
        )
        .reply_markup(food_menu_keyboard())
        .await?;
        return Ok(());
    }
    let can_manage = can_manage_food_current(pool, food_id)
        .await
        .unwrap_or(false);
    bot.send_message(chat_id, format!("✏️ Modifica {}", food.name))
        .reply_markup(food_edit_keyboard(food_id, can_manage))
        .await?;
    Ok(())
}

async fn send_edit_unit_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
) -> ResponseResult<()> {
    if !can_edit_food_current(pool, food_id).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ Non hai il permesso di modificare questo alimento.",
        )
        .await?;
        return Ok(());
    }
    match list_units(pool).await {
        Ok(units) => {
            bot.send_message(chat_id, "📏 Scegli la nuova unità predefinita.")
                .reply_markup(edit_unit_keyboard(food_id, &units))
                .await?;
        }
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}")).await?;
        }
    }
    Ok(())
}

async fn send_edit_visibility_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
) -> ResponseResult<()> {
    if !can_manage_food_current(pool, food_id)
        .await
        .unwrap_or(false)
    {
        bot.send_message(chat_id, "⚠️ Non hai il permesso di gestire la visibilità.")
            .await?;
        return Ok(());
    }
    bot.send_message(
        chat_id,
        "👥 Visibilità alimento\n\nScegli dove rendere visibile lo stesso alimento, senza crearne copie.",
    )
    .reply_markup(edit_visibility_keyboard(food_id))
    .await?;
    Ok(())
}

#[derive(Debug, Clone, FromRow)]
struct PermissionGrantRecord {
    user_id: i64,
    name: String,
    can_edit: i64,
    can_manage: i64,
}

#[derive(Debug, Clone, FromRow)]
struct PermissionCandidateRecord {
    user_id: i64,
    name: String,
}

#[derive(Debug, Clone, FromRow)]
struct PendingInviteRecord {
    name: String,
    can_manage: i64,
}

async fn send_food_permissions_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
) -> ResponseResult<()> {
    if !can_manage_food_current(pool, food_id)
        .await
        .unwrap_or(false)
    {
        bot.send_message(
            chat_id,
            "⚠️ Non hai il permesso di gestire i collaboratori.",
        )
        .await?;
        return Ok(());
    }

    let grants = sqlx::query_as::<_, PermissionGrantRecord>(
        "SELECT pr.utente_id AS user_id, u.nome_visualizzato AS name, \
                pr.puo_modificare AS can_edit, pr.puo_gestire_permessi AS can_manage \
         FROM permessi_risorsa pr \
         JOIN utenti u ON u.id = pr.utente_id \
         WHERE pr.tipo_risorsa = 'alimento' AND pr.risorsa_id = ? \
         ORDER BY u.nome_visualizzato COLLATE NOCASE",
    )
    .bind(food_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let pending = sqlx::query_as::<_, PendingInviteRecord>(
        "SELECT u.nome_visualizzato AS name, ir.puo_gestire_permessi AS can_manage \
         FROM inviti_risorsa ir \
         JOIN utenti u ON u.id = ir.invitato_utente_id \
         WHERE ir.tipo_risorsa = 'alimento' AND ir.risorsa_id = ? AND ir.stato = 'pendente' \
         ORDER BY u.nome_visualizzato COLLATE NOCASE",
    )
    .bind(food_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let candidates = sqlx::query_as::<_, PermissionCandidateRecord>(
        "SELECT DISTINCT u.id AS user_id, u.nome_visualizzato AS name \
         FROM utenti u \
         JOIN account_telegram at ON at.utente_id = u.id \
         JOIN membri_spazio ms ON ms.utente_id = u.id \
         JOIN alimento_spazi asp ON asp.spazio_id = ms.spazio_id AND asp.alimento_id = ? \
         JOIN alimenti a ON a.id = asp.alimento_id \
         WHERE u.stato = 'attivo' \
           AND u.id <> a.proprietario_utente_id \
           AND NOT EXISTS (\
                SELECT 1 FROM permessi_risorsa pr \
                WHERE pr.tipo_risorsa = 'alimento' AND pr.risorsa_id = ? AND pr.utente_id = u.id\
           ) \
           AND NOT EXISTS (\
                SELECT 1 FROM inviti_risorsa ir \
                WHERE ir.tipo_risorsa = 'alimento' AND ir.risorsa_id = ? \
                  AND ir.invitato_utente_id = u.id AND ir.stato = 'pendente'\
           ) \
         ORDER BY u.nome_visualizzato COLLATE NOCASE",
    )
    .bind(food_id)
    .bind(food_id)
    .bind(food_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut text = String::from(
        "🔐 Collaboratori alimento\n\nLa visibilità non concede automaticamente la modifica.",
    );
    if !grants.is_empty() {
        text.push_str("\n\nPermessi attivi:");
        for grant in &grants {
            let level = if grant.can_manage == 1 {
                "🛡 modifica + gestione"
            } else if grant.can_edit == 1 {
                "✏️ modifica"
            } else {
                "👁 sola visibilità"
            };
            text.push_str(&format!("\n• {} · {level}", grant.name));
        }
    }
    if !pending.is_empty() {
        text.push_str("\n\nInviti in attesa:");
        for invite in &pending {
            let level = if invite.can_manage == 1 {
                "🛡 gestione"
            } else {
                "✏️ modifica"
            };
            text.push_str(&format!("\n• {} · {level}", invite.name));
        }
    }
    if candidates.is_empty() {
        text.push_str("\n\nNessun nuovo utente invitabile. Per invitare qualcuno, l'alimento deve essere visibile in almeno uno spazio che condividete.");
    }

    bot.send_message(chat_id, text)
        .reply_markup(food_permissions_keyboard(food_id, &grants, &candidates))
        .await?;
    Ok(())
}

async fn send_permission_level_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
    user_id: i64,
) -> ResponseResult<()> {
    if !can_manage_food_current(pool, food_id)
        .await
        .unwrap_or(false)
    {
        bot.send_message(chat_id, "⚠️ Non hai il permesso di invitare collaboratori.")
            .await?;
        return Ok(());
    }
    let name: Option<String> = sqlx::query_scalar(
        "SELECT nome_visualizzato FROM utenti WHERE id = ? AND stato = 'attivo'",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let Some(name) = name else {
        bot.send_message(chat_id, "Utente non disponibile.").await?;
        return Ok(());
    };
    bot.send_message(
        chat_id,
        format!("Invita {name}\n\nScegli il livello di permesso."),
    )
    .reply_markup(permission_level_keyboard(food_id, user_id))
    .await?;
    Ok(())
}

async fn create_and_send_food_invite(
    bot: &Bot,
    pool: &SqlitePool,
    food_id: i64,
    target_user_id: i64,
    permission: crate::resource_permissions::ResourcePermission,
) -> Result<()> {
    let creator_id = ensure_food_manage(pool, food_id).await?;
    let chats: Vec<i64> =
        sqlx::query_scalar("SELECT DISTINCT chat_id FROM account_telegram WHERE utente_id = ?")
            .bind(target_user_id)
            .fetch_all(pool)
            .await?;
    if chats.is_empty() {
        bail!("Il destinatario non ha un account Telegram collegato");
    }

    let food_name: String = sqlx::query_scalar("SELECT nome FROM alimenti WHERE id = ?")
        .bind(food_id)
        .fetch_one(pool)
        .await?;
    let creator_name: String =
        sqlx::query_scalar("SELECT nome_visualizzato FROM utenti WHERE id = ?")
            .bind(creator_id)
            .fetch_one(pool)
            .await?;
    let invite_id = crate::resource_permissions::create_invite(
        pool,
        "alimento",
        food_id,
        target_user_id,
        creator_id,
        permission,
    )
    .await?;

    let level = match permission {
        crate::resource_permissions::ResourcePermission::Edit => "✏️ può modificare",
        crate::resource_permissions::ResourcePermission::Manage => {
            "🛡 può modificare e gestire i permessi"
        }
    };
    for raw_chat_id in chats {
        bot.send_message(
            ChatId(raw_chat_id),
            format!(
                "🔐 Invito collaborazione alimento\n\n{creator_name} ti invita a collaborare su «{food_name}».\nPermesso: {level}."
            ),
        )
        .reply_markup(permission_invite_keyboard(invite_id))
        .await
        .context("Invio invito Telegram fallito")?;
    }
    Ok(())
}

async fn revoke_food_permission(pool: &SqlitePool, food_id: i64, user_id: i64) -> Result<()> {
    ensure_food_manage(pool, food_id).await?;
    crate::resource_permissions::revoke_permission(pool, "alimento", food_id, user_id).await
}

async fn send_food_category_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
) -> ResponseResult<()> {
    let actor = identity::current_actor();
    let food = match get_food(pool, food_id).await {
        Ok(Some(food)) => food,
        Ok(None) => {
            bot.send_message(chat_id, "Alimento non disponibile.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, food_id, "Errore alimento per categoria");
            bot.send_message(chat_id, "Non riesco a leggere l'alimento.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
    };

    let can_edit = match actor.utente_id {
        Some(user_id) => can_edit_food(pool, food_id, user_id).await.unwrap_or(false),
        None => false,
    };
    if !can_edit {
        bot.send_message(
            chat_id,
            "Non hai il permesso di modificare la categoria di questo alimento.",
        )
        .reply_markup(food_detail_keyboard(food_id, false, false))
        .await?;
        return Ok(());
    }

    match list_categories(pool).await {
        Ok(categories) => {
            bot.send_message(
                chat_id,
                format!(
                    "Categoria di {}\n\nScegli la categoria principale.",
                    food.name
                ),
            )
            .reply_markup(category_assignment_keyboard(food_id, &categories))
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore categorie per alimento");
            bot.send_message(chat_id, "Non riesco a leggere le categorie.")
                .reply_markup(food_detail_keyboard(food_id, true, false))
                .await?;
        }
    }
    Ok(())
}

async fn list_categories(pool: &SqlitePool) -> Result<Vec<CategoryRecord>> {
    sqlx::query_as::<_, CategoryRecord>(
        "SELECT id, nome AS name, emoji \
         FROM categorie_alimento \
         WHERE attiva = 1 \
         ORDER BY ordinamento, id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le categorie alimentari")
}

async fn food_category_names(pool: &SqlitePool, food_id: i64) -> Result<Vec<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT c.emoji || ' ' || c.nome \
         FROM alimento_categorie ac \
         JOIN categorie_alimento c ON c.id = ac.categoria_id \
         WHERE ac.alimento_id = ? AND c.attiva = 1 \
         ORDER BY c.ordinamento, c.id",
    )
    .bind(food_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le categorie dell'alimento")
}

async fn set_food_category(pool: &SqlitePool, food_id: i64, category_id: i64) -> Result<()> {
    let user_id = identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")?;

    if !can_edit_food(pool, food_id, user_id).await? {
        bail!("Non hai il permesso di modificare questo alimento");
    }

    let category_valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM categorie_alimento \
            WHERE id = ? AND attiva = 1\
         )",
    )
    .bind(category_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare la categoria")?;
    if !category_valid {
        bail!("Categoria non disponibile");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare il cambio categoria")?;
    sqlx::query("DELETE FROM alimento_categorie WHERE alimento_id = ?")
        .bind(food_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile rimuovere la categoria precedente")?;
    sqlx::query(
        "INSERT INTO alimento_categorie (\
            alimento_id, categoria_id, assegnata_da_utente_id\
         ) VALUES (?, ?, ?)",
    )
    .bind(food_id)
    .bind(category_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile assegnare la nuova categoria")?;
    tx.commit()
        .await
        .context("Impossibile completare il cambio categoria")?;
    Ok(())
}

async fn send_food_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_food(pool, id).await {
        Ok(Some(food)) => {
            let actor = identity::current_actor();
            let is_owner = actor.utente_id == food.owner_user_id;
            let can_edit = match actor.utente_id {
                Some(user_id) => can_edit_food(pool, food.id, user_id).await.unwrap_or(false),
                None => false,
            };
            let can_manage = match actor.utente_id {
                Some(user_id) => can_manage_food(pool, food.id, user_id)
                    .await
                    .unwrap_or(false),
                None => false,
            };
            let mut lines = vec![
                format!("🥕 {}", food.name),
                String::new(),
                ownership_label(&food, actor.utente_id),
            ];

            if let Some(symbol) = food.unit_symbol.as_deref() {
                let name_or_code = food.unit_code.as_deref().unwrap_or(symbol);
                lines.push(format!(
                    "⚖️ Unità predefinita: {}",
                    unit_display_label(name_or_code, symbol)
                ));
            } else {
                lines.push("⚖️ Unità predefinita: non impostata".to_string());
            }

            match food_category_names(pool, food.id).await {
                Ok(categories) if categories.is_empty() => {
                    lines.push("🏷 Categoria: non assegnata".to_string());
                }
                Ok(categories) => {
                    lines.push(format!("🏷 Categoria: {}", categories.join(", ")));
                }
                Err(error) => {
                    tracing::error!(?error, food_id = food.id, "Errore categoria alimento");
                }
            }
            if food.global_catalog == 0 {
                match visible_share_names(pool, food.id, actor.utente_id, is_owner).await {
                    Ok(names) if names.is_empty() => {
                        lines.push("🔒 Visibilità: solo personale".to_string());
                    }
                    Ok(names) => {
                        lines.push(format!("👥 Visibile in: {}", names.join(", ")));
                    }
                    Err(error) => {
                        tracing::error!(?error, food_id = food.id, "Errore spazi alimento");
                    }
                }
            }

            if let Some(description) = food.description.as_deref() {
                lines.push(format!("📝 {description}"));
            }

            bot.send_message(chat_id, lines.join("\n"))
                .reply_markup(food_detail_keyboard(food.id, can_edit, can_manage))
                .await?;
        }
        Ok(None) => {
            bot.send_message(
                chat_id,
                "Alimento non disponibile nella vista corrente.".to_string(),
            )
            .reply_markup(food_menu_keyboard())
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, food_id = id, "Errore dettaglio alimento");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo alimento.")
                .reply_markup(food_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
async fn create_food(
    pool: &SqlitePool,
    raw_name: &str,
    unit_id: Option<i64>,
    shared_spaces: &[i64],
) -> Result<i64> {
    create_food_with_category(pool, raw_name, unit_id, None, shared_spaces).await
}

async fn create_food_with_category(
    pool: &SqlitePool,
    raw_name: &str,
    unit_id: Option<i64>,
    category_id: Option<i64>,
    shared_spaces: &[i64],
) -> Result<i64> {
    let actor = identity::current_actor();
    let owner_user_id = actor.utente_id.context("Identità utente non disponibile")?;

    let name = clean_food_name(raw_name).context("Il nome dell'alimento non è valido")?;
    let normalized = normalize_name(&name);

    let unit_id = unit_id.context("Scegli un'unità di misura prima di salvare")?;

    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM unita_misura WHERE id = ? AND attiva = 1)")
            .bind(unit_id)
            .fetch_one(pool)
            .await
            .context("Impossibile verificare l'unità di misura")?;
    if !valid {
        bail!("Unità di misura non disponibile");
    }

    if let Some(category_id) = category_id {
        let category_valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM categorie_alimento WHERE id = ? AND attiva = 1)",
        )
        .bind(category_id)
        .fetch_one(pool)
        .await
        .context("Impossibile verificare la categoria alimento")?;
        if !category_valid {
            bail!("Categoria alimento non disponibile");
        }
    }

    let global_duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM alimenti \
            WHERE catalogo_globale = 1 \
              AND archiviato = 0 \
              AND nome_normalizzato = ?\
         )",
    )
    .bind(&normalized)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il catalogo globale")?;
    if global_duplicate {
        bail!("Esiste già un alimento con questo nome nel catalogo globale");
    }

    let owner_duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM alimenti \
            WHERE proprietario_utente_id = ? \
              AND archiviato = 0 \
              AND nome_normalizzato = ?\
         )",
    )
    .bind(owner_user_id)
    .bind(&normalized)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il catalogo personale")?;
    if owner_duplicate {
        bail!("Hai già un alimento con questo nome");
    }

    let mut unique_spaces = shared_spaces.to_vec();
    unique_spaces.sort_unstable();
    unique_spaces.dedup();

    for space_id in &unique_spaces {
        if !user_can_share_to_space(pool, owner_user_id, *space_id).await? {
            bail!("Non hai diritto di condividere alimenti in uno degli spazi selezionati");
        }

        let duplicate_in_space: bool = sqlx::query_scalar(
            "SELECT EXISTS(\
                SELECT 1 \
                FROM alimento_spazi asp \
                JOIN alimenti a ON a.id = asp.alimento_id \
                WHERE asp.spazio_id = ? \
                  AND a.archiviato = 0 \
                  AND a.nome_normalizzato = ?\
             )",
        )
        .bind(space_id)
        .bind(&normalized)
        .fetch_one(pool)
        .await
        .context("Impossibile verificare gli alimenti condivisi")?;

        if duplicate_in_space {
            bail!("Uno degli spazi selezionati possiede già un alimento con questo nome");
        }
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare il salvataggio alimento")?;

    let result = sqlx::query(
        "INSERT INTO alimenti (\
            spazio_id, nome, nome_normalizzato, unita_predefinita_id, \
            creato_da_utente_id, proprietario_utente_id, catalogo_globale\
         ) VALUES (NULL, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&name)
    .bind(&normalized)
    .bind(unit_id)
    .bind(owner_user_id)
    .bind(owner_user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile salvare l'alimento")?;

    let food_id = result.last_insert_rowid();

    if let Some(category_id) = category_id {
        sqlx::query("DELETE FROM alimento_categorie WHERE alimento_id = ?")
            .bind(food_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile sostituire la categoria automatica")?;
        sqlx::query(
            "INSERT INTO alimento_categorie (\
                alimento_id, categoria_id, assegnata_da_utente_id\
             ) VALUES (?, ?, ?)",
        )
        .bind(food_id)
        .bind(category_id)
        .bind(owner_user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile assegnare la categoria scelta")?;
    }

    for space_id in unique_spaces {
        sqlx::query(
            "INSERT INTO alimento_spazi (\
                alimento_id, spazio_id, condiviso_da_utente_id\
             ) VALUES (?, ?, ?)",
        )
        .bind(food_id)
        .bind(space_id)
        .bind(owner_user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile condividere l'alimento")?;
    }

    tx.commit()
        .await
        .context("Impossibile completare il salvataggio alimento")?;

    Ok(food_id)
}

async fn user_can_share_to_space(pool: &SqlitePool, user_id: i64, space_id: i64) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 \
            FROM membri_spazio \
            WHERE utente_id = ? \
              AND spazio_id = ? \
              AND ruolo IN ('proprietario', 'amministratore', 'membro')\
         )",
    )
    .bind(user_id)
    .bind(space_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare i permessi dello spazio")
}

async fn ensure_shareable_space(pool: &SqlitePool, space_id: i64) -> Result<bool> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(false);
    };
    user_can_share_to_space(pool, user_id, space_id).await
}

async fn list_shareable_spaces(pool: &SqlitePool) -> Result<Vec<SpaceRecord>> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Identità utente non disponibile")?;

    sqlx::query_as::<_, SpaceRecord>(
        "SELECT s.id, s.nome AS name \
         FROM membri_spazio ms \
         JOIN spazi s ON s.id = ms.spazio_id \
         WHERE ms.utente_id = ? \
           AND ms.ruolo IN ('proprietario', 'amministratore', 'membro') \
         ORDER BY CASE WHEN s.id = ? THEN 0 ELSE 1 END, \
                  s.nome COLLATE NOCASE, s.id",
    )
    .bind(user_id)
    .bind(actor.spazio_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi condivisibili")
}

async fn count_foods(
    pool: &SqlitePool,
    normalized_search: Option<&str>,
    category_id: Option<i64>,
) -> Result<usize> {
    let actor = identity::current_actor();
    let search = normalized_search
        .map(normalize_name)
        .filter(|value| !value.is_empty());

    let total: i64 = if let Some(user_id) = actor.utente_id {
        if actor.view_all {
            sqlx::query_scalar(
                "SELECT COUNT(*) \
                 FROM alimenti a \
                 WHERE a.archiviato = 0 \
                   AND (\
                        a.catalogo_globale = 1 \
                        OR a.proprietario_utente_id = ? \
                        OR EXISTS (\
                            SELECT 1 \
                            FROM alimento_spazi asp \
                            JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
                            WHERE asp.alimento_id = a.id \
                              AND ms.utente_id = ?\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR instr(a.nome_normalizzato, ?) > 0 \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_alias aa \
                            WHERE aa.alimento_id = a.id \
                              AND instr(aa.alias_normalizzato, ?) > 0\
                        )\
                        OR EXISTS (\
                            SELECT 1 FROM prodotti_alimentari p \
                            WHERE p.alimento_id = a.id \
                              AND p.attivo = 1 \
                              AND instr(\
                                    p.marca_normalizzata || ' ' || p.nome_commerciale_normalizzato, \
                                    ?\
                                  ) > 0\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_categorie ac \
                            WHERE ac.alimento_id = a.id \
                              AND ac.categoria_id = ?\
                        )\
                   )",
            )
            .bind(user_id)
            .bind(user_id)
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(category_id)
            .bind(category_id)
            .fetch_one(pool)
            .await
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) \
                 FROM alimenti a \
                 WHERE a.archiviato = 0 \
                   AND (\
                        a.catalogo_globale = 1 \
                        OR a.proprietario_utente_id = ? \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_spazi asp \
                            WHERE asp.alimento_id = a.id \
                              AND asp.spazio_id = ?\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR instr(a.nome_normalizzato, ?) > 0 \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_alias aa \
                            WHERE aa.alimento_id = a.id \
                              AND instr(aa.alias_normalizzato, ?) > 0\
                        )\
                        OR EXISTS (\
                            SELECT 1 FROM prodotti_alimentari p \
                            WHERE p.alimento_id = a.id \
                              AND p.attivo = 1 \
                              AND instr(\
                                    p.marca_normalizzata || ' ' || p.nome_commerciale_normalizzato, \
                                    ?\
                                  ) > 0\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_categorie ac \
                            WHERE ac.alimento_id = a.id \
                              AND ac.categoria_id = ?\
                        )\
                   )",
            )
            .bind(user_id)
            .bind(actor.spazio_id)
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(category_id)
            .bind(category_id)
            .fetch_one(pool)
            .await
        }
    } else {
        sqlx::query_scalar(
            "SELECT COUNT(*) \
             FROM alimenti a \
             WHERE a.archiviato = 0 \
               AND a.catalogo_globale = 1 \
               AND (\
                    ? IS NULL \
                    OR instr(a.nome_normalizzato, ?) > 0 \
                    OR EXISTS (\
                        SELECT 1 FROM alimento_alias aa \
                        WHERE aa.alimento_id = a.id \
                          AND instr(aa.alias_normalizzato, ?) > 0\
                    )\
                    OR EXISTS (\
                        SELECT 1 FROM prodotti_alimentari p \
                        WHERE p.alimento_id = a.id \
                          AND p.attivo = 1 \
                          AND instr(\
                                p.marca_normalizzata || ' ' || p.nome_commerciale_normalizzato, \
                                ?\
                              ) > 0\
                    )\
               ) \
               AND (\
                    ? IS NULL \
                    OR EXISTS (\
                        SELECT 1 FROM alimento_categorie ac \
                        WHERE ac.alimento_id = a.id \
                          AND ac.categoria_id = ?\
                    )\
               )",
        )
        .bind(search.as_deref())
        .bind(search.as_deref())
        .bind(search.as_deref())
        .bind(search.as_deref())
        .bind(category_id)
        .bind(category_id)
        .fetch_one(pool)
        .await
    }
    .context("Impossibile contare il catalogo alimenti")?;

    usize::try_from(total).context("Conteggio alimenti non rappresentabile in memoria")
}

async fn list_foods(
    pool: &SqlitePool,
    normalized_search: Option<&str>,
    category_id: Option<i64>,
    limit: i64,
) -> Result<Vec<FoodRecord>> {
    list_foods_with_offset(pool, normalized_search, category_id, limit, 0).await
}

async fn list_foods_with_offset(
    pool: &SqlitePool,
    normalized_search: Option<&str>,
    category_id: Option<i64>,
    limit: i64,
    offset: i64,
) -> Result<Vec<FoodRecord>> {
    let actor = identity::current_actor();
    let search = normalized_search
        .map(normalize_name)
        .filter(|value| !value.is_empty());

    let select = "\
        SELECT \
            a.id, \
            a.nome AS name, \
            a.descrizione AS description, \
            um.codice AS unit_code, \
            um.simbolo AS unit_symbol, \
            a.proprietario_utente_id AS owner_user_id, \
            u.nome_visualizzato AS owner_name, \
            a.catalogo_globale AS global_catalog \
        FROM alimenti a \
        LEFT JOIN unita_misura um ON um.id = a.unita_predefinita_id \
        LEFT JOIN utenti u ON u.id = a.proprietario_utente_id ";

    let rows = if let Some(user_id) = actor.utente_id {
        if actor.view_all {
            let sql = format!(
                "{select}\
                 WHERE a.archiviato = 0 \
                   AND (\
                        a.catalogo_globale = 1 \
                        OR a.proprietario_utente_id = ? \
                        OR EXISTS (\
                            SELECT 1 \
                            FROM alimento_spazi asp \
                            JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
                            WHERE asp.alimento_id = a.id \
                              AND ms.utente_id = ?\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR instr(a.nome_normalizzato, ?) > 0 \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_alias aa \
                            WHERE aa.alimento_id = a.id \
                              AND instr(aa.alias_normalizzato, ?) > 0\
                        )\
                        OR EXISTS (\
                            SELECT 1 FROM prodotti_alimentari p \
                            WHERE p.alimento_id = a.id \
                              AND p.attivo = 1 \
                              AND instr(\
                                    p.marca_normalizzata || ' ' || p.nome_commerciale_normalizzato, \
                                    ?\
                                  ) > 0\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_categorie ac \
                            WHERE ac.alimento_id = a.id \
                              AND ac.categoria_id = ?\
                        )\
                   ) \
                 ORDER BY CASE \
                            WHEN a.catalogo_globale = 1 THEN 2 \
                            WHEN a.proprietario_utente_id = ? THEN 0 \
                            ELSE 1 \
                          END, \
                          a.nome COLLATE NOCASE, a.id \
                 LIMIT ? OFFSET ?"
            );
            sqlx::query_as::<_, FoodRecord>(&sql)
                .bind(user_id)
                .bind(user_id)
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(category_id)
                .bind(category_id)
                .bind(user_id)
                .bind(limit)
                .bind(offset.max(0))
                .fetch_all(pool)
                .await
        } else {
            let sql = format!(
                "{select}\
                 WHERE a.archiviato = 0 \
                   AND (\
                        a.catalogo_globale = 1 \
                        OR a.proprietario_utente_id = ? \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_spazi asp \
                            WHERE asp.alimento_id = a.id \
                              AND asp.spazio_id = ?\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR instr(a.nome_normalizzato, ?) > 0 \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_alias aa \
                            WHERE aa.alimento_id = a.id \
                              AND instr(aa.alias_normalizzato, ?) > 0\
                        )\
                        OR EXISTS (\
                            SELECT 1 FROM prodotti_alimentari p \
                            WHERE p.alimento_id = a.id \
                              AND p.attivo = 1 \
                              AND instr(\
                                    p.marca_normalizzata || ' ' || p.nome_commerciale_normalizzato, \
                                    ?\
                                  ) > 0\
                        )\
                   ) \
                   AND (\
                        ? IS NULL \
                        OR EXISTS (\
                            SELECT 1 FROM alimento_categorie ac \
                            WHERE ac.alimento_id = a.id \
                              AND ac.categoria_id = ?\
                        )\
                   ) \
                 ORDER BY CASE \
                            WHEN a.catalogo_globale = 1 THEN 2 \
                            WHEN a.proprietario_utente_id = ? THEN 0 \
                            ELSE 1 \
                          END, \
                          a.nome COLLATE NOCASE, a.id \
                 LIMIT ? OFFSET ?"
            );
            sqlx::query_as::<_, FoodRecord>(&sql)
                .bind(user_id)
                .bind(actor.spazio_id)
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(category_id)
                .bind(category_id)
                .bind(user_id)
                .bind(limit)
                .bind(offset.max(0))
                .fetch_all(pool)
                .await
        }
    } else {
        let sql = format!(
            "{select}\
             WHERE a.archiviato = 0 \
               AND a.catalogo_globale = 1 \
               AND (\
                    ? IS NULL \
                    OR instr(a.nome_normalizzato, ?) > 0 \
                    OR EXISTS (\
                        SELECT 1 FROM alimento_alias aa \
                        WHERE aa.alimento_id = a.id \
                          AND instr(aa.alias_normalizzato, ?) > 0\
                    )\
                    OR EXISTS (\
                        SELECT 1 FROM prodotti_alimentari p \
                        WHERE p.alimento_id = a.id \
                          AND p.attivo = 1 \
                          AND instr(\
                                p.marca_normalizzata || ' ' || p.nome_commerciale_normalizzato, \
                                ?\
                              ) > 0\
                    )\
               ) \
               AND (\
                    ? IS NULL \
                    OR EXISTS (\
                        SELECT 1 FROM alimento_categorie ac \
                        WHERE ac.alimento_id = a.id \
                          AND ac.categoria_id = ?\
                    )\
               ) \
             ORDER BY a.nome COLLATE NOCASE, a.id \
             LIMIT ? OFFSET ?"
        );
        sqlx::query_as::<_, FoodRecord>(&sql)
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(category_id)
            .bind(category_id)
            .bind(limit)
            .bind(offset.max(0))
            .fetch_all(pool)
            .await
    }
    .context("Impossibile leggere il catalogo alimenti")?;

    Ok(rows)
}

async fn get_food(pool: &SqlitePool, id: i64) -> Result<Option<FoodRecord>> {
    let actor = identity::current_actor();
    let select = "\
        SELECT \
            a.id, \
            a.nome AS name, \
            a.descrizione AS description, \
            um.codice AS unit_code, \
            um.simbolo AS unit_symbol, \
            a.proprietario_utente_id AS owner_user_id, \
            u.nome_visualizzato AS owner_name, \
            a.catalogo_globale AS global_catalog \
        FROM alimenti a \
        LEFT JOIN unita_misura um ON um.id = a.unita_predefinita_id \
        LEFT JOIN utenti u ON u.id = a.proprietario_utente_id \
        WHERE a.id = ? AND a.archiviato = 0 ";

    let row = if let Some(user_id) = actor.utente_id {
        if actor.view_all {
            let sql = format!(
                "{select}\
                 AND (\
                    a.catalogo_globale = 1 \
                    OR a.proprietario_utente_id = ? \
                    OR EXISTS (\
                        SELECT 1 \
                        FROM alimento_spazi asp \
                        JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
                        WHERE asp.alimento_id = a.id \
                          AND ms.utente_id = ?\
                    )\
                 )"
            );
            sqlx::query_as::<_, FoodRecord>(&sql)
                .bind(id)
                .bind(user_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await
        } else {
            let sql = format!(
                "{select}\
                 AND (\
                    a.catalogo_globale = 1 \
                    OR a.proprietario_utente_id = ? \
                    OR EXISTS (\
                        SELECT 1 FROM alimento_spazi asp \
                        WHERE asp.alimento_id = a.id \
                          AND asp.spazio_id = ?\
                    )\
                 )"
            );
            sqlx::query_as::<_, FoodRecord>(&sql)
                .bind(id)
                .bind(user_id)
                .bind(actor.spazio_id)
                .fetch_optional(pool)
                .await
        }
    } else {
        let sql = format!("{select} AND a.catalogo_globale = 1");
        sqlx::query_as::<_, FoodRecord>(&sql)
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    .context("Impossibile leggere l'alimento")?;

    Ok(row)
}

async fn visible_share_names(
    pool: &SqlitePool,
    food_id: i64,
    user_id: Option<i64>,
    is_owner: bool,
) -> Result<Vec<String>> {
    if is_owner {
        return sqlx::query_scalar::<_, String>(
            "SELECT s.nome \
             FROM alimento_spazi asp \
             JOIN spazi s ON s.id = asp.spazio_id \
             WHERE asp.alimento_id = ? \
             ORDER BY s.nome COLLATE NOCASE, s.id",
        )
        .bind(food_id)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere gli spazi condivisi");
    }

    let Some(user_id) = user_id else {
        return Ok(Vec::new());
    };

    sqlx::query_scalar::<_, String>(
        "SELECT s.nome \
         FROM alimento_spazi asp \
         JOIN spazi s ON s.id = asp.spazio_id \
         JOIN membri_spazio ms \
           ON ms.spazio_id = asp.spazio_id \
          AND ms.utente_id = ? \
         WHERE asp.alimento_id = ? \
         ORDER BY s.nome COLLATE NOCASE, s.id",
    )
    .bind(user_id)
    .bind(food_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi condivisi")
}

async fn list_units(pool: &SqlitePool) -> Result<Vec<UnitRecord>> {
    sqlx::query_as::<_, UnitRecord>(
        "SELECT id, nome, simbolo \
         FROM unita_misura \
         WHERE attiva = 1 \
         ORDER BY ordinamento, id",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le unità di misura")
}

async fn find_unit_by_text(pool: &SqlitePool, raw: &str) -> Result<Option<UnitRecord>> {
    let clean = raw.trim().to_lowercase();
    if clean.is_empty() {
        return Ok(None);
    }
    let compact = clean.replace('.', "");

    sqlx::query_as::<_, UnitRecord>(
        "SELECT id, nome, simbolo \
         FROM unita_misura \
         WHERE attiva = 1 \
           AND (\
                lower(codice) = ? \
                OR lower(codice) = ? \
                OR lower(simbolo) = ? \
                OR lower(nome) = ?\
           ) \
         ORDER BY ordinamento, id \
         LIMIT 1",
    )
    .bind(&clean)
    .bind(&compact)
    .bind(&clean)
    .bind(&clean)
    .fetch_optional(pool)
    .await
    .context("Impossibile cercare l'unità di misura")
}

async fn unit_keyboard_from_db(pool: &SqlitePool) -> Result<InlineKeyboardMarkup> {
    let units = list_units(pool).await?;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for pair in units.chunks(2) {
        rows.push(
            pair.iter()
                .map(|unit| {
                    InlineKeyboardButton::callback(
                        unit_display_label(&unit.nome, &unit.simbolo),
                        format!("food:new:unit:{}", unit.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", "food:new:back:name"),
        button("❌ Annulla", "food:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    Ok(InlineKeyboardMarkup::new(rows))
}

fn unit_display_label(name_or_code: &str, symbol: &str) -> String {
    match symbol {
        "g" => "grammi (g)".to_string(),
        "kg" => "chilogrammi (kg)".to_string(),
        "ml" => "millilitri (ml)".to_string(),
        "l" => "litri (l)".to_string(),
        "pz" => "pezzi (pz)".to_string(),
        "cucchiaio" => "cucchiaio".to_string(),
        "cucchiaino" => "cucchiaino".to_string(),
        "q.b." => "quanto basta (q.b.)".to_string(),
        _ if name_or_code.eq_ignore_ascii_case(symbol) => name_or_code.to_string(),
        _ => format!("{name_or_code} ({symbol})"),
    }
}

fn visibility_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("🔒 Solo mio", "food:new:visibility:private")],
        vec![button(
            "🎯 Spazio predefinito",
            "food:new:visibility:default",
        )],
        vec![button("🌐 Tutti i miei spazi", "food:new:visibility:all")],
        vec![button("🎛 Scegli spazi", "food:new:visibility:choose")],
        vec![
            button("⬅️ Indietro", "food:new:back:category"),
            button("❌ Annulla", "food:cancel"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn space_selection_keyboard(spaces: &[SpaceRecord], selected: &[i64]) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for space in spaces {
        let checked = selected.contains(&space.id);
        rows.push(vec![button(
            format!("{} {}", if checked { "☑️" } else { "⬜" }, space.name),
            format!("food:new:space:{}", space.id),
        )]);
    }
    rows.push(vec![button("✅ Salva", "food:new:spaces:save")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:new:back:visibility"),
        button("❌ Annulla", "food:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn alimentation_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("🥕 Alimenti", "food:foods")],
        vec![button("🍳 Ricette", "recipe:menu")],
        vec![button("👥 Profili alimentari", "foodprof:menu")],
        vec![button("📅 Planner alimentare", "planner:menu")],
        // Convenzione C3: Alimentazione e' una sezione di primo livello, quindi
        // `⬅️ Indietro` porterebbe esattamente dove porta `🏠 Menù principale`.
        // Due pulsanti per la stessa destinazione non aiutano nessuno.
        vec![button("🏠 Menù principale", "menu:main")],
    ])
}

fn food_menu_keyboard() -> InlineKeyboardMarkup {
    food_menu_keyboard_con_conteggio(None)
}

/// Menu' della sezione Alimenti, nell'ordine canonico di C11.
///
/// C6: sopra le 20 voci sfogliare non e' una strada praticabile — con 422
/// alimenti l'elenco e' lungo 85 pagine — quindi `🔎 Cerca` diventa la prima
/// azione e l'elenco la seconda. Sotto la soglia l'elenco resta primo, perche'
/// con poche voci scorrere e' piu' veloce che scrivere.
///
/// C7: il conteggio sta sull'etichetta dell'elenco, cosi' si sa quanto e'
/// grande la lista prima di aprirla.
fn food_menu_keyboard_con_conteggio(total: Option<i64>) -> InlineKeyboardMarkup {
    let elenco = match total {
        Some(total) => button(
            liste::etichetta_con_conteggio("📋 Elenco alimenti", total),
            "food:list",
        ),
        None => button("📋 Elenco alimenti", "food:list"),
    };
    let cerca = button("🔎 Cerca", "food:search");

    let mut rows = Vec::new();
    if total.is_some_and(liste::si_cerca_invece_di_sfogliare) {
        rows.push(vec![cerca]);
        rows.push(vec![elenco]);
    } else {
        rows.push(vec![elenco]);
        rows.push(vec![cerca]);
    }
    rows.push(vec![button("➕ Nuovo alimento", "food:new")]);
    rows.push(vec![button("🏷 Filtra", "food:filter")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn food_results_keyboard(
    foods: &[FoodRecord],
    page: i64,
    total: usize,
    current_user: Option<i64>,
) -> InlineKeyboardMarkup {
    let mut rows = food_item_rows(foods, current_user);
    push_pagination_row(&mut rows, page, total, "food:list:page");
    // C6: sopra le 20 voci la ricerca e' la strada principale, quindi viene
    // prima della creazione.
    rows.push(vec![
        button("🔎 Cerca", "food:search:list"),
        button("➕ Nuovo alimento", "food:new"),
    ]);
    rows.push(vec![button("🏷 Filtra", "food:filter")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:back"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn food_search_results_keyboard(
    foods: &[FoodRecord],
    page: i64,
    total: usize,
    current_user: Option<i64>,
) -> InlineKeyboardMarkup {
    let mut rows = food_item_rows(foods, current_user);
    push_pagination_row(&mut rows, page, total, "food:search:page");
    rows.push(vec![
        button("🔎 Nuova ricerca", "food:search:list"),
        button("🏷 Filtra", "food:filter"),
    ]);
    rows.push(vec![button("📋 Elenco alimenti", "food:list")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:list"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn food_created_keyboard(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("🥕 Apri alimento", format!("food:view:{id}"))],
        vec![button(
            "✏️ Modifica alimento",
            format!("food:edit:menu:{id}"),
        )],
        vec![button("➕ Altro alimento", "food:new")],
        vec![
            button("⬅️ Indietro", "food:back"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn food_detail_keyboard(id: i64, can_edit: bool, can_manage: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    rows.push(vec![button(
        "🛒 Prodotti associati",
        format!("food:products:{id}"),
    )]);
    if can_edit {
        rows.push(vec![button(
            "✏️ Modifica alimento",
            format!("food:edit:menu:{id}"),
        )]);
    }
    if can_manage && !can_edit {
        rows.push(vec![button(
            "🔐 Collaboratori",
            format!("food:permissions:{id}"),
        )]);
    }
    rows.push(vec![
        button("🔎 Cerca", "food:search:list"),
        button("🏷 Filtra", "food:filter"),
    ]);
    rows.push(vec![button("📋 Elenco alimenti", "food:list")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:list"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn category_filter_keyboard(
    categories: &[CategoryRecord],
    selected: &[i64],
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();

    for pair in categories.chunks(2) {
        rows.push(
            pair.iter()
                .map(|category| {
                    let checked = selected.contains(&category.id);
                    button(
                        format!(
                            "{} {} {}",
                            if checked { "☑️" } else { "⬜" },
                            category.emoji,
                            category.name
                        ),
                        format!("food:filter:toggle:{}", category.id),
                    )
                })
                .collect(),
        );
    }

    rows.push(vec![
        button("✅ Applica", "food:filter:apply"),
        button("🧹 Azzera", "food:filter:clear"),
    ]);
    rows.push(vec![button("📋 Tutti", "food:filter:all")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:list"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn food_filtered_results_keyboard(
    foods: &[FoodRecord],
    page: i64,
    total: usize,
    current_user: Option<i64>,
) -> InlineKeyboardMarkup {
    let mut rows = food_item_rows(foods, current_user);
    push_pagination_row(&mut rows, page, total, "food:filter:page");
    // C6: sopra le 20 voci la ricerca e' la strada principale, quindi viene
    // prima della creazione.
    rows.push(vec![
        button("🔎 Cerca", "food:search:list"),
        button("➕ Nuovo alimento", "food:new"),
    ]);
    rows.push(vec![button("🏷 Cambia filtro", "food:filter")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:list"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn category_before_save_keyboard(categories: &[CategoryRecord]) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for pair in categories.chunks(2) {
        rows.push(
            pair.iter()
                .map(|category| {
                    button(
                        format!("{} {}", category.emoji, category.name),
                        format!("food:new:category:{}", category.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", "food:new:back:unit"),
        button("❌ Annulla", "food:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn category_assignment_keyboard(
    food_id: i64,
    categories: &[CategoryRecord],
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for pair in categories.chunks(2) {
        rows.push(
            pair.iter()
                .map(|category| {
                    button(
                        format!("{} {}", category.emoji, category.name),
                        format!("food:setcat:{food_id}:{}", category.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:view:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn empty_filtered_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("➕ Nuovo alimento", "food:new"),
            button("🔎 Cerca", "food:search:list"),
        ],
        vec![button("🏷 Cambia filtro", "food:filter")],
        vec![
            button("⬅️ Indietro", "food:list"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn food_edit_keyboard(food_id: i64, can_manage: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![
            button("📝 Nome", format!("food:edit:name:{food_id}")),
            button("📏 Unità", format!("food:edit:unit:{food_id}")),
        ],
        vec![button("🏷 Categoria", format!("food:category:{food_id}"))],
    ];
    if can_manage {
        rows.push(vec![
            button("👥 Visibilità", format!("food:edit:visibility:{food_id}")),
            button("🔐 Collaboratori", format!("food:permissions:{food_id}")),
        ]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:view:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn edit_text_keyboard(food_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", format!("food:edit:menu:{food_id}")),
        button("❌ Annulla", format!("food:edit:cancel:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn edit_unit_keyboard(food_id: i64, units: &[UnitRecord]) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for pair in units.chunks(2) {
        rows.push(
            pair.iter()
                .map(|unit| {
                    button(
                        unit_display_label(&unit.nome, &unit.simbolo),
                        format!("food:edit:setunit:{food_id}:{}", unit.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:edit:menu:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn edit_visibility_keyboard(food_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "🔒 Solo mio",
            format!("food:edit:vis:private:{food_id}"),
        )],
        vec![button(
            "🎯 Spazio predefinito",
            format!("food:edit:vis:default:{food_id}"),
        )],
        vec![button(
            "🌐 Tutti i miei spazi",
            format!("food:edit:vis:all:{food_id}"),
        )],
        vec![button(
            "🎛 Scegli spazi",
            format!("food:edit:vis:choose:{food_id}"),
        )],
        vec![
            button("⬅️ Indietro", format!("food:edit:menu:{food_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn edit_space_selection_keyboard(
    food_id: i64,
    spaces: &[SpaceRecord],
    selected: &[i64],
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for space in spaces {
        let checked = selected.contains(&space.id);
        rows.push(vec![button(
            format!("{} {}", if checked { "☑️" } else { "⬜" }, space.name),
            format!("food:edit:space:{food_id}:{}", space.id),
        )]);
    }
    rows.push(vec![button(
        "✅ Salva",
        format!("food:edit:spaces:save:{food_id}"),
    )]);
    rows.push(vec![
        button("⬅️ Indietro", format!("food:edit:visibility:{food_id}")),
        button("❌ Annulla", format!("food:edit:cancel:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn food_permissions_keyboard(
    food_id: i64,
    grants: &[PermissionGrantRecord],
    candidates: &[PermissionCandidateRecord],
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for grant in grants {
        rows.push(vec![button(
            format!("❌ Revoca · {}", grant.name),
            format!("food:perm:revoke:{food_id}:{}", grant.user_id),
        )]);
    }
    for candidate in candidates {
        rows.push(vec![button(
            format!("➕ Invita · {}", candidate.name),
            format!("food:perm:choose:{food_id}:{}", candidate.user_id),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:edit:menu:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn permission_level_keyboard(food_id: i64, user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "✏️ Può modificare",
            format!("food:perm:send:{food_id}:{user_id}:edit"),
        )],
        vec![button(
            "🛡 Modifica + gestisce permessi",
            format!("food:perm:send:{food_id}:{user_id}:manage"),
        )],
        vec![
            button("⬅️ Indietro", format!("food:permissions:{food_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn permission_invite_keyboard(invite_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("✅ Accetta", format!("food:invite:accept:{invite_id}")),
            button("❌ Rifiuta", format!("food:invite:decline:{invite_id}")),
        ],
        vec![button("🏠 Menù principale", "menu:main")],
    ])
}

fn search_from_list_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", "food:list"),
        button("❌ Annulla", "food:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", "food:back"),
        button("❌ Annulla", "food:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn button(text: impl Into<String>, callback: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), callback.into())
}

fn food_summary_line(food: &FoodRecord, current_user: Option<i64>) -> String {
    match food_origin_marker(food, current_user) {
        Some(marker) => format!("{} {marker}", food.name),
        None => food.name.clone(),
    }
}

fn food_origin_marker(food: &FoodRecord, current_user: Option<i64>) -> Option<&'static str> {
    if food.global_catalog == 1 {
        None
    } else if food.owner_user_id == current_user && current_user.is_some() {
        Some("👤")
    } else {
        Some("👥")
    }
}

fn food_origin_rank(food: &FoodRecord) -> i64 {
    if food.global_catalog == 1 {
        2
    } else if food.owner_user_id == identity::current_actor().utente_id {
        0
    } else {
        1
    }
}

/// Cio' che il testo di un elenco di alimenti puo' dire e i pulsanti no (C1).
///
/// Sono due cose e nessuna delle due sta su un pulsante: **con che criterio e'
/// ordinata la lista**, che C6 impone di dichiarare quando non e' alfabetico,
/// e **cosa vogliono dire i marcatori** che compaiono sulle etichette. La
/// legenda si mostra solo quando in pagina c'e' almeno un marcatore da
/// spiegare: il catalogo base non ne porta.
fn note_elenco_alimenti(foods: &[FoodRecord], current_user: Option<i64>) -> String {
    let mut note = String::from("\n\nPrima i tuoi, poi i condivisi, poi il catalogo base.");
    if foods
        .iter()
        .any(|food| food_origin_marker(food, current_user).is_some())
    {
        note.push_str("\n👤 tuo · 👥 condiviso");
    }
    note
}

/// Nella ricerca il testo non elenca piu' i risultati — stanno sui pulsanti —
/// ma tiene l'unica cosa che i pulsanti non possono dire (C1): **perche'** un
/// alimento e' finito fra i risultati quando il suo nome non contiene la
/// parola cercata.
///
/// Cercando «barilla» compare `Amido di mais`, e senza questa riga sembra un
/// errore del bot: e' li' perche' un prodotto commerciale associato si chiama
/// cosi'. Gli alimenti trovati per nome non producono nessuna riga, perche'
/// per loro non c'e' niente da spiegare.
async fn append_search_food_lines(
    pool: &SqlitePool,
    text: &mut String,
    foods: &[FoodRecord],
    current_user: Option<i64>,
    normalized_query: &str,
) {
    let mut spiegazioni = Vec::new();
    for food in foods {
        if normalize_name(&food.name).contains(normalized_query) {
            continue;
        }
        match matching_products_for_food(pool, food.id, normalized_query).await {
            Ok(products) => {
                for (brand, product_name) in products {
                    spiegazioni.push(format!(
                        "{} → 🛒 {brand} · {product_name}",
                        food_summary_line(food, current_user)
                    ));
                }
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    food_id = food.id,
                    "Errore prodotti corrispondenti ricerca"
                );
            }
        }
    }

    if !spiegazioni.is_empty() {
        text.push_str("\n\nTrovati per il prodotto associato:\n");
        text.push_str(&spiegazioni.join("\n"));
    }

    if foods
        .iter()
        .any(|food| food_origin_marker(food, current_user).is_some())
    {
        text.push_str("\n\n👤 tuo · 👥 condiviso");
    }
}

fn food_item_rows(
    foods: &[FoodRecord],
    current_user: Option<i64>,
) -> Vec<Vec<InlineKeyboardButton>> {
    foods
        .iter()
        .map(|food| {
            vec![button(
                food_summary_line(food, current_user),
                format!("food:view:{}", food.id),
            )]
        })
        .collect()
}

fn push_pagination_row(
    rows: &mut Vec<Vec<InlineKeyboardButton>>,
    page: i64,
    total: usize,
    callback_prefix: &str,
) {
    if let Some(riga) = liste::riga_paginazione(page, total as i64, "food:noop", |pagina| {
        format!("{callback_prefix}:{pagina}")
    }) {
        rows.push(riga);
    }
}

fn page_offset(page: i64) -> i64 {
    liste::scarto(page)
}

fn split_food_page(mut rows: Vec<FoodRecord>) -> (Vec<FoodRecord>, bool) {
    let has_next = rows.len() > FOOD_PAGE_SIZE;
    rows.truncate(FOOD_PAGE_SIZE);
    (rows, has_next)
}

fn parse_nonnegative_page(raw: &str) -> Option<i64> {
    let page = raw.parse::<i64>().ok()?;
    (page >= 0).then_some(page)
}

fn clamp_page(page: i64, total: usize) -> i64 {
    liste::pagina_valida(page, total as i64)
}

fn result_count_label(total: usize) -> String {
    if total == 1 {
        "1 risultato".to_string()
    } else {
        format!("{total} risultati")
    }
}

fn ownership_label(food: &FoodRecord, current_user: Option<i64>) -> String {
    if food.global_catalog == 1 {
        return "🌐 Catalogo base".to_string();
    }

    if food.owner_user_id == current_user && current_user.is_some() {
        return "👤 Proprietà: tua".to_string();
    }

    match food.owner_name.as_deref() {
        Some(name) => format!("👤 Proprietà: {name}"),
        None => "👤 Proprietà: non disponibile".to_string(),
    }
}

async fn send_food_products(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
) -> ResponseResult<()> {
    let food = match get_food(pool, food_id).await {
        Ok(Some(food)) => food,
        Ok(None) => {
            bot.send_message(chat_id, "Alimento non disponibile.")
                .reply_markup(food_menu_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, food_id, "Errore alimento per prodotti associati");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo alimento.")
                .await?;
            return Ok(());
        }
    };

    let products = match list_products_for_food(pool, food_id).await {
        Ok(products) => products,
        Err(error) => {
            tracing::error!(?error, food_id, "Errore prodotti associati");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i prodotti associati.")
                .await?;
            return Ok(());
        }
    };
    let can_edit = can_edit_food_current(pool, food_id).await.unwrap_or(false);

    let text = if products.is_empty() {
        format!(
            "🛒 Prodotti associati\n{}\n\nNessun prodotto commerciale associato.",
            food.name
        )
    } else {
        format!(
            "🛒 Prodotti associati\n{}\n\n{}",
            food.name,
            result_count_label(products.len())
                .replace("risultato", "prodotto")
                .replace("risultati", "prodotti")
        )
    };

    bot.send_message(chat_id, text)
        .reply_markup(food_products_keyboard(food_id, &products, can_edit))
        .await?;
    Ok(())
}

async fn matching_products_for_food(
    pool: &SqlitePool,
    food_id: i64,
    normalized_query: &str,
) -> Result<Vec<(String, String)>> {
    if normalized_query.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, (String, String)>(
        "SELECT marca, nome_commerciale \
         FROM prodotti_alimentari \
         WHERE alimento_id = ? AND attivo = 1 \
           AND instr(marca_normalizzata || ' ' || nome_commerciale_normalizzato, ?) > 0 \
         ORDER BY marca COLLATE NOCASE, nome_commerciale COLLATE NOCASE, id \
         LIMIT 3",
    )
    .bind(food_id)
    .bind(normalized_query)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i prodotti corrispondenti alla ricerca")
}

async fn list_products_for_food(pool: &SqlitePool, food_id: i64) -> Result<Vec<ProductRecord>> {
    sqlx::query_as::<_, ProductRecord>(
        "SELECT \
            p.id, \
            p.alimento_id AS food_id, \
            p.marca AS brand, \
            p.nome_commerciale AS product_name, \
            (SELECT COUNT(*) FROM formati_prodotto_alimentare f \
             WHERE f.prodotto_alimentare_id = p.id AND f.attivo = 1) AS format_count \
         FROM prodotti_alimentari p \
         WHERE p.alimento_id = ? AND p.attivo = 1 \
         ORDER BY p.marca COLLATE NOCASE, p.nome_commerciale COLLATE NOCASE, p.id",
    )
    .bind(food_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i prodotti associati")
}

async fn get_product(pool: &SqlitePool, product_id: i64) -> Result<Option<ProductRecord>> {
    let product = sqlx::query_as::<_, ProductRecord>(
        "SELECT \
            p.id, \
            p.alimento_id AS food_id, \
            p.marca AS brand, \
            p.nome_commerciale AS product_name, \
            (SELECT COUNT(*) FROM formati_prodotto_alimentare f \
             WHERE f.prodotto_alimentare_id = p.id AND f.attivo = 1) AS format_count \
         FROM prodotti_alimentari p \
         WHERE p.id = ? AND p.attivo = 1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il prodotto commerciale")?;

    if let Some(product) = product {
        if get_food(pool, product.food_id).await?.is_some() {
            Ok(Some(product))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

async fn list_product_formats(
    pool: &SqlitePool,
    product_id: i64,
) -> Result<Vec<ProductFormatRecord>> {
    sqlx::query_as::<_, ProductFormatRecord>(
        "SELECT \
            f.id, \
            f.prodotto_alimentare_id AS product_id, \
            f.quantita_confezione AS package_quantity, \
            f.unita_confezione_id AS package_unit_id, \
            um.simbolo AS package_unit_symbol, \
            f.codice_ean AS ean \
         FROM formati_prodotto_alimentare f \
         JOIN unita_misura um ON um.id = f.unita_confezione_id \
         WHERE f.prodotto_alimentare_id = ? AND f.attivo = 1 \
         ORDER BY f.quantita_confezione, um.simbolo, f.id",
    )
    .bind(product_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i formati del prodotto")
}

async fn get_product_format(
    pool: &SqlitePool,
    format_id: i64,
) -> Result<Option<ProductFormatRecord>> {
    sqlx::query_as::<_, ProductFormatRecord>(
        "SELECT \
            f.id, \
            f.prodotto_alimentare_id AS product_id, \
            f.quantita_confezione AS package_quantity, \
            f.unita_confezione_id AS package_unit_id, \
            um.simbolo AS package_unit_symbol, \
            f.codice_ean AS ean \
         FROM formati_prodotto_alimentare f \
         JOIN prodotti_alimentari p ON p.id = f.prodotto_alimentare_id AND p.attivo = 1 \
         JOIN unita_misura um ON um.id = f.unita_confezione_id \
         WHERE f.id = ? AND f.attivo = 1",
    )
    .bind(format_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il formato del prodotto")
}

fn product_history_name(brand: &str, product_name: &str) -> String {
    format!("{brand} · {product_name}")
}

fn package_history_value(quantity: f64, unit_symbol: &str) -> String {
    format!("{} {unit_symbol}", display_quantity(quantity))
}

async fn record_product_history_event(
    conn: &mut SqliteConnection,
    product_id: i64,
    product_label: &str,
    operation: &'static str,
    component: &'static str,
    changes: &[NewFieldChange],
) -> Result<()> {
    let entity_id = storico::ensure_entity(conn, "prodotto_alimentare", product_id, product_label)
        .await
        .context("Impossibile preparare lo storico del prodotto")?;

    let event_id = storico::record_event(
        conn,
        &NewHistoryEvent {
            entita_storico_id: entity_id,
            modulo: "alimentazione",
            componente: component,
            operazione: operation,
            nome_entita_snapshot: product_label,
            abitazione_storico_id: None,
            abitazione_nome_snapshot: None,
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await
    .context("Impossibile registrare l'evento nello storico")?;

    storico::record_field_changes(conn, event_id, changes)
        .await
        .context("Impossibile registrare i cambiamenti nello storico")?;
    Ok(())
}

async fn find_product_by_identity(
    pool: &SqlitePool,
    food_id: i64,
    brand: &str,
    product_name: &str,
) -> Result<Option<i64>> {
    sqlx::query_scalar(
        "SELECT id FROM prodotti_alimentari \
         WHERE alimento_id = ? AND attivo = 1 \
           AND marca_normalizzata = ? \
           AND nome_commerciale_normalizzato = ? \
         ORDER BY id LIMIT 1",
    )
    .bind(food_id)
    .bind(normalize_name(brand))
    .bind(normalize_name(product_name))
    .fetch_optional(pool)
    .await
    .context("Impossibile cercare il prodotto commerciale")
}

async fn product_identity_duplicate_exists(
    pool: &SqlitePool,
    product_id: i64,
    food_id: i64,
    brand: &str,
    product_name: &str,
) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM prodotti_alimentari \
            WHERE alimento_id = ? AND attivo = 1 AND id <> ? \
              AND marca_normalizzata = ? \
              AND nome_commerciale_normalizzato = ?\
         )",
    )
    .bind(food_id)
    .bind(product_id)
    .bind(normalize_name(brand))
    .bind(normalize_name(product_name))
    .fetch_one(pool)
    .await
    .context("Impossibile verificare i duplicati del prodotto")
}

async fn format_duplicate_exists(
    pool: &SqlitePool,
    product_id: i64,
    excluded_format_id: Option<i64>,
    quantity: f64,
    unit_id: i64,
) -> Result<bool> {
    let excluded = excluded_format_id.unwrap_or(0);
    sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM formati_prodotto_alimentare \
            WHERE prodotto_alimentare_id = ? AND attivo = 1 \
              AND id <> ? AND quantita_confezione = ? AND unita_confezione_id = ?\
         )",
    )
    .bind(product_id)
    .bind(excluded)
    .bind(quantity)
    .bind(unit_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare i duplicati del formato")
}

async fn create_product_association(
    pool: &SqlitePool,
    food_id: i64,
    raw_brand: &str,
    raw_product_name: &str,
    quantity: f64,
    unit_id: i64,
) -> Result<i64> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Identità utente non disponibile")?;
    if !can_edit_food(pool, food_id, user_id).await? {
        bail!("Non hai il permesso di associare prodotti a questo alimento");
    }

    let brand = clean_product_text(raw_brand).context("Marca non valida")?;
    let product_name = clean_product_text(raw_product_name).context("Nome prodotto non valido")?;
    if !quantity.is_finite() || quantity <= 0.0 {
        bail!("Quantità confezione non valida");
    }
    let unit = get_product_package_unit(pool, unit_id)
        .await?
        .context("Unità confezione non disponibile")?;

    if let Some(existing_product_id) =
        find_product_by_identity(pool, food_id, &brand, &product_name).await?
    {
        create_product_format(pool, existing_product_id, quantity, unit_id).await?;
        return Ok(existing_product_id);
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare il salvataggio del prodotto")?;
    let id = sqlx::query(
        "INSERT INTO prodotti_alimentari (\
            alimento_id, marca, marca_normalizzata, nome_commerciale, \
            nome_commerciale_normalizzato, quantita_confezione, \
            unita_confezione_id, codice_ean, creato_da_utente_id\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?)",
    )
    .bind(food_id)
    .bind(&brand)
    .bind(normalize_name(&brand))
    .bind(&product_name)
    .bind(normalize_name(&product_name))
    .bind(quantity)
    .bind(unit_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile associare il prodotto commerciale")?
    .last_insert_rowid();

    sqlx::query(
        "INSERT INTO formati_prodotto_alimentare (\
            prodotto_alimentare_id, quantita_confezione, unita_confezione_id\
         ) VALUES (?, ?, ?)",
    )
    .bind(id)
    .bind(quantity)
    .bind(unit_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile creare il formato iniziale del prodotto")?;

    let food_name: String =
        sqlx::query_scalar("SELECT nome FROM alimenti WHERE id = ? AND archiviato = 0")
            .bind(food_id)
            .fetch_one(&mut *tx)
            .await
            .context("Impossibile leggere l'alimento associato per lo storico")?;
    let product_label = product_history_name(&brand, &product_name);
    record_product_history_event(
        &mut tx,
        id,
        &product_label,
        "creazione",
        "prodotto_commerciale",
        &[
            NewFieldChange {
                campo: "alimento_associato",
                tipo_valore: "testo",
                valore_prima: None,
                valore_dopo: Some(food_name),
            },
            NewFieldChange {
                campo: "marca",
                tipo_valore: "testo",
                valore_prima: None,
                valore_dopo: Some(brand.clone()),
            },
            NewFieldChange {
                campo: "nome_commerciale",
                tipo_valore: "testo",
                valore_prima: None,
                valore_dopo: Some(product_name.clone()),
            },
        ],
    )
    .await?;
    record_product_history_event(
        &mut tx,
        id,
        &product_label,
        "modifica",
        "formato_prodotto",
        &[NewFieldChange {
            campo: "confezione",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(package_history_value(quantity, &unit.simbolo)),
        }],
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare il salvataggio del prodotto")?;
    Ok(id)
}

async fn create_product_format(
    pool: &SqlitePool,
    product_id: i64,
    quantity: f64,
    unit_id: i64,
) -> Result<i64> {
    let product = get_product(pool, product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di aggiungere formati a questo prodotto");
    }
    if !quantity.is_finite() || quantity <= 0.0 {
        bail!("Quantità formato non valida");
    }
    let unit = get_product_package_unit(pool, unit_id)
        .await?
        .context("Unità formato non disponibile")?;
    if format_duplicate_exists(pool, product_id, None, quantity, unit_id).await? {
        bail!("Questo formato è già disponibile per il prodotto");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare il salvataggio del formato")?;
    let format_id = sqlx::query(
        "INSERT INTO formati_prodotto_alimentare (\
            prodotto_alimentare_id, quantita_confezione, unita_confezione_id\
         ) VALUES (?, ?, ?)",
    )
    .bind(product_id)
    .bind(quantity)
    .bind(unit_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiungere il formato")?
    .last_insert_rowid();

    let product_label = product_history_name(&product.brand, &product.product_name);
    record_product_history_event(
        &mut tx,
        product_id,
        &product_label,
        "modifica",
        "formato_prodotto",
        &[NewFieldChange {
            campo: "confezione",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(package_history_value(quantity, &unit.simbolo)),
        }],
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare il salvataggio del formato")?;
    Ok(format_id)
}

async fn update_product_brand(pool: &SqlitePool, product_id: i64, raw_brand: &str) -> Result<()> {
    let product = get_product(pool, product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di modificare questo prodotto");
    }
    let brand = clean_product_text(raw_brand).context("Marca non valida")?;
    if product.brand == brand {
        return Ok(());
    }
    if product_identity_duplicate_exists(
        pool,
        product_id,
        product.food_id,
        &brand,
        &product.product_name,
    )
    .await?
    {
        bail!("Esiste già lo stesso prodotto commerciale per questo alimento");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica della marca")?;
    sqlx::query(
        "UPDATE prodotti_alimentari SET marca = ?, marca_normalizzata = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(&brand)
    .bind(normalize_name(&brand))
    .bind(product_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare la marca")?;

    let product_label = product_history_name(&brand, &product.product_name);
    record_product_history_event(
        &mut tx,
        product_id,
        &product_label,
        "modifica",
        "prodotto_commerciale",
        &[NewFieldChange {
            campo: "marca",
            tipo_valore: "testo",
            valore_prima: Some(product.brand),
            valore_dopo: Some(brand),
        }],
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare la modifica della marca")?;
    Ok(())
}

async fn update_product_name(
    pool: &SqlitePool,
    product_id: i64,
    raw_product_name: &str,
) -> Result<()> {
    let product = get_product(pool, product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di modificare questo prodotto");
    }
    let product_name = clean_product_text(raw_product_name).context("Nome prodotto non valido")?;
    if product.product_name == product_name {
        return Ok(());
    }
    if product_identity_duplicate_exists(
        pool,
        product_id,
        product.food_id,
        &product.brand,
        &product_name,
    )
    .await?
    {
        bail!("Esiste già lo stesso prodotto commerciale per questo alimento");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica del nome commerciale")?;
    sqlx::query(
        "UPDATE prodotti_alimentari SET nome_commerciale = ?, \
         nome_commerciale_normalizzato = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(&product_name)
    .bind(normalize_name(&product_name))
    .bind(product_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare il nome commerciale")?;

    let product_label = product_history_name(&product.brand, &product_name);
    record_product_history_event(
        &mut tx,
        product_id,
        &product_label,
        "modifica",
        "prodotto_commerciale",
        &[NewFieldChange {
            campo: "nome_commerciale",
            tipo_valore: "testo",
            valore_prima: Some(product.product_name),
            valore_dopo: Some(product_name),
        }],
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare la modifica del nome commerciale")?;
    Ok(())
}

async fn update_product_format_quantity(
    pool: &SqlitePool,
    format_id: i64,
    quantity: f64,
    unit_id: i64,
) -> Result<()> {
    let format = get_product_format(pool, format_id)
        .await?
        .context("Formato non disponibile")?;
    let product = get_product(pool, format.product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di modificare questo formato");
    }
    if !quantity.is_finite() || quantity <= 0.0 {
        bail!("Quantità formato non valida");
    }
    let unit = get_product_package_unit(pool, unit_id)
        .await?
        .context("Unità formato non disponibile")?;
    if format.package_quantity == quantity && format.package_unit_id == unit_id {
        return Ok(());
    }
    if format_duplicate_exists(pool, format.product_id, Some(format_id), quantity, unit_id).await? {
        bail!("Questo formato è già disponibile per il prodotto");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica del formato")?;
    sqlx::query(
        "UPDATE formati_prodotto_alimentare \
         SET quantita_confezione = ?, unita_confezione_id = ?, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
    )
    .bind(quantity)
    .bind(unit_id)
    .bind(format_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare il formato")?;

    let product_label = product_history_name(&product.brand, &product.product_name);
    record_product_history_event(
        &mut tx,
        product.id,
        &product_label,
        "modifica",
        "formato_prodotto",
        &[NewFieldChange {
            campo: "confezione",
            tipo_valore: "testo",
            valore_prima: Some(package_history_value(
                format.package_quantity,
                &format.package_unit_symbol,
            )),
            valore_dopo: Some(package_history_value(quantity, &unit.simbolo)),
        }],
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare la modifica del formato")?;
    Ok(())
}

fn parse_ean_input(raw: &str) -> Result<Option<String>> {
    let value = raw.trim();
    if value == "-" || value.is_empty() {
        return Ok(None);
    }
    if !value.chars().all(|ch| ch.is_ascii_digit()) {
        bail!("Il barcode / EAN deve contenere solo cifre");
    }
    if !matches!(value.len(), 8 | 12 | 13 | 14) {
        bail!("Usa un codice da 8, 12, 13 o 14 cifre, oppure - per rimuoverlo");
    }
    Ok(Some(value.to_string()))
}

async fn update_product_format_ean(
    pool: &SqlitePool,
    format_id: i64,
    ean: Option<&str>,
) -> Result<()> {
    let format = get_product_format(pool, format_id)
        .await?
        .context("Formato non disponibile")?;
    let product = get_product(pool, format.product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di modificare questo formato");
    }
    if format.ean.as_deref() == ean {
        return Ok(());
    }
    if let Some(ean) = ean {
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM formati_prodotto_alimentare \
             WHERE codice_ean = ? AND attivo = 1 AND id <> ?)",
        )
        .bind(ean)
        .bind(format_id)
        .fetch_one(pool)
        .await
        .context("Impossibile verificare il barcode / EAN")?;
        if duplicate {
            bail!("Questo barcode / EAN è già associato a un altro formato");
        }
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica del barcode / EAN")?;
    sqlx::query(
        "UPDATE formati_prodotto_alimentare SET codice_ean = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(ean)
    .bind(format_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare il barcode / EAN del formato")?;

    let product_label = product_history_name(&product.brand, &product.product_name);
    record_product_history_event(
        &mut tx,
        product.id,
        &product_label,
        "modifica",
        "formato_prodotto",
        &[NewFieldChange {
            campo: "barcode_ean",
            tipo_valore: "testo",
            valore_prima: format.ean,
            valore_dopo: ean.map(str::to_string),
        }],
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare la modifica del barcode / EAN")?;
    Ok(())
}

async fn delete_product_format(pool: &SqlitePool, format_id: i64) -> Result<i64> {
    let format = get_product_format(pool, format_id)
        .await?
        .context("Formato non disponibile")?;
    let product = get_product(pool, format.product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di eliminare questo formato");
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare l'eliminazione del formato")?;
    let affected = sqlx::query(
        "UPDATE formati_prodotto_alimentare SET attivo = 0, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND attivo = 1",
    )
    .bind(format_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile eliminare il formato")?
    .rows_affected();
    if affected != 1 {
        bail!("Formato non disponibile");
    }

    let product_label = product_history_name(&product.brand, &product.product_name);
    record_product_history_event(
        &mut tx,
        product.id,
        &product_label,
        "eliminazione",
        "formato_prodotto",
        &[NewFieldChange {
            campo: "confezione",
            tipo_valore: "testo",
            valore_prima: Some(package_history_value(
                format.package_quantity,
                &format.package_unit_symbol,
            )),
            valore_dopo: None,
        }],
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare l'eliminazione del formato")?;
    Ok(product.id)
}

async fn get_unit_by_id(pool: &SqlitePool, unit_id: i64) -> Result<Option<UnitRecord>> {
    sqlx::query_as::<_, UnitRecord>(
        "SELECT id, nome, simbolo FROM unita_misura WHERE id = ? AND attiva = 1",
    )
    .bind(unit_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'unità di misura")
}

async fn get_product_package_unit(pool: &SqlitePool, unit_id: i64) -> Result<Option<UnitRecord>> {
    let unit = get_unit_by_id(pool, unit_id).await?;
    Ok(unit.filter(|unit| matches!(unit.simbolo.as_str(), "g" | "kg" | "ml" | "l" | "pz")))
}

async fn default_product_package_unit(pool: &SqlitePool, food_id: i64) -> Result<UnitRecord> {
    let default_unit = sqlx::query_as::<_, UnitRecord>(
        "SELECT um.id, um.nome, um.simbolo \
         FROM alimenti a \
         JOIN unita_misura um ON um.id = a.unita_predefinita_id \
         WHERE a.id = ? AND a.archiviato = 0 AND um.attiva = 1",
    )
    .bind(food_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'unità predefinita dell'alimento")?;

    if let Some(unit) = default_unit {
        if matches!(unit.simbolo.as_str(), "g" | "kg" | "ml" | "l" | "pz") {
            return Ok(unit);
        }
    }

    find_unit_by_text(pool, "g")
        .await?
        .context("Unità grammi non disponibile")
}

async fn send_product_quantity_prompt(
    bot: &Bot,
    chat_id: ChatId,
    food_id: i64,
    unit: &UnitRecord,
) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "📦 Quantità confezione\n\nUnità attuale: {} {}\n\nScrivi solo il numero.\nEsempio: 200",
            unit_icon(&unit.simbolo),
            unit_display_label(&unit.nome, &unit.simbolo)
        ),
    )
    .reply_markup(product_quantity_keyboard(food_id))
    .await?;
    Ok(())
}

async fn send_product_unit_choice(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    food_id: i64,
) -> ResponseResult<()> {
    match list_units(pool).await {
        Ok(units) => {
            let allowed = units
                .into_iter()
                .filter(|unit| matches!(unit.simbolo.as_str(), "g" | "kg" | "ml" | "l" | "pz"))
                .collect::<Vec<_>>();
            bot.send_message(
                chat_id,
                "⚖️ Unità confezione\n\nScegli la nuova unità. Tornerai poi all'inserimento della quantità.",
            )
            .reply_markup(product_unit_keyboard(food_id, &allowed))
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore unità confezione prodotto");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le unità di misura.")
                .reply_markup(product_cancel_keyboard(food_id))
                .await?;
        }
    }
    Ok(())
}

async fn default_product_format_unit(pool: &SqlitePool, product_id: i64) -> Result<UnitRecord> {
    if let Some(unit) = sqlx::query_as::<_, UnitRecord>(
        "SELECT um.id, um.nome, um.simbolo \
         FROM formati_prodotto_alimentare f \
         JOIN unita_misura um ON um.id = f.unita_confezione_id \
         WHERE f.prodotto_alimentare_id = ? AND f.attivo = 1 AND um.attiva = 1 \
         ORDER BY f.id LIMIT 1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'unità dei formati")?
    {
        return Ok(unit);
    }

    let product = get_product(pool, product_id)
        .await?
        .context("Prodotto non disponibile")?;
    default_product_package_unit(pool, product.food_id).await
}

async fn send_product_edit_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    product_id: i64,
) -> ResponseResult<()> {
    let Some(product) = (match get_product(pool, product_id).await {
        Ok(product) => product,
        Err(error) => {
            tracing::error!(?error, product_id, "Errore menu modifica prodotto");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                .await?;
            return Ok(());
        }
    }) else {
        bot.send_message(chat_id, "Prodotto non disponibile.")
            .await?;
        return Ok(());
    };
    if !can_edit_food_current(pool, product.food_id)
        .await
        .unwrap_or(false)
    {
        bot.send_message(
            chat_id,
            "⚠️ Non hai il permesso di modificare questo prodotto.",
        )
        .await?;
        return Ok(());
    }
    bot.send_message(
        chat_id,
        format!(
            "✏️ Modifica prodotto\n\n{} · {}\n\nMarca e nome commerciale appartengono al prodotto. Quantità, unità e barcode si modificano invece nei singoli 📦 formati.",
            product.brand, product.product_name,
        ),
    )
    .reply_markup(product_edit_keyboard(product_id))
    .await?;
    Ok(())
}

async fn send_product_format_quantity_prompt(
    bot: &Bot,
    chat_id: ChatId,
    product_id: i64,
    unit: &UnitRecord,
) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "📦 Nuovo formato\n\nUnità attuale: {} {}\n\nScrivi la quantità della confezione.\nEsempio: 350",
            unit_icon(&unit.simbolo),
            unit_display_label(&unit.nome, &unit.simbolo),
        ),
    )
    .reply_markup(product_format_quantity_keyboard(product_id))
    .await?;
    Ok(())
}

async fn send_product_format_unit_choice(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    product_id: i64,
) -> ResponseResult<()> {
    match list_units(pool).await {
        Ok(units) => {
            let allowed = units
                .into_iter()
                .filter(|unit| matches!(unit.simbolo.as_str(), "g" | "kg" | "ml" | "l" | "pz"))
                .collect::<Vec<_>>();
            bot.send_message(
                chat_id,
                "📏 Unità formato\n\nScegli l'unità e poi inserisci la quantità.",
            )
            .reply_markup(product_format_unit_keyboard(product_id, &allowed))
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, product_id, "Errore unità nuovo formato");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le unità di misura.")
                .reply_markup(product_format_cancel_keyboard(product_id))
                .await?;
        }
    }
    Ok(())
}

async fn send_product_format_edit_quantity_prompt(
    bot: &Bot,
    chat_id: ChatId,
    format_id: i64,
    unit: &UnitRecord,
) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "📦 Modifica formato\n\nUnità attuale: {} {}\n\nScrivi la nuova quantità come numero maggiore di zero.",
            unit_icon(&unit.simbolo),
            unit_display_label(&unit.nome, &unit.simbolo),
        ),
    )
    .reply_markup(product_format_edit_quantity_keyboard(format_id))
    .await?;
    Ok(())
}

async fn send_product_format_edit_unit_choice(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    format_id: i64,
) -> ResponseResult<()> {
    match list_units(pool).await {
        Ok(units) => {
            let allowed = units
                .into_iter()
                .filter(|unit| matches!(unit.simbolo.as_str(), "g" | "kg" | "ml" | "l" | "pz"))
                .collect::<Vec<_>>();
            bot.send_message(
                chat_id,
                "📏 Unità formato\n\nScegli l'unità e poi inserisci la nuova quantità.",
            )
            .reply_markup(product_format_edit_unit_keyboard(format_id, &allowed))
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, format_id, "Errore unità modifica formato");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le unità di misura.")
                .await?;
        }
    }
    Ok(())
}

fn product_formats_summary(formats: &[ProductFormatRecord]) -> String {
    if formats.is_empty() {
        return "Nessun formato disponibile.".to_string();
    }
    let mut lines = formats
        .iter()
        .take(5)
        .map(|format| {
            let ean = format
                .ean
                .as_deref()
                .map(|value| format!(" · EAN {value}"))
                .unwrap_or_default();
            format!(
                "• {} {} {}{}",
                unit_icon(&format.package_unit_symbol),
                display_quantity(format.package_quantity),
                format.package_unit_symbol,
                ean
            )
        })
        .collect::<Vec<_>>();
    if formats.len() > 5 {
        lines.push(format!("• … e altri {} formati", formats.len() - 5));
    }
    lines.join("\n")
}

async fn send_product_formats(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    product_id: i64,
) -> ResponseResult<()> {
    let Some(product) = (match get_product(pool, product_id).await {
        Ok(product) => product,
        Err(error) => {
            tracing::error!(?error, product_id, "Errore prodotto per formati");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                .await?;
            return Ok(());
        }
    }) else {
        bot.send_message(chat_id, "Prodotto non disponibile.")
            .await?;
        return Ok(());
    };
    let formats = match list_product_formats(pool, product_id).await {
        Ok(formats) => formats,
        Err(error) => {
            tracing::error!(?error, product_id, "Errore elenco formati");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i formati.")
                .await?;
            return Ok(());
        }
    };
    let can_edit = can_edit_food_current(pool, product.food_id)
        .await
        .unwrap_or(false);
    bot.send_message(
        chat_id,
        format!(
            "📦 Formati disponibili\n{} · {}\n\n{}",
            product.brand,
            product.product_name,
            product_formats_summary(&formats),
        ),
    )
    .reply_markup(product_formats_keyboard(&product, &formats, can_edit))
    .await?;
    Ok(())
}

async fn send_product_format_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    format_id: i64,
) -> ResponseResult<()> {
    let Some(format) = (match get_product_format(pool, format_id).await {
        Ok(format) => format,
        Err(error) => {
            tracing::error!(?error, format_id, "Errore dettaglio formato");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo formato.")
                .await?;
            return Ok(());
        }
    }) else {
        bot.send_message(chat_id, "Formato non disponibile.")
            .await?;
        return Ok(());
    };
    let Some(product) = get_product(pool, format.product_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, "Prodotto non disponibile.")
            .await?;
        return Ok(());
    };
    let can_edit = can_edit_food_current(pool, product.food_id)
        .await
        .unwrap_or(false);
    let ean = format.ean.as_deref().unwrap_or("non impostato");
    bot.send_message(
        chat_id,
        format!(
            "📦 Formato\n{} · {}\n\n{} Confezione: {} {}\n🔢 Barcode / EAN: {}",
            product.brand,
            product.product_name,
            unit_icon(&format.package_unit_symbol),
            display_quantity(format.package_quantity),
            format.package_unit_symbol,
            ean,
        ),
    )
    .reply_markup(product_format_detail_keyboard(
        format_id, product.id, can_edit,
    ))
    .await?;
    Ok(())
}

async fn send_product_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    product_id: i64,
) -> ResponseResult<()> {
    let Some(product) = (match get_product(pool, product_id).await {
        Ok(product) => product,
        Err(error) => {
            tracing::error!(?error, product_id, "Errore dettaglio prodotto commerciale");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                .await?;
            return Ok(());
        }
    }) else {
        bot.send_message(chat_id, "Prodotto non disponibile.")
            .await?;
        return Ok(());
    };

    let can_edit = can_edit_food_current(pool, product.food_id)
        .await
        .unwrap_or(false);
    let formats = list_product_formats(pool, product_id)
        .await
        .unwrap_or_default();
    bot.send_message(
        chat_id,
        format!(
            "🛒 {} · {}\n\n📦 Formati disponibili: {}\n{}\n\nIl prodotto commerciale è indipendente dal formato: Ricette e valori nutrizionali possono riferirsi al prodotto, mentre Lista spesa, barcode, disponibilità e futuri prezzi useranno il formato acquistabile.",
            product.brand,
            product.product_name,
            formats.len(),
            product_formats_summary(&formats),
        ),
    )
    .reply_markup(product_detail_keyboard(&product, can_edit))
    .await?;
    Ok(())
}

async fn get_product_nutrition(
    pool: &SqlitePool,
    product_id: i64,
) -> Result<Option<NutritionRecord>> {
    sqlx::query_as::<_, NutritionRecord>(
        "SELECT \
            um.simbolo AS reference_unit_symbol, \
            vn.energia_kcal AS energy_kcal, \
            vn.energia_kj AS energy_kj, \
            vn.grassi_g AS fat_g, \
            vn.saturi_g AS saturated_fat_g, \
            vn.carboidrati_g AS carbohydrates_g, \
            vn.zuccheri_g AS sugars_g, \
            vn.fibre_g AS fibre_g, \
            vn.proteine_g AS protein_g, \
            vn.sale_g AS salt_g \
         FROM valori_nutrizionali_prodotto vn \
         JOIN unita_misura um ON um.id = vn.riferimento_unita_id \
         WHERE vn.prodotto_alimentare_id = ?",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere i valori nutrizionali")
}

async fn default_nutrition_reference_unit(
    pool: &SqlitePool,
    product: &ProductRecord,
) -> Result<UnitRecord> {
    let format_symbol: Option<String> = sqlx::query_scalar(
        "SELECT um.simbolo \
         FROM formati_prodotto_alimentare f \
         JOIN unita_misura um ON um.id = f.unita_confezione_id \
         WHERE f.prodotto_alimentare_id = ? AND f.attivo = 1 \
         ORDER BY f.id LIMIT 1",
    )
    .bind(product.id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il formato per la nutrizione")?;
    if let Some(format_symbol) = format_symbol.as_deref() {
        let symbol = match format_symbol {
            "ml" | "l" => "ml",
            _ => "g",
        };
        return find_unit_by_text(pool, symbol)
            .await?
            .context("Unità nutrizionale di riferimento non disponibile");
    }
    let fallback = default_product_package_unit(pool, product.food_id).await?;
    let symbol = match fallback.simbolo.as_str() {
        "ml" | "l" => "ml",
        _ => "g",
    };
    find_unit_by_text(pool, symbol)
        .await?
        .context("Unità nutrizionale di riferimento non disponibile")
}

async fn send_product_nutrition(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    product_id: i64,
) -> ResponseResult<()> {
    let Some(product) = (match get_product(pool, product_id).await {
        Ok(product) => product,
        Err(error) => {
            tracing::error!(
                ?error,
                product_id,
                "Errore prodotto per valori nutrizionali"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo prodotto.")
                .await?;
            return Ok(());
        }
    }) else {
        bot.send_message(chat_id, "Prodotto non disponibile.")
            .await?;
        return Ok(());
    };
    let can_edit = can_edit_food_current(pool, product.food_id)
        .await
        .unwrap_or(false);
    let nutrition = match get_product_nutrition(pool, product_id).await {
        Ok(nutrition) => nutrition,
        Err(error) => {
            tracing::error!(?error, product_id, "Errore valori nutrizionali prodotto");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i valori nutrizionali.")
                .await?;
            return Ok(());
        }
    };

    let text = if let Some(n) = nutrition.as_ref() {
        format!(
            "🧮 Valori nutrizionali\n{} · {}\n\nPer 100 {}:\n🔥 Energia: {} kcal · {} kJ\n🥑 Grassi: {} g\n   di cui saturi: {} g\n🍞 Carboidrati: {} g\n   di cui zuccheri: {} g\n🌾 Fibre: {} g\n💪 Proteine: {} g\n🧂 Sale: {} g",
            product.brand,
            product.product_name,
            n.reference_unit_symbol,
            display_optional_number(n.energy_kcal),
            display_optional_number(n.energy_kj),
            display_optional_number(n.fat_g),
            display_optional_number(n.saturated_fat_g),
            display_optional_number(n.carbohydrates_g),
            display_optional_number(n.sugars_g),
            display_optional_number(n.fibre_g),
            display_optional_number(n.protein_g),
            display_optional_number(n.salt_g),
        )
    } else {
        format!(
            "🧮 Valori nutrizionali\n{} · {}\n\nNessun valore inserito. Questa sezione è facoltativa.",
            product.brand, product.product_name
        )
    };

    bot.send_message(chat_id, text)
        .reply_markup(product_nutrition_keyboard(
            product_id,
            nutrition.is_some(),
            can_edit,
        ))
        .await?;
    Ok(())
}

async fn send_nutrition_input_prompt(
    bot: &Bot,
    chat_id: ChatId,
    product_id: i64,
    reference_unit: &UnitRecord,
) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "🧮 Inserisci valori nutrizionali\n\nRiferimento attuale: 100 {}\n\nInvia fino a 9 valori separati da punto e virgola in questo ordine:\n1. kcal\n2. kJ\n3. grassi\n4. saturi\n5. carboidrati\n6. zuccheri\n7. fibre\n8. proteine\n9. sale\n\nUsa - per un valore non disponibile. Se interrompi prima del nono valore, i campi rimanenti verranno completati automaticamente con -.\nPrima del salvataggio ti mostrerò un riepilogo da confermare.\nEsempio:\n225; 934; 21; 14; 4,3; 4,3; 0,3; 5,4; 0,75",
            reference_unit.simbolo
        ),
    )
    .reply_markup(nutrition_input_keyboard(product_id))
    .await?;
    Ok(())
}

async fn send_nutrition_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    product_id: i64,
    reference_unit: &UnitRecord,
    values: &[Option<f64>; 9],
) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "🧮 Conferma valori nutrizionali\n\nRiferimento: 100 {}\n\n🔥 kcal: {}\n⚡ kJ: {}\n🥑 Grassi: {} g\n   Saturi: {} g\n🍞 Carboidrati: {} g\n   Zuccheri: {} g\n🌾 Fibre: {} g\n💪 Proteine: {} g\n🧂 Sale: {} g\n\nI campi mancanti sono stati completati con —.",
            reference_unit.simbolo,
            display_optional_number(values[0]),
            display_optional_number(values[1]),
            display_optional_number(values[2]),
            display_optional_number(values[3]),
            display_optional_number(values[4]),
            display_optional_number(values[5]),
            display_optional_number(values[6]),
            display_optional_number(values[7]),
            display_optional_number(values[8]),
        ),
    )
    .reply_markup(nutrition_confirmation_keyboard(product_id))
    .await?;
    Ok(())
}

fn parse_nutrition_values(raw: &str) -> Result<[Option<f64>; 9]> {
    let parts = raw.split(';').map(str::trim).collect::<Vec<_>>();
    if parts.len() > 9 {
        bail!("Puoi inserire al massimo 9 valori separati da punto e virgola");
    }
    let mut values = [None; 9];
    for (index, raw_value) in parts.iter().enumerate() {
        if *raw_value == "-" || raw_value.is_empty() {
            continue;
        }
        let normalized = raw_value.replace(',', ".");
        let value = normalized
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .with_context(|| format!("Valore {} non valido", index + 1))?;
        values[index] = Some(value);
    }
    if values.iter().all(Option::is_none) {
        bail!("Inserisci almeno un valore nutrizionale oppure annulla");
    }
    Ok(values)
}

fn nutrition_history_changes(
    before: Option<&NutritionRecord>,
    after_reference_unit: Option<&str>,
    after_values: Option<&[Option<f64>; 9]>,
) -> Vec<NewFieldChange> {
    let before_value = |extract: fn(&NutritionRecord) -> Option<f64>| {
        before.and_then(extract).map(display_quantity)
    };
    let after_value = |index: usize| {
        after_values
            .and_then(|values| values[index])
            .map(display_quantity)
    };

    vec![
        NewFieldChange {
            campo: "riferimento_nutrizionale",
            tipo_valore: "testo",
            valore_prima: before.map(|record| format!("100 {}", record.reference_unit_symbol)),
            valore_dopo: after_reference_unit.map(|symbol| format!("100 {symbol}")),
        },
        NewFieldChange {
            campo: "energia_kcal",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.energy_kcal),
            valore_dopo: after_value(0),
        },
        NewFieldChange {
            campo: "energia_kj",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.energy_kj),
            valore_dopo: after_value(1),
        },
        NewFieldChange {
            campo: "grassi_g",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.fat_g),
            valore_dopo: after_value(2),
        },
        NewFieldChange {
            campo: "saturi_g",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.saturated_fat_g),
            valore_dopo: after_value(3),
        },
        NewFieldChange {
            campo: "carboidrati_g",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.carbohydrates_g),
            valore_dopo: after_value(4),
        },
        NewFieldChange {
            campo: "zuccheri_g",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.sugars_g),
            valore_dopo: after_value(5),
        },
        NewFieldChange {
            campo: "fibre_g",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.fibre_g),
            valore_dopo: after_value(6),
        },
        NewFieldChange {
            campo: "proteine_g",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.protein_g),
            valore_dopo: after_value(7),
        },
        NewFieldChange {
            campo: "sale_g",
            tipo_valore: "testo",
            valore_prima: before_value(|record| record.salt_g),
            valore_dopo: after_value(8),
        },
    ]
}

async fn save_product_nutrition(
    pool: &SqlitePool,
    product_id: i64,
    reference_unit_id: i64,
    values: &[Option<f64>; 9],
) -> Result<()> {
    let product = get_product(pool, product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di modificare questo prodotto");
    }
    let reference_unit = get_unit_by_id(pool, reference_unit_id)
        .await?
        .filter(|unit| matches!(unit.simbolo.as_str(), "g" | "ml"))
        .context("Unità nutrizionale non valida")?;
    let before = get_product_nutrition(pool, product_id).await?;
    let changes =
        nutrition_history_changes(before.as_ref(), Some(&reference_unit.simbolo), Some(values));
    if changes
        .iter()
        .all(|change| change.valore_prima == change.valore_dopo)
    {
        return Ok(());
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare il salvataggio dei valori nutrizionali")?;
    sqlx::query(
        "INSERT INTO valori_nutrizionali_prodotto (\
            prodotto_alimentare_id, riferimento_unita_id, energia_kcal, energia_kj, \
            grassi_g, saturi_g, carboidrati_g, zuccheri_g, fibre_g, proteine_g, sale_g, \
            fonte_tipo, aggiornato_il\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'manuale', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         ON CONFLICT(prodotto_alimentare_id) DO UPDATE SET \
            riferimento_unita_id = excluded.riferimento_unita_id, \
            energia_kcal = excluded.energia_kcal, energia_kj = excluded.energia_kj, \
            grassi_g = excluded.grassi_g, saturi_g = excluded.saturi_g, \
            carboidrati_g = excluded.carboidrati_g, zuccheri_g = excluded.zuccheri_g, \
            fibre_g = excluded.fibre_g, proteine_g = excluded.proteine_g, sale_g = excluded.sale_g, \
            fonte_tipo = 'manuale', aggiornato_il = excluded.aggiornato_il",
    )
    .bind(product_id)
    .bind(reference_unit.id)
    .bind(values[0])
    .bind(values[1])
    .bind(values[2])
    .bind(values[3])
    .bind(values[4])
    .bind(values[5])
    .bind(values[6])
    .bind(values[7])
    .bind(values[8])
    .execute(&mut *tx)
    .await
    .context("Impossibile salvare i valori nutrizionali")?;

    let product_label = product_history_name(&product.brand, &product.product_name);
    record_product_history_event(
        &mut tx,
        product_id,
        &product_label,
        "modifica",
        "valori_nutrizionali",
        &changes,
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare il salvataggio dei valori nutrizionali")?;
    Ok(())
}

async fn remove_product_nutrition(pool: &SqlitePool, product_id: i64) -> Result<()> {
    let product = get_product(pool, product_id)
        .await?
        .context("Prodotto non disponibile")?;
    if !can_edit_food_current(pool, product.food_id).await? {
        bail!("Non hai il permesso di modificare questo prodotto");
    }
    let before = get_product_nutrition(pool, product_id).await?;
    let Some(before) = before else {
        return Ok(());
    };
    let changes = nutrition_history_changes(Some(&before), None, None);

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la rimozione dei valori nutrizionali")?;
    sqlx::query("DELETE FROM valori_nutrizionali_prodotto WHERE prodotto_alimentare_id = ?")
        .bind(product_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile rimuovere i valori nutrizionali")?;

    let product_label = product_history_name(&product.brand, &product.product_name);
    record_product_history_event(
        &mut tx,
        product_id,
        &product_label,
        "modifica",
        "valori_nutrizionali",
        &changes,
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare la rimozione dei valori nutrizionali")?;
    Ok(())
}

fn food_products_keyboard(
    food_id: i64,
    products: &[ProductRecord],
    can_edit: bool,
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for product in products {
        let format_label = if product.format_count == 1 {
            "1 formato".to_string()
        } else {
            format!("{} formati", product.format_count)
        };
        rows.push(vec![button(
            format!(
                "{} · {} · {format_label}",
                product.brand, product.product_name
            ),
            format!("food:product:view:{}", product.id),
        )]);
    }
    if can_edit {
        rows.push(vec![button(
            "➕ Associa prodotto",
            format!("food:product:new:{food_id}"),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:view:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn product_detail_keyboard(product: &ProductRecord, can_edit: bool) -> InlineKeyboardMarkup {
    let format_label = if product.format_count == 1 {
        "📦 Formati (1)".to_string()
    } else {
        format!("📦 Formati ({})", product.format_count)
    };
    let mut rows = vec![
        vec![button(
            format_label,
            format!("food:product:formats:{}", product.id),
        )],
        vec![button(
            "🧮 Valori nutrizionali",
            format!("food:product:nutrition:{}", product.id),
        )],
    ];
    if can_edit {
        rows.push(vec![button(
            "✏️ Modifica prodotto",
            format!("food:product:edit:{}", product.id),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:products:{}", product.food_id)),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn product_edit_keyboard(product_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("🏷 Marca", format!("food:product:edit:brand:{product_id}")),
            button(
                "🛒 Nome commerciale",
                format!("food:product:edit:name:{product_id}"),
            ),
        ],
        vec![button(
            "📦 Gestisci formati",
            format!("food:product:formats:{product_id}"),
        )],
        vec![
            button("⬅️ Indietro", format!("food:product:view:{product_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn product_edit_cancel_keyboard(product_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", format!("food:product:edit:{product_id}")),
        button("❌ Annulla", format!("food:product:view:{product_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn product_formats_keyboard(
    product: &ProductRecord,
    formats: &[ProductFormatRecord],
    can_edit: bool,
) -> InlineKeyboardMarkup {
    let mut rows = formats
        .iter()
        .map(|format| {
            vec![button(
                format!(
                    "{} {} {}",
                    unit_icon(&format.package_unit_symbol),
                    display_quantity(format.package_quantity),
                    format.package_unit_symbol
                ),
                format!("food:format:view:{}", format.id),
            )]
        })
        .collect::<Vec<_>>();
    if can_edit {
        rows.push(vec![button(
            "➕ Aggiungi formato",
            format!("food:product:format:new:{}", product.id),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:product:view:{}", product.id)),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn product_format_quantity_keyboard(product_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "📏 Cambia unità",
            format!("food:product:format:changeunit:{product_id}"),
        )],
        vec![
            button("⬅️ Indietro", format!("food:product:formats:{product_id}")),
            button("❌ Annulla", format!("food:product:formats:{product_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn product_format_unit_keyboard(product_id: i64, units: &[UnitRecord]) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for pair in units.chunks(2) {
        rows.push(
            pair.iter()
                .map(|unit| {
                    button(
                        unit_display_label(&unit.nome, &unit.simbolo),
                        format!("food:product:format:unit:{product_id}:{}", unit.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:product:formats:{product_id}")),
        button("❌ Annulla", format!("food:product:formats:{product_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn product_format_cancel_keyboard(product_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", format!("food:product:formats:{product_id}")),
        button("❌ Annulla", format!("food:product:view:{product_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn product_format_detail_keyboard(
    format_id: i64,
    product_id: i64,
    can_edit: bool,
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if can_edit {
        rows.push(vec![
            button(
                "⚖️ Quantità e unità",
                format!("food:format:edit:quantity:{format_id}"),
            ),
            button(
                "🔢 Barcode / EAN",
                format!("food:format:edit:ean:{format_id}"),
            ),
        ]);
        rows.push(vec![button(
            "🗑 Elimina formato",
            format!("food:format:delete:ask:{format_id}"),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:product:formats:{product_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn product_format_edit_quantity_keyboard(format_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "📏 Cambia unità",
            format!("food:format:edit:changeunit:{format_id}"),
        )],
        vec![
            button("⬅️ Indietro", format!("food:format:view:{format_id}")),
            button("❌ Annulla", format!("food:format:view:{format_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn product_format_edit_unit_keyboard(format_id: i64, units: &[UnitRecord]) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for pair in units.chunks(2) {
        rows.push(
            pair.iter()
                .map(|unit| {
                    button(
                        unit_display_label(&unit.nome, &unit.simbolo),
                        format!("food:format:edit:unit:{format_id}:{}", unit.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:format:view:{format_id}")),
        button("❌ Annulla", format!("food:format:view:{format_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn product_cancel_keyboard(food_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", format!("food:products:{food_id}")),
        button("❌ Annulla", format!("food:product:cancel:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn product_quantity_keyboard(food_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "📏 Cambia unità",
            format!("food:product:changeunit:{food_id}"),
        )],
        vec![
            button("⬅️ Indietro", format!("food:products:{food_id}")),
            button("❌ Annulla", format!("food:product:cancel:{food_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn product_unit_keyboard(food_id: i64, units: &[UnitRecord]) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for pair in units.chunks(2) {
        rows.push(
            pair.iter()
                .map(|unit| {
                    button(
                        unit_display_label(&unit.nome, &unit.simbolo),
                        format!("food:product:unit:{}", unit.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:product:quantity:{food_id}")),
        button("❌ Annulla", format!("food:product:cancel:{food_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn product_nutrition_keyboard(
    product_id: i64,
    has_values: bool,
    can_edit: bool,
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if can_edit {
        rows.push(vec![button(
            if has_values {
                "✏️ Modifica valori"
            } else {
                "➕ Inserisci valori"
            },
            format!("food:product:nutrition:edit:{product_id}"),
        )]);
        if has_values {
            rows.push(vec![button(
                "🧹 Rimuovi valori",
                format!("food:product:nutrition:remove:{product_id}"),
            )]);
        }
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("food:product:view:{product_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn nutrition_confirmation_keyboard(product_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button(
                "✅ Conferma",
                format!("food:product:nutrition:confirm:{product_id}"),
            ),
            button(
                "✏️ Modifica",
                format!("food:product:nutrition:modify:{product_id}"),
            ),
        ],
        vec![
            button(
                "❌ Annulla",
                format!("food:product:nutrition:cancel:{product_id}"),
            ),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn nutrition_input_keyboard(product_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button(
                "⚖️ Per 100 g",
                format!("food:product:nutrition:ref:{product_id}:g"),
            ),
            button(
                "🥤 Per 100 ml",
                format!("food:product:nutrition:ref:{product_id}:ml"),
            ),
        ],
        vec![
            button(
                "⬅️ Indietro",
                format!("food:product:nutrition:{product_id}"),
            ),
            button("❌ Annulla", format!("food:product:nutrition:{product_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
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

fn display_optional_number(value: Option<f64>) -> String {
    value
        .map(display_quantity)
        .unwrap_or_else(|| "—".to_string())
}

fn clean_product_text(raw: &str) -> Option<String> {
    let clean = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = clean.chars().count();
    if clean.is_empty() || len > PRODUCT_TEXT_MAX_CHARS {
        None
    } else {
        Some(clean)
    }
}

fn clean_food_name(raw: &str) -> Option<String> {
    let clean = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = clean.chars().count();
    if clean.is_empty() || len > FOOD_NAME_MAX_CHARS {
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

fn parse_command(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let split_at = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let token = &trimmed[..split_at];
    let args = trimmed[split_at..].trim();
    let command = token.split('@').next().unwrap_or(token);
    Some((command, args))
}

fn parse_two_positive_ids(raw: &str) -> Option<(i64, i64)> {
    let mut parts = raw.split(':');
    let first = parts.next()?.parse::<i64>().ok()?;
    let second = parts.next()?.parse::<i64>().ok()?;
    if parts.next().is_some() || first <= 0 || second <= 0 {
        return None;
    }
    Some((first, second))
}

fn parse_positive_id(raw: &str) -> Option<i64> {
    let id = raw.trim().parse::<i64>().ok()?;
    (id > 0).then_some(id)
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
            .expect("database in memoria");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign key di test");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration di test");
        pool
    }

    async fn create_user(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("utente")
            .last_insert_rowid()
    }

    async fn add_membership(pool: &SqlitePool, space_id: i64, user_id: i64, role: &str) {
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) \
             VALUES (?, ?, ?)",
        )
        .bind(space_id)
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await
        .expect("membership");
    }

    fn actor(user_id: i64, space_id: i64, view_all: bool, name: &str) -> identity::AuditActor {
        identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: name.to_string(),
            spazio_id: space_id,
            spazio_nome_snapshot: format!("Spazio #{space_id}"),
            view_all,
            origine: "telegram",
            telegram_user_id: Some(user_id + 1000),
            telegram_username: None,
        }
    }

    async fn compatibility_status(pool: &SqlitePool, food_name: &str, label_code: &str) -> String {
        sqlx::query_scalar(
            "SELECT ac.stato \
             FROM alimento_compatibilita ac \
             JOIN alimenti a ON a.id = ac.alimento_id \
             JOIN etichette_alimentari e ON e.id = ac.etichetta_id \
             WHERE a.catalogo_globale = 1 \
               AND a.nome_normalizzato = ? \
               AND e.codice = ?",
        )
        .bind(food_name)
        .bind(label_code)
        .fetch_one(pool)
        .await
        .expect("compatibilità alimento")
    }

    #[test]
    fn nomi_alimento_vengono_ripuliti_e_normalizzati() {
        assert_eq!(
            clean_food_name("  Petto   di pollo  ").as_deref(),
            Some("Petto di pollo")
        );
        assert_eq!(normalize_name("  Petto   DI Pollo "), "petto di pollo");
        assert!(clean_food_name("   ").is_none());
    }

    #[tokio::test]
    async fn catalogo_base_viene_seedato_ed_e_visibile() {
        let pool = test_pool().await;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM alimenti WHERE catalogo_globale = 1 AND archiviato = 0",
        )
        .fetch_one(&pool)
        .await
        .expect("conteggio catalogo base");

        assert_eq!(count, 421);

        let pollo: (String, String) = sqlx::query_as(
            "SELECT nome, nome_normalizzato FROM alimenti \
             WHERE catalogo_globale = 1 AND nome_normalizzato = 'petto di pollo'",
        )
        .fetch_one(&pool)
        .await
        .expect("petto di pollo base");

        assert_eq!(pollo.0, "🥩 Petto di pollo");
        assert_eq!(pollo.1, "petto di pollo");
    }

    #[tokio::test]
    async fn catalogo_base_include_le_gallette_in_cereali() {
        let pool = test_pool().await;
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT a.nome_normalizzato, c.codice FROM alimenti a \
             JOIN alimento_categorie ac ON ac.alimento_id = a.id \
             JOIN categorie_alimento c ON c.id = ac.categoria_id \
             WHERE a.catalogo_globale = 1 \
               AND a.nome_normalizzato IN ('gallette', 'gallette di riso', 'gallette di mais') \
             ORDER BY a.nome_normalizzato",
        )
        .fetch_all(&pool)
        .await
        .expect("gallette del catalogo");

        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|(_, category)| category == "cereali"));
    }

    #[tokio::test]
    async fn catalogo_base_e_modificabile_solo_da_admin_e_non_ha_condivisione() {
        let pool = test_pool().await;
        let admin_id = create_user(&pool, "Admin catalogo").await;
        let user_id = create_user(&pool, "Utente catalogo").await;

        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await
            .expect("promozione admin");

        let food_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE catalogo_globale = 1 AND nome_normalizzato = 'petto di pollo'",
        )
        .fetch_one(&pool)
        .await
        .expect("alimento base");

        assert!(can_edit_food(&pool, food_id, admin_id)
            .await
            .expect("edit admin"));
        assert!(!can_edit_food(&pool, food_id, user_id)
            .await
            .expect("edit utente"));
        assert!(!can_manage_food(&pool, food_id, admin_id)
            .await
            .expect("manage catalogo globale"));
    }

    #[tokio::test]
    async fn catalogo_base_ha_compatibilita_alimentare_completa() {
        let pool = test_pool().await;

        let labels: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM etichette_alimentari WHERE attiva = 1")
                .fetch_one(&pool)
                .await
                .expect("conteggio etichette");
        assert_eq!(labels, 19);

        let foods: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM alimenti WHERE catalogo_globale = 1 AND archiviato = 0",
        )
        .fetch_one(&pool)
        .await
        .expect("conteggio alimenti base");
        assert_eq!(foods, 421);

        let assignments: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) \
             FROM alimento_compatibilita ac \
             JOIN alimenti a ON a.id = ac.alimento_id \
             WHERE a.catalogo_globale = 1 AND a.archiviato = 0",
        )
        .fetch_one(&pool)
        .await
        .expect("conteggio compatibilità");
        assert_eq!(assignments, foods * labels);

        let missing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) \
             FROM alimenti a \
             CROSS JOIN etichette_alimentari e \
             LEFT JOIN alimento_compatibilita ac \
               ON ac.alimento_id = a.id AND ac.etichetta_id = e.id \
             WHERE a.catalogo_globale = 1 \
               AND a.archiviato = 0 \
               AND e.attiva = 1 \
               AND ac.alimento_id IS NULL",
        )
        .fetch_one(&pool)
        .await
        .expect("compatibilità mancanti");
        assert_eq!(missing, 0);
    }

    #[tokio::test]
    async fn compatibilita_catalogo_base_copre_casi_significativi() {
        let pool = test_pool().await;

        assert_eq!(compatibility_status(&pool, "riso", "vegano").await, "si");
        assert_eq!(
            compatibility_status(&pool, "riso", "senza_glutine").await,
            "si"
        );
        assert_eq!(
            compatibility_status(&pool, "petto di pollo", "vegano").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "tofu", "senza_soia").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "latte intero", "senza_lattosio").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "grana padano", "senza_uova").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "tagliatelle", "senza_glutine").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "salsa di soia", "senza_soia").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "gamberi", "senza_crostacei").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "cozze", "senza_molluschi").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "sedano", "senza_sedano").await,
            "no"
        );
        assert_eq!(
            compatibility_status(&pool, "birra analcolica", "senza_alcol").await,
            "verificare"
        );
        assert_eq!(
            compatibility_status(&pool, "uvetta", "senza_solfiti").await,
            "verificare"
        );
    }

    #[test]
    fn paginazione_alimenti_usa_cinque_elementi() {
        assert_eq!(FOOD_PAGE_SIZE, 5);
        assert_eq!(page_offset(0), 0);
        assert_eq!(page_offset(1), 5);
        assert_eq!(page_offset(3), 15);
    }

    #[test]
    fn callback_formati_restano_nel_limite_telegram() {
        let id = i64::MAX;
        let callbacks = [
            format!("food:product:formats:{id}"),
            format!("food:product:format:new:{id}"),
            format!("food:product:format:changeunit:{id}"),
            format!("food:product:format:unit:{id}:{id}"),
            format!("food:format:view:{id}"),
            format!("food:format:edit:quantity:{id}"),
            format!("food:format:edit:changeunit:{id}"),
            format!("food:format:edit:unit:{id}:{id}"),
            format!("food:format:edit:ean:{id}"),
        ];
        assert!(callbacks.iter().all(|callback| callback.len() <= 64));
    }

    #[tokio::test]
    async fn elenco_catalogo_base_puo_raggiungere_pagine_successive() {
        let pool = test_pool().await;
        let rows = list_foods_with_offset(&pool, None, None, FOOD_PAGE_FETCH, page_offset(1))
            .await
            .expect("seconda pagina catalogo");
        let (foods, has_next) = split_food_page(rows);
        assert_eq!(foods.len(), FOOD_PAGE_SIZE);
        assert!(has_next);
    }

    #[tokio::test]
    async fn secondo_utente_in_spazio_personale_legge_catalogo_e_pagine() {
        let pool = test_pool().await;
        let bootstrap_id = create_user(&pool, "Bootstrap").await;
        add_membership(&pool, 1, bootstrap_id, "proprietario").await;

        let user_id = create_user(&pool, "Secondo utente").await;
        let personal_space = sqlx::query(
            "INSERT INTO spazi (nome, tipo, creato_da_utente_id) \
             VALUES ('Spazio personale secondo utente', 'personale', ?)",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("spazio personale")
        .last_insert_rowid();
        add_membership(&pool, personal_space, user_id, "proprietario").await;

        identity::with_actor(
            actor(user_id, personal_space, false, "Secondo utente"),
            async {
                let total = count_foods(&pool, None, None)
                    .await
                    .expect("conteggio catalogo secondo utente");
                assert_eq!(total, 421);

                let rows =
                    list_foods_with_offset(&pool, None, None, FOOD_PAGE_FETCH, page_offset(1))
                        .await
                        .expect("seconda pagina secondo utente");
                let (foods, has_next) = split_food_page(rows);
                assert_eq!(foods.len(), FOOD_PAGE_SIZE);
                assert!(has_next);
                assert!(foods.iter().all(|food| food.global_catalog == 1));
            },
        )
        .await;
    }

    #[tokio::test]
    async fn prodotto_commerciale_si_collega_all_alimento_senza_sostituirlo() {
        let pool = test_pool().await;
        let admin_id = create_user(&pool, "Admin prodotti").await;
        add_membership(&pool, 1, admin_id, "proprietario").await;
        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await
            .expect("promozione admin");
        let food_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE catalogo_globale = 1 AND nome_normalizzato = 'formaggio spalmabile'",
        )
        .fetch_one(&pool)
        .await
        .expect("formaggio spalmabile base");
        let unit_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
            .fetch_one(&pool)
            .await
            .expect("unità grammi");
        let actor = actor(admin_id, 1, true, "Admin prodotti");
        identity::with_actor(actor, async {
            create_product_association(&pool, food_id, "Philadelphia", "Original", 200.0, unit_id)
                .await
                .expect("prodotto associato");
        })
        .await;

        let products = list_products_for_food(&pool, food_id)
            .await
            .expect("prodotti alimento");
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].brand, "Philadelphia");
        assert_eq!(products[0].product_name, "Original");
        assert_eq!(products[0].format_count, 1);
        let formats = list_product_formats(&pool, products[0].id)
            .await
            .expect("formati prodotto");
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].package_quantity, 200.0);
        assert_eq!(formats[0].package_unit_symbol, "g");
    }

    #[tokio::test]
    async fn stesso_prodotto_puo_avere_piu_formati_senza_duplicarsi() {
        let pool = test_pool().await;
        let admin_id = create_user(&pool, "Admin formati").await;
        add_membership(&pool, 1, admin_id, "proprietario").await;
        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await
            .expect("promozione admin");
        let food_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE catalogo_globale = 1 AND nome_normalizzato = 'formaggio spalmabile'",
        )
        .fetch_one(&pool)
        .await
        .expect("formaggio spalmabile");
        let grammi_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
            .fetch_one(&pool)
            .await
            .expect("grammi");
        identity::with_actor(actor(admin_id, 1, true, "Admin formati"), async {
            let first = create_product_association(
                &pool,
                food_id,
                "Philadelphia",
                "Original",
                200.0,
                grammi_id,
            )
            .await
            .expect("primo formato");
            let second = create_product_association(
                &pool,
                food_id,
                "Philadelphia",
                "Original",
                350.0,
                grammi_id,
            )
            .await
            .expect("secondo formato");
            assert_eq!(first, second);
            assert!(create_product_format(&pool, first, 350.0, grammi_id)
                .await
                .is_err());
        })
        .await;

        let products = list_products_for_food(&pool, food_id)
            .await
            .expect("prodotti");
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].format_count, 2);
        let formats = list_product_formats(&pool, products[0].id)
            .await
            .expect("formati");
        assert_eq!(formats.len(), 2);
        assert_eq!(formats[0].package_quantity, 200.0);
        assert_eq!(formats[1].package_quantity, 350.0);
    }

    #[tokio::test]
    async fn ricerca_alimenti_trova_anche_marca_e_nome_commerciale() {
        let pool = test_pool().await;
        let admin_id = create_user(&pool, "Admin ricerca prodotti").await;
        add_membership(&pool, 1, admin_id, "proprietario").await;
        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await
            .expect("promozione admin");
        let food_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE catalogo_globale = 1 AND nome_normalizzato = 'formaggio spalmabile'",
        )
        .fetch_one(&pool)
        .await
        .expect("formaggio spalmabile");
        let unit_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
            .fetch_one(&pool)
            .await
            .expect("grammi");
        let actor = actor(admin_id, 1, true, "Admin ricerca prodotti");
        identity::with_actor(actor, async {
            create_product_association(&pool, food_id, "Philadelphia", "Original", 200.0, unit_id)
                .await
                .expect("prodotto");
            let by_brand = list_foods(&pool, Some("philadelphia"), None, 20)
                .await
                .expect("ricerca marca");
            let by_name = list_foods(&pool, Some("original"), None, 20)
                .await
                .expect("ricerca nome commerciale");
            assert_eq!(by_brand.iter().filter(|food| food.id == food_id).count(), 1);
            assert_eq!(by_name.iter().filter(|food| food.id == food_id).count(), 1);
        })
        .await;
    }

    #[tokio::test]
    async fn prodotto_commerciale_puo_essere_modificato_senza_cambiare_alimento() {
        let pool = test_pool().await;
        let admin_id = create_user(&pool, "Admin modifica prodotto").await;
        add_membership(&pool, 1, admin_id, "proprietario").await;
        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await
            .expect("promozione admin");
        let food_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE catalogo_globale = 1 AND nome_normalizzato = 'formaggio spalmabile'",
        )
        .fetch_one(&pool)
        .await
        .expect("formaggio spalmabile");
        let grammi_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
            .fetch_one(&pool)
            .await
            .expect("grammi");
        let actor = actor(admin_id, 1, true, "Admin modifica prodotto");
        identity::with_actor(actor, async {
            let product_id = create_product_association(
                &pool,
                food_id,
                "Philadelphia prova",
                "Original",
                200.0,
                grammi_id,
            )
            .await
            .expect("prodotto");
            update_product_brand(&pool, product_id, "Philadelphia")
                .await
                .expect("marca");
            update_product_name(&pool, product_id, "Original Light")
                .await
                .expect("nome");
            let format_id = list_product_formats(&pool, product_id)
                .await
                .expect("formati")
                .first()
                .expect("formato iniziale")
                .id;
            update_product_format_quantity(&pool, format_id, 175.0, grammi_id)
                .await
                .expect("quantità formato");
            update_product_format_ean(&pool, format_id, Some("1234567890123"))
                .await
                .expect("ean formato");
            let product = get_product(&pool, product_id)
                .await
                .expect("lettura")
                .expect("prodotto presente");
            let format = get_product_format(&pool, format_id)
                .await
                .expect("lettura formato")
                .expect("formato presente");
            assert_eq!(product.food_id, food_id);
            assert_eq!(product.brand, "Philadelphia");
            assert_eq!(product.product_name, "Original Light");
            assert_eq!(product.format_count, 1);
            assert_eq!(format.package_quantity, 175.0);
            assert_eq!(format.ean.as_deref(), Some("1234567890123"));
        })
        .await;
    }

    fn etichette_tastiera(markup: &InlineKeyboardMarkup) -> Vec<Vec<String>> {
        markup
            .inline_keyboard
            .iter()
            .map(|riga| riga.iter().map(|b| b.text.clone()).collect())
            .collect()
    }

    #[test]
    fn sopra_le_venti_voci_la_ricerca_viene_prima_dell_elenco() {
        // C6: con 422 alimenti l'elenco e' lungo 85 pagine, e sfogliarlo non
        // e' una strada. Il conteggio sta sul pulsante (C7).
        let righe = etichette_tastiera(&food_menu_keyboard_con_conteggio(Some(422)));
        assert_eq!(righe[0], vec!["🔎 Cerca"]);
        assert_eq!(righe[1], vec!["📋 Elenco alimenti · 422"]);
    }

    #[test]
    fn sotto_la_soglia_l_elenco_resta_la_prima_azione() {
        let righe = etichette_tastiera(&food_menu_keyboard_con_conteggio(Some(7)));
        assert_eq!(righe[0], vec!["📋 Elenco alimenti · 7"]);
        assert_eq!(righe[1], vec!["🔎 Cerca"]);
    }

    #[test]
    fn senza_conteggio_il_menu_resta_usabile() {
        // Se il conteggio non si legge, la sezione deve restare raggiungibile.
        let righe = etichette_tastiera(&food_menu_keyboard_con_conteggio(None));
        assert_eq!(righe[0], vec!["📋 Elenco alimenti"]);
    }

    #[test]
    fn intestazione_paginata_calcola_totale_e_pagine() {
        assert_eq!(result_count_label(1), "1 risultato");
        assert_eq!(result_count_label(37), "37 risultati");
        assert_eq!(liste::totale_pagine(37), 8);
        assert_eq!(clamp_page(1, 37), 1);
        assert_eq!(clamp_page(99, 37), 7);
    }

    #[test]
    fn valori_nutrizionali_accettano_campi_opzionali() {
        let values = parse_nutrition_values("225; 934; 21; 14; 4,3; 4,3; -; 5,4; 0,75")
            .expect("tabella nutrizionale valida");
        assert_eq!(values[0], Some(225.0));
        assert_eq!(values[6], None);
        assert_eq!(values[8], Some(0.75));

        let partial = parse_nutrition_values("225; 934; -")
            .expect("campi mancanti completati automaticamente");
        assert_eq!(partial[0], Some(225.0));
        assert_eq!(partial[1], Some(934.0));
        assert!(partial[2..].iter().all(Option::is_none));
        assert!(parse_nutrition_values("1;2;3;4;5;6;7;8;9;10").is_err());
    }

    #[tokio::test]
    async fn valori_nutrizionali_prodotto_sono_facoltativi_e_salvabili() {
        let pool = test_pool().await;
        let admin_id = create_user(&pool, "Admin nutrizione").await;
        add_membership(&pool, 1, admin_id, "proprietario").await;
        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await
            .expect("promozione admin");
        let food_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE catalogo_globale = 1 AND nome_normalizzato = 'formaggio spalmabile'",
        )
        .fetch_one(&pool)
        .await
        .expect("formaggio spalmabile");
        let unit_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
            .fetch_one(&pool)
            .await
            .expect("grammi");
        let actor = actor(admin_id, 1, true, "Admin nutrizione");
        let product_id = identity::with_actor(actor.clone(), async {
            create_product_association(&pool, food_id, "Philadelphia", "Original", 200.0, unit_id)
                .await
                .expect("prodotto")
        })
        .await;
        assert!(get_product_nutrition(&pool, product_id)
            .await
            .expect("nutrizione iniziale")
            .is_none());
        let values = parse_nutrition_values("225;934;21;14;4.3;4.3;-;5.4;0.75").expect("valori");
        identity::with_actor(actor, async {
            save_product_nutrition(&pool, product_id, unit_id, &values)
                .await
                .expect("salvataggio nutrizione");
        })
        .await;
        let nutrition = get_product_nutrition(&pool, product_id)
            .await
            .expect("nutrizione")
            .expect("presente");
        assert_eq!(nutrition.energy_kcal, Some(225.0));
        assert_eq!(nutrition.fibre_g, None);
    }

    #[tokio::test]
    async fn storico_prodotti_registra_creazione_modifiche_e_nutrizione() {
        let pool = test_pool().await;
        let admin_id = create_user(&pool, "Admin storico prodotti").await;
        add_membership(&pool, 1, admin_id, "proprietario").await;
        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(admin_id)
            .execute(&pool)
            .await
            .expect("promozione admin");
        let food_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE catalogo_globale = 1 AND nome_normalizzato = 'formaggio spalmabile'",
        )
        .fetch_one(&pool)
        .await
        .expect("formaggio spalmabile");
        let unit_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
            .fetch_one(&pool)
            .await
            .expect("grammi");
        let actor = actor(admin_id, 1, true, "Admin storico prodotti");

        let product_id = identity::with_actor(actor.clone(), async {
            create_product_association(&pool, food_id, "Philadelphia", "Original", 200.0, unit_id)
                .await
                .expect("creazione prodotto")
        })
        .await;

        identity::with_actor(actor.clone(), async {
            update_product_brand(&pool, product_id, "Philadelphia prova")
                .await
                .expect("modifica marca");
            let values =
                parse_nutrition_values("225;934;-;14").expect("valori nutrizionali parziali");
            save_product_nutrition(&pool, product_id, unit_id, &values)
                .await
                .expect("salvataggio nutrizione");
            remove_product_nutrition(&pool, product_id)
                .await
                .expect("rimozione nutrizione");
        })
        .await;

        let events: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT e.modulo, e.componente, e.operazione \
             FROM storico_eventi e \
             JOIN storico_entita se ON se.id = e.entita_storico_id \
             WHERE se.tipo_entita = 'prodotto_alimentare' AND se.id_origine = ? \
             ORDER BY e.id",
        )
        .bind(product_id)
        .fetch_all(&pool)
        .await
        .expect("eventi prodotto");

        assert_eq!(events.len(), 5);
        assert_eq!(events[0].0.as_str(), "alimentazione");
        assert_eq!(events[0].1.as_str(), "prodotto_commerciale");
        assert_eq!(events[0].2.as_str(), "creazione");
        assert_eq!(events[1].1.as_str(), "formato_prodotto");
        assert_eq!(events[1].2.as_str(), "modifica");
        assert_eq!(events[2].1.as_str(), "prodotto_commerciale");
        assert_eq!(events[2].2.as_str(), "modifica");
        assert_eq!(events[3].1.as_str(), "valori_nutrizionali");
        assert_eq!(events[4].1.as_str(), "valori_nutrizionali");

        let fields: Vec<String> = sqlx::query_scalar(
            "SELECT sc.campo \
             FROM storico_cambiamenti sc \
             JOIN storico_eventi e ON e.id = sc.evento_id \
             JOIN storico_entita se ON se.id = e.entita_storico_id \
             WHERE se.tipo_entita = 'prodotto_alimentare' AND se.id_origine = ?",
        )
        .bind(product_id)
        .fetch_all(&pool)
        .await
        .expect("cambiamenti prodotto");
        assert!(fields.iter().any(|field| field == "alimento_associato"));
        assert!(fields.iter().any(|field| field == "marca"));
        assert!(fields.iter().any(|field| field == "confezione"));
        assert!(fields.iter().any(|field| field == "energia_kcal"));
        assert!(fields
            .iter()
            .any(|field| field == "riferimento_nutrizionale"));
    }

    #[tokio::test]
    async fn prodotto_specifico_ricetta_deve_appartenere_all_alimento() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester ricetta prodotto").await;
        add_membership(&pool, 1, user_id, "proprietario").await;
        sqlx::query("UPDATE utenti SET ruolo_sistema = 'admin' WHERE id = ?")
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("admin test");
        let formaggio_id: i64 = sqlx::query_scalar(
            "SELECT id FROM alimenti WHERE nome_normalizzato = 'formaggio spalmabile'",
        )
        .fetch_one(&pool)
        .await
        .expect("formaggio");
        let riso_id: i64 =
            sqlx::query_scalar("SELECT id FROM alimenti WHERE nome_normalizzato = 'riso'")
                .fetch_one(&pool)
                .await
                .expect("riso");
        let grammi_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
            .fetch_one(&pool)
            .await
            .expect("grammi");
        let product_id =
            identity::with_actor(actor(user_id, 1, true, "Tester ricetta prodotto"), async {
                create_product_association(
                    &pool,
                    formaggio_id,
                    "Philadelphia",
                    "Original",
                    200.0,
                    grammi_id,
                )
                .await
                .expect("prodotto")
            })
            .await;
        let recipe_id = sqlx::query(
            "INSERT INTO ricette (proprietario_utente_id, nome, nome_normalizzato) VALUES (?, 'Test', 'test')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("ricetta")
        .last_insert_rowid();
        let invalid = sqlx::query(
            "INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, prodotto_alimentare_id, quantita, unita_misura_id) VALUES (?, ?, ?, 100, ?)",
        )
        .bind(recipe_id)
        .bind(riso_id)
        .bind(product_id)
        .bind(grammi_id)
        .execute(&pool)
        .await;
        assert!(invalid.is_err());
        sqlx::query(
            "INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, prodotto_alimentare_id, quantita, unita_misura_id) VALUES (?, ?, ?, 100, ?)",
        )
        .bind(recipe_id)
        .bind(formaggio_id)
        .bind(product_id)
        .bind(grammi_id)
        .execute(&pool)
        .await
        .expect("prodotto coerente");
    }

    #[tokio::test]
    async fn alimento_personale_viene_creato_elencato_e_cercato() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let unit_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE codice = 'g'")
            .fetch_one(&pool)
            .await
            .expect("unita g");

        let id = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Petto di pollo test personale", Some(unit_id), &[])
                .await
                .expect("creazione alimento")
        })
        .await;

        let foods = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            list_foods(&pool, None, None, 20)
                .await
                .expect("elenco alimenti")
        })
        .await;

        let personal = foods
            .iter()
            .find(|food| food.id == id)
            .expect("alimento personale nell'elenco");
        assert_eq!(personal.owner_user_id, Some(user_id));
        assert_eq!(personal.unit_code.as_deref(), Some("g"));

        let found = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            list_foods(&pool, Some("POLLO TEST PERSONALE"), None, 20)
                .await
                .expect("ricerca alimenti")
        })
        .await;
        assert_eq!(found.len(), 1);
    }

    #[tokio::test]
    async fn alimento_senza_unita_viene_rifiutato() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let result = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Senza unita", None, &[]).await
        })
        .await;

        assert!(result.is_err());

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM alimenti WHERE nome_normalizzato = 'senza unità'",
        )
        .fetch_one(&pool)
        .await
        .expect("conteggio alimento senza unita");

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn categorie_alimenti_sono_predisposte_e_default_altro() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let category_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM categorie_alimento WHERE attiva = 1")
                .fetch_one(&pool)
                .await
                .expect("conteggio categorie");
        assert_eq!(category_count, 12);

        let food_id = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Alimento categoria test", Some(1), &[])
                .await
                .expect("creazione alimento")
        })
        .await;

        let category_code: String = sqlx::query_scalar(
            "SELECT c.codice \
             FROM alimento_categorie ac \
             JOIN categorie_alimento c ON c.id = ac.categoria_id \
             WHERE ac.alimento_id = ?",
        )
        .bind(food_id)
        .fetch_one(&pool)
        .await
        .expect("categoria alimento");

        assert_eq!(category_code, "altro");
    }

    #[tokio::test]
    async fn duplicato_personale_viene_rifiutato() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Riso duplicato test", Some(1), &[])
                .await
                .expect("primo alimento");
            let duplicate = create_food(&pool, "  RISO DUPLICATO TEST ", Some(1), &[]).await;
            assert!(duplicate.is_err());
        })
        .await;
    }

    #[tokio::test]
    async fn alimento_puo_essere_condiviso_con_spazi_selezionati() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let space_2 =
            sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Condiviso', 'condiviso')")
                .execute(&pool)
                .await
                .expect("spazio 2")
                .last_insert_rowid();
        add_membership(&pool, space_2, user_id, "membro").await;

        let food_id = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Pasta condivisione test", Some(1), &[1, space_2])
                .await
                .expect("alimento condiviso")
        })
        .await;

        let spaces: Vec<i64> = sqlx::query_scalar(
            "SELECT spazio_id FROM alimento_spazi \
             WHERE alimento_id = ? ORDER BY spazio_id",
        )
        .bind(food_id)
        .fetch_all(&pool)
        .await
        .expect("spazi alimento");

        assert_eq!(spaces, vec![1, space_2]);
    }

    #[tokio::test]
    async fn proprietario_conserva_alimento_anche_se_perde_membership() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let space_2 = sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Comune', 'condiviso')")
            .execute(&pool)
            .await
            .expect("spazio 2")
            .last_insert_rowid();
        add_membership(&pool, space_2, user_id, "membro").await;

        let food_id = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Avena ownership test", Some(1), &[space_2])
                .await
                .expect("creazione alimento")
        })
        .await;

        sqlx::query("DELETE FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?")
            .bind(space_2)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("rimozione membership");

        let foods = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            list_foods(&pool, None, None, 20)
                .await
                .expect("catalogo personale")
        })
        .await;

        assert!(foods.iter().any(|food| food.id == food_id));
    }

    #[tokio::test]
    async fn alimento_altrui_compare_solo_finche_esiste_uno_spazio_comune() {
        let pool = test_pool().await;

        let owner_id = create_user(&pool, "Owner").await;
        let guest_id = create_user(&pool, "Guest").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;

        let shared_space =
            sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Casa comune', 'condiviso')")
                .execute(&pool)
                .await
                .expect("spazio comune")
                .last_insert_rowid();

        add_membership(&pool, shared_space, owner_id, "membro").await;
        add_membership(&pool, shared_space, guest_id, "lettura").await;

        let food_id = identity::with_actor(actor(owner_id, 1, true, "Owner"), async {
            create_food(&pool, "Cous cous visibilita test", Some(1), &[shared_space])
                .await
                .expect("alimento condiviso")
        })
        .await;

        let visible = identity::with_actor(actor(guest_id, shared_space, true, "Guest"), async {
            list_foods(&pool, None, None, 20)
                .await
                .expect("catalogo guest")
        })
        .await;
        assert!(visible.iter().any(|food| food.id == food_id));

        sqlx::query("DELETE FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?")
            .bind(shared_space)
            .bind(guest_id)
            .execute(&pool)
            .await
            .expect("rimozione guest");

        let hidden = identity::with_actor(actor(guest_id, 1, true, "Guest"), async {
            list_foods(&pool, None, None, 20)
                .await
                .expect("catalogo guest dopo rimozione")
        })
        .await;
        assert!(!hidden.iter().any(|food| food.id == food_id));
    }

    #[tokio::test]
    async fn filtro_categoria_restituisce_solo_alimenti_assegnati() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let bistecca_id = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Bistecca filtro", Some(1), &[])
                .await
                .expect("bistecca")
        })
        .await;

        let zucchina_id = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Zucchina filtro", Some(1), &[])
                .await
                .expect("zucchina")
        })
        .await;

        let carne_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'carne'")
                .fetch_one(&pool)
                .await
                .expect("carne");
        let verdura_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'verdura'")
                .fetch_one(&pool)
                .await
                .expect("verdura");

        identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            set_food_category(&pool, bistecca_id, carne_id)
                .await
                .expect("categoria carne");
            set_food_category(&pool, zucchina_id, verdura_id)
                .await
                .expect("categoria verdura");
        })
        .await;

        let carne = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            list_foods(&pool, None, Some(carne_id), 20)
                .await
                .expect("filtro carne")
        })
        .await;

        assert!(carne.iter().any(|food| food.id == bistecca_id));
        assert!(!carne.iter().any(|food| food.id == zucchina_id));
    }

    #[tokio::test]
    async fn filtro_multi_categoria_unisce_risultati_senza_duplicati() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester multi").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let bistecca_id = identity::with_actor(actor(user_id, 1, false, "Tester multi"), async {
            create_food(&pool, "Bistecca multi", Some(1), &[])
                .await
                .expect("bistecca")
        })
        .await;

        let zucchina_id = identity::with_actor(actor(user_id, 1, false, "Tester multi"), async {
            create_food(&pool, "Zucchina multi", Some(1), &[])
                .await
                .expect("zucchina")
        })
        .await;

        let riso_id = identity::with_actor(actor(user_id, 1, false, "Tester multi"), async {
            create_food(&pool, "Riso multi", Some(1), &[])
                .await
                .expect("riso")
        })
        .await;

        let carne_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'carne'")
                .fetch_one(&pool)
                .await
                .expect("carne");
        let verdura_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'verdura'")
                .fetch_one(&pool)
                .await
                .expect("verdura");
        let cereali_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'cereali'")
                .fetch_one(&pool)
                .await
                .expect("cereali");

        identity::with_actor(actor(user_id, 1, false, "Tester multi"), async {
            set_food_category(&pool, bistecca_id, carne_id)
                .await
                .expect("categoria carne");
            set_food_category(&pool, zucchina_id, verdura_id)
                .await
                .expect("categoria verdura");
            set_food_category(&pool, riso_id, cereali_id)
                .await
                .expect("categoria cereali");
        })
        .await;

        sqlx::query(
            "INSERT INTO alimento_categorie (\
                alimento_id, categoria_id, assegnata_da_utente_id\
             ) VALUES (?, ?, ?)",
        )
        .bind(bistecca_id)
        .bind(verdura_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("seconda categoria test");

        let foods = identity::with_actor(actor(user_id, 1, false, "Tester multi"), async {
            let mut foods = Vec::new();
            let mut page = 0;

            loop {
                let (rows, has_next, _total, resolved_page) =
                    list_foods_multi_categories_page(&pool, &[carne_id, verdura_id], page)
                        .await
                        .expect("filtro multi paginato");
                assert_eq!(resolved_page, page);
                foods.extend(rows);

                if !has_next {
                    break;
                }
                page += 1;
            }

            foods
        })
        .await;

        assert!(foods.iter().any(|food| food.id == bistecca_id));
        assert!(foods.iter().any(|food| food.id == zucchina_id));
        assert!(!foods.iter().any(|food| food.id == riso_id));
        assert_eq!(
            foods.iter().filter(|food| food.id == bistecca_id).count(),
            1
        );
    }

    #[tokio::test]
    async fn solo_proprietario_puo_modificare_categoria_alimento() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner").await;
        let other_id = create_user(&pool, "Other").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        add_membership(&pool, 1, other_id, "membro").await;

        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner"), async {
            create_food(&pool, "Pollo categoria owner", Some(1), &[1])
                .await
                .expect("alimento owner")
        })
        .await;

        let carne_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'carne'")
                .fetch_one(&pool)
                .await
                .expect("carne");

        let result = identity::with_actor(actor(other_id, 1, false, "Other"), async {
            set_food_category(&pool, food_id, carne_id).await
        })
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn visibilita_alimento_non_concede_modifica() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner visibilita").await;
        let other_id = create_user(&pool, "Other visibilita").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        add_membership(&pool, 1, other_id, "membro").await;

        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner visibilita"), async {
            create_food(&pool, "Alimento solo visibile", Some(1), &[1])
                .await
                .expect("food")
        })
        .await;

        assert!(food_visible_to_user(&pool, food_id, other_id)
            .await
            .expect("visible"));
        assert!(!can_edit_food(&pool, food_id, other_id).await.expect("edit"));
    }

    #[tokio::test]
    async fn permesso_esplicito_consente_modifica_categoria() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner permesso").await;
        let other_id = create_user(&pool, "Other permesso").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        add_membership(&pool, 1, other_id, "membro").await;

        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner permesso"), async {
            create_food(&pool, "Alimento collaborativo", Some(1), &[1])
                .await
                .expect("food")
        })
        .await;

        sqlx::query(
            "INSERT INTO permessi_risorsa (\
                tipo_risorsa, risorsa_id, utente_id, puo_modificare, \
                puo_gestire_permessi, concesso_da_utente_id\
             ) VALUES ('alimento', ?, ?, 1, 0, ?)",
        )
        .bind(food_id)
        .bind(other_id)
        .bind(owner_id)
        .execute(&pool)
        .await
        .expect("permesso");

        let carne_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'carne'")
                .fetch_one(&pool)
                .await
                .expect("carne");

        identity::with_actor(actor(other_id, 1, false, "Other permesso"), async {
            set_food_category(&pool, food_id, carne_id)
                .await
                .expect("categoria da collaboratore");
        })
        .await;
    }

    #[tokio::test]
    async fn perdita_visibilita_disattiva_permesso_alimento() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner perdita").await;
        let other_id = create_user(&pool, "Other perdita").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        add_membership(&pool, 1, other_id, "membro").await;

        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner perdita"), async {
            create_food(&pool, "Alimento perdita visibilita", Some(1), &[1])
                .await
                .expect("food")
        })
        .await;

        sqlx::query(
            "INSERT INTO permessi_risorsa (\
                tipo_risorsa, risorsa_id, utente_id, puo_modificare, \
                puo_gestire_permessi, concesso_da_utente_id\
             ) VALUES ('alimento', ?, ?, 1, 0, ?)",
        )
        .bind(food_id)
        .bind(other_id)
        .bind(owner_id)
        .execute(&pool)
        .await
        .expect("permesso");

        assert!(can_edit_food(&pool, food_id, other_id)
            .await
            .expect("edit before"));
        sqlx::query("DELETE FROM membri_spazio WHERE spazio_id = 1 AND utente_id = ?")
            .bind(other_id)
            .execute(&pool)
            .await
            .expect("remove membership");
        assert!(!can_edit_food(&pool, food_id, other_id)
            .await
            .expect("edit after"));
    }

    #[tokio::test]
    async fn creazione_categoria_esplicita_sostituisce_altro() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Create category").await;
        add_membership(&pool, 1, user_id, "proprietario").await;
        let carne_id: i64 =
            sqlx::query_scalar("SELECT id FROM categorie_alimento WHERE codice = 'carne'")
                .fetch_one(&pool)
                .await
                .expect("carne");
        let food_id = identity::with_actor(actor(user_id, 1, false, "Create category"), async {
            create_food_with_category(&pool, "Bistecca esplicita", Some(1), Some(carne_id), &[])
                .await
                .expect("food")
        })
        .await;
        let categories: Vec<i64> =
            sqlx::query_scalar("SELECT categoria_id FROM alimento_categorie WHERE alimento_id = ?")
                .bind(food_id)
                .fetch_all(&pool)
                .await
                .expect("categories");
        assert_eq!(categories, vec![carne_id]);
    }

    #[tokio::test]
    async fn proprietario_puo_modificare_nome_e_unita() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner edit").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner edit"), async {
            create_food(&pool, "Nome prima", Some(1), &[])
                .await
                .expect("food")
        })
        .await;
        identity::with_actor(actor(owner_id, 1, false, "Owner edit"), async {
            update_food_name(&pool, food_id, "Nome dopo")
                .await
                .expect("name");
            update_food_unit(&pool, food_id, 2).await.expect("unit");
        })
        .await;
        let row: (String, i64) =
            sqlx::query_as("SELECT nome, unita_predefinita_id FROM alimenti WHERE id = ?")
                .bind(food_id)
                .fetch_one(&pool)
                .await
                .expect("row");
        assert_eq!(row.0, "Nome dopo");
        assert_eq!(row.1, 2);
    }

    #[tokio::test]
    async fn permesso_modifica_non_concede_gestione_visibilita() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner edit only").await;
        let other_id = create_user(&pool, "Editor only").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        add_membership(&pool, 1, other_id, "membro").await;
        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner edit only"), async {
            create_food(&pool, "Edit only food", Some(1), &[1])
                .await
                .expect("food")
        })
        .await;
        sqlx::query(
            "INSERT INTO permessi_risorsa (tipo_risorsa, risorsa_id, utente_id, puo_modificare, puo_gestire_permessi, concesso_da_utente_id) \
             VALUES ('alimento', ?, ?, 1, 0, ?)",
        )
        .bind(food_id).bind(other_id).bind(owner_id)
        .execute(&pool).await.expect("grant");
        assert!(can_edit_food(&pool, food_id, other_id).await.expect("edit"));
        assert!(!can_manage_food(&pool, food_id, other_id)
            .await
            .expect("manage"));
    }

    #[tokio::test]
    async fn permesso_gestione_consente_modifica_visibilita() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner manage").await;
        let other_id = create_user(&pool, "Manager").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        add_membership(&pool, 1, other_id, "membro").await;
        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner manage"), async {
            create_food(&pool, "Manage food", Some(1), &[1])
                .await
                .expect("food")
        })
        .await;
        sqlx::query(
            "INSERT INTO permessi_risorsa (tipo_risorsa, risorsa_id, utente_id, puo_modificare, puo_gestire_permessi, concesso_da_utente_id) \
             VALUES ('alimento', ?, ?, 1, 1, ?)",
        )
        .bind(food_id).bind(other_id).bind(owner_id)
        .execute(&pool).await.expect("grant");
        assert!(can_manage_food(&pool, food_id, other_id)
            .await
            .expect("manage"));
        identity::with_actor(actor(other_id, 1, false, "Manager"), async {
            replace_food_shares(&pool, food_id, &[])
                .await
                .expect("private");
        })
        .await;
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM alimento_spazi WHERE alimento_id = ?")
                .bind(food_id)
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn invito_accettato_attiva_permesso_modifica() {
        let pool = test_pool().await;
        let owner_id = create_user(&pool, "Owner invite flow").await;
        let other_id = create_user(&pool, "Invited flow").await;
        add_membership(&pool, 1, owner_id, "proprietario").await;
        add_membership(&pool, 1, other_id, "membro").await;
        let food_id = identity::with_actor(actor(owner_id, 1, false, "Owner invite flow"), async {
            create_food(&pool, "Invite flow food", Some(1), &[1])
                .await
                .expect("food")
        })
        .await;
        let invite_id = crate::resource_permissions::create_invite(
            &pool,
            "alimento",
            food_id,
            other_id,
            owner_id,
            crate::resource_permissions::ResourcePermission::Edit,
        )
        .await
        .expect("invite");
        crate::resource_permissions::accept_invite(&pool, invite_id, other_id)
            .await
            .expect("accept");
        assert!(can_edit_food(&pool, food_id, other_id).await.expect("edit"));
    }

    #[tokio::test]
    async fn ruolo_lettura_non_puo_condividere_ma_puo_creare_personale() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Reader").await;
        add_membership(&pool, 1, user_id, "lettura").await;

        identity::with_actor(actor(user_id, 1, false, "Reader"), async {
            create_food(&pool, "Mela", Some(1), &[])
                .await
                .expect("alimento personale consentito");

            let shared = create_food(&pool, "Pera", Some(1), &[1]).await;
            assert!(shared.is_err());
        })
        .await;
    }
}
