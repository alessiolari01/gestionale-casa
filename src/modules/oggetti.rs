//! Modulo "oggetti generici".
//!
//! Step 5A: anagrafica base, menu Telegram con inline keyboard, comandi
//! testuali equivalenti, inserimento guidato, elenco, ricerca e scheda singola.

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
    condition: Option<ObjectCondition>,
    notes: Option<String>,
}

impl ObjectDraft {
    fn new(name: &str) -> Option<Self> {
        let name = clean_required(name, 120)?;
        Some(Self {
            name,
            ..Self::default()
        })
    }
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
}

#[derive(Debug, Clone, FromRow)]
struct ObjectSummary {
    id: i64,
    name: String,
    position: Option<String>,
}

pub fn main_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("📦 Oggetti", "oggetti:menu")],
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
                        "🔎 Cerca oggetto\n\nScrivi nome, marca, modello, posizione, seriale o una parola presente nelle note.\n\n/annulla per uscire.",
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
                if let Ok(id) = args.parse::<i64>() {
                    send_object_detail(bot, msg.chat.id, pool, id).await?;
                } else {
                    bot.send_message(msg.chat.id, "Uso: /oggetto <id>\nEsempio: /oggetto 12")
                        .await?;
                }
                return Ok(true);
            }
            "/annulla" => {
                sessions.clear_chat(chat_id);
                bot.send_message(msg.chat.id, "Operazione annullata.")
                    .reply_markup(objects_menu_keyboard())
                    .await?;
                return Ok(true);
            }
            "/salta" => {
                skip_current_field(bot, msg.chat.id, chat_id, sessions).await?;
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
        "oggetti:draft:brand" => {
            set_draft_field(bot, chat_id, raw_chat_id, sessions, DraftField::Brand).await?;
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
            if matches!(
                sessions.get(raw_chat_id),
                Some(ConversationState::EditingObject { .. })
            ) {
                bot.send_message(chat_id, "🛠 Scegli la condizione dell'oggetto:")
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
        "oggetti:draft:other" => {
            if matches!(
                sessions.get(raw_chat_id),
                Some(ConversationState::EditingObject { .. })
            ) {
                bot.send_message(chat_id, "⋯ Altri dettagli")
                    .reply_markup(other_details_keyboard())
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
            sessions.clear_chat(raw_chat_id);
            bot.send_message(chat_id, "Creazione oggetto annullata.")
                .reply_markup(objects_menu_keyboard())
                .await?;
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

    sessions.set(
        raw_chat_id,
        ConversationState::EditingObject {
            draft,
            field: Some(field),
        },
    );

    bot.send_message(chat_id, field_prompt(field)).await?;
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
        DraftField::Brand => {
            draft.brand = clean_optional(input, 120);
            sessions.set(
                raw_chat_id,
                ConversationState::EditingObject {
                    draft: Box::new(draft),
                    field: Some(DraftField::Model),
                },
            );
            bot.send_message(chat_id, field_prompt(DraftField::Model))
                .await?;
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
                sessions.set(
                    raw_chat_id,
                    ConversationState::EditingObject {
                        draft: Box::new(draft),
                        field: Some(DraftField::PurchasePrice),
                    },
                );
                bot.send_message(chat_id, field_prompt(DraftField::PurchasePrice))
                    .await?;
            }
            None => {
                bot.send_message(
                    chat_id,
                    "Data non valida. Usa GG/MM/AAAA oppure AAAA-MM-GG. Esempio: 14/05/2025.\nUsa /salta per lasciare il campo vuoto.",
                )
                .await?;
            }
        },
        DraftField::PurchasePrice => match parse_money_to_cents(input) {
            Some(cents) => {
                draft.purchase_price_cents = Some(cents);
                sessions.set(
                    raw_chat_id,
                    ConversationState::EditingObject {
                        draft: Box::new(draft),
                        field: Some(DraftField::Seller),
                    },
                );
                bot.send_message(chat_id, field_prompt(DraftField::Seller))
                    .await?;
            }
            None => {
                bot.send_message(
                    chat_id,
                    "Prezzo non valido. Esempi validi: 89,90 oppure 89.90 oppure 89.\nUsa /salta per lasciare il campo vuoto.",
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
                    "Valore non valido. Esempi validi: 250 oppure 250,00.\nUsa /salta per lasciare il campo vuoto.",
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

    let next = match field {
        DraftField::Brand => Some(DraftField::Model),
        DraftField::PurchaseDate => Some(DraftField::PurchasePrice),
        DraftField::PurchasePrice => Some(DraftField::Seller),
        _ => None,
    };

    if let Some(next_field) = next {
        sessions.set(
            raw_chat_id,
            ConversationState::EditingObject {
                draft,
                field: Some(next_field),
            },
        );
        bot.send_message(chat_id, field_prompt(next_field)).await?;
    } else {
        finish_field(bot, chat_id, raw_chat_id, sessions, *draft).await?;
    }

    Ok(())
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

    match create_object(pool, &draft).await {
        Ok(id) => {
            sessions.clear_chat(raw_chat_id);
            bot.send_message(chat_id, format!("✅ Oggetto salvato con ID #{id}."))
                .await?;
            send_object_detail(bot, chat_id, pool, id).await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore durante il salvataggio dell'oggetto");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a salvare l'oggetto. La bozza resta aperta: puoi riprovare con ✅ Salva o usare /annulla.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn send_draft_panel(bot: &Bot, chat_id: ChatId, draft: &ObjectDraft) -> ResponseResult<()> {
    bot.send_message(chat_id, format_draft(draft))
        .reply_markup(draft_keyboard())
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
                if let Some(position) = &object.position {
                    text.push_str(&format!("\n📍 {position}"));
                }
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
                if let Some(position) = &object.position {
                    text.push_str(&format!("\n📍 {position}"));
                }
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

async fn send_object_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
) -> ResponseResult<()> {
    match get_object(pool, id).await {
        Ok(Some(object)) => {
            bot.send_message(chat_id, format_object(&object))
                .reply_markup(object_detail_keyboard())
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

async fn no_active_draft(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "Questa bozza non è più attiva. Avvia un nuovo inserimento da 📦 Oggetti.",
    )
    .reply_markup(objects_menu_keyboard())
    .await?;
    Ok(())
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
    tx.commit().await?;
    Ok(id)
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
            o.condizione AS condition, o.note AS notes \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
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
        "SELECT i.id AS id, i.nome AS name, o.posizione AS position \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
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
        "SELECT i.id AS id, i.nome AS name, o.posizione AS position \
         FROM items i \
         JOIN oggetti o ON o.item_id = i.id \
         WHERE i.tipo = 'oggetto' AND (\
            i.nome LIKE ? COLLATE NOCASE OR \
            o.marca LIKE ? COLLATE NOCASE OR \
            o.modello LIKE ? COLLATE NOCASE OR \
            o.numero_serie LIKE ? COLLATE NOCASE OR \
            o.posizione LIKE ? COLLATE NOCASE OR \
            o.venditore LIKE ? COLLATE NOCASE OR \
            o.descrizione LIKE ? COLLATE NOCASE OR \
            o.note LIKE ? COLLATE NOCASE\
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
    .bind(limit)
    .fetch_all(pool)
    .await
}

fn format_draft(draft: &ObjectDraft) -> String {
    let mut lines = vec![
        "📦 Nuovo oggetto".to_string(),
        String::new(),
        format!("Nome: {}", draft.name),
    ];

    push_optional_line(&mut lines, "Marca", draft.brand.as_deref());
    push_optional_line(&mut lines, "Modello", draft.model.as_deref());
    push_optional_line(&mut lines, "Posizione", draft.position.as_deref());
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
    lines.push("Aggiungi solo i dettagli che ti servono, poi premi ✅ Salva.".to_string());
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
    if let Some(position) = &object.position {
        lines.push(format!("📍 {position}"));
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
        vec![button("🏠 Menu principale", "menu:main")],
    ])
}

fn draft_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("🏷 Marca e modello", "oggetti:draft:brand")],
        vec![
            button("📍 Posizione", "oggetti:draft:position"),
            button("💶 Acquisto", "oggetti:draft:purchase"),
        ],
        vec![
            button("🛠 Condizione", "oggetti:draft:condition"),
            button("📝 Note", "oggetti:draft:notes"),
        ],
        vec![button("⋯ Altri dettagli", "oggetti:draft:other")],
        vec![
            button("✅ Salva", "oggetti:draft:save"),
            button("❌ Annulla", "oggetti:draft:cancel"),
        ],
    ])
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
        vec![button("⬅️ Dettagli", "oggetti:draft:back")],
    ])
}

fn other_details_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("📝 Descrizione", "oggetti:draft:description")],
        vec![button("💰 Valore stimato", "oggetti:draft:value")],
        vec![button("🔢 Numero seriale", "oggetti:draft:serial")],
        vec![button("⬅️ Dettagli", "oggetti:draft:back")],
    ])
}

fn cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button("❌ Annulla", "oggetti:draft:cancel")]])
}

fn object_detail_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("📋 Elenco", "oggetti:list:0")],
        vec![
            button("🔎 Cerca", "oggetti:search"),
            button("➕ Nuovo", "oggetti:new"),
        ],
        vec![button("📦 Menu oggetti", "oggetti:menu")],
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

fn field_prompt(field: DraftField) -> &'static str {
    match field {
        DraftField::Brand => "🏷 Inserisci la marca.\nUsa /salta per lasciarla vuota.",
        DraftField::Model => "🏷 Inserisci il modello.\nUsa /salta per lasciarlo vuoto.",
        DraftField::Position => {
            "📍 Dove si trova l'oggetto?\nEsempio: Garage - scaffale 2\nUsa /salta per lasciare vuoto."
        }
        DraftField::PurchaseDate => {
            "📅 Inserisci la data di acquisto (GG/MM/AAAA o AAAA-MM-GG).\nUsa /salta per lasciare vuoto."
        }
        DraftField::PurchasePrice => {
            "💶 Inserisci il prezzo pagato.\nEsempio: 89,90\nUsa /salta per lasciare vuoto."
        }
        DraftField::Seller => {
            "🏪 Inserisci negozio o venditore.\nEsempio: Amazon\nUsa /salta per lasciare vuoto."
        }
        DraftField::Notes => "📝 Inserisci le note.\nUsa /salta per lasciare vuoto.",
        DraftField::Description => {
            "📝 Inserisci una descrizione.\nUsa /salta per lasciare vuoto."
        }
        DraftField::EstimatedValue => {
            "💰 Inserisci il valore stimato attuale.\nEsempio: 250\nUsa /salta per lasciare vuoto."
        }
        DraftField::SerialNumber => {
            "🔢 Inserisci il numero seriale.\nUsa /salta per lasciare vuoto."
        }
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
}
