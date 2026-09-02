//! Backlog interno dei miglioramenti del gestionale.
//!
//! Workflow Step 7.2G.1:
//! `da_approvare -> da_fare -> fatto -> archivio`.
//! `fatto` significa implementato ma ancora da verificare dall'amministratore
//! principale. Solo dopo una verifica positiva l'elemento può essere archiviato.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    net::Download,
    prelude::*,
    types::{CopyTextButton, InlineKeyboardButton, InlineKeyboardMarkup, InputFile},
};
use tokio::{fs::File, task};

use crate::identity;

type Bot = crate::context_bot::ContextBot;

const MEDIA_ROOT: &str = "data/media/miglioramenti";
use crate::modules::liste;

const LIST_PAGE_SIZE: i64 = 5;
const MAX_DESCRIPTION_CHARS: usize = 50_000;
const DESCRIPTION_PAGE_CHARS: usize = 3000;
const DETAIL_DESCRIPTION_PREVIEW_CHARS: usize = 1800;
const EXPORT_ROOT: &str = "data/tmp/miglioramenti_export";
const EXPORT_SCRIPT: &str = "scripts/export_miglioramenti.py";
const PROJECT_EXPORT_ROOT: &str = "data/tmp/progetto_export";
const PROJECT_EXPORT_SCRIPT: &str = "scripts/export_progetto.py";
const EXPORT_ORPHAN_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone)]
struct ExportBundle {
    path: PathBuf,
    active: i64,
    archived: i64,
    attachments: i64,
    size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExportSelection(u8);

impl ExportSelection {
    const PENDING: u8 = 1;
    const TODO: u8 = 2;
    const DONE: u8 = 4;
    const ARCHIVED: u8 = 8;
    const ALL: u8 = Self::PENDING | Self::TODO | Self::DONE | Self::ARCHIVED;

    fn empty() -> Self {
        Self(0)
    }
    fn all() -> Self {
        Self(Self::ALL)
    }
    fn from_mask(mask: u8) -> Option<Self> {
        (mask <= Self::ALL).then_some(Self(mask))
    }
    fn mask(self) -> u8 {
        self.0
    }
    fn is_empty(self) -> bool {
        self.0 == 0
    }
    fn contains(self, bit: u8) -> bool {
        self.0 & bit != 0
    }

    fn toggle(self, bit: u8) -> Option<Self> {
        matches!(
            bit,
            Self::PENDING | Self::TODO | Self::DONE | Self::ARCHIVED
        )
        .then_some(Self(self.0 ^ bit))
    }

    fn scope_arg(self) -> String {
        let mut values = Vec::new();
        if self.contains(Self::PENDING) {
            values.push("pending");
        }
        if self.contains(Self::TODO) {
            values.push("todo");
        }
        if self.contains(Self::DONE) {
            values.push("done");
        }
        if self.contains(Self::ARCHIVED) {
            values.push("archived");
        }
        values.join(",")
    }

    fn label(self) -> String {
        if self.0 == Self::ALL {
            return "Tutti".to_string();
        }
        let mut labels = Vec::new();
        if self.contains(Self::PENDING) {
            labels.push("Da approvare");
        }
        if self.contains(Self::TODO) {
            labels.push("Da fare");
        }
        if self.contains(Self::DONE) {
            labels.push("Fatte");
        }
        if self.contains(Self::ARCHIVED) {
            labels.push("Archiviate");
        }
        labels.join(" + ")
    }
}

#[derive(Clone, Default)]
pub struct ImprovementSessionStore {
    inner: Arc<Mutex<HashMap<i64, ImprovementConversationState>>>,
}

#[derive(Debug, Clone)]
struct DescriptionDraftData {
    parts: Vec<String>,
    context: Option<String>,
    origin_token: Option<u64>,
    editing_id: Option<i64>,
    return_to: Option<(ListScope, i64)>,
}

#[derive(Debug, Clone)]
enum ImprovementConversationState {
    DescriptionDraft {
        parts: Vec<String>,
        context: Option<String>,
        origin_token: Option<u64>,
        editing_id: Option<i64>,
        return_to: Option<(ListScope, i64)>,
    },
    OptionalPhoto {
        description: String,
        context: Option<String>,
        origin_token: Option<u64>,
    },
    ExistingPhoto {
        improvement_id: i64,
    },
    VerificationPhoto {
        improvement_id: i64,
    },
    VerificationVideo {
        improvement_id: i64,
    },
    VerificationProblem {
        improvement_id: i64,
    },
    ExportReady {
        file_path: PathBuf,
        document_message_id: i32,
    },
}

impl ImprovementSessionStore {
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

    fn get(&self, chat_id: i64) -> Option<ImprovementConversationState> {
        self.with_sessions(|sessions| sessions.get(&chat_id).cloned())
    }

    fn set(&self, chat_id: i64, state: ImprovementConversationState) {
        self.with_sessions(|sessions| {
            sessions.insert(chat_id, state);
        });
    }

    fn take(&self, chat_id: i64) -> Option<ImprovementConversationState> {
        self.with_sessions(|sessions| sessions.remove(&chat_id))
    }

    fn with_sessions<T>(
        &self,
        f: impl FnOnce(&mut HashMap<i64, ImprovementConversationState>) -> T,
    ) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListScope {
    Mine,
    All,
    Pending,
    Todo,
    Done,
    Discarded,
}

/// I conteggi mostrati sulle etichette del menu' Miglioramenti (C7).
#[derive(Debug, Clone, Copy, Default)]
struct ConteggiMiglioramenti {
    miei: i64,
    attivi: i64,
    da_approvare: i64,
    da_fare: i64,
    da_verificare: i64,
    scartati: i64,
}

impl ConteggiMiglioramenti {
    fn solo_miei(miei: i64) -> Self {
        Self {
            miei,
            ..Self::default()
        }
    }

    fn per(self, scope: ListScope) -> i64 {
        match scope {
            ListScope::Mine => self.miei,
            ListScope::All => self.attivi,
            ListScope::Pending => self.da_approvare,
            ListScope::Todo => self.da_fare,
            ListScope::Done => self.da_verificare,
            ListScope::Discarded => self.scartati,
        }
    }
}

impl ListScope {
    fn token(self) -> &'static str {
        match self {
            Self::Mine => "mine",
            Self::All => "all",
            Self::Pending => "pending",
            Self::Todo => "todo",
            Self::Done => "done",
            Self::Discarded => "discarded",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Mine => "📋 I miei miglioramenti",
            Self::All => "🗂️ Tutti i miglioramenti",
            Self::Pending => "🟡 Da approvare",
            Self::Todo => "🟢 Da fare",
            Self::Done => "✅ Fatti da verificare",
            Self::Discarded => "❌ Scartati",
        }
    }

    fn admin_only(self) -> bool {
        !matches!(self, Self::Mine)
    }
}

#[derive(Debug, Clone, FromRow)]
struct ImprovementRecord {
    id: i64,
    autore_utente_id: i64,
    autore_nome: String,
    descrizione: String,
    modulo: Option<String>,
    contesto: Option<String>,
    stato: String,
    letto_admin_il: Option<String>,
    verifica_esito: Option<String>,
    verifica_note: Option<String>,
    verificato_il: Option<String>,
    creato_il: String,
    allegati: i64,
    prove: i64,
}

#[derive(Debug, Clone, FromRow)]
struct ArchivedImprovementRecord {
    autore_nome: String,
    descrizione: String,
    archiviato_il: String,
    allegati: i64,
    prove: i64,
}

#[derive(Debug, Clone, FromRow)]
struct AttachmentRecord {
    id: i64,
    percorso_file: String,
    descrizione: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct VerificationAttachmentRecord {
    id: i64,
    tipo: String,
    percorso_file: String,
    descrizione: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct VerificationPlan {
    titolo: String,
    istruzioni: String,
    azione_label: Option<String>,
    azione_callback: Option<String>,
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
    text_hint: Option<&str>,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;
    let command = text_hint.and_then(first_command);

    match command {
        Some("/miglioramenti") | Some("/miglioramento") => {
            sessions.clear_chat(chat_id);
            show_menu(bot, msg.chat.id, pool).await?;
            return Ok(true);
        }
        Some("/miglioramento_nuovo") => {
            start_new(bot, msg.chat.id, sessions).await?;
            return Ok(true);
        }
        Some("/miglioramenti_miei") => {
            sessions.clear_chat(chat_id);
            show_list(bot, msg.chat.id, pool, ListScope::Mine, 0).await?;
            return Ok(true);
        }
        Some("/miglioramenti_tutti") => {
            sessions.clear_chat(chat_id);
            show_list(bot, msg.chat.id, pool, ListScope::All, 0).await?;
            return Ok(true);
        }
        Some("/miglioramenti_da_approvare") => {
            sessions.clear_chat(chat_id);
            show_list(bot, msg.chat.id, pool, ListScope::Pending, 0).await?;
            return Ok(true);
        }
        Some("/miglioramenti_da_fare") => {
            sessions.clear_chat(chat_id);
            show_list(bot, msg.chat.id, pool, ListScope::Todo, 0).await?;
            return Ok(true);
        }
        Some("/miglioramenti_fatti") => {
            sessions.clear_chat(chat_id);
            show_list(bot, msg.chat.id, pool, ListScope::Done, 0).await?;
            return Ok(true);
        }
        Some("/miglioramenti_archivio") => {
            sessions.clear_chat(chat_id);
            show_archive(bot, msg.chat.id, pool, 0).await?;
            return Ok(true);
        }
        Some("/annulla") if sessions.has_active(chat_id) => {
            cancel_improvement_flow(bot, msg.chat.id, pool, sessions).await?;
            return Ok(true);
        }
        Some(_) => return Ok(false),
        None => {}
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        ImprovementConversationState::ExportReady { .. } => {
            bot.send_message(
                msg.chat.id,
                "📦 L'esportazione è pronta. Scarica il documento e poi premi ✅ Ho scaricato il file, oppure usa i pulsanti di navigazione per uscire.",
            )
            .reply_markup(export_ready_keyboard())
            .await?;
            Ok(true)
        }
        ImprovementConversationState::DescriptionDraft {
            parts,
            context,
            origin_token,
            editing_id,
            return_to,
        } => {
            append_description_part(
                bot,
                msg,
                sessions,
                DescriptionDraftData {
                    parts,
                    context,
                    origin_token,
                    editing_id,
                    return_to,
                },
                text_hint,
            )
            .await?;
            Ok(true)
        }
        ImprovementConversationState::OptionalPhoto {
            description,
            context,
            origin_token,
        } => {
            if msg.photo().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "📷 Sto aspettando una foto/screenshot. In alternativa usa ✅ Salva senza foto o ❌ Annulla.",
                )
                .reply_markup(optional_photo_keyboard())
                .await?;
                return Ok(true);
            }
            let improvement_id =
                match create_improvement(pool, &description, context.as_deref()).await {
                    Ok(id) => id,
                    Err(error) => {
                        tracing::error!(?error, "Errore creazione miglioramento con foto");
                        bot.send_message(msg.chat.id, "⚠️ Non riesco a creare il miglioramento.")
                            .await?;
                        return Ok(true);
                    }
                };
            match save_original_photo(bot, msg, pool, improvement_id).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    if !restore_origin_after_save(
                        bot,
                        msg.chat.id,
                        origin_token,
                        "✅ Miglioramento salvato con screenshot.",
                    )
                    .await?
                    {
                        bot.send_message(msg.chat.id, "✅ Miglioramento salvato con screenshot.")
                            .reply_markup(after_save_keyboard(improvement_id))
                            .await?;
                    }
                }
                Err(error) => {
                    tracing::error!(?error, improvement_id, "Errore allegato miglioramento");
                    let _ = delete_owned_improvement(pool, improvement_id).await;
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Non sono riuscito a salvare lo screenshot. Il miglioramento non è stato registrato: puoi riprovare.",
                    )
                    .reply_markup(optional_photo_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
        ImprovementConversationState::ExistingPhoto { improvement_id } => {
            if msg.photo().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "📷 Invia una foto/screenshot oppure usa ❌ Annulla.",
                )
                .reply_markup(flow_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            match save_original_photo(bot, msg, pool, improvement_id).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    bot.send_message(msg.chat.id, "✅ Screenshot aggiunto al miglioramento.")
                        .reply_markup(after_save_keyboard(improvement_id))
                        .await?;
                }
                Err(error) => {
                    tracing::error!(?error, improvement_id, "Errore aggiunta screenshot");
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Non riesco a salvare lo screenshot. Riprova.",
                    )
                    .reply_markup(flow_cancel_keyboard())
                    .await?;
                }
            }
            Ok(true)
        }
        ImprovementConversationState::VerificationPhoto { improvement_id } => {
            if msg.photo().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "📸 Invia lo screenshot del collaudo oppure usa ❌ Annulla.",
                )
                .reply_markup(flow_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            match save_verification_media(bot, msg, pool, improvement_id, "foto").await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    bot.send_message(msg.chat.id, "✅ Screenshot di verifica salvato.")
                        .await?;
                    show_verification(bot, msg.chat.id, pool, improvement_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_cancel_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        ImprovementConversationState::VerificationVideo { improvement_id } => {
            if msg.video().is_none() {
                bot.send_message(
                    msg.chat.id,
                    "🎥 Invia il video del collaudo oppure usa ❌ Annulla.",
                )
                .reply_markup(flow_cancel_keyboard())
                .await?;
                return Ok(true);
            }
            match save_verification_media(bot, msg, pool, improvement_id, "video").await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.delete_user_input(msg.chat.id, msg.id).await;
                    bot.send_message(msg.chat.id, "✅ Video di verifica salvato.")
                        .await?;
                    show_verification(bot, msg.chat.id, pool, improvement_id).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_cancel_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
        ImprovementConversationState::VerificationProblem { improvement_id } => {
            let note = text_hint.map(str::trim).filter(|value| !value.is_empty());
            let Some(note) = note else {
                bot.send_message(
                    msg.chat.id,
                    "🐛 Descrivi il problema trovato durante il collaudo.",
                )
                .reply_markup(flow_cancel_keyboard())
                .await?;
                return Ok(true);
            };
            match mark_verification_problem(pool, improvement_id, note).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Problema di collaudo registrato. Il miglioramento resta ✅ Fatto e non può essere archiviato finché non viene verificato con esito positivo.",
                    )
                    .await?;
                    show_detail(bot, msg.chat.id, pool, improvement_id, None).await?;
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                        .reply_markup(flow_cancel_keyboard())
                        .await?;
                }
            }
            Ok(true)
        }
    }
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    match data {
        "improve:noop" => return Ok(true),
        "menu:main" if sessions.has_active(chat_id.0) => {
            discard_pending_export(bot, chat_id, sessions).await;
            return Ok(false);
        }
        "improve:menu" => {
            discard_pending_export(bot, chat_id, sessions).await;
            sessions.clear_chat(chat_id.0);
            show_menu(bot, chat_id, pool).await?;
            return Ok(true);
        }
        "improve:new" => {
            start_new(bot, chat_id, sessions).await?;
            return Ok(true);
        }
        "improve:cancel" => {
            cancel_improvement_flow(bot, chat_id, pool, sessions).await?;
            return Ok(true);
        }
        "improve:save:no_photo" => {
            let Some(ImprovementConversationState::OptionalPhoto {
                description,
                context,
                origin_token,
            }) = sessions.take(chat_id.0)
            else {
                bot.send_message(chat_id, "⚠️ Non c'è un miglioramento pronto da salvare.")
                    .await?;
                return Ok(true);
            };
            match create_improvement(pool, &description, context.as_deref()).await {
                Ok(id) => {
                    if !restore_origin_after_save(
                        bot,
                        chat_id,
                        origin_token,
                        "✅ Miglioramento salvato.",
                    )
                    .await?
                    {
                        bot.send_message(chat_id, "✅ Miglioramento salvato.")
                            .reply_markup(after_save_keyboard(id))
                            .await?;
                    }
                }
                Err(error) => {
                    tracing::error!(?error, "Errore salvataggio miglioramento");
                    bot.send_message(chat_id, "⚠️ Non riesco a salvare il miglioramento.")
                        .await?;
                }
            }
            return Ok(true);
        }
        "improve:description:finish" => {
            finish_description(bot, chat_id, pool, sessions).await?;
            return Ok(true);
        }
        "improve:export" => {
            show_export_scope_menu(bot, chat_id, pool, sessions, ExportSelection::empty()).await?;
            return Ok(true);
        }
        "improve:export:project" => {
            start_project_export(bot, chat_id, pool, sessions).await?;
            return Ok(true);
        }
        "improve:export:downloaded" => {
            confirm_export_download(bot, chat_id, pool, sessions).await?;
            return Ok(true);
        }
        "improve:archive:list" => {
            sessions.clear_chat(chat_id.0);
            show_archive(bot, chat_id, pool, 0).await?;
            return Ok(true);
        }
        _ => {}
    }

    if let Some(rest) = data.strip_prefix("improve:export:toggle:") {
        let mut parts = rest.split(':');
        let selection = parts
            .next()
            .and_then(|value| value.parse::<u8>().ok())
            .and_then(ExportSelection::from_mask);
        let bit = parts.next().and_then(|value| value.parse::<u8>().ok());
        if let (Some(selection), Some(bit)) = (selection, bit) {
            if parts.next().is_none() {
                if let Some(updated) = selection.toggle(bit) {
                    show_export_scope_menu(bot, chat_id, pool, sessions, updated).await?;
                    return Ok(true);
                }
            }
        }
    }

    if let Some(selection) = data
        .strip_prefix("improve:export:all:")
        .and_then(|value| value.parse::<u8>().ok())
        .and_then(ExportSelection::from_mask)
    {
        let updated = if selection == ExportSelection::all() {
            ExportSelection::empty()
        } else {
            ExportSelection::all()
        };
        show_export_scope_menu(bot, chat_id, pool, sessions, updated).await?;
        return Ok(true);
    }

    if let Some(selection) = data
        .strip_prefix("improve:export:run:")
        .and_then(|value| value.parse::<u8>().ok())
        .and_then(ExportSelection::from_mask)
    {
        if selection.is_empty() {
            bot.send_message(chat_id, "⚠️ Seleziona almeno una categoria.")
                .reply_markup(export_scope_keyboard(selection))
                .await?;
        } else {
            start_export(bot, chat_id, pool, sessions, selection).await?;
        }
        return Ok(true);
    }

    if let Some(token) = data
        .strip_prefix("improve:context:")
        .and_then(|value| value.parse::<u64>().ok())
    {
        if let Some(snapshot) = bot.improve_context(chat_id.0, token) {
            let context = snapshot.summary();
            sessions.set(
                chat_id.0,
                ImprovementConversationState::DescriptionDraft {
                    parts: Vec::new(),
                    context: Some(context.clone()),
                    origin_token: Some(token),
                    editing_id: None,
                    return_to: None,
                },
            );
            bot.send_message_without_improve(
                chat_id,
                format!(
                    "💡 Migliora questa schermata

📍 Contesto corrente
{context}

Descrivi cosa vorresti cambiare o migliorare. Puoi usare più messaggi; quando hai finito premi ✅ Fine descrizione. Dopo potrai allegare uno screenshot facoltativo."
                ),
            )
            .reply_markup(flow_cancel_keyboard())
            .await?;
        } else {
            bot.send_message_without_improve(
                chat_id,
                "⚠️ Il contesto di questa schermata non è più disponibile. Apri nuovamente la sezione e premi 💡 Migliora.",
            )
            .reply_markup(menu_keyboard(is_primary_admin(pool).await.unwrap_or(false)))
            .await?;
        }
        return Ok(true);
    }

    if let Some((scope, page)) = parse_list_callback(data) {
        sessions.clear_chat(chat_id.0);
        show_list(bot, chat_id, pool, scope, page).await?;
        return Ok(true);
    }

    if let Some(page) = data
        .strip_prefix("improve:archive:page:")
        .and_then(parse_nonnegative_i64)
    {
        sessions.clear_chat(chat_id.0);
        show_archive(bot, chat_id, pool, page).await?;
        return Ok(true);
    }

    if let Some((id, return_to)) = parse_view_callback(data) {
        sessions.clear_chat(chat_id.0);
        show_detail(bot, chat_id, pool, id, return_to).await?;
        return Ok(true);
    }

    if let Some((id, page)) = parse_full_description_callback(data) {
        show_full_description(bot, chat_id, pool, id, page).await?;
        return Ok(true);
    }

    if let Some((id, return_to)) = parse_edit_callback(data) {
        if can_edit_owned(pool, id).await.unwrap_or(false) {
            let current_description = owned_improvement_description(pool, id)
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
            sessions.set(
                chat_id.0,
                ImprovementConversationState::DescriptionDraft {
                    parts: Vec::new(),
                    context: None,
                    origin_token: None,
                    editing_id: Some(id),
                    return_to,
                },
            );
            let copy_hint = if current_description.chars().count() <= 256 {
                "Puoi usare 📋 Copia testo originale, incollarlo nel campo di scrittura e cambiare solo ciò che serve."
            } else {
                "Il testo supera il limite di 256 caratteri del pulsante copia di Telegram: trovi comunque qui sotto il testo corrente come riferimento."
            };
            bot.send_message(
                chat_id,
                format!(
                    "✏️ Modifica testo\n\n{copy_hint}\n\nTesto attuale:\n{}\n\nInvia la nuova descrizione. Puoi dividerla in più messaggi; quando hai finito premi ✅ Fine descrizione.",
                    truncate(&current_description, 1200)
                ),
            )
            .reply_markup(edit_description_keyboard(&current_description))
            .await?;
        } else {
            bot.send_message(
                chat_id,
                "⚠️ Puoi modificare soltanto un tuo suggerimento attivo.",
            )
            .await?;
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:add_photo:") {
        if can_edit_owned(pool, id).await.unwrap_or(false) {
            sessions.set(
                chat_id.0,
                ImprovementConversationState::ExistingPhoto { improvement_id: id },
            );
            bot.send_message(
                chat_id,
                "📷 Invia ora lo screenshot da associare al miglioramento.",
            )
            .reply_markup(flow_cancel_keyboard())
            .await?;
        } else {
            bot.send_message(
                chat_id,
                "⚠️ Puoi aggiungere screenshot soltanto a un tuo suggerimento attivo.",
            )
            .await?;
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:photos:") {
        show_original_attachments(bot, chat_id, pool, id).await?;
        return Ok(true);
    }

    if let Some(raw) = data.strip_prefix("improve:photo:delete:") {
        let mut parts = raw.split(':');
        let improvement_id = parts.next().and_then(|value| value.parse::<i64>().ok());
        let attachment_id = parts.next().and_then(|value| value.parse::<i64>().ok());
        if parts.next().is_none() {
            if let (Some(improvement_id), Some(attachment_id)) = (improvement_id, attachment_id) {
                match delete_original_attachment(pool, improvement_id, attachment_id).await {
                    Ok(()) => {
                        bot.send_message(chat_id, "🗑️ Screenshot eliminato.")
                            .await?;
                        show_detail(bot, chat_id, pool, improvement_id, None).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
                return Ok(true);
            }
        }
    }

    if let Some(id) = parse_id(data, "improve:delete:ask:") {
        if can_edit_owned(pool, id).await.unwrap_or(false) {
            bot.send_message(
                chat_id,
                "🗑️ Eliminare questo suggerimento?\n\nL'operazione rimuove il miglioramento attivo e i suoi allegati.",
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![InlineKeyboardButton::callback(
                    "🗑️ Elimina definitivamente".to_string(),
                    format!("improve:delete:yes:{id}"),
                )],
                vec![
                    InlineKeyboardButton::callback(
                        "❌ Annulla".to_string(),
                        format!("improve:view:{id}"),
                    ),
                    InlineKeyboardButton::callback(
                        "🏠 Menù principale".to_string(),
                        "menu:main".to_string(),
                    ),
                ],
            ]))
            .await?;
        } else {
            bot.send_message(
                chat_id,
                "⚠️ Puoi eliminare soltanto un tuo suggerimento attivo.",
            )
            .await?;
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:delete:yes:") {
        match delete_owned_improvement(pool, id).await {
            Ok(()) => {
                bot.send_message(chat_id, "🗑️ Suggerimento eliminato.")
                    .await?;
                show_menu(bot, chat_id, pool).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:verify:") {
        sessions.clear_chat(chat_id.0);
        show_verification(bot, chat_id, pool, id).await?;
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:verify:photo:") {
        if ensure_primary_admin_for_done(pool, id).await.is_ok() {
            sessions.set(
                chat_id.0,
                ImprovementConversationState::VerificationPhoto { improvement_id: id },
            );
            bot.send_message(chat_id, "📸 Invia ora lo screenshot del collaudo.")
                .reply_markup(flow_cancel_keyboard())
                .await?;
        } else {
            bot.send_message(chat_id, "⚠️ Verifica non disponibile.")
                .await?;
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:verify:video:") {
        if ensure_primary_admin_for_done(pool, id).await.is_ok() {
            sessions.set(
                chat_id.0,
                ImprovementConversationState::VerificationVideo { improvement_id: id },
            );
            bot.send_message(chat_id, "🎥 Invia ora il video del collaudo.")
                .reply_markup(flow_cancel_keyboard())
                .await?;
        } else {
            bot.send_message(chat_id, "⚠️ Verifica non disponibile.")
                .await?;
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:verify:problem:") {
        if ensure_primary_admin_for_done(pool, id).await.is_ok() {
            sessions.set(
                chat_id.0,
                ImprovementConversationState::VerificationProblem { improvement_id: id },
            );
            bot.send_message(chat_id, "🐛 Descrivi cosa non funziona nel collaudo.")
                .reply_markup(flow_cancel_keyboard())
                .await?;
        } else {
            bot.send_message(chat_id, "⚠️ Verifica non disponibile.")
                .await?;
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:verify:ok:") {
        match verify_and_archive_improvement(pool, id).await {
            Ok(()) => {
                bot.send_message(chat_id, "✅ Miglioramento verificato e archiviato.")
                    .await?;
                show_list(bot, chat_id, pool, ListScope::Done, 0).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:verify:attachments:") {
        show_verification_attachments(bot, chat_id, pool, id).await?;
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:archive:") {
        match archive_verified_improvement(pool, id).await {
            Ok(()) => {
                bot.send_message(chat_id, "📦 Miglioramento verificato e archiviato.")
                    .await?;
                show_list(bot, chat_id, pool, ListScope::Done, 0).await?;
            }
            Err(error) => {
                tracing::warn!(?error, id, "Archiviazione miglioramento non riuscita");
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }

    if data == "improve:discarded:delete_all:ask" {
        if !is_primary_admin(pool).await.unwrap_or(false) {
            bot.send_message(chat_id, "⚠️ Operazione non disponibile.")
                .await?;
        } else {
            bot.send_message(
                chat_id,
                "🗑️ Eliminare tutti i miglioramenti scartati?

Verranno rimossi definitivamente anche i relativi allegati. Questa operazione non può essere annullata.",
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![InlineKeyboardButton::callback(
                    "✅ Elimina tutti gli scartati".to_string(),
                    "improve:discarded:delete_all:yes".to_string(),
                )],
                vec![InlineKeyboardButton::callback(
                    "❌ Annulla".to_string(),
                    "improve:list:discarded:0".to_string(),
                )],
            ]))
            .await?;
        }
        return Ok(true);
    }

    if data == "improve:discarded:delete_all:yes" {
        match delete_all_discarded_improvements(pool).await {
            Ok(count) => {
                bot.send_message(
                    chat_id,
                    format!("🗑️ Eliminati {count} miglioramenti scartati."),
                )
                .await?;
                show_list(bot, chat_id, pool, ListScope::Discarded, 0).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }

    if let Some(id) = parse_id(data, "improve:delete_discarded:") {
        if !is_primary_admin(pool).await.unwrap_or(false) {
            bot.send_message(chat_id, "⚠️ Comando non disponibile.")
                .await?;
        } else if let Err(error) = delete_discarded_improvement(pool, id).await {
            bot.send_message(chat_id, format!("⚠️ {error}")).await?;
        } else {
            bot.send_message(chat_id, "🗑️ Miglioramento scartato eliminato.")
                .await?;
            show_list(bot, chat_id, pool, ListScope::Discarded, 0).await?;
        }
        return Ok(true);
    }

    if let Some(rest) = data.strip_prefix("improve:status:") {
        let mut parts = rest.split(':');
        let id = parts.next().and_then(|value| value.parse::<i64>().ok());
        let state = parts.next();
        if parts.next().is_none() {
            if let (Some(id), Some(state)) = (id, state) {
                match set_status(pool, id, state).await {
                    Ok(()) => {
                        // Miglioramento #5: dopo il cambio stato si torna alla
                        // schermata Miglioramenti, non al dettaglio appena modificato.
                        bot.send_message(
                            chat_id,
                            format!(
                                "✅ Stato aggiornato: {} {}.",
                                status_icon(state),
                                status_label(state)
                            ),
                        )
                        .await?;
                        show_menu(bot, chat_id, pool).await?;
                    }
                    Err(error) => {
                        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                    }
                }
                return Ok(true);
            }
        }
    }

    Ok(false)
}

async fn append_description_part(
    bot: &Bot,
    msg: &Message,
    sessions: &ImprovementSessionStore,
    draft: DescriptionDraftData,
    text_hint: Option<&str>,
) -> ResponseResult<()> {
    let DescriptionDraftData {
        mut parts,
        context,
        origin_token,
        editing_id,
        return_to,
    } = draft;
    let Some(raw) = text_hint.map(str::trim).filter(|value| !value.is_empty()) else {
        bot.send_message(
            msg.chat.id,
            "✏️ Invia una parte della descrizione. Quando hai finito premi ✅ Fine descrizione.",
        )
        .reply_markup(description_keyboard())
        .await?;
        return Ok(());
    };

    let current_chars: usize = parts.iter().map(|part| part.chars().count()).sum();
    let separator_chars = parts.len().saturating_mul(2);
    let new_total = current_chars + separator_chars + raw.chars().count();
    if new_total > MAX_DESCRIPTION_CHARS {
        bot.send_message(
            msg.chat.id,
            format!(
                "⚠️ La descrizione supera il limite tecnico di sicurezza di {MAX_DESCRIPTION_CHARS} caratteri.\n\nLa parte appena inviata non è stata aggiunta."
            ),
        )
        .reply_markup(description_keyboard())
        .await?;
        sessions.set(
            msg.chat.id.0,
            ImprovementConversationState::DescriptionDraft {
                parts,
                context,
                origin_token,
                editing_id,
                return_to,
            },
        );
        return Ok(());
    }

    parts.push(raw.to_string());
    let parts_count = parts.len();
    sessions.set(
        msg.chat.id.0,
        ImprovementConversationState::DescriptionDraft {
            parts,
            context,
            origin_token,
            editing_id,
            return_to,
        },
    );
    bot.send_message(
        msg.chat.id,
        format!(
            "📝 Parte {parts_count} aggiunta · {new_total} caratteri totali.\n\nPuoi inviare un altro messaggio oppure premere ✅ Fine descrizione."
        ),
    )
    .reply_markup(description_keyboard())
    .await?;
    Ok(())
}

async fn finish_description(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
) -> ResponseResult<()> {
    let Some(ImprovementConversationState::DescriptionDraft {
        parts,
        context,
        origin_token,
        editing_id,
        return_to,
    }) = sessions.take(chat_id.0)
    else {
        bot.send_message(chat_id, "⚠️ Non c'è una descrizione in corso.")
            .await?;
        return Ok(());
    };

    let description = parts.join("\n\n");
    let description = match validate_description(&description) {
        Ok(value) => value.to_string(),
        Err(error) => {
            sessions.set(
                chat_id.0,
                ImprovementConversationState::DescriptionDraft {
                    parts,
                    context,
                    origin_token,
                    editing_id,
                    return_to,
                },
            );
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(description_keyboard())
                .await?;
            return Ok(());
        }
    };

    if let Some(improvement_id) = editing_id {
        match update_owned_description(pool, improvement_id, &description).await {
            Ok(()) => {
                bot.send_message(chat_id, "✅ Testo del miglioramento aggiornato.")
                    .await?;
                show_detail(bot, chat_id, pool, improvement_id, return_to).await?;
            }
            Err(error) => {
                sessions.set(
                    chat_id.0,
                    ImprovementConversationState::DescriptionDraft {
                        parts: vec![description],
                        context,
                        origin_token,
                        editing_id: Some(improvement_id),
                        return_to,
                    },
                );
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(description_keyboard())
                    .await?;
            }
        }
        return Ok(());
    }

    sessions.set(
        chat_id.0,
        ImprovementConversationState::OptionalPhoto {
            description,
            context,
            origin_token,
        },
    );
    bot.send_message(
        chat_id,
        "📷 Vuoi aggiungere uno screenshot?\n\nInvia una foto oppure premi ✅ Salva senza foto.",
    )
    .reply_markup(optional_photo_keyboard())
    .await?;
    Ok(())
}

async fn restore_origin_after_save(
    bot: &Bot,
    chat_id: ChatId,
    origin_token: Option<u64>,
    notice: &str,
) -> ResponseResult<bool> {
    let Some(token) = origin_token else {
        return Ok(false);
    };
    let Some(snapshot) = bot.improve_context(chat_id.0, token) else {
        return Ok(false);
    };
    let request = bot.send_message(chat_id, format!("{notice}\n\n{}", snapshot.screen_text));
    if let Some(keyboard) = snapshot.keyboard {
        request.reply_markup(keyboard).await?;
    } else {
        request.await?;
    }
    Ok(true)
}

async fn cancel_improvement_flow(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
) -> ResponseResult<()> {
    let mut origin_token = None;
    let mut editing_id = None;
    let mut return_to = None;
    match sessions.take(chat_id.0) {
        Some(ImprovementConversationState::DescriptionDraft {
            origin_token: token,
            editing_id: edited,
            return_to: target,
            ..
        }) => {
            origin_token = token;
            editing_id = edited;
            return_to = target;
        }
        Some(ImprovementConversationState::OptionalPhoto {
            origin_token: token,
            ..
        }) => origin_token = token,
        _ => {}
    }

    if let Some(token) = origin_token {
        if let Some(snapshot) = bot.improve_context(chat_id.0, token) {
            let request = bot.send_message(chat_id, snapshot.screen_text);
            if let Some(keyboard) = snapshot.keyboard {
                request.reply_markup(keyboard).await?;
            } else {
                request.await?;
            }
            return Ok(());
        }
    }

    if let Some(improvement_id) = editing_id {
        if let Some((scope, page)) = return_to {
            show_list(bot, chat_id, pool, scope, page).await?;
        } else {
            show_detail(bot, chat_id, pool, improvement_id, None).await?;
        }
        return Ok(());
    }

    bot.send_message(chat_id, "❌ Operazione annullata.")
        .reply_markup(menu_keyboard(is_primary_admin(pool).await.unwrap_or(false)))
        .await?;
    Ok(())
}

pub async fn cleanup_old_exports() -> Result<usize> {
    let root = export_root_path();
    let mut removed = 0usize;
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error).context("Impossibile leggere gli export temporanei"),
    };
    let now = SystemTime::now();
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("Impossibile leggere un export temporaneo")?
    {
        let path = entry.path();
        if !is_export_zip_path(&path) {
            continue;
        }
        let metadata = match entry.metadata().await {
            Ok(metadata) => metadata,
            Err(error) => {
                tracing::debug!(?error, ?path, "Metadata export temporaneo non leggibile");
                continue;
            }
        };
        let modified = match metadata.modified() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let age = now.duration_since(modified).unwrap_or_default();
        if age < EXPORT_ORPHAN_MAX_AGE {
            continue;
        }
        match tokio::fs::remove_file(&path).await {
            Ok(()) => removed += 1,
            Err(error) => tracing::debug!(?error, ?path, "Export orfano non eliminabile"),
        }
    }
    Ok(removed)
}

async fn cleanup_old_project_exports() -> Result<usize> {
    let root = project_export_root_path();
    let mut removed = 0usize;
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(error).context("Impossibile leggere gli export progetto temporanei")
        }
    };
    let now = SystemTime::now();
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("Impossibile leggere un export progetto temporaneo")?
    {
        let path = entry.path();
        if !is_export_zip_path(&path) {
            continue;
        }
        let metadata = match entry.metadata().await {
            Ok(metadata) => metadata,
            Err(error) => {
                tracing::debug!(?error, ?path, "Metadata export progetto non leggibile");
                continue;
            }
        };
        let modified = match metadata.modified() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let age = now.duration_since(modified).unwrap_or_default();
        if age < EXPORT_ORPHAN_MAX_AGE {
            continue;
        }
        match tokio::fs::remove_file(&path).await {
            Ok(()) => removed += 1,
            Err(error) => tracing::debug!(?error, ?path, "Export progetto orfano non eliminabile"),
        }
    }
    Ok(removed)
}

async fn discard_pending_export(bot: &Bot, chat_id: ChatId, sessions: &ImprovementSessionStore) {
    if !matches!(
        sessions.get(chat_id.0),
        Some(ImprovementConversationState::ExportReady { .. })
    ) {
        return;
    }

    let Some(ImprovementConversationState::ExportReady {
        file_path,
        document_message_id,
    }) = sessions.take(chat_id.0)
    else {
        return;
    };

    if is_export_zip_path(&file_path) {
        match tokio::fs::remove_file(&file_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(
                ?error,
                ?file_path,
                "Export abbandonato non eliminabile dall'S9"
            ),
        }
    } else {
        tracing::error!(?file_path, "Percorso export abbandonato rifiutato");
    }

    let message_id = teloxide::types::MessageId(document_message_id);
    bot.mark_transient_message(chat_id.0, message_id);
    bot.delete_user_input(chat_id, message_id).await;
}

async fn show_export_scope_menu(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
    selection: ExportSelection,
) -> ResponseResult<()> {
    if !is_primary_admin(pool).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ L'esportazione dei miglioramenti è riservata all'amministratore principale.",
        )
        .await?;
        return Ok(());
    }

    discard_pending_export(bot, chat_id, sessions).await;
    sessions.clear_chat(chat_id.0);
    bot.send_message_without_improve(
        chat_id,
        format!(
            "📦 Esporta miglioramenti\n\nSeleziona una o più categorie.\n\n📌 Selezione corrente: {}\n\nGli allegati vengono filtrati insieme ai relativi elementi.",
            if selection.is_empty() { "nessuna".to_string() } else { selection.label() }
        ),
    )
    .reply_markup(export_scope_keyboard(selection))
    .await?;
    Ok(())
}

async fn start_export(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
    selection: ExportSelection,
) -> ResponseResult<()> {
    if !is_primary_admin(pool).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ L'esportazione dei miglioramenti è riservata all'amministratore principale.",
        )
        .await?;
        return Ok(());
    }

    discard_pending_export(bot, chat_id, sessions).await;
    sessions.clear_chat(chat_id.0);
    if let Err(error) = cleanup_old_exports().await {
        tracing::warn!(
            ?error,
            "Pulizia preventiva export miglioramenti non riuscita"
        );
    }

    bot.send_message_without_improve(
        chat_id,
        format!(
            "⏳ Preparazione esportazione miglioramenti...\n\n📌 Filtro: {}\n\nCreo uno ZIP sanitizzato dello stato reale corrente.",
            selection.label()
        ),
    )
    .await?;

    let bundle = match build_export_bundle(selection).await {
        Ok(bundle) => bundle,
        Err(error) => {
            tracing::error!(?error, "Esportazione miglioramenti fallita");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a creare l'esportazione. Nessun dato del gestionale è stato modificato; puoi riprovare.",
            )
            .reply_markup(menu_keyboard(true))
            .await?;
            return Ok(());
        }
    };

    let file_name = bundle
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("gestionale-casa_handoff_miglioramenti.zip")
        .to_string();
    let document = match bot
        .send_document_untracked(chat_id, InputFile::file(bundle.path.clone()))
        .caption(format!("📦 {file_name}"))
        .await
    {
        Ok(message) => message,
        Err(error) => {
            tracing::error!(?error, path = ?bundle.path, "Invio export Telegram fallito");
            bot.send_message(
                chat_id,
                format!(
                    "⚠️ Telegram non ha ancora confermato l'invio dello ZIP. Per file grandi ora attendo molto più a lungo prima di mostrare questo avviso.\n\nSe il documento compare comunque dopo questo messaggio, puoi usarlo normalmente: non verrà eliminato. La copia temporanea resta sull'S9 e verrà ripulita automaticamente solo se diventa obsoleta.\n\n📄 File: {}",
                    bundle.path.file_name().and_then(|name| name.to_str()).unwrap_or("export miglioramenti")
                ),
            )
            .reply_markup(menu_keyboard(true))
            .await?;
            return Ok(());
        }
    };

    sessions.set(
        chat_id.0,
        ImprovementConversationState::ExportReady {
            file_path: bundle.path.clone(),
            document_message_id: document.id.0,
        },
    );

    bot.send_message_without_improve(
        chat_id,
        format!(
            "✅ Esportazione pronta\n\n📌 Filtro: {}\n📄 File: {file_name}\n💾 Dimensione: {}\n🗂️ Miglioramenti attivi: {}\n📦 Archiviati: {}\n🖼️ Allegati inclusi: {}\n\nScarica il documento qui sopra. Quando hai verificato che il download è completato premi ✅ Ho scaricato il file: solo allora eliminerò la copia temporanea dall'S9.",
            selection.label(),
            human_file_size(bundle.size_bytes),
            bundle.active,
            bundle.archived,
            bundle.attachments,
        ),
    )
    .reply_markup(export_ready_keyboard())
    .await?;
    Ok(())
}

async fn start_project_export(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
) -> ResponseResult<()> {
    if !is_primary_admin(pool).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ L'esportazione del progetto è riservata all'amministratore principale.",
        )
        .await?;
        return Ok(());
    }

    discard_pending_export(bot, chat_id, sessions).await;
    sessions.clear_chat(chat_id.0);
    if let Err(error) = cleanup_old_project_exports().await {
        tracing::warn!(?error, "Pulizia preventiva export progetto non riuscita");
    }

    bot.send_message_without_improve(
        chat_id,
        "⏳ Preparazione esportazione progetto...\n\nCreo uno ZIP tecnico sanitizzato con sorgenti, migration, documentazione, script e metadati Git. Escludo database, .env, token, backup, allegati utente e file runtime.",
    )
    .await?;

    let bundle = match build_project_export_bundle().await {
        Ok(bundle) => bundle,
        Err(error) => {
            tracing::error!(?error, "Esportazione progetto fallita");
            bot.send_message(
                chat_id,
                "⚠️ Non sono riuscito a creare l'esportazione del progetto. Nessun dato del gestionale è stato modificato; puoi riprovare.",
            )
            .reply_markup(menu_keyboard(true))
            .await?;
            return Ok(());
        }
    };

    let file_name = bundle
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("gestionale-casa_handoff_progetto.zip")
        .to_string();

    let document = match bot
        .send_document_untracked(chat_id, InputFile::file(bundle.path.clone()))
        .caption(format!("📦 {file_name}"))
        .await
    {
        Ok(message) => message,
        Err(error) => {
            tracing::error!(?error, path = ?bundle.path, "Invio export progetto Telegram fallito");
            let _ = tokio::fs::remove_file(&bundle.path).await;
            bot.send_message(
                chat_id,
                "⚠️ Telegram non ha confermato l'invio dello ZIP del progetto. Ho eliminato la copia temporanea dall'S9; puoi riprovare.",
            )
            .reply_markup(menu_keyboard(true))
            .await?;
            return Ok(());
        }
    };

    sessions.set(
        chat_id.0,
        ImprovementConversationState::ExportReady {
            file_path: bundle.path.clone(),
            document_message_id: document.id.0,
        },
    );

    bot.send_message_without_improve(
        chat_id,
        format!(
            "✅ Esportazione progetto pronta\n\n📄 File: {file_name}\n💾 Dimensione: {}\n📚 File inclusi: {}\n\n🔒 Esclusi: .env, database, token, backup, data/, target/, .git/, allegati utente e file temporanei.\n\nScarica il documento qui sopra e poi premi ✅ Ho scaricato il file.",
            human_file_size(bundle.size_bytes),
            bundle.files,
        ),
    )
    .reply_markup(export_ready_keyboard())
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ProjectExportBundle {
    path: PathBuf,
    files: i64,
    size_bytes: u64,
}

async fn build_project_export_bundle() -> Result<ProjectExportBundle> {
    task::spawn_blocking(build_project_export_bundle_blocking)
        .await
        .context("Task esportazione progetto interrotto")?
}

fn build_project_export_bundle_blocking() -> Result<ProjectExportBundle> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = root.join(PROJECT_EXPORT_ROOT);
    std::fs::create_dir_all(&output_dir)
        .context("Impossibile creare la directory export progetto")?;
    let script = root.join(PROJECT_EXPORT_SCRIPT);
    if !script.is_file() {
        bail!("Script export progetto non trovato: {}", script.display());
    }

    let mut last_error = None;
    for executable in ["python", "python3"] {
        let output = match Command::new(executable)
            .arg(&script)
            .arg("--root")
            .arg(&root)
            .arg("--output-dir")
            .arg(&output_dir)
            .current_dir(&root)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                last_error = Some(anyhow::anyhow!(error));
                continue;
            }
            Err(error) => return Err(error).context("Impossibile avviare l'exporter progetto"),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            bail!("Exporter progetto terminato con errore: {stderr}");
        }
        let stdout =
            String::from_utf8(output.stdout).context("Output exporter progetto non UTF-8")?;
        let mut values = HashMap::new();
        for line in stdout.lines() {
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.trim(), value.trim());
            }
        }
        let path = PathBuf::from(
            values
                .get("EXPORT_PATH")
                .copied()
                .context("EXPORT_PATH mancante dall'exporter progetto")?,
        );
        if !path.is_file() || !is_export_zip_path(&path) {
            bail!(
                "L'exporter progetto ha restituito un file non valido: {}",
                path.display()
            );
        }
        return Ok(ProjectExportBundle {
            path,
            files: parse_export_number(&values, "FILES")?,
            size_bytes: values
                .get("SIZE_BYTES")
                .copied()
                .context("SIZE_BYTES mancante dall'exporter progetto")?
                .parse()
                .context("SIZE_BYTES non valido")?,
        });
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Python non disponibile")))
        .context("Serve Python per creare lo ZIP del progetto")
}

async fn confirm_export_download(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ImprovementSessionStore,
) -> ResponseResult<()> {
    if !is_primary_admin(pool).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ Operazione riservata all'amministratore principale.",
        )
        .await?;
        return Ok(());
    }

    let Some(ImprovementConversationState::ExportReady {
        file_path,
        document_message_id,
    }) = sessions.take(chat_id.0)
    else {
        bot.send_message(
            chat_id,
            "⚠️ Non c'è un'esportazione corrente da confermare. Creane una nuova dal menu Miglioramenti.",
        )
        .reply_markup(menu_keyboard(true))
        .await?;
        return Ok(());
    };

    if !is_export_zip_path(&file_path) {
        tracing::error!(?file_path, "Percorso export rifiutato durante la pulizia");
        bot.send_message(
            chat_id,
            "⚠️ Per sicurezza non ho cancellato il file: il percorso non appartiene all'area export prevista.",
        )
        .reply_markup(menu_keyboard(true))
        .await?;
        return Ok(());
    }

    match tokio::fs::remove_file(&file_path).await {
        Ok(()) => {
            bot.delete_user_input(chat_id, teloxide::types::MessageId(document_message_id))
                .await;
            bot.send_message(
                chat_id,
                "✅ Download confermato. La copia temporanea dello ZIP è stata eliminata dall'S9 e il documento Telegram è stato rimosso.",
            )
            .reply_markup(menu_keyboard(true))
            .await?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            bot.delete_user_input(chat_id, teloxide::types::MessageId(document_message_id))
                .await;
            bot.send_message(
                chat_id,
                "✅ Download confermato. La copia temporanea non era più presente sull'S9 e il documento Telegram è stato rimosso.",
            )
            .reply_markup(menu_keyboard(true))
            .await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                ?file_path,
                "Impossibile eliminare export confermato"
            );
            sessions.set(
                chat_id.0,
                ImprovementConversationState::ExportReady {
                    file_path,
                    document_message_id,
                },
            );
            bot.send_message(
                chat_id,
                "⚠️ Hai confermato il download, ma non sono riuscito a eliminare la copia temporanea dall'S9. Il file resta disponibile e puoi riprovare la conferma.",
            )
            .reply_markup(export_ready_keyboard())
            .await?;
        }
    }
    Ok(())
}

async fn build_export_bundle(selection: ExportSelection) -> Result<ExportBundle> {
    task::spawn_blocking(move || build_export_bundle_blocking(selection))
        .await
        .context("Task esportazione miglioramenti interrotto")?
}

fn build_export_bundle_blocking(selection: ExportSelection) -> Result<ExportBundle> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = root.join(EXPORT_ROOT);
    std::fs::create_dir_all(&output_dir).context("Impossibile creare la directory export")?;
    let script = root.join(EXPORT_SCRIPT);
    if !script.is_file() {
        bail!("Script export non trovato: {}", script.display());
    }

    let mut last_error = None;
    for executable in ["python", "python3"] {
        let output = match Command::new(executable)
            .arg(&script)
            .arg("--root")
            .arg(&root)
            .arg("--output-dir")
            .arg(&output_dir)
            .arg("--scope")
            .arg(selection.scope_arg())
            .current_dir(&root)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                last_error = Some(anyhow::anyhow!(error));
                continue;
            }
            Err(error) => return Err(error).context("Impossibile avviare l'exporter"),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            bail!("Exporter terminato con errore: {stderr}");
        }
        let stdout = String::from_utf8(output.stdout).context("Output exporter non UTF-8")?;
        return parse_export_output(&stdout);
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Python non disponibile")))
        .context("Serve Python per creare lo ZIP dei miglioramenti")
}

fn parse_export_output(output: &str) -> Result<ExportBundle> {
    let mut values = HashMap::new();
    for line in output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim(), value.trim());
        }
    }
    let path = PathBuf::from(
        values
            .get("EXPORT_PATH")
            .copied()
            .context("EXPORT_PATH mancante dall'exporter")?,
    );
    if !path.is_file() || !is_export_zip_path(&path) {
        bail!(
            "L'exporter ha restituito un file non valido: {}",
            path.display()
        );
    }
    Ok(ExportBundle {
        path,
        active: parse_export_number(&values, "ACTIVE")?,
        archived: parse_export_number(&values, "ARCHIVED")?,
        attachments: parse_export_number(&values, "ATTACHMENTS")?,
        size_bytes: values
            .get("SIZE_BYTES")
            .copied()
            .context("SIZE_BYTES mancante dall'exporter")?
            .parse()
            .context("SIZE_BYTES non valido")?,
    })
}

fn parse_export_number(values: &HashMap<&str, &str>, key: &str) -> Result<i64> {
    values
        .get(key)
        .copied()
        .with_context(|| format!("{key} mancante dall'exporter"))?
        .parse()
        .with_context(|| format!("{key} non valido"))
}

fn export_root_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(EXPORT_ROOT)
}

fn project_export_root_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT_EXPORT_ROOT)
}

fn is_export_zip_path(path: &Path) -> bool {
    let parent = path.parent();
    let name = path.file_name().and_then(|name| name.to_str());

    let improvement_export = parent.is_some_and(|parent| parent == export_root_path().as_path())
        && name.is_some_and(|name| {
            name.starts_with("gestionale-casa_handoff_miglioramenti_") && name.ends_with(".zip")
        });

    let project_export = parent
        .is_some_and(|parent| parent == project_export_root_path().as_path())
        && name.is_some_and(|name| {
            name.starts_with("gestionale-casa_handoff_progetto_") && name.ends_with(".zip")
        });

    improvement_export || project_export
}

fn human_file_size(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;
    if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

async fn start_new(
    bot: &Bot,
    chat_id: ChatId,
    sessions: &ImprovementSessionStore,
) -> ResponseResult<()> {
    sessions.set(
        chat_id.0,
        ImprovementConversationState::DescriptionDraft {
            parts: Vec::new(),
            context: None,
            origin_token: None,
            editing_id: None,
            return_to: None,
        },
    );
    bot.send_message(
        chat_id,
        "💡 Nuovo miglioramento\n\nDescrivi cosa vorresti migliorare nel gestionale. Puoi usare più messaggi; quando hai finito premi ✅ Fine descrizione.",
    )
    .reply_markup(description_keyboard())
    .await?;
    Ok(())
}

pub async fn show_menu(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let admin = is_primary_admin(pool).await.unwrap_or(false);

    // C7: questa schermata era l'esempio da cui nasce la convenzione. I
    // conteggi c'erano gia', ma stavano in un blocco di testo sopra i
    // pulsanti — «🟡 Da approvare: 0» sopra un pulsante `🟡 Da approvare` —
    // e con loro c'era la frase «Usa i pulsanti qui sotto», che C2 vieta.
    // Ora il numero e' sull'etichetta e il testo non ha piu' niente da dire
    // che i pulsanti non dicano gia'.
    let conteggi = if admin {
        Some(ConteggiMiglioramenti {
            da_approvare: count_scope(pool, ListScope::Pending).await.unwrap_or(0),
            da_fare: count_scope(pool, ListScope::Todo).await.unwrap_or(0),
            da_verificare: count_scope(pool, ListScope::Done).await.unwrap_or(0),
            attivi: count_scope(pool, ListScope::All).await.unwrap_or(0),
            scartati: count_scope(pool, ListScope::Discarded).await.unwrap_or(0),
            miei: count_scope(pool, ListScope::Mine).await.unwrap_or(0),
        })
    } else {
        count_scope(pool, ListScope::Mine)
            .await
            .ok()
            .map(ConteggiMiglioramenti::solo_miei)
    };

    let text = if admin {
        // Resta la sola riga che nessun pulsante dice: dove finiscono i
        // miglioramenti una volta verificati.
        "💡 Miglioramenti\n\n📦 Un miglioramento verificato viene archiviato direttamente."
            .to_string()
    } else {
        "💡 Miglioramenti\n\nPuoi creare suggerimenti e gestire soltanto i tuoi: testo, screenshot ed eliminazione del suggerimento attivo. Lo stato amministrativo viene gestito dall'amministratore.".to_string()
    };
    bot.send_message(chat_id, text)
        .reply_markup(menu_keyboard_con_conteggi(admin, conteggi))
        .await?;
    Ok(())
}

async fn show_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    scope: ListScope,
    requested_page: i64,
) -> ResponseResult<()> {
    if scope.admin_only() && !is_primary_admin(pool).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ Comando riservato all'amministratore principale.",
        )
        .await?;
        return Ok(());
    }

    let total = match count_scope(pool, scope).await {
        Ok(total) => total,
        Err(error) => {
            tracing::error!(?error, ?scope, "Errore conteggio miglioramenti");
            bot.send_message(chat_id, "⚠️ Non riesco a contare i miglioramenti.")
                .await?;
            return Ok(());
        }
    };
    let pages = total_pages(total, LIST_PAGE_SIZE);
    let page = normalize_page(requested_page, pages);
    let rows = match fetch_scope(pool, scope, page).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(?error, ?scope, "Errore elenco miglioramenti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere i miglioramenti.")
                .await?;
            return Ok(());
        }
    };

    if rows.is_empty() {
        bot.send_message(chat_id, format!("{}\n\nNessun elemento.", scope.title()))
            .reply_markup(menu_keyboard(is_primary_admin(pool).await.unwrap_or(false)))
            .await?;
        return Ok(());
    }

    // C1: il testo non elenca piu' i miglioramenti — stanno sui pulsanti —
    // e l'etichetta porta cio' che li distingue: lo stato, il testo del
    // suggerimento, il segno di «non letto» e l'esito del collaudo, che prima
    // stava solo nel messaggio. Autore e allegati sono nel dettaglio.
    let lines = [liste::intestazione(scope.title(), total, page)];
    let mut buttons = Vec::new();
    let admin = is_primary_admin(pool).await.unwrap_or(false);
    for item in rows {
        let unread_suffix = if admin && item.letto_admin_il.is_none() {
            " 🆕"
        } else {
            ""
        };
        // Sul pulsante l'esito del collaudo e' la sola icona: la parola sta
        // nel dettaglio, dove c'e' lo spazio per scriverla.
        let verification_suffix = match item.verifica_esito.as_deref() {
            Some("problema") => " ⚠️",
            Some("ok") => " 🧪",
            _ if item.stato == "fatto" => " 🧪",
            _ => "",
        };
        buttons.push(vec![InlineKeyboardButton::callback(
            format!(
                "{} {}{}{}",
                display_status_icon(&item),
                liste::tronca(&item.descrizione, 32),
                unread_suffix,
                verification_suffix
            ),
            format!("improve:view:{}:{}:{}", item.id, scope.token(), page),
        )]);
    }
    if let Some(riga) = liste::riga_paginazione_da_totale(page, total, "improve:noop", |pagina| {
        format!("improve:list:{}:{}", scope.token(), pagina)
    }) {
        buttons.push(riga);
    }
    if scope == ListScope::Discarded && total > 0 {
        buttons.push(vec![InlineKeyboardButton::callback(
            "🗑 Elimina tutti gli scartati".to_string(),
            "improve:discarded:delete_all:ask".to_string(),
        )]);
    }
    buttons.push(vec![
        InlineKeyboardButton::callback("⬅️ Miglioramenti".to_string(), "improve:menu".to_string()),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]);
    bot.send_message(chat_id, lines.join("\n"))
        .reply_markup(InlineKeyboardMarkup::new(buttons))
        .await?;
    Ok(())
}

async fn show_archive(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    requested_page: i64,
) -> ResponseResult<()> {
    if !is_primary_admin(pool).await.unwrap_or(false) {
        bot.send_message(
            chat_id,
            "⚠️ Comando riservato all'amministratore principale.",
        )
        .await?;
        return Ok(());
    }
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti_archivio")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let pages = total_pages(total, LIST_PAGE_SIZE);
    let page = normalize_page(requested_page, pages);
    let offset = page * LIST_PAGE_SIZE;
    let rows: Vec<ArchivedImprovementRecord> = match sqlx::query_as(
        "SELECT u.nome_visualizzato AS autore_nome, a.descrizione, \
                strftime('%d/%m/%Y %H:%M', a.archiviato_il, 'localtime') AS archiviato_il, \
                (SELECT COUNT(*) FROM miglioramento_archivio_allegati aa WHERE aa.miglioramento_archivio_id = a.id) AS allegati, \
                (SELECT COUNT(*) FROM miglioramento_archivio_verifica_allegati va WHERE va.miglioramento_archivio_id = a.id) AS prove \
         FROM miglioramenti_archivio a \
         JOIN utenti u ON u.id = a.autore_utente_id \
         ORDER BY a.archiviato_il DESC, a.id DESC LIMIT ? OFFSET ?",
    )
    .bind(LIST_PAGE_SIZE)
    .bind(offset)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(?error, "Errore archivio miglioramenti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere l'archivio.")
                .await?;
            return Ok(());
        }
    };

    if rows.is_empty() {
        bot.send_message(chat_id, "📦 L'archivio dei miglioramenti è vuoto.")
            .reply_markup(menu_keyboard(true))
            .await?;
        return Ok(());
    }
    let mut lines = vec![format!(
        "{}",
        liste::intestazione("📦 Archivio miglioramenti", total, page)
    )];
    for item in rows {
        lines.push(String::new());
        lines.push(format!(
            "✅ {}\n👤 {} · Archiviato: {} · {}",
            truncate(&item.descrizione, 100),
            item.autore_nome,
            item.archiviato_il,
            attachment_summary(item.allegati, item.prove),
        ));
    }
    let mut keyboard = Vec::new();
    if let Some(riga) = liste::riga_paginazione_da_totale(page, total, "improve:noop", |pagina| {
        format!("improve:archive:page:{pagina}")
    }) {
        keyboard.push(riga);
    }
    keyboard.push(vec![InlineKeyboardButton::callback(
        "⬅️ Miglioramenti".to_string(),
        "improve:menu".to_string(),
    )]);
    keyboard.push(vec![InlineKeyboardButton::callback(
        "🏠 Menù principale".to_string(),
        "menu:main".to_string(),
    )]);
    bot.send_message(chat_id, lines.join("\n"))
        .reply_markup(InlineKeyboardMarkup::new(keyboard))
        .await?;
    Ok(())
}

async fn show_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    improvement_id: i64,
    return_to: Option<(ListScope, i64)>,
) -> ResponseResult<()> {
    let Some(item) = visible_improvement(pool, improvement_id)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(?error, improvement_id, "Errore dettaglio miglioramento");
            None
        })
    else {
        bot.send_message(chat_id, "⚠️ Miglioramento non disponibile.")
            .await?;
        return Ok(());
    };
    let admin = is_primary_admin(pool).await.unwrap_or(false);
    if admin && item.letto_admin_il.is_none() {
        if let Err(error) = mark_read(pool, improvement_id).await {
            tracing::warn!(
                ?error,
                improvement_id,
                "Lettura miglioramento non registrata"
            );
        }
    }
    let actor_user_id = identity::current_actor().utente_id;
    let owner = actor_user_id == Some(item.autore_utente_id);
    let module_line = item
        .modulo
        .as_deref()
        .map(|value| format!("\nSezione: {value}"))
        .unwrap_or_default();
    let context_line = item
        .contesto
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("\n\n📍 Contesto rilevato:\n{value}"))
        .unwrap_or_default();
    let verification_line = match item.verifica_esito.as_deref() {
        Some("ok") => format!(
            "\n🧪 Collaudo: verificato{}",
            item.verificato_il
                .as_deref()
                .map(|value| format!(" · {value}"))
                .unwrap_or_default()
        ),
        Some("problema") => format!(
            "\n⚠️ Collaudo: problema{}",
            item.verifica_note
                .as_deref()
                .map(|value| format!("\nNota: {value}"))
                .unwrap_or_default()
        ),
        _ if item.stato == "fatto" => "\n🧪 Collaudo: da verificare".to_string(),
        _ => String::new(),
    };
    let description_long = item.descrizione.chars().count() > DETAIL_DESCRIPTION_PREVIEW_CHARS;
    let description = if description_long {
        format!(
            "{}\n\n…\n\n📖 Descrizione abbreviata: usa il pulsante sotto per leggerla tutta.",
            truncate(&item.descrizione, DETAIL_DESCRIPTION_PREVIEW_CHARS)
        )
    } else {
        item.descrizione.clone()
    };
    let message = format!(
        "💡 Miglioramento\n\n{}{}\n\nStato: {} {}\nAutore: {}\nCreato: {}{}\n{}{}",
        description,
        context_line,
        display_status_icon(&item),
        display_status_label(&item),
        item.autore_nome,
        item.creato_il,
        module_line,
        attachment_summary(item.allegati, item.prove),
        verification_line,
    );
    bot.send_message(chat_id, message)
        .reply_markup(detail_keyboard(
            &item,
            admin,
            owner,
            return_to,
            description_long,
        ))
        .await?;
    Ok(())
}

async fn show_original_attachments(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    improvement_id: i64,
) -> ResponseResult<()> {
    if !can_view(pool, improvement_id).await.unwrap_or(false) {
        bot.send_message(chat_id, "⚠️ Miglioramento non disponibile.")
            .await?;
        return Ok(());
    }
    let attachments: Vec<AttachmentRecord> = match sqlx::query_as(
        "SELECT id, percorso_file, descrizione FROM miglioramento_allegati \
         WHERE miglioramento_id = ? ORDER BY creato_il, id",
    )
    .bind(improvement_id)
    .fetch_all(pool)
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(
                ?error,
                improvement_id,
                "Errore lettura screenshot miglioramento"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a leggere gli screenshot.")
                .await?;
            return Ok(());
        }
    };
    if attachments.is_empty() {
        bot.send_message(chat_id, "📷 Nessuno screenshot associato.")
            .reply_markup(after_save_keyboard(improvement_id))
            .await?;
        return Ok(());
    }
    let can_edit = can_edit_owned(pool, improvement_id).await.unwrap_or(false);
    for attachment in attachments {
        let path = PathBuf::from(&attachment.percorso_file);
        let caption = attachment
            .descrizione
            .clone()
            .unwrap_or_else(|| format!("📷 Screenshot #{}", attachment.id));
        if path.exists() {
            bot.send_photo(chat_id, InputFile::file(path))
                .caption(caption)
                .await?;
        } else {
            bot.send_message(
                chat_id,
                format!("⚠️ File screenshot non trovato: {caption}"),
            )
            .await?;
        }
        if can_edit {
            bot.send_message(chat_id, format!("Gestione screenshot #{}", attachment.id))
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::callback(
                        "🗑️ Elimina screenshot".to_string(),
                        format!("improve:photo:delete:{improvement_id}:{}", attachment.id),
                    ),
                ]]))
                .await?;
        }
    }
    bot.send_message(chat_id, "📷 Fine screenshot.")
        .reply_markup(after_save_keyboard(improvement_id))
        .await?;
    Ok(())
}

async fn show_verification(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    improvement_id: i64,
) -> ResponseResult<()> {
    if let Err(error) = ensure_primary_admin_for_done(pool, improvement_id).await {
        bot.send_message(chat_id, format!("⚠️ {error}")).await?;
        return Ok(());
    }
    let already_verified: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM miglioramenti WHERE id = ? AND stato = 'fatto' AND verifica_esito = 'ok')",
    )
    .bind(improvement_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);
    if already_verified {
        match archive_verified_improvement(pool, improvement_id).await {
            Ok(()) => {
                bot.send_message(
                    chat_id,
                    "✅ Miglioramento già verificato: archiviato automaticamente.",
                )
                .await?;
                show_list(bot, chat_id, pool, ListScope::Done, 0).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(());
    }

    let plan = sqlx::query_as::<_, VerificationPlan>(
        "SELECT titolo, istruzioni, azione_label, azione_callback FROM miglioramento_piani_verifica WHERE miglioramento_id = ?",
    )
    .bind(improvement_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let proof_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM miglioramento_verifica_allegati WHERE miglioramento_id = ?",
    )
    .bind(improvement_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let (title, instructions, action) = if let Some(plan) = plan {
        (
            plan.titolo,
            plan.istruzioni,
            plan.azione_label.zip(plan.azione_callback),
        )
    } else {
        (
            "Verifica manuale".to_string(),
            "Esegui il collaudo del miglioramento. Puoi allegare screenshot/video, segnalare un problema oppure confermare che funziona.".to_string(),
            None,
        )
    };
    let mut rows = Vec::new();
    if let Some((label, callback)) = action {
        rows.push(vec![InlineKeyboardButton::callback(label, callback)]);
    }
    rows.push(vec![
        InlineKeyboardButton::callback(
            "📸 Invia screenshot".to_string(),
            format!("improve:verify:photo:{improvement_id}"),
        ),
        InlineKeyboardButton::callback(
            "🎥 Invia video".to_string(),
            format!("improve:verify:video:{improvement_id}"),
        ),
    ]);
    if proof_count > 0 {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("📎 Vedi prove ({proof_count})"),
            format!("improve:verify:attachments:{improvement_id}"),
        )]);
    }
    rows.push(vec![
        InlineKeyboardButton::callback(
            "🐛 Problema trovato".to_string(),
            format!("improve:verify:problem:{improvement_id}"),
        ),
        InlineKeyboardButton::callback(
            "✅ Verificato".to_string(),
            format!("improve:verify:ok:{improvement_id}"),
        ),
    ]);
    rows.push(vec![
        InlineKeyboardButton::callback(
            "⬅️ Miglioramento".to_string(),
            format!("improve:view:{improvement_id}"),
        ),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]);
    bot.send_message(
        chat_id,
        format!(
            "🧪 Verifica miglioramento #{improvement_id}\n\n{title}\n\n{instructions}\n\n📎 Prove allegate: {proof_count}\n\nQuando il test richiede una schermata usa il pulsante di apertura rapida qui sotto."
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_verification_attachments(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    improvement_id: i64,
) -> ResponseResult<()> {
    if ensure_primary_admin_for_done(pool, improvement_id)
        .await
        .is_err()
    {
        bot.send_message(chat_id, "⚠️ Prove di verifica non disponibili.")
            .await?;
        return Ok(());
    }
    let rows: Vec<VerificationAttachmentRecord> = sqlx::query_as(
        "SELECT id, tipo, percorso_file, descrizione FROM miglioramento_verifica_allegati \
         WHERE miglioramento_id = ? ORDER BY creato_il, id",
    )
    .bind(improvement_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    if rows.is_empty() {
        bot.send_message(chat_id, "📎 Nessuna prova allegata.")
            .await?;
        return Ok(());
    }
    for item in rows {
        let path = PathBuf::from(&item.percorso_file);
        let caption = item
            .descrizione
            .unwrap_or_else(|| format!("Prova #{}", item.id));
        if !path.exists() {
            bot.send_message(chat_id, format!("⚠️ File prova non trovato: {caption}"))
                .await?;
            continue;
        }
        if item.tipo == "video" {
            bot.send_video(chat_id, InputFile::file(path))
                .caption(caption)
                .await?;
        } else {
            bot.send_photo(chat_id, InputFile::file(path))
                .caption(caption)
                .await?;
        }
    }
    bot.send_message(chat_id, "📎 Fine prove di collaudo.")
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback(
                "⬅️ Verifica".to_string(),
                format!("improve:verify:{improvement_id}"),
            ),
            InlineKeyboardButton::callback(
                "🏠 Menù principale".to_string(),
                "menu:main".to_string(),
            ),
        ]]))
        .await?;
    Ok(())
}

async fn create_improvement(
    pool: &SqlitePool,
    description: &str,
    context: Option<&str>,
) -> Result<i64> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Miglioramento non disponibile per un attore di sistema")?;
    let admin = identity::is_primary_admin(pool, &actor).await?;
    let state = if admin { "da_fare" } else { "da_approvare" };
    let result = sqlx::query(
        "INSERT INTO miglioramenti (autore_utente_id, descrizione, contesto, stato, letto_admin_il) \
         VALUES (?, ?, ?, ?, CASE WHEN ? = 1 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE NULL END)",
    )
    .bind(user_id)
    .bind(description.trim())
    .bind(context.map(str::trim).filter(|value| !value.is_empty()))
    .bind(state)
    .bind(if admin { 1_i64 } else { 0_i64 })
    .execute(pool)
    .await
    .context("Impossibile creare il miglioramento")?;
    Ok(result.last_insert_rowid())
}

async fn update_owned_description(
    pool: &SqlitePool,
    improvement_id: i64,
    description: &str,
) -> Result<()> {
    let description = validate_description(description)?;
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let result = sqlx::query(
        "UPDATE miglioramenti SET descrizione = ?, \
         stato = CASE WHEN stato = 'fatto' THEN 'da_fare' ELSE stato END, \
         fatto_il = CASE WHEN stato = 'fatto' THEN NULL ELSE fatto_il END, \
         verifica_esito = CASE WHEN stato = 'fatto' THEN NULL ELSE verifica_esito END, \
         verifica_note = CASE WHEN stato = 'fatto' THEN NULL ELSE verifica_note END, \
         verificato_il = CASE WHEN stato = 'fatto' THEN NULL ELSE verificato_il END, \
         verificato_da_utente_id = CASE WHEN stato = 'fatto' THEN NULL ELSE verificato_da_utente_id END, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND autore_utente_id = ?",
    )
    .bind(description)
    .bind(improvement_id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare il miglioramento")?;
    if result.rows_affected() != 1 {
        bail!("Miglioramento non disponibile o non di tua proprietà");
    }
    Ok(())
}

async fn delete_owned_improvement(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let original_paths: Vec<String> = sqlx::query_scalar(
        "SELECT a.percorso_file FROM miglioramento_allegati a \
         JOIN miglioramenti m ON m.id = a.miglioramento_id \
         WHERE m.id = ? AND m.autore_utente_id = ?",
    )
    .bind(improvement_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli allegati")?;
    let verification_paths: Vec<String> = sqlx::query_scalar(
        "SELECT a.percorso_file FROM miglioramento_verifica_allegati a \
         JOIN miglioramenti m ON m.id = a.miglioramento_id \
         WHERE m.id = ? AND m.autore_utente_id = ?",
    )
    .bind(improvement_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le prove di verifica")?;
    let result = sqlx::query("DELETE FROM miglioramenti WHERE id = ? AND autore_utente_id = ?")
        .bind(improvement_id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Impossibile eliminare il miglioramento")?;
    if result.rows_affected() != 1 {
        bail!("Miglioramento non disponibile o non di tua proprietà");
    }
    cleanup_files(
        original_paths
            .into_iter()
            .chain(verification_paths)
            .collect(),
    )
    .await;
    cleanup_improvement_directory(improvement_id).await;
    Ok(())
}

async fn reset_to_todo_after_content_change(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    sqlx::query(
        "UPDATE miglioramenti SET stato = 'da_fare', fatto_il = NULL, verifica_esito = NULL, \
         verifica_note = NULL, verificato_il = NULL, verificato_da_utente_id = NULL, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND stato = 'fatto'",
    )
    .bind(improvement_id)
    .execute(pool)
    .await
    .context("Impossibile riaprire il miglioramento dopo la modifica")?;
    Ok(())
}

async fn delete_original_attachment(
    pool: &SqlitePool,
    improvement_id: i64,
    attachment_id: i64,
) -> Result<()> {
    if !can_edit_owned(pool, improvement_id).await? {
        bail!("Puoi modificare soltanto i tuoi screenshot");
    }
    let path: Option<String> = sqlx::query_scalar(
        "SELECT percorso_file FROM miglioramento_allegati WHERE id = ? AND miglioramento_id = ?",
    )
    .bind(attachment_id)
    .bind(improvement_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere lo screenshot")?;
    let Some(path) = path else {
        bail!("Screenshot non disponibile");
    };
    sqlx::query("DELETE FROM miglioramento_allegati WHERE id = ? AND miglioramento_id = ?")
        .bind(attachment_id)
        .bind(improvement_id)
        .execute(pool)
        .await
        .context("Impossibile eliminare lo screenshot")?;
    reset_to_todo_after_content_change(pool, improvement_id).await?;
    if let Err(error) = tokio::fs::remove_file(&path).await {
        if Path::new(&path).exists() {
            tracing::warn!(?error, %path, "Screenshot non eliminato dal filesystem");
        }
    }
    Ok(())
}

async fn save_original_photo(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    improvement_id: i64,
) -> Result<()> {
    if !can_edit_owned(pool, improvement_id).await? {
        bail!("Puoi aggiungere screenshot soltanto a un tuo suggerimento attivo");
    }
    let photo_sizes = msg.photo().context("Foto Telegram non presente")?;
    let photo = photo_sizes
        .iter()
        .max_by_key(|photo| u64::from(photo.width) * u64::from(photo.height))
        .context("Foto Telegram non leggibile")?;
    let telegram_file = bot
        .get_file(photo.file.id.clone())
        .await
        .context("Impossibile leggere il file Telegram")?;
    let extension = safe_extension(&telegram_file.path, "jpg");
    let directory = PathBuf::from(MEDIA_ROOT).join(improvement_id.to_string());
    tokio::fs::create_dir_all(&directory)
        .await
        .context("Impossibile creare la cartella screenshot")?;
    let filename = format!("telegram_{}_{}.{}", msg.chat.id.0, msg.id.0, extension);
    let local_path = directory.join(filename);
    download_to_path(bot, &telegram_file.path, &local_path).await?;
    let description = msg
        .caption()
        .map(str::trim)
        .filter(|caption| !caption.is_empty());
    let path_for_db = local_path.to_string_lossy().into_owned();
    if let Err(error) = sqlx::query(
        "INSERT INTO miglioramento_allegati (miglioramento_id, percorso_file, descrizione) VALUES (?, ?, ?)",
    )
    .bind(improvement_id)
    .bind(&path_for_db)
    .bind(description)
    .execute(pool)
    .await
    {
        let _ = tokio::fs::remove_file(&local_path).await;
        return Err(error).context("Impossibile registrare lo screenshot");
    }
    reset_to_todo_after_content_change(pool, improvement_id).await?;
    Ok(())
}

async fn save_verification_media(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    improvement_id: i64,
    kind: &str,
) -> Result<()> {
    ensure_primary_admin_for_done(pool, improvement_id).await?;
    let file_id = match kind {
        "foto" => msg
            .photo()
            .and_then(|photos| {
                photos
                    .iter()
                    .max_by_key(|photo| u64::from(photo.width) * u64::from(photo.height))
            })
            .map(|photo| photo.file.id.clone())
            .context("Foto Telegram non presente")?,
        "video" => msg
            .video()
            .map(|video| video.file.id.clone())
            .context("Video Telegram non presente")?,
        _ => bail!("Tipo allegato di verifica non valido"),
    };
    let telegram_file = bot
        .get_file(file_id)
        .await
        .context("Impossibile leggere il file Telegram")?;
    let fallback = if kind == "video" { "mp4" } else { "jpg" };
    let extension = safe_extension(&telegram_file.path, fallback);
    let directory = PathBuf::from(MEDIA_ROOT)
        .join(improvement_id.to_string())
        .join("verifica");
    tokio::fs::create_dir_all(&directory)
        .await
        .context("Impossibile creare la cartella di verifica")?;
    let filename = format!("{}_{}_{}.{}", kind, msg.chat.id.0, msg.id.0, extension);
    let local_path = directory.join(filename);
    download_to_path(bot, &telegram_file.path, &local_path).await?;
    let description = msg
        .caption()
        .map(str::trim)
        .filter(|caption| !caption.is_empty());
    let path_for_db = local_path.to_string_lossy().into_owned();
    if let Err(error) = sqlx::query(
        "INSERT INTO miglioramento_verifica_allegati (miglioramento_id, tipo, percorso_file, descrizione) VALUES (?, ?, ?, ?)",
    )
    .bind(improvement_id)
    .bind(kind)
    .bind(&path_for_db)
    .bind(description)
    .execute(pool)
    .await
    {
        let _ = tokio::fs::remove_file(&local_path).await;
        return Err(error).context("Impossibile registrare la prova di verifica");
    }
    Ok(())
}

async fn download_to_path(bot: &Bot, remote_path: &str, local_path: &Path) -> Result<()> {
    let mut destination = File::create(local_path)
        .await
        .context("Impossibile creare il file locale")?;
    if let Err(error) = bot.download_file(remote_path, &mut destination).await {
        drop(destination);
        let _ = tokio::fs::remove_file(local_path).await;
        return Err(error).context("Download Telegram fallito");
    }
    drop(destination);
    Ok(())
}

async fn visible_improvement(
    pool: &SqlitePool,
    improvement_id: i64,
) -> Result<Option<ImprovementRecord>> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(None);
    };
    let admin = identity::is_primary_admin(pool, &actor).await?;
    let base = "SELECT m.id, m.autore_utente_id, u.nome_visualizzato AS autore_nome, \
                m.descrizione, m.modulo, m.contesto, m.stato, m.letto_admin_il, \
                m.verifica_esito, m.verifica_note, m.verificato_il, \
                strftime('%d/%m/%Y %H:%M', m.creato_il, 'localtime') AS creato_il, \
                (SELECT COUNT(*) FROM miglioramento_allegati a WHERE a.miglioramento_id = m.id) AS allegati, \
                (SELECT COUNT(*) FROM miglioramento_verifica_allegati va WHERE va.miglioramento_id = m.id) AS prove \
         FROM miglioramenti m JOIN utenti u ON u.id = m.autore_utente_id ";
    let sql = if admin {
        format!("{base} WHERE m.id = ?")
    } else {
        format!("{base} WHERE m.id = ? AND m.autore_utente_id = ?")
    };
    let query = sqlx::query_as::<_, ImprovementRecord>(&sql).bind(improvement_id);
    if admin {
        query
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere il miglioramento")
    } else {
        query
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere il miglioramento")
    }
}

async fn show_full_description(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    improvement_id: i64,
    requested_page: i64,
) -> ResponseResult<()> {
    let Some(item) = visible_improvement(pool, improvement_id)
        .await
        .unwrap_or(None)
    else {
        bot.send_message(chat_id, "⚠️ Miglioramento non disponibile.")
            .await?;
        return Ok(());
    };
    let pages = split_text_pages(&item.descrizione, DESCRIPTION_PAGE_CHARS);
    let page = requested_page.clamp(0, pages.len().saturating_sub(1) as i64);
    let mut rows = Vec::new();
    if let Some(nav) = liste::riga_paginazione(page, pages.len() as i64, "improve:noop", |pagina| {
        format!("improve:description:full:{improvement_id}:{pagina}")
    }) {
        rows.push(nav);
    }
    rows.push(vec![
        InlineKeyboardButton::callback(
            "⬅️ Indietro".to_string(),
            format!("improve:view:{improvement_id}"),
        ),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]);
    bot.send_message(
        chat_id,
        format!(
            "📖 Descrizione completa · pagina {}/{}\n\n{}",
            page + 1,
            pages.len(),
            pages[page as usize]
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn can_view(pool: &SqlitePool, improvement_id: i64) -> Result<bool> {
    Ok(visible_improvement(pool, improvement_id).await?.is_some())
}

async fn can_edit_owned(pool: &SqlitePool, improvement_id: i64) -> Result<bool> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(false);
    };
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM miglioramenti WHERE id = ? AND autore_utente_id = ?)",
    )
    .bind(improvement_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare la proprietà del miglioramento")
}

async fn owned_improvement_description(
    pool: &SqlitePool,
    improvement_id: i64,
) -> Result<Option<String>> {
    let actor = identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(None);
    };
    sqlx::query_scalar(
        "SELECT descrizione FROM miglioramenti WHERE id = ? AND autore_utente_id = ?",
    )
    .bind(improvement_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il testo originale del miglioramento")
}

async fn mark_read(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    if !is_primary_admin(pool).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    sqlx::query(
        "UPDATE miglioramenti SET letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?",
    )
    .bind(improvement_id)
    .execute(pool)
    .await
    .context("Impossibile segnare il miglioramento come letto")?;
    Ok(())
}

async fn set_status(pool: &SqlitePool, improvement_id: i64, state: &str) -> Result<()> {
    if !is_primary_admin(pool).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    if !matches!(state, "da_fare" | "fatto" | "scartato") {
        bail!("Stato miglioramento non valido");
    }
    let affected = if state == "fatto" {
        sqlx::query(
            "UPDATE miglioramenti SET stato = 'fatto', fatto_il = COALESCE(fatto_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
             verifica_esito = NULL, verifica_note = NULL, verificato_il = NULL, verificato_da_utente_id = NULL, \
             letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(improvement_id)
        .execute(pool)
        .await
        .context("Impossibile segnare il miglioramento come Fatto")?
        .rows_affected()
    } else {
        sqlx::query(
            "UPDATE miglioramenti SET stato = ?, fatto_il = NULL, verifica_esito = NULL, verifica_note = NULL, \
             verificato_il = NULL, verificato_da_utente_id = NULL, \
             letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(state)
        .bind(improvement_id)
        .execute(pool)
        .await
        .context("Impossibile aggiornare lo stato")?
        .rows_affected()
    };
    if affected != 1 {
        bail!("Miglioramento non trovato");
    }
    Ok(())
}

async fn verify_and_archive_improvement(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    ensure_primary_admin_for_done(pool, improvement_id).await?;
    let admin_id = identity::current_actor()
        .utente_id
        .context("Amministratore senza identità interna")?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare verifica e archiviazione")?;

    let updated = sqlx::query(
        "UPDATE miglioramenti SET verifica_esito = 'ok', verifica_note = NULL, \
         verificato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), verificato_da_utente_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND stato = 'fatto'",
    )
    .bind(admin_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile confermare il collaudo")?;

    if updated.rows_affected() != 1 {
        bail!("Miglioramento non disponibile per il collaudo");
    }

    let inserted = sqlx::query(
        "INSERT INTO miglioramenti_archivio (miglioramento_origine_id, autore_utente_id, descrizione, modulo, contesto, creato_il, \
            completato_il, archiviato_da_utente_id, verifica_esito, verifica_note, verificato_il, verificato_da_utente_id) \
         SELECT id, autore_utente_id, descrizione, modulo, contesto, creato_il, COALESCE(fatto_il, aggiornato_il), ?, \
                verifica_esito, verifica_note, verificato_il, verificato_da_utente_id \
         FROM miglioramenti WHERE id = ? AND stato = 'fatto' AND verifica_esito = 'ok'",
    )
    .bind(admin_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare il miglioramento verificato")?;

    if inserted.rows_affected() != 1 {
        bail!("Impossibile creare la copia archiviata del miglioramento");
    }
    let archive_id = inserted.last_insert_rowid();

    sqlx::query(
        "INSERT INTO miglioramento_archivio_allegati \
         (miglioramento_archivio_id, tipo, percorso_file, descrizione, creato_il) \
         SELECT ?, tipo, percorso_file, descrizione, creato_il \
         FROM miglioramento_allegati WHERE miglioramento_id = ?",
    )
    .bind(archive_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare gli allegati originali")?;

    sqlx::query(
        "INSERT INTO miglioramento_archivio_verifica_allegati \
         (miglioramento_archivio_id, tipo, percorso_file, descrizione, creato_il) \
         SELECT ?, tipo, percorso_file, descrizione, creato_il \
         FROM miglioramento_verifica_allegati WHERE miglioramento_id = ?",
    )
    .bind(archive_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare le prove di collaudo")?;

    sqlx::query(
        "DELETE FROM miglioramenti WHERE id = ? AND stato = 'fatto' AND verifica_esito = 'ok'",
    )
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile rimuovere il miglioramento dal backlog")?;

    tx.commit()
        .await
        .context("Impossibile completare verifica e archiviazione")?;
    Ok(())
}

async fn mark_verification_problem(
    pool: &SqlitePool,
    improvement_id: i64,
    note: &str,
) -> Result<()> {
    ensure_primary_admin_for_done(pool, improvement_id).await?;
    let admin_id = identity::current_actor()
        .utente_id
        .context("Amministratore senza identità interna")?;
    sqlx::query(
        "UPDATE miglioramenti SET verifica_esito = 'problema', verifica_note = ?, \
         verificato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), verificato_da_utente_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND stato = 'fatto'",
    )
    .bind(note.trim())
    .bind(admin_id)
    .bind(improvement_id)
    .execute(pool)
    .await
    .context("Impossibile registrare il problema di collaudo")?;
    Ok(())
}

async fn ensure_primary_admin_for_done(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    if !is_primary_admin(pool).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    let done: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM miglioramenti WHERE id = ? AND stato = 'fatto')",
    )
    .bind(improvement_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare lo stato del miglioramento")?;
    if !done {
        bail!("Il miglioramento deve essere nello stato Fatto per il collaudo");
    }
    Ok(())
}

async fn archive_verified_improvement(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    ensure_primary_admin_for_done(pool, improvement_id).await?;
    let actor = identity::current_actor();
    let admin_user_id = actor
        .utente_id
        .context("Amministratore privo di identità interna")?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione di archiviazione")?;
    let inserted = sqlx::query(
        "INSERT INTO miglioramenti_archivio (miglioramento_origine_id, autore_utente_id, descrizione, modulo, contesto, creato_il, \
            completato_il, archiviato_da_utente_id, verifica_esito, verifica_note, verificato_il, verificato_da_utente_id) \
         SELECT id, autore_utente_id, descrizione, modulo, contesto, creato_il, COALESCE(fatto_il, aggiornato_il), ?, \
                verifica_esito, verifica_note, verificato_il, verificato_da_utente_id \
         FROM miglioramenti WHERE id = ? AND stato = 'fatto' AND verifica_esito = 'ok'",
    )
    .bind(admin_user_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare il miglioramento")?;
    if inserted.rows_affected() != 1 {
        bail!("Prima di archiviare devi segnare il miglioramento come ✅ Verificato");
    }
    let archive_id = inserted.last_insert_rowid();
    sqlx::query(
        "INSERT INTO miglioramento_archivio_allegati (miglioramento_archivio_id, tipo, percorso_file, descrizione, creato_il) \
         SELECT ?, tipo, percorso_file, descrizione, creato_il FROM miglioramento_allegati WHERE miglioramento_id = ?",
    )
    .bind(archive_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare gli allegati originali")?;
    sqlx::query(
        "INSERT INTO miglioramento_archivio_verifica_allegati (miglioramento_archivio_id, tipo, percorso_file, descrizione, creato_il) \
         SELECT ?, tipo, percorso_file, descrizione, creato_il FROM miglioramento_verifica_allegati WHERE miglioramento_id = ?",
    )
    .bind(archive_id)
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile archiviare le prove di collaudo")?;
    sqlx::query(
        "DELETE FROM miglioramenti WHERE id = ? AND stato = 'fatto' AND verifica_esito = 'ok'",
    )
    .bind(improvement_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile rimuovere il miglioramento dal backlog")?;
    tx.commit()
        .await
        .context("Impossibile salvare l'archiviazione")?;
    Ok(())
}

async fn delete_discarded_improvement(pool: &SqlitePool, improvement_id: i64) -> Result<()> {
    if !is_primary_admin(pool).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    let paths: Vec<String> = sqlx::query_scalar(
        "SELECT a.percorso_file FROM miglioramento_allegati a JOIN miglioramenti m ON m.id = a.miglioramento_id \
         WHERE m.id = ? AND m.stato = 'scartato'",
    )
    .bind(improvement_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli allegati da eliminare")?;
    let affected = sqlx::query("DELETE FROM miglioramenti WHERE id = ? AND stato = 'scartato'")
        .bind(improvement_id)
        .execute(pool)
        .await
        .context("Impossibile eliminare il miglioramento scartato")?
        .rows_affected();
    if affected != 1 {
        bail!("Il miglioramento non è scartato o non esiste");
    }
    cleanup_files(paths).await;
    cleanup_improvement_directory(improvement_id).await;
    Ok(())
}

async fn delete_all_discarded_improvements(pool: &SqlitePool) -> Result<u64> {
    if !is_primary_admin(pool).await? {
        bail!("Operazione riservata all'amministratore principale");
    }
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM miglioramenti WHERE stato = 'scartato'")
        .fetch_all(pool)
        .await
        .context("Impossibile leggere i miglioramenti scartati")?;
    let paths: Vec<String> = sqlx::query_scalar(
        "SELECT a.percorso_file FROM miglioramento_allegati a JOIN miglioramenti m ON m.id = a.miglioramento_id WHERE m.stato = 'scartato' \
         UNION ALL \
         SELECT v.percorso_file FROM miglioramento_verifica_allegati v JOIN miglioramenti m ON m.id = v.miglioramento_id WHERE m.stato = 'scartato'",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli allegati degli scartati")?;
    let result = sqlx::query("DELETE FROM miglioramenti WHERE stato = 'scartato'")
        .execute(pool)
        .await
        .context("Impossibile eliminare tutti i miglioramenti scartati")?;
    cleanup_files(paths).await;
    for id in ids {
        cleanup_improvement_directory(id).await;
    }
    Ok(result.rows_affected())
}

async fn count_scope(pool: &SqlitePool, scope: ListScope) -> Result<i64> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let count = match scope {
        ListScope::Mine => sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti WHERE autore_utente_id = ?")
            .bind(user_id).fetch_one(pool).await,
        ListScope::All => sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti").fetch_one(pool).await,
        ListScope::Pending => sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti WHERE stato = 'da_approvare'").fetch_one(pool).await,
        ListScope::Todo => sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti WHERE stato = 'da_fare'").fetch_one(pool).await,
        ListScope::Done => sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti WHERE stato = 'fatto' AND COALESCE(verifica_esito, '') <> 'ok'").fetch_one(pool).await,
        ListScope::Discarded => sqlx::query_scalar("SELECT COUNT(*) FROM miglioramenti WHERE stato = 'scartato'").fetch_one(pool).await,
    };
    count.context("Impossibile contare i miglioramenti")
}

async fn fetch_scope(
    pool: &SqlitePool,
    scope: ListScope,
    page: i64,
) -> Result<Vec<ImprovementRecord>> {
    let actor = identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let offset = page * LIST_PAGE_SIZE;
    let select = "SELECT m.id, m.autore_utente_id, u.nome_visualizzato AS autore_nome, \
            m.descrizione, m.modulo, m.contesto, m.stato, m.letto_admin_il, m.verifica_esito, m.verifica_note, m.verificato_il, \
            strftime('%d/%m/%Y %H:%M', m.creato_il, 'localtime') AS creato_il, \
            (SELECT COUNT(*) FROM miglioramento_allegati a WHERE a.miglioramento_id = m.id) AS allegati, \
            (SELECT COUNT(*) FROM miglioramento_verifica_allegati va WHERE va.miglioramento_id = m.id) AS prove \
        FROM miglioramenti m JOIN utenti u ON u.id = m.autore_utente_id ";
    let order = " ORDER BY m.aggiornato_il DESC, m.id DESC LIMIT ? OFFSET ?";
    let rows = match scope {
        ListScope::Mine => {
            let sql = format!("{select} WHERE m.autore_utente_id = ?{order}");
            sqlx::query_as(&sql)
                .bind(user_id)
                .bind(LIST_PAGE_SIZE)
                .bind(offset)
                .fetch_all(pool)
                .await
        }
        ListScope::All => {
            let sql = format!("{select}{order}");
            sqlx::query_as(&sql)
                .bind(LIST_PAGE_SIZE)
                .bind(offset)
                .fetch_all(pool)
                .await
        }
        ListScope::Pending => {
            fetch_state_page(pool, select, "m.stato = 'da_approvare'", order, offset).await
        }
        ListScope::Todo => {
            fetch_state_page(pool, select, "m.stato = 'da_fare'", order, offset).await
        }
        ListScope::Done => {
            fetch_state_page(
                pool,
                select,
                "m.stato = 'fatto' AND COALESCE(m.verifica_esito, '') <> 'ok'",
                order,
                offset,
            )
            .await
        }
        ListScope::Discarded => {
            fetch_state_page(pool, select, "m.stato = 'scartato'", order, offset).await
        }
    };
    rows.context("Impossibile leggere i miglioramenti")
}

async fn fetch_state_page(
    pool: &SqlitePool,
    select: &str,
    condition: &str,
    order: &str,
    offset: i64,
) -> std::result::Result<Vec<ImprovementRecord>, sqlx::Error> {
    let sql = format!("{select} WHERE {condition}{order}");
    sqlx::query_as(&sql)
        .bind(LIST_PAGE_SIZE)
        .bind(offset)
        .fetch_all(pool)
        .await
}

async fn is_primary_admin(pool: &SqlitePool) -> Result<bool> {
    identity::is_primary_admin(pool, &identity::current_actor()).await
}

fn menu_keyboard(admin: bool) -> InlineKeyboardMarkup {
    menu_keyboard_con_conteggi(admin, None)
}

/// Menu' Miglioramenti con il conteggio sulle etichette (C7).
///
/// Senza conteggi (quando non si riescono a leggere) le etichette restano
/// quelle di prima: una sezione raggiungibile vale piu' di un numero esatto.
/// Il nome di ogni voce viene da `ListScope::title`, cosi' il pulsante e il
/// titolo della schermata a cui porta non possono divergere (C10).
fn menu_keyboard_con_conteggi(
    admin: bool,
    conteggi: Option<ConteggiMiglioramenti>,
) -> InlineKeyboardMarkup {
    let voce = |scope: ListScope| {
        let etichetta = match conteggi {
            Some(conteggi) => liste::etichetta_con_conteggio(scope.title(), conteggi.per(scope)),
            None => scope.title().to_string(),
        };
        InlineKeyboardButton::callback(etichetta, format!("improve:list:{}:0", scope.token()))
    };

    let mut rows = vec![
        vec![InlineKeyboardButton::callback(
            "➕ Nuovo miglioramento".to_string(),
            "improve:new".to_string(),
        )],
        vec![voce(ListScope::Mine)],
    ];
    if admin {
        rows.push(vec![voce(ListScope::Pending), voce(ListScope::Todo)]);
        rows.push(vec![voce(ListScope::Done)]);
        rows.push(vec![voce(ListScope::All)]);
        rows.push(vec![
            InlineKeyboardButton::callback(
                "📦 Esporta miglioramenti".to_string(),
                "improve:export".to_string(),
            ),
            InlineKeyboardButton::callback(
                "📦 Esporta progetto".to_string(),
                "improve:export:project".to_string(),
            ),
        ]);
        rows.push(vec![
            voce(ListScope::Discarded),
            InlineKeyboardButton::callback(
                "📦 Archivio".to_string(),
                "improve:archive:list".to_string(),
            ),
        ]);
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "🏠 Menù principale".to_string(),
        "menu:main".to_string(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

fn export_scope_keyboard(selection: ExportSelection) -> InlineKeyboardMarkup {
    fn mark(selected: bool, label: &str) -> String {
        format!("{} {label}", if selected { "✅" } else { "☐" })
    }
    let mask = selection.mask();
    let mut rows = vec![
        vec![
            InlineKeyboardButton::callback(
                mark(selection.contains(ExportSelection::PENDING), "Da approvare"),
                format!("improve:export:toggle:{mask}:{}", ExportSelection::PENDING),
            ),
            InlineKeyboardButton::callback(
                mark(selection.contains(ExportSelection::TODO), "Da fare"),
                format!("improve:export:toggle:{mask}:{}", ExportSelection::TODO),
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                mark(selection.contains(ExportSelection::DONE), "Fatte"),
                format!("improve:export:toggle:{mask}:{}", ExportSelection::DONE),
            ),
            InlineKeyboardButton::callback(
                mark(selection.contains(ExportSelection::ARCHIVED), "Archiviate"),
                format!("improve:export:toggle:{mask}:{}", ExportSelection::ARCHIVED),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            mark(selection == ExportSelection::all(), "Tutti"),
            format!("improve:export:all:{mask}"),
        )],
    ];
    if !selection.is_empty() {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("📦 Esporta · {}", selection.label()),
            format!("improve:export:run:{mask}"),
        )]);
    }
    rows.push(vec![
        InlineKeyboardButton::callback("⬅️ Miglioramenti".to_string(), "improve:menu".to_string()),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn export_ready_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✅ Ho scaricato il file".to_string(),
            "improve:export:downloaded".to_string(),
        )],
        vec![
            InlineKeyboardButton::callback(
                "⬅️ Miglioramenti".to_string(),
                "improve:menu".to_string(),
            ),
            InlineKeyboardButton::callback(
                "🏠 Menù principale".to_string(),
                "menu:main".to_string(),
            ),
        ],
    ])
}

fn flow_cancel_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("❌ Annulla".to_string(), "improve:cancel".to_string()),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}

fn edit_description_keyboard(current_description: &str) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    let chunks = copy_text_chunks(current_description, 240);
    if chunks.len() == 1 {
        rows.push(vec![InlineKeyboardButton::copy_text_button(
            "📋 Copia testo originale".to_string(),
            CopyTextButton {
                text: chunks[0].clone(),
            },
        )]);
    } else {
        let total = chunks.len();
        for (index, chunk) in chunks.into_iter().enumerate() {
            rows.push(vec![InlineKeyboardButton::copy_text_button(
                format!("📋 Copia parte {}/{}", index + 1, total),
                CopyTextButton { text: chunk },
            )]);
        }
    }
    rows.extend(description_keyboard().inline_keyboard);
    InlineKeyboardMarkup::new(rows)
}

fn copy_text_chunks(value: &str, max_chars: usize) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    let chars = value.chars().collect::<Vec<_>>();
    chars
        .chunks(max_chars.max(1))
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

fn description_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✅ Fine descrizione".to_string(),
            "improve:description:finish".to_string(),
        )],
        vec![
            InlineKeyboardButton::callback("❌ Annulla".to_string(), "improve:cancel".to_string()),
            InlineKeyboardButton::callback(
                "🏠 Menù principale".to_string(),
                "menu:main".to_string(),
            ),
        ],
    ])
}

fn optional_photo_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✅ Salva senza foto".to_string(),
            "improve:save:no_photo".to_string(),
        )],
        vec![
            InlineKeyboardButton::callback("❌ Annulla".to_string(), "improve:cancel".to_string()),
            InlineKeyboardButton::callback(
                "🏠 Menù principale".to_string(),
                "menu:main".to_string(),
            ),
        ],
    ])
}

fn after_save_keyboard(improvement_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "💡 Apri miglioramento".to_string(),
            format!("improve:view:{improvement_id}"),
        )],
        vec![
            InlineKeyboardButton::callback(
                "⬅️ Miglioramenti".to_string(),
                "improve:menu".to_string(),
            ),
            InlineKeyboardButton::callback(
                "🏠 Menù principale".to_string(),
                "menu:main".to_string(),
            ),
        ],
    ])
}

fn detail_keyboard(
    item: &ImprovementRecord,
    admin: bool,
    owner: bool,
    return_to: Option<(ListScope, i64)>,
    description_long: bool,
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if description_long {
        rows.push(vec![InlineKeyboardButton::callback(
            "📖 Leggi descrizione completa".to_string(),
            format!("improve:description:full:{}:0", item.id),
        )]);
    }
    if owner {
        rows.push(vec![
            InlineKeyboardButton::callback(
                "✏️ Modifica testo".to_string(),
                return_to
                    .map(|(scope, page)| {
                        format!("improve:edit:{}:{}:{page}", item.id, scope.token())
                    })
                    .unwrap_or_else(|| format!("improve:edit:{}", item.id)),
            ),
            InlineKeyboardButton::callback(
                "📷 Aggiungi screenshot".to_string(),
                format!("improve:add_photo:{}", item.id),
            ),
        ]);
        if item.allegati > 0 {
            rows.push(vec![InlineKeyboardButton::callback(
                format!("🖼️ Vedi/gestisci screenshot ({})", item.allegati),
                format!("improve:photos:{}", item.id),
            )]);
        }
        rows.push(vec![InlineKeyboardButton::callback(
            "🗑️ Elimina mio suggerimento".to_string(),
            format!("improve:delete:ask:{}", item.id),
        )]);
    } else if item.allegati > 0 {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("🖼️ Vedi screenshot ({})", item.allegati),
            format!("improve:photos:{}", item.id),
        )]);
    }

    if admin {
        match item.stato.as_str() {
            "da_approvare" => rows.push(vec![
                InlineKeyboardButton::callback(
                    "✅ Approva".to_string(),
                    format!("improve:status:{}:da_fare", item.id),
                ),
                InlineKeyboardButton::callback(
                    "❌ Scarta".to_string(),
                    format!("improve:status:{}:scartato", item.id),
                ),
            ]),
            "da_fare" => rows.push(vec![
                InlineKeyboardButton::callback(
                    "✅ Segna Fatto".to_string(),
                    format!("improve:status:{}:fatto", item.id),
                ),
                InlineKeyboardButton::callback(
                    "❌ Scarta".to_string(),
                    format!("improve:status:{}:scartato", item.id),
                ),
            ]),
            "fatto" if item.verifica_esito.as_deref() == Some("ok") => {
                if item.prove > 0 {
                    rows.push(vec![InlineKeyboardButton::callback(
                        format!("📎 Prove collaudo ({})", item.prove),
                        format!("improve:verify:attachments:{}", item.id),
                    )]);
                }
                rows.push(vec![InlineKeyboardButton::callback(
                    "📦 Archivia miglioramento".to_string(),
                    format!("improve:archive:{}", item.id),
                )]);
            }
            "fatto" => {
                rows.push(vec![InlineKeyboardButton::callback(
                    "🧪 Verifica miglioramento".to_string(),
                    format!("improve:verify:{}", item.id),
                )]);
                if item.prove > 0 {
                    rows.push(vec![InlineKeyboardButton::callback(
                        format!("📎 Prove collaudo ({})", item.prove),
                        format!("improve:verify:attachments:{}", item.id),
                    )]);
                }
            }
            "scartato" => rows.push(vec![InlineKeyboardButton::callback(
                "🗑️ Elimina scartato".to_string(),
                format!("improve:delete_discarded:{}", item.id),
            )]),
            _ => {}
        }
    }
    let back_callback = return_to
        .map(|(scope, page)| format!("improve:list:{}:{page}", scope.token()))
        .unwrap_or_else(|| "improve:menu".to_string());
    rows.push(vec![
        InlineKeyboardButton::callback("⬅️ Indietro".to_string(), back_callback),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn parse_list_callback(data: &str) -> Option<(ListScope, i64)> {
    let rest = data.strip_prefix("improve:list:")?;
    let mut parts = rest.split(':');
    let scope = match parts.next()? {
        "mine" => ListScope::Mine,
        "all" => ListScope::All,
        "pending" => ListScope::Pending,
        "todo" => ListScope::Todo,
        "done" => ListScope::Done,
        "discarded" => ListScope::Discarded,
        _ => return None,
    };
    let page = parse_nonnegative_i64(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((scope, page))
}

fn parse_scope_token(value: &str) -> Option<ListScope> {
    match value {
        "mine" => Some(ListScope::Mine),
        "all" => Some(ListScope::All),
        "pending" => Some(ListScope::Pending),
        "todo" => Some(ListScope::Todo),
        "done" => Some(ListScope::Done),
        "discarded" => Some(ListScope::Discarded),
        _ => None,
    }
}

fn parse_view_callback(data: &str) -> Option<(i64, Option<(ListScope, i64)>)> {
    let rest = data.strip_prefix("improve:view:")?;
    let mut parts = rest.split(':');
    let id = parts.next()?.parse::<i64>().ok()?;
    let Some(scope_raw) = parts.next() else {
        return Some((id, None));
    };
    let scope = parse_scope_token(scope_raw)?;
    let page = parse_nonnegative_i64(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((id, Some((scope, page))))
}

fn parse_edit_callback(data: &str) -> Option<(i64, Option<(ListScope, i64)>)> {
    let rest = data.strip_prefix("improve:edit:")?;
    let mut parts = rest.split(':');
    let id = parts.next()?.parse::<i64>().ok()?;
    let Some(scope_raw) = parts.next() else {
        return Some((id, None));
    };
    let scope = parse_scope_token(scope_raw)?;
    let page = parse_nonnegative_i64(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((id, Some((scope, page))))
}

fn parse_full_description_callback(data: &str) -> Option<(i64, i64)> {
    let rest = data.strip_prefix("improve:description:full:")?;
    let mut parts = rest.split(':');
    let id = parts.next()?.parse::<i64>().ok()?;
    let page = parse_nonnegative_i64(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((id, page))
}

fn split_text_pages(value: &str, max_chars: usize) -> Vec<String> {
    if value.is_empty() {
        return vec![String::new()];
    }
    let mut pages = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if current.chars().count() >= max_chars {
            pages.push(current);
            current = String::new();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        pages.push(current);
    }
    pages
}

fn total_pages(total: i64, page_size: i64) -> i64 {
    if total <= 0 {
        1
    } else {
        (total + page_size - 1) / page_size
    }
}

fn normalize_page(page: i64, pages: i64) -> i64 {
    page.max(0).min((pages - 1).max(0))
}

fn attachment_summary(original: i64, verification: i64) -> String {
    match (original, verification) {
        (0, 0) => "Nessun allegato".to_string(),
        (photos, 0) => format!("📷 {photos}"),
        (0, proofs) => format!("🧪📎 {proofs}"),
        (photos, proofs) => format!("📷 {photos} · 🧪📎 {proofs}"),
    }
}

fn display_status_icon(item: &ImprovementRecord) -> &'static str {
    if item.stato == "fatto" && item.verifica_esito.as_deref() == Some("ok") {
        "🧪"
    } else {
        status_icon(&item.stato)
    }
}

fn display_status_label(item: &ImprovementRecord) -> &'static str {
    if item.stato == "fatto" && item.verifica_esito.as_deref() == Some("ok") {
        "Verificato · da archiviare"
    } else {
        status_label(&item.stato)
    }
}

fn status_icon(value: &str) -> &'static str {
    match value {
        "da_approvare" => "🟡",
        "da_fare" => "🟢",
        "fatto" => "✅",
        "scartato" => "❌",
        _ => "•",
    }
}

fn status_label(value: &str) -> &'static str {
    match value {
        "da_approvare" => "Da approvare",
        "da_fare" => "Da fare",
        "fatto" => "Fatto · da verificare",
        "scartato" => "Scartato",
        _ => "Sconosciuto",
    }
}

fn validate_description(value: &str) -> Result<&str> {
    let description = value.trim();
    if description.len() < 3 {
        bail!("La descrizione è troppo breve. Scrivi almeno qualche parola");
    }
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        bail!(
            "La descrizione supera il limite tecnico di sicurezza di {MAX_DESCRIPTION_CHARS} caratteri"
        );
    }
    Ok(description)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn parse_id(data: &str, prefix: &str) -> Option<i64> {
    data.strip_prefix(prefix)?.parse().ok()
}

fn parse_nonnegative_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|value| *value >= 0)
}

fn first_command(text: &str) -> Option<&str> {
    let token = text.split_whitespace().next()?;
    if !token.starts_with('/') {
        return None;
    }
    Some(token.split('@').next().unwrap_or(token))
}

fn safe_extension(path: &str, fallback: &str) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or(fallback)
        .to_ascii_lowercase();
    if !extension.is_empty()
        && extension.len() <= 5
        && extension.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        extension
    } else {
        fallback.to_string()
    }
}

async fn cleanup_files(paths: Vec<String>) {
    for path in paths {
        if let Err(error) = tokio::fs::remove_file(&path).await {
            if Path::new(&path).exists() {
                tracing::warn!(?error, %path, "File miglioramento non eliminato");
            }
        }
    }
}

async fn cleanup_improvement_directory(improvement_id: i64) {
    let directory = PathBuf::from(MEDIA_ROOT).join(improvement_id.to_string());
    if directory.exists() {
        if let Err(error) = tokio::fs::remove_dir_all(&directory).await {
            tracing::warn!(?error, ?directory, "Cartella miglioramento non eliminata");
        }
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
            .expect("foreign key");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration");
        pool
    }

    #[tokio::test]
    async fn struttura_miglioramenti_supporta_piu_allegati() {
        let pool = test_pool().await;
        let user_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Tester')")
            .execute(&pool)
            .await
            .expect("utente")
            .last_insert_rowid();
        let improvement_id = sqlx::query(
            "INSERT INTO miglioramenti (autore_utente_id, descrizione) VALUES (?, 'Migliorare pulsante')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("miglioramento")
        .last_insert_rowid();
        for path in ["data/media/a.jpg", "data/media/b.jpg"] {
            sqlx::query(
                "INSERT INTO miglioramento_allegati (miglioramento_id, percorso_file) VALUES (?, ?)",
            )
            .bind(improvement_id)
            .bind(path)
            .execute(&pool)
            .await
            .expect("allegato");
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM miglioramento_allegati WHERE miglioramento_id = ?",
        )
        .bind(improvement_id)
        .fetch_one(&pool)
        .await
        .expect("conteggio");
        assert_eq!(count, 2);
    }

    #[test]
    fn ritorno_dettaglio_conserva_lista_e_pagina() {
        assert_eq!(
            parse_view_callback("improve:view:22:all:2"),
            Some((22, Some((ListScope::All, 2))))
        );
        assert_eq!(parse_view_callback("improve:view:22"), Some((22, None)));
    }

    #[test]
    fn modifica_testo_conserva_lista_e_pagina_di_provenienza() {
        let parsed = parse_edit_callback("improve:edit:29:done:2");
        assert_eq!(parsed, Some((29, Some((ListScope::Done, 2)))));
        assert_eq!(parse_edit_callback("improve:edit:29"), Some((29, None)));
    }

    #[test]
    fn descrizione_lunga_viene_paginata_senza_limite_normale_di_duemila() {
        let value = "a".repeat(7_100);
        assert!(validate_description(&value).is_ok());
        let pages = split_text_pages(&value, DESCRIPTION_PAGE_CHARS);
        assert_eq!(pages.len(), 3);
        assert!(pages
            .iter()
            .all(|page| page.chars().count() <= DESCRIPTION_PAGE_CHARS));
        assert_eq!(pages.concat(), value);
    }

    #[test]
    fn percorso_export_accetta_solo_zip_nella_directory_dedicata() {
        let valid =
            export_root_path().join("gestionale-casa_handoff_miglioramenti_20260827_120000.zip");
        assert!(is_export_zip_path(&valid));
        assert!(!is_export_zip_path(&PathBuf::from(
            "/tmp/gestionale-casa_handoff_miglioramenti_20260827_120000.zip"
        )));
        assert!(!is_export_zip_path(&export_root_path().join("altro.zip")));
    }

    #[test]
    fn percorso_export_progetto_accetta_solo_zip_nella_directory_dedicata() {
        let project =
            project_export_root_path().join("gestionale-casa_handoff_progetto_20260829_010000.zip");
        assert!(is_export_zip_path(&project));
        assert!(!is_export_zip_path(&project_export_root_path().join(
            "gestionale-casa_handoff_miglioramenti_20260829_010000.zip"
        )));
        assert!(!is_export_zip_path(&PathBuf::from(
            "/tmp/gestionale-casa_handoff_progetto_20260829_010000.zip"
        )));
    }

    #[test]
    fn copia_testo_lungo_viene_divisa_entra_limite_telegram() {
        let value = "à".repeat(700);
        let chunks = copy_text_chunks(&value, 240);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 240));
        assert_eq!(chunks.concat(), value);
    }

    #[test]
    fn filtro_export_miglioramenti_supporta_selezione_multipla() {
        let selection = ExportSelection::empty()
            .toggle(ExportSelection::TODO)
            .unwrap()
            .toggle(ExportSelection::DONE)
            .unwrap();
        assert_eq!(selection.scope_arg(), "todo,done");
        assert_eq!(selection.label(), "Da fare + Fatte");
        assert!(selection.contains(ExportSelection::TODO));
        assert!(selection.contains(ExportSelection::DONE));
        assert!(!selection.contains(ExportSelection::ARCHIVED));
        assert_eq!(
            ExportSelection::all().scope_arg(),
            "pending,todo,done,archived"
        );
        assert_eq!(ExportSelection::all().label(), "Tutti");
        assert!(ExportSelection::from_mask(16).is_none());
    }

    #[test]
    fn dimensione_export_viene_formattata_in_modo_leggibile() {
        assert_eq!(human_file_size(512), "512 B");
        assert_eq!(human_file_size(2048), "2.0 KiB");
        assert_eq!(human_file_size(2 * 1024 * 1024), "2.00 MiB");
    }

    #[test]
    fn stati_paginazione_e_troncamento_sono_stabili() {
        assert_eq!(status_label("da_approvare"), "Da approvare");
        assert_eq!(status_label("da_fare"), "Da fare");
        assert_eq!(status_label("fatto"), "Fatto · da verificare");
        assert_eq!(total_pages(13, 5), 3);
        assert_eq!(total_pages(0, 5), 1);
        assert_eq!(truncate("abcdef", 3), "abc…");
        assert_eq!(truncate("abc", 3), "abc");
    }
}
