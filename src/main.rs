//! Punto di ingresso del Gestionale Casa.
//!
//! Step corrente: Step 7.2E, accesso controllato e backlog Miglioramenti.

mod access_control;
mod auth;
mod config;
mod db;
mod identity;
mod modules;
mod resource_permissions;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use config::Config;
use modules::{
    alimentazione::FoodSessionStore, contenitori::ContainerSessionStore, foto::PhotoSessionStore,
    luoghi::LocationSessionStore, miglioramenti::ImprovementSessionStore, oggetti::SessionStore,
};
use sqlx::SqlitePool;
use teloxide::{
    dptree,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, User},
};

#[derive(Clone, Default)]
struct IdentitySessionStore {
    inner: Arc<Mutex<HashMap<i64, IdentityConversationState>>>,
}

#[derive(Debug, Clone, Copy)]
enum IdentityConversationState {
    AwaitingNewSpaceName,
    AwaitingRenameSpaceName,
}

impl IdentitySessionStore {
    fn new() -> Self {
        Self::default()
    }

    fn get(&self, chat_id: i64) -> Option<IdentityConversationState> {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&chat_id)
            .copied()
    }

    fn set(&self, chat_id: i64, state: IdentityConversationState) {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(chat_id, state);
    }

    fn clear_chat(&self, chat_id: i64) {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&chat_id);
    }
}

const TOKIO_THREAD_STACK_SIZE: usize = 8 * 1024 * 1024;

fn main() -> anyhow::Result<()> {
    // Su Termux/Android gli handler Telegram più grandi possono avvicinarsi
    // al limite dello stack dei worker Tokio. Alimentazione è il modulo più
    // corposo: usiamo worker con stack esplicito e manteniamo i suoi future
    // pesanti boxed nei punti di dispatch, evitando crash nativi da stack
    // exhaustion (SIGSEGV) durante le await di rete.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(TOKIO_THREAD_STACK_SIZE)
        .build()
        .context("Impossibile inizializzare il runtime Tokio")?;

    runtime.block_on(Box::pin(async_main()))
}

async fn async_main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    tracing::info!(
        thread_stack_bytes = TOKIO_THREAD_STACK_SIZE,
        "Runtime Tokio inizializzato con stack rinforzato"
    );

    let config = Arc::new(Config::load()?);
    tracing::info!(
        authorized_chats = config.allowed_chat_ids.len(),
        "Configurazione caricata"
    );

    let pool = db::connect(&config.database_url).await?;
    let database_status = db::status(&pool).await?;
    tracing::info!(
        applied_migrations = database_status.applied_migrations,
        schema_core = database_status.schema_core_present,
        shared_foundations = database_status.shared_foundations_present,
        operational_spaces = database_status.operational_spaces_present,
        multi_space_view = database_status.multi_space_view_present,
        system_roles = database_status.system_roles_present,
        access_improvements = database_status.access_improvements_present,
        product_formats = database_status.product_formats_present,
        "Database SQLite pronto"
    );

    let bot = Bot::new(config.telegram_token.clone());
    let me = bot
        .get_me()
        .await
        .context("Impossibile collegarsi al bot Telegram")?;
    tracing::info!(bot_username = ?me.username(), "Gestionale Casa online");

    // Le notifiche tecniche di avvio sono riservate agli amministratori del
    // gestionale. Gli utenti normali non devono ricevere messaggi operativi
    // legati al runtime del bot.
    let admin_chat_ids = match identity::list_system_admin_chat_ids(&pool).await {
        Ok(chat_ids) => chat_ids,
        Err(error) => {
            tracing::warn!(?error, "Impossibile leggere gli amministratori all'avvio");
            Vec::new()
        }
    };
    for chat_id in admin_chat_ids {
        if let Err(error) = send_online_menu(&bot, ChatId(chat_id)).await {
            tracing::warn!(
                chat_id,
                ?error,
                "Impossibile inviare la notifica di avvio all'amministratore"
            );
        }
    }

    let sessions = SessionStore::new();
    let location_sessions = LocationSessionStore::new();
    let container_sessions = ContainerSessionStore::new();
    let photo_sessions = PhotoSessionStore::new();
    let food_sessions = FoodSessionStore::new();
    let improvement_sessions = ImprovementSessionStore::new();
    let identity_sessions = IdentitySessionStore::new();
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    // Il dispatcher prende possesso di bot/config. Conserviamo cio' che serve
    // per notificare uno shutdown controllato ai soli amministratori.
    let shutdown_bot = bot.clone();
    let shutdown_pool = pool.clone();

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            config,
            pool,
            sessions,
            location_sessions,
            container_sessions,
            photo_sessions,
            food_sessions,
            improvement_sessions,
            identity_sessions
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    let shutdown_chat_ids = match identity::list_system_admin_chat_ids(&shutdown_pool).await {
        Ok(chat_ids) => chat_ids,
        Err(error) => {
            tracing::warn!(
                ?error,
                "Impossibile leggere gli amministratori allo spegnimento"
            );
            Vec::new()
        }
    };
    for chat_id in shutdown_chat_ids {
        if let Err(error) = shutdown_bot
            .send_message(ChatId(chat_id), "🔴 Gestionale Casa è offline.")
            .await
        {
            tracing::warn!(
                chat_id,
                ?error,
                "Impossibile inviare la notifica di spegnimento all'amministratore"
            );
        }
    }

    tracing::info!("Gestionale Casa offline");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_message(
    bot: Bot,
    msg: Message,
    config: Arc<Config>,
    pool: SqlitePool,
    sessions: SessionStore,
    location_sessions: LocationSessionStore,
    container_sessions: ContainerSessionStore,
    photo_sessions: PhotoSessionStore,
    food_sessions: FoodSessionStore,
    improvement_sessions: ImprovementSessionStore,
    identity_sessions: IdentitySessionStore,
) -> ResponseResult<()> {
    let chat_id = msg.chat.id.0;

    let Some(sender) = msg.from.as_ref() else {
        tracing::warn!(chat_id, "Messaggio senza autore Telegram");
        bot.send_message(
            msg.chat.id,
            "⚠️ Non riesco a identificare l'autore Telegram di questo messaggio.",
        )
        .await?;
        return respond(());
    };

    let actor = match identity::lookup_telegram_actor(&pool, chat_id, sender).await {
        Ok(Some(actor)) => actor,
        Ok(None) if auth::is_authorized(chat_id, &config.allowed_chat_ids) => {
            // Bootstrap/emergenza: una chat esplicitamente configurata può
            // inizializzare il primo account anche su un database nuovo.
            match identity::resolve_telegram_actor(&pool, chat_id, sender).await {
                Ok(actor) => actor,
                Err(error) => {
                    tracing::error!(chat_id, ?error, "Errore bootstrap identità Telegram");
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Non riesco a collegare il tuo account Telegram al profilo del gestionale.",
                    )
                    .await?;
                    return respond(());
                }
            }
        }
        Ok(None) => {
            handle_unapproved_message(&bot, &msg, &pool, sender).await?;
            return respond(());
        }
        Err(error) => {
            tracing::warn!(chat_id, ?error, "Accesso Telegram non disponibile");
            bot.send_message(
                msg.chat.id,
                "🔒 Questo account non può usare il gestionale in questo momento.",
            )
            .await?;
            return respond(());
        }
    };

    identity::with_actor(
        actor.clone(),
        Box::pin(handle_authorized_message(
            bot,
            msg,
            pool,
            sessions,
            location_sessions,
            container_sessions,
            photo_sessions,
            food_sessions,
            improvement_sessions,
            identity_sessions,
            actor,
        )),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn handle_authorized_message(
    bot: Bot,
    msg: Message,
    pool: SqlitePool,
    sessions: SessionStore,
    location_sessions: LocationSessionStore,
    container_sessions: ContainerSessionStore,
    photo_sessions: PhotoSessionStore,
    food_sessions: FoodSessionStore,
    improvement_sessions: ImprovementSessionStore,
    identity_sessions: IdentitySessionStore,
    actor: identity::AuditActor,
) -> ResponseResult<()> {
    let chat_id = msg.chat.id.0;

    // Miglioramenti gestisce anche foto/screenshot, quindi ha priorità sul
    // modulo Foto quando esiste una bozza attiva.
    if modules::miglioramenti::handle_message(&bot, &msg, &pool, &improvement_sessions, msg.text())
        .await?
    {
        sessions.clear_chat(chat_id);
        location_sessions.clear_chat(chat_id);
        container_sessions.clear_chat(chat_id);
        photo_sessions.clear_chat(chat_id);
        food_sessions.clear_chat(chat_id);
        identity_sessions.clear_chat(chat_id);
        return respond(());
    }

    // Se si entra esplicitamente nel flusso foto da comando, chiudiamo una
    // eventuale bozza oggetto rimasta aperta per evitare stati concorrenti.
    if matches!(
        msg.text().and_then(first_command),
        Some("/foto") | Some("/foto_aggiungi")
    ) {
        sessions.clear_chat(chat_id);
        location_sessions.clear_chat(chat_id);
        container_sessions.clear_chat(chat_id);
    }

    // I comandi foto e, soprattutto, le foto vere e proprie devono essere
    // gestiti prima del controllo msg.text(), perche' una foto non e' testo.
    if modules::foto::handle_message(&bot, &msg, &pool, &photo_sessions).await? {
        return respond(());
    }

    let Some(text) = msg.text() else {
        if msg.photo().is_some() {
            bot.send_message(
                msg.chat.id,
                "📷 Non sto aspettando una foto. Apri la scheda di un oggetto e usa 📷 Foto → ➕ Aggiungi foto.",
            )
            .await?;
        }
        return respond(());
    };

    let command = first_command(text);

    // Qualunque altro comando esplicito interrompe un'eventuale attesa foto:
    // evita che una foto inviata piu' tardi venga associata per errore.
    if command.is_some() {
        photo_sessions.clear_chat(chat_id);
        improvement_sessions.clear_chat(chat_id);
        if command != Some("/spazio_nuovo")
            && command != Some("/spazio_rinomina")
            && command != Some("/annulla")
        {
            identity_sessions.clear_chat(chat_id);
        }
    }

    if command == Some("/annulla") && identity_sessions.get(chat_id).is_some() {
        identity_sessions.clear_chat(chat_id);
        send_spaces(&bot, msg.chat.id, &pool, &actor).await?;
        return respond(());
    }

    if command.is_none() {
        if let Some(state) = identity_sessions.get(chat_id) {
            let result = match state {
                IdentityConversationState::AwaitingNewSpaceName => {
                    identity::create_space(&pool, &actor, text, "condiviso")
                        .await
                        .map(|space| {
                            format!(
                                "✅ Spazio creato e impostato come predefinito: {}",
                                space.nome
                            )
                        })
                }
                IdentityConversationState::AwaitingRenameSpaceName => {
                    identity::rename_active_space(&pool, &actor, text)
                        .await
                        .map(|name| format!("✅ Spazio predefinito rinominato: {name}"))
                }
            };
            match result {
                Ok(message) => {
                    identity_sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, message)
                        .reply_markup(profile_keyboard())
                        .await?;
                }
                Err(error) => {
                    tracing::warn!(?error, "Operazione spazio guidata non riuscita");
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nRiprova oppure usa /annulla."),
                    )
                    .reply_markup(space_flow_keyboard())
                    .await?;
                }
            }
            return respond(());
        }
    }

    // Box intenzionale: Alimentazione ha un future molto grande; tenerlo
    // fuori dal frame del dispatcher riduce la pressione sullo stack.
    if Box::pin(modules::alimentazione::handle_message(
        &bot,
        &msg,
        &pool,
        &food_sessions,
        text,
    ))
    .await?
    {
        sessions.clear_chat(chat_id);
        location_sessions.clear_chat(chat_id);
        container_sessions.clear_chat(chat_id);
        photo_sessions.clear_chat(chat_id);
        identity_sessions.clear_chat(chat_id);
        return respond(());
    }

    if modules::contenitori::handle_message(&bot, &msg, &pool, &container_sessions, text).await? {
        sessions.clear_chat(chat_id);
        location_sessions.clear_chat(chat_id);
        return respond(());
    }

    if modules::luoghi::handle_message(&bot, &msg, &pool, &location_sessions, text).await? {
        sessions.clear_chat(chat_id);
        container_sessions.clear_chat(chat_id);
        return respond(());
    }

    if modules::oggetti::handle_message(&bot, &msg, &pool, &sessions, text).await? {
        location_sessions.clear_chat(chat_id);
        container_sessions.clear_chat(chat_id);
        return respond(());
    }

    match command {
        Some("/start") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            container_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            improvement_sessions.clear_chat(chat_id);
            send_main_menu(&bot, msg.chat.id, &pool, &actor).await?;
        }
        Some("/ping") => {
            bot.send_message(msg.chat.id, "Pong! Gestionale Casa è online.")
                .await?;
        }
        Some("/storico") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            container_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            modules::storico::show_global_history(&bot, msg.chat.id, &pool, 0).await?;
        }
        Some("/profilo") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            container_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            send_profile(&bot, msg.chat.id, &pool, &actor).await?;
        }
        Some("/spazi") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            container_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            send_spaces(&bot, msg.chat.id, &pool, &actor).await?;
        }
        Some("/spazio_nuovo") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            container_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            let name = command_args(text);
            if name.is_empty() {
                identity_sessions.set(chat_id, IdentityConversationState::AwaitingNewSpaceName);
                bot.send_message(
                    msg.chat.id,
                    "➕ Nuovo spazio\n\nScrivi il nome del nuovo spazio.\nPuoi usare /annulla per uscire.",
                )
                .reply_markup(space_flow_keyboard())
                .await?;
            } else {
                match identity::create_space(&pool, &actor, name, "condiviso").await {
                    Ok(space) => {
                        bot.send_message(
                            msg.chat.id,
                            format!(
                                "✅ Spazio creato e attivato: {}\n\nDa questo momento le sezioni del gestionale usano questo spazio.",
                                space.nome
                            ),
                        )
                        .reply_markup(profile_keyboard())
                        .await?;
                    }
                    Err(error) => {
                        tracing::warn!(?error, "Creazione spazio non riuscita");
                        bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                            .reply_markup(profile_keyboard())
                            .await?;
                    }
                }
            }
        }
        Some("/spazio_rinomina") => {
            sessions.clear_chat(chat_id);
            location_sessions.clear_chat(chat_id);
            container_sessions.clear_chat(chat_id);
            photo_sessions.clear_chat(chat_id);
            let name = command_args(text);
            if name.is_empty() {
                identity_sessions.set(chat_id, IdentityConversationState::AwaitingRenameSpaceName);
                bot.send_message(
                    msg.chat.id,
                    format!("✏️ Rinomina spazio\n\nSpazio predefinito attuale: {}\nScrivi il nuovo nome oppure /annulla.", actor.spazio_nome_snapshot),
                )
                    .reply_markup(space_flow_keyboard())
                    .await?;
            } else {
                match identity::rename_active_space(&pool, &actor, name).await {
                    Ok(name) => {
                        bot.send_message(msg.chat.id, format!("✅ Spazio rinominato: {name}"))
                            .reply_markup(profile_keyboard())
                            .await?;
                    }
                    Err(error) => {
                        tracing::warn!(?error, "Rinomina spazio non riuscita");
                        bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                            .reply_markup(profile_keyboard())
                            .await?;
                    }
                }
            }
        }
        Some("/vista_tutti") => match identity::set_view_all(&pool, &actor, true).await {
            Ok(()) => {
                bot.send_message(msg.chat.id, "🌐 Vista impostata su: tutti i miei spazi.")
                    .reply_markup(profile_keyboard())
                    .await?;
            }
            Err(error) => {
                bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                    .reply_markup(profile_keyboard())
                    .await?;
            }
        },
        Some("/vista_spazio") => match identity::set_view_all(&pool, &actor, false).await {
            Ok(()) => {
                bot.send_message(
                    msg.chat.id,
                    "🎯 Vista impostata su: solo spazio predefinito.",
                )
                .reply_markup(profile_keyboard())
                .await?;
            }
            Err(error) => {
                bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                    .reply_markup(profile_keyboard())
                    .await?;
            }
        },
        Some("/admin") => {
            send_admin_menu(&bot, msg.chat.id, &pool, &actor).await?;
        }
        Some("/status") => {
            send_status(&bot, msg.chat.id, &pool, &actor).await?;
        }
        Some(_) => {
            bot.send_message(
                msg.chat.id,
                "Comando non riconosciuto.\nUsa /start per aprire il menu principale.",
            )
            .await?;
        }
        None => {
            bot.send_message(
                msg.chat.id,
                "Non c'è un'operazione attiva. Usa /start oppure i pulsanti del menu.",
            )
            .await?;
        }
    }

    respond(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    config: Arc<Config>,
    pool: SqlitePool,
    sessions: SessionStore,
    location_sessions: LocationSessionStore,
    container_sessions: ContainerSessionStore,
    photo_sessions: PhotoSessionStore,
    food_sessions: FoodSessionStore,
    improvement_sessions: ImprovementSessionStore,
    identity_sessions: IdentitySessionStore,
) -> ResponseResult<()> {
    bot.answer_callback_query(q.id.clone()).await?;

    let Some(message) = q.regular_message() else {
        return respond(());
    };
    let chat_id = message.chat.id;
    let Some(data) = q.data.clone() else {
        return respond(());
    };

    let actor = match identity::lookup_telegram_actor(&pool, chat_id.0, &q.from).await {
        Ok(Some(actor)) => actor,
        Ok(None) if auth::is_authorized(chat_id.0, &config.allowed_chat_ids) => {
            match identity::resolve_telegram_actor(&pool, chat_id.0, &q.from).await {
                Ok(actor) => actor,
                Err(error) => {
                    tracing::error!(
                        chat_id = chat_id.0,
                        ?error,
                        "Errore bootstrap identità callback"
                    );
                    return respond(());
                }
            }
        }
        Ok(None) => {
            handle_unapproved_callback(&bot, chat_id, &pool, &q.from, &data).await?;
            return respond(());
        }
        Err(error) => {
            tracing::warn!(
                chat_id = chat_id.0,
                ?error,
                "Callback senza accesso applicativo"
            );
            bot.send_message(
                chat_id,
                "🔒 Questo account non può usare il gestionale in questo momento.",
            )
            .await?;
            return respond(());
        }
    };

    identity::with_actor(
        actor.clone(),
        Box::pin(handle_authorized_callback(
            bot,
            chat_id,
            pool,
            sessions,
            location_sessions,
            container_sessions,
            photo_sessions,
            food_sessions,
            improvement_sessions,
            identity_sessions,
            actor,
            data,
        )),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn handle_authorized_callback(
    bot: Bot,
    chat_id: ChatId,
    pool: SqlitePool,
    sessions: SessionStore,
    location_sessions: LocationSessionStore,
    container_sessions: ContainerSessionStore,
    photo_sessions: PhotoSessionStore,
    food_sessions: FoodSessionStore,
    improvement_sessions: ImprovementSessionStore,
    identity_sessions: IdentitySessionStore,
    actor: identity::AuditActor,
    data: String,
) -> ResponseResult<()> {
    let data = data.as_str();

    if data.starts_with("improve:")
        && modules::miglioramenti::handle_callback(
            &bot,
            chat_id,
            &pool,
            &improvement_sessions,
            data,
        )
        .await?
    {
        sessions.clear_chat(chat_id.0);
        location_sessions.clear_chat(chat_id.0);
        container_sessions.clear_chat(chat_id.0);
        photo_sessions.clear_chat(chat_id.0);
        food_sessions.clear_chat(chat_id.0);
        identity_sessions.clear_chat(chat_id.0);
        return respond(());
    }

    if data.starts_with("food:") || (data == "menu:main" && food_sessions.has_active(chat_id.0)) {
        // Box intenzionale: evita che il grande future di Alimentazione
        // venga inglobato nello stack frame del dispatcher callback.
        if Box::pin(modules::alimentazione::handle_callback(
            &bot,
            chat_id,
            &pool,
            &food_sessions,
            data,
        ))
        .await?
        {
            sessions.clear_chat(chat_id.0);
            location_sessions.clear_chat(chat_id.0);
            container_sessions.clear_chat(chat_id.0);
            photo_sessions.clear_chat(chat_id.0);
            identity_sessions.clear_chat(chat_id.0);
            return respond(());
        }
    } else {
        food_sessions.clear_chat(chat_id.0);
    }

    match data {
        "menu:main" => {
            sessions.clear_chat(chat_id.0);
            location_sessions.clear_chat(chat_id.0);
            container_sessions.clear_chat(chat_id.0);
            photo_sessions.clear_chat(chat_id.0);
            food_sessions.clear_chat(chat_id.0);
            improvement_sessions.clear_chat(chat_id.0);
            send_main_menu(&bot, chat_id, &pool, &actor).await?;
        }
        "menu:soon" => {
            bot.send_message(
                chat_id,
                "Questo modulo non è ancora implementato. Per ora sono disponibili 🏷️ Oggetti e 🏠 Case, stanze e contenitori.",
            )
            .await?;
        }
        "identity:profile" => {
            send_profile(&bot, chat_id, &pool, &actor).await?;
        }
        "identity:spaces" => {
            sessions.clear_chat(chat_id.0);
            location_sessions.clear_chat(chat_id.0);
            container_sessions.clear_chat(chat_id.0);
            photo_sessions.clear_chat(chat_id.0);
            send_spaces(&bot, chat_id, &pool, &actor).await?;
        }
        "identity:space:new" => {
            identity_sessions.set(chat_id.0, IdentityConversationState::AwaitingNewSpaceName);
            bot.send_message(
                chat_id,
                "➕ Nuovo spazio\n\nScrivi il nome del nuovo spazio.\nPuoi usare /annulla per uscire.",
            )
            .reply_markup(space_flow_keyboard())
            .await?;
        }
        "identity:space:rename" => {
            identity_sessions.set(
                chat_id.0,
                IdentityConversationState::AwaitingRenameSpaceName,
            );
            bot.send_message(
                chat_id,
                format!("✏️ Rinomina spazio\n\nSpazio predefinito attuale: {}\nScrivi il nuovo nome oppure /annulla.", actor.spazio_nome_snapshot),
            )
            .reply_markup(space_flow_keyboard())
            .await?;
        }
        "identity:view:all" => {
            if let Err(error) = identity::set_view_all(&pool, &actor, true).await {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            } else {
                bot.send_message(chat_id, "🌐 Ora visualizzi tutti i tuoi spazi.")
                    .reply_markup(profile_keyboard())
                    .await?;
            }
        }
        "identity:view:default" => {
            if let Err(error) = identity::set_view_all(&pool, &actor, false).await {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            } else {
                bot.send_message(chat_id, "🎯 Ora visualizzi solo lo spazio predefinito.")
                    .reply_markup(profile_keyboard())
                    .await?;
            }
        }
        _ if data.starts_with("identity:space:") => {
            sessions.clear_chat(chat_id.0);
            location_sessions.clear_chat(chat_id.0);
            container_sessions.clear_chat(chat_id.0);
            photo_sessions.clear_chat(chat_id.0);
            let target = data
                .strip_prefix("identity:space:")
                .and_then(|value| value.parse::<i64>().ok());
            match target {
                Some(space_id) => {
                    match identity::switch_active_space(&pool, &actor, space_id).await {
                        Ok(space) => {
                            bot.send_message(
                                chat_id,
                                format!("⭐ Spazio predefinito: {}", space.nome),
                            )
                            .reply_markup(profile_keyboard())
                            .await?;
                        }
                        Err(error) => {
                            tracing::warn!(?error, space_id, "Cambio spazio non riuscito");
                            bot.send_message(
                                chat_id,
                                "⚠️ Spazio non disponibile per questo account.",
                            )
                            .reply_markup(profile_keyboard())
                            .await?;
                        }
                    }
                }
                None => {
                    bot.send_message(chat_id, "Pulsante spazio non valido.")
                        .reply_markup(profile_keyboard())
                        .await?;
                }
            }
        }
        "admin:menu" => {
            send_admin_menu(&bot, chat_id, &pool, &actor).await?;
        }
        "admin:overview" => {
            send_admin_overview(&bot, chat_id, &pool, &actor).await?;
        }
        "admin:users" => {
            send_admin_users(&bot, chat_id, &pool, &actor).await?;
        }
        "admin:access" => {
            send_admin_access_requests(&bot, chat_id, &pool, &actor).await?;
        }
        _ if data.starts_with("admin:access:view:") => {
            if let Some(request_id) = data
                .strip_prefix("admin:access:view:")
                .and_then(|value| value.parse::<i64>().ok())
            {
                send_admin_access_request_detail(&bot, chat_id, &pool, &actor, request_id).await?;
            } else {
                bot.send_message(chat_id, "⚠️ Richiesta non valida.")
                    .await?;
            }
        }
        _ if data.starts_with("admin:access:approve:") => {
            if let Some(request_id) = data
                .strip_prefix("admin:access:approve:")
                .and_then(|value| value.parse::<i64>().ok())
            {
                approve_access_request_ui(&bot, chat_id, &pool, &actor, request_id).await?;
            } else {
                bot.send_message(chat_id, "⚠️ Richiesta non valida.")
                    .await?;
            }
        }
        _ if data.starts_with("admin:access:reject:") => {
            if let Some(request_id) = data
                .strip_prefix("admin:access:reject:")
                .and_then(|value| value.parse::<i64>().ok())
            {
                reject_access_request_ui(&bot, chat_id, &pool, &actor, request_id).await?;
            } else {
                bot.send_message(chat_id, "⚠️ Richiesta non valida.")
                    .await?;
            }
        }
        "admin:status" | "system:status" => {
            send_status(&bot, chat_id, &pool, &actor).await?;
        }
        _ if data.starts_with("history:") || data.starts_with("h:") => {
            sessions.clear_chat(chat_id.0);
            location_sessions.clear_chat(chat_id.0);
            container_sessions.clear_chat(chat_id.0);
            photo_sessions.clear_chat(chat_id.0);
            if !modules::storico::handle_callback(&bot, chat_id, &pool, data).await? {
                bot.send_message(
                    chat_id,
                    "Pulsante storico non riconosciuto o non più valido.",
                )
                .await?;
            }
        }
        _ => {
            if data.starts_with("oggetti:") {
                photo_sessions.clear_chat(chat_id.0);
            }

            if modules::foto::handle_callback(&bot, chat_id, &pool, &photo_sessions, data).await? {
                location_sessions.clear_chat(chat_id.0);
                container_sessions.clear_chat(chat_id.0);
                return respond(());
            }

            if modules::contenitori::handle_callback(
                &bot,
                chat_id,
                &pool,
                &container_sessions,
                data,
            )
            .await?
            {
                sessions.clear_chat(chat_id.0);
                location_sessions.clear_chat(chat_id.0);
                photo_sessions.clear_chat(chat_id.0);
                return respond(());
            }

            if modules::luoghi::handle_callback(&bot, chat_id, &pool, &location_sessions, data)
                .await?
            {
                sessions.clear_chat(chat_id.0);
                container_sessions.clear_chat(chat_id.0);
                photo_sessions.clear_chat(chat_id.0);
                return respond(());
            }

            if !modules::oggetti::handle_callback(&bot, chat_id, &pool, &sessions, data).await? {
                bot.send_message(chat_id, "Pulsante non riconosciuto o non più valido.")
                    .await?;
            } else {
                location_sessions.clear_chat(chat_id.0);
                container_sessions.clear_chat(chat_id.0);
            }
        }
    }

    respond(())
}

async fn send_online_menu(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "🟢 Gestionale Casa è online.\n\n🏠 Menu principale\nScegli una sezione.",
    )
    .reply_markup(modules::oggetti::main_menu_keyboard(true))
    .await?;
    Ok(())
}

async fn send_main_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    let is_admin = match identity::is_system_admin(pool, actor).await {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(
                ?error,
                "Errore verifica ruolo amministratore nel menu principale"
            );
            false
        }
    };
    bot.send_message(
        chat_id,
        "🏠 Gestionale Casa\n\nScegli una sezione. I moduli non ancora disponibili sono indicati come prossimamente.",
    )
    .reply_markup(modules::oggetti::main_menu_keyboard(is_admin))
    .await?;
    Ok(())
}

async fn send_profile(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    match identity::profile_summary(pool, actor).await {
        Ok(summary) => {
            bot.send_message(chat_id, summary)
                .reply_markup(profile_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore lettura profilo Step 7");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere il profilo corrente.")
                .reply_markup(profile_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn send_spaces(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    let Some(user_id) = actor.utente_id else {
        bot.send_message(
            chat_id,
            "⚠️ Spazi non disponibili per un attore di sistema.",
        )
        .reply_markup(profile_keyboard())
        .await?;
        return Ok(());
    };

    match identity::list_user_spaces(pool, user_id).await {
        Ok(spaces) => {
            let summary = identity::spaces_summary(pool, actor)
                .await
                .unwrap_or_else(|_| "👥 Spazi".to_string());
            let mut rows = Vec::new();
            for space in spaces {
                let marker = if space.attivo != 0 { "⭐" } else { "○" };
                rows.push(vec![InlineKeyboardButton::callback(
                    format!("{marker} {}", space.nome),
                    format!("identity:space:{}", space.id),
                )]);
            }
            rows.push(vec![
                InlineKeyboardButton::callback(
                    if actor.view_all {
                        "✅ 🌐 Tutti i miei spazi"
                    } else {
                        "🌐 Tutti i miei spazi"
                    }
                    .to_string(),
                    "identity:view:all".to_string(),
                ),
                InlineKeyboardButton::callback(
                    if actor.view_all {
                        "🎯 Solo predefinito"
                    } else {
                        "✅ 🎯 Solo predefinito"
                    }
                    .to_string(),
                    "identity:view:default".to_string(),
                ),
            ]);
            rows.push(vec![
                InlineKeyboardButton::callback(
                    "➕ Nuovo spazio".to_string(),
                    "identity:space:new".to_string(),
                ),
                InlineKeyboardButton::callback(
                    "✏️ Rinomina".to_string(),
                    "identity:space:rename".to_string(),
                ),
            ]);
            rows.push(vec![InlineKeyboardButton::callback(
                "👤 Profilo".to_string(),
                "identity:profile".to_string(),
            )]);
            rows.push(vec![InlineKeyboardButton::callback(
                "🏠 Menu principale".to_string(),
                "menu:main".to_string(),
            )]);

            bot.send_message(chat_id, summary)
                .reply_markup(InlineKeyboardMarkup::new(rows))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore lettura spazi");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere gli spazi disponibili.")
                .reply_markup(profile_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn ensure_admin_access(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<bool> {
    match identity::is_system_admin(pool, actor).await {
        Ok(true) => Ok(true),
        Ok(false) => {
            bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                .await?;
            Ok(false)
        }
        Err(error) => {
            tracing::error!(?error, "Errore verifica ruolo amministratore");
            bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                .await?;
            Ok(false)
        }
    }
}

async fn send_admin_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    if !ensure_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }
    let primary = identity::is_primary_admin(pool, actor)
        .await
        .unwrap_or(false);
    let pending = if primary {
        access_control::pending_count(pool).await.unwrap_or(0)
    } else {
        0
    };
    bot.send_message(
        chat_id,
        "🛠️ Amministrazione\n\nArea riservata per monitorare il gestionale. I ruoli di sistema sono separati dai permessi negli spazi e sulle singole risorse.",
    )
    .reply_markup(admin_menu_keyboard(primary, pending))
    .await?;
    Ok(())
}

async fn send_admin_overview(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    if !ensure_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }

    let counts = async {
        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM utenti WHERE stato = 'attivo'")
            .fetch_one(pool)
            .await?;
        let spaces: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spazi")
            .fetch_one(pool)
            .await?;
        let homes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM abitazioni")
            .fetch_one(pool)
            .await?;
        let items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
            .fetch_one(pool)
            .await?;
        let foods: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM alimenti WHERE archiviato = 0")
            .fetch_one(pool)
            .await?;
        let recipes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ricette WHERE archiviata = 0")
            .fetch_one(pool)
            .await?;
        let history: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM storico_eventi")
            .fetch_one(pool)
            .await?;
        let improvements: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti")
            .fetch_one(pool)
            .await?;
        let pending_access: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM richieste_accesso WHERE stato = 'pendente'")
                .fetch_one(pool)
                .await?;
        Ok::<_, sqlx::Error>((
            users,
            spaces,
            homes,
            items,
            foods,
            recipes,
            history,
            improvements,
            pending_access,
        ))
    }
    .await;

    match counts {
        Ok((
            users,
            spaces,
            homes,
            items,
            foods,
            recipes,
            history,
            improvements,
            pending_access,
        )) => {
            bot.send_message(
                chat_id,
                format!(
                    "🧭 Panoramica gestionale\n\n👥 Utenti attivi: {users}\n📨 Richieste accesso pendenti: {pending_access}\n🌐 Spazi: {spaces}\n🏠 Case: {homes}\n🏷️ Oggetti: {items}\n🥕 Alimenti: {foods}\n🍳 Ricette: {recipes}\n💡 Miglioramenti: {improvements}\n📜 Eventi nello storico: {history}"
                ),
            )
            .reply_markup(admin_back_keyboard())
            .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore panoramica amministrativa");
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a leggere la panoramica del gestionale.",
            )
            .reply_markup(admin_back_keyboard())
            .await?;
        }
    }
    Ok(())
}

async fn send_admin_users(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    if !ensure_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }

    match identity::list_system_users(pool).await {
        Ok(users) => {
            let mut lines = vec!["👥 Utenti del gestionale".to_string(), String::new()];
            for user in users {
                let icon = if user.ruolo_sistema == identity::SYSTEM_ROLE_ADMIN {
                    "🛡️"
                } else {
                    "👤"
                };
                let role = if user.ruolo_sistema == identity::SYSTEM_ROLE_ADMIN {
                    "Amministratore"
                } else {
                    "Utente"
                };
                let telegram = user
                    .telegram_username
                    .as_deref()
                    .map(|value| format!("@{value}"))
                    .unwrap_or_else(|| "account Telegram collegato".to_string());
                lines.push(format!(
                    "{icon} {}\n   Ruolo sistema: {role}\n   Stato: {}\n   Telegram: {telegram}\n   Spazi: {}",
                    user.nome,
                    if user.stato == "attivo" { "Attivo" } else { "Disabilitato" },
                    user.numero_spazi
                ));
                lines.push(String::new());
            }
            bot.send_message(chat_id, lines.join("\n"))
                .reply_markup(admin_back_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco utenti amministrativo");
            bot.send_message(
                chat_id,
                "⚠️ Non riesco a leggere gli utenti del gestionale.",
            )
            .reply_markup(admin_back_keyboard())
            .await?;
        }
    }
    Ok(())
}

async fn send_status(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    if !ensure_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }

    match db::status(pool).await {
        Ok(status) => {
            let fk = if status.foreign_keys_enabled {
                "✅"
            } else {
                "❌"
            };
            let schema = if status.schema_core_present {
                "✅"
            } else {
                "❌"
            };
            let shared = if status.shared_foundations_present {
                "✅"
            } else {
                "❌"
            };
            let operational = if status.operational_spaces_present {
                "✅"
            } else {
                "❌"
            };
            let multi_view = if status.multi_space_view_present {
                "✅"
            } else {
                "❌"
            };
            let system_roles = if status.system_roles_present {
                "✅"
            } else {
                "❌"
            };
            let access_improvements = if status.access_improvements_present {
                "✅"
            } else {
                "❌"
            };
            let product_formats = if status.product_formats_present {
                "✅"
            } else {
                "❌"
            };
            let message = format!(
                "📊 Stato sistema\n\n\
                 Bot Telegram: ✅\n\
                 Database SQLite: ✅\n\
                 Foreign key: {fk}\n\
                 Migrazioni applicate: {}\n\
                 Schema core: {schema}\n\
                 Fondazioni condivise Step 7: {shared}\n\
                 Isolamento multi-spazio: {operational}\n\
                 Vista multi-spazio Step 7.1B: {multi_view}\n\
                 Ruoli di sistema: {system_roles}\n\
                 Accesso controllato + Miglioramenti: {access_improvements}\n\
                 Formati prodotto: {product_formats}",
                status.applied_migrations
            );
            bot.send_message(chat_id, message)
                .reply_markup(admin_back_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore durante la lettura dello stato");
            bot.send_message(
                chat_id,
                "⚠️ Il bot è online, ma non riesco a leggere lo stato del database.",
            )
            .reply_markup(admin_back_keyboard())
            .await?;
        }
    }
    Ok(())
}

async fn handle_unapproved_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    user: &User,
) -> ResponseResult<()> {
    let command = msg.text().and_then(first_command);
    if command == Some("/richiedi_accesso") {
        match access_control::submit_request(pool, msg.chat.id.0, user).await {
            Ok(request_id) => {
                bot.send_message(
                    msg.chat.id,
                    "📨 Richiesta di accesso inviata.\n\nL'amministratore principale deve approvarla prima che tu possa usare il gestionale.",
                )
                .reply_markup(access_pending_keyboard())
                .await?;
                notify_primary_admin_access_request(bot, pool, user, request_id).await;
            }
            Err(error) => {
                tracing::warn!(?error, "Richiesta di accesso non riuscita");
                send_access_gate(bot, msg.chat.id, pool, user).await?;
            }
        }
    } else {
        send_access_gate(bot, msg.chat.id, pool, user).await?;
    }
    Ok(())
}

async fn handle_unapproved_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    user: &User,
    data: &str,
) -> ResponseResult<()> {
    match data {
        "access:request" => match access_control::submit_request(pool, chat_id.0, user).await {
            Ok(request_id) => {
                bot.send_message(
                    chat_id,
                    "📨 Richiesta di accesso inviata.\n\nRiceverai accesso solo dopo l'approvazione dell'amministratore principale.",
                )
                .reply_markup(access_pending_keyboard())
                .await?;
                notify_primary_admin_access_request(bot, pool, user, request_id).await;
            }
            Err(error) => {
                tracing::warn!(?error, "Richiesta di accesso da callback non riuscita");
                send_access_gate(bot, chat_id, pool, user).await?;
            }
        },
        "access:status" => {
            send_access_gate(bot, chat_id, pool, user).await?;
        }
        _ => {
            send_access_gate(bot, chat_id, pool, user).await?;
        }
    }
    Ok(())
}

async fn send_access_gate(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    user: &User,
) -> ResponseResult<()> {
    use access_control::AccessRequestState;
    let state = access_control::request_state(pool, user)
        .await
        .unwrap_or(AccessRequestState::Unknown);
    match state {
        AccessRequestState::Pending => {
            bot.send_message(
                chat_id,
                "⏳ Richiesta di accesso in attesa.\n\nL'amministratore principale non l'ha ancora approvata.",
            )
            .reply_markup(access_pending_keyboard())
            .await?;
        }
        AccessRequestState::Rejected => {
            bot.send_message(
                chat_id,
                "🔒 Accesso non autorizzato.\n\nLa richiesta precedente è stata rifiutata. Se necessario puoi inviarne una nuova.",
            )
            .reply_markup(access_request_keyboard())
            .await?;
        }
        AccessRequestState::Approved => {
            bot.send_message(
                chat_id,
                "✅ La richiesta risulta approvata. Usa /start per entrare nel gestionale.",
            )
            .await?;
        }
        AccessRequestState::Unknown => {
            bot.send_message(
                chat_id,
                "🔒 Questo account Telegram non è ancora autorizzato.\n\nPuoi chiedere l'accesso all'amministratore principale.",
            )
            .reply_markup(access_request_keyboard())
            .await?;
        }
    }
    Ok(())
}

async fn notify_primary_admin_access_request(
    bot: &Bot,
    pool: &SqlitePool,
    user: &User,
    request_id: i64,
) {
    let chats = match identity::list_primary_admin_chat_ids(pool).await {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(?error, "Impossibile trovare l'amministratore principale");
            return;
        }
    };
    let username = user
        .username
        .as_deref()
        .map(|value| format!("@{value}"))
        .unwrap_or_else(|| "nessun username".to_string());
    let text = format!(
        "📨 Nuova richiesta di accesso\n\n👤 {}\nTelegram: {}",
        user.full_name(),
        username
    );
    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "🔎 Apri richiesta".to_string(),
        format!("admin:access:view:{request_id}"),
    )]]);
    for chat in chats {
        if let Err(error) = bot
            .send_message(ChatId(chat), text.clone())
            .reply_markup(keyboard.clone())
            .await
        {
            tracing::warn!(chat, ?error, "Notifica richiesta accesso non inviata");
        }
    }
}

async fn ensure_primary_admin_access(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<bool> {
    match identity::is_primary_admin(pool, actor).await {
        Ok(true) => Ok(true),
        Ok(false) => {
            bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                .await?;
            Ok(false)
        }
        Err(error) => {
            tracing::error!(?error, "Errore verifica amministratore principale");
            bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                .await?;
            Ok(false)
        }
    }
}

async fn send_admin_access_requests(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    if !ensure_primary_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }
    match access_control::list_pending(pool).await {
        Ok(requests) if requests.is_empty() => {
            bot.send_message(chat_id, "📨 Non ci sono richieste di accesso in attesa.")
                .reply_markup(admin_back_keyboard())
                .await?;
        }
        Ok(requests) => {
            let mut lines = vec!["📨 Richieste di accesso".to_string()];
            let mut rows = Vec::new();
            for request in requests {
                let username = request
                    .username_snapshot
                    .as_deref()
                    .map(|value| format!("@{value}"))
                    .unwrap_or_else(|| "senza username".to_string());
                lines.push(format!(
                    "\n👤 {} · {}\nRichiesta: {}",
                    request.nome_snapshot, username, request.richiesta_il
                ));
                rows.push(vec![InlineKeyboardButton::callback(
                    format!("👤 {}", request.nome_snapshot),
                    format!("admin:access:view:{}", request.id),
                )]);
            }
            rows.push(vec![
                InlineKeyboardButton::callback(
                    "⬅️ Amministrazione".to_string(),
                    "admin:menu".to_string(),
                ),
                InlineKeyboardButton::callback(
                    "🏠 Menu principale".to_string(),
                    "menu:main".to_string(),
                ),
            ]);
            bot.send_message(chat_id, lines.join("\n"))
                .reply_markup(InlineKeyboardMarkup::new(rows))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco richieste accesso");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le richieste di accesso.")
                .reply_markup(admin_back_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn send_admin_access_request_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    request_id: i64,
) -> ResponseResult<()> {
    if !ensure_primary_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }
    match access_control::get_request(pool, request_id).await {
        Ok(Some(request)) => {
            let username = request
                .username_snapshot
                .as_deref()
                .map(|value| format!("@{value}"))
                .unwrap_or_else(|| "Nessuno".to_string());
            let surname = request.cognome_snapshot.as_deref().unwrap_or("-");
            let text = format!(
                "📨 Richiesta di accesso\n\nNome: {}\nCognome: {}\nUsername: {}\nRichiesta: {}\nStato: {}",
                request.nome_snapshot,
                surname,
                username,
                request.richiesta_il,
                access_request_status_label(&request.stato)
            );
            let mut rows = Vec::new();
            if request.stato == "pendente" {
                rows.push(vec![
                    InlineKeyboardButton::callback(
                        "✅ Approva".to_string(),
                        format!("admin:access:approve:{}", request.id),
                    ),
                    InlineKeyboardButton::callback(
                        "❌ Rifiuta".to_string(),
                        format!("admin:access:reject:{}", request.id),
                    ),
                ]);
            }
            rows.push(vec![
                InlineKeyboardButton::callback(
                    "⬅️ Richieste".to_string(),
                    "admin:access".to_string(),
                ),
                InlineKeyboardButton::callback(
                    "🏠 Menu principale".to_string(),
                    "menu:main".to_string(),
                ),
            ]);
            bot.send_message(chat_id, text)
                .reply_markup(InlineKeyboardMarkup::new(rows))
                .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Richiesta di accesso non trovata.")
                .reply_markup(admin_back_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, request_id, "Errore dettaglio richiesta accesso");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere la richiesta.")
                .reply_markup(admin_back_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn approve_access_request_ui(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    request_id: i64,
) -> ResponseResult<()> {
    if !ensure_primary_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }
    match access_control::approve_request(pool, actor, request_id).await {
        Ok(request) => {
            bot.send_message(
                chat_id,
                format!("✅ Accesso approvato per {}.", request.nome_snapshot),
            )
            .reply_markup(admin_back_keyboard())
            .await?;
            if let Err(error) = bot
                .send_message(
                    ChatId(request.chat_id),
                    "✅ La tua richiesta di accesso è stata approvata.\n\nUsa /start per aprire il gestionale. Ti è stato creato uno spazio personale; non hai ricevuto automaticamente accesso agli spazi degli altri utenti.",
                )
                .await
            {
                tracing::warn!(?error, "Notifica approvazione non inviata");
            }
        }
        Err(error) => {
            tracing::warn!(?error, request_id, "Approvazione richiesta fallita");
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(admin_back_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn reject_access_request_ui(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    request_id: i64,
) -> ResponseResult<()> {
    if !ensure_primary_admin_access(bot, chat_id, pool, actor).await? {
        return Ok(());
    }
    match access_control::reject_request(pool, actor, request_id).await {
        Ok(request) => {
            bot.send_message(
                chat_id,
                format!("❌ Richiesta rifiutata per {}.", request.nome_snapshot),
            )
            .reply_markup(admin_back_keyboard())
            .await?;
            if let Err(error) = bot
                .send_message(
                    ChatId(request.chat_id),
                    "❌ La tua richiesta di accesso non è stata approvata.",
                )
                .await
            {
                tracing::warn!(?error, "Notifica rifiuto non inviata");
            }
        }
        Err(error) => {
            tracing::warn!(?error, request_id, "Rifiuto richiesta fallito");
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(admin_back_keyboard())
                .await?;
        }
    }
    Ok(())
}

fn access_request_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "📨 Richiedi accesso".to_string(),
        "access:request".to_string(),
    )]])
}

fn access_pending_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "🔄 Controlla stato".to_string(),
        "access:status".to_string(),
    )]])
}

fn access_request_status_label(value: &str) -> &'static str {
    match value {
        "pendente" => "In attesa",
        "approvata" => "Approvata",
        "rifiutata" => "Rifiutata",
        _ => "Sconosciuto",
    }
}

fn admin_menu_keyboard(primary: bool, pending_access: i64) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![
            InlineKeyboardButton::callback(
                "🧭 Panoramica".to_string(),
                "admin:overview".to_string(),
            ),
            InlineKeyboardButton::callback(
                "📊 Stato sistema".to_string(),
                "admin:status".to_string(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "👥 Utenti".to_string(),
            "admin:users".to_string(),
        )],
    ];
    if primary {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("📨 Richieste di accesso ({pending_access})"),
            "admin:access".to_string(),
        )]);
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "🏠 Menu principale".to_string(),
        "menu:main".to_string(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

fn admin_back_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("⬅️ Amministrazione".to_string(), "admin:menu".to_string()),
        InlineKeyboardButton::callback("🏠 Menu principale".to_string(), "menu:main".to_string()),
    ]])
}

fn profile_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "👥 Spazi".to_string(),
            "identity:spaces".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "🏠 Menu principale".to_string(),
            "menu:main".to_string(),
        )],
    ])
}

fn space_flow_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "👤 Profilo".to_string(),
            "identity:profile".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "👥 Spazi".to_string(),
            "identity:spaces".to_string(),
        )],
        vec![InlineKeyboardButton::callback(
            "🏠 Menu principale".to_string(),
            "menu:main".to_string(),
        )],
    ])
}

fn command_args(text: &str) -> &str {
    text.split_once(char::is_whitespace)
        .map_or("", |(_, args)| args.trim())
}

fn first_command(text: &str) -> Option<&str> {
    let token = text.split_whitespace().next()?;
    if !token.starts_with('/') {
        return None;
    }
    token.split('@').next()
}

#[cfg(test)]
mod runtime_tests {
    use super::TOKIO_THREAD_STACK_SIZE;

    #[test]
    fn runtime_tokio_mantiene_stack_rinforzato() {
        const { assert!(TOKIO_THREAD_STACK_SIZE >= 8 * 1024 * 1024) };
    }
}
