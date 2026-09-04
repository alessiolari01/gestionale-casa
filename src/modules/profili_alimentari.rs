//! Step 7.2H.2 - Gestione completa dei Profili alimentari.
//!
//! Un profilo alimentare rappresenta una persona che partecipa ai pasti.
//! Può essere collegato a un account del gestionale oppure esistere senza account.
//! I profili alimentari non sono mai risorse globali.
//! In questo step il gestore può rinominare, condividere negli spazi e archiviare.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::{
    identity,
    modules::storico::{self, NewFieldChange, NewHistoryEvent},
};

type Bot = crate::context_bot::ContextBot;

const PROFILE_PAGE_SIZE: i64 = 5;
const SPACE_PAGE_SIZE: i64 = 5;
const PROFILE_NAME_MAX_CHARS: usize = 80;

#[derive(Clone, Default)]
pub struct ProfileSessionStore {
    inner: Arc<Mutex<HashMap<i64, ProfileConversationState>>>,
}

#[derive(Debug, Clone)]
enum ProfileConversationState {
    NewPersonName,
    Rename {
        profile_id: i64,
    },
    PortionPercentage {
        profile_id: i64,
        recipe_id: i64,
    },
    IngredientQuantity {
        profile_id: i64,
        recipe_id: i64,
        ingredient_id: i64,
    },
}

impl ProfileSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, chat_id: i64) -> Option<ProfileConversationState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&chat_id)
            .cloned()
    }

    fn set(&self, chat_id: i64, state: ProfileConversationState) {
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

    /// Chat con una sessione attiva in questa mappa. Usata dal controllo
    /// pre-swap (sotto-step 4/5 del punto 6 del ciclo di automazione) per
    /// sapere se rimandare lo spegnimento del bot, non da un singolo
    /// handler — quelli usano `has_active` su un chat_id preciso.
    #[allow(dead_code)]
    pub fn active_chat_ids(&self) -> Vec<i64> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .copied()
            .collect()
    }
}

#[derive(Debug, Clone, FromRow)]
struct ProfileRecord {
    id: i64,
    name: String,
    manager_user_id: i64,
    linked_user_id: Option<i64>,
    manager_name: String,
}

#[derive(Debug, Clone)]
struct ProfilePage {
    items: Vec<ProfileRecord>,
    total: i64,
    page: i64,
}

#[derive(Debug, Clone, FromRow)]
struct ManageableSpaceRecord {
    id: i64,
    name: String,
    selected: i64,
}

#[derive(Debug, Clone)]
struct ManageableSpacePage {
    items: Vec<ManageableSpaceRecord>,
    total: i64,
    selected_total: i64,
    page: i64,
}

pub async fn show_menu(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let has_self = has_linked_profile_for_current_user(pool)
        .await
        .unwrap_or(false);
    let mut text = String::from(
        "👥 Profili alimentari\n\nGestisci le persone che potranno partecipare ai pasti, alle porzioni personalizzate e ai planner.\n\nUn profilo può esistere anche senza un account Telegram.",
    );
    if has_self {
        text.push_str("\n\n🔗 Il tuo account è già collegato a un profilo alimentare.");
    }

    bot.send_message(chat_id, text)
        .reply_markup(profile_menu_keyboard())
        .await?;
    Ok(())
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &ProfileSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;
    let command = first_command(text);

    if command == Some("/profili_alimentari") {
        sessions.clear_chat(chat_id);
        show_menu(bot, msg.chat.id, pool).await?;
        return Ok(true);
    }

    if command == Some("/annulla") && sessions.has_active(chat_id) {
        match sessions.get(chat_id) {
            Some(ProfileConversationState::Rename { profile_id }) => {
                sessions.clear_chat(chat_id);
                show_profile_detail(bot, msg.chat.id, pool, profile_id).await?;
            }
            _ => {
                sessions.clear_chat(chat_id);
                show_menu(bot, msg.chat.id, pool).await?;
            }
        }
        return Ok(true);
    }

    if command.is_some() {
        sessions.clear_chat(chat_id);
        return Ok(false);
    }

    match sessions.get(chat_id) {
        Some(ProfileConversationState::NewPersonName) => {
            match create_profile(pool, text, false).await {
                Ok(profile_id) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(msg.chat.id, "✅ Profilo alimentare creato.")
                        .await?;
                    show_profile_detail(bot, msg.chat.id, pool, profile_id).await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nScrivi un altro nome oppure premi ❌ Annulla."),
                    )
                    .reply_markup(cancel_new_profile_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
        Some(ProfileConversationState::Rename { profile_id }) => {
            match rename_profile(pool, profile_id, text).await {
                Ok(changed) => {
                    sessions.clear_chat(chat_id);
                    if changed {
                        bot.send_message(msg.chat.id, "✅ Profilo rinominato.")
                            .await?;
                    } else {
                        bot.send_message(msg.chat.id, "ℹ️ Il nome è già quello indicato.")
                            .await?;
                    }
                    show_profile_detail(bot, msg.chat.id, pool, profile_id).await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nScrivi un altro nome oppure annulla."),
                    )
                    .reply_markup(rename_cancel_keyboard(profile_id))
                    .await?;
                }
            }
            Ok(true)
        }
        Some(ProfileConversationState::PortionPercentage {
            profile_id,
            recipe_id,
        }) => {
            crate::modules::porzioni_profili::handle_percentage_message(
                bot, msg, pool, profile_id, recipe_id, text,
            )
            .await?;
            Ok(true)
        }
        Some(ProfileConversationState::IngredientQuantity {
            profile_id,
            recipe_id,
            ingredient_id,
        }) => {
            let completed = crate::modules::porzioni_ingredienti::quantity_input_is_valid(text);
            crate::modules::porzioni_ingredienti::handle_quantity_message(
                bot,
                msg,
                pool,
                profile_id,
                recipe_id,
                ingredient_id,
                text,
            )
            .await?;
            if completed {
                sessions.clear_chat(chat_id);
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ProfileSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    if data.starts_with("foodprof:ing:") {
        if let Some((profile_id, recipe_id, ingredient_id)) =
            crate::modules::porzioni_ingredienti::input_context_from_callback(data)
        {
            sessions.set(
                chat_id.0,
                ProfileConversationState::IngredientQuantity {
                    profile_id,
                    recipe_id,
                    ingredient_id,
                },
            );
        } else {
            sessions.clear_chat(chat_id.0);
        }
        return crate::modules::porzioni_ingredienti::handle_callback(bot, chat_id, pool, data)
            .await;
    }
    if data.starts_with("foodprof:portion:") {
        if let Some((profile_id, recipe_id)) =
            crate::modules::porzioni_profili::portion_context_from_callback(data)
        {
            sessions.set(
                chat_id.0,
                ProfileConversationState::PortionPercentage {
                    profile_id,
                    recipe_id,
                },
            );
        } else {
            sessions.clear_chat(chat_id.0);
        }
        return crate::modules::porzioni_profili::handle_callback(bot, chat_id, pool, data).await;
    }

    match data {
        "foodprof:noop" => Ok(true),
        "foodprof:menu" => {
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id, pool).await?;
            Ok(true)
        }
        "foodprof:new" => {
            sessions.clear_chat(chat_id.0);
            send_new_profile_menu(bot, chat_id, pool).await?;
            Ok(true)
        }
        "foodprof:new:self" => {
            sessions.clear_chat(chat_id.0);
            let actor = identity::current_actor();
            match create_profile(pool, &actor.nome_snapshot, true).await {
                Ok(profile_id) => {
                    bot.send_message(chat_id, "✅ Il tuo profilo alimentare è stato creato.")
                        .await?;
                    show_profile_detail(bot, chat_id, pool, profile_id).await?;
                }
                Err(error) => {
                    bot.send_message(chat_id, format!("⚠️ {error}"))
                        .reply_markup(new_profile_keyboard(true))
                        .await?;
                }
            }
            Ok(true)
        }
        "foodprof:new:other" => {
            sessions.set(chat_id.0, ProfileConversationState::NewPersonName);
            bot.send_message(
                chat_id,
                "➕ Persona senza account\n\nScrivi il nome del profilo.\nEsempio: Giulia\n\nIl profilo nasce privato. Potrai condividerlo negli spazi dal suo dettaglio.",
            )
            .reply_markup(cancel_new_profile_keyboard())
            .await?;
            Ok(true)
        }
        "foodprof:list" => {
            sessions.clear_chat(chat_id.0);
            show_profile_list(bot, chat_id, pool, 0).await?;
            Ok(true)
        }
        "foodprof:cancel" => {
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id, pool).await?;
            Ok(true)
        }
        "menu:main" if sessions.has_active(chat_id.0) => {
            sessions.clear_chat(chat_id.0);
            Ok(false)
        }
        _ if data.starts_with("foodprof:list:page:") => {
            let page = data
                .strip_prefix("foodprof:list:page:")
                .and_then(parse_nonnegative_i64);
            if let Some(page) = page {
                sessions.clear_chat(chat_id.0);
                show_profile_list(bot, chat_id, pool, page).await?;
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("foodprof:view:") => {
            let profile_id = data
                .strip_prefix("foodprof:view:")
                .and_then(parse_positive_i64);
            if let Some(profile_id) = profile_id {
                sessions.clear_chat(chat_id.0);
                show_profile_detail(bot, chat_id, pool, profile_id).await?;
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("foodprof:rename:") => {
            let profile_id = data
                .strip_prefix("foodprof:rename:")
                .and_then(parse_positive_i64);
            if let Some(profile_id) = profile_id {
                match get_managed_profile(pool, profile_id).await {
                    Ok(Some(profile)) => {
                        sessions.set(chat_id.0, ProfileConversationState::Rename { profile_id });
                        bot.send_message(
                            chat_id,
                            format!(
                                "✏️ Rinomina profilo\n\nNome attuale: {}\n\nScrivi il nuovo nome.",
                                profile.name
                            ),
                        )
                        .reply_markup(rename_cancel_keyboard(profile_id))
                        .await?;
                    }
                    Ok(None) => {
                        bot.send_message(chat_id, "⚠️ Non puoi modificare questo profilo.")
                            .reply_markup(profile_menu_keyboard())
                            .await?;
                    }
                    Err(error) => {
                        tracing::error!(?error, profile_id, "Errore permessi rinomina profilo");
                        bot.send_message(chat_id, "⚠️ Non riesco a verificare questo profilo.")
                            .reply_markup(profile_menu_keyboard())
                            .await?;
                    }
                }
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("foodprof:spaces:") => {
            if let Some((profile_id, page)) = parse_profile_page_callback(data, "foodprof:spaces:")
            {
                sessions.clear_chat(chat_id.0);
                show_space_manager(bot, chat_id, pool, profile_id, page).await?;
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("foodprof:space:") => {
            if let Some((profile_id, space_id, page)) =
                parse_profile_space_callback(data, "foodprof:space:")
            {
                match toggle_profile_space(pool, profile_id, space_id).await {
                    Ok(_) => show_space_manager(bot, chat_id, pool, profile_id, page).await?,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            profile_id,
                            space_id,
                            "Condivisione profilo rifiutata"
                        );
                        bot.send_message(chat_id, format!("⚠️ {error}"))
                            .reply_markup(profile_return_keyboard(profile_id))
                            .await?;
                    }
                }
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("foodprof:private:") => {
            if let Some((profile_id, page)) = parse_profile_page_callback(data, "foodprof:private:")
            {
                match make_profile_private(pool, profile_id).await {
                    Ok(_) => show_space_manager(bot, chat_id, pool, profile_id, page).await?,
                    Err(error) => {
                        tracing::warn!(?error, profile_id, "Profilo privato rifiutato");
                        bot.send_message(chat_id, format!("⚠️ {error}"))
                            .reply_markup(profile_return_keyboard(profile_id))
                            .await?;
                    }
                }
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("foodprof:archive:confirm:") => {
            let profile_id = data
                .strip_prefix("foodprof:archive:confirm:")
                .and_then(parse_positive_i64);
            if let Some(profile_id) = profile_id {
                sessions.clear_chat(chat_id.0);
                match archive_profile(pool, profile_id).await {
                    Ok(()) => {
                        bot.send_message(
                            chat_id,
                            "📦 Profilo archiviato.\n\nNon compare più nelle liste attive e non è più condiviso negli spazi.",
                        )
                        .await?;
                        show_profile_list(bot, chat_id, pool, 0).await?;
                    }
                    Err(error) => {
                        tracing::warn!(?error, profile_id, "Archiviazione profilo rifiutata");
                        bot.send_message(chat_id, format!("⚠️ {error}"))
                            .reply_markup(profile_menu_keyboard())
                            .await?;
                    }
                }
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ if data.starts_with("foodprof:archive:") => {
            let profile_id = data
                .strip_prefix("foodprof:archive:")
                .and_then(parse_positive_i64);
            if let Some(profile_id) = profile_id {
                sessions.clear_chat(chat_id.0);
                show_archive_confirmation(bot, chat_id, pool, profile_id).await?;
            } else {
                send_invalid_profile_action(bot, chat_id).await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn send_invalid_profile_action(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "⚠️ Azione profilo non valida o non più disponibile.",
    )
    .reply_markup(profile_menu_keyboard())
    .await?;
    Ok(())
}

async fn send_new_profile_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
) -> ResponseResult<()> {
    let has_self = has_linked_profile_for_current_user(pool)
        .await
        .unwrap_or(false);
    bot.send_message(
        chat_id,
        "➕ Nuovo profilo alimentare\n\nScegli chi rappresenta il profilo.\n\nUn account può essere collegato a un solo profilo alimentare attivo.",
    )
    .reply_markup(new_profile_keyboard(has_self))
    .await?;
    Ok(())
}

async fn show_profile_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    requested_page: i64,
) -> ResponseResult<()> {
    match list_visible_profiles(pool, requested_page).await {
        Ok(profile_page) => {
            let pages = page_count(profile_page.total);
            let text = if profile_page.total == 0 {
                "👥 Profili alimentari\n\nNessun profilo disponibile.\n\nCrea il tuo profilo oppure una persona senza account.".to_string()
            } else {
                format!(
                    "👥 Profili alimentari\n\nTotale: {}\nPagina {}/{}\n\nApri un profilo per consultarne i dettagli.",
                    profile_page.total,
                    profile_page.page + 1,
                    pages
                )
            };

            bot.send_message(chat_id, text)
                .reply_markup(profile_list_keyboard(&profile_page, pages))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, "Errore elenco profili alimentari");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i profili alimentari.")
                .reply_markup(profile_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_profile_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
) -> ResponseResult<()> {
    match get_visible_profile(pool, profile_id).await {
        Ok(Some(profile)) => {
            let current_user = identity::current_actor().utente_id;
            let own_profile = Some(profile.manager_user_id) == current_user;
            let linked_to_current = profile.linked_user_id == current_user;

            let linked_label = match profile.linked_user_id {
                Some(id) if Some(id) == current_user => "🔗 Account: collegato al tuo account",
                Some(_) => "🔗 Account: collegato a un account del gestionale",
                None => "🔗 Account: profilo interno senza account",
            };

            let manager_label = if own_profile {
                "⚙️ Gestione: gestito da te".to_string()
            } else {
                format!("⚙️ Gestione: {}", profile.manager_name)
            };

            let visible_space_names = load_visible_profile_space_names(pool, &profile)
                .await
                .unwrap_or_default();
            let visibility_label = if own_profile {
                format_owned_visibility(&visible_space_names)
            } else if !visible_space_names.is_empty() {
                format!(
                    "👥 Condiviso con te tramite: {}",
                    visible_space_names.join(" · ")
                )
            } else if linked_to_current {
                "🔗 Profilo personale collegato al tuo account".to_string()
            } else {
                "👥 Profilo condiviso con te".to_string()
            };

            bot.send_message(
                chat_id,
                format!(
                    "👤 {}\n\n{}\n{}\n{}\n\n🌐 Un profilo alimentare non può mai essere globale.",
                    profile.name, linked_label, manager_label, visibility_label
                ),
            )
            .reply_markup(profile_detail_keyboard(profile.id, own_profile))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Profilo non disponibile per questo account.")
                .reply_markup(profile_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, profile_id, "Errore dettaglio profilo alimentare");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo profilo.")
                .reply_markup(profile_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_space_manager(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
    requested_page: i64,
) -> ResponseResult<()> {
    let profile = match get_managed_profile(pool, profile_id).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            bot.send_message(
                chat_id,
                "⚠️ Solo il gestore può cambiare la visibilità del profilo.",
            )
            .reply_markup(profile_menu_keyboard())
            .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, profile_id, "Errore gestione visibilità profilo");
            bot.send_message(chat_id, "⚠️ Non riesco a verificare questo profilo.")
                .reply_markup(profile_menu_keyboard())
                .await?;
            return Ok(());
        }
    };

    match list_manageable_spaces(pool, profile_id, requested_page).await {
        Ok(space_page) => {
            let pages = space_page_count(space_page.total);
            let text = if space_page.total == 0 {
                format!(
                    "🏠 Visibilità · {}\n\nNon hai spazi modificabili disponibili.\nIl profilo rimane privato.",
                    profile.name
                )
            } else {
                format!(
                    "🏠 Visibilità · {}\n\nScegli in quali spazi rendere disponibile questo profilo.\n\n✅ condiviso · ◻️ non condiviso\nPagina {}/{}",
                    profile.name,
                    space_page.page + 1,
                    pages
                )
            };
            bot.send_message(chat_id, text)
                .reply_markup(space_manager_keyboard(profile_id, &space_page, pages))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, profile_id, "Errore elenco spazi profilo");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere gli spazi disponibili.")
                .reply_markup(profile_return_keyboard(profile_id))
                .await?;
        }
    }
    Ok(())
}

async fn show_archive_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
) -> ResponseResult<()> {
    match get_managed_profile(pool, profile_id).await {
        Ok(Some(profile)) => {
            bot.send_message(
                chat_id,
                format!(
                    "📦 Archivia profilo\n\nVuoi archiviare \"{}\"?\n\nIl profilo non verrà cancellato fisicamente, ma sparirà dalle liste attive e verranno rimosse le condivisioni negli spazi.",
                    profile.name
                ),
            )
            .reply_markup(archive_confirmation_keyboard(profile_id))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Solo il gestore può archiviare questo profilo.")
                .reply_markup(profile_menu_keyboard())
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, profile_id, "Errore conferma archiviazione profilo");
            bot.send_message(chat_id, "⚠️ Non riesco a verificare questo profilo.")
                .reply_markup(profile_menu_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn has_linked_profile_for_current_user(pool: &SqlitePool) -> Result<bool> {
    let user_id = current_user_id()?;
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM profili_alimentari WHERE utente_collegato_id = ? AND archiviato = 0)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il profilo collegato")
}

async fn create_profile(pool: &SqlitePool, raw_name: &str, link_to_self: bool) -> Result<i64> {
    let user_id = current_user_id()?;
    let name = clean_name(raw_name)?;
    let normalized = normalize_name(&name);

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la creazione del profilo alimentare")?;

    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM profili_alimentari WHERE gestore_utente_id = ? AND nome_normalizzato = ? AND archiviato = 0)",
    )
    .bind(user_id)
    .bind(&normalized)
    .fetch_one(&mut *tx)
    .await
    .context("Impossibile verificare i profili esistenti")?;
    if duplicate {
        bail!("Hai già un profilo alimentare attivo con questo nome");
    }

    if link_to_self {
        let already_linked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM profili_alimentari WHERE utente_collegato_id = ? AND archiviato = 0)",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .context("Impossibile verificare il profilo collegato")?;
        if already_linked {
            bail!("Il tuo account è già collegato a un profilo alimentare");
        }
    }

    let linked_user_id = link_to_self.then_some(user_id);
    let profile_id = sqlx::query(
        "INSERT INTO profili_alimentari (gestore_utente_id, utente_collegato_id, nome, nome_normalizzato) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(linked_user_id)
    .bind(&name)
    .bind(&normalized)
    .execute(&mut *tx)
    .await
    .context("Impossibile creare il profilo alimentare")?
    .last_insert_rowid();

    record_profile_history_event(
        &mut tx,
        profile_id,
        &name,
        "creazione",
        "profili_alimentari",
        &[NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: None,
            valore_dopo: Some(name.clone()),
        }],
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare la creazione del profilo")?;
    Ok(profile_id)
}

async fn rename_profile(pool: &SqlitePool, profile_id: i64, raw_name: &str) -> Result<bool> {
    let user_id = current_user_id()?;
    let new_name = clean_name(raw_name)?;
    let normalized = normalize_name(&new_name);

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la rinomina del profilo")?;
    let current_name: Option<String> = sqlx::query_scalar(
        "SELECT nome FROM profili_alimentari WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile verificare il gestore del profilo")?;
    let current_name = current_name.context("Non hai il permesso di rinominare questo profilo")?;

    if current_name == new_name {
        return Ok(false);
    }

    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM profili_alimentari WHERE gestore_utente_id = ? AND nome_normalizzato = ? AND archiviato = 0 AND id <> ?)",
    )
    .bind(user_id)
    .bind(&normalized)
    .bind(profile_id)
    .fetch_one(&mut *tx)
    .await
    .context("Impossibile verificare i nomi già esistenti")?;
    if duplicate {
        bail!("Hai già un altro profilo alimentare attivo con questo nome");
    }

    let affected = sqlx::query(
        "UPDATE profili_alimentari SET nome = ?, nome_normalizzato = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(&new_name)
    .bind(&normalized)
    .bind(profile_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile rinominare il profilo")?
    .rows_affected();
    if affected != 1 {
        bail!("Profilo non disponibile o non modificabile");
    }

    record_profile_history_event(
        &mut tx,
        profile_id,
        &new_name,
        "rinomina",
        "profili_alimentari",
        &[NewFieldChange {
            campo: "nome",
            tipo_valore: "testo",
            valore_prima: Some(current_name),
            valore_dopo: Some(new_name.clone()),
        }],
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare la rinomina del profilo")?;
    Ok(true)
}

async fn toggle_profile_space(pool: &SqlitePool, profile_id: i64, space_id: i64) -> Result<bool> {
    let user_id = current_user_id()?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica della visibilità")?;

    let profile_name = managed_profile_name_conn(&mut tx, profile_id, user_id).await?;
    ensure_writable_space_conn(&mut tx, space_id, user_id).await?;
    let before = shared_space_names_conn(&mut tx, profile_id).await?;

    let selected: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ? AND spazio_id = ?)",
    )
    .bind(profile_id)
    .bind(space_id)
    .fetch_one(&mut *tx)
    .await
    .context("Impossibile verificare la condivisione del profilo")?;

    if selected {
        let affected = sqlx::query(
            "DELETE FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ? AND spazio_id = ? AND EXISTS (SELECT 1 FROM profili_alimentari pa WHERE pa.id = ? AND pa.gestore_utente_id = ? AND pa.archiviato = 0)",
        )
        .bind(profile_id)
        .bind(space_id)
        .bind(profile_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile rimuovere la condivisione")?
        .rows_affected();
        if affected != 1 {
            bail!("Non hai il permesso di rimuovere questa condivisione");
        }
    } else {
        sqlx::query(
            "INSERT INTO profilo_alimentare_spazi (profilo_alimentare_id, spazio_id, condiviso_da_utente_id) VALUES (?, ?, ?)",
        )
        .bind(profile_id)
        .bind(space_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile condividere il profilo nello spazio")?;
    }

    sqlx::query(
        "UPDATE profili_alimentari SET aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare il profilo")?;

    let after = shared_space_names_conn(&mut tx, profile_id).await?;
    record_visibility_history(&mut tx, profile_id, &profile_name, &before, &after).await?;

    tx.commit()
        .await
        .context("Impossibile completare la modifica della visibilità")?;
    Ok(!selected)
}

async fn make_profile_private(pool: &SqlitePool, profile_id: i64) -> Result<bool> {
    let user_id = current_user_id()?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica della visibilità")?;
    let profile_name = managed_profile_name_conn(&mut tx, profile_id, user_id).await?;
    let before = shared_space_names_conn(&mut tx, profile_id).await?;
    if before.is_empty() {
        return Ok(false);
    }

    sqlx::query(
        "DELETE FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ? AND EXISTS (SELECT 1 FROM profili_alimentari pa WHERE pa.id = ? AND pa.gestore_utente_id = ? AND pa.archiviato = 0)",
    )
    .bind(profile_id)
    .bind(profile_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile rendere privato il profilo")?;

    sqlx::query(
        "UPDATE profili_alimentari SET aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare il profilo")?;

    let after = Vec::new();
    record_visibility_history(&mut tx, profile_id, &profile_name, &before, &after).await?;
    tx.commit()
        .await
        .context("Impossibile completare la modifica della visibilità")?;
    Ok(true)
}

async fn archive_profile(pool: &SqlitePool, profile_id: i64) -> Result<()> {
    let user_id = current_user_id()?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare l'archiviazione del profilo")?;
    let profile_name = managed_profile_name_conn(&mut tx, profile_id, user_id).await?;

    let affected = sqlx::query(
        "UPDATE profili_alimentari SET archiviato = 1, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare il profilo")?
    .rows_affected();
    if affected != 1 {
        bail!("Profilo non disponibile o non archiviabile");
    }

    sqlx::query("DELETE FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ?")
        .bind(profile_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile rimuovere le condivisioni del profilo archiviato")?;

    record_profile_history_event(
        &mut tx,
        profile_id,
        &profile_name,
        "modifica",
        "archiviazione_profilo",
        &[NewFieldChange {
            campo: "stato",
            tipo_valore: "testo",
            valore_prima: Some("Attivo".to_string()),
            valore_dopo: Some("Archiviato".to_string()),
        }],
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare l'archiviazione del profilo")?;
    Ok(())
}

async fn managed_profile_name_conn(
    conn: &mut SqliteConnection,
    profile_id: i64,
    user_id: i64,
) -> Result<String> {
    sqlx::query_scalar(
        "SELECT nome FROM profili_alimentari WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await
    .context("Impossibile verificare il gestore del profilo")?
    .context("Non hai il permesso di modificare questo profilo")
}

async fn ensure_writable_space_conn(
    conn: &mut SqliteConnection,
    space_id: i64,
    user_id: i64,
) -> Result<()> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM membri_spazio WHERE spazio_id = ? AND utente_id = ? AND ruolo IN ('proprietario', 'amministratore', 'membro'))",
    )
    .bind(space_id)
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await
    .context("Impossibile verificare i permessi nello spazio")?;
    if !allowed {
        bail!("Non hai il permesso di condividere risorse in questo spazio");
    }
    Ok(())
}

async fn shared_space_names_conn(
    conn: &mut SqliteConnection,
    profile_id: i64,
) -> Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT s.nome FROM profilo_alimentare_spazi pas JOIN spazi s ON s.id = pas.spazio_id WHERE pas.profilo_alimentare_id = ? ORDER BY s.nome COLLATE NOCASE, s.id",
    )
    .bind(profile_id)
    .fetch_all(&mut *conn)
    .await
    .context("Impossibile leggere gli spazi condivisi")
}

async fn record_visibility_history(
    conn: &mut SqliteConnection,
    profile_id: i64,
    profile_name: &str,
    before: &[String],
    after: &[String],
) -> Result<()> {
    let before_value = visibility_history_value(before);
    let after_value = visibility_history_value(after);
    if before_value == after_value {
        return Ok(());
    }
    let component = if after.is_empty() {
        "privatizzazione_profilo"
    } else {
        "condivisione_profilo"
    };
    record_profile_history_event(
        conn,
        profile_id,
        profile_name,
        "modifica",
        component,
        &[NewFieldChange {
            campo: "visibilita",
            tipo_valore: "testo",
            valore_prima: Some(before_value),
            valore_dopo: Some(after_value),
        }],
    )
    .await
}

async fn record_profile_history_event(
    conn: &mut SqliteConnection,
    profile_id: i64,
    profile_name: &str,
    operation: &'static str,
    component: &'static str,
    changes: &[NewFieldChange],
) -> Result<()> {
    let entity_id = storico::ensure_entity(conn, "profilo_alimentare", profile_id, profile_name)
        .await
        .context("Impossibile preparare lo storico del profilo")?;
    let event_id = storico::record_event(
        conn,
        &NewHistoryEvent {
            entita_storico_id: entity_id,
            modulo: "alimentazione",
            componente: component,
            operazione: operation,
            nome_entita_snapshot: profile_name,
            abitazione_storico_id: None,
            abitazione_nome_snapshot: None,
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await
    .context("Impossibile registrare l'evento del profilo nello storico")?;
    storico::record_field_changes(conn, event_id, changes)
        .await
        .context("Impossibile registrare i cambiamenti del profilo nello storico")?;
    Ok(())
}

fn visibility_history_value(spaces: &[String]) -> String {
    if spaces.is_empty() {
        "Privato".to_string()
    } else {
        format!("Condiviso: {}", spaces.join(" · "))
    }
}

async fn list_visible_profiles(pool: &SqlitePool, requested_page: i64) -> Result<ProfilePage> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Identità utente non disponibile per i profili alimentari")?;
    let view_all = if actor.view_all { 1_i64 } else { 0_i64 };

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM profili_alimentari pa WHERE pa.archiviato = 0 AND (pa.gestore_utente_id = ? OR pa.utente_collegato_id = ? OR EXISTS (SELECT 1 FROM profilo_alimentare_spazi pas JOIN membri_spazio ms ON ms.spazio_id = pas.spazio_id WHERE pas.profilo_alimentare_id = pa.id AND ms.utente_id = ? AND (? = 1 OR pas.spazio_id = ?)))",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .fetch_one(pool)
    .await
    .context("Impossibile contare i profili alimentari")?;

    let pages = page_count(total);
    let page = requested_page.max(0).min(pages.saturating_sub(1));
    let offset = page * PROFILE_PAGE_SIZE;

    let items = sqlx::query_as::<_, ProfileRecord>(
        "SELECT pa.id, pa.nome AS name, pa.gestore_utente_id AS manager_user_id, pa.utente_collegato_id AS linked_user_id, u.nome_visualizzato AS manager_name FROM profili_alimentari pa JOIN utenti u ON u.id = pa.gestore_utente_id WHERE pa.archiviato = 0 AND (pa.gestore_utente_id = ? OR pa.utente_collegato_id = ? OR EXISTS (SELECT 1 FROM profilo_alimentare_spazi pas JOIN membri_spazio ms ON ms.spazio_id = pas.spazio_id WHERE pas.profilo_alimentare_id = pa.id AND ms.utente_id = ? AND (? = 1 OR pas.spazio_id = ?))) ORDER BY CASE WHEN pa.gestore_utente_id = ? THEN 0 WHEN pa.utente_collegato_id = ? THEN 1 ELSE 2 END, pa.nome_normalizzato, pa.id LIMIT ? OFFSET ?",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .bind(user_id)
    .bind(user_id)
    .bind(PROFILE_PAGE_SIZE)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i profili alimentari")?;

    Ok(ProfilePage { items, total, page })
}

async fn get_visible_profile(pool: &SqlitePool, profile_id: i64) -> Result<Option<ProfileRecord>> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Identità utente non disponibile per i profili alimentari")?;
    let view_all = if actor.view_all { 1_i64 } else { 0_i64 };

    sqlx::query_as::<_, ProfileRecord>(
        "SELECT pa.id, pa.nome AS name, pa.gestore_utente_id AS manager_user_id, pa.utente_collegato_id AS linked_user_id, u.nome_visualizzato AS manager_name FROM profili_alimentari pa JOIN utenti u ON u.id = pa.gestore_utente_id WHERE pa.id = ? AND pa.archiviato = 0 AND (pa.gestore_utente_id = ? OR pa.utente_collegato_id = ? OR EXISTS (SELECT 1 FROM profilo_alimentare_spazi pas JOIN membri_spazio ms ON ms.spazio_id = pas.spazio_id WHERE pas.profilo_alimentare_id = pa.id AND ms.utente_id = ? AND (? = 1 OR pas.spazio_id = ?)))",
    )
    .bind(profile_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(view_all)
    .bind(actor.spazio_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il profilo alimentare")
}

async fn get_managed_profile(pool: &SqlitePool, profile_id: i64) -> Result<Option<ProfileRecord>> {
    let user_id = current_user_id()?;
    sqlx::query_as::<_, ProfileRecord>(
        "SELECT pa.id, pa.nome AS name, pa.gestore_utente_id AS manager_user_id, pa.utente_collegato_id AS linked_user_id, u.nome_visualizzato AS manager_name FROM profili_alimentari pa JOIN utenti u ON u.id = pa.gestore_utente_id WHERE pa.id = ? AND pa.gestore_utente_id = ? AND pa.archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile verificare la gestione del profilo")
}

async fn load_visible_profile_space_names(
    pool: &SqlitePool,
    profile: &ProfileRecord,
) -> Result<Vec<String>> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Identità utente non disponibile per la visibilità del profilo")?;
    if profile.manager_user_id == user_id {
        return sqlx::query_scalar(
            "SELECT s.nome FROM profilo_alimentare_spazi pas JOIN spazi s ON s.id = pas.spazio_id WHERE pas.profilo_alimentare_id = ? ORDER BY s.nome COLLATE NOCASE, s.id",
        )
        .bind(profile.id)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere gli spazi del profilo");
    }

    sqlx::query_scalar(
        "SELECT s.nome FROM profilo_alimentare_spazi pas JOIN spazi s ON s.id = pas.spazio_id JOIN membri_spazio ms ON ms.spazio_id = pas.spazio_id WHERE pas.profilo_alimentare_id = ? AND ms.utente_id = ? ORDER BY s.nome COLLATE NOCASE, s.id",
    )
    .bind(profile.id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi condivisi visibili")
}

async fn list_manageable_spaces(
    pool: &SqlitePool,
    profile_id: i64,
    requested_page: i64,
) -> Result<ManageableSpacePage> {
    let user_id = current_user_id()?;
    if get_managed_profile(pool, profile_id).await?.is_none() {
        bail!("Non hai il permesso di modificare la visibilità di questo profilo");
    }

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM membri_spazio WHERE utente_id = ? AND ruolo IN ('proprietario', 'amministratore', 'membro')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile contare gli spazi modificabili")?;
    let selected_total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ?",
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await
    .context("Impossibile contare gli spazi condivisi")?;

    let pages = space_page_count(total);
    let page = requested_page.max(0).min(pages.saturating_sub(1));
    let offset = page * SPACE_PAGE_SIZE;
    let items = sqlx::query_as::<_, ManageableSpaceRecord>(
        "SELECT s.id, s.nome AS name, CASE WHEN pas.spazio_id IS NULL THEN 0 ELSE 1 END AS selected FROM membri_spazio ms JOIN spazi s ON s.id = ms.spazio_id LEFT JOIN profilo_alimentare_spazi pas ON pas.spazio_id = s.id AND pas.profilo_alimentare_id = ? WHERE ms.utente_id = ? AND ms.ruolo IN ('proprietario', 'amministratore', 'membro') ORDER BY CASE WHEN s.id = ? THEN 0 ELSE 1 END, s.nome COLLATE NOCASE, s.id LIMIT ? OFFSET ?",
    )
    .bind(profile_id)
    .bind(user_id)
    .bind(identity::current_actor().spazio_id)
    .bind(SPACE_PAGE_SIZE)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli spazi modificabili")?;

    Ok(ManageableSpacePage {
        items,
        total,
        selected_total,
        page,
    })
}

fn format_owned_visibility(spaces: &[String]) -> String {
    if spaces.is_empty() {
        "🔒 Visibilità: privata".to_string()
    } else if spaces.len() == 1 {
        format!("👥 Visibilità: condivisa in {}", spaces[0])
    } else {
        format!(
            "👥 Visibilità: condivisa in {} spazi\n• {}",
            spaces.len(),
            spaces.join("\n• ")
        )
    }
}

fn current_user_id() -> Result<i64> {
    identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")
}

fn clean_name(raw: &str) -> Result<String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = name.chars().count();
    if len == 0 || len > PROFILE_NAME_MAX_CHARS {
        bail!("Il nome del profilo deve contenere da 1 a {PROFILE_NAME_MAX_CHARS} caratteri");
    }
    Ok(name)
}

fn normalize_name(value: &str) -> String {
    value.to_lowercase()
}

fn first_command(text: &str) -> Option<&str> {
    let first = text.split_whitespace().next()?;
    if !first.starts_with('/') {
        return None;
    }
    Some(first.split('@').next().unwrap_or(first))
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|value| *value > 0)
}

fn parse_nonnegative_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|value| *value >= 0)
}

fn parse_profile_page_callback(data: &str, prefix: &str) -> Option<(i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let profile_id = parse_positive_i64(parts.next()?)?;
    let page = parse_nonnegative_i64(parts.next()?)?;
    (parts.next().is_none()).then_some((profile_id, page))
}

fn parse_profile_space_callback(data: &str, prefix: &str) -> Option<(i64, i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let profile_id = parse_positive_i64(parts.next()?)?;
    let space_id = parse_positive_i64(parts.next()?)?;
    let page = parse_nonnegative_i64(parts.next()?)?;
    (parts.next().is_none()).then_some((profile_id, space_id, page))
}

fn page_count(total: i64) -> i64 {
    ((total + PROFILE_PAGE_SIZE - 1) / PROFILE_PAGE_SIZE).max(1)
}

fn space_page_count(total: i64) -> i64 {
    ((total + SPACE_PAGE_SIZE - 1) / SPACE_PAGE_SIZE).max(1)
}

fn button(text: impl Into<String>, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), data.into())
}

fn profile_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("➕ Nuovo profilo", "foodprof:new")],
        vec![button("📋 Elenco profili", "foodprof:list")],
        vec![
            button("⬅️ Indietro", "food:menu"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn new_profile_keyboard(has_self_profile: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if !has_self_profile {
        rows.push(vec![button("👤 Me stesso", "foodprof:new:self")]);
    }
    rows.push(vec![button(
        "➕ Persona senza account",
        "foodprof:new:other",
    )]);
    rows.push(vec![
        button("⬅️ Indietro", "foodprof:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn cancel_new_profile_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", "foodprof:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn rename_cancel_keyboard(profile_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", format!("foodprof:view:{profile_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn profile_list_keyboard(page: &ProfilePage, pages: i64) -> InlineKeyboardMarkup {
    let current_user = identity::current_actor().utente_id;
    let mut rows = page
        .items
        .iter()
        .map(|profile| {
            let icon = if profile.linked_user_id == current_user {
                "🔗"
            } else if Some(profile.manager_user_id) == current_user {
                "👤"
            } else {
                "👥"
            };
            vec![button(
                format!("{icon} {}", profile.name),
                format!("foodprof:view:{}", profile.id),
            )]
        })
        .collect::<Vec<_>>();

    if page.total > 0 {
        let mut pagination = Vec::new();
        if page.page > 0 {
            pagination.push(button(
                "⬅️ Pagina precedente",
                format!("foodprof:list:page:{}", page.page - 1),
            ));
        }
        pagination.push(button(
            format!("{}/{}", page.page + 1, pages),
            "foodprof:noop",
        ));
        if page.page + 1 < pages {
            pagination.push(button(
                "Pagina successiva ➡️",
                format!("foodprof:list:page:{}", page.page + 1),
            ));
        }
        rows.push(pagination);
    }

    rows.push(vec![button("➕ Nuovo profilo", "foodprof:new")]);
    rows.push(vec![
        button("⬅️ Indietro", "foodprof:menu"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn profile_detail_keyboard(profile_id: i64, can_manage: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if can_manage {
        rows.push(vec![
            button("✏️ Rinomina", format!("foodprof:rename:{profile_id}")),
            button(
                "🏠 Gestisci visibilità",
                format!("foodprof:spaces:{profile_id}:0"),
            ),
        ]);
        rows.push(vec![button(
            "🍽️ Porzioni e preferenze",
            format!("foodprof:portion:list:{profile_id}:0"),
        )]);
        rows.push(vec![button(
            "📦 Archivia profilo",
            format!("foodprof:archive:{profile_id}"),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", "foodprof:list"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn profile_return_keyboard(profile_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("⬅️ Torna al profilo", format!("foodprof:view:{profile_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn space_manager_keyboard(
    profile_id: i64,
    page: &ManageableSpacePage,
    pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows = page
        .items
        .iter()
        .map(|space| {
            let marker = if space.selected != 0 { "✅" } else { "◻️" };
            vec![button(
                format!("{marker} {}", space.name),
                format!("foodprof:space:{profile_id}:{}:{}", space.id, page.page),
            )]
        })
        .collect::<Vec<_>>();

    if page.total > 0 {
        let mut pagination = Vec::new();
        if page.page > 0 {
            pagination.push(button(
                "⬅️ Pagina precedente",
                format!("foodprof:spaces:{profile_id}:{}", page.page - 1),
            ));
        }
        pagination.push(button(
            format!("{}/{}", page.page + 1, pages),
            "foodprof:noop",
        ));
        if page.page + 1 < pages {
            pagination.push(button(
                "Pagina successiva ➡️",
                format!("foodprof:spaces:{profile_id}:{}", page.page + 1),
            ));
        }
        rows.push(pagination);
    }

    if page.selected_total > 0 {
        rows.push(vec![button(
            "🔒 Rendi privato",
            format!("foodprof:private:{profile_id}:{}", page.page),
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", format!("foodprof:view:{profile_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn archive_confirmation_keyboard(profile_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "✅ Sì, archivia",
            format!("foodprof:archive:confirm:{profile_id}"),
        )],
        vec![
            button("❌ Annulla", format!("foodprof:view:{profile_id}")),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn actor(user_id: i64, space_id: i64, view_all: bool, name: &str) -> identity::AuditActor {
        identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: name.to_string(),
            spazio_id: space_id,
            spazio_nome_snapshot: format!("Spazio {space_id}"),
            view_all,
            origine: "telegram",
            telegram_user_id: Some(user_id),
            telegram_username: None,
        }
    }

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

    async fn create_space(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO spazi (nome, tipo) VALUES (?, 'condiviso')")
            .bind(name)
            .execute(pool)
            .await
            .expect("spazio")
            .last_insert_rowid()
    }

    async fn add_membership(pool: &SqlitePool, space_id: i64, user_id: i64, role: &str) {
        sqlx::query("INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, ?)")
            .bind(space_id)
            .bind(user_id)
            .bind(role)
            .execute(pool)
            .await
            .expect("membership");
    }

    async fn direct_profile(pool: &SqlitePool, manager_id: i64, name: &str) -> i64 {
        sqlx::query(
            "INSERT INTO profili_alimentari (gestore_utente_id, nome, nome_normalizzato) VALUES (?, ?, ?)",
        )
        .bind(manager_id)
        .bind(name)
        .bind(normalize_name(name))
        .execute(pool)
        .await
        .expect("profilo")
        .last_insert_rowid()
    }

    #[test]
    fn nome_profilo_normalizza_spazi() {
        assert_eq!(clean_name("  Mario   Rossi  ").unwrap(), "Mario Rossi");
    }

    #[test]
    fn paginazione_profili_usa_cinque_elementi() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(5), 1);
        assert_eq!(page_count(6), 2);
        assert_eq!(page_count(11), 3);
        assert_eq!(space_page_count(6), 2);
    }

    #[test]
    fn callback_profili_restano_nel_limite_telegram() {
        let samples = [
            "foodprof:menu".to_string(),
            "foodprof:list:page:999999".to_string(),
            format!("foodprof:view:{}", i64::MAX),
            format!("foodprof:spaces:{}:999999", i64::MAX),
            format!("foodprof:space:{}:{}:999999", i64::MAX, i64::MAX),
            format!("foodprof:archive:confirm:{}", i64::MAX),
        ];
        assert!(samples.iter().all(|value| value.len() <= 64));
    }

    #[tokio::test]
    async fn backend_blocca_rinomina_da_un_altro_account() {
        let pool = test_pool().await;
        let manager = create_user(&pool, "Gestore").await;
        let intruder = create_user(&pool, "Altro").await;
        add_membership(&pool, 1, manager, "proprietario").await;
        add_membership(&pool, 1, intruder, "membro").await;
        let profile_id = direct_profile(&pool, manager, "Profilo privato").await;

        identity::with_actor(actor(intruder, 1, false, "Altro"), async {
            let result = rename_profile(&pool, profile_id, "Nome rubato").await;
            assert!(result.is_err());
        })
        .await;

        let name: String = sqlx::query_scalar("SELECT nome FROM profili_alimentari WHERE id = ?")
            .bind(profile_id)
            .fetch_one(&pool)
            .await
            .expect("nome");
        assert_eq!(name, "Profilo privato");
    }

    #[tokio::test]
    async fn backend_blocca_condivisione_da_un_altro_account() {
        let pool = test_pool().await;
        let manager = create_user(&pool, "Gestore").await;
        let intruder = create_user(&pool, "Altro").await;
        let shared_space = create_space(&pool, "Casa condivisa").await;
        add_membership(&pool, 1, manager, "proprietario").await;
        add_membership(&pool, 1, intruder, "membro").await;
        add_membership(&pool, shared_space, manager, "membro").await;
        add_membership(&pool, shared_space, intruder, "membro").await;
        let profile_id = direct_profile(&pool, manager, "Profilo privato").await;

        identity::with_actor(actor(intruder, 1, false, "Altro"), async {
            let result = toggle_profile_space(&pool, profile_id, shared_space).await;
            assert!(result.is_err());
        })
        .await;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ?",
        )
        .bind(profile_id)
        .fetch_one(&pool)
        .await
        .expect("condivisioni");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn gestore_puo_condividere_e_rendere_privato() {
        let pool = test_pool().await;
        let manager = create_user(&pool, "Gestore").await;
        let shared_space = create_space(&pool, "Casa condivisa").await;
        add_membership(&pool, 1, manager, "proprietario").await;
        add_membership(&pool, shared_space, manager, "membro").await;
        let profile_id = direct_profile(&pool, manager, "Profilo").await;

        identity::with_actor(actor(manager, 1, false, "Gestore"), async {
            assert!(toggle_profile_space(&pool, profile_id, shared_space)
                .await
                .expect("condivisione"));
            assert!(make_profile_private(&pool, profile_id)
                .await
                .expect("privato"));
        })
        .await;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ?",
        )
        .bind(profile_id)
        .fetch_one(&pool)
        .await
        .expect("condivisioni");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn archiviazione_rimuove_le_condivisioni_ma_non_il_record() {
        let pool = test_pool().await;
        let manager = create_user(&pool, "Gestore").await;
        let shared_space = create_space(&pool, "Casa condivisa").await;
        add_membership(&pool, 1, manager, "proprietario").await;
        add_membership(&pool, shared_space, manager, "membro").await;
        let profile_id = direct_profile(&pool, manager, "Profilo").await;

        identity::with_actor(actor(manager, 1, false, "Gestore"), async {
            toggle_profile_space(&pool, profile_id, shared_space)
                .await
                .expect("condivisione");
            archive_profile(&pool, profile_id)
                .await
                .expect("archiviazione");
        })
        .await;

        let archived: i64 =
            sqlx::query_scalar("SELECT archiviato FROM profili_alimentari WHERE id = ?")
                .bind(profile_id)
                .fetch_one(&pool)
                .await
                .expect("profilo archiviato");
        let shares: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM profilo_alimentare_spazi WHERE profilo_alimentare_id = ?",
        )
        .bind(profile_id)
        .fetch_one(&pool)
        .await
        .expect("condivisioni");
        assert_eq!(archived, 1);
        assert_eq!(shares, 0);
    }

    #[tokio::test]
    async fn rinomina_registra_lo_storico_alimentazione() {
        let pool = test_pool().await;
        let manager = create_user(&pool, "Gestore").await;
        add_membership(&pool, 1, manager, "proprietario").await;
        let profile_id = direct_profile(&pool, manager, "Prima").await;

        identity::with_actor(actor(manager, 1, false, "Gestore"), async {
            assert!(rename_profile(&pool, profile_id, "Dopo")
                .await
                .expect("rinomina"));
        })
        .await;

        let event: (String, String, String) = sqlx::query_as(
            "SELECT modulo, componente, operazione FROM storico_eventi e JOIN storico_entita se ON se.id = e.entita_storico_id WHERE se.tipo_entita = 'profilo_alimentare' AND se.id_origine = ? ORDER BY e.id DESC LIMIT 1",
        )
        .bind(profile_id)
        .fetch_one(&pool)
        .await
        .expect("storico profilo");
        assert_eq!(event.0, "alimentazione");
        assert_eq!(event.1, "profili_alimentari");
        assert_eq!(event.2, "rinomina");
    }
}
