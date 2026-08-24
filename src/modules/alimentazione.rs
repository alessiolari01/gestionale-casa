//! Step 7.2B - backend e UI Telegram per gli alimenti.
//!
//! Proprietà e visibilità sono separate:
//! - l'alimento appartiene all'utente che lo crea;
//! - può essere condiviso con zero, uno o più spazi;
//! - perdere una membership non fa perdere gli alimenti posseduti;
//! - gli alimenti altrui condivisi in spazi comuni vengono letti dal DB
//!   centrale e quindi si sincronizzano senza creare copie.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::identity;

const LIST_LIMIT: i64 = 20;
const FOOD_NAME_MAX_CHARS: usize = 120;

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
    Visibility {
        name: String,
        unit_id: Option<i64>,
    },
    Spaces {
        name: String,
        unit_id: Option<i64>,
        selected: Vec<i64>,
    },
    Search,
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

pub async fn show_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🍽️ Alimentazione\n\n\
         Gli alimenti che crei restano tuoi e puoi decidere in quali spazi \
         renderli visibili.\n\n\
         Gli alimenti condivisi da altre persone negli spazi comuni vengono \
         letti direttamente dal catalogo centrale: usa 🔄 Aggiorna alimenti \
         per rileggere subito le novita'.\n\n\
         Comandi: /alimenti · /alimento_nuovo · /alimenti_lista · \
         /alimento_cerca · /alimento <id>",
    )
    .reply_markup(food_menu_keyboard())
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
            "/alimenti" | "/alimentazione" => {
                sessions.clear_chat(chat_id);
                show_menu(bot, msg.chat.id).await?;
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
                         Usa /annulla per uscire.",
                    )
                    .reply_markup(cancel_keyboard())
                    .await?;
                } else {
                    start_unit_choice(bot, msg.chat.id, chat_id, pool, sessions, args).await?;
                }
                return Ok(true);
            }
            "/alimenti_lista" | "/alimenti_aggiorna" => {
                sessions.clear_chat(chat_id);
                send_food_list(bot, msg.chat.id, pool, command == "/alimenti_aggiorna").await?;
                return Ok(true);
            }
            "/alimento_cerca" => {
                if args.is_empty() {
                    sessions.set(chat_id, FoodConversationState::Search);
                    bot.send_message(
                        msg.chat.id,
                        "🔎 Cerca alimento\n\n\
                         Scrivi il nome o un alias da cercare.\n\n\
                         Usa /annulla per uscire.",
                    )
                    .reply_markup(cancel_keyboard())
                    .await?;
                } else {
                    sessions.clear_chat(chat_id);
                    send_search_results(bot, msg.chat.id, pool, args).await?;
                }
                return Ok(true);
            }
            "/alimento" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_food_detail(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(msg.chat.id, "Uso: /alimento <id>\nEsempio: /alimento 4")
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
                        "❌ Operazione annullata. Nessuna bozza e nessun nuovo alimento sono stati salvati.",
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
                    start_visibility_choice(
                        bot,
                        msg.chat.id,
                        chat_id,
                        sessions,
                        name,
                        Some(unit.id),
                    )
                    .await?;
                }
                Ok(None) => {
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Unita' non riconosciuta.\n\n\
                         Scrivi uno tra: g, kg, ml, l, pz, cucchiaio, \
                         cucchiaino, qb.\n\
                         Scegli un'unita' per continuare.",
                    )
                    .reply_markup(
                        unit_keyboard_from_db(pool)
                            .await
                            .unwrap_or_else(|_| cancel_keyboard()),
                    )
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore ricerca unita' alimento");
                    bot.send_message(msg.chat.id, "⚠️ Non riesco a leggere le unita' di misura.")
                        .reply_markup(cancel_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        Some(FoodConversationState::Visibility { .. })
        | Some(FoodConversationState::Spaces { .. }) => {
            bot.send_message(
                msg.chat.id,
                "Usa i pulsanti per scegliere dove rendere visibile l'alimento, \
                 oppure /annulla.",
            )
            .await?;
            Ok(true)
        }
        Some(FoodConversationState::Search) => {
            sessions.clear_chat(chat_id);
            send_search_results(bot, msg.chat.id, pool, text).await?;
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
        "food:menu" => {
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id).await?;
            Ok(true)
        }
        "food:new" => {
            sessions.set(chat_id.0, FoodConversationState::Name);
            bot.send_message(
                chat_id,
                "➕ Nuovo alimento\n\n\
                 Scrivi il nome dell'alimento.\n\
                 Esempio: Pollo\n\n\
                 Usa /annulla per uscire.",
            )
            .reply_markup(cancel_keyboard())
            .await?;
            Ok(true)
        }
        "food:list" => {
            sessions.clear_chat(chat_id.0);
            send_food_list(bot, chat_id, pool, false).await?;
            Ok(true)
        }
        "food:refresh" => {
            sessions.clear_chat(chat_id.0);
            send_food_list(bot, chat_id, pool, true).await?;
            Ok(true)
        }
        "food:search" => {
            sessions.set(chat_id.0, FoodConversationState::Search);
            bot.send_message(
                chat_id,
                "🔎 Cerca alimento\n\n\
                 Scrivi il nome o un alias da cercare.\n\n\
                 Usa /annulla per uscire.",
            )
            .reply_markup(cancel_keyboard())
            .await?;
            Ok(true)
        }
        "food:cancel" => {
            let had_active_operation = sessions.has_active(chat_id.0);
            sessions.clear_chat(chat_id.0);
            if had_active_operation {
                bot.send_message(
                    chat_id,
                    "❌ Operazione annullata. Nessuna bozza e nessun nuovo alimento sono stati salvati.",
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
                    "❌ Operazione Alimentazione annullata. Nessuna bozza e nessun nuovo alimento sono stati salvati.",
                )
                .await?;
            }
            Ok(false)
        }
        "food:back" => {
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id).await?;
            Ok(true)
        }
        "food:new:back:name" => {
            sessions.set(chat_id.0, FoodConversationState::Name);
            bot.send_message(
                chat_id,
                "➕ Nuovo alimento\n\nScrivi il nome dell'alimento.\nEsempio: Pollo\n\nUsa /annulla per uscire.",
            )
            .reply_markup(cancel_keyboard())
            .await?;
            Ok(true)
        }
        "food:new:back:unit" => {
            let Some(FoodConversationState::Visibility { name, .. }) = sessions.get(chat_id.0)
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
                        format!("🥕 {name}\n\nScegli di nuovo l'unita' predefinita."),
                    )
                    .reply_markup(keyboard)
                    .await?;
                }
                Err(error) => {
                    tracing::error!(?error, "Errore ritorno scelta unita'");
                    bot.send_message(chat_id, "⚠️ Non riesco a leggere le unita' di misura.")
                        .reply_markup(visibility_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        "food:new:back:visibility" => {
            let Some(FoodConversationState::Spaces { name, unit_id, .. }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            sessions.set(
                chat_id.0,
                FoodConversationState::Visibility {
                    name: name.clone(),
                    unit_id,
                },
            );
            bot.send_message(
                chat_id,
                format!(
                    "👁 Visibilita' di {name}\n\nL'alimento restera' sempre di tua proprieta'.\nScegli dove renderlo visibile:"
                ),
            )
            .reply_markup(visibility_keyboard())
            .await?;
            Ok(true)
        }
        "food:new:visibility:private" => {
            let Some(FoodConversationState::Visibility { name, unit_id }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };
            finish_creation(bot, chat_id, chat_id.0, pool, sessions, &name, unit_id, &[]).await?;
            Ok(true)
        }
        "food:new:visibility:default" => {
            let Some(FoodConversationState::Visibility { name, unit_id }) = sessions.get(chat_id.0)
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
            let Some(FoodConversationState::Visibility { name, unit_id }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };
            match list_shareable_spaces(pool).await {
                Ok(spaces) => {
                    let ids: Vec<i64> = spaces.iter().map(|space| space.id).collect();
                    finish_creation(
                        bot, chat_id, chat_id.0, pool, sessions, &name, unit_id, &ids,
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
            let Some(FoodConversationState::Visibility { name, unit_id }) = sessions.get(chat_id.0)
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
                selected,
            }) = sessions.get(chat_id.0)
            else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            finish_creation(
                bot, chat_id, chat_id.0, pool, sessions, &name, unit_id, &selected,
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
                bot.send_message(chat_id, "Pulsante unita' non valido.")
                    .reply_markup(food_menu_keyboard())
                    .await?;
                return Ok(true);
            };

            let Some(FoodConversationState::Unit { name }) = sessions.get(chat_id.0) else {
                expired_creation(bot, chat_id).await?;
                return Ok(true);
            };

            start_visibility_choice(bot, chat_id, chat_id.0, sessions, name, Some(unit_id)).await?;
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
    bot.send_message(chat_id, "La creazione alimento non e' piu' attiva.")
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
                 Riprova oppure usa /annulla."
            ),
        )
        .reply_markup(cancel_keyboard())
        .await?;
        return Ok(());
    };

    let keyboard = match unit_keyboard_from_db(pool).await {
        Ok(keyboard) => keyboard,
        Err(error) => {
            tracing::error!(?error, "Errore caricamento unita' alimento");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le unita' di misura.")
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
             Scegli l'unita' predefinita.\n\
             Puoi anche scrivere g, kg, ml, l, pz, cucchiaio, \
             cucchiaino o qb.\n\
             L'unita' e' obbligatoria per salvare l'alimento."
        ),
    )
    .reply_markup(keyboard)
    .await?;
    Ok(())
}

async fn start_visibility_choice(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &FoodSessionStore,
    name: String,
    unit_id: Option<i64>,
) -> ResponseResult<()> {
    sessions.set(
        raw_chat_id,
        FoodConversationState::Visibility {
            name: name.clone(),
            unit_id,
        },
    );

    bot.send_message(
        chat_id,
        format!(
            "👁 Visibilita' di {name}\n\n\
             L'alimento restera' sempre di tua proprieta'.\n\
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
    shared_spaces: &[i64],
) -> ResponseResult<()> {
    match create_food(pool, name, unit_id, shared_spaces).await {
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
    refreshed: bool,
) -> ResponseResult<()> {
    match list_foods(pool, None, LIST_LIMIT).await {
        Ok(foods) if foods.is_empty() => {
            let title = if refreshed {
                "🔄 Alimenti aggiornati"
            } else {
                "📋 Alimenti"
            };
            bot.send_message(
                chat_id,
                format!("{title}\n\nNessun alimento disponibile nella vista corrente."),
            )
            .reply_markup(food_menu_keyboard())
            .await?;
        }
        Ok(foods) => {
            let title = if refreshed {
                "🔄 Alimenti aggiornati"
            } else {
                "📋 Alimenti"
            };
            let current_user = identity::current_actor().utente_id;
            let mut text = format!("{title} · primi {}\n\n", foods.len());
            for food in &foods {
                text.push_str(&food_summary_line(food, current_user));
                text.push('\n');
            }
            bot.send_message(chat_id, text)
                .reply_markup(food_results_keyboard(&foods))
                .await?;
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
) -> ResponseResult<()> {
    let normalized = normalize_name(raw_query);
    if normalized.is_empty() {
        bot.send_message(chat_id, "⚠️ Scrivi almeno un termine da cercare.")
            .reply_markup(food_menu_keyboard())
            .await?;
        return Ok(());
    }

    match list_foods(pool, Some(&normalized), LIST_LIMIT).await {
        Ok(foods) if foods.is_empty() => {
            bot.send_message(
                chat_id,
                format!("🔎 Nessun alimento trovato per: {raw_query}"),
            )
            .reply_markup(food_menu_keyboard())
            .await?;
        }
        Ok(foods) => {
            let current_user = identity::current_actor().utente_id;
            let mut text = format!("🔎 Risultati per: {raw_query}\n\n");
            for food in &foods {
                text.push_str(&food_summary_line(food, current_user));
                text.push('\n');
            }
            bot.send_message(chat_id, text)
                .reply_markup(food_results_keyboard(&foods))
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
            let mut lines = vec![
                format!("🥕 {}", food.name),
                format!("ID: #{}", food.id),
                String::new(),
                ownership_label(&food, actor.utente_id),
            ];

            if let Some(symbol) = food.unit_symbol.as_deref() {
                let code = food.unit_code.as_deref().unwrap_or(symbol);
                lines.push(format!("⚖️ Unita' predefinita: {symbol} ({code})"));
            } else {
                lines.push("⚖️ Unita' predefinita: non impostata".to_string());
            }

            if food.global_catalog == 0 {
                match visible_share_names(pool, food.id, actor.utente_id, is_owner).await {
                    Ok(names) if names.is_empty() => {
                        lines.push("🔒 Visibilita': solo personale".to_string());
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
                .reply_markup(food_detail_keyboard())
                .await?;
        }
        Ok(None) => {
            bot.send_message(
                chat_id,
                format!("Alimento #{id} non disponibile nella vista corrente."),
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

async fn create_food(
    pool: &SqlitePool,
    raw_name: &str,
    unit_id: Option<i64>,
    shared_spaces: &[i64],
) -> Result<i64> {
    let actor = identity::current_actor();
    let owner_user_id = actor
        .utente_id
        .context("Identita' utente non disponibile")?;

    let name = clean_food_name(raw_name).context("Il nome dell'alimento non e' valido")?;
    let normalized = normalize_name(&name);

    let unit_id = unit_id.context("Scegli un'unita' di misura prima di salvare")?;

    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM unita_misura WHERE id = ? AND attiva = 1)")
            .bind(unit_id)
            .fetch_one(pool)
            .await
            .context("Impossibile verificare l'unita' di misura")?;
    if !valid {
        bail!("Unita' di misura non disponibile");
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
        bail!("Esiste gia' un alimento con questo nome nel catalogo globale");
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
        bail!("Hai gia' un alimento con questo nome");
    }

    let mut unique_spaces = shared_spaces.to_vec();
    unique_spaces.sort_unstable();
    unique_spaces.dedup();

    for space_id in &unique_spaces {
        if !user_can_share_to_space(pool, owner_user_id, *space_id).await? {
            bail!("Non hai diritto di condividere alimenti nello spazio #{space_id}");
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
            bail!("Uno degli spazi selezionati possiede gia' un alimento con questo nome");
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
    let user_id = actor
        .utente_id
        .context("Identita' utente non disponibile")?;

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

async fn list_foods(
    pool: &SqlitePool,
    normalized_search: Option<&str>,
    limit: i64,
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
                   ) \
                 ORDER BY CASE \
                            WHEN a.catalogo_globale = 1 THEN 2 \
                            WHEN a.proprietario_utente_id = ? THEN 0 \
                            ELSE 1 \
                          END, \
                          a.nome COLLATE NOCASE, a.id \
                 LIMIT ?"
            );
            sqlx::query_as::<_, FoodRecord>(&sql)
                .bind(user_id)
                .bind(user_id)
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(user_id)
                .bind(limit)
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
                   ) \
                 ORDER BY CASE \
                            WHEN a.catalogo_globale = 1 THEN 2 \
                            WHEN a.proprietario_utente_id = ? THEN 0 \
                            ELSE 1 \
                          END, \
                          a.nome COLLATE NOCASE, a.id \
                 LIMIT ?"
            );
            sqlx::query_as::<_, FoodRecord>(&sql)
                .bind(user_id)
                .bind(actor.spazio_id)
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(search.as_deref())
                .bind(user_id)
                .bind(limit)
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
               ) \
             ORDER BY a.nome COLLATE NOCASE, a.id \
             LIMIT ?"
        );
        sqlx::query_as::<_, FoodRecord>(&sql)
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(limit)
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
    .context("Impossibile leggere le unita' di misura")
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
    .context("Impossibile cercare l'unita' di misura")
}

async fn unit_keyboard_from_db(pool: &SqlitePool) -> Result<InlineKeyboardMarkup> {
    let units = list_units(pool).await?;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for pair in units.chunks(2) {
        rows.push(
            pair.iter()
                .map(|unit| {
                    InlineKeyboardButton::callback(
                        format!("{} · {}", unit.simbolo, unit.nome),
                        format!("food:new:unit:{}", unit.id),
                    )
                })
                .collect(),
        );
    }
    rows.push(vec![
        button("⬅️ Indietro", "food:new:back:name"),
        button("❌ Annulla", "food:cancel"),
        button("🏠 Menu principale", "menu:main"),
    ]);
    Ok(InlineKeyboardMarkup::new(rows))
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
            button("⬅️ Indietro", "food:new:back:unit"),
            button("❌ Annulla", "food:cancel"),
            button("🏠 Menu principale", "menu:main"),
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
        button("🏠 Menu principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn food_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("➕ Nuovo alimento", "food:new")],
        vec![
            button("📋 Elenco alimenti", "food:list"),
            button("🔎 Cerca", "food:search"),
        ],
        vec![button("🔄 Aggiorna alimenti", "food:refresh")],
        vec![button("🍝 Ricette · prossimamente", "menu:soon")],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn food_results_keyboard(foods: &[FoodRecord]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = foods
        .iter()
        .map(|food| {
            vec![button(
                format!("#{} · {}", food.id, food.name),
                format!("food:view:{}", food.id),
            )]
        })
        .collect();
    rows.push(vec![button("🔄 Aggiorna alimenti", "food:refresh")]);
    rows.push(vec![
        button("⬅️ Indietro", "food:back"),
        button("🏠 Menu principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn food_created_keyboard(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("🥕 Apri alimento", format!("food:view:{id}"))],
        vec![button("➕ Altro alimento", "food:new")],
        vec![
            button("⬅️ Indietro", "food:back"),
            button("🏠 Menu principale", "menu:main"),
        ],
    ])
}

fn food_detail_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("🔄 Aggiorna alimenti", "food:refresh")],
        vec![button("📋 Elenco alimenti", "food:list")],
        vec![
            button("⬅️ Indietro", "food:list"),
            button("🏠 Menu principale", "menu:main"),
        ],
    ])
}

fn cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Indietro", "food:back"),
        button("❌ Annulla", "food:cancel"),
        button("🏠 Menu principale", "menu:main"),
    ]])
}

fn button(text: impl Into<String>, callback: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), callback.into())
}

fn food_summary_line(food: &FoodRecord, current_user: Option<i64>) -> String {
    let unit = food
        .unit_symbol
        .as_deref()
        .map(|symbol| format!(" · {symbol}"))
        .unwrap_or_default();

    format!(
        "#{} · {}{}\n{}",
        food.id,
        food.name,
        unit,
        ownership_label(food, current_user)
    )
}

fn ownership_label(food: &FoodRecord, current_user: Option<i64>) -> String {
    if food.global_catalog == 1 {
        return "🌐 Catalogo globale".to_string();
    }

    if food.owner_user_id == current_user && current_user.is_some() {
        return "👤 Proprietà: tua".to_string();
    }

    match food.owner_name.as_deref() {
        Some(name) => format!("👤 Proprietà: {name}"),
        None => "👤 Proprietà: non disponibile".to_string(),
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
    async fn alimento_personale_viene_creato_elencato_e_cercato() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        let unit_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE codice = 'g'")
            .fetch_one(&pool)
            .await
            .expect("unita g");

        let id = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Petto di pollo", Some(unit_id), &[])
                .await
                .expect("creazione alimento")
        })
        .await;

        let foods = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            list_foods(&pool, None, 20).await.expect("elenco alimenti")
        })
        .await;

        assert_eq!(foods.len(), 1);
        assert_eq!(foods[0].id, id);
        assert_eq!(foods[0].owner_user_id, Some(user_id));
        assert_eq!(foods[0].unit_code.as_deref(), Some("g"));

        let found = identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            list_foods(&pool, Some("POLLO"), 20)
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
            "SELECT COUNT(*) FROM alimenti WHERE nome_normalizzato = 'senza unita'",
        )
        .fetch_one(&pool)
        .await
        .expect("conteggio alimento senza unita");

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn duplicato_personale_viene_rifiutato() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Tester").await;
        add_membership(&pool, 1, user_id, "proprietario").await;

        identity::with_actor(actor(user_id, 1, false, "Tester"), async {
            create_food(&pool, "Riso", Some(1), &[])
                .await
                .expect("primo alimento");
            let duplicate = create_food(&pool, "  RISO ", Some(1), &[]).await;
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
            create_food(&pool, "Pasta", Some(1), &[1, space_2])
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
            create_food(&pool, "Avena", Some(1), &[space_2])
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
            list_foods(&pool, None, 20)
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
            create_food(&pool, "Cous cous", Some(1), &[shared_space])
                .await
                .expect("alimento condiviso")
        })
        .await;

        let visible = identity::with_actor(actor(guest_id, shared_space, true, "Guest"), async {
            list_foods(&pool, None, 20).await.expect("catalogo guest")
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
            list_foods(&pool, None, 20)
                .await
                .expect("catalogo guest dopo rimozione")
        })
        .await;
        assert!(!hidden.iter().any(|food| food.id == food_id));
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
