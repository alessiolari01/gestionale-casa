//! Case, stanze e posizione strutturata degli item.
//!
//! Step 6C.4 estende lo storico della posizione fino ai contenitori annidabili,
//! mantenendo snapshot immutabili di casa, stanza e percorso del contenitore.
//! La navigazione resta quella completata nel 6C.3C:
//! abitazione -> stanza -> contenitori annidabili -> oggetto.

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
enum LocationReturnTarget {
    LocationsMenu,
    CreateMenu,
    Home(i64),
    Room(i64),
}

#[derive(Debug, Clone)]
enum LocationConversationState {
    AwaitingHomeName {
        rename_id: Option<i64>,
        return_to: LocationReturnTarget,
    },
    AwaitingRoomName {
        home_id: i64,
        rename_id: Option<i64>,
        return_to: LocationReturnTarget,
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

#[derive(Debug, Clone, FromRow)]
struct TreeContainerRecord {
    id: i64,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
    name: String,
}

#[derive(Debug, Clone)]
struct LocationTreeNode {
    label: String,
    command: String,
    children: Vec<LocationTreeNode>,
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

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
struct ItemLocation {
    home_id: i64,
    room_id: Option<i64>,
    container_id: Option<i64>,
    home_name: Option<String>,
    home_space_name: Option<String>,
    room_name: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct LocatedObjectSummary {
    id: i64,
    name: String,
    home_id: Option<i64>,
    home_name: Option<String>,
    room_id: Option<i64>,
    room_name: Option<String>,
    container_id: Option<i64>,
}

fn location_return_target(state: &LocationConversationState) -> LocationReturnTarget {
    match state {
        LocationConversationState::AwaitingHomeName { return_to, .. }
        | LocationConversationState::AwaitingRoomName { return_to, .. } => return_to.clone(),
    }
}

async fn show_location_return_target(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    target: LocationReturnTarget,
) -> ResponseResult<()> {
    match target {
        LocationReturnTarget::LocationsMenu => show_menu(bot, chat_id).await?,
        LocationReturnTarget::CreateMenu => show_create_menu(bot, chat_id).await?,
        LocationReturnTarget::Home(id) => show_home_detail(bot, chat_id, pool, id).await?,
        LocationReturnTarget::Room(id) => show_room_detail(bot, chat_id, pool, id).await?,
    }
    Ok(())
}

pub async fn show_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🏠 Case, stanze e contenitori\n\nVisualizza, crea e naviga tutti i luoghi del gestionale da un'unica sezione.",
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
        if let Some((kind, id)) = parse_location_command(command) {
            sessions.clear_chat(chat_id);
            match kind {
                'h' => show_home_detail(bot, msg.chat.id, pool, id).await?,
                'r' => show_room_detail(bot, msg.chat.id, pool, id).await?,
                'c' => {
                    crate::modules::contenitori::show_container_detail(bot, msg.chat.id, pool, id)
                        .await?
                }
                _ => {}
            }
            return Ok(true);
        }

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
                        LocationConversationState::AwaitingHomeName {
                            rename_id: None,
                            return_to: LocationReturnTarget::LocationsMenu,
                        },
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
            "/stanze_lista" => {
                sessions.clear_chat(chat_id);
                send_room_list(bot, msg.chat.id, pool).await?;
                return Ok(true);
            }
            "/struttura" => {
                sessions.clear_chat(chat_id);
                send_location_tree(bot, msg.chat.id, pool).await?;
                return Ok(true);
            }
            "/casa" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    show_home_detail(bot, msg.chat.id, pool, id).await?;
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
                                return_to: LocationReturnTarget::Home(id),
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
                                return_to: LocationReturnTarget::Home(home_id),
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
                    show_room_detail(bot, msg.chat.id, pool, id).await?;
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
                                    return_to: LocationReturnTarget::Room(id),
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
                if let Some(state) = sessions.get(chat_id) {
                    let return_to = location_return_target(&state);
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "↩️ Operazione annullata.")
                        .await?;
                    show_location_return_target(bot, msg.chat.id, pool, return_to).await?;
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
        LocationConversationState::AwaitingHomeName {
            rename_id: None, ..
        } => {
            create_home_from_input(bot, msg.chat.id, pool, sessions, text).await?;
        }
        LocationConversationState::AwaitingHomeName {
            rename_id: Some(id),
            ..
        } => {
            rename_home_from_input(bot, msg.chat.id, pool, sessions, id, text).await?;
        }
        LocationConversationState::AwaitingRoomName {
            home_id,
            rename_id: None,
            ..
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
        "loc:create" => {
            sessions.clear_chat(raw_chat_id);
            show_create_menu(bot, chat_id).await?;
        }
        "loc:tree" => {
            sessions.clear_chat(raw_chat_id);
            send_location_tree(bot, chat_id, pool).await?;
        }
        "loc:room:list" => {
            sessions.clear_chat(raw_chat_id);
            send_room_list(bot, chat_id, pool).await?;
        }
        "loc:room:new:pick" => {
            sessions.clear_chat(raw_chat_id);
            show_home_picker_for_new_room(bot, chat_id, pool).await?;
        }
        "loc:home:new" => {
            sessions.set(
                raw_chat_id,
                LocationConversationState::AwaitingHomeName {
                    rename_id: None,
                    return_to: LocationReturnTarget::CreateMenu,
                },
            );
            ask_home_name(bot, chat_id, false).await?;
        }
        "loc:home:list" => {
            sessions.clear_chat(raw_chat_id);
            send_home_list(bot, chat_id, pool).await?;
        }
        _ if data.starts_with("loc:home:manage:") => {
            if let Some(id) = parse_id_callback(data, "loc:home:manage:") {
                sessions.clear_chat(raw_chat_id);
                show_home_manage(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:home:rename:") => {
            if let Some(id) = parse_id_callback(data, "loc:home:rename:") {
                if home_exists(pool, id).await.unwrap_or(false) {
                    sessions.set(
                        raw_chat_id,
                        LocationConversationState::AwaitingHomeName {
                            rename_id: Some(id),
                            return_to: LocationReturnTarget::Home(id),
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
                            return_to: LocationReturnTarget::Home(home_id),
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
        _ if data.starts_with("loc:room:manage:") => {
            if let Some(id) = parse_id_callback(data, "loc:room:manage:") {
                sessions.clear_chat(raw_chat_id);
                show_room_manage(bot, chat_id, pool, id).await?;
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
                                return_to: LocationReturnTarget::Room(id),
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
        _ if data.starts_with("loc:item:setcontainer:") => {
            if let Some((item_id, container_id)) =
                parse_two_ids_callback(data, "loc:item:setcontainer:")
            {
                let previous = get_item_location(pool, item_id).await.unwrap_or(None);
                match set_item_container(pool, item_id, container_id).await {
                    Ok(()) => {
                        let current = get_item_location(pool, item_id).await.unwrap_or(None);
                        bot.send_message(
                            chat_id,
                            location_change_message_full(pool, previous.as_ref(), current.as_ref())
                                .await,
                        )
                        .await?;
                        crate::modules::oggetti::send_object_detail(bot, chat_id, pool, item_id)
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(
                            ?error,
                            item_id,
                            container_id,
                            "Errore assegnazione contenitore"
                        );
                        bot.send_message(
                            chat_id,
                            "⚠️ Non riesco a spostare l'oggetto in questo contenitore.",
                        )
                        .await?;
                    }
                }
            }
        }
        _ if data.starts_with("loc:item:container:") => {
            if let Some((item_id, container_id)) =
                parse_two_ids_callback(data, "loc:item:container:")
            {
                show_container_destination_picker(bot, chat_id, pool, item_id, container_id)
                    .await?;
            }
        }
        _ if data.starts_with("loc:item:room:") => {
            if let Some((item_id, room_id)) = parse_two_ids_callback(data, "loc:item:room:") {
                show_room_destination_picker(bot, chat_id, pool, item_id, room_id).await?;
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
                            location_change_message_full(pool, previous.as_ref(), current.as_ref())
                                .await,
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
                            location_change_message_full(pool, previous.as_ref(), current.as_ref())
                                .await,
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
                            location_change_message_full(pool, previous.as_ref(), current.as_ref())
                                .await,
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
                show_home_destination_picker(bot, chat_id, pool, item_id, home_id).await?;
            }
        }
        _ if data.starts_with("loc:item:") => {
            if let Some(item_id) = parse_id_callback(data, "loc:item:") {
                show_item_location_picker(bot, chat_id, pool, item_id).await?;
            }
        }
        _ if data.starts_with("loc:room:") => {
            if let Some(id) = parse_id_callback(data, "loc:room:") {
                show_room_detail(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("loc:home:") => {
            if let Some(id) = parse_id_callback(data, "loc:home:") {
                show_home_detail(bot, chat_id, pool, id).await?;
            }
        }
        _ => return Ok(false),
    }

    Ok(true)
}

async fn send_location_tree(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let homes = list_homes(pool).await.unwrap_or_default();
    if homes.is_empty() {
        bot.send_message(chat_id, "🌳 La struttura è vuota: non ci sono ancora case.")
            .reply_markup(location_navigation_keyboard())
            .await?;
        return Ok(());
    }

    let room_sql = format!(
        "SELECT s.id AS id, s.abitazione_id AS home_id, s.nome AS name, a.nome AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE {} ORDER BY a.nome COLLATE NOCASE, s.nome COLLATE NOCASE, s.id",
        crate::identity::visible_space_sql("a")
    );
    let rooms = sqlx::query_as::<_, RoomRecord>(&room_sql)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let container_sql = format!(
        "SELECT c.id, c.abitazione_id AS home_id, c.stanza_id AS room_id, \
                c.contenitore_padre_id AS parent_id, c.nome AS name \
         FROM contenitori c JOIN abitazioni a ON a.id = c.abitazione_id \
         WHERE {} ORDER BY a.nome COLLATE NOCASE, c.nome COLLATE NOCASE, c.id",
        crate::identity::visible_space_sql("a")
    );
    let containers = sqlx::query_as::<_, TreeContainerRecord>(&container_sql)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let mut text = "🌳 Struttura completa dei luoghi\n\n".to_string();
    for home in &homes {
        text.push_str(&format!("🏠 {}  /luogo_h{}\n", home.name, home.id));
        let nodes = build_home_nodes(home.id, &rooms, &containers);
        render_tree_nodes(&nodes, "", &mut text, 0);
        text.push('\n');
        if text.chars().count() > 3500 {
            text.push_str("… struttura abbreviata per il limite dei messaggi Telegram.\n");
            break;
        }
    }
    text.push_str("Tocca un comando /luogo_… per aprire direttamente quel luogo.");

    bot.send_message(chat_id, text)
        .reply_markup(location_navigation_keyboard())
        .await?;
    Ok(())
}

fn build_home_nodes(
    home_id: i64,
    rooms: &[RoomRecord],
    containers: &[TreeContainerRecord],
) -> Vec<LocationTreeNode> {
    let mut nodes = Vec::new();
    for room in rooms.iter().filter(|room| room.home_id == home_id) {
        let children = build_container_nodes(containers, home_id, Some(room.id), None, 0);
        nodes.push(LocationTreeNode {
            label: format!("🚪 {}", room.name),
            command: format!("/luogo_r{}", room.id),
            children,
        });
    }
    nodes.extend(build_container_nodes(containers, home_id, None, None, 0));
    nodes
}

fn build_container_nodes(
    containers: &[TreeContainerRecord],
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
    depth: usize,
) -> Vec<LocationTreeNode> {
    if depth >= 20 {
        return Vec::new();
    }
    containers
        .iter()
        .filter(|container| {
            container.home_id == home_id
                && container.room_id == room_id
                && container.parent_id == parent_id
        })
        .map(|container| LocationTreeNode {
            label: format!("📦 {}", container.name),
            command: format!("/luogo_c{}", container.id),
            children: build_container_nodes(
                containers,
                home_id,
                room_id,
                Some(container.id),
                depth + 1,
            ),
        })
        .collect()
}

fn render_tree_nodes(nodes: &[LocationTreeNode], prefix: &str, text: &mut String, depth: usize) {
    if depth >= 20 || text.chars().count() > 3500 {
        return;
    }
    for (index, node) in nodes.iter().enumerate() {
        let last = index + 1 == nodes.len();
        let branch = if last { "└── " } else { "├── " };
        text.push_str(&format!(
            "{prefix}{branch}{}  {}\n",
            node.label, node.command
        ));
        let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
        render_tree_nodes(&node.children, &child_prefix, text, depth + 1);
        if text.chars().count() > 3500 {
            break;
        }
    }
}

fn parse_location_command(command: &str) -> Option<(char, i64)> {
    let value = command.strip_prefix("/luogo_")?;
    let mut chars = value.chars();
    let kind = chars.next()?;
    if !matches!(kind, 'h' | 'r' | 'c') {
        return None;
    }
    let id = chars.as_str().parse::<i64>().ok().filter(|id| *id > 0)?;
    Some((kind, id))
}

async fn ask_home_name(bot: &Bot, chat_id: ChatId, rename: bool) -> ResponseResult<()> {
    let text = if rename {
        "✏️ Scrivi il nuovo nome della casa.\n\n/annulla per uscire."
    } else {
        "➕ Nuova casa\n\nScrivi il nome dell'abitazione.\nEsempio: Casa principale\n\n/annulla per uscire."
    };
    bot.send_message(chat_id, text)
        .reply_markup(location_navigation_keyboard())
        .await?;
    Ok(())
}

async fn ask_room_name(bot: &Bot, chat_id: ChatId, rename: bool) -> ResponseResult<()> {
    let text = if rename {
        "✏️ Scrivi il nuovo nome della stanza.\n\n/annulla per uscire."
    } else {
        "➕ Nuova stanza\n\nScrivi il nome della stanza.\nEsempio: Garage\n\n/annulla per uscire."
    };
    bot.send_message(chat_id, text)
        .reply_markup(location_navigation_keyboard())
        .await?;
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
            show_home_detail(bot, chat_id, pool, id).await?;
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
            show_home_detail(bot, chat_id, pool, id).await?;
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
            show_room_detail(bot, chat_id, pool, id).await?;
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
            show_room_detail(bot, chat_id, pool, room.id).await?;
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
                text.push_str(&format!(
                    "#{} · {}\n/luogo_h{}\n\n",
                    home.id, home.name, home.id
                ));
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

async fn send_room_list(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let rooms_sql = format!(
        "SELECT s.id AS id, s.abitazione_id AS home_id, s.nome AS name, \
                CASE WHEN ? = 1 THEN a.nome || ' · ' || sp.nome ELSE a.nome END AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         JOIN spazi sp ON sp.id = a.spazio_id \
         WHERE {} ORDER BY sp.nome COLLATE NOCASE, a.nome COLLATE NOCASE, s.nome COLLATE NOCASE, s.id",
        crate::identity::visible_space_sql("a")
    );
    let rooms = sqlx::query_as::<_, RoomRecord>(&rooms_sql)
        .bind(if crate::identity::current_view_all() {
            1_i64
        } else {
            0_i64
        })
        .bind(crate::identity::visible_space_bind_id())
        .fetch_all(pool)
        .await;

    match rooms {
        Ok(rooms) if rooms.is_empty() => {
            bot.send_message(chat_id, "🚪 Non ci sono ancora stanze registrate.")
                .reply_markup(location_navigation_keyboard())
                .await?;
        }
        Ok(rooms) => {
            let mut text = "🚪 Elenco stanze\n\n".to_string();
            let mut rows = Vec::new();
            for room in rooms.iter().take(50) {
                text.push_str(&format!(
                    "#{} · {}\n📍 {}\n/luogo_r{}\n\n",
                    room.id, room.name, room.home_name, room.id
                ));
                rows.push(vec![button(
                    &format!("🚪 #{} · {}", room.id, truncate_chars(&room.name, 32)),
                    &format!("loc:room:{}", room.id),
                )]);
                if text.chars().count() > 3200 {
                    text.push_str(
                        "… elenco testuale abbreviato. Usa i pulsanti o la struttura completa.\n",
                    );
                    break;
                }
            }
            rows.push(vec![
                button("↩️ Case, stanze e contenitori", "loc:menu"),
                button("🏠 Menu principale", "menu:main"),
            ]);
            bot.send_message(chat_id, text)
                .reply_markup(InlineKeyboardMarkup::new(rows))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco stanze");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere l'elenco delle stanze.")
                .reply_markup(location_navigation_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_create_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "➕ Crea un nuovo luogo\n\nCosa vuoi aggiungere?")
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![
                button("➕🏠 Casa", "loc:home:new"),
                button("➕🚪 Stanza", "loc:room:new:pick"),
                button("➕📦 Contenitore", "c:n"),
            ],
            vec![
                button("↩️ Case, stanze e contenitori", "loc:menu"),
                button("🏠 Menu principale", "menu:main"),
            ],
        ]))
        .await?;
    Ok(())
}

async fn show_home_picker_for_new_room(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
) -> ResponseResult<()> {
    let homes = list_homes(pool).await.unwrap_or_default();
    if homes.is_empty() {
        bot.send_message(chat_id, "🚪 Per creare una stanza serve prima una casa.")
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![button("➕ Crea una casa", "loc:home:new")],
                vec![
                    button("↩️ Crea…", "loc:create"),
                    button("🏠 Menu principale", "menu:main"),
                ],
            ]))
            .await?;
        return Ok(());
    }

    let mut rows = homes
        .iter()
        .take(30)
        .map(|home| {
            vec![button(
                &format!("🏠 {}", truncate_chars(&home.name, 38)),
                &format!("loc:room:new:{}", home.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![
        button("↩️ Crea…", "loc:create"),
        button("🏠 Menu principale", "menu:main"),
    ]);
    bot.send_message(chat_id, "🚪 Nuova stanza\n\nScegli la casa.")
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

pub(crate) async fn show_home_detail(
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

pub(crate) async fn show_room_detail(
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

async fn show_home_manage(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    let Some(home) = get_home(pool, id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Casa #{id} non trovata."))
            .reply_markup(locations_menu_keyboard())
            .await?;
        return Ok(());
    };

    bot.send_message(
        chat_id,
        format!("⚙️ Gestisci casa\n\n🏠 #{} · {}", home.id, home.name),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button("✏️ Rinomina", &format!("loc:home:rename:{id}"))],
        vec![button(
            "🗑 Elimina casa",
            &format!("loc:home:delete:ask:{id}"),
        )],
        vec![
            button("↩️ Torna alla casa", &format!("loc:home:{id}")),
            button("🏠 Menu principale", "menu:main"),
        ],
    ]))
    .await?;
    Ok(())
}

async fn show_room_manage(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    let Some(room) = get_room(pool, id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Stanza #{id} non trovata."))
            .reply_markup(locations_menu_keyboard())
            .await?;
        return Ok(());
    };

    bot.send_message(
        chat_id,
        format!(
            "⚙️ Gestisci stanza\n\n🚪 #{} · {}\n🏠 {}",
            room.id, room.name, room.home_name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button("✏️ Rinomina", &format!("loc:room:rename:{id}"))],
        vec![button(
            "🗑 Elimina stanza",
            &format!("loc:room:delete:ask:{id}"),
        )],
        vec![
            button("↩️ Torna alla stanza", &format!("loc:room:{id}")),
            button("🏠 Menu principale", "menu:main"),
        ],
    ]))
    .await?;
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
            let container_count = count_containers_for_room(pool, id).await.unwrap_or(0);
            bot.send_message(
                chat_id,
                format!(
                    "⚠️ Eliminare la stanza?\n\n🚪 {}\n🏠 {}\n#{}\n\nI {} elementi collegati NON verranno eliminati: resteranno associati alla casa, ma senza stanza.\nI {} contenitori della stanza verranno mantenuti e promossi direttamente nella casa, conservando la loro gerarchia.\n\nL'operazione sulla stanza non può essere annullata.",
                    room.name, room.home_name, room.id, item_count, container_count
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
            show_home_detail(bot, chat_id, pool, room.home_id).await?;
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

    let current = format_location_full(pool, location.as_ref()).await;
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
            "🚚 Sposta oggetto #{item_id}\n\nPosizione attuale: {current}\n\nScegli la nuova casa. Nei passaggi successivi potrai scegliere anche una stanza o un contenitore."
        )
    } else {
        format!(
            "🏠 Assegna luogo all'oggetto #{item_id}\n\nPosizione attuale: {current}\n\nScegli la casa. Nei passaggi successivi potrai scegliere anche una stanza o un contenitore."
        )
    };

    bot.send_message(chat_id, text)
        .reply_markup(item_home_picker_keyboard(item_id, &homes))
        .await?;
    Ok(())
}

async fn show_home_destination_picker(
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
    let containers = crate::modules::contenitori::list_root_containers(pool, home_id, None)
        .await
        .unwrap_or_default();
    let current_location = get_item_location(pool, item_id).await.unwrap_or(None);
    let is_move = has_structured_location(current_location.as_ref());
    let current = format_location_full(pool, current_location.as_ref()).await;
    let action = if is_move {
        "🚚 Sposta oggetto"
    } else {
        "🏠 Assegna luogo"
    };

    let current_is_home_only = current_location.as_ref().is_some_and(|location| {
        location.home_id == home_id && location.room_id.is_none() && location.container_id.is_none()
    });
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
        let exact_room = current_location.as_ref().is_some_and(|location| {
            location.room_id == Some(room.id) && location.container_id.is_none()
        });
        let label = room_picker_label(&room.name, is_move, exact_room);
        rows.push(vec![button(
            &label,
            &format!("loc:item:room:{item_id}:{}", room.id),
        )]);
    }

    for container in containers.iter().take(20) {
        let exact_container = current_location
            .as_ref()
            .and_then(|location| location.container_id)
            == Some(container.id);
        let label = if is_move && exact_container {
            format!(
                "📦 {} · Attualmente qui",
                truncate_chars(&container.name, 28)
            )
        } else {
            format!("📦 {}", truncate_chars(&container.name, 40))
        };
        rows.push(vec![button(
            &label,
            &format!("loc:item:container:{item_id}:{}", container.id),
        )]);
    }

    rows.push(vec![
        button("↩️ Scegli un'altra casa", &format!("loc:item:{item_id}")),
        button("🏠 Menu principale", "menu:main"),
    ]);

    bot.send_message(
        chat_id,
        format!(
            "{action} #{item_id}\n\nPosizione attuale: {current}\nDestinazione scelta: 🏠 {}\n\nPuoi spostare l'oggetto direttamente nella casa, entrare in una stanza oppure scegliere un contenitore direttamente nella casa.",
            home.name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_room_destination_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    item_id: i64,
    room_id: i64,
) -> ResponseResult<()> {
    let Some(room) = get_room(pool, room_id).await.unwrap_or(None) else {
        bot.send_message(chat_id, format!("Stanza #{room_id} non trovata."))
            .reply_markup(locations_menu_keyboard())
            .await?;
        return Ok(());
    };

    let containers =
        crate::modules::contenitori::list_root_containers(pool, room.home_id, Some(room.id))
            .await
            .unwrap_or_default();
    let current_location = get_item_location(pool, item_id).await.unwrap_or(None);
    let is_move = has_structured_location(current_location.as_ref());
    let current = format_location_full(pool, current_location.as_ref()).await;
    let exact_room = current_location.as_ref().is_some_and(|location| {
        location.room_id == Some(room.id) && location.container_id.is_none()
    });

    let room_label = if is_move && exact_room {
        "🚚 Sposta qui (stanza) · Attualmente qui"
    } else if is_move {
        "🚚 Sposta qui (stanza)"
    } else {
        "🚪 Assegna direttamente alla stanza"
    };

    let mut rows = vec![vec![button(
        room_label,
        &format!("loc:item:setroom:{item_id}:{room_id}"),
    )]];

    for container in containers.iter().take(25) {
        let exact_container = current_location
            .as_ref()
            .and_then(|location| location.container_id)
            == Some(container.id);
        let label = if is_move && exact_container {
            format!(
                "📦 {} · Attualmente qui",
                truncate_chars(&container.name, 28)
            )
        } else {
            format!("📦 {}", truncate_chars(&container.name, 40))
        };
        rows.push(vec![button(
            &label,
            &format!("loc:item:container:{item_id}:{}", container.id),
        )]);
    }

    rows.push(vec![
        button(
            "↩️ Torna alla casa scelta",
            &format!("loc:item:home:{item_id}:{}", room.home_id),
        ),
        button("🏠 Menu principale", "menu:main"),
    ]);

    let action = if is_move {
        "🚚 Sposta oggetto"
    } else {
        "🏠 Assegna luogo"
    };

    bot.send_message(
        chat_id,
        format!(
            "{action} #{item_id}\n\nPosizione attuale: {current}\nDestinazione scelta: 🏠 {} / 🚪 {}\n\nPuoi fermarti nella stanza oppure entrare in uno dei suoi contenitori.",
            room.home_name, room.name
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_container_destination_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    item_id: i64,
    container_id: i64,
) -> ResponseResult<()> {
    let Some(container) = crate::modules::contenitori::get_container(pool, container_id)
        .await
        .ok()
        .flatten()
    else {
        bot.send_message(chat_id, format!("Contenitore #{container_id} non trovato."))
            .reply_markup(locations_menu_keyboard())
            .await?;
        return Ok(());
    };

    let Some(path) = crate::modules::contenitori::container_path(pool, container_id)
        .await
        .ok()
        .flatten()
    else {
        bot.send_message(
            chat_id,
            "⚠️ Non riesco a ricostruire il percorso del contenitore.",
        )
        .await?;
        return Ok(());
    };

    let children = crate::modules::contenitori::list_container_children(pool, container_id)
        .await
        .unwrap_or_default();
    let current_location = get_item_location(pool, item_id).await.unwrap_or(None);
    let is_move = has_structured_location(current_location.as_ref());
    let current = format_location_full(pool, current_location.as_ref()).await;
    let exact_container = current_location
        .as_ref()
        .and_then(|location| location.container_id)
        == Some(container_id);

    let set_label = if is_move && exact_container {
        "🚚 Sposta qui (contenitore) · Attualmente qui"
    } else if is_move {
        "🚚 Sposta qui (contenitore)"
    } else {
        "📦 Assegna a questo contenitore"
    };

    let mut rows = vec![vec![button(
        set_label,
        &format!("loc:item:setcontainer:{item_id}:{container_id}"),
    )]];

    for child in children.iter().take(25) {
        let child_is_current = current_location
            .as_ref()
            .and_then(|location| location.container_id)
            == Some(child.id);
        let label = if is_move && child_is_current {
            format!("📦 {} · Attualmente qui", truncate_chars(&child.name, 28))
        } else {
            format!("📦 {}", truncate_chars(&child.name, 40))
        };
        rows.push(vec![button(
            &label,
            &format!("loc:item:container:{item_id}:{}", child.id),
        )]);
    }

    let back_callback = match container.parent_id {
        Some(parent_id) => format!("loc:item:container:{item_id}:{parent_id}"),
        None => match container.room_id {
            Some(room_id) => format!("loc:item:room:{item_id}:{room_id}"),
            None => format!("loc:item:home:{item_id}:{}", container.home_id),
        },
    };
    rows.push(vec![
        button("↩️ Livello precedente", &back_callback),
        button("🏠 Menu principale", "menu:main"),
    ]);

    let action = if is_move {
        "🚚 Sposta oggetto"
    } else {
        "🏠 Assegna luogo"
    };

    bot.send_message(
        chat_id,
        format!(
            "{action} #{item_id}\n\nPosizione attuale: {current}\nDestinazione scelta: 📦 {}\n\nPuoi spostare qui l'oggetto oppure entrare in un sottocontenitore.",
            crate::modules::contenitori::format_path_for_ui(&path)
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
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
        pool,
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
        pool,
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
    pool: &SqlitePool,
    title: String,
    total: i64,
    objects: &[LocatedObjectSummary],
    back_callback: String,
) -> ResponseResult<()> {
    if objects.is_empty() {
        bot.send_message(chat_id, format!("🏷️ Nessun oggetto in:\n{title}"))
            .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                button("↩️ Indietro", &back_callback),
                button("🏠 Menu principale", "menu:main"),
            ]]))
            .await?;
        return Ok(());
    }

    let mut text = format!("🏷️ Oggetti in:\n{title}\n\n");
    for object in objects {
        text.push_str(&format!("#{} · {}", object.id, object.name));
        if let Some((location, command)) = summary_location(pool, object).await {
            text.push_str(&format!("\n📍 {location}\n{command}"));
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

#[cfg(test)]
fn format_location(location: Option<&ItemLocation>) -> String {
    format_location_with_space(location, false)
}

fn format_location_with_space(location: Option<&ItemLocation>, show_space: bool) -> String {
    match location {
        Some(location) => match (&location.home_name, &location.room_name) {
            (Some(home), Some(room)) => {
                let home =
                    location_home_label(home, location.home_space_name.as_deref(), show_space);
                format!("🏠 {home} / 🚪 {room}")
            }
            (Some(home), None) => {
                let home =
                    location_home_label(home, location.home_space_name.as_deref(), show_space);
                format!("🏠 {home}")
            }
            _ => "Nessun luogo strutturato".to_string(),
        },
        None => "Nessun luogo strutturato".to_string(),
    }
}

fn location_home_label(home: &str, space: Option<&str>, show_space: bool) -> String {
    if show_space {
        if let Some(space) = space {
            return format!("{home} · {space}");
        }
    }
    home.to_string()
}

async fn format_location_full(pool: &SqlitePool, location: Option<&ItemLocation>) -> String {
    format_location_full_mode(pool, location, crate::identity::current_view_all()).await
}

async fn format_location_full_mode(
    pool: &SqlitePool,
    location: Option<&ItemLocation>,
    show_space: bool,
) -> String {
    let Some(location) = location else {
        return "Nessun luogo strutturato".to_string();
    };

    if let Some(container_id) = location.container_id {
        if let Ok(Some(path)) =
            crate::modules::contenitori::container_path(pool, container_id).await
        {
            return crate::modules::contenitori::format_path_for_ui_with_space(&path, show_space);
        }
    }

    format_location_with_space(Some(location), show_space)
}

async fn location_change_message_full(
    pool: &SqlitePool,
    previous: Option<&ItemLocation>,
    current: Option<&ItemLocation>,
) -> String {
    // Nei messaggi di modifica del luogo mostriamo sempre lo spazio: due case
    // possono avere lo stesso nome e uno spostamento cross-space deve essere inequivocabile.
    let mut before = format_location_full_mode(pool, previous, true).await;
    let mut after = format_location_full_mode(pool, current, true).await;
    let had_location = has_structured_location(previous);
    let has_location = has_structured_location(current);

    // I percorsi che terminano in un contenitore non includono l'emoji della casa.
    // Nei messaggi di spostamento la aggiungiamo qui per rendere Da/A coerenti.
    if had_location && !before.starts_with("🏠 ") {
        before = format!("🏠 {before}");
    }
    if has_location && !after.starts_with("🏠 ") {
        after = format!("🏠 {after}");
    }
    let unchanged = previous == current;

    match (had_location, has_location, unchanged) {
        (false, false, _) => {
            "ℹ️ L'oggetto non ha un luogo strutturato. Nessuna modifica effettuata.".to_string()
        }
        (false, true, _) => format!("✅ Luogo assegnato all'oggetto.\n\nNuovo luogo: {after}"),
        (true, false, _) => format!("🧹 Luogo rimosso dall'oggetto.\n\nPrima: {before}"),
        (true, true, true) => {
            format!("ℹ️ L'oggetto è già in:\n{after}\n\nNessuno spostamento effettuato.")
        }
        (true, true, false) => format!("🚚 Oggetto spostato.\n\nDa: {before}\nA: {after}"),
    }
}

fn has_structured_location(location: Option<&ItemLocation>) -> bool {
    location
        .and_then(|location| location.home_name.as_ref())
        .is_some()
}

#[cfg(test)]
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

async fn summary_location(
    pool: &SqlitePool,
    object: &LocatedObjectSummary,
) -> Option<(String, String)> {
    if let Some(container_id) = object.container_id {
        if let Ok(Some(path)) =
            crate::modules::contenitori::container_path(pool, container_id).await
        {
            return Some((
                crate::modules::contenitori::format_path_for_ui(&path),
                format!("/luogo_c{container_id}"),
            ));
        }
    }
    if let (Some(room_id), Some(home), Some(room)) = (
        object.room_id,
        object.home_name.as_deref(),
        object.room_name.as_deref(),
    ) {
        return Some((format!("{home} / {room}"), format!("/luogo_r{room_id}")));
    }
    if let (Some(home_id), Some(home)) = (object.home_id, object.home_name.as_deref()) {
        return Some((home.to_string(), format!("/luogo_h{home_id}")));
    }
    None
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
    container_id: Option<i64>,
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

    let (container_history_id, container_path) = if let Some(container_id) = container_id {
        let Some((container_home_id, container_room_id, container_name)) =
            sqlx::query_as::<_, (i64, Option<i64>, String)>(
                "SELECT abitazione_id, stanza_id, nome FROM contenitori WHERE id = ?",
            )
            .bind(container_id)
            .fetch_optional(&mut **tx)
            .await?
        else {
            return Err(sqlx::Error::RowNotFound);
        };

        if container_home_id != home_id || container_room_id != room_id {
            return Err(sqlx::Error::RowNotFound);
        }

        let history_id = crate::modules::storico::ensure_entity(
            tx,
            "contenitore",
            container_id,
            &container_name,
        )
        .await?;

        let path_parts: Vec<String> = sqlx::query_scalar(
            "WITH RECURSIVE chain(id, nome, contenitore_padre_id, depth) AS (                 SELECT id, nome, contenitore_padre_id, 0 FROM contenitori WHERE id = ?                 UNION ALL                 SELECT c.id, c.nome, c.contenitore_padre_id, chain.depth + 1                 FROM contenitori c JOIN chain ON chain.contenitore_padre_id = c.id              ) SELECT nome FROM chain ORDER BY depth DESC",
        )
        .bind(container_id)
        .fetch_all(&mut **tx)
        .await?;

        (Some(history_id), Some(path_parts.join(" / ")))
    } else {
        (None, None)
    };

    Ok(crate::modules::storico::LocationSnapshot {
        abitazione_storico_id: Some(home_history_id),
        abitazione_nome: Some(home_name),
        stanza_storico_id: room_history_id,
        stanza_nome: room_name,
        contenitore_storico_id: container_history_id,
        contenitore_percorso: container_path,
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
    history_location_snapshot(
        tx,
        location.home_id,
        location.room_id,
        location.container_id,
    )
    .await
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

pub(crate) async fn record_item_location_event(
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

    crate::modules::storico::record_event_location_context(tx, event_id, context).await?;
    crate::modules::storico::record_location_change(tx, event_id, before, after).await
}

async fn create_home(pool: &SqlitePool, name: &str) -> Result<i64, sqlx::Error> {
    crate::identity::ensure_can_write_sqlx(pool).await?;
    let space_id = crate::identity::current_space_id();
    let mut tx = pool.begin().await?;
    let result = sqlx::query("INSERT INTO abitazioni (nome, spazio_id) VALUES (?, ?)")
        .bind(name)
        .bind(space_id)
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
    let Some(space_id) = visible_home_space_id(pool, id).await? else {
        return Ok(false);
    };
    crate::identity::ensure_can_write_space_sqlx(pool, space_id).await?;
    let mut tx = pool.begin().await?;
    let Some(old_name) = sqlx::query_scalar::<_, String>(
        "SELECT nome FROM abitazioni WHERE id = ? AND spazio_id = ?",
    )
    .bind(id)
    .bind(space_id)
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
        "UPDATE abitazioni SET nome = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND spazio_id = ?",
    )
    .bind(name)
    .bind(id)
    .bind(space_id)
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
    let Some(space_id) = visible_home_space_id(pool, id).await? else {
        return Ok(false);
    };
    crate::identity::ensure_can_write_space_sqlx(pool, space_id).await?;
    let mut tx = pool.begin().await?;
    let Some(home_name) = sqlx::query_scalar::<_, String>(
        "SELECT nome FROM abitazioni WHERE id = ? AND spazio_id = ?",
    )
    .bind(id)
    .bind(space_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };

    let rooms: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, nome FROM stanze WHERE abitazione_id = ? ORDER BY id")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;

    let containers: Vec<(i64, Option<i64>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, stanza_id, nome, descrizione \
         FROM contenitori WHERE abitazione_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let mut container_before = Vec::with_capacity(containers.len());
    for (container_id, room_id, container_name, description) in &containers {
        container_before.push((
            *container_id,
            *room_id,
            container_name.clone(),
            description.clone(),
            history_location_snapshot(&mut tx, id, *room_id, Some(*container_id)).await?,
        ));
    }

    let affected_items: Vec<i64> = sqlx::query_scalar(
        "SELECT item_id FROM item_luogo WHERE abitazione_id = ? ORDER BY item_id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let mut item_before = Vec::with_capacity(affected_items.len());
    for item_id in &affected_items {
        item_before.push((
            *item_id,
            history_item_location_snapshot(&mut tx, *item_id).await?,
        ));
    }

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

    let mut room_events = HashMap::new();
    for (room_id, room_name) in &rooms {
        let room_history_id =
            crate::modules::storico::ensure_entity(&mut tx, "stanza", *room_id, room_name).await?;
        let room_event_id = crate::modules::storico::record_event(
            &mut tx,
            &crate::modules::storico::NewHistoryEvent {
                entita_storico_id: room_history_id,
                modulo: "luoghi",
                componente: "stanze",
                operazione: "eliminazione",
                nome_entita_snapshot: room_name,
                abitazione_storico_id: Some(home_history_id),
                abitazione_nome_snapshot: Some(&home_name),
                stanza_storico_id: Some(room_history_id),
                stanza_nome_snapshot: Some(room_name),
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
                valore_prima: Some(room_name.clone()),
                valore_dopo: None,
            }],
        )
        .await?;
        room_events.insert(*room_id, (room_history_id, room_event_id));
    }

    for (container_id, room_id, container_name, description, before) in container_before {
        let mut changes = vec![crate::modules::storico::NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: Some(container_name.clone()),
            valore_dopo: None,
        }];
        if let Some(description) = description {
            changes.push(crate::modules::storico::NewFieldChange {
                campo: "descrizione",
                tipo_valore: "testo",
                valore_prima: Some(description),
                valore_dopo: None,
            });
        }
        let parent_event_id = room_id
            .and_then(|room_id| room_events.get(&room_id).map(|(_, event_id)| *event_id))
            .unwrap_or(home_event_id);
        crate::modules::contenitori::record_container_history_event(
            &mut tx,
            container_id,
            &container_name,
            "eliminazione",
            &before,
            &crate::modules::storico::LocationSnapshot::default(),
            &changes,
            Some(parent_event_id),
        )
        .await?;
        if let Some(history_id) = before.contenitore_storico_id {
            crate::modules::storico::mark_entity_deleted(&mut tx, history_id).await?;
        }
    }

    for (item_id, before) in item_before {
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

    for (room_history_id, _) in room_events.values() {
        crate::modules::storico::mark_entity_deleted(&mut tx, *room_history_id).await?;
    }
    crate::modules::storico::mark_entity_deleted(&mut tx, home_history_id).await?;

    let result = sqlx::query("DELETE FROM abitazioni WHERE id = ? AND spazio_id = ?")
        .bind(id)
        .bind(space_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

async fn get_home(pool: &SqlitePool, id: i64) -> Result<Option<HomeRecord>, sqlx::Error> {
    let sql = format!(
        "SELECT a.id, CASE WHEN ? = 1 THEN a.nome || ' · ' || sp.nome ELSE a.nome END AS name \
         FROM abitazioni a JOIN spazi sp ON sp.id = a.spazio_id \
         WHERE a.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    sqlx::query_as::<_, HomeRecord>(&sql)
        .bind(if crate::identity::current_view_all() {
            1_i64
        } else {
            0_i64
        })
        .bind(id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await
}

async fn home_exists(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*) FROM abitazioni a WHERE a.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    let count: i64 = sqlx::query_scalar(&sql)
        .bind(id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await?;
    Ok(count == 1)
}

async fn list_homes(pool: &SqlitePool) -> Result<Vec<HomeRecord>, sqlx::Error> {
    let sql = format!(
        "SELECT a.id, CASE WHEN ? = 1 THEN a.nome || ' · ' || sp.nome ELSE a.nome END AS name \
         FROM abitazioni a JOIN spazi sp ON sp.id = a.spazio_id \
         WHERE {} ORDER BY sp.nome COLLATE NOCASE, a.nome COLLATE NOCASE, a.id",
        crate::identity::visible_space_sql("a")
    );
    sqlx::query_as::<_, HomeRecord>(&sql)
        .bind(if crate::identity::current_view_all() {
            1_i64
        } else {
            0_i64
        })
        .bind(crate::identity::visible_space_bind_id())
        .fetch_all(pool)
        .await
}

async fn visible_home_space_id(
    pool: &SqlitePool,
    home_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    let sql = format!(
        "SELECT a.spazio_id FROM abitazioni a WHERE a.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    sqlx::query_scalar(&sql)
        .bind(home_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await
}

async fn visible_item_space_id(
    pool: &SqlitePool,
    item_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    let sql = format!(
        "SELECT i.spazio_id FROM items i WHERE i.id = ? AND {}",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_scalar(&sql)
        .bind(item_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await
}

pub(crate) async fn ensure_location_target_writable(
    pool: &SqlitePool,
    home_id: Option<i64>,
    container_id: Option<i64>,
) -> anyhow::Result<()> {
    let target_space = if let Some(container_id) = container_id {
        let sql = format!(
            "SELECT a.spazio_id FROM contenitori c JOIN abitazioni a ON a.id = c.abitazione_id \
             WHERE c.id = ? AND {}",
            crate::identity::visible_space_sql("a")
        );
        sqlx::query_scalar::<_, i64>(&sql)
            .bind(container_id)
            .bind(crate::identity::visible_space_bind_id())
            .fetch_optional(pool)
            .await?
    } else if let Some(home_id) = home_id {
        visible_home_space_id(pool, home_id).await?
    } else {
        None
    };
    if let Some(space_id) = target_space {
        crate::identity::ensure_can_write_space(pool, space_id).await?;
        Ok(())
    } else if home_id.is_none() && container_id.is_none() {
        Ok(())
    } else {
        anyhow::bail!("Luogo di destinazione non accessibile")
    }
}

async fn create_room(pool: &SqlitePool, home_id: i64, name: &str) -> Result<i64, sqlx::Error> {
    let home_space = visible_home_space_id(pool, home_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    crate::identity::ensure_can_write_space_sqlx(pool, home_space).await?;
    let mut tx = pool.begin().await?;
    let home_name: String =
        sqlx::query_scalar("SELECT nome FROM abitazioni WHERE id = ? AND spazio_id = ?")
            .bind(home_id)
            .bind(home_space)
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
    let room_sql = format!(
        "SELECT a.spazio_id FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id WHERE s.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    let Some(space_id) = sqlx::query_scalar::<_, i64>(&room_sql)
        .bind(id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await?
    else {
        return Ok(false);
    };
    crate::identity::ensure_can_write_space_sqlx(pool, space_id).await?;
    let mut tx = pool.begin().await?;
    let Some((home_id, old_name, home_name)) = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT s.abitazione_id, s.nome, a.nome \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.id = ? AND a.spazio_id = ?",
    )
    .bind(id)
    .bind(space_id)
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
        "UPDATE stanze SET nome = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')          WHERE id = ? AND EXISTS (             SELECT 1 FROM abitazioni a WHERE a.id = stanze.abitazione_id AND a.spazio_id = ?         )",
    )
    .bind(name)
    .bind(id)
    .bind(space_id)
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
    let room_sql = format!(
        "SELECT a.spazio_id FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id WHERE s.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    let Some(space_id) = sqlx::query_scalar::<_, i64>(&room_sql)
        .bind(id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await?
    else {
        return Ok(false);
    };
    crate::identity::ensure_can_write_space_sqlx(pool, space_id).await?;
    let mut tx = pool.begin().await?;
    let Some((home_id, room_name, home_name)) = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT s.abitazione_id, s.nome, a.nome \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.id = ? AND a.spazio_id = ?",
    )
    .bind(id)
    .bind(space_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };

    let room_before = history_location_snapshot(&mut tx, home_id, Some(id), None).await?;
    let home_after = history_location_snapshot(&mut tx, home_id, None, None).await?;
    let home_history_id = home_after
        .abitazione_storico_id
        .expect("la casa esiste durante l'eliminazione della stanza");
    let room_history_id = room_before
        .stanza_storico_id
        .expect("la stanza esiste durante la propria eliminazione");

    let affected_containers: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, nome FROM contenitori WHERE stanza_id = ? ORDER BY id")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    let mut container_before = Vec::with_capacity(affected_containers.len());
    for (container_id, container_name) in &affected_containers {
        container_before.push((
            *container_id,
            container_name.clone(),
            history_location_snapshot(&mut tx, home_id, Some(id), Some(*container_id)).await?,
        ));
    }

    let affected_items: Vec<i64> =
        sqlx::query_scalar("SELECT item_id FROM item_luogo WHERE stanza_id = ? ORDER BY item_id")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    let mut item_before = Vec::with_capacity(affected_items.len());
    for item_id in &affected_items {
        item_before.push((
            *item_id,
            history_item_location_snapshot(&mut tx, *item_id).await?,
        ));
    }

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

    // I contenitori sono posizioni, non dati usa-e-getta. Eliminando una
    // stanza vengono promossi alla casa e la gerarchia padre/figlio resta
    // intatta. Gli effetti vengono tracciati come eventi figli.
    sqlx::query(
        "UPDATE contenitori \
         SET stanza_id = NULL, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE stanza_id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    crate::modules::storico::mark_entity_deleted(&mut tx, room_history_id).await?;
    let result = sqlx::query(
        "DELETE FROM stanze WHERE id = ? AND EXISTS (             SELECT 1 FROM abitazioni a WHERE a.id = stanze.abitazione_id AND a.spazio_id = ?         )",
    )
    .bind(id)
    .bind(space_id)
    .execute(&mut *tx)
        .await?;
    if result.rows_affected() != 1 {
        return Ok(false);
    }

    for (container_id, container_name, before) in container_before {
        let after = history_location_snapshot(&mut tx, home_id, None, Some(container_id)).await?;
        if before != after {
            crate::modules::contenitori::record_container_history_event(
                &mut tx,
                container_id,
                &container_name,
                "spostamento",
                &before,
                &after,
                &[],
                Some(room_event_id),
            )
            .await?;
        }
    }

    for (item_id, before) in item_before {
        let after = history_item_location_snapshot(&mut tx, item_id).await?;
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

    tx.commit().await?;
    Ok(true)
}

async fn get_room(pool: &SqlitePool, id: i64) -> Result<Option<RoomRecord>, sqlx::Error> {
    let sql = format!(
        "SELECT s.id AS id, s.abitazione_id AS home_id, s.nome AS name, \
                CASE WHEN ? = 1 THEN a.nome || ' · ' || sp.nome ELSE a.nome END AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id JOIN spazi sp ON sp.id = a.spazio_id \
         WHERE s.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    sqlx::query_as::<_, RoomRecord>(&sql)
        .bind(if crate::identity::current_view_all() {
            1_i64
        } else {
            0_i64
        })
        .bind(id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await
}

async fn list_rooms_for_home(
    pool: &SqlitePool,
    home_id: i64,
) -> Result<Vec<RoomRecord>, sqlx::Error> {
    let sql = format!(
        "SELECT s.id AS id, s.abitazione_id AS home_id, s.nome AS name, \
                CASE WHEN ? = 1 THEN a.nome || ' · ' || sp.nome ELSE a.nome END AS home_name \
         FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id JOIN spazi sp ON sp.id = a.spazio_id \
         WHERE s.abitazione_id = ? AND {} ORDER BY s.nome COLLATE NOCASE, s.id",
        crate::identity::visible_space_sql("a")
    );
    sqlx::query_as::<_, RoomRecord>(&sql)
        .bind(if crate::identity::current_view_all() {
            1_i64
        } else {
            0_i64
        })
        .bind(home_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_all(pool)
        .await
}

async fn count_rooms_for_home(pool: &SqlitePool, home_id: i64) -> Result<i64, sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*) FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.abitazione_id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    sqlx::query_scalar(&sql)
        .bind(home_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await
}

async fn count_items_for_home(pool: &SqlitePool, home_id: i64) -> Result<i64, sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*) FROM item_luogo il JOIN items i ON i.id = il.item_id \
         WHERE il.abitazione_id = ? AND {}",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_scalar(&sql)
        .bind(home_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await
}

async fn count_items_for_room(pool: &SqlitePool, room_id: i64) -> Result<i64, sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*) FROM item_luogo il JOIN items i ON i.id = il.item_id \
         WHERE il.stanza_id = ? AND {}",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_scalar(&sql)
        .bind(room_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await
}

async fn count_containers_for_room(pool: &SqlitePool, room_id: i64) -> Result<i64, sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*) FROM contenitori c JOIN abitazioni a ON a.id = c.abitazione_id \
         WHERE c.stanza_id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    sqlx::query_scalar(&sql)
        .bind(room_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await
}

async fn object_exists(pool: &SqlitePool, item_id: i64) -> Result<bool, sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*) FROM items i WHERE i.id = ? AND i.tipo = 'oggetto' AND {}",
        crate::identity::visible_space_sql("i")
    );
    let count: i64 = sqlx::query_scalar(&sql)
        .bind(item_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await?;
    Ok(count == 1)
}

async fn get_item_location(
    pool: &SqlitePool,
    item_id: i64,
) -> Result<Option<ItemLocation>, sqlx::Error> {
    let sql = format!(
        "SELECT il.abitazione_id AS home_id, il.stanza_id AS room_id, il.contenitore_id AS container_id, \
                a.nome AS home_name, ps.nome AS home_space_name, s.nome AS room_name \
         FROM item_luogo il JOIN items i ON i.id = il.item_id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN spazi ps ON ps.id = a.spazio_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE il.item_id = ? AND {}",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_as::<_, ItemLocation>(&sql)
        .bind(item_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(pool)
        .await
}

async fn get_item_location_tx(
    tx: &mut Transaction<'_, Sqlite>,
    item_id: i64,
) -> Result<Option<ItemLocation>, sqlx::Error> {
    let sql = format!(
        "SELECT il.abitazione_id AS home_id, il.stanza_id AS room_id, il.contenitore_id AS container_id, \
                a.nome AS home_name, ps.nome AS home_space_name, s.nome AS room_name \
         FROM item_luogo il JOIN items i ON i.id = il.item_id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN spazi ps ON ps.id = a.spazio_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE il.item_id = ? AND {}",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_as::<_, ItemLocation>(&sql)
        .bind(item_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_optional(&mut **tx)
        .await
}

async fn set_item_home(pool: &SqlitePool, item_id: i64, home_id: i64) -> Result<(), sqlx::Error> {
    let item_space = visible_item_space_id(pool, item_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let home_space = visible_home_space_id(pool, home_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    crate::identity::ensure_can_write_space_sqlx(pool, item_space).await?;
    crate::identity::ensure_can_write_space_sqlx(pool, home_space).await?;
    let mut tx = pool.begin().await?;
    let previous = get_item_location_tx(&mut tx, item_id).await?;
    if previous.as_ref().is_some_and(|location| {
        location.home_id == home_id && location.room_id.is_none() && location.container_id.is_none()
    }) {
        return Ok(());
    }
    let before = if let Some(previous) = previous.as_ref() {
        history_snapshot_from_item_location(&mut tx, previous).await?
    } else {
        crate::modules::storico::LocationSnapshot::default()
    };
    let after = history_location_snapshot(&mut tx, home_id, None, None).await?;
    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id) VALUES (?, ?, NULL, NULL) \
         ON CONFLICT(item_id) DO UPDATE SET abitazione_id = excluded.abitazione_id, stanza_id = NULL, contenitore_id = NULL",
    ).bind(item_id).bind(home_id).execute(&mut *tx).await?;
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
    let item_space = visible_item_space_id(pool, item_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let room_sql = format!(
        "SELECT s.abitazione_id, a.spazio_id FROM stanze s JOIN abitazioni a ON a.id = s.abitazione_id \
         WHERE s.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    let (home_id, home_space): (i64, i64) = sqlx::query_as(&room_sql)
        .bind(room_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await?;
    crate::identity::ensure_can_write_space_sqlx(pool, item_space).await?;
    crate::identity::ensure_can_write_space_sqlx(pool, home_space).await?;
    let mut tx = pool.begin().await?;
    let previous = get_item_location_tx(&mut tx, item_id).await?;
    if previous.as_ref().is_some_and(|location| {
        location.home_id == home_id
            && location.room_id == Some(room_id)
            && location.container_id.is_none()
    }) {
        return Ok(());
    }
    let before = if let Some(previous) = previous.as_ref() {
        history_snapshot_from_item_location(&mut tx, previous).await?
    } else {
        crate::modules::storico::LocationSnapshot::default()
    };
    let after = history_location_snapshot(&mut tx, home_id, Some(room_id), None).await?;
    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id) VALUES (?, ?, ?, NULL) \
         ON CONFLICT(item_id) DO UPDATE SET abitazione_id = excluded.abitazione_id, stanza_id = excluded.stanza_id, contenitore_id = NULL",
    ).bind(item_id).bind(home_id).bind(room_id).execute(&mut *tx).await?;
    let operation = if previous.is_some() {
        "spostamento"
    } else {
        "assegnazione"
    };
    record_item_location_event(&mut tx, item_id, operation, &before, &after, None).await?;
    tx.commit().await?;
    Ok(())
}

async fn set_item_container(
    pool: &SqlitePool,
    item_id: i64,
    container_id: i64,
) -> Result<(), sqlx::Error> {
    let item_space = visible_item_space_id(pool, item_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let target_sql = format!(
        "SELECT c.abitazione_id, c.stanza_id, a.spazio_id FROM contenitori c \
         JOIN abitazioni a ON a.id = c.abitazione_id WHERE c.id = ? AND {}",
        crate::identity::visible_space_sql("a")
    );
    let (home_id, room_id, home_space): (i64, Option<i64>, i64) = sqlx::query_as(&target_sql)
        .bind(container_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await?;
    crate::identity::ensure_can_write_space_sqlx(pool, item_space).await?;
    crate::identity::ensure_can_write_space_sqlx(pool, home_space).await?;
    let mut tx = pool.begin().await?;
    let previous = get_item_location_tx(&mut tx, item_id).await?;
    if previous
        .as_ref()
        .is_some_and(|location| location.container_id == Some(container_id))
    {
        return Ok(());
    }
    let before = if let Some(previous) = previous.as_ref() {
        history_snapshot_from_item_location(&mut tx, previous).await?
    } else {
        crate::modules::storico::LocationSnapshot::default()
    };
    let after = history_location_snapshot(&mut tx, home_id, room_id, Some(container_id)).await?;
    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id) VALUES (?, ?, ?, ?) \
         ON CONFLICT(item_id) DO UPDATE SET abitazione_id = excluded.abitazione_id, stanza_id = excluded.stanza_id, contenitore_id = excluded.contenitore_id",
    ).bind(item_id).bind(home_id).bind(room_id).bind(container_id).execute(&mut *tx).await?;
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
    let item_space = visible_item_space_id(pool, item_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    crate::identity::ensure_can_write_space_sqlx(pool, item_space).await?;
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
    let sql = format!(
        "SELECT COUNT(*) FROM items i JOIN item_luogo il ON il.item_id = i.id \
         WHERE i.tipo = 'oggetto' AND il.abitazione_id = ? AND {}",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_scalar(&sql)
        .bind(home_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await
}

async fn count_objects_for_room(pool: &SqlitePool, room_id: i64) -> Result<i64, sqlx::Error> {
    let sql = format!(
        "SELECT COUNT(*) FROM items i JOIN item_luogo il ON il.item_id = i.id \
         WHERE i.tipo = 'oggetto' AND il.stanza_id = ? AND {}",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_scalar(&sql)
        .bind(room_id)
        .bind(crate::identity::visible_space_bind_id())
        .fetch_one(pool)
        .await
}

async fn list_objects_for_home(
    pool: &SqlitePool,
    home_id: i64,
    limit: i64,
) -> Result<Vec<LocatedObjectSummary>, sqlx::Error> {
    let sql = format!(
        "SELECT i.id AS id, i.nome AS name, \
                il.abitazione_id AS home_id, a.nome AS home_name, \
                il.stanza_id AS room_id, s.nome AS room_name, il.contenitore_id AS container_id \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         JOIN item_luogo il ON il.item_id = i.id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE i.tipo = 'oggetto' AND il.abitazione_id = ? AND {} \
         ORDER BY i.nome COLLATE NOCASE, i.id LIMIT ?",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_as::<_, LocatedObjectSummary>(&sql)
        .bind(home_id)
        .bind(crate::identity::visible_space_bind_id())
        .bind(limit)
        .fetch_all(pool)
        .await
}

async fn list_objects_for_room(
    pool: &SqlitePool,
    room_id: i64,
    limit: i64,
) -> Result<Vec<LocatedObjectSummary>, sqlx::Error> {
    let sql = format!(
        "SELECT i.id AS id, i.nome AS name, \
                il.abitazione_id AS home_id, a.nome AS home_name, \
                il.stanza_id AS room_id, s.nome AS room_name, il.contenitore_id AS container_id \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         JOIN item_luogo il ON il.item_id = i.id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE i.tipo = 'oggetto' AND il.stanza_id = ? AND {} \
         ORDER BY i.nome COLLATE NOCASE, i.id LIMIT ?",
        crate::identity::visible_space_sql("i")
    );
    sqlx::query_as::<_, LocatedObjectSummary>(&sql)
        .bind(room_id)
        .bind(crate::identity::visible_space_bind_id())
        .bind(limit)
        .fetch_all(pool)
        .await
}

fn locations_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("🏠 Case", "loc:home:list"),
            button("🚪 Stanze", "loc:room:list"),
        ],
        vec![
            button("📦 Contenitori", "c:a"),
            button("🌳 Struttura", "loc:tree"),
        ],
        vec![button("➕ Crea…", "loc:create")],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

pub(crate) fn location_navigation_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("↩️ Case, stanze e contenitori", "loc:menu"),
        button("🏠 Menu principale", "menu:main"),
    ]])
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
    rows.push(vec![button("➕🏠 Casa", "loc:home:new")]);
    rows.push(vec![
        button("↩️ Case, stanze e contenitori", "loc:menu"),
        button("🏠 Menu principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn home_detail_keyboard(home_id: i64, rooms: &[RoomRecord]) -> InlineKeyboardMarkup {
    let mut rows = rooms
        .iter()
        .take(20)
        .map(|room| {
            vec![button(
                &format!("🚪 {}", truncate_chars(&room.name, 42)),
                &format!("loc:room:{}", room.id),
            )]
        })
        .collect::<Vec<_>>();

    rows.push(vec![
        button("➕🚪 Stanza", &format!("loc:room:new:{home_id}")),
        button(
            "➕📦 Contenitore",
            &format!(
                "c:nl:{}:0",
                crate::modules::contenitori::encode_callback_id(home_id)
            ),
        ),
        button("➕🏷️ Oggetto", &format!("oggetti:newat:h:{home_id}")),
    ]);
    rows.push(vec![
        button(
            "📋📦 Contenitori qui",
            &format!(
                "c:lh:{}",
                crate::modules::contenitori::encode_callback_id(home_id)
            ),
        ),
        button("📋🏷️ Oggetti qui", &format!("loc:filter:home:{home_id}")),
    ]);
    rows.push(vec![button(
        "⚙️ Gestisci",
        &format!("loc:home:manage:{home_id}"),
    )]);
    rows.push(vec![
        button("↩️ Elenco case", "loc:home:list"),
        button("🏠 Menu principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn room_detail_keyboard(room: &RoomRecord) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button(
                "➕📦 Contenitore",
                &format!(
                    "c:nl:{}:{}",
                    crate::modules::contenitori::encode_callback_id(room.home_id),
                    crate::modules::contenitori::encode_callback_id(room.id)
                ),
            ),
            button("➕🏷️ Oggetto", &format!("oggetti:newat:r:{}", room.id)),
        ],
        vec![
            button(
                "📋📦 Contenitori qui",
                &format!(
                    "c:lr:{}",
                    crate::modules::contenitori::encode_callback_id(room.id)
                ),
            ),
            button("📋🏷️ Oggetti qui", &format!("loc:filter:room:{}", room.id)),
        ],
        vec![button(
            "⚙️ Gestisci",
            &format!("loc:room:manage:{}", room.id),
        )],
        vec![
            button("↩️ Torna alla casa", &format!("loc:home:{}", room.home_id)),
            button("🏠 Menu principale", "menu:main"),
        ],
    ])
}

fn home_delete_keyboard(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "🗑 Sì, elimina casa",
            &format!("loc:home:delete:do:{id}"),
        )],
        vec![
            button("↩️ Annulla", &format!("loc:home:{id}")),
            button("🏠 Menu principale", "menu:main"),
        ],
    ])
}

fn room_delete_keyboard(room: &RoomRecord) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "🗑 Sì, elimina stanza",
            &format!("loc:room:delete:do:{}", room.id),
        )],
        vec![
            button("↩️ Annulla", &format!("loc:room:{}", room.id)),
            button("🏠 Menu principale", "menu:main"),
        ],
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
    rows.push(vec![
        button("↩️ Scheda oggetto", &format!("oggetti:view:{item_id}")),
        button("🏠 Menu principale", "menu:main"),
    ]);
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
                &format!("🏷️ #{} · {}", object.id, truncate_chars(&object.name, 36)),
                &format!("oggetti:view:{}", object.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![
        button("↩️ Indietro", back_callback),
        button("🏠 Menu principale", "menu:main"),
    ]);
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

    #[test]
    fn callback_gestione_luoghi_restano_sotto_limite_telegram() {
        let home = format!("loc:home:manage:{}", i64::MAX);
        let room = format!("loc:room:manage:{}", i64::MAX);
        assert!(home.len() <= 64);
        assert!(room.len() <= 64);
        assert_eq!(parse_id_callback(&home, "loc:home:manage:"), Some(i64::MAX));
        assert_eq!(parse_id_callback(&room, "loc:room:manage:"), Some(i64::MAX));
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
    async fn oggetto_puo_spostarsi_da_stanza_a_contenitore_e_tornare_in_stanza() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Trapano").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");
        let root = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            None,
            "Scaffale",
            None,
        )
        .await
        .expect("contenitore");
        let child = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            Some(root),
            "Scatola",
            None,
        )
        .await
        .expect("sottocontenitore");

        set_item_room(&pool, item_id, room_id)
            .await
            .expect("stanza iniziale");
        set_item_container(&pool, item_id, root)
            .await
            .expect("spostamento nel contenitore");

        let in_root: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("luogo nel contenitore");
        assert_eq!(in_root, (home_id, Some(room_id), Some(root)));

        set_item_container(&pool, item_id, child)
            .await
            .expect("spostamento nel sottocontenitore");
        let in_child: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("luogo nel sottocontenitore");
        assert_eq!(in_child, (home_id, Some(room_id), Some(child)));

        set_item_room(&pool, item_id, room_id)
            .await
            .expect("ritorno diretto nella stanza");
        let in_room: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("luogo nella stanza");
        assert_eq!(in_room, (home_id, Some(room_id), None));
    }

    #[tokio::test]
    async fn spostare_da_contenitore_a_casa_rimuove_stanza_e_contenitore() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Valigia").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Camera").await.expect("stanza");
        let container_id = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            None,
            "Armadio",
            None,
        )
        .await
        .expect("contenitore");

        set_item_container(&pool, item_id, container_id)
            .await
            .expect("assegnazione contenitore");
        set_item_home(&pool, item_id, home_id)
            .await
            .expect("spostamento alla sola casa");

        let location: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("luogo finale");
        assert_eq!(location, (home_id, None, None));
    }

    #[tokio::test]
    async fn posizione_completa_include_contenitori_annidati() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Chiavi").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");
        let root = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            None,
            "Scaffale",
            None,
        )
        .await
        .expect("contenitore");
        let child = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            Some(root),
            "Scatola",
            None,
        )
        .await
        .expect("sottocontenitore");

        set_item_container(&pool, item_id, child)
            .await
            .expect("assegnazione contenitore");
        let location = get_item_location(&pool, item_id)
            .await
            .expect("lettura luogo")
            .expect("luogo presente");
        let formatted = format_location_full(&pool, Some(&location)).await;

        assert!(formatted.contains("Casa principale"));
        assert!(formatted.contains("Garage"));
        assert!(formatted.contains("Scaffale"));
        assert!(formatted.contains("Scatola"));
    }

    #[test]
    fn callback_picker_contenitori_restano_sotto_limite_telegram() {
        let max = i64::MAX;
        for callback in [
            format!("loc:item:room:{max}:{max}"),
            format!("loc:item:container:{max}:{max}"),
            format!("loc:item:setcontainer:{max}:{max}"),
        ] {
            assert!(callback.len() <= 64, "callback troppo lunga: {callback}");
        }
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
        let container_id = sqlx::query(
            "INSERT INTO contenitori (abitazione_id, stanza_id, nome) VALUES (?, ?, 'Armadio')",
        )
        .bind(home_id)
        .bind(room_id)
        .execute(&pool)
        .await
        .expect("contenitore")
        .last_insert_rowid();

        assert!(delete_room(&pool, room_id).await.expect("delete stanza"));
        let location: (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT abitazione_id, stanza_id FROM item_luogo WHERE item_id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .expect("riga luogo presente");
        assert_eq!(location, (Some(home_id), None));

        let container_location: (i64, Option<i64>) =
            sqlx::query_as("SELECT abitazione_id, stanza_id FROM contenitori WHERE id = ?")
                .bind(container_id)
                .fetch_one(&pool)
                .await
                .expect("contenitore mantenuto");
        assert_eq!(container_location, (home_id, None));

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

    #[tokio::test]
    async fn eliminare_stanza_storicizza_promozione_di_contenitori_e_oggetti() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Trapano").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");
        let root = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            None,
            "Scaffale",
            None,
        )
        .await
        .expect("root");
        let child = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            Some(root),
            "Scatola",
            None,
        )
        .await
        .expect("child");
        set_item_container(&pool, item_id, child)
            .await
            .expect("item nel contenitore");

        let room_history: i64 = sqlx::query_scalar(
            "SELECT id FROM storico_entita WHERE tipo_entita='stanza' AND id_origine=?",
        )
        .bind(room_id)
        .fetch_one(&pool)
        .await
        .expect("storico stanza");

        assert!(delete_room(&pool, room_id).await.expect("delete stanza"));

        let room_event: i64 = sqlx::query_scalar(
            "SELECT id FROM storico_eventi \
             WHERE entita_storico_id=? AND operazione='eliminazione' ORDER BY id DESC LIMIT 1",
        )
        .bind(room_history)
        .fetch_one(&pool)
        .await
        .expect("evento stanza");

        let child_history: i64 = sqlx::query_scalar(
            "SELECT id FROM storico_entita WHERE tipo_entita='contenitore' AND id_origine=?",
        )
        .bind(child)
        .fetch_one(&pool)
        .await
        .expect("storico child");
        let child_event: (i64, Option<i64>) = sqlx::query_as(
            "SELECT id, evento_padre_id FROM storico_eventi \
             WHERE entita_storico_id=? AND operazione='spostamento' ORDER BY id DESC LIMIT 1",
        )
        .bind(child_history)
        .fetch_one(&pool)
        .await
        .expect("evento child");
        assert_eq!(child_event.1, Some(room_event));

        let child_change: (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT stanza_prima_nome, contenitore_prima_percorso, \
                    stanza_dopo_nome, contenitore_dopo_percorso \
             FROM storico_cambi_luogo WHERE evento_id=?",
        )
        .bind(child_event.0)
        .fetch_one(&pool)
        .await
        .expect("cambio child");
        assert_eq!(child_change.0.as_deref(), Some("Garage"));
        assert_eq!(child_change.1.as_deref(), Some("Scaffale / Scatola"));
        assert_eq!(child_change.2, None);
        assert_eq!(child_change.3.as_deref(), Some("Scaffale / Scatola"));

        let item_history: i64 = sqlx::query_scalar(
            "SELECT id FROM storico_entita WHERE tipo_entita='oggetto' AND id_origine=?",
        )
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("storico item");
        let item_event: (i64, Option<i64>) = sqlx::query_as(
            "SELECT id, evento_padre_id FROM storico_eventi \
             WHERE entita_storico_id=? AND operazione='spostamento' ORDER BY id DESC LIMIT 1",
        )
        .bind(item_history)
        .fetch_one(&pool)
        .await
        .expect("evento item");
        assert_eq!(item_event.1, Some(room_event));
        let item_path_after: Option<String> = sqlx::query_scalar(
            "SELECT contenitore_dopo_percorso FROM storico_cambi_luogo WHERE evento_id=?",
        )
        .bind(item_event.0)
        .fetch_one(&pool)
        .await
        .expect("path item");
        assert_eq!(item_path_after.as_deref(), Some("Scaffale / Scatola"));
    }

    #[tokio::test]
    async fn eliminare_casa_storicizza_contenitori_e_percorso_oggetto() {
        let pool = test_pool().await;
        let item_id = create_test_object(&pool, "Trapano").await;
        let home_id = create_home(&pool, "Casa principale").await.expect("casa");
        let room_id = create_room(&pool, home_id, "Garage").await.expect("stanza");
        let root = crate::modules::contenitori::create_container(
            &pool,
            home_id,
            Some(room_id),
            None,
            "Scaffale",
            None,
        )
        .await
        .expect("contenitore");
        set_item_container(&pool, item_id, root)
            .await
            .expect("item nel contenitore");

        let container_history: i64 = sqlx::query_scalar(
            "SELECT id FROM storico_entita WHERE tipo_entita='contenitore' AND id_origine=?",
        )
        .bind(root)
        .fetch_one(&pool)
        .await
        .expect("storico contenitore");

        assert!(delete_home(&pool, home_id).await.expect("delete casa"));

        let deleted_at: Option<String> =
            sqlx::query_scalar("SELECT eliminato_il FROM storico_entita WHERE id=?")
                .bind(container_history)
                .fetch_one(&pool)
                .await
                .expect("contenitore storico");
        assert!(deleted_at.is_some());

        let item_history: i64 = sqlx::query_scalar(
            "SELECT id FROM storico_entita WHERE tipo_entita='oggetto' AND id_origine=?",
        )
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .expect("storico item");
        let removal_event: i64 = sqlx::query_scalar(
            "SELECT id FROM storico_eventi \
             WHERE entita_storico_id=? AND operazione='rimozione' ORDER BY id DESC LIMIT 1",
        )
        .bind(item_history)
        .fetch_one(&pool)
        .await
        .expect("evento rimozione");
        let before_path: Option<String> = sqlx::query_scalar(
            "SELECT contenitore_prima_percorso FROM storico_cambi_luogo WHERE evento_id=?",
        )
        .bind(removal_event)
        .fetch_one(&pool)
        .await
        .expect("path prima");
        assert_eq!(before_path.as_deref(), Some("Scaffale"));
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
            container_id: None,
            home_name: Some("Casa principale".to_string()),
            home_space_name: Some("Spazio principale".to_string()),
            room_name: Some("Garage".to_string()),
        };
        let camera = ItemLocation {
            home_id: 1,
            room_id: Some(11),
            container_id: None,
            home_name: Some("Casa principale".to_string()),
            home_space_name: Some("Casa condivisa".to_string()),
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

        let garage_con_spazio = format_location_with_space(Some(&garage), true);
        let camera_con_spazio = format_location_with_space(Some(&camera), true);
        assert!(garage_con_spazio.contains("Casa principale · Spazio principale"));
        assert!(camera_con_spazio.contains("Casa principale · Casa condivisa"));

        let invariato = location_change_message(Some(&camera), Some(&camera));
        assert!(invariato.contains("Nessuno spostamento effettuato"));

        let rimozione = location_change_message(Some(&camera), nessun_luogo.as_ref());
        assert!(rimozione.contains("Luogo rimosso"));
        assert!(rimozione.contains("Prima:"));
    }
    #[test]
    fn comando_luogo_tipizzato_apre_case_stanze_e_contenitori() {
        assert_eq!(parse_location_command("/luogo_h12"), Some(('h', 12)));
        assert_eq!(parse_location_command("/luogo_r7"), Some(('r', 7)));
        assert_eq!(parse_location_command("/luogo_c33"), Some(('c', 33)));
        assert_eq!(parse_location_command("/luogo_x1"), None);
        assert_eq!(parse_location_command("/luogo_h0"), None);
    }

    #[test]
    fn albero_luoghi_mantiene_gerarchia_stanza_e_contenitori() {
        let rooms = vec![RoomRecord {
            id: 2,
            home_id: 1,
            name: "Garage".to_string(),
            home_name: "Casa".to_string(),
        }];
        let containers = vec![
            TreeContainerRecord {
                id: 10,
                home_id: 1,
                room_id: Some(2),
                parent_id: None,
                name: "Armadio".to_string(),
            },
            TreeContainerRecord {
                id: 11,
                home_id: 1,
                room_id: Some(2),
                parent_id: Some(10),
                name: "Ripiano".to_string(),
            },
        ];

        let nodes = build_home_nodes(1, &rooms, &containers);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].command, "/luogo_r2");
        assert_eq!(nodes[0].children[0].command, "/luogo_c10");
        assert_eq!(nodes[0].children[0].children[0].command, "/luogo_c11");
    }

    #[test]
    fn annulla_nuova_stanza_ricorda_la_casa_di_partenza() {
        let state = LocationConversationState::AwaitingRoomName {
            home_id: 7,
            rename_id: None,
            return_to: LocationReturnTarget::Home(7),
        };
        assert!(matches!(
            location_return_target(&state),
            LocationReturnTarget::Home(7)
        ));
    }
    #[tokio::test]
    async fn case_con_lo_stesso_nome_restano_isolate_per_spazio() {
        let pool = test_pool().await;
        let space_two =
            sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Spazio due', 'personale')")
                .execute(&pool)
                .await
                .expect("spazio due")
                .last_insert_rowid();

        let actor_one = crate::identity::AuditActor::system();
        let actor_two = crate::identity::AuditActor {
            utente_id: None,
            nome_snapshot: "Sistema test".to_string(),
            spazio_id: space_two,
            spazio_nome_snapshot: "Spazio due".to_string(),
            view_all: false,
            origine: "sistema",
            telegram_user_id: None,
            telegram_username: None,
        };

        let home_one = crate::identity::with_actor(actor_one.clone(), async {
            create_home(&pool, "Casa principale")
                .await
                .expect("casa spazio uno")
        })
        .await;
        let home_two = crate::identity::with_actor(actor_two.clone(), async {
            create_home(&pool, "Casa principale")
                .await
                .expect("casa spazio due")
        })
        .await;
        assert_ne!(home_one, home_two);

        crate::identity::with_actor(actor_one, async {
            let homes = home_choices(&pool).await.expect("case spazio uno");
            assert_eq!(homes.len(), 1);
            assert_eq!(homes[0].id, home_one);
            assert!(home_choice(&pool, home_two)
                .await
                .expect("casa cross-space")
                .is_none());
        })
        .await;

        crate::identity::with_actor(actor_two, async {
            let homes = home_choices(&pool).await.expect("case spazio due");
            assert_eq!(homes.len(), 1);
            assert_eq!(homes[0].id, home_two);
            assert!(home_choice(&pool, home_one)
                .await
                .expect("casa cross-space inversa")
                .is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn vista_tutti_consente_oggetto_personale_in_casa_condivisa_senza_cambiarne_proprieta() {
        let pool = test_pool().await;
        let user_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Alessio')")
            .execute(&pool)
            .await
            .expect("utente")
            .last_insert_rowid();
        let space_two = sqlx::query(
            "INSERT INTO spazi (nome, tipo, creato_da_utente_id) VALUES ('Casa condivisa', 'condiviso', ?)",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("spazio due")
        .last_insert_rowid();
        let space_three = sqlx::query(
            "INSERT INTO spazi (nome, tipo) VALUES ('Spazio non accessibile', 'condiviso')",
        )
        .execute(&pool)
        .await
        .expect("spazio tre")
        .last_insert_rowid();

        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (1, ?, 'proprietario')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("membership personale");
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'membro')",
        )
        .bind(space_two)
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("membership condivisa");

        let personal_home =
            sqlx::query("INSERT INTO abitazioni (spazio_id, nome) VALUES (1, 'Casa principale')")
                .execute(&pool)
                .await
                .expect("casa personale")
                .last_insert_rowid();
        let shared_home =
            sqlx::query("INSERT INTO abitazioni (spazio_id, nome) VALUES (?, 'Casa principale')")
                .bind(space_two)
                .execute(&pool)
                .await
                .expect("casa condivisa")
                .last_insert_rowid();
        let forbidden_home =
            sqlx::query("INSERT INTO abitazioni (spazio_id, nome) VALUES (?, 'Casa terza')")
                .bind(space_three)
                .execute(&pool)
                .await
                .expect("casa non accessibile")
                .last_insert_rowid();
        let item_id = sqlx::query(
            "INSERT INTO items (tipo, nome, spazio_id) VALUES ('oggetto', 'Portatile', 1)",
        )
        .execute(&pool)
        .await
        .expect("item")
        .last_insert_rowid();
        sqlx::query("INSERT INTO oggetti (item_id) VALUES (?)")
            .bind(item_id)
            .execute(&pool)
            .await
            .expect("oggetto");

        let actor = crate::identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: "Alessio".to_string(),
            spazio_id: 1,
            spazio_nome_snapshot: "Spazio principale".to_string(),
            view_all: true,
            origine: "telegram",
            telegram_user_id: None,
            telegram_username: None,
        };

        crate::identity::with_actor(actor.clone(), async {
            let homes = home_choices(&pool).await.expect("case visibili");
            assert!(homes.iter().any(|home| home.id == personal_home));
            assert!(homes.iter().any(|home| home.id == shared_home));
            assert!(!homes.iter().any(|home| home.id == forbidden_home));

            set_item_home(&pool, item_id, shared_home)
                .await
                .expect("spostamento cross-space consentito");
            let owner: i64 = sqlx::query_scalar("SELECT spazio_id FROM items WHERE id = ?")
                .bind(item_id)
                .fetch_one(&pool)
                .await
                .expect("proprietario");
            let location: i64 =
                sqlx::query_scalar("SELECT abitazione_id FROM item_luogo WHERE item_id = ?")
                    .bind(item_id)
                    .fetch_one(&pool)
                    .await
                    .expect("posizione");
            assert_eq!(owner, 1);
            assert_eq!(location, shared_home);

            assert!(set_item_home(&pool, item_id, forbidden_home).await.is_err());
        })
        .await;

        sqlx::query(
            "UPDATE membri_spazio SET ruolo = 'lettura' WHERE spazio_id = ? AND utente_id = ?",
        )
        .bind(space_two)
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("ruolo lettura");

        crate::identity::with_actor(actor, async {
            assert!(set_item_home(&pool, item_id, shared_home).await.is_err());
        })
        .await;
    }
}
