//! Modulo "oggetti generici".
//!
//! Step 6A: agli oggetti gia' completi di CRUD e foto si aggiunge una
//! posizione strutturata casa/stanza condivisa tramite `item_luogo`. Il campo
//! `posizione` resta un dettaglio libero (es. scaffale, cassetto, contenitore).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use sqlx::{sqlite::SqliteQueryResult, FromRow, Sqlite, SqlitePool, Transaction};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

const PAGE_SIZE: i64 = 8;
const MAX_SEARCH_RESULTS: i64 = 12;

#[derive(Clone, Default)]
pub struct SessionStore {
    inner: Arc<Mutex<HashMap<i64, ConversationState>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_chat(&self, chat_id: i64) {
        self.with_sessions(|sessions| {
            sessions.remove(&chat_id);
        });
    }

    fn get(&self, chat_id: i64) -> Option<ConversationState> {
        self.with_sessions(|sessions| sessions.get(&chat_id).cloned())
    }

    fn set(&self, chat_id: i64, state: ConversationState) {
        self.with_sessions(|sessions| {
            sessions.insert(chat_id, state);
        });
    }

    fn with_sessions<T>(&self, f: impl FnOnce(&mut HashMap<i64, ConversationState>) -> T) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

#[derive(Clone)]
enum ConversationState {
    AwaitingObjectName,
    EditingObject {
        draft: Box<ObjectDraft>,
        field: Option<DraftField>,
    },
    AwaitingSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DraftField {
    Name,
    Brand,
    Model,
    Position,
    PurchaseDate,
    PurchasePrice,
    Seller,
    Notes,
    Description,
    EstimatedValue,
    SerialNumber,
}

#[derive(Debug, Clone, Default)]
struct ObjectDraft {
    object_id: Option<i64>,
    name: String,
    description: Option<String>,
    brand: Option<String>,
    model: Option<String>,
    serial_number: Option<String>,
    position: Option<String>,
    home_id: Option<i64>,
    home_name: Option<String>,
    room_id: Option<i64>,
    room_name: Option<String>,
    purchase_date: Option<String>,
    purchase_price_cents: Option<i64>,
    seller: Option<String>,
    estimated_value_cents: Option<i64>,
    condition: Option<ObjectCondition>,
    notes: Option<String>,
}

impl ObjectDraft {
    fn new(name: &str) -> Option<Self> {
        let name = clean_required(name, 120)?;
        Some(Self {
            object_id: None,
            name,
            ..Self::default()
        })
    }

    fn from_record(record: &ObjectRecord) -> Self {
        Self {
            object_id: Some(record.id),
            name: record.name.clone(),
            description: record.description.clone(),
            brand: record.brand.clone(),
            model: record.model.clone(),
            serial_number: record.serial_number.clone(),
            position: record.position.clone(),
            home_id: None,
            home_name: None,
            room_id: None,
            room_name: None,
            purchase_date: record.purchase_date.clone(),
            purchase_price_cents: record.purchase_price_cents,
            seller: record.seller.clone(),
            estimated_value_cents: record.estimated_value_cents,
            condition: record
                .condition
                .as_deref()
                .and_then(ObjectCondition::from_db),
            notes: record.notes.clone(),
        }
    }

    fn is_update(&self) -> bool {
        self.object_id.is_some()
    }
}

#[derive(Debug, Clone, FromRow)]
struct ObjectHistorySnapshot {
    name: String,
    description: Option<String>,
    brand: Option<String>,
    model: Option<String>,
    serial_number: Option<String>,
    position: Option<String>,
    purchase_date: Option<String>,
    purchase_price_cents: Option<i64>,
    seller: Option<String>,
    estimated_value_cents: Option<i64>,
    condition: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectCondition {
    Excellent,
    Good,
    Worn,
    NeedsRepair,
}

impl ObjectCondition {
    const fn as_db(self) -> &'static str {
        match self {
            Self::Excellent => "ottimo",
            Self::Good => "buono",
            Self::Worn => "usurato",
            Self::NeedsRepair => "da_riparare",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Excellent => "🟢 Ottimo",
            Self::Good => "🔵 Buono",
            Self::Worn => "🟡 Usurato",
            Self::NeedsRepair => "🔴 Da riparare",
        }
    }

    fn from_db(value: &str) -> Option<Self> {
        match value {
            "ottimo" => Some(Self::Excellent),
            "buono" => Some(Self::Good),
            "usurato" => Some(Self::Worn),
            "da_riparare" => Some(Self::NeedsRepair),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct ObjectRecord {
    id: i64,
    name: String,
    description: Option<String>,
    brand: Option<String>,
    model: Option<String>,
    serial_number: Option<String>,
    position: Option<String>,
    purchase_date: Option<String>,
    purchase_price_cents: Option<i64>,
    seller: Option<String>,
    estimated_value_cents: Option<i64>,
    condition: Option<String>,
    notes: Option<String>,
    home_name: Option<String>,
    room_name: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct ObjectSummary {
    id: i64,
    name: String,
    position: Option<String>,
    home_name: Option<String>,
    room_name: Option<String>,
}

pub fn main_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("📦 Oggetti", "oggetti:menu")],
        vec![button("🏠 Case e stanze", "loc:menu")],
        vec![
            button("👕 Vestiti · prossimamente", "menu:soon"),
            button("🚗 Veicoli · prossimamente", "menu:soon"),
        ],
        vec![button("🍝 Ricette · prossimamente", "menu:soon")],
        vec![button("📊 Stato sistema", "system:status")],
    ])
}

pub async fn show_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "📦 Oggetti generici\n\nScegli cosa vuoi fare. I pulsanti e i /comandi usano la stessa logica.",
    )
    .reply_markup(objects_menu_keyboard())
    .await?;
    Ok(())
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &SessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if let Some((command, args)) = parse_command(text) {
        match command {
            "/oggetti" => {
                sessions.clear_chat(chat_id);
                show_menu(bot, msg.chat.id).await?;
                return Ok(true);
            }
            "/oggetto_nuovo" => {
                start_new_object(bot, msg.chat.id, chat_id, sessions, args).await?;
                return Ok(true);
            }
            "/oggetti_lista" => {
                sessions.clear_chat(chat_id);
                send_object_list(bot, msg.chat.id, pool, 0).await?;
                return Ok(true);
            }
            "/oggetto_cerca" => {
                if args.is_empty() {
                    sessions.set(chat_id, ConversationState::AwaitingSearch);
                    bot.send_message(
                        msg.chat.id,
                        "🔎 Cerca oggetto\n\nScrivi nome, marca, modello, casa, stanza, dettaglio posizione, seriale o una parola presente nelle note.\n\n/annulla per uscire.",
                    )
                    .await?;
                } else {
                    sessions.clear_chat(chat_id);
                    send_search_results(bot, msg.chat.id, pool, args).await?;
                }
                return Ok(true);
            }
            "/oggetto" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_object_detail(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(msg.chat.id, "Uso: /oggetto <id>\nEsempio: /oggetto 12")
                        .await?;
                }
                return Ok(true);
            }
            "/oggetto_modifica" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    start_edit_object(bot, msg.chat.id, chat_id, pool, sessions, id).await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /oggetto_modifica <id>\nEsempio: /oggetto_modifica 12",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/oggetto_elimina" => {
                sessions.clear_chat(chat_id);
                if let Some(id) = parse_positive_id(args) {
                    send_delete_confirmation(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Uso: /oggetto_elimina <id>\nEsempio: /oggetto_elimina 12",
                    )
                    .await?;
                }
                return Ok(true);
            }
            "/annulla" => {
                cancel_current_operation(bot, msg.chat.id, chat_id, pool, sessions).await?;
                return Ok(true);
            }
            "/salta" => {
                skip_current_field(bot, msg.chat.id, chat_id, sessions).await?;
                return Ok(true);
            }
            "/rimuovi" => {
                remove_current_field(bot, msg.chat.id, chat_id, sessions).await?;
                return Ok(true);
            }
            _ => return Ok(false),
        }
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        ConversationState::AwaitingObjectName => {
            if let Some(draft) = ObjectDraft::new(text) {
                sessions.set(
                    chat_id,
                    ConversationState::EditingObject {
                        draft: Box::new(draft.clone()),
                        field: None,
                    },
                );
                send_draft_panel(bot, msg.chat.id, &draft).await?;
            } else {
                bot.send_message(
                    msg.chat.id,
                    "Il nome non può essere vuoto e deve restare entro 120 caratteri. Riprova oppure usa /annulla.",
                )
                .await?;
            }
            Ok(true)
        }
        ConversationState::AwaitingSearch => {
            sessions.clear_chat(chat_id);
            if text.trim().is_empty() {
                bot.send_message(msg.chat.id, "La ricerca non può essere vuota.")
                    .await?;
            } else {
                send_search_results(bot, msg.chat.id, pool, text).await?;
            }
            Ok(true)
        }
        ConversationState::EditingObject { draft, field } => {
            let Some(field) = field else {
                bot.send_message(
                    msg.chat.id,
                    "Usa i pulsanti del pannello dettagli, oppure /annulla per uscire.",
                )
                .await?;
                return Ok(true);
            };

            apply_field_input(bot, msg.chat.id, chat_id, sessions, *draft, field, text).await?;
            Ok(true)
        }
    }
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &SessionStore,
    data: &str,
) -> ResponseResult<bool> {
    let raw_chat_id = chat_id.0;

    match data {
        "oggetti:menu" => {
            sessions.clear_chat(raw_chat_id);
            show_menu(bot, chat_id).await?;
        }
        "oggetti:new" => {
            start_new_object(bot, chat_id, raw_chat_id, sessions, "").await?;
        }
        "oggetti:list:0" => {
            sessions.clear_chat(raw_chat_id);
            send_object_list(bot, chat_id, pool, 0).await?;
        }
        "oggetti:search" => {
            sessions.set(raw_chat_id, ConversationState::AwaitingSearch);
            bot.send_message(
                chat_id,
                "🔎 Cerca oggetto\n\nScrivi cosa vuoi cercare.\n\n/annulla per uscire.",
            )
            .await?;
        }
        "oggetti:draft:name" => {
            set_draft_field(bot, chat_id, raw_chat_id, sessions, DraftField::Name).await?;
        }
        "oggetti:draft:brand" => {
            set_draft_field(bot, chat_id, raw_chat_id, sessions, DraftField::Brand).await?;
        }
        "oggetti:draft:location" => {
            show_new_object_home_picker(bot, chat_id, raw_chat_id, pool, sessions).await?;
        }
        "oggetti:draft:location:skip-home" => {
            clear_draft_structured_location_and_ask_detail(bot, chat_id, raw_chat_id, sessions)
                .await?;
        }
        _ if data.starts_with("oggetti:draft:location:home-only:") => {
            if let Some(home_id) = parse_callback_i64(data, "oggetti:draft:location:home-only:") {
                select_draft_home_only_and_ask_detail(
                    bot,
                    chat_id,
                    raw_chat_id,
                    pool,
                    sessions,
                    home_id,
                )
                .await?;
            }
        }
        _ if data.starts_with("oggetti:draft:location:room:") => {
            if let Some(room_id) = parse_callback_i64(data, "oggetti:draft:location:room:") {
                select_draft_room_and_ask_detail(
                    bot,
                    chat_id,
                    raw_chat_id,
                    pool,
                    sessions,
                    room_id,
                )
                .await?;
            }
        }
        _ if data.starts_with("oggetti:draft:location:home:") => {
            if let Some(home_id) = parse_callback_i64(data, "oggetti:draft:location:home:") {
                show_new_object_room_picker(bot, chat_id, raw_chat_id, pool, sessions, home_id)
                    .await?;
            }
        }
        "oggetti:draft:position" => {
            set_draft_field(bot, chat_id, raw_chat_id, sessions, DraftField::Position).await?;
        }
        "oggetti:draft:purchase" => {
            set_draft_field(
                bot,
                chat_id,
                raw_chat_id,
                sessions,
                DraftField::PurchaseDate,
            )
            .await?;
        }
        "oggetti:draft:notes" => {
            set_draft_field(bot, chat_id, raw_chat_id, sessions, DraftField::Notes).await?;
        }
        "oggetti:draft:condition" => {
            if let Some(ConversationState::EditingObject { draft, .. }) = sessions.get(raw_chat_id)
            {
                let text = if let Some(condition) = draft.condition {
                    format!(
                        "🛠 Condizione attuale: {}\n\nScegli una nuova condizione oppure torna ai dettagli.",
                        condition.label()
                    )
                } else {
                    "🛠 Scegli la condizione dell'oggetto:".to_string()
                };
                bot.send_message(chat_id, text)
                    .reply_markup(condition_keyboard())
                    .await?;
            } else {
                no_active_draft(bot, chat_id).await?;
            }
        }
        "oggetti:draft:condition:excellent"
        | "oggetti:draft:condition:good"
        | "oggetti:draft:condition:worn"
        | "oggetti:draft:condition:repair" => {
            let condition = match data {
                "oggetti:draft:condition:excellent" => ObjectCondition::Excellent,
                "oggetti:draft:condition:good" => ObjectCondition::Good,
                "oggetti:draft:condition:worn" => ObjectCondition::Worn,
                _ => ObjectCondition::NeedsRepair,
            };
            update_condition(bot, chat_id, raw_chat_id, sessions, condition).await?;
        }
        "oggetti:draft:condition:clear" => {
            clear_condition(bot, chat_id, raw_chat_id, sessions).await?;
        }
        "oggetti:draft:other" => {
            if let Some(ConversationState::EditingObject { draft, .. }) = sessions.get(raw_chat_id)
            {
                bot.send_message(chat_id, "⋯ Altri dettagli")
                    .reply_markup(other_details_keyboard(&draft))
                    .await?;
            } else {
                no_active_draft(bot, chat_id).await?;
            }
        }
        "oggetti:draft:description" => {
            set_draft_field(bot, chat_id, raw_chat_id, sessions, DraftField::Description).await?;
        }
        "oggetti:draft:value" => {
            set_draft_field(
                bot,
                chat_id,
                raw_chat_id,
                sessions,
                DraftField::EstimatedValue,
            )
            .await?;
        }
        "oggetti:draft:serial" => {
            set_draft_field(
                bot,
                chat_id,
                raw_chat_id,
                sessions,
                DraftField::SerialNumber,
            )
            .await?;
        }
        "oggetti:draft:back" => {
            if let Some(ConversationState::EditingObject { draft, .. }) = sessions.get(raw_chat_id)
            {
                sessions.set(
                    raw_chat_id,
                    ConversationState::EditingObject {
                        draft: draft.clone(),
                        field: None,
                    },
                );
                send_draft_panel(bot, chat_id, &draft).await?;
            } else {
                no_active_draft(bot, chat_id).await?;
            }
        }
        "oggetti:draft:save" => {
            save_current_draft(bot, chat_id, raw_chat_id, pool, sessions).await?;
        }
        "oggetti:draft:cancel" => {
            cancel_current_operation(bot, chat_id, raw_chat_id, pool, sessions).await?;
        }
        _ if data.starts_with("oggetti:edit:") => {
            sessions.clear_chat(raw_chat_id);
            if let Some(id) = parse_callback_i64(data, "oggetti:edit:") {
                start_edit_object(bot, chat_id, raw_chat_id, pool, sessions, id).await?;
            }
        }
        _ if data.starts_with("oggetti:delete:ask:") => {
            sessions.clear_chat(raw_chat_id);
            if let Some(id) = parse_callback_i64(data, "oggetti:delete:ask:") {
                send_delete_confirmation(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("oggetti:delete:do:") => {
            sessions.clear_chat(raw_chat_id);
            if let Some(id) = parse_callback_i64(data, "oggetti:delete:do:") {
                delete_object_and_media(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("oggetti:view:") => {
            sessions.clear_chat(raw_chat_id);
            if let Some(id) = parse_callback_i64(data, "oggetti:view:") {
                send_object_detail(bot, chat_id, pool, id).await?;
            }
        }
        _ if data.starts_with("oggetti:list:") => {
            sessions.clear_chat(raw_chat_id);
            if let Some(page) = parse_callback_i64(data, "oggetti:list:") {
                send_object_list(bot, chat_id, pool, page.max(0)).await?;
            }
        }
        _ => return Ok(false),
    }

    Ok(true)
}

fn object_id_to_return_after_cancel(state: Option<ConversationState>) -> Option<i64> {
    match state {
        Some(ConversationState::EditingObject { draft, .. }) => draft.object_id,
        _ => None,
    }
}

async fn cancel_current_operation(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &SessionStore,
) -> ResponseResult<()> {
    let return_object_id = object_id_to_return_after_cancel(sessions.get(raw_chat_id));
    sessions.clear_chat(raw_chat_id);

    let Some(id) = return_object_id else {
        bot.send_message(chat_id, "Operazione annullata.")
            .reply_markup(objects_menu_keyboard())
            .await?;
        return Ok(());
    };

    match get_object(pool, id).await {
        Ok(Some(object)) => {
            bot.send_message(
                chat_id,
                format!(
                    "↩️ Modifica annullata. Nessuna modifica salvata.\n\n{}",
                    format_object(&object)
                ),
            )
            .reply_markup(object_detail_keyboard(id, object.home_name.is_some()))
            .await?;
        }
        Ok(None) => {
            bot.send_message(
                chat_id,
                "Operazione annullata. L'oggetto non esiste più nel database.",
            )
            .reply_markup(objects_menu_keyboard())
            .await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                object_id = id,
                "Errore ritorno alla scheda dopo annullamento modifica"
            );
            bot.send_message(
                chat_id,
                "Operazione annullata, ma non riesco a riaprire la scheda dell'oggetto.",
            )
            .reply_markup(objects_menu_keyboard())
            .await?;
        }
    }

    Ok(())
}

async fn start_new_object(
    bot: &Bot,
    telegram_chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
    name: &str,
) -> ResponseResult<()> {
    if name.trim().is_empty() {
        sessions.set(raw_chat_id, ConversationState::AwaitingObjectName);
        bot.send_message(
            telegram_chat_id,
            "📦 Nuovo oggetto\n\nCome vuoi chiamarlo?\n\nEsempio: Trapano Bosch\n/annulla per uscire.",
        )
        .reply_markup(cancel_keyboard())
        .await?;
        return Ok(());
    }

    if let Some(draft) = ObjectDraft::new(name) {
        sessions.set(
            raw_chat_id,
            ConversationState::EditingObject {
                draft: Box::new(draft.clone()),
                field: None,
            },
        );
        send_draft_panel(bot, telegram_chat_id, &draft).await?;
    } else {
        bot.send_message(
            telegram_chat_id,
            "Il nome deve contenere almeno un carattere e non superare 120 caratteri.",
        )
        .await?;
    }

    Ok(())
}

async fn start_edit_object(
    bot: &Bot,
    telegram_chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &SessionStore,
    id: i64,
) -> ResponseResult<()> {
    match get_object(pool, id).await {
        Ok(Some(object)) => {
            let draft = ObjectDraft::from_record(&object);
            sessions.set(
                raw_chat_id,
                ConversationState::EditingObject {
                    draft: Box::new(draft.clone()),
                    field: None,
                },
            );
            send_draft_panel(bot, telegram_chat_id, &draft).await?;
        }
        Ok(None) => {
            bot.send_message(telegram_chat_id, format!("Oggetto #{id} non trovato."))
                .reply_markup(objects_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, object_id = id, "Errore avvio modifica oggetto");
            bot.send_message(
                telegram_chat_id,
                "⚠️ Non riesco ad aprire questo oggetto in modifica.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn show_new_object_home_picker(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &SessionStore,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    if draft.is_update() {
        set_draft_field(bot, chat_id, raw_chat_id, sessions, DraftField::Position).await?;
        return Ok(());
    }

    match crate::modules::luoghi::home_choices(pool).await {
        Ok(homes) => {
            let current = draft_structured_location_label(&draft)
                .unwrap_or_else(|| "Nessuna casa/stanza selezionata".to_string());
            let text = if homes.is_empty() {
                format!(
                    "🏠 Posizione del nuovo oggetto\n\n1/3 · Casa\n\nSelezione attuale: {current}\n\nNon ci sono ancora case registrate. Puoi saltare la casa e inserire direttamente il dettaglio libero della posizione."
                )
            } else {
                format!(
                    "🏠 Posizione del nuovo oggetto\n\n1/3 · Casa\n\nSelezione attuale: {current}\n\nScegli una casa. Se non vuoi assegnare un luogo strutturato, premi ⏭ Salta casa: passerai direttamente al dettaglio posizione."
                )
            };
            bot.send_message(chat_id, text)
                .reply_markup(new_object_home_picker_keyboard(&homes))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco case durante creazione oggetto");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le case disponibili.")
                .await?;
        }
    }

    Ok(())
}

async fn show_new_object_room_picker(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &SessionStore,
    home_id: i64,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    if draft.is_update() {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    }

    let home = match crate::modules::luoghi::home_choice(pool, home_id).await {
        Ok(Some(home)) => home,
        Ok(None) => {
            bot.send_message(chat_id, "La casa scelta non esiste più. Scegline un'altra.")
                .await?;
            show_new_object_home_picker(bot, chat_id, raw_chat_id, pool, sessions).await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(
                ?error,
                home_id,
                "Errore lettura casa durante creazione oggetto"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a leggere la casa scelta.")
                .await?;
            return Ok(());
        }
    };

    let rooms = match crate::modules::luoghi::room_choices(pool, home_id).await {
        Ok(rooms) => rooms,
        Err(error) => {
            tracing::error!(
                ?error,
                home_id,
                "Errore elenco stanze durante creazione oggetto"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le stanze disponibili.")
                .await?;
            return Ok(());
        }
    };

    let text = if rooms.is_empty() {
        format!(
            "🏠 Posizione del nuovo oggetto\n\n2/3 · Stanza\n\nCasa scelta: 🏠 {}\n\nQuesta casa non ha ancora stanze. Puoi assegnare l'oggetto direttamente alla casa e passare al dettaglio posizione.",
            home.name
        )
    } else {
        format!(
            "🏠 Posizione del nuovo oggetto\n\n2/3 · Stanza\n\nCasa scelta: 🏠 {}\n\nScegli una stanza oppure usa la sola casa. Una stanza può essere scelta solo dopo la sua casa.",
            home.name
        )
    };

    bot.send_message(chat_id, text)
        .reply_markup(new_object_room_picker_keyboard(home_id, &home.name, &rooms))
        .await?;
    Ok(())
}

async fn clear_draft_structured_location_and_ask_detail(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { mut draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    if draft.is_update() {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    }

    draft.home_id = None;
    draft.home_name = None;
    draft.room_id = None;
    draft.room_name = None;
    ask_position_detail_after_location(bot, chat_id, raw_chat_id, sessions, *draft).await
}

async fn select_draft_home_only_and_ask_detail(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &SessionStore,
    home_id: i64,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { mut draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    if draft.is_update() {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    }

    match crate::modules::luoghi::home_choice(pool, home_id).await {
        Ok(Some(home)) => {
            draft.home_id = Some(home.id);
            draft.home_name = Some(home.name);
            draft.room_id = None;
            draft.room_name = None;
            ask_position_detail_after_location(bot, chat_id, raw_chat_id, sessions, *draft).await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "La casa scelta non esiste più. Scegline un'altra.")
                .await?;
            show_new_object_home_picker(bot, chat_id, raw_chat_id, pool, sessions).await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                home_id,
                "Errore selezione casa durante creazione oggetto"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a selezionare questa casa.")
                .await?;
        }
    }

    Ok(())
}

async fn select_draft_room_and_ask_detail(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &SessionStore,
    room_id: i64,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { mut draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    if draft.is_update() {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    }

    match crate::modules::luoghi::room_choice(pool, room_id).await {
        Ok(Some(room)) => {
            draft.home_id = Some(room.home_id);
            draft.home_name = Some(room.home_name);
            draft.room_id = Some(room.id);
            draft.room_name = Some(room.name);
            ask_position_detail_after_location(bot, chat_id, raw_chat_id, sessions, *draft).await?;
        }
        Ok(None) => {
            bot.send_message(
                chat_id,
                "La stanza scelta non esiste più. Riapri la posizione e riprova.",
            )
            .await?;
            finish_field(bot, chat_id, raw_chat_id, sessions, *draft).await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                room_id,
                "Errore selezione stanza durante creazione oggetto"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a selezionare questa stanza.")
                .await?;
        }
    }

    Ok(())
}

async fn ask_position_detail_after_location(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
    draft: ObjectDraft,
) -> ResponseResult<()> {
    let prompt = field_prompt(DraftField::Position, &draft);
    sessions.set(
        raw_chat_id,
        ConversationState::EditingObject {
            draft: Box::new(draft),
            field: Some(DraftField::Position),
        },
    );
    bot.send_message(chat_id, prompt).await?;
    Ok(())
}

async fn set_draft_field(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
    field: DraftField,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    let prompt = field_prompt(field, &draft);

    sessions.set(
        raw_chat_id,
        ConversationState::EditingObject {
            draft,
            field: Some(field),
        },
    );

    bot.send_message(chat_id, prompt).await?;
    Ok(())
}

async fn apply_field_input(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
    mut draft: ObjectDraft,
    field: DraftField,
    input: &str,
) -> ResponseResult<()> {
    let cleaned = clean_optional(input, 500);

    match field {
        DraftField::Name => {
            if let Some(name) = clean_required(input, 120) {
                draft.name = name;
                finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
            } else {
                bot.send_message(
                    chat_id,
                    "Il nome non può essere vuoto e deve restare entro 120 caratteri. Riprova oppure usa /salta per mantenere quello attuale.",
                )
                .await?;
            }
        }
        DraftField::Brand => {
            draft.brand = clean_optional(input, 120);
            let prompt = field_prompt(DraftField::Model, &draft);
            sessions.set(
                raw_chat_id,
                ConversationState::EditingObject {
                    draft: Box::new(draft),
                    field: Some(DraftField::Model),
                },
            );
            bot.send_message(chat_id, prompt).await?;
        }
        DraftField::Model => {
            draft.model = clean_optional(input, 120);
            finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
        }
        DraftField::Position => {
            draft.position = clean_optional(input, 160);
            finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
        }
        DraftField::PurchaseDate => match parse_date_to_iso(input) {
            Some(date) => {
                draft.purchase_date = Some(date);
                let prompt = field_prompt(DraftField::PurchasePrice, &draft);
                sessions.set(
                    raw_chat_id,
                    ConversationState::EditingObject {
                        draft: Box::new(draft),
                        field: Some(DraftField::PurchasePrice),
                    },
                );
                bot.send_message(chat_id, prompt).await?;
            }
            None => {
                bot.send_message(
                    chat_id,
                    "Data non valida. Usa GG/MM/AAAA oppure AAAA-MM-GG. Esempio: 14/05/2025.\nUsa /salta per non modificare il valore.",
                )
                .await?;
            }
        },
        DraftField::PurchasePrice => match parse_money_to_cents(input) {
            Some(cents) => {
                draft.purchase_price_cents = Some(cents);
                let prompt = field_prompt(DraftField::Seller, &draft);
                sessions.set(
                    raw_chat_id,
                    ConversationState::EditingObject {
                        draft: Box::new(draft),
                        field: Some(DraftField::Seller),
                    },
                );
                bot.send_message(chat_id, prompt).await?;
            }
            None => {
                bot.send_message(
                    chat_id,
                    "Prezzo non valido. Esempi validi: 89,90 oppure 89.90 oppure 89.\nUsa /salta per non modificare il valore.",
                )
                .await?;
            }
        },
        DraftField::Seller => {
            draft.seller = clean_optional(input, 160);
            finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
        }
        DraftField::Notes => {
            draft.notes = cleaned;
            finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
        }
        DraftField::Description => {
            draft.description = cleaned;
            finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
        }
        DraftField::EstimatedValue => match parse_money_to_cents(input) {
            Some(cents) => {
                draft.estimated_value_cents = Some(cents);
                finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
            }
            None => {
                bot.send_message(
                    chat_id,
                    "Valore non valido. Esempi validi: 250 oppure 250,00.\nUsa /salta per non modificare il valore.",
                )
                .await?;
            }
        },
        DraftField::SerialNumber => {
            draft.serial_number = clean_optional(input, 160);
            finish_field(bot, chat_id, raw_chat_id, sessions, draft).await?;
        }
    }

    Ok(())
}

async fn finish_field(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
    draft: ObjectDraft,
) -> ResponseResult<()> {
    sessions.set(
        raw_chat_id,
        ConversationState::EditingObject {
            draft: Box::new(draft.clone()),
            field: None,
        },
    );
    send_draft_panel(bot, chat_id, &draft).await
}

async fn skip_current_field(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { draft, field }) = sessions.get(raw_chat_id) else {
        bot.send_message(chat_id, "Non c'è nessun campo da saltare.")
            .await?;
        return Ok(());
    };

    let Some(field) = field else {
        bot.send_message(chat_id, "Non c'è nessun campo da saltare.")
            .await?;
        return Ok(());
    };

    let next = next_draft_field(field);

    if let Some(next_field) = next {
        let prompt = field_prompt(next_field, &draft);
        sessions.set(
            raw_chat_id,
            ConversationState::EditingObject {
                draft,
                field: Some(next_field),
            },
        );
        bot.send_message(chat_id, prompt).await?;
    } else {
        finish_field(bot, chat_id, raw_chat_id, sessions, *draft).await?;
    }

    Ok(())
}

async fn remove_current_field(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { mut draft, field }) = sessions.get(raw_chat_id)
    else {
        bot.send_message(chat_id, "Non c'è nessun campo da rimuovere.")
            .await?;
        return Ok(());
    };

    let Some(field) = field else {
        bot.send_message(chat_id, "Apri prima un campo dal pannello dettagli.")
            .await?;
        return Ok(());
    };

    match field {
        DraftField::Name => {
            bot.send_message(chat_id, "Il nome è obbligatorio e non può essere rimosso. Usa /salta per mantenerlo oppure scrivi un nuovo nome.")
                .await?;
            return Ok(());
        }
        DraftField::Brand => draft.brand = None,
        DraftField::Model => draft.model = None,
        DraftField::Position => draft.position = None,
        DraftField::PurchaseDate => draft.purchase_date = None,
        DraftField::PurchasePrice => draft.purchase_price_cents = None,
        DraftField::Seller => draft.seller = None,
        DraftField::Notes => draft.notes = None,
        DraftField::Description => draft.description = None,
        DraftField::EstimatedValue => draft.estimated_value_cents = None,
        DraftField::SerialNumber => draft.serial_number = None,
    }

    if let Some(next_field) = next_draft_field(field) {
        let prompt = field_prompt(next_field, &draft);
        sessions.set(
            raw_chat_id,
            ConversationState::EditingObject {
                draft,
                field: Some(next_field),
            },
        );
        bot.send_message(chat_id, prompt).await?;
    } else {
        finish_field(bot, chat_id, raw_chat_id, sessions, *draft).await?;
    }

    Ok(())
}

fn next_draft_field(field: DraftField) -> Option<DraftField> {
    match field {
        DraftField::Brand => Some(DraftField::Model),
        DraftField::PurchaseDate => Some(DraftField::PurchasePrice),
        DraftField::PurchasePrice => Some(DraftField::Seller),
        _ => None,
    }
}

async fn update_condition(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
    condition: ObjectCondition,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { mut draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    draft.condition = Some(condition);
    finish_field(bot, chat_id, raw_chat_id, sessions, *draft).await
}

async fn clear_condition(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    sessions: &SessionStore,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { mut draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    draft.condition = None;
    finish_field(bot, chat_id, raw_chat_id, sessions, *draft).await
}

async fn save_current_draft(
    bot: &Bot,
    chat_id: ChatId,
    raw_chat_id: i64,
    pool: &SqlitePool,
    sessions: &SessionStore,
) -> ResponseResult<()> {
    let Some(ConversationState::EditingObject { draft, .. }) = sessions.get(raw_chat_id) else {
        no_active_draft(bot, chat_id).await?;
        return Ok(());
    };

    let result = if let Some(id) = draft.object_id {
        update_object(pool, id, &draft).await.map(|()| id)
    } else {
        create_object(pool, &draft).await
    };

    match result {
        Ok(id) => {
            let was_update = draft.object_id.is_some();
            sessions.clear_chat(raw_chat_id);
            let message = if was_update {
                format!("✅ Modifiche salvate per l'oggetto #{id}.")
            } else {
                format!("✅ Oggetto salvato con ID #{id}.")
            };
            bot.send_message(chat_id, message).await?;
            send_object_detail(bot, chat_id, pool, id).await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                object_id = draft.object_id,
                "Errore durante il salvataggio dell'oggetto"
            );
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a salvare. La bozza resta aperta: puoi riprovare oppure usare /annulla.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn send_draft_panel(bot: &Bot, chat_id: ChatId, draft: &ObjectDraft) -> ResponseResult<()> {
    bot.send_message(chat_id, format_draft(draft))
        .reply_markup(draft_keyboard(draft))
        .await?;
    Ok(())
}

async fn send_object_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    page: i64,
) -> ResponseResult<()> {
    match list_objects(pool, page, PAGE_SIZE).await {
        Ok((objects, total)) => {
            if objects.is_empty() {
                bot.send_message(
                    chat_id,
                    "📋 Non ci sono ancora oggetti registrati.\n\nPuoi crearne uno con ➕ Nuovo oggetto oppure /oggetto_nuovo.",
                )
                .reply_markup(objects_menu_keyboard())
                .await?;
                return Ok(());
            }

            let total_pages = ((total + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
            let mut text = format!("📋 Oggetti · pagina {}/{}\n\n", page + 1, total_pages);
            for object in &objects {
                text.push_str(&format!("#{} · {}", object.id, object.name));
                push_summary_location(&mut text, object);
                text.push_str("\n\n");
            }

            bot.send_message(chat_id, text)
                .reply_markup(list_keyboard(&objects, page, total_pages))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore durante l'elenco oggetti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere l'elenco degli oggetti.")
                .await?;
        }
    }
    Ok(())
}

async fn send_search_results(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    query: &str,
) -> ResponseResult<()> {
    match search_objects(pool, query, MAX_SEARCH_RESULTS).await {
        Ok(objects) if objects.is_empty() => {
            bot.send_message(chat_id, format!("🔎 Nessun oggetto trovato per: {query}"))
                .reply_markup(objects_menu_keyboard())
                .await?;
        }
        Ok(objects) => {
            let mut text = format!("🔎 Risultati per: {query}\n\n");
            for object in &objects {
                text.push_str(&format!("#{} · {}", object.id, object.name));
                push_summary_location(&mut text, object);
                text.push_str("\n\n");
            }
            bot.send_message(chat_id, text)
                .reply_markup(search_results_keyboard(&objects))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore durante la ricerca oggetti");
            bot.send_message(chat_id, "⚠️ Non riesco a completare la ricerca.")
                .await?;
        }
    }
    Ok(())
}

pub async fn send_object_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_object(pool, id).await {
        Ok(Some(object)) => {
            bot.send_message(chat_id, format_object(&object))
                .reply_markup(object_detail_keyboard(id, object.home_name.is_some()))
                .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, format!("Oggetto #{id} non trovato."))
                .reply_markup(objects_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, object_id = id, "Errore lettura oggetto");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo oggetto.")
                .await?;
        }
    }
    Ok(())
}

async fn send_delete_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_object(pool, id).await {
        Ok(Some(object)) => {
            bot.send_message(
                chat_id,
                format!(
                    "⚠️ Eliminare definitivamente?

📦 {}
#{}

Verranno eliminati anche i dati collegati nel database e le foto locali dell'oggetto. Questa operazione non può essere annullata.",
                    object.name, object.id
                ),
            )
            .reply_markup(delete_confirmation_keyboard(id))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, format!("Oggetto #{id} non trovato."))
                .reply_markup(objects_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                object_id = id,
                "Errore conferma eliminazione oggetto"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a preparare l'eliminazione.")
                .await?;
        }
    }
    Ok(())
}

async fn delete_object_and_media(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match delete_object(pool, id).await {
        Ok(true) => match crate::modules::foto::remove_object_media(id).await {
            Ok(()) => {
                bot.send_message(
                    chat_id,
                    format!("🗑 Oggetto #{id} eliminato definitivamente."),
                )
                .reply_markup(objects_menu_keyboard())
                .await?;
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    object_id = id,
                    "Oggetto eliminato ma pulizia media locale fallita"
                );
                bot.send_message(
                    chat_id,
                    format!(
                        "🗑 Oggetto #{id} eliminato dal database.
⚠️ Non sono riuscito a rimuovere tutti i file locali: controlla data/media/oggetti/{id}."
                    ),
                )
                .reply_markup(objects_menu_keyboard())
                .await?;
            }
        },
        Ok(false) => {
            bot.send_message(chat_id, format!("Oggetto #{id} non trovato."))
                .reply_markup(objects_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, object_id = id, "Errore eliminazione oggetto");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a eliminare l'oggetto. Nessun file locale è stato rimosso.",
            )
            .await?;
        }
    }
    Ok(())
}

async fn no_active_draft(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "Questa bozza non è più attiva. Avvia un nuovo inserimento da 📦 Oggetti.",
    )
    .reply_markup(objects_menu_keyboard())
    .await?;
    Ok(())
}

fn push_history_change(
    changes: &mut Vec<crate::modules::storico::NewFieldChange>,
    campo: &'static str,
    tipo_valore: &'static str,
    valore_prima: Option<String>,
    valore_dopo: Option<String>,
) {
    if valore_prima != valore_dopo {
        changes.push(crate::modules::storico::NewFieldChange {
            campo,
            tipo_valore,
            valore_prima,
            valore_dopo,
        });
    }
}

fn object_creation_changes(draft: &ObjectDraft) -> Vec<crate::modules::storico::NewFieldChange> {
    let mut changes = Vec::new();
    push_history_change(
        &mut changes,
        "nome",
        "testo",
        None,
        Some(draft.name.clone()),
    );
    push_history_change(
        &mut changes,
        "descrizione",
        "testo",
        None,
        draft.description.clone(),
    );
    push_history_change(&mut changes, "marca", "testo", None, draft.brand.clone());
    push_history_change(&mut changes, "modello", "testo", None, draft.model.clone());
    push_history_change(
        &mut changes,
        "numero_serie",
        "testo",
        None,
        draft.serial_number.clone(),
    );
    push_history_change(
        &mut changes,
        "posizione",
        "testo",
        None,
        draft.position.clone(),
    );
    push_history_change(
        &mut changes,
        "data_acquisto",
        "data",
        None,
        draft.purchase_date.clone(),
    );
    push_history_change(
        &mut changes,
        "prezzo_acquisto_centesimi",
        "denaro_centesimi",
        None,
        draft.purchase_price_cents.map(|value| value.to_string()),
    );
    push_history_change(
        &mut changes,
        "venditore",
        "testo",
        None,
        draft.seller.clone(),
    );
    push_history_change(
        &mut changes,
        "valore_stimato_centesimi",
        "denaro_centesimi",
        None,
        draft.estimated_value_cents.map(|value| value.to_string()),
    );
    push_history_change(
        &mut changes,
        "condizione",
        "testo",
        None,
        draft.condition.map(|value| value.as_db().to_string()),
    );
    push_history_change(&mut changes, "note", "testo", None, draft.notes.clone());
    changes
}

fn object_update_changes(
    before: &ObjectHistorySnapshot,
    draft: &ObjectDraft,
) -> Vec<crate::modules::storico::NewFieldChange> {
    let mut changes = Vec::new();
    push_history_change(
        &mut changes,
        "nome",
        "testo",
        Some(before.name.clone()),
        Some(draft.name.clone()),
    );
    push_history_change(
        &mut changes,
        "descrizione",
        "testo",
        before.description.clone(),
        draft.description.clone(),
    );
    push_history_change(
        &mut changes,
        "marca",
        "testo",
        before.brand.clone(),
        draft.brand.clone(),
    );
    push_history_change(
        &mut changes,
        "modello",
        "testo",
        before.model.clone(),
        draft.model.clone(),
    );
    push_history_change(
        &mut changes,
        "numero_serie",
        "testo",
        before.serial_number.clone(),
        draft.serial_number.clone(),
    );
    push_history_change(
        &mut changes,
        "posizione",
        "testo",
        before.position.clone(),
        draft.position.clone(),
    );
    push_history_change(
        &mut changes,
        "data_acquisto",
        "data",
        before.purchase_date.clone(),
        draft.purchase_date.clone(),
    );
    push_history_change(
        &mut changes,
        "prezzo_acquisto_centesimi",
        "denaro_centesimi",
        before.purchase_price_cents.map(|value| value.to_string()),
        draft.purchase_price_cents.map(|value| value.to_string()),
    );
    push_history_change(
        &mut changes,
        "venditore",
        "testo",
        before.seller.clone(),
        draft.seller.clone(),
    );
    push_history_change(
        &mut changes,
        "valore_stimato_centesimi",
        "denaro_centesimi",
        before.estimated_value_cents.map(|value| value.to_string()),
        draft.estimated_value_cents.map(|value| value.to_string()),
    );
    push_history_change(
        &mut changes,
        "condizione",
        "testo",
        before.condition.clone(),
        draft.condition.map(|value| value.as_db().to_string()),
    );
    push_history_change(
        &mut changes,
        "note",
        "testo",
        before.notes.clone(),
        draft.notes.clone(),
    );
    changes
}

fn object_deletion_changes(
    before: &ObjectHistorySnapshot,
) -> Vec<crate::modules::storico::NewFieldChange> {
    let mut changes = Vec::new();
    push_history_change(
        &mut changes,
        "nome",
        "testo",
        Some(before.name.clone()),
        None,
    );
    push_history_change(
        &mut changes,
        "descrizione",
        "testo",
        before.description.clone(),
        None,
    );
    push_history_change(&mut changes, "marca", "testo", before.brand.clone(), None);
    push_history_change(&mut changes, "modello", "testo", before.model.clone(), None);
    push_history_change(
        &mut changes,
        "numero_serie",
        "testo",
        before.serial_number.clone(),
        None,
    );
    push_history_change(
        &mut changes,
        "posizione",
        "testo",
        before.position.clone(),
        None,
    );
    push_history_change(
        &mut changes,
        "data_acquisto",
        "data",
        before.purchase_date.clone(),
        None,
    );
    push_history_change(
        &mut changes,
        "prezzo_acquisto_centesimi",
        "denaro_centesimi",
        before.purchase_price_cents.map(|value| value.to_string()),
        None,
    );
    push_history_change(
        &mut changes,
        "venditore",
        "testo",
        before.seller.clone(),
        None,
    );
    push_history_change(
        &mut changes,
        "valore_stimato_centesimi",
        "denaro_centesimi",
        before.estimated_value_cents.map(|value| value.to_string()),
        None,
    );
    push_history_change(
        &mut changes,
        "condizione",
        "testo",
        before.condition.clone(),
        None,
    );
    push_history_change(&mut changes, "note", "testo", before.notes.clone(), None);
    changes
}

async fn get_object_history_snapshot(
    tx: &mut Transaction<'_, Sqlite>,
    id: i64,
) -> Result<Option<ObjectHistorySnapshot>, sqlx::Error> {
    sqlx::query_as::<_, ObjectHistorySnapshot>(
        "SELECT i.nome AS name, \
                o.descrizione AS description, o.marca AS brand, o.modello AS model, \
                o.numero_serie AS serial_number, o.posizione AS position, \
                o.data_acquisto AS purchase_date, \
                o.prezzo_acquisto_centesimi AS purchase_price_cents, \
                o.venditore AS seller, \
                o.valore_stimato_centesimi AS estimated_value_cents, \
                o.condizione AS condition, o.note AS notes \
         FROM items i JOIN oggetti o ON o.item_id = i.id \
         WHERE i.id = ? AND i.tipo = 'oggetto'",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
}

async fn create_object(pool: &SqlitePool, draft: &ObjectDraft) -> anyhow::Result<i64> {
    let mut tx = pool.begin().await?;
    let item_result: SqliteQueryResult =
        sqlx::query("INSERT INTO items (tipo, nome) VALUES ('oggetto', ?)")
            .bind(&draft.name)
            .execute(&mut *tx)
            .await?;
    let id = item_result.last_insert_rowid();

    insert_object_details(&mut tx, id, draft).await?;

    if draft.room_id.is_some() && draft.home_id.is_none() {
        anyhow::bail!("una stanza non può essere salvata senza la relativa casa");
    }
    if let Some(home_id) = draft.home_id {
        crate::modules::luoghi::insert_item_location(&mut tx, id, home_id, draft.room_id).await?;
    }

    let storico_id =
        crate::modules::storico::ensure_entity(&mut tx, "oggetto", id, &draft.name).await?;
    let event_location =
        crate::modules::luoghi::history_item_location_snapshot(&mut tx, id).await?;
    let creation_event = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: storico_id,
            modulo: "oggetti",
            componente: "anagrafica",
            operazione: "creazione",
            nome_entita_snapshot: &draft.name,
            abitazione_storico_id: event_location.abitazione_storico_id,
            abitazione_nome_snapshot: event_location.abitazione_nome.as_deref(),
            stanza_storico_id: event_location.stanza_storico_id,
            stanza_nome_snapshot: event_location.stanza_nome.as_deref(),
            evento_padre_id: None,
        },
    )
    .await?;
    let creation_changes = object_creation_changes(draft);
    crate::modules::storico::record_field_changes(&mut tx, creation_event, &creation_changes)
        .await?;

    if let Some(home_id) = draft.home_id {
        let location_after =
            crate::modules::luoghi::history_location_snapshot(&mut tx, home_id, draft.room_id)
                .await?;
        let location_event = crate::modules::storico::record_event(
            &mut tx,
            &crate::modules::storico::NewHistoryEvent {
                entita_storico_id: storico_id,
                modulo: "oggetti",
                componente: "luoghi",
                operazione: "assegnazione",
                nome_entita_snapshot: &draft.name,
                abitazione_storico_id: location_after.abitazione_storico_id,
                abitazione_nome_snapshot: location_after.abitazione_nome.as_deref(),
                stanza_storico_id: location_after.stanza_storico_id,
                stanza_nome_snapshot: location_after.stanza_nome.as_deref(),
                evento_padre_id: Some(creation_event),
            },
        )
        .await?;
        crate::modules::storico::record_location_change(
            &mut tx,
            location_event,
            &crate::modules::storico::LocationSnapshot::default(),
            &location_after,
        )
        .await?;
    }

    tx.commit().await?;
    Ok(id)
}

async fn update_object(pool: &SqlitePool, id: i64, draft: &ObjectDraft) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    let Some(before) = get_object_history_snapshot(&mut tx, id).await? else {
        anyhow::bail!("oggetto #{id} non trovato durante l'aggiornamento");
    };
    let changes = object_update_changes(&before, draft);

    // Salvare una modifica senza cambiare nulla non genera UPDATE né storico.
    if changes.is_empty() {
        return Ok(());
    }

    let storico_id =
        crate::modules::storico::ensure_entity(&mut tx, "oggetto", id, &before.name).await?;

    let item = sqlx::query(
        "UPDATE items SET nome = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND tipo = 'oggetto'",
    )
    .bind(&draft.name)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if item.rows_affected() != 1 {
        anyhow::bail!("oggetto #{id} non trovato durante l'aggiornamento");
    }

    let details = sqlx::query(
        "UPDATE oggetti SET \
            descrizione = ?, marca = ?, modello = ?, numero_serie = ?, posizione = ?, \
            data_acquisto = ?, prezzo_acquisto_centesimi = ?, venditore = ?, \
            valore_stimato_centesimi = ?, condizione = ?, note = ? \
         WHERE item_id = ?",
    )
    .bind(draft.description.as_deref())
    .bind(draft.brand.as_deref())
    .bind(draft.model.as_deref())
    .bind(draft.serial_number.as_deref())
    .bind(draft.position.as_deref())
    .bind(draft.purchase_date.as_deref())
    .bind(draft.purchase_price_cents)
    .bind(draft.seller.as_deref())
    .bind(draft.estimated_value_cents)
    .bind(draft.condition.map(ObjectCondition::as_db))
    .bind(draft.notes.as_deref())
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if details.rows_affected() != 1 {
        anyhow::bail!("dettagli oggetto #{id} non trovati durante l'aggiornamento");
    }

    let event_location =
        crate::modules::luoghi::history_item_location_snapshot(&mut tx, id).await?;
    let event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: storico_id,
            modulo: "oggetti",
            componente: "anagrafica",
            operazione: "modifica",
            nome_entita_snapshot: &draft.name,
            abitazione_storico_id: event_location.abitazione_storico_id,
            abitazione_nome_snapshot: event_location.abitazione_nome.as_deref(),
            stanza_storico_id: event_location.stanza_storico_id,
            stanza_nome_snapshot: event_location.stanza_nome.as_deref(),
            evento_padre_id: None,
        },
    )
    .await?;
    crate::modules::storico::record_field_changes(&mut tx, event_id, &changes).await?;

    if before.name != draft.name {
        crate::modules::storico::rename_entity(&mut tx, storico_id, &draft.name).await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn delete_object(pool: &SqlitePool, id: i64) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;

    let Some(before) = get_object_history_snapshot(&mut tx, id).await? else {
        return Ok(false);
    };
    let storico_id =
        crate::modules::storico::ensure_entity(&mut tx, "oggetto", id, &before.name).await?;

    let event_location =
        crate::modules::luoghi::history_item_location_snapshot(&mut tx, id).await?;
    let event_id = crate::modules::storico::record_event(
        &mut tx,
        &crate::modules::storico::NewHistoryEvent {
            entita_storico_id: storico_id,
            modulo: "oggetti",
            componente: "anagrafica",
            operazione: "eliminazione",
            nome_entita_snapshot: &before.name,
            abitazione_storico_id: event_location.abitazione_storico_id,
            abitazione_nome_snapshot: event_location.abitazione_nome.as_deref(),
            stanza_storico_id: event_location.stanza_storico_id,
            stanza_nome_snapshot: event_location.stanza_nome.as_deref(),
            evento_padre_id: None,
        },
    )
    .await?;
    let deletion_changes = object_deletion_changes(&before);
    crate::modules::storico::record_field_changes(&mut tx, event_id, &deletion_changes).await?;
    crate::modules::storico::mark_entity_deleted(&mut tx, storico_id).await?;

    let result = sqlx::query("DELETE FROM items WHERE id = ? AND tipo = 'oggetto'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

async fn insert_object_details(
    tx: &mut Transaction<'_, Sqlite>,
    id: i64,
    draft: &ObjectDraft,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO oggetti (\
            item_id, descrizione, marca, modello, numero_serie, posizione, \
            data_acquisto, prezzo_acquisto_centesimi, venditore, \
            valore_stimato_centesimi, condizione, note\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(draft.description.as_deref())
    .bind(draft.brand.as_deref())
    .bind(draft.model.as_deref())
    .bind(draft.serial_number.as_deref())
    .bind(draft.position.as_deref())
    .bind(draft.purchase_date.as_deref())
    .bind(draft.purchase_price_cents)
    .bind(draft.seller.as_deref())
    .bind(draft.estimated_value_cents)
    .bind(draft.condition.map(ObjectCondition::as_db))
    .bind(draft.notes.as_deref())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn get_object(pool: &SqlitePool, id: i64) -> Result<Option<ObjectRecord>, sqlx::Error> {
    sqlx::query_as::<_, ObjectRecord>(
        "SELECT \
            i.id AS id, i.nome AS name, \
            o.descrizione AS description, o.marca AS brand, o.modello AS model, \
            o.numero_serie AS serial_number, o.posizione AS position, \
            o.data_acquisto AS purchase_date, \
            o.prezzo_acquisto_centesimi AS purchase_price_cents, \
            o.venditore AS seller, o.valore_stimato_centesimi AS estimated_value_cents, \
            o.condizione AS condition, o.note AS notes, \
            a.nome AS home_name, s.nome AS room_name \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         LEFT JOIN item_luogo il ON il.item_id = i.id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE i.id = ? AND i.tipo = 'oggetto'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

async fn list_objects(
    pool: &SqlitePool,
    page: i64,
    page_size: i64,
) -> Result<(Vec<ObjectSummary>, i64), sqlx::Error> {
    let offset = page.max(0) * page_size;
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         WHERE i.tipo = 'oggetto'",
    )
    .fetch_one(pool)
    .await?;
    let objects = sqlx::query_as::<_, ObjectSummary>(
        "SELECT i.id AS id, i.nome AS name, o.posizione AS position, \
                a.nome AS home_name, s.nome AS room_name \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         LEFT JOIN item_luogo il ON il.item_id = i.id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE i.tipo = 'oggetto' \
         ORDER BY i.nome COLLATE NOCASE, i.id \
         LIMIT ? OFFSET ?",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok((objects, total))
}

async fn search_objects(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> Result<Vec<ObjectSummary>, sqlx::Error> {
    let pattern = format!("%{}%", query.trim());
    sqlx::query_as::<_, ObjectSummary>(
        "SELECT i.id AS id, i.nome AS name, o.posizione AS position, \
                a.nome AS home_name, s.nome AS room_name \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         LEFT JOIN item_luogo il ON il.item_id = i.id \
         LEFT JOIN abitazioni a ON a.id = il.abitazione_id \
         LEFT JOIN stanze s ON s.id = il.stanza_id \
         WHERE i.tipo = 'oggetto' AND (\
            i.nome LIKE ? COLLATE NOCASE OR \
            o.marca LIKE ? COLLATE NOCASE OR \
            o.modello LIKE ? COLLATE NOCASE OR \
            o.numero_serie LIKE ? COLLATE NOCASE OR \
            o.posizione LIKE ? COLLATE NOCASE OR \
            o.venditore LIKE ? COLLATE NOCASE OR \
            o.descrizione LIKE ? COLLATE NOCASE OR \
            o.note LIKE ? COLLATE NOCASE OR \
            a.nome LIKE ? COLLATE NOCASE OR \
            s.nome LIKE ? COLLATE NOCASE\
         ) \
         ORDER BY i.nome COLLATE NOCASE, i.id \
         LIMIT ?",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
}

fn format_draft(draft: &ObjectDraft) -> String {
    let title = draft.object_id.map_or_else(
        || "📦 Nuovo oggetto".to_string(),
        |id| format!("✏️ Modifica oggetto #{id}"),
    );
    let mut lines = vec![title, String::new(), format!("Nome: {}", draft.name)];

    push_optional_line(&mut lines, "Marca", draft.brand.as_deref());
    push_optional_line(&mut lines, "Modello", draft.model.as_deref());
    if let Some(location) = draft_structured_location_label(draft) {
        lines.push(format!("Luogo: {location}"));
    }
    push_optional_line(&mut lines, "Dettaglio posizione", draft.position.as_deref());
    if let Some(date) = &draft.purchase_date {
        lines.push(format!("Data acquisto: {}", display_date(date)));
    }
    if let Some(cents) = draft.purchase_price_cents {
        lines.push(format!("Prezzo acquisto: {}", format_money(cents)));
    }
    push_optional_line(&mut lines, "Venditore", draft.seller.as_deref());
    if let Some(condition) = draft.condition {
        lines.push(format!("Condizione: {}", condition.label()));
    }
    push_optional_line(&mut lines, "Note", draft.notes.as_deref());
    push_optional_line(&mut lines, "Descrizione", draft.description.as_deref());
    if let Some(cents) = draft.estimated_value_cents {
        lines.push(format!("Valore stimato: {}", format_money(cents)));
    }
    push_optional_line(&mut lines, "Numero seriale", draft.serial_number.as_deref());

    lines.push(String::new());
    if draft.is_update() {
        lines.push(
            "Modifica solo ciò che serve. /salta mantiene il valore attuale; /rimuovi cancella il campo aperto. Poi premi 💾 Salva modifiche."
                .to_string(),
        );
    } else {
        lines.push("Aggiungi solo i dettagli che ti servono, poi premi ✅ Salva.".to_string());
    }
    lines.join("\n")
}

fn format_object(object: &ObjectRecord) -> String {
    let mut lines = vec![format!("📦 {}", object.name), format!("#{}", object.id)];

    if object.brand.is_some() || object.model.is_some() {
        let brand_model = [object.brand.as_deref(), object.model.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" — ");
        lines.push(format!("🏷 {brand_model}"));
    }
    if let Some(home) = &object.home_name {
        if let Some(room) = &object.room_name {
            lines.push(format!("🏠 {home} / 🚪 {room}"));
        } else {
            lines.push(format!("🏠 {home}"));
        }
    }
    if let Some(position) = &object.position {
        lines.push(format!("📌 {position}"));
    }
    if let Some(condition) = object
        .condition
        .as_deref()
        .and_then(ObjectCondition::from_db)
    {
        lines.push(format!("🛠 {}", condition.label()));
    }
    if let Some(date) = &object.purchase_date {
        lines.push(format!("📅 Acquistato: {}", display_date(date)));
    }
    if let Some(cents) = object.purchase_price_cents {
        lines.push(format!("💶 Prezzo: {}", format_money(cents)));
    }
    if let Some(seller) = &object.seller {
        lines.push(format!("🏪 Venditore: {seller}"));
    }
    if let Some(cents) = object.estimated_value_cents {
        lines.push(format!("💰 Valore stimato: {}", format_money(cents)));
    }
    if let Some(serial) = &object.serial_number {
        lines.push(format!("🔢 Seriale: {serial}"));
    }
    if let Some(description) = &object.description {
        lines.push(format!("\nDescrizione:\n{description}"));
    }
    if let Some(notes) = &object.notes {
        lines.push(format!("\n📝 Note:\n{notes}"));
    }

    lines.join("\n")
}

fn objects_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("➕ Nuovo oggetto", "oggetti:new")],
        vec![
            button("📋 Elenco oggetti", "oggetti:list:0"),
            button("🔎 Cerca", "oggetti:search"),
        ],
        vec![button("🏠 Filtra per casa / stanza", "loc:home:list")],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn draft_keyboard(draft: &ObjectDraft) -> InlineKeyboardMarkup {
    let brand_model = section_label(
        "🏷 Marca e modello",
        draft.brand.is_some() || draft.model.is_some(),
    );
    let position = if draft.is_update() {
        section_label("📌 Dettaglio posizione", draft.position.is_some())
    } else {
        section_label(
            "🏠 Posizione",
            draft.home_id.is_some() || draft.position.is_some(),
        )
    };
    let purchase = section_label(
        "💶 Acquisto",
        draft.purchase_date.is_some()
            || draft.purchase_price_cents.is_some()
            || draft.seller.is_some(),
    );
    let condition = section_label("🛠 Condizione", draft.condition.is_some());
    let notes = section_label("📝 Note", draft.notes.is_some());
    let other = section_label(
        "⋯ Altri dettagli",
        draft.description.is_some()
            || draft.estimated_value_cents.is_some()
            || draft.serial_number.is_some(),
    );

    let mut rows = Vec::new();
    if draft.is_update() {
        rows.push(vec![button("✏️ Nome", "oggetti:draft:name")]);
    }
    rows.push(vec![button(&brand_model, "oggetti:draft:brand")]);
    rows.push(vec![
        button(
            &position,
            if draft.is_update() {
                "oggetti:draft:position"
            } else {
                "oggetti:draft:location"
            },
        ),
        button(&purchase, "oggetti:draft:purchase"),
    ]);
    rows.push(vec![
        button(&condition, "oggetti:draft:condition"),
        button(&notes, "oggetti:draft:notes"),
    ]);
    rows.push(vec![button(&other, "oggetti:draft:other")]);
    rows.push(vec![
        button(
            if draft.is_update() {
                "💾 Salva modifiche"
            } else {
                "✅ Salva"
            },
            "oggetti:draft:save",
        ),
        button("❌ Annulla", "oggetti:draft:cancel"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn new_object_home_picker_keyboard(
    homes: &[crate::modules::luoghi::HomeChoice],
) -> InlineKeyboardMarkup {
    let mut rows = homes
        .iter()
        .map(|home| {
            vec![button(
                &format!("🏠 {}", home.name),
                &format!("oggetti:draft:location:home:{}", home.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        "⏭ Salta casa → dettaglio",
        "oggetti:draft:location:skip-home",
    )]);
    rows.push(vec![button("↩️ Torna ai dettagli", "oggetti:draft:back")]);
    InlineKeyboardMarkup::new(rows)
}

fn new_object_room_picker_keyboard(
    home_id: i64,
    home_name: &str,
    rooms: &[crate::modules::luoghi::RoomChoice],
) -> InlineKeyboardMarkup {
    let mut rows = rooms
        .iter()
        .map(|room| {
            vec![button(
                &format!("🚪 {}", room.name),
                &format!("oggetti:draft:location:room:{}", room.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button(
        &format!("🏠 Solo {} → dettaglio", truncate_chars(home_name, 28)),
        &format!("oggetti:draft:location:home-only:{home_id}"),
    )]);
    rows.push(vec![button("↩️ Cambia casa", "oggetti:draft:location")]);
    rows.push(vec![button("↩️ Torna ai dettagli", "oggetti:draft:back")]);
    InlineKeyboardMarkup::new(rows)
}

fn draft_structured_location_label(draft: &ObjectDraft) -> Option<String> {
    let home = draft.home_name.as_deref()?;
    Some(match draft.room_name.as_deref() {
        Some(room) => format!("🏠 {home} / 🚪 {room}"),
        None => format!("🏠 {home}"),
    })
}

fn section_label(label: &str, filled: bool) -> String {
    if filled {
        format!("✅ {label}")
    } else {
        label.to_string()
    }
}

fn condition_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            button("🟢 Ottimo", "oggetti:draft:condition:excellent"),
            button("🔵 Buono", "oggetti:draft:condition:good"),
        ],
        vec![
            button("🟡 Usurato", "oggetti:draft:condition:worn"),
            button("🔴 Da riparare", "oggetti:draft:condition:repair"),
        ],
        vec![button(
            "🗑 Rimuovi condizione",
            "oggetti:draft:condition:clear",
        )],
        vec![button("⬅️ Dettagli", "oggetti:draft:back")],
    ])
}

fn other_details_keyboard(draft: &ObjectDraft) -> InlineKeyboardMarkup {
    let description = section_label("📝 Descrizione", draft.description.is_some());
    let value = section_label("💰 Valore stimato", draft.estimated_value_cents.is_some());
    let serial = section_label("🔢 Numero seriale", draft.serial_number.is_some());

    InlineKeyboardMarkup::new(vec![
        vec![button(&description, "oggetti:draft:description")],
        vec![button(&value, "oggetti:draft:value")],
        vec![button(&serial, "oggetti:draft:serial")],
        vec![button("⬅️ Dettagli", "oggetti:draft:back")],
    ])
}

fn cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button("❌ Annulla", "oggetti:draft:cancel")]])
}

fn object_detail_keyboard(id: i64, has_structured_location: bool) -> InlineKeyboardMarkup {
    let location_label = if has_structured_location {
        "🚚 Sposta oggetto"
    } else {
        "🏠 Assegna casa / stanza"
    };

    InlineKeyboardMarkup::new(vec![
        vec![
            button("✏️ Modifica", &format!("oggetti:edit:{id}")),
            button("🗑 Elimina", &format!("oggetti:delete:ask:{id}")),
        ],
        vec![button(location_label, &format!("loc:item:{id}"))],
        vec![button("📷 Foto", &format!("foto:menu:{id}"))],
        vec![button("📋 Elenco", "oggetti:list:0")],
        vec![
            button("🔎 Cerca", "oggetti:search"),
            button("➕ Nuovo", "oggetti:new"),
        ],
        vec![button("📦 Menu oggetti", "oggetti:menu")],
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn delete_confirmation_keyboard(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "🗑 Sì, elimina definitivamente",
            &format!("oggetti:delete:do:{id}"),
        )],
        vec![button("↩️ Annulla", &format!("oggetti:view:{id}"))],
    ])
}

fn list_keyboard(objects: &[ObjectSummary], page: i64, total_pages: i64) -> InlineKeyboardMarkup {
    let mut rows = objects
        .iter()
        .map(|object| {
            let label = object_button_label(object);
            let data = format!("oggetti:view:{}", object.id);
            vec![button(&label, &data)]
        })
        .collect::<Vec<_>>();

    let mut navigation = Vec::new();
    if page > 0 {
        navigation.push(button(
            "◀️",
            &format!("oggetti:list:{}", page.saturating_sub(1)),
        ));
    }
    if page + 1 < total_pages {
        navigation.push(button("▶️", &format!("oggetti:list:{}", page + 1)));
    }
    if !navigation.is_empty() {
        rows.push(navigation);
    }
    rows.push(vec![
        button("🔎 Cerca", "oggetti:search"),
        button("➕ Nuovo", "oggetti:new"),
    ]);
    rows.push(vec![button("📦 Menu oggetti", "oggetti:menu")]);
    InlineKeyboardMarkup::new(rows)
}

fn search_results_keyboard(objects: &[ObjectSummary]) -> InlineKeyboardMarkup {
    let mut rows = objects
        .iter()
        .map(|object| {
            let label = object_button_label(object);
            let data = format!("oggetti:view:{}", object.id);
            vec![button(&label, &data)]
        })
        .collect::<Vec<_>>();
    rows.push(vec![button("🔎 Nuova ricerca", "oggetti:search")]);
    rows.push(vec![button("📦 Menu oggetti", "oggetti:menu")]);
    InlineKeyboardMarkup::new(rows)
}

fn push_summary_location(text: &mut String, object: &ObjectSummary) {
    if let Some(home) = &object.home_name {
        if let Some(room) = &object.room_name {
            text.push_str(&format!("\n🏠 {home} / {room}"));
        } else {
            text.push_str(&format!("\n🏠 {home}"));
        }
    }
    if let Some(position) = &object.position {
        text.push_str(&format!("\n📌 {position}"));
    }
}

fn object_button_label(object: &ObjectSummary) -> String {
    let short_name = truncate_chars(&object.name, 42);
    format!("📦 #{} · {short_name}", object.id)
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

fn button(label: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.to_string(), data.to_string())
}

fn field_prompt(field: DraftField, draft: &ObjectDraft) -> String {
    let instruction = match field {
        DraftField::Name => "✏️ Inserisci il nome dell'oggetto.".to_string(),
        DraftField::Brand => "🏷 Inserisci la marca.".to_string(),
        DraftField::Model => "🏷 Inserisci il modello.".to_string(),
        DraftField::Position if draft.is_update() => "📌 Inserisci un dettaglio libero della posizione.\nEsempio: scaffale 2, cassetto alto.\nCasa e stanza si cambiano dalla scheda dell'oggetto con 🚚 Sposta oggetto.".to_string(),
        DraftField::Position => {
            let structured = draft_structured_location_label(draft)
                .unwrap_or_else(|| "Nessuna casa/stanza".to_string());
            format!(
                "📌 3/3 · Dettaglio posizione\n\nLuogo scelto: {structured}\n\nInserisci un dettaglio libero opzionale.\nEsempio: scaffale 2, cassetto alto."
            )
        }
        DraftField::PurchaseDate => "📅 Inserisci la data di acquisto (GG/MM/AAAA o AAAA-MM-GG).".to_string(),
        DraftField::PurchasePrice => "💶 Inserisci il prezzo pagato.\nEsempio: 89,90".to_string(),
        DraftField::Seller => "🏪 Inserisci negozio o venditore.\nEsempio: Amazon".to_string(),
        DraftField::Notes => "📝 Inserisci le note.".to_string(),
        DraftField::Description => "📝 Inserisci una descrizione.".to_string(),
        DraftField::EstimatedValue => "💰 Inserisci il valore stimato attuale.\nEsempio: 250".to_string(),
        DraftField::SerialNumber => "🔢 Inserisci il numero seriale.".to_string(),
    };

    let current = match field {
        DraftField::Name => Some(draft.name.clone()),
        DraftField::Brand => draft.brand.clone(),
        DraftField::Model => draft.model.clone(),
        DraftField::Position => draft.position.clone(),
        DraftField::PurchaseDate => draft.purchase_date.as_deref().map(display_date),
        DraftField::PurchasePrice => draft.purchase_price_cents.map(format_money),
        DraftField::Seller => draft.seller.clone(),
        DraftField::Notes => draft.notes.clone(),
        DraftField::Description => draft.description.clone(),
        DraftField::EstimatedValue => draft.estimated_value_cents.map(format_money),
        DraftField::SerialNumber => draft.serial_number.clone(),
    };

    if let Some(current) = current {
        if field == DraftField::Name {
            format!(
                "{instruction}\n\nValore attuale:\n{current}\n\nScrivi un nuovo valore oppure usa /salta per mantenere quello attuale. Il nome è obbligatorio e non può essere rimosso."
            )
        } else {
            format!(
                "{instruction}\n\nValore attuale:\n{current}\n\nScrivi un nuovo valore, usa /salta per mantenerlo oppure /rimuovi per cancellarlo."
            )
        }
    } else if field == DraftField::Name {
        instruction
    } else {
        format!("{instruction}\n\nUsa /salta per lasciare il campo vuoto.")
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

fn parse_callback_i64(data: &str, prefix: &str) -> Option<i64> {
    data.strip_prefix(prefix)?.parse().ok()
}

fn clean_required(value: &str, max_chars: usize) -> Option<String> {
    let trimmed = value.trim();
    let len = trimmed.chars().count();
    if trimmed.is_empty() || len > max_chars {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn clean_optional(value: &str, max_chars: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.chars().count() <= max_chars {
        Some(trimmed.to_string())
    } else {
        Some(trimmed.chars().take(max_chars).collect())
    }
}

fn parse_money_to_cents(input: &str) -> Option<i64> {
    let compact = input.trim().replace(['€', ' ', '\u{00a0}'], "");
    if compact.is_empty() || compact.starts_with('-') {
        return None;
    }

    let normalized = if compact.contains(',') {
        compact.replace('.', "").replace(',', ".")
    } else {
        compact
    };

    let (whole, fraction) = normalized
        .split_once('.')
        .map_or((normalized.as_str(), ""), |parts| parts);
    if whole.is_empty()
        || !whole.chars().all(|ch| ch.is_ascii_digit())
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
        || fraction.len() > 2
    {
        return None;
    }

    let euros = whole.parse::<i64>().ok()?;
    let cents = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        2 => fraction.parse::<i64>().ok()?,
        _ => return None,
    };
    euros.checked_mul(100)?.checked_add(cents)
}

fn parse_date_to_iso(input: &str) -> Option<String> {
    let value = input.trim();
    let (year, month, day) = if value.contains('/') {
        let mut parts = value.split('/');
        let day = parts.next()?.parse::<u32>().ok()?;
        let month = parts.next()?.parse::<u32>().ok()?;
        let year = parts.next()?.parse::<i32>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        (year, month, day)
    } else {
        let mut parts = value.split('-');
        let year = parts.next()?.parse::<i32>().ok()?;
        let month = parts.next()?.parse::<u32>().ok()?;
        let day = parts.next()?.parse::<u32>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        (year, month, day)
    };

    if !valid_date(year, month, day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn valid_date(year: i32, month: u32, day: u32) -> bool {
    if !(1900..=2200).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=max_day).contains(&day)
}

fn format_money(cents: i64) -> String {
    format!("{},{:02} €", cents / 100, cents % 100)
}

fn display_date(iso: &str) -> String {
    let mut parts = iso.split('-');
    let Some(year) = parts.next() else {
        return iso.to_string();
    };
    let Some(month) = parts.next() else {
        return iso.to_string();
    };
    let Some(day) = parts.next() else {
        return iso.to_string();
    };
    if parts.next().is_some() {
        return iso.to_string();
    }
    format!("{day}/{month}/{year}")
}

fn push_optional_line(lines: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        lines.push(format!("{label}: {value}"));
    }
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

    #[test]
    fn prezzi_accettano_formati_italiani_e_decimali() {
        assert_eq!(parse_money_to_cents("89,90"), Some(8_990));
        assert_eq!(parse_money_to_cents("89.90"), Some(8_990));
        assert_eq!(parse_money_to_cents("1.234,56 €"), Some(123_456));
        assert_eq!(parse_money_to_cents("89"), Some(8_900));
        assert_eq!(parse_money_to_cents("-1"), None);
        assert_eq!(parse_money_to_cents("12,345"), None);
    }

    #[test]
    fn date_vengono_normalizzate_e_validate() {
        assert_eq!(
            parse_date_to_iso("14/05/2025").as_deref(),
            Some("2025-05-14")
        );
        assert_eq!(
            parse_date_to_iso("2025-05-14").as_deref(),
            Some("2025-05-14")
        );
        assert_eq!(
            parse_date_to_iso("29/02/2024").as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(parse_date_to_iso("29/02/2025"), None);
        assert_eq!(parse_date_to_iso("31/04/2025"), None);
    }

    #[test]
    fn parser_comandi_mantiene_argomenti_e_rimuove_username_bot() {
        assert_eq!(
            parse_command("/oggetto_nuovo@CasaBot Trapano Bosch"),
            Some(("/oggetto_nuovo", "Trapano Bosch"))
        );
    }

    #[test]
    fn campo_gia_compilato_mostra_il_valore_attuale() {
        let mut draft = ObjectDraft::new("Trapano").expect("bozza");
        draft.brand = Some("Bosch".to_string());

        let prompt = field_prompt(DraftField::Brand, &draft);

        assert!(prompt.contains("Valore attuale:"));
        assert!(prompt.contains("Bosch"));
        assert!(prompt.contains("/salta per mantenerlo"));
        assert!(prompt.contains("/rimuovi per cancellarlo"));
    }

    #[test]
    fn annulla_modifica_torna_all_oggetto_originale() {
        let mut update_draft = ObjectDraft::new("Trapano").expect("bozza");
        update_draft.object_id = Some(42);
        let update_state = ConversationState::EditingObject {
            draft: Box::new(update_draft),
            field: Some(DraftField::Brand),
        };
        assert_eq!(
            object_id_to_return_after_cancel(Some(update_state)),
            Some(42)
        );

        let create_draft = ObjectDraft::new("Valigia").expect("bozza");
        let create_state = ConversationState::EditingObject {
            draft: Box::new(create_draft),
            field: None,
        };
        assert_eq!(object_id_to_return_after_cancel(Some(create_state)), None);
        assert_eq!(
            object_id_to_return_after_cancel(Some(ConversationState::AwaitingSearch)),
            None
        );
    }

    #[test]
    fn bozza_di_modifica_conserva_id_e_valori_correnti() {
        let record = ObjectRecord {
            id: 42,
            name: "MacBook".to_string(),
            description: None,
            brand: Some("Apple".to_string()),
            model: Some("Pro".to_string()),
            serial_number: None,
            position: Some("Studio".to_string()),
            purchase_date: None,
            purchase_price_cents: None,
            seller: None,
            estimated_value_cents: None,
            condition: Some("ottimo".to_string()),
            notes: None,
            home_name: Some("Casa principale".to_string()),
            room_name: Some("Studio".to_string()),
        };

        let draft = ObjectDraft::from_record(&record);
        assert_eq!(draft.object_id, Some(42));
        assert_eq!(draft.name, "MacBook");
        assert_eq!(draft.brand.as_deref(), Some("Apple"));
        assert_eq!(draft.condition, Some(ObjectCondition::Excellent));
        assert!(draft.is_update());
    }

    #[tokio::test]
    async fn oggetto_viene_salvato_letto_elencato_e_cercato() {
        let pool = test_pool().await;
        let mut draft = ObjectDraft::new("Trapano Bosch").expect("bozza");
        draft.brand = Some("Bosch".to_string());
        draft.position = Some("Garage - scaffale 2".to_string());
        draft.purchase_price_cents = Some(8_990);
        draft.condition = Some(ObjectCondition::Good);

        let id = create_object(&pool, &draft).await.expect("salvataggio");
        let object = get_object(&pool, id)
            .await
            .expect("lettura")
            .expect("oggetto presente");
        assert_eq!(object.name, "Trapano Bosch");
        assert_eq!(object.brand.as_deref(), Some("Bosch"));
        assert_eq!(object.purchase_price_cents, Some(8_990));

        let (objects, total) = list_objects(&pool, 0, PAGE_SIZE).await.expect("elenco");
        assert_eq!(total, 1);
        assert_eq!(objects[0].id, id);

        let search = search_objects(&pool, "garage", 10).await.expect("ricerca");
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].id, id);

        let home = sqlx::query("INSERT INTO abitazioni (nome) VALUES ('Casa principale')")
            .execute(&pool)
            .await
            .expect("casa");
        let home_id = home.last_insert_rowid();
        let room = sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, 'Officina')")
            .bind(home_id)
            .execute(&pool)
            .await
            .expect("stanza");
        let room_id = room.last_insert_rowid();
        sqlx::query("INSERT INTO item_luogo (item_id, abitazione_id, stanza_id) VALUES (?, ?, ?)")
            .bind(id)
            .bind(home_id)
            .bind(room_id)
            .execute(&pool)
            .await
            .expect("luogo");

        let by_home = search_objects(&pool, "principale", 10)
            .await
            .expect("ricerca casa");
        assert_eq!(by_home.len(), 1);
        assert_eq!(by_home[0].home_name.as_deref(), Some("Casa principale"));

        let by_room = search_objects(&pool, "officina", 10)
            .await
            .expect("ricerca stanza");
        assert_eq!(by_room.len(), 1);
        assert_eq!(by_room[0].room_name.as_deref(), Some("Officina"));
    }

    #[tokio::test]
    async fn oggetto_salvato_puo_essere_modificato_senza_creare_duplicati() {
        let pool = test_pool().await;
        let mut original = ObjectDraft::new("Trapano Bosch").expect("bozza");
        original.brand = Some("Bosch".to_string());
        original.position = Some("Garage".to_string());
        original.notes = Some("Prima nota".to_string());

        let id = create_object(&pool, &original).await.expect("salvataggio");
        let record = get_object(&pool, id)
            .await
            .expect("lettura")
            .expect("oggetto presente");
        let mut edited = ObjectDraft::from_record(&record);
        edited.name = "Trapano officina".to_string();
        edited.brand = None;
        edited.position = Some("Cantina".to_string());
        edited.notes = None;

        update_object(&pool, id, &edited)
            .await
            .expect("aggiornamento");

        let updated = get_object(&pool, id)
            .await
            .expect("rilettura")
            .expect("oggetto presente");
        assert_eq!(updated.name, "Trapano officina");
        assert_eq!(updated.brand, None);
        assert_eq!(updated.position.as_deref(), Some("Cantina"));
        assert_eq!(updated.notes, None);

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE tipo = 'oggetto'")
            .fetch_one(&pool)
            .await
            .expect("conteggio items");
        assert_eq!(total, 1);
    }

    #[tokio::test]
    async fn eliminazione_oggetto_rimuove_record_e_foto_collegate() {
        let pool = test_pool().await;
        let draft = ObjectDraft::new("Oggetto con foto").expect("bozza");
        let id = create_object(&pool, &draft).await.expect("salvataggio");

        sqlx::query(
            "INSERT INTO foto (item_id, percorso_file, ruolo, descrizione) VALUES (?, ?, 'principale', ?)",
        )
        .bind(id)
        .bind(format!("data/media/oggetti/{id}/test.jpg"))
        .bind("foto test")
        .execute(&pool)
        .await
        .expect("foto test");

        assert!(delete_object(&pool, id).await.expect("eliminazione"));

        let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("conteggio item");
        let object_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM oggetti WHERE item_id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("conteggio oggetto");
        let photo_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foto WHERE item_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("conteggio foto");

        assert_eq!(item_count, 0);
        assert_eq!(object_count, 0);
        assert_eq!(photo_count, 0);
    }

    #[tokio::test]
    async fn dettaglio_oggetto_segue_cascade_di_items() {
        let pool = test_pool().await;
        let draft = ObjectDraft::new("Valigia grande").expect("bozza");
        let id = create_object(&pool, &draft).await.expect("salvataggio");

        sqlx::query("DELETE FROM items WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .expect("delete item");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oggetti WHERE item_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("conteggio");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn nuovo_oggetto_salva_casa_stanza_e_dettaglio_nella_stessa_creazione() {
        let pool = test_pool().await;
        let home = sqlx::query("INSERT INTO abitazioni (nome) VALUES ('Casa principale')")
            .execute(&pool)
            .await
            .expect("casa");
        let home_id = home.last_insert_rowid();
        let room = sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, 'Garage')")
            .bind(home_id)
            .execute(&pool)
            .await
            .expect("stanza");
        let room_id = room.last_insert_rowid();

        let mut draft = ObjectDraft::new("Trapano guidato").expect("bozza");
        draft.home_id = Some(home_id);
        draft.home_name = Some("Casa principale".to_string());
        draft.room_id = Some(room_id);
        draft.room_name = Some("Garage".to_string());
        draft.position = Some("Scaffale 2".to_string());

        let id = create_object(&pool, &draft).await.expect("salvataggio");
        let object = get_object(&pool, id)
            .await
            .expect("lettura")
            .expect("oggetto presente");

        assert_eq!(object.home_name.as_deref(), Some("Casa principale"));
        assert_eq!(object.room_name.as_deref(), Some("Garage"));
        assert_eq!(object.position.as_deref(), Some("Scaffale 2"));
    }

    #[tokio::test]
    async fn nuova_creazione_non_salva_stanza_senza_casa() {
        let pool = test_pool().await;
        let mut draft = ObjectDraft::new("Bozza incoerente").expect("bozza");
        draft.room_id = Some(999);
        draft.room_name = Some("Garage".to_string());

        let result = create_object(&pool, &draft).await;
        assert!(result.is_err());

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE tipo = 'oggetto'")
            .fetch_one(&pool)
            .await
            .expect("conteggio");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn migration_rifiuta_valori_monetari_negativi() {
        let pool = test_pool().await;
        let item = sqlx::query("INSERT INTO items (tipo, nome) VALUES ('oggetto', 'Test')")
            .execute(&pool)
            .await
            .expect("item");
        let id = item.last_insert_rowid();
        let result =
            sqlx::query("INSERT INTO oggetti (item_id, prezzo_acquisto_centesimi) VALUES (?, -1)")
                .bind(id)
                .execute(&pool)
                .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn storico_oggetto_conserva_contesto_luogo_e_non_registra_noop() {
        let pool = test_pool().await;

        let home = sqlx::query("INSERT INTO abitazioni (nome) VALUES ('Casa storico')")
            .execute(&pool)
            .await
            .expect("casa");
        let home_id = home.last_insert_rowid();

        let room =
            sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, 'Garage storico')")
                .bind(home_id)
                .execute(&pool)
                .await
                .expect("stanza");
        let room_id = room.last_insert_rowid();

        let mut draft = ObjectDraft::new("Trapano storico").expect("bozza");
        draft.brand = Some("Bosch".to_string());
        draft.home_id = Some(home_id);
        draft.home_name = Some("Casa storico".to_string());
        draft.room_id = Some(room_id);
        draft.room_name = Some("Garage storico".to_string());

        let id = create_object(&pool, &draft).await.expect("creazione");

        let creation_context: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT abitazione_nome_snapshot, stanza_nome_snapshot \
             FROM storico_eventi \
             WHERE operazione = 'creazione' AND nome_entita_snapshot = 'Trapano storico' \
             ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("contesto creazione");
        assert_eq!(
            creation_context,
            (
                Some("Casa storico".to_string()),
                Some("Garage storico".to_string())
            )
        );

        let record = get_object(&pool, id)
            .await
            .expect("lettura")
            .expect("oggetto");
        let mut edited = ObjectDraft::from_record(&record);
        edited.brand = Some("Makita".to_string());

        update_object(&pool, id, &edited)
            .await
            .expect("prima modifica");

        let modification_count_before: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM storico_eventi \
             WHERE operazione = 'modifica' AND nome_entita_snapshot = 'Trapano storico'",
        )
        .fetch_one(&pool)
        .await
        .expect("conteggio modifica");
        assert_eq!(modification_count_before, 1);

        update_object(&pool, id, &edited)
            .await
            .expect("salvataggio invariato");

        let modification_count_after: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM storico_eventi \
             WHERE operazione = 'modifica' AND nome_entita_snapshot = 'Trapano storico'",
        )
        .fetch_one(&pool)
        .await
        .expect("conteggio dopo noop");
        assert_eq!(modification_count_after, 1);

        let modification_context: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT abitazione_nome_snapshot, stanza_nome_snapshot \
             FROM storico_eventi \
             WHERE operazione = 'modifica' AND nome_entita_snapshot = 'Trapano storico' \
             ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("contesto modifica");
        assert_eq!(
            modification_context,
            (
                Some("Casa storico".to_string()),
                Some("Garage storico".to_string())
            )
        );

        assert!(delete_object(&pool, id).await.expect("eliminazione"));

        let deletion_context: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT abitazione_nome_snapshot, stanza_nome_snapshot \
             FROM storico_eventi \
             WHERE operazione = 'eliminazione' AND nome_entita_snapshot = 'Trapano storico' \
             ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("contesto eliminazione");
        assert_eq!(
            deletion_context,
            (
                Some("Casa storico".to_string()),
                Some("Garage storico".to_string())
            )
        );
    }
}
