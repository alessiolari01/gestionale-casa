//! Case, stanze e posizione strutturata degli item.
//!
//! Step 6A introduce una gerarchia riutilizzabile da tutti i moduli:
//! abitazione -> stanza -> dettaglio libero specifico del modulo.
//! Gli oggetti generici usano `item_luogo` per casa/stanza e mantengono
//! `oggetti.posizione` come dettaglio libero (es. scaffale o cassetto).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

const MAX_LIST_RESULTS: i64 = 30;

#[derive(Clone, Default)]
pub struct LocationSessionStore {
    inner: Arc<Mutex<HashMap<i64, LocationConversationState>>>,
}

impl LocationSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.with_sessions(|sessions| {
            sessions.remove(&chat_id);
        });
    }

    fn get(&self, chat_id: i64) -> Option<LocationConversationState> {
        self.with_sessions(|sessions| sessions.get(&chat_id).cloned())
    }

    fn set(&self, chat_id: i64, state: LocationConversationState) {
        self.with_sessions(|sessions| {
            sessions.insert(chat_id, state);
        });
    }

    fn with_sessions<T>(
        &self,
        f: impl FnOnce(&mut HashMap<i64, LocationConversationState>) -> T,
    ) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

#[derive(Debug, Clone)]
enum LocationConversationState {
    AwaitingHomeName {
        rename_id: Option<i64>,
    },
    AwaitingRoomName {
        home_id: i64,
        rename_id: Option<i64>,
    },
}

#[derive(Debug, Clone, FromRow)]
struct HomeRecord {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, FromRow)]
struct RoomRecord {
    id: i64,
    home_id: i64,
    name: String,
    home_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HomeChoice {
    pub(crate) id: i64,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoomChoice {
    pub(crate) id: i64,
    pub(crate) home_id: i64,
    pub(crate) name: String,
    pub(crate) home_name: String,
}

#[derive(Debug, Clone, FromRow)]
struct ItemLocation {
    home_id: i64,
    room_id: Option<i64>,
    home_name: Option<String>,
    room_name: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct LocatedObjectSummary {
    id: i64,
    name: String,
    detail_position: Option<String>,
    home_name: Option<String>,
    room_name: Option<String>,
}

pub async fn show_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🏠 Case e stanze\n\nGestisci le abitazioni, le stanze e consulta gli oggetti per luogo.",
    )
    .reply_markup(locations_menu_keyboard())
    .await?;
    Ok(())
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &LocationSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if let Some((command, args)) = parse_command(text) {
        match command {
            "/luoghi" | "/case" => {
                sessions.clear_chat(chat_id);
                show_menu(bot, msg.chat.id).await?;
                return Ok(true);
            }
            "/casa_nuova" => {
                if args.is_empty() {
                    sessions.set(
                        chat_id,
                        LocationConversationState::AwaitingHomeName { rename_id: None },
                    );
                    ask_home_name(bot, msg.chat.id, false).await?;
                } else {
                    create_home_from_input(bot, msg.chat.id, pool, sessions, args).await?;
                }
                return Ok(true);
            }
            "/case_lista" => {
                sessions.clear_chat(chat_id);
                send_home_list(bot, msg.chat.id, pool).await?;
                return Ok(true);
            }
            "/casa" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_home_detail(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(msg.chat.id, "Uso: /casa <id>\nEsempio: /casa 1")
                        .await?;
                }
                return Ok(true);
            }
            "/casa_rinomina" => {
                if let Some((id, new_name)) = split_id_and_rest(args) {
                    if !home_exists(pool, id).await.unwrap_or(false) {
                        bot.send_message(msg.chat.id, format!("Casa #{id} non trovata."))
                            .reply_markup(locations_menu_keyboard())
                            .await?;
                    } else if new_name.is_empty() {
                        sessions.set(
                            chat_id,
                            LocationConversationState::AwaitingHomeName {
                                rename_id: Some(id),
                            },
                        );
                        ask_home_name(bot, msg.chat.id, true).await?;
                    } else {
                        rename_home_from_input(bot, msg.chat.id, pool, sessions, id, new_name)
                            .await?;
                    }
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /casa_rinomina <id> [nuovo nome]\nEsempio: /casa_rinomina 1 Casa principale",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/casa_elimina" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_home_delete_confirmation(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /casa_elimina <id>\nEsempio: /casa_elimina 1",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/stanza_nuova" => {
                if let Some((home_id, room_name)) = split_id_and_rest(args) {
                    if !home_exists(pool, home_id).await.unwrap_or(false) {
                        bot.send_message(msg.chat.id, format!("Casa #{home_id} non trovata."))
                            .reply_markup(locations_menu_keyboard())
                            .await?;
                    } else if room_name.is_empty() {
                        sessions.set(
                            chat_id,
                            LocationConversationState::AwaitingRoomName {
                                home_id,
                                rename_id: None,
                            },
                        );
                        ask_room_name(bot, msg.chat.id, false).await?;
                    } else {
                        create_room_from_input(
                            bot,
                            msg.chat.id,
                            pool,
                            sessions,
                            home_id,
                            room_name,
                        )
                        .await?;
                    }
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /stanza_nuova <casa_id> [nome]\nEsempio: /stanza_nuova 1 Garage",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/stanza" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_room_detail(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(msg.chat.id, "Uso: /stanza <id>\nEsempio: /stanza 3")
                        .await?;
                }
                return Ok(true);
            }
            "/stanza_rinomina" => {
                if let Some((id, new_name)) = split_id_and_rest(args) {
                    match get_room(pool, id).await {
                        Ok(Some(room)) if new_name.is_empty() => {
                            sessions.set(
                                chat_id,
                                LocationConversationState::AwaitingRoomName {
                                    home_id: room.home_id,
                                    rename_id: Some(id),
                                },
                            );
                            ask_room_name(bot, msg.chat.id, true).await?;
                        }
                        Ok(Some(room)) => {
                            rename_room_from_input(
                                bot,
                                msg.chat.id,
                                pool,
                                sessions,
                                &room,
                                new_name,
                            )
                            .await?;
                        }
                        Ok(None) => {
                            bot.send_message(msg.chat.id, format!("Stanza #{id} non trovata."))
                                .reply_markup(locations_menu_keyboard())
                                .await?;
                        }
                        Err(error) => {
                            tracing::error!(?error, room_id = id, "Errore lettura stanza");
                            bot.send_message(msg.chat.id, "⚠️ Non riesco a leggere la stanza.")
                                .await?;
                        }
                    }
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /stanza_rinomina <id> [nuovo nome]\nEsempio: /stanza_rinomina 3 Garage grande",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/stanza_elimina" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_room_delete_confirmation(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /stanza_elimina <id>\nEsempio: /stanza_elimina 3",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/oggetto_luogo" | "/oggetto_sposta" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    show_item_location_picker(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /oggetto_luogo <id>\nEsempio: /oggetto_luogo 12",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/annulla" => {
                if sessions.get(chat_id).is_some() {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "Operazione sui luoghi annullata.")
                        .reply_markup(locations_menu_keyboard())
                        .await?;
                    return Ok(true);
                }
                return Ok(false);
            }
            _ => return Ok(false),
        }
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        LocationConversationState::AwaitingHomeName { rename_id: None } => {
            create_home_from_input(bot, msg.chat.id, pool, sessions, text).await?;
        }
        LocationConversationState::AwaitingHomeName {
            rename_id: Some(id),
        } => {
            rename_home_from_input(bot, msg.chat.id, pool, sessions, id, text).await?;
        }
        LocationConversationState::AwaitingRoomName {
            home_id,
            rename_id: None,
        } => {
            create_room_from_input(bot, msg.chat.id, pool, sessions, home_id, text).await?;
        }
        LocationConversationState::AwaitingRoomName {
            rename_id: Some(id),
            ..
        } => match get_room(pool, id).await {
            Ok(Some(room)) => {
                rename_room_from_input(bot, msg.chat.id, pool, sessions, &room, text).await?;
            }
            Ok(None) => {
                sessions.clear_chat(chat_id);
                bot.send_message(msg.chat.id, format!("Stanza #{id} non trovata."))
                    .reply_markup(locations_menu_keyboard())
                    .await?;
            }
            Err(error) => {
                tracing::error!(?error, room_id = id, "Errore lettura stanza");
                bot.send_message(msg.chat.id, "⚠️ Non riesco a leggere la stanza.")
                    .await?;
            }
        },
    }

    Ok(true)
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &LocationSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    let raw_chat_id = chat_id.0;

    match data {
        "loc:menu" => {
            sessions.clear_chat(raw_chat_id);
            show_menu(bot, chat_id).await?;
        }
        "loc:home:new" => {
            sessions.set(
                raw_chat_id,
                LocationConversationState::AwaitingHomeName { rename_id: None },
            );
            ask_home_name(bot, chat_id, false).await?;
        }
        "loc:home:list" => {
            sessions.clear_chat(raw_chat_id);
            send_home_list(bot, chat_id, pool).await?;
        }
        _ if data.starts_with("loc:home:rename:") => {
            if let Some(id) = parse_id_callback(data, "loc:home:rename:") {
                if home_exists(pool, id).await.unwrap_or(false) {
                    sessions.set(
                        raw_chat_id,
                        LocationConversationState::AwaitingHomeName {
                            rename_id: Some(id),
                        },
                    );
                    ask_home_name(bot, chat_id, true).await?;
                } else {
                    bot.send_message(chat_id, format!("Casa #{id} non trovata."))
                        .reply_markup(locations_menu_keyboard())
                        .await?;
                }
            }
        }
        _ if data.starts_with("loc:home:delete:ask:") => {
            if let Some(id) = parse_id_callback(data, "loc:home:delete:ask:") {
                send_home_delete_confirmation(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:home:delete:do:") => {
            if let Some(id) = parse_id_callback(data, "loc:home:delete:do:") {
                delete_home_and_report(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:room:new:") => {
            if let Some(home_id) = parse_id_callback(data, "loc:room:new:") {
                if home_exists(pool, home_id).await.unwrap_or(false) {
                    sessions.set(
                        raw_chat_id,
                        LocationConversationState::AwaitingRoomName {
                            home_id,
                            rename_id: None,
                        },
                    );
                    ask_room_name(bot, chat_id, false).await?;
                } else {
                    bot.send_message(chat_id, format!("Casa #{home_id} non trovata."))
                        .reply_markup(locations_menu_keyboard())
                        .await?;
                }
            }
        }
        _ if data.starts_with("loc:room:rename:") => {
            if let Some(id) = parse_id_callback(data, "loc:room:rename:") {
                match get_room(pool, id).await {
                    Ok(Some(room)) => {
                        sessions.set(
                            raw_chat_id,
                            LocationConversationState::AwaitingRoomName {
                                home_id: room.home_id,
                                rename_id: Some(id),
                            },
                        );
                        ask_room_name(bot, chat_id, true).await?;
                    }
                    Ok(None) => {
                        bot.send_message(chat_id, format!("Stanza #{id} non trovata."))
                            .reply_markup(locations_menu_keyboard())
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, room_id = id, "Errore lettura stanza");
                        bot.send_message(chat_id, "⚠️ Non riesco a leggere la stanza.")
                            .await?;
                    }
                }
            }
        }
        _ if data.starts_with("loc:room:delete:ask:") => {
            if let Some(id) = parse_id_callback(data, "loc:room:delete:ask:") {
                send_room_delete_confirmation(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:room:delete:do:") => {
            if let Some(id) = parse_id_callback(data, "loc:room:delete:do:") {
                delete_room_and_report(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:filter:home:") => {
            if let Some(id) = parse_id_callback(data, "loc:filter:home:") {
                send_objects_for_home(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:filter:room:") => {
            if let Some(id) = parse_id_callback(data, "loc:filter:room:") {
                send_objects_for_room(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:item:sethome:") => {
            if let Some((item_id, home_id)) = parse_two_ids_callback(data, "loc:item:sethome:") {
                let previous = get_item_location(pool, item_id).await.unwrap_or(None);
                match set_item_home(pool, item_id, home_id).await {
                    Ok(()) => {
                        let current = get_item_location(pool, item_id).await.unwrap_or(None);
                        bot.send_message(
                            chat_id,
                            location_change_message(previous.as_ref(), current.as_ref()),
                        )
                        .await?;
                        crate::modules::oggetti::send_object_detail(bot, chat_id, pool, item_id)
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, item_id, home_id, "Errore assegnazione casa");
                        bot.send_message(chat_id, "⚠️ Non riesco ad assegnare questa casa.")
                            .await?;
                    }
                }
            }
        }
        _ if data.starts_with("loc:item:setroom:") => {
            if let Some((item_id, room_id)) = parse_two_ids_callback(data, "loc:item:setroom:") {
                let previous = get_item_location(pool, item_id).await.unwrap_or(None);
                match set_item_room(pool, item_id, room_id).await {
                    Ok(()) => {
                        let current = get_item_location(pool, item_id).await.unwrap_or(None);
                        bot.send_message(
                            chat_id,
                            location_change_message(previous.as_ref(), current.as_ref()),
                        )
                        .await?;
                        crate::modules::oggetti::send_object_detail(bot, chat_id, pool, item_id)
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, item_id, room_id, "Errore assegnazione stanza");
                        bot.send_message(chat_id, "⚠️ Non riesco ad assegnare questa stanza.")
                            .await?;
                    }
                }
            }
        }
        _ if data.starts_with("loc:item:clear:") => {
            if let Some(item_id) = parse_id_callback(data, "loc:item:clear:") {
                let previous = get_item_location(pool, item_id).await.unwrap_or(None);
                match clear_item_location(pool, item_id).await {
                    Ok(()) => {
                        let current = get_item_location(pool, item_id).await.unwrap_or(None);
                        bot.send_message(
                            chat_id,
                            location_change_message(previous.as_ref(), current.as_ref()),
                        )
                        .await?;
                        crate::modules::oggetti::send_object_detail(bot, chat_id, pool, item_id)
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, item_id, "Errore rimozione luogo");
                        bot.send_message(chat_id, "⚠️ Non riesco a rimuovere il luogo.")
                            .await?;
                    }
                }
            }
        }
        _ if data.starts_with("loc:item:home:") => {
            if let Some((item_id, home_id)) = parse_two_ids_callback(data, "loc:item:home:") {
                show_room_picker(bot, chat_id, pool, item_id, home_id).await?;
            }
        }
        _ if data.starts_with("loc:item:") => {
            if let Some(item_id) = parse_id_callback(data, "loc:item:") {
                show_item_location_picker(bot, chat_id, pool, item_id).await?;
            }
        }
        _ if data.starts_with("loc:room:") => {
            if let Some(id) = parse_id_callback(data, "loc:room:") {
                send_room_detail(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:home:") => {
            if let Some(id) = parse_id_callback(data, "loc:home:") {
                send_home_detail(bot, chat_id, pool, id).await?;
            }
        }
        _ => return Ok(false),
    }

    Ok(true)
}

async fn ask_home_name(bot: &Bot, chat_id: ChatId, rename: bool) -> ResponseResult<()> {
    let text = if rename {
        "✏️ Scrivi il nuovo nome della casa.\n\n/annulla per uscire."
    } else {
        "➕ Nuova casa\n\nScrivi il nome dell'abitazione.\nEsempio: Casa principale\n\n/annulla per uscire."
    };
    bot.send_message(chat_id, text).await?;
    Ok(())
}

async fn ask_room_name(bot: &Bot, chat_id: ChatId, rename: bool) -> ResponseResult<()> {
    let text = if rename {
        "✏️ Scrivi il nuovo nome della stanza.\n\n/annulla per uscire."
    } else {
        "➕ Nuova stanza\n\nScrivi il nome della stanza.\nEsempio: Garage\n\n/annulla per uscire."
    };
    bot.send_message(chat_id, text).await?;
    Ok(())
}

async fn create_home_from_input(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &LocationSessionStore,
    input: &str,
) -> ResponseResult<()> {
    let Some(name) = clean_name(input) else {
        bot.send_message(
            chat_id,
            "Il nome della casa non può essere vuoto e deve restare entro 120 caratteri. Riprova oppure usa /annulla.",
        )
        .await?;
        return Ok(());
    };

    match create_home(pool, &name).await {
        Ok(id) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("✅ Casa creata: {name}"))
                .await?;
            send_home_detail(bot, chat_id, pool, id).await?;
        }
        Err(error) => {
            tracing::warn!(?error, home_name = %name, "Creazione casa fallita");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a creare la casa. Controlla che non esista già una casa con lo stesso nome.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn rename_home_from_input(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &LocationSessionStore,
    id: i64,
    input: &str,
) -> ResponseResult<()> {
    let Some(name) = clean_name(input) else {
        bot.send_message(
            chat_id,
            "Il nome della casa non può essere vuoto e deve restare entro 120 caratteri. Riprova oppure usa /annulla.",
        )
        .await?;
        return Ok(());
    };

    match rename_home(pool, id, &name).await {
        Ok(true) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("✅ Casa rinominata: {name}"))
                .await?;
            send_home_detail(bot, chat_id, pool, id).await?;
        }
        Ok(false) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("Casa #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::warn!(?error, home_id = id, "Rinomina casa fallita");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a rinominare la casa. Controlla che il nome non sia già usato.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn create_room_from_input(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &LocationSessionStore,
    home_id: i64,
    input: &str,
) -> ResponseResult<()> {
    let Some(name) = clean_name(input) else {
        bot.send_message(
            chat_id,
            "Il nome della stanza non può essere vuoto e deve restare entro 120 caratteri. Riprova oppure usa /annulla.",
        )
        .await?;
        return Ok(());
    };

    match create_room(pool, home_id, &name).await {
        Ok(id) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("✅ Stanza creata: {name}"))
                .await?;
            send_room_detail(bot, chat_id, pool, id).await?;
        }
        Err(error) => {
            tracing::warn!(?error, home_id, room_name = %name, "Creazione stanza fallita");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a creare la stanza. Controlla che la casa esista e che non ci sia già una stanza con lo stesso nome in quella casa.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn rename_room_from_input(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &LocationSessionStore,
    room: &RoomRecord,
    input: &str,
) -> ResponseResult<()> {
    let Some(name) = clean_name(input) else {
        bot.send_message(
            chat_id,
            "Il nome della stanza non può essere vuoto e deve restare entro 120 caratteri. Riprova oppure usa /annulla.",
        )
        .await?;
        return Ok(());
    };

    match rename_room(pool, room.id, &name).await {
        Ok(true) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("✅ Stanza rinominata: {name}"))
                .await?;
            send_room_detail(bot, chat_id, pool, room.id).await?;
        }
        Ok(false) => {
            sessions.clear_chat(chat_id.0);
            bot.send_message(chat_id, format!("Stanza #{} non trovata.", room.id))
                .reply_markup(home_detail_keyboard(room.home_id, &[]))
                .await?;
        }
        Err(error) => {
            tracing::warn!(?error, room_id = room.id, "Rinomina stanza fallita");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a rinominare la stanza. Controlla che il nome non sia già usato nella stessa casa.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn send_home_list(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    match list_homes(pool).await {
        Ok(homes) if homes.is_empty() => {
            bot.send_message(
                chat_id,
                "🏠 Non ci sono ancora case registrate.\n\nUsa ➕ Nuova casa oppure /casa_nuova.",
            )
            .reply_markup(locations_menu_keyboard())
            .await?;
        }
        Ok(homes) => {
            let mut text = "🏠 Case registrate\n\n".to_string();
            for home in &homes {
                text.push_str(&format!("#{} · {}\n", home.id, home.name));
            }
            bot.send_message(chat_id, text)
                .reply_markup(home_list_keyboard(&homes))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco case");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere l'elenco delle case.")
                .await?;
        }
    }
    Ok(())
}

async fn send_home_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_home(pool, id).await {
        Ok(Some(home)) => {
            let rooms = list_rooms_for_home(pool, id).await.unwrap_or_default();
            let item_count = count_items_for_home(pool, id).await.unwrap_or(0);
            let mut text = format!(
                "🏠 {}\n#{}\n\n🚪 Stanze: {}\n📦 Elementi assegnati: {}",
                home.name,
                home.id,
                rooms.len(),
                item_count
            );
            if !rooms.is_empty() {
                text.push_str("\n\nStanze:\n");
                for room in &rooms {
                    text.push_str(&format!("• {}\n", room.name));
                }
            }
            bot.send_message(chat_id, text)
                .reply_markup(home_detail_keyboard(home.id, &rooms))
                .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, format!("Casa #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, home_id = id, "Errore lettura casa");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questa casa.")
                .await?;
        }
    }
    Ok(())
}

async fn send_room_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_room(pool, id).await {
        Ok(Some(room)) => {
            let item_count = count_items_for_room(pool, id).await.unwrap_or(0);
            bot.send_message(
                chat_id,
                format!(
                    "🚪 {}\n#{}\n\n🏠 Casa: {}\n📦 Elementi assegnati: {}",
                    room.name, room.id, room.home_name, item_count
                ),
            )
            .reply_markup(room_detail_keyboard(&room))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, format!("Stanza #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, room_id = id, "Errore lettura stanza");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questa stanza.")
                .await?;
        }
    }
    Ok(())
}

async fn send_home_delete_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_home(pool, id).await {
        Ok(Some(home)) => {
            let room_count = count_rooms_for_home(pool, id).await.unwrap_or(0);
            let item_count = count_items_for_home(pool, id).await.unwrap_or(0);
            bot.send_message(
                chat_id,
                format!(
                    "⚠️ Eliminare la casa?\n\n🏠 {}\n#{}\n\nVerranno eliminate anche {} stanze. I {} elementi collegati NON verranno eliminati: resteranno nel gestionale senza luogo strutturato.\n\nL'operazione sulla casa non può essere annullata.",
                    home.name, home.id, room_count, item_count
                ),
            )
            .reply_markup(home_delete_keyboard(id))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, format!("Casa #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, home_id = id, "Errore conferma eliminazione casa");
            bot.send_message(chat_id, "⚠️ Non riesco a preparare l'eliminazione.")
                .await?;
        }
    }
    Ok(())
}

async fn delete_home_and_report(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match delete_home(pool, id).await {
        Ok(true) => {
            bot.send_message(
                chat_id,
                format!("🗑 Casa #{id} eliminata. Gli oggetti sono rimasti nel gestionale."),
            )
            .reply_markup(locations_menu_keyboard())
            .await?;
        }
        Ok(false) => {
            bot.send_message(chat_id, format!("Casa #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, home_id = id, "Errore eliminazione casa");
            bot.send_message(chat_id, "⚠️ Non sono riuscito a eliminare la casa.")
                .await?;
        }
    }
    Ok(())
}

async fn send_room_delete_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_room(pool, id).await {
        Ok(Some(room)) => {
            let item_count = count_items_for_room(pool, id).await.unwrap_or(0);
            bot.send_message(
                chat_id,
                format!(
                    "⚠️ Eliminare la stanza?\n\n🚪 {}\n🏠 {}\n#{}\n\nI {} elementi collegati NON verranno eliminati: resteranno associati alla casa, ma senza stanza.\n\nL'operazione sulla stanza non può essere annullata.",
                    room.name, room.home_name, room.id, item_count
                ),
            )
            .reply_markup(room_delete_keyboard(&room))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, format!("Stanza #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, room_id = id, "Errore conferma eliminazione stanza");
            bot.send_message(chat_id, "⚠️ Non riesco a preparare l'eliminazione.")
                .await?;
        }
    }
    Ok(())
}

async fn delete_room_and_report(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    let room = match get_room(pool, id).await {
        Ok(Some(room)) => room,
        Ok(None) => {
            bot.send_message(chat_id, format!("Stanza #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, room_id = id, "Errore lettura stanza");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questa stanza.")
                .await?;
            return Ok(());
        }
    };

    match delete_room(pool, id).await {
        Ok(true) => {
            bot.send_message(
                chat_id,
                format!("🗑 Stanza #{id} eliminata. Gli oggetti restano associati alla casa."),
            )
            .reply_markup(home_detail_keyboard(room.home_id, &[]))
            .await?;
            send_home_detail(bot, chat_id, pool, room.home_id).await?;
        }
        Ok(false) => {
            bot.send_message(chat_id, format!("Stanza #{id} non trovata."))
                .reply_markup(locations_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, room_id = id, "Errore eliminazione stanza");
            bot.send_message(chat_id, "⚠️ Non sono riuscito a eliminare la stanza.")
                .await?;
        }
    }
    Ok(())
}

async fn show_item_location_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    item_id: i64,
) -> ResponseResult<()> {
    if !object_exists(pool, item_id).await.unwrap_or(false) {
        bot.send_message(chat_id, format!("Oggetto #{item_id} non trovato."))
            .await?;
        return Ok(());
    }

    let homes = match list_homes(pool).await {
        Ok(homes) => homes,
        Err(error) => {
            tracing::error!(?error, "Errore elenco case per assegnazione");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le case.")
                .await?;
            return Ok(());
        }
    };
    let location = get_item_location(pool, item_id).await.unwrap_or(None);

    let current = format_location(location.as_ref());
    let is_move = has_structured_location(location.as_ref());
    let text = if homes.is_empty() {
        if is_move {
            format!(
                "🚚 Sposta oggetto #{item_id}\n\nPosizione attuale: {current}\n\nNon ci sono altre case disponibili. Creane una dalla sezione 🏠 Case e stanze."
            )
        } else {
            format!(
                "🏠 Assegna luogo all'oggetto #{item_id}\n\nPosizione attuale: {current}\n\nNon ci sono ancora case. Creane una dalla sezione 🏠 Case e stanze."
            )
        }
    } else if is_move {
        format!(
            "🚚 Sposta oggetto #{item_id}\n\nPosizione attuale: {current}\n\nScegli la nuova casa. Nel passaggio successivo potrai scegliere anche una stanza."
        )
    } else {
        format!(
            "🏠 Assegna luogo all'oggetto #{item_id}\n\nPosizione attuale: {current}\n\nScegli la casa. Nel passaggio successivo potrai scegliere anche una stanza."
        )
    };

    bot.send_message(chat_id, text)
        .reply_markup(item_home_picker_keyboard(item_id, &homes))
        .await?;
    Ok(())
}

async fn show_room_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    item_id: i64,
    home_id: i64,
) -> ResponseResult<()> {
    let Some(home) = get_home(pool, home_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Casa #{home_id} non trovata."))
            .reply_markup(locations_menu_keyboard())
            .await?;
        return Ok(());
    };
    let rooms = list_rooms_for_home(pool, home_id).await.unwrap_or_default();
    let current_location = get_item_location(pool, item_id).await.unwrap_or(None);
    let is_move = has_structured_location(current_location.as_ref());
    let current = format_location(current_location.as_ref());
    let action = if is_move {
        "🚚 Sposta oggetto"
    } else {
        "🏠 Assegna luogo"
    };

    bot.send_message(
        chat_id,
        format!(
            "{action} #{item_id}\n\nPosizione attuale: {current}\nDestinazione scelta: 🏠 {}\n\nScegli una stanza oppure la sola casa.",
            home.name
        ),
    )
    .reply_markup(item_room_picker_keyboard(
        item_id,
        home_id,
        &rooms,
        current_location.as_ref(),
        &home.name,
    ))
    .await?;
    Ok(())
}

async fn send_objects_for_home(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    home_id: i64,
) -> ResponseResult<()> {
    let Some(home) = get_home(pool, home_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Casa #{home_id} non trovata."))
            .reply_markup(locations_menu_keyboard())
            .await?;
        return Ok(());
    };
    let total = count_objects_for_home(pool, home_id).await.unwrap_or(0);
    let objects = list_objects_for_home(pool, home_id, MAX_LIST_RESULTS)
        .await
        .unwrap_or_default();
    send_filtered_objects(
        bot,
        chat_id,
        format!("🏠 {}", home.name),
        total,
        &objects,
        format!("loc:home:{home_id}"),
    )
    .await
}

async fn send_objects_for_room(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    room_id: i64,
) -> ResponseResult<()> {
    let Some(room) = get_room(pool, room_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Stanza #{room_id} non trovata."))
            .reply_markup(locations_menu_keyboard())
            .await?;
        return Ok(());
    };
    let total = count_objects_for_room(pool, room_id).await.unwrap_or(0);
    let objects = list_objects_for_room(pool, room_id, MAX_LIST_RESULTS)
        .await
        .unwrap_or_default();
    send_filtered_objects(
        bot,
        chat_id,
        format!("🏠 {} / 🚪 {}", room.home_name, room.name),
        total,
        &objects,
        format!("loc:room:{room_id}"),
    )
    .await
}

async fn send_filtered_objects(
    bot: &Bot,
    chat_id: ChatId,
    title: String,
    total: i64,
    objects: &[LocatedObjectSummary],
    back_callback: String,
) -> ResponseResult<()> {
    if objects.is_empty() {
        bot.send_message(chat_id, format!("📦 Nessun oggetto in:\n{title}"))
            .reply_markup(InlineKeyboardMarkup::new(vec![vec![button(
                "↩️ Indietro",
                &back_callback,
            )]]))
            .await?;
        return Ok(());
    }

    let mut text = format!("📦 Oggetti in:\n{title}\n\n");
    for object in objects {
        text.push_str(&format!("#{} · {}", object.id, object.name));
        if let Some(location) = summary_location(object) {
            text.push_str(&format!("\n{location}"));
        }
        text.push_str("\n\n");
    }
    if total > objects.len() as i64 {
        text.push_str(&format!(
            "Mostrati i primi {} di {} elementi.",
            objects.len(),
            total
        ));
    }

    bot.send_message(chat_id, text)
        .reply_markup(filtered_objects_keyboard(objects, &back_callback))
        .await?;
    Ok(())
}

fn format_location(location: Option<&ItemLocation>) -> String {
    match location {
        Some(location) => match (&location.home_name, &location.room_name) {
            (Some(home), Some(room)) => format!("🏠 {home} / 🚪 {room}"),
            (Some(home), None) => format!("🏠 {home}"),
            _ => "Nessun luogo strutturato".to_string(),
        },
        None => "Nessun luogo strutturato".to_string(),
    }
}

fn has_structured_location(location: Option<&ItemLocation>) -> bool {
    location
        .and_then(|location| location.home_name.as_ref())
        .is_some()
}

fn location_change_message(
    previous: Option<&ItemLocation>,
    current: Option<&ItemLocation>,
) -> String {
    let before = format_location(previous);
    let after = format_location(current);
    let had_location = has_structured_location(previous);
    let has_location = has_structured_location(current);

    match (had_location, has_location, before == after) {
        (false, false, _) => {
            "ℹ️ L'oggetto non ha un luogo strutturato. Nessuna modifica effettuata.".to_string()
        }
        (false, true, _) => format!("✅ Luogo assegnato all'oggetto.\n\nNuovo luogo: {after}"),
        (true, false, _) => format!("🧹 Luogo rimosso dall'oggetto.\n\nPrima: {before}"),
        (true, true, true) => {
            format!("ℹ️ L'oggetto è già in:\n{after}\n\nNessuno spostamento effettuato.")
        }
        (true, true, false) => {
            format!("🚚 Oggetto spostato.\n\nDa: {before}\nA: {after}")
        }
    }
}

fn summary_location(object: &LocatedObjectSummary) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(home) = &object.home_name {
        if let Some(room) = &object.room_name {
            parts.push(format!("🏠 {home} / {room}"));
        } else {
            parts.push(format!("🏠 {home}"));
        }
    }
    if let Some(detail) = &object.detail_position {
        parts.push(format!("📌 {detail}"));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

pub(crate) async fn home_choices(pool: &SqlitePool) -> Result<Vec<HomeChoice>, sqlx::Error> {
    Ok(list_homes(pool)
        .await?
        .into_iter()
        .map(|home| HomeChoice {
            id: home.id,
            name: home.name,
        })
        .collect())
}

pub(crate) async fn home_choice(
    pool: &SqlitePool,
    home_id: i64,
) -> Result<Option<HomeChoice>, sqlx::Error> {
    Ok(get_home(pool, home_id).await?.map(|home| HomeChoice {
        id: home.id,
        name: home.name,
    }))
}

pub(crate) async fn room_choices(
    pool: &SqlitePool,
    home_id: i64,
) -> Result<Vec<RoomChoice>, sqlx::Error> {
    Ok(list_rooms_for_home(pool, home_id)
        .await?
        .into_iter()
        .map(|room| RoomChoice {
            id: room.id,
            home_id: room.home_id,
            name: room.name,
            home_name: room.home_name,
        })
        .collect())
}

pub(crate) async fn room_choice(
    pool: &SqlitePool,
    room_id: i64,
) -> Result<Option<RoomChoice>, sqlx::Error> {
    Ok(get_room(pool, room_id).await?.map(|room| RoomChoice {
        id: room.id,
        home_id: room.home_id,
        name: room.name,
        home_name: room.home_name,
    }))
}

pub(crate) async fn insert_item_location(
    tx: &mut Transaction<'_, Sqlite>,
    item_id: i64,
    home_id: i64,
    room_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO item_luogo (item_id, abitazione_id, stanza_id) VALUES (?, ?, ?)")
        .bind(item_id)
        .bind(home_id)
        .bind(room_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn history_location_snapshot(
    tx: &mut Transaction<'_, Sqlite>,
    home_id: i64,
    room_id: Option<i64>,
) -> Result<crate::modules::storico::LocationSnapshot, sqlx::Error> {
    let home_name: String = sqlx::query_scalar("SELECT nome FROM abitazioni WHERE id = ?")
        .bind(home_id)
        .fetch_one(&mut **tx)
        .await?;
    let home_history_id =
        crate::modules::storico::ensure_entity(tx, "abitazione", home_id, &home_name).await?;

    let (room_history_id, room_name) = if let Some(room_id) = room_id {
        let room_name: String =
            sqlx::query_scalar("SELECT nome FROM stanze WHERE id = ? AND abitazione_id = ?")
                .bind(room_id)
                .bind(home_id)
                .fetch_one(&mut **tx)
                .await?;
        let history_id =
            crate::modules::storico::ensure_entity(tx, "stanza", room_id, &room_name).await?;
        (Some(history_id), Some(room_name))
    } else {
        (None, None)
    };

    Ok(crate::modules::storico::LocationSnapshot {
        abitazione_storico_id: Some(home_history_id),
        abitazione_nome: Some(home_name),
        stanza_storico_id: room_history_id,
        stanza_nome: room_name,
    })
}

pub(crate) async fn history_item_location_snapshot(
    tx: &mut Transaction<'_, Sqlite>,
    item_id: i64,
) -> Result<crate::modules::storico::LocationSnapshot, sqlx::Error> {
    match get_item_location_tx(tx, item_id).await? {
        Some(location) => history_snapshot_from_item_location(tx, &location).await,
        None => Ok(crate::modules::storico::LocationSnapshot::default()),
    }
}

async fn history_snapshot_from_item_location(
    tx: &mut Transaction<'_, Sqlite>,
    location: &ItemLocation,
) -> Result<crate::modules::storico::LocationSnapshot, sqlx::Error> {
    history_location_snapshot(tx, location.home_id, location.room_id).await
}

async fn history_item_identity(
    tx: &mut Transaction<'_, Sqlite>,
    item_id: i64,
) -> Result<(i64, String), sqlx::Error> {
    let name: String = sqlx::query_scalar("SELECT nome FROM items WHERE id = ?")
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;
    let history_id = crate::modules::storico::ensure_entity(tx, "oggetto", item_id, &name).await?;
    Ok((history_id, name))
}

async fn record_item_location_event(
    tx: &mut Transaction<'_, Sqlite>,
    item_id: i64,
    operation: &str,
    before: &crate::modules::storico::LocationSnapshot,
    after: &crate::modules::storico::LocationSnapshot,
    parent_event_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    if before == after {
        return Ok(());
    }

    let (item_history_id, item_name) = history_item_identity(tx, item_id).await?;
    let context = if after.abitazione_storico_id.is_some() {
        after
    } else {
        before
    };

    let event_id = crate::modules::storico::record_event(
        tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: item_history_id,
            modulo: "oggetti",
            componente: "luoghi",
            operazione: operation,
            nome_entita_snapshot: &item_name,
            abitazione_storico_id: context.abitazione_storico_id,
            abitazione_nome_snapshot: context.abitazione_nome.as_deref(),
            stanza_storico_id: context.stanza_storico_id,
            stanza_nome_snapshot: context.stanza_nome.as_deref(),
            evento_padre_id: parent_event_id,
        },
    )
    .await?;

    crate::modules::storico::record_location_change(tx, event_id, before, after).await
}

async fn create_home(pool: &SqlitePool, name: &str) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query("INSERT INTO abitazioni (nome) VALUES (?)")
        .bind(name)
        .execute(&mut *tx)
        .await?;
    let id = result.last_insert_rowid();

    let history_id =
        crate::modules::storico::ensure_entity(&mut tx, "abitazione", id, name).await?;
    let event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: history_id,
            modulo: "luoghi",
            componente: "abitazioni",
            operazione: "creazione",
            nome_entita_snapshot: name,
            abitazione_storico_id: Some(history_id),
            abitazione_nome_snapshot: Some(name),
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_field_changes(
        &mut tx,
        event_id,
        &[crate::modules::storico::NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(name.to_string()),
        }],
    )
    .await?;

    tx.commit().await?;
    Ok(id)
}

async fn rename_home(pool: &SqlitePool, id: i64, name: &str) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(old_name) =
        sqlx::query_scalar::<_, String>("SELECT nome FROM abitazioni WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
    else {
        return Ok(false);
    };

    if old_name == name {
        return Ok(true);
    }

    let history_id =
        crate::modules::storico::ensure_entity(&mut tx, "abitazione", id, &old_name).await?;
    let result = sqlx::query(
        "UPDATE abitazioni SET nome = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(name)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() != 1 {
        return Ok(false);
    }

    let event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: history_id,
            modulo: "luoghi",
            componente: "abitazioni",
            operazione: "rinomina",
            nome_entita_snapshot: name,
            abitazione_storico_id: Some(history_id),
            abitazione_nome_snapshot: Some(name),
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_field_changes(
        &mut tx,
        event_id,
        &[crate::modules::storico::NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: Some(old_name),
            valore_dopo: Some(name.to_string()),
        }],
    )
    .await?;
    crate::modules::storico::rename_entity(&mut tx, history_id, name).await?;

    tx.commit().await?;
    Ok(true)
}

async fn delete_home(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(home_name) =
        sqlx::query_scalar::<_, String>("SELECT nome FROM abitazioni WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
    else {
        return Ok(false);
    };

    let home_history_id =
        crate::modules::storico::ensure_entity(&mut tx, "abitazione", id, &home_name).await?;
    let home_event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: home_history_id,
            modulo: "luoghi",
            componente: "abitazioni",
            operazione: "eliminazione",
            nome_entita_snapshot: &home_name,
            abitazione_storico_id: Some(home_history_id),
            abitazione_nome_snapshot: Some(&home_name),
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_field_changes(
        &mut tx,
        home_event_id,
        &[crate::modules::storico::NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: Some(home_name.clone()),
            valore_dopo: None,
        }],
    )
    .await?;

    let rooms: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, nome FROM stanze WHERE abitazione_id = ? ORDER BY id")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;

    let affected_items: Vec<(i64, Option<i64>)> =
        sqlx::query_as("SELECT item_id, stanza_id FROM item_luogo WHERE abitazione_id = ?")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;

    for (item_id, room_id) in affected_items {
        let before = history_location_snapshot(&mut tx, id, room_id).await?;
        record_item_location_event(
            &mut tx,
            item_id,
            "rimozione",
            &before,
            &crate::modules::storico::LocationSnapshot::default(),
            Some(home_event_id),
        )
        .await?;
    }

    for (room_id, room_name) in rooms {
        let room_history_id =
            crate::modules::storico::ensure_entity(&mut tx, "stanza", room_id, &room_name).await?;
        let room_event_id = crate::modules::storico::record_event(
            &mut tx,
            &crate::modules::storico::NewHistoryEvent {
                entita_storico_id: room_history_id,
                modulo: "luoghi",
                componente: "stanze",
                operazione: "eliminazione",
                nome_entita_snapshot: &room_name,
                abitazione_storico_id: Some(home_history_id),
                abitazione_nome_snapshot: Some(&home_name),
                stanza_storico_id: Some(room_history_id),
                stanza_nome_snapshot: Some(&room_name),
                evento_padre_id: Some(home_event_id),
            },
        )
        .await?;
        crate::modules::storico::record_field_changes(
            &mut tx,
            room_event_id,
            &[crate::modules::storico::NewFieldChange {
                campo: "nome",
                tipo_valore: "testo",
                valore_prima: Some(room_name),
                valore_dopo: None,
            }],
        )
        .await?;
        crate::modules::storico::mark_entity_deleted(&mut tx, room_history_id).await?;
    }

    crate::modules::storico::mark_entity_deleted(&mut tx, home_history_id).await?;

    let result = sqlx::query("DELETE FROM abitazioni WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

async fn get_home(pool: &SqlitePool, id: i64) -> Result<Option<HomeRecord>, sqlx::Error> {
    sqlx::query_as::<_, HomeRecord>("SELECT id, nome AS name FROM abitazioni WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

async fn home_exists(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM abitazioni WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(count == 1)
}

async fn list_homes(pool: &SqlitePool) -> Result<Vec<HomeRecord>, sqlx::Error> {
    sqlx::query_as::<_, HomeRecord>(
        "SELECT id, nome AS name FROM abitazioni ORDER BY nome COLLATE NOCASE, id",
    )
    .fetch_all(pool)
    .await
}

async fn create_room(pool: &SqlitePool, home_id: i64, name: &str) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let home_name: String = sqlx::query_scalar("SELECT nome FROM abitazioni WHERE id = ?")
        .bind(home_id)
        .fetch_one(&mut *tx)
        .await?;
    let home_history_id =
        crate::modules::storico::ensure_entity(&mut tx, "abitazione", home_id, &home_name).await?;

    let result = sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, ?)")
        .bind(home_id)
        .bind(name)
        .execute(&mut *tx)
        .await?;
    let id = result.last_insert_rowid();
    let room_history_id =
        crate::modules::storico::ensure_entity(&mut tx, "stanza", id, name).await?;

    let event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: room_history_id,
            modulo: "luoghi",
            componente: "stanze",
            operazione: "creazione",
            nome_entita_snapshot: name,
            abitazione_storico_id: Some(home_history_id),
            abitazione_nome_snapshot: Some(&home_name),
            stanza_storico_id: Some(room_history_id),
            stanza_nome_snapshot: Some(name),
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_field_changes(
        &mut tx,
        event_id,
        &[crate::modules::storico::NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(name.to_string()),
        }],
    )
    .await?;

    tx.commit().await?;
    Ok(id)
}

async fn rename_room(pool: &SqlitePool, id: i64, name: &str) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some((home_id, old_name, home_name)) = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT s.abitazione_id, s.nome, a.nome \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };

    if old_name == name {
        return Ok(true);
    }

    let home_history_id =
        crate::modules::storico::ensure_entity(&mut tx, "abitazione", home_id, &home_name).await?;
    let room_history_id =
        crate::modules::storico::ensure_entity(&mut tx, "stanza", id, &old_name).await?;

    let result = sqlx::query(
        "UPDATE stanze SET nome = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(name)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(false);
    }

    let event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: room_history_id,
            modulo: "luoghi",
            componente: "stanze",
            operazione: "rinomina",
            nome_entita_snapshot: name,
            abitazione_storico_id: Some(home_history_id),
            abitazione_nome_snapshot: Some(&home_name),
            stanza_storico_id: Some(room_history_id),
            stanza_nome_snapshot: Some(name),
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_field_changes(
        &mut tx,
        event_id,
        &[crate::modules::storico::NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: Some(old_name),
            valore_dopo: Some(name.to_string()),
        }],
    )
    .await?;
    crate::modules::storico::rename_entity(&mut tx, room_history_id, name).await?;

    tx.commit().await?;
    Ok(true)
}

async fn delete_room(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some((home_id, room_name, home_name)) = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT s.abitazione_id, s.nome, a.nome \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };

    let before = history_location_snapshot(&mut tx, home_id, Some(id)).await?;
    let after = history_location_snapshot(&mut tx, home_id, None).await?;
    let home_history_id = after
        .abitazione_storico_id
        .expect("la casa esiste durante l'eliminazione della stanza");
    let room_history_id = before
        .stanza_storico_id
        .expect("la stanza esiste durante la propria eliminazione");

    let room_event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: room_history_id,
            modulo: "luoghi",
            componente: "stanze",
            operazione: "eliminazione",
            nome_entita_snapshot: &room_name,
            abitazione_storico_id: Some(home_history_id),
            abitazione_nome_snapshot: Some(&home_name),
            stanza_storico_id: Some(room_history_id),
            stanza_nome_snapshot: Some(&room_name),
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_field_changes(
        &mut tx,
        room_event_id,
        &[crate::modules::storico::NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: Some(room_name),
            valore_dopo: None,
        }],
    )
    .await?;

    let affected_items: Vec<i64> =
        sqlx::query_scalar("SELECT item_id FROM item_luogo WHERE stanza_id = ?")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    for item_id in affected_items {
        record_item_location_event(
            &mut tx,
            item_id,
            "spostamento",
            &before,
            &after,
            Some(room_event_id),
        )
        .await?;
    }

    crate::modules::storico::mark_entity_deleted(&mut tx, room_history_id).await?;
    let result = sqlx::query("DELETE FROM stanze WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

async fn get_room(pool: &SqlitePool, id: i64) -> Result<Option<RoomRecord>, sqlx::Error> {
    sqlx::query_as::<_, RoomRecord>(
        "SELECT s.id AS id, s.abitazione_id AS home_id, s.nome AS name, a.nome AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

async fn list_rooms_for_home(
    pool: &SqlitePool,
    home_id: i64,
) -> Result<Vec<RoomRecord>, sqlx::Error> {
    sqlx::query_as::<_, RoomRecord>(
        "SELECT s.id AS id, s.abitazione_id AS home_id, s.nome AS name, a.nome AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.abitazione_id = ? ORDER BY s.nome COLLATE NOCASE, s.id",
    )
    .bind(home_id)
    .fetch_all(pool)
    .await
}

async fn count_rooms_for_home(pool: &SqlitePool, home_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM stanze WHERE abitazione_id = ?")
        .bind(home_id)
        .fetch_one(pool)
        .await
}

async fn count_items_for_home(pool: &SqlitePool, home_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM item_luogo WHERE abitazione_id = ?")
        .bind(home_id)
        .fetch_one(pool)
        .await
}

async fn count_items_for_room(pool: &SqlitePool, room_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM item_luogo WHERE stanza_id = ?")
        .bind(room_id)
        .fetch_one(pool)
        .await
}

async fn object_exists(pool: &SqlitePool, item_id: i64) -> Result<bool, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE id = ? AND tipo = 'oggetto'")
            .bind(item_id)
            .fetch_one(pool)
            .await?;
    Ok(count == 1)
}

async fn get_item_location(
    pool: &SqlitePool,
    item_id: i64,
) -> Result<Option<ItemLocation>, sqlx::Error> {
    sqlx::query_as::<_, ItemLocation>(
        "SELECT il.abitazione_id AS home_id, il.stanza_id AS room_id, \
                a.nome AS home_name, s.nome AS room_name \
         FROM item_luogo il \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE il.item_id = ?",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await
}

async fn get_item_location_tx(
    tx: &mut Transaction<'_, Sqlite>,
    item_id: i64,
) -> Result<Option<ItemLocation>, sqlx::Error> {
    sqlx::query_as::<_, ItemLocation>(
        "SELECT il.abitazione_id AS home_id, il.stanza_id AS room_id, \
                a.nome AS home_name, s.nome AS room_name \
         FROM item_luogo il \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE il.item_id = ?",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await
}

async fn set_item_home(pool: &SqlitePool, item_id: i64, home_id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let previous = get_item_location_tx(&mut tx, item_id).await?;

    if previous
        .as_ref()
        .is_some_and(|location| location.home_id == home_id && location.room_id.is_none())
    {
        return Ok(());
    }

    let before = if let Some(previous) = previous.as_ref() {
        history_snapshot_from_item_location(&mut tx, previous).await?
    } else {
        crate::modules::storico::LocationSnapshot::default()
    };
    let after = history_location_snapshot(&mut tx, home_id, None).await?;

    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id) VALUES (?, ?, NULL) \
         ON CONFLICT(item_id) DO UPDATE SET abitazione_id = excluded.abitazione_id, stanza_id = NULL",
    )
    .bind(item_id)
    .bind(home_id)
    .execute(&mut *tx)
    .await?;

    let operation = if previous.is_some() {
        "spostamento"
    } else {
        "assegnazione"
    };
    record_item_location_event(&mut tx, item_id, operation, &before, &after, None).await?;

    tx.commit().await?;
    Ok(())
}

async fn set_item_room(pool: &SqlitePool, item_id: i64, room_id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let home_id: i64 = sqlx::query_scalar("SELECT abitazione_id FROM stanze WHERE id = ?")
        .bind(room_id)
        .fetch_one(&mut *tx)
        .await?;
    let previous = get_item_location_tx(&mut tx, item_id).await?;

    if previous
        .as_ref()
        .is_some_and(|location| location.home_id == home_id && location.room_id == Some(room_id))
    {
        return Ok(());
    }

    let before = if let Some(previous) = previous.as_ref() {
        history_snapshot_from_item_location(&mut tx, previous).await?
    } else {
        crate::modules::storico::LocationSnapshot::default()
    };
    let after = history_location_snapshot(&mut tx, home_id, Some(room_id)).await?;

    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id) VALUES (?, ?, ?) \
         ON CONFLICT(item_id) DO UPDATE SET abitazione_id = excluded.abitazione_id, stanza_id = excluded.stanza_id",
    )
    .bind(item_id)
    .bind(home_id)
    .bind(room_id)
    .execute(&mut *tx)
    .await?;

    let operation = if previous.is_some() {
        "spostamento"
    } else {
        "assegnazione"
    };
    record_item_location_event(&mut tx, item_id, operation, &before, &after, None).await?;

    tx.commit().await?;
    Ok(())
}

async fn clear_item_location(pool: &SqlitePool, item_id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let Some(previous) = get_item_location_tx(&mut tx, item_id).await? else {
        return Ok(());
    };
    let before = history_snapshot_from_item_location(&mut tx, &previous).await?;
    let after = crate::modules::storico::LocationSnapshot::default();

    sqlx::query("DELETE FROM item_luogo WHERE item_id = ?")
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

    record_item_location_event(&mut tx, item_id, "rimozione", &before, &after, None).await?;

    tx.commit().await?;
    Ok(())
}

async fn count_objects_for_home(pool: &SqlitePool, home_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM items i JOIN item_luogo il ON il.item_id = i.id \
         WHERE i.tipo = 'oggetto' AND il.abitazione_id = ?",
    )
    .bind(home_id)
    .fetch_one(pool)
    .await
}

async fn count_objects_for_room(pool: &SqlitePool, room_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM items i JOIN item_luogo il ON il.item_id = i.id \
         WHERE i.tipo = 'oggetto' AND il.stanza_id = ?",
    )
    .bind(room_id)
    .fetch_one(pool)
    .await
}

async fn list_objects_for_home(
    pool: &SqlitePool,
    home_id: i64,
    limit: i64,
) -> Result<Vec<LocatedObjectSummary>, sqlx::Error> {
    sqlx::query_as::<_, LocatedObjectSummary>(
        "SELECT i.id AS id, i.nome AS name, o.posizione AS detail_position, \
                a.nome AS home_name, s.nome AS room_name \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         JOIN item_luogo il ON il.item_id = i.id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE i.tipo = 'oggetto' AND il.abitazione_id = ? \
         ORDER BY i.nome COLLATE NOCASE, i.id LIMIT ?",
    )
    .bind(home_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

async fn list_objects_for_room(
    pool: &SqlitePool,
    room_id: i64,
    limit: i64,
) -> Result<Vec<LocatedObjectSummary>, sqlx::Error> {
    sqlx::query_as::<_, LocatedObjectSummary>(
        "SELECT i.id AS id, i.nome AS name, o.posizione AS detail_position, \
                a.nome AS home_name, s.nome AS room_name \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         JOIN item_luogo il ON il.item_id = i.id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE i.tipo = 'oggetto' AND il.stanza_id = ? \
         ORDER BY i.nome COLLATE NOCASE, i.id LIMIT ?",
    )
    .bind(room_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

fn locations_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("➕ Nuova casa", "loc:home:new")],
        vec![button("📋 Elenco case", "loc:home:list")],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn home_list_keyboard(homes: &[HomeRecord]) -> InlineKeyboardMarkup {
    let mut rows = homes
        .iter()
        .map(|home| {
            vec![button(
                &format!("🏠 #{} · {}", home.id, truncate_chars(&home.name, 38)),
                &format!("loc:home:{}", home.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button("➕ Nuova casa", "loc:home:new")]);
    rows.push(vec![button("🏠 Menu principale", "menu:main")]);
    InlineKeyboardMarkup::new(rows)
}

fn home_detail_keyboard(home_id: i64, rooms: &[RoomRecord]) -> InlineKeyboardMarkup {
    let mut rows = vec![vec![button(
        "➕ Nuova stanza",
        &format!("loc:room:new:{home_id}"),
    )]];
    for room in rooms.iter().take(20) {
        rows.push(vec![button(
            &format!("🚪 {}", truncate_chars(&room.name, 42)),
            &format!("loc:room:{}", room.id),
        )]);
    }
    rows.push(vec![button(
        "📦 Oggetti in questa casa",
        &format!("loc:filter:home:{home_id}"),
    )]);
    rows.push(vec![
        button("✏️ Rinomina", &format!("loc:home:rename:{home_id}")),
        button("🗑 Elimina", &format!("loc:home:delete:ask:{home_id}")),
    ]);
    rows.push(vec![button("↩️ Elenco case", "loc:home:list")]);
    InlineKeyboardMarkup::new(rows)
}

fn room_detail_keyboard(room: &RoomRecord) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "📦 Oggetti nella stanza",
            &format!("loc:filter:room:{}", room.id),
        )],
        vec![
            button("✏️ Rinomina", &format!("loc:room:rename:{}", room.id)),
            button("🗑 Elimina", &format!("loc:room:delete:ask:{}", room.id)),
        ],
        vec![button(
            "↩️ Torna alla casa",
            &format!("loc:home:{}", room.home_id),
        )],
    ])
}

fn home_delete_keyboard(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "🗑 Sì, elimina casa",
            &format!("loc:home:delete:do:{id}"),
        )],
        vec![button("↩️ Annulla", &format!("loc:home:{id}"))],
    ])
}

fn room_delete_keyboard(room: &RoomRecord) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "🗑 Sì, elimina stanza",
            &format!("loc:room:delete:do:{}", room.id),
        )],
        vec![button("↩️ Annulla", &format!("loc:room:{}", room.id))],
    ])
}

fn item_home_picker_keyboard(item_id: i64, homes: &[HomeRecord]) -> InlineKeyboardMarkup {
    let mut rows = homes
        .iter()
        .take(20)
        .map(|home| {
            vec![button(
                &format!("🏠 {}", truncate_chars(&home.name, 42)),
                &format!("loc:item:home:{item_id}:{}", home.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        "🧹 Nessun luogo",
        &format!("loc:item:clear:{item_id}"),
    )]);
    rows.push(vec![button(
        "↩️ Scheda oggetto",
        &format!("oggetti:view:{item_id}"),
    )]);
    InlineKeyboardMarkup::new(rows)
}

fn item_room_picker_keyboard(
    item_id: i64,
    home_id: i64,
    rooms: &[RoomRecord],
    current_location: Option<&ItemLocation>,
    selected_home_name: &str,
) -> InlineKeyboardMarkup {
    let is_move = has_structured_location(current_location);
    let current_is_selected_home = current_location
        .and_then(|location| location.home_name.as_deref())
        == Some(selected_home_name);
    let current_room_name = current_location
        .filter(|_| current_is_selected_home)
        .and_then(|location| location.room_name.as_deref());
    let current_is_home_only = current_is_selected_home && current_room_name.is_none();

    let home_label = if is_move && current_is_home_only {
        "🚚 Sposta qui (solo casa) · Attualmente qui"
    } else if is_move {
        "🚚 Sposta qui (solo casa)"
    } else {
        "🏠 Assegna solo alla casa"
    };
    let mut rows = vec![vec![button(
        home_label,
        &format!("loc:item:sethome:{item_id}:{home_id}"),
    )]];
    for room in rooms.iter().take(20) {
        let is_current_room = current_room_name == Some(room.name.as_str());
        let room_label = room_picker_label(&room.name, is_move, is_current_room);
        rows.push(vec![button(
            &room_label,
            &format!("loc:item:setroom:{item_id}:{}", room.id),
        )]);
    }
    rows.push(vec![button(
        "↩️ Scegli un'altra casa",
        &format!("loc:item:{item_id}"),
    )]);
    InlineKeyboardMarkup::new(rows)
}

fn room_picker_label(room_name: &str, is_move: bool, is_current: bool) -> String {
    if is_move && is_current {
        format!("🚚 → {} (Attualmente qui)", truncate_chars(room_name, 24))
    } else if is_move {
        format!("🚚 → {}", truncate_chars(room_name, 36))
    } else {
        format!("🚪 {}", truncate_chars(room_name, 42))
    }
}

fn filtered_objects_keyboard(
    objects: &[LocatedObjectSummary],
    back_callback: &str,
) -> InlineKeyboardMarkup {
    let mut rows = objects
        .iter()
        .map(|object| {
            vec![button(
                &format!("📦 #{} · {}", object.id, truncate_chars(&object.name, 36)),
                &format!("oggetti:view:{}", object.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button("↩️ Indietro", back_callback)]);
    InlineKeyboardMarkup::new(rows)
}

fn button(label: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.to_string(), data.to_string())
}

fn clean_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let len = trimmed.chars().count();
    if trimmed.is_empty() || len > 120 {
        None
    } else {
        Some(trimmed.to_string())
    }
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

fn parse_positive_id(value: &str) -> Option<i64> {
    let id = value.trim().parse::<i64>().ok()?;
    (id > 0).then_some(id)
}

fn split_id_and_rest(value: &str) -> Option<(i64, &str)> {
    let trimmed = value.trim();
    let split_at = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let id = parse_positive_id(&trimmed[..split_at])?;
    Some((id, trimmed[split_at..].trim()))
}

fn parse_id_callback(data: &str, prefix: &str) -> Option<i64> {
    parse_positive_id(data.strip_prefix(prefix)?)
}

fn parse_two_ids_callback(data: &str, prefix: &str) -> Option<(i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let (first, second) = rest.split_once(':')?;
    if second.contains(':') {
        return None;
    }
    Some((parse_positive_id(first)?, parse_positive_id(second)?))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
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

    async fn create_test_object(pool: &SqlitePool, name: &str) -> i64 {
        let item = sqlx::query("INSERT INTO items (tipo, nome) VALUES ('oggetto', ?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("item test");
        let id = item.last_insert_rowid();
        sqlx::query("INSERT INTO oggetti (item_id) VALUES (?)")
            .bind(id)
            .execute(pool)
            .await
            .expect("dettaglio oggetto test");
        id
    }

    #[test]
    fn parser_callback_due_id_rifiuta_formati_ambigui() {
        assert_eq!(
            parse_two_ids_callback("loc:item:setroom:12:7", "loc:item:setroom:"),
            Some((12, 7))
        );
        assert_eq!(
            parse_two_ids_callback("loc:item:setroom:12:7:3", "loc:item:setroom:"),
            None
        );
    }

    #[tokio::test]
    async fn case_e_stanze_vengono_create_e_lette() {
        let pool = test_pool().await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");

        let home = get_home(&pool, home_id)
            .await
            .expect("lettura casa")
            .expect("casa presente");
        let room = get_room(&pool, room_id)
            .await
            .expect("lettura stanza")
            .expect("stanza presente");

        assert_eq!(home.name, "Casa principale");
        assert_eq!(room.home_id, home_id);
        assert_eq!(room.name, "Garage");
    }

    #[tokio::test]
    async fn nomi_case_e_stanze_sono_unici_nel_proprio_ambito() {
        let pool = test_pool().await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        assert!(create_home(&pool, "casa PRINCIPALE").await.is_err());

        create_room(&pool, home_id, "Garage").await.expect("stanza");
        assert!(create_room(&pool, home_id, "GARAGE").await.is_err());

        let second_home = create_home(&pool, "Casa al mare")
            .await
            .expect("seconda casa");
        assert!(create_room(&pool, second_home, "Garage").await.is_ok());
    }

    #[tokio::test]
    async fn oggetto_puo_stare_in_casa_o_in_una_stanza() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Trapano").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");

        set_item_home(&pool, item_id, home_id)
            .await
            .expect("assegnazione casa");
        let home_only: (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT abitazione_id, stanza_id FROM item_luogo WHERE item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .expect("luogo presente");
        assert_eq!(home_only, (Some(home_id), None));

        set_item_room(&pool, item_id, room_id)
            .await
            .expect("assegnazione stanza");
        let in_room: (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT abitazione_id, stanza_id FROM item_luogo WHERE item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .expect("luogo presente");
        assert_eq!(in_room, (Some(home_id), Some(room_id)));
    }

    #[tokio::test]
    async fn trigger_rifiuta_stanza_di_un_altra_casa() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Trapano").await;
        let home_a = create_home(&pool, "Casa A").await.expect("casa A");
        let home_b = create_home(&pool, "Casa B").await.expect("casa B");
        let room_b = create_room(&pool, home_b, "Garage")
            .await
            .expect("stanza B");

        let result = sqlx::query(
            "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id) VALUES (?, ?, ?)",
        )
        .bind(item_id)
        .bind(home_a)
        .bind(room_b)
        .execute(&pool)
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn eliminare_stanza_mantiene_la_casa_sull_oggetto() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Trapano").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");
        set_item_room(&pool, item_id, room_id)
            .await
            .expect("assegnazione");

        assert!(delete_room(&pool, room_id).await.expect("delete stanza"));
        let location: (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT abitazione_id, stanza_id FROM item_luogo WHERE item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .expect("riga luogo presente");
        assert_eq!(location, (Some(home_id), None));

        let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE id = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .expect("item");
        assert_eq!(item_count, 1);
    }

    #[tokio::test]
    async fn eliminare_casa_non_elimina_oggetto_e_rimuove_il_luogo() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Trapano").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");
        set_item_room(&pool, item_id, room_id)
            .await
            .expect("assegnazione");

        assert!(delete_home(&pool, home_id).await.expect("delete casa"));

        let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE id = ?")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .expect("item");
        assert_eq!(item_count, 1);

        let location_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM item_luogo WHERE item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .expect("luogo");
        assert_eq!(location_count, 0);
    }

    #[test]
    fn picker_stanza_segnala_quella_attuale_durante_lo_spostamento() {
        let current = room_picker_label("Garage", true, true);
        assert!(current.contains("🚚 → Garage"));
        assert!(current.contains("Attualmente qui"));

        let other = room_picker_label("Camera", true, false);
        assert!(other.contains("🚚 → Camera"));
        assert!(!other.contains("Attualmente qui"));

        let first_assignment = room_picker_label("Garage", false, false);
        assert!(first_assignment.starts_with("🚪 Garage"));
    }

    #[test]
    fn messaggio_luogo_distingue_assegnazione_spostamento_e_rimozione() {
        let nessun_luogo = None;
        let garage = ItemLocation {
            home_id: 1,
            room_id: Some(10),
            home_name: Some("Casa principale".to_string()),
            room_name: Some("Garage".to_string()),
        };
        let camera = ItemLocation {
            home_id: 1,
            room_id: Some(11),
            home_name: Some("Casa principale".to_string()),
            room_name: Some("Camera".to_string()),
        };

        let assegnazione = location_change_message(nessun_luogo.as_ref(), Some(&garage));
        assert!(assegnazione.contains("Luogo assegnato"));
        assert!(assegnazione.contains("Garage"));

        let spostamento = location_change_message(Some(&garage), Some(&camera));
        assert!(spostamento.contains("Oggetto spostato"));
        assert!(spostamento.contains("Da:"));
        assert!(spostamento.contains("A:"));
        assert!(spostamento.contains("Garage"));
        assert!(spostamento.contains("Camera"));

        let invariato = location_change_message(Some(&camera), Some(&camera));
        assert!(invariato.contains("Nessuno spostamento effettuato"));

        let rimozione = location_change_message(Some(&camera), nessun_luogo.as_ref());
        assert!(rimozione.contains("Luogo rimosso"));
        assert!(rimozione.contains("Prima:"));
    }
}
