//! Wrapper Telegram che centralizza:
//! - pulsante contestuale `💡 Migliora`;
//! - contesto/azioni recenti;
//! - una sola schermata UI attiva per chat;
//! - invalidazione dei callback di schermate vecchie;
//! - pulizia dei media temporanei alla successiva interazione.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    future::{Future, IntoFuture},
    ops::Deref,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use sqlx::SqlitePool;
use teloxide::{
    payloads::{SendMessage, SendPhoto, SendVideo},
    prelude::Requester,
    requests::{HasPayload, Output, Payload, Request},
    types::{
        ChatId, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, InputFile,
        Message, MessageId, Recipient, ReplyMarkup,
    },
    Bot as TelegramBot,
};

const MAX_RECENT_ACTIONS: usize = 6;
const MAX_CONTEXTS_PER_CHAT: usize = 120;
const MAX_SCREEN_TITLE_CHARS: usize = 180;
const MAX_TYPED_TEXT_CHARS: usize = 100;

#[derive(Debug, Clone)]
pub(crate) struct ImproveContextSnapshot {
    pub chat_id: i64,
    pub section: String,
    pub screen: String,
    pub recent_actions: Vec<String>,
    pub screen_text: String,
    pub keyboard: Option<InlineKeyboardMarkup>,
}

impl ImproveContextSnapshot {
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!("Sezione: {}", self.section),
            format!("Schermata: {}", self.screen),
        ];
        if !self.recent_actions.is_empty() {
            lines.push("Azioni recenti (dalla più recente):".to_string());
            for action in &self.recent_actions {
                lines.push(format!("• {action}"));
            }
        }
        lines.join("\n")
    }
}

#[derive(Debug, Default)]
struct ChatUiState {
    active_ui: Option<MessageId>,
    transient_media: Vec<MessageId>,
    claimed_messages: HashSet<MessageId>,
    callback_labels: HashMap<String, String>,
    current_section: Option<String>,
}

#[derive(Debug, Default)]
struct ContextState {
    recent_actions: HashMap<i64, VecDeque<String>>,
    snapshots: HashMap<u64, ImproveContextSnapshot>,
    snapshot_order: HashMap<i64, VecDeque<u64>>,
    ui: HashMap<i64, ChatUiState>,
}

#[derive(Clone, Default)]
pub(crate) struct ImproveContextStore {
    inner: Arc<Mutex<ContextState>>,
    next_token: Arc<AtomicU64>,
}

impl ImproveContextStore {
    pub fn record_text(&self, chat_id: i64, text: Option<&str>) {
        let Some(text) = text.map(str::trim).filter(|value| !value.is_empty()) else {
            return;
        };
        let section = section_for_command(text);
        if let Some(section) = section.as_deref() {
            self.set_current_section(chat_id, section);
        }
        let action = if text.starts_with('/') {
            let label = humanize_command(text);
            match section {
                Some(section) => format!("{label} → {section}"),
                None => label,
            }
        } else {
            format!("hai scritto «{}»", truncate(text, MAX_TYPED_TEXT_CHARS))
        };
        self.push_action(chat_id, action);
    }

    pub fn record_callback(&self, chat_id: i64, data: &str) {
        if data.starts_with("improve:context:") || data == "improve:noop" || data.ends_with(":noop")
        {
            return;
        }
        let (label, current_section) = {
            let state = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let ui = state.ui.get(&chat_id);
            (
                ui.and_then(|ui| ui.callback_labels.get(data)).cloned(),
                ui.and_then(|ui| ui.current_section.clone()),
            )
        };
        let target_section = section_for_callback(data).or(current_section);
        if let Some(section) = target_section.as_deref() {
            self.set_current_section(chat_id, section);
        }
        let base = label
            .map(|label| format!("hai premuto «{label}»"))
            .unwrap_or_else(|| humanize_callback(data));
        let action = match target_section {
            Some(section) => format!("{base} → {section}"),
            None => base,
        };
        self.push_action(chat_id, action);
    }

    fn set_current_section(&self, chat_id: i64, section: &str) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.ui.entry(chat_id).or_default().current_section = Some(section.to_string());
    }

    fn push_action(&self, chat_id: i64, action: String) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let recent = state.recent_actions.entry(chat_id).or_default();
        if recent.back().map(String::as_str) == Some(action.as_str()) {
            return;
        }
        recent.push_back(action);
        while recent.len() > MAX_RECENT_ACTIONS {
            recent.pop_front();
        }
    }

    fn create_snapshot(
        &self,
        chat_id: i64,
        text: &str,
        keyboard: Option<InlineKeyboardMarkup>,
    ) -> u64 {
        let token = self.next_token.fetch_add(1, Ordering::Relaxed) + 1;
        let screen = screen_title(text);
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current_section = state
            .ui
            .get(&chat_id)
            .and_then(|ui| ui.current_section.as_deref());
        let section = infer_section(text, &screen, current_section);
        state.ui.entry(chat_id).or_default().current_section = Some(section.clone());
        let recent_actions = state
            .recent_actions
            .get(&chat_id)
            .map(|items| items.iter().rev().cloned().collect())
            .unwrap_or_default();
        state.snapshots.insert(
            token,
            ImproveContextSnapshot {
                chat_id,
                section,
                screen,
                recent_actions,
                screen_text: text.to_string(),
                keyboard,
            },
        );
        let expired_tokens = {
            let order = state.snapshot_order.entry(chat_id).or_default();
            order.push_back(token);

            let mut expired_tokens = Vec::new();
            while order.len() > MAX_CONTEXTS_PER_CHAT {
                if let Some(expired) = order.pop_front() {
                    expired_tokens.push(expired);
                }
            }
            expired_tokens
        };
        for expired in expired_tokens {
            state.snapshots.remove(&expired);
        }
        token
    }

    pub fn get_snapshot(&self, chat_id: i64, token: u64) -> Option<ImproveContextSnapshot> {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .snapshots
            .get(&token)
            .filter(|snapshot| snapshot.chat_id == chat_id)
            .cloned()
    }

    fn remember_callback_labels(&self, chat_id: i64, markup: &InlineKeyboardMarkup) {
        let mut labels = HashMap::new();
        for row in &markup.inline_keyboard {
            for button in row {
                if let InlineKeyboardButtonKind::CallbackData(data) = &button.kind {
                    labels.insert(data.clone(), button.text.clone());
                }
            }
        }
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.ui.entry(chat_id).or_default().callback_labels = labels;
    }

    fn restore_ui_message(&self, chat_id: i64, message_id: MessageId) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.ui.entry(chat_id).or_default().active_ui = Some(message_id);
    }

    fn register_ui_message(&self, chat_id: i64, message_id: MessageId) -> Option<MessageId> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ui = state.ui.entry(chat_id).or_default();
        ui.claimed_messages.clear();
        let previous = ui.active_ui.replace(message_id);
        previous.filter(|old| *old != message_id)
    }

    fn register_transient_media(&self, chat_id: i64, message_id: MessageId) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ui = state.ui.entry(chat_id).or_default();
        if !ui.transient_media.contains(&message_id) {
            ui.transient_media.push(message_id);
        }
    }

    fn take_transient_media(&self, chat_id: i64) -> Vec<MessageId> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .ui
            .entry(chat_id)
            .or_default()
            .transient_media
            .drain(..)
            .collect()
    }

    fn claim_callback(&self, chat_id: i64, message_id: MessageId, data: &str) -> bool {
        if data == "improve:noop" || data.ends_with(":noop") {
            return self.is_current_message(chat_id, message_id);
        }
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(ui) = state.ui.get_mut(&chat_id) else {
            return false;
        };
        let current = ui.active_ui == Some(message_id) || ui.transient_media.contains(&message_id);
        if !current || ui.claimed_messages.contains(&message_id) {
            return false;
        }
        ui.claimed_messages.insert(message_id);
        true
    }

    fn is_current_message(&self, chat_id: i64, message_id: MessageId) -> bool {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.ui.get(&chat_id).is_some_and(|ui| {
            ui.active_ui == Some(message_id) || ui.transient_media.contains(&message_id)
        })
    }
}

#[derive(Clone)]
pub(crate) struct ContextBot {
    inner: TelegramBot,
    contexts: ImproveContextStore,
    pool: SqlitePool,
}

impl ContextBot {
    pub fn new(inner: TelegramBot, contexts: ImproveContextStore, pool: SqlitePool) -> Self {
        Self {
            inner,
            contexts,
            pool,
        }
    }

    pub async fn restore_persisted_ui(&self) {
        match sqlx::query_as::<_, (i64, i64)>(
            "SELECT chat_id, active_message_id FROM telegram_ui_state WHERE active_message_id IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => {
                for (chat_id, message_id) in rows {
                    if let Ok(message_id) = i32::try_from(message_id) {
                        self.contexts
                            .restore_ui_message(chat_id, MessageId(message_id));
                    }
                }
            }
            Err(error) => {
                tracing::warn!(?error, "Impossibile ripristinare le schermate Telegram persistenti");
            }
        }
    }

    pub fn record_text(&self, chat_id: i64, text: Option<&str>) {
        self.contexts.record_text(chat_id, text);
    }

    pub fn record_callback(&self, chat_id: i64, data: &str) {
        self.contexts.record_callback(chat_id, data);
    }

    pub fn improve_context(&self, chat_id: i64, token: u64) -> Option<ImproveContextSnapshot> {
        self.contexts.get_snapshot(chat_id, token)
    }

    pub fn claim_callback(&self, chat_id: i64, message_id: MessageId, data: &str) -> bool {
        self.contexts.claim_callback(chat_id, message_id, data)
    }

    pub async fn cleanup_transient_media(&self, chat_id: ChatId) {
        for message_id in self.contexts.take_transient_media(chat_id.0) {
            if let Err(error) = self.inner.delete_message(chat_id, message_id).await {
                tracing::debug!(
                    chat_id = chat_id.0,
                    message_id = message_id.0,
                    ?error,
                    "Media temporaneo non eliminabile"
                );
            }
        }
    }

    pub async fn delete_user_input(&self, chat_id: ChatId, message_id: MessageId) {
        if let Err(error) = self.inner.delete_message(chat_id, message_id).await {
            tracing::debug!(
                chat_id = chat_id.0,
                message_id = message_id.0,
                ?error,
                "Input utente non eliminabile"
            );
        }
    }

    pub fn send_message<C, T>(
        &self,
        chat_id: C,
        text: T,
    ) -> ContextRequest<<TelegramBot as Requester>::SendMessage>
    where
        C: Into<Recipient>,
        T: Into<String>,
    {
        ContextRequest::new(
            self.inner.send_message(chat_id, text),
            self.contexts.clone(),
            self.inner.clone(),
            OutputMode::Ui,
            true,
            self.pool.clone(),
        )
    }

    pub fn send_photo<C>(
        &self,
        chat_id: C,
        photo: InputFile,
    ) -> ContextRequest<<TelegramBot as Requester>::SendPhoto>
    where
        C: Into<Recipient>,
    {
        ContextRequest::new(
            self.inner.send_photo(chat_id, photo),
            self.contexts.clone(),
            self.inner.clone(),
            OutputMode::TransientMedia,
            true,
            self.pool.clone(),
        )
    }

    pub fn send_video<C>(
        &self,
        chat_id: C,
        video: InputFile,
    ) -> ContextRequest<<TelegramBot as Requester>::SendVideo>
    where
        C: Into<Recipient>,
    {
        ContextRequest::new(
            self.inner.send_video(chat_id, video),
            self.contexts.clone(),
            self.inner.clone(),
            OutputMode::TransientMedia,
            true,
            self.pool.clone(),
        )
    }

    pub fn send_document_untracked<C>(
        &self,
        chat_id: C,
        document: InputFile,
    ) -> <TelegramBot as Requester>::SendDocument
    where
        C: Into<Recipient>,
    {
        self.inner.send_document(chat_id, document)
    }

    pub fn mark_transient_message(&self, chat_id: i64, message_id: MessageId) {
        self.contexts.register_transient_media(chat_id, message_id);
    }

    pub fn send_message_without_improve<C, T>(
        &self,
        chat_id: C,
        text: T,
    ) -> ContextRequest<<TelegramBot as Requester>::SendMessage>
    where
        C: Into<Recipient>,
        T: Into<String>,
    {
        ContextRequest::new(
            self.inner.send_message(chat_id, text),
            self.contexts.clone(),
            self.inner.clone(),
            OutputMode::Ui,
            false,
            self.pool.clone(),
        )
    }
}

impl Deref for ContextBot {
    type Target = TelegramBot;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub(crate) trait ImprovePayload {
    fn chat_id(&self) -> &Recipient;
    fn context_text(&self) -> String;
    fn reply_markup_mut(&mut self) -> &mut Option<ReplyMarkup>;
}

impl ImprovePayload for SendMessage {
    fn chat_id(&self) -> &Recipient {
        &self.chat_id
    }

    fn context_text(&self) -> String {
        self.text.clone()
    }

    fn reply_markup_mut(&mut self) -> &mut Option<ReplyMarkup> {
        &mut self.reply_markup
    }
}

impl ImprovePayload for SendPhoto {
    fn chat_id(&self) -> &Recipient {
        &self.chat_id
    }

    fn context_text(&self) -> String {
        self.caption
            .clone()
            .unwrap_or_else(|| "📷 Foto del gestionale".to_string())
    }

    fn reply_markup_mut(&mut self) -> &mut Option<ReplyMarkup> {
        &mut self.reply_markup
    }
}

impl ImprovePayload for SendVideo {
    fn chat_id(&self) -> &Recipient {
        &self.chat_id
    }

    fn context_text(&self) -> String {
        self.caption
            .clone()
            .unwrap_or_else(|| "🎥 Video del gestionale".to_string())
    }

    fn reply_markup_mut(&mut self) -> &mut Option<ReplyMarkup> {
        &mut self.reply_markup
    }
}

#[derive(Debug, Clone, Copy)]
enum OutputMode {
    Ui,
    TransientMedia,
}

#[derive(Clone)]
pub(crate) struct ContextRequest<R> {
    request: R,
    contexts: ImproveContextStore,
    bot: TelegramBot,
    mode: OutputMode,
    include_improve: bool,
    pool: SqlitePool,
}

impl<R> ContextRequest<R> {
    fn new(
        request: R,
        contexts: ImproveContextStore,
        bot: TelegramBot,
        mode: OutputMode,
        include_improve: bool,
        pool: SqlitePool,
    ) -> Self {
        Self {
            request,
            contexts,
            bot,
            mode,
            include_improve,
            pool,
        }
    }
}

impl<R> ContextRequest<R>
where
    R: Request,
    R::Payload: ImprovePayload,
{
    fn add_context_button(&mut self) {
        let payload = self.request.payload_mut();
        let Recipient::Id(chat_id) = payload.chat_id().clone() else {
            return;
        };

        let context_text = payload.context_text();
        let original_keyboard = match payload.reply_markup_mut().as_ref() {
            Some(ReplyMarkup::InlineKeyboard(keyboard)) => Some(keyboard.clone()),
            _ => None,
        };
        let token = self
            .contexts
            .create_snapshot(chat_id.0, &context_text, original_keyboard);
        let improve = InlineKeyboardButton::callback(
            "💡 Migliora".to_string(),
            format!("improve:context:{token}"),
        );

        let markup = payload.reply_markup_mut();
        match markup.take() {
            None if self.include_improve => {
                let keyboard = InlineKeyboardMarkup::new(vec![vec![improve]]);
                self.contexts.remember_callback_labels(chat_id.0, &keyboard);
                *markup = Some(keyboard.into());
            }
            None => {}
            Some(ReplyMarkup::InlineKeyboard(mut keyboard)) => {
                if self.include_improve {
                    if let Some(row) = keyboard.inline_keyboard.iter_mut().rev().find(|row| {
                        row.iter().any(|button| {
                            matches!(
                                &button.kind,
                                InlineKeyboardButtonKind::CallbackData(data) if data == "menu:main"
                            )
                        })
                    }) {
                        if row.len() < 3 {
                            row.push(improve);
                        } else {
                            keyboard.inline_keyboard.push(vec![improve]);
                        }
                    } else {
                        keyboard.inline_keyboard.push(vec![improve]);
                    }
                }
                self.contexts.remember_callback_labels(chat_id.0, &keyboard);
                *markup = Some(ReplyMarkup::InlineKeyboard(keyboard));
            }
            Some(other) => {
                *markup = Some(other);
            }
        }
    }

    fn chat_id(&self) -> Option<ChatId> {
        match self.request.payload_ref().chat_id() {
            Recipient::Id(chat_id) => Some(*chat_id),
            Recipient::ChannelUsername(_) => None,
        }
    }
}

impl<R> Request for ContextRequest<R>
where
    R: Request + Clone + Send + 'static,
    R::Payload: ImprovePayload + Payload<Output = Message> + Send + 'static,
    R::Err: 'static,
    R::Send: 'static,
{
    type Err = R::Err;
    type Send = Pin<Box<dyn Future<Output = Result<Message, Self::Err>> + Send>>;
    type SendRef = Pin<Box<dyn Future<Output = Result<Message, Self::Err>> + Send>>;

    fn send(mut self) -> Self::Send {
        self.add_context_button();
        let chat_id = self.chat_id();
        let request = self.request;
        let contexts = self.contexts;
        let bot = self.bot;
        let mode = self.mode;
        let pool = self.pool;

        Box::pin(async move {
            let message = request.send().await?;
            if let Some(chat_id) = chat_id {
                match mode {
                    OutputMode::Ui => {
                        if let Some(previous) = contexts.register_ui_message(chat_id.0, message.id)
                        {
                            if let Err(error) = bot.delete_message(chat_id, previous).await {
                                tracing::debug!(
                                    chat_id = chat_id.0,
                                    message_id = previous.0,
                                    ?error,
                                    "Schermata UI precedente non eliminabile"
                                );
                            }
                        }
                        if let Err(error) = persist_active_ui(&pool, chat_id.0, message.id).await {
                            tracing::warn!(
                                chat_id = chat_id.0,
                                message_id = message.id.0,
                                ?error,
                                "Schermata UI Telegram non persistita"
                            );
                        }
                    }
                    OutputMode::TransientMedia => {
                        contexts.register_transient_media(chat_id.0, message.id);
                    }
                }
            }
            Ok(message)
        })
    }

    fn send_ref(&self) -> Self::SendRef {
        self.clone().send()
    }
}

impl<R> HasPayload for ContextRequest<R>
where
    R: Request,
    R::Payload: ImprovePayload,
{
    type Payload = R::Payload;

    fn payload_mut(&mut self) -> &mut Self::Payload {
        self.request.payload_mut()
    }

    fn payload_ref(&self) -> &Self::Payload {
        self.request.payload_ref()
    }
}

impl<R> IntoFuture for ContextRequest<R>
where
    Self: Request,
{
    type Output = Result<Output<Self>, <Self as Request>::Err>;
    type IntoFuture = <Self as Request>::Send;

    fn into_future(self) -> Self::IntoFuture {
        self.send()
    }
}

fn infer_section(text: &str, screen: &str, current_section: Option<&str>) -> String {
    let normalized = text.to_lowercase();
    let explicit = if normalized.contains("menù principale")
        || normalized.contains("menu principale")
        || normalized.contains("scegli una sezione")
    {
        Some("Menù principale")
    } else if normalized.contains("amministrazione") {
        Some("Amministrazione")
    } else if normalized.contains("migliorament") {
        Some("Miglioramenti")
    } else if normalized.contains("ricett") {
        Some("Alimentazione › Ricette")
    } else if normalized.contains("aliment") || normalized.contains("alimenti") {
        Some("Alimentazione › Alimenti")
    } else if normalized.contains("contenitor") {
        Some("Oggetti › Contenitori")
    } else if normalized.contains("storico") {
        Some("Storico")
    } else if normalized.contains("luoghi") || normalized.contains("case, stanze") {
        Some("Luoghi")
    } else if normalized.contains("oggett") {
        Some("Oggetti")
    } else if normalized.contains("profilo") || normalized.contains("spazi") {
        Some("Profilo e spazi")
    } else {
        None
    };

    explicit.or(current_section).unwrap_or(screen).to_string()
}

fn section_for_callback(data: &str) -> Option<String> {
    let section = if data == "menu:main" || data == "menu:soon" {
        "Menù principale"
    } else if data.starts_with("recipe:") {
        "Alimentazione › Ricette"
    } else if data.starts_with("food:") {
        "Alimentazione › Alimenti"
    } else if data.starts_with("improve:") {
        "Miglioramenti"
    } else if data.starts_with("history:") || data.starts_with("storico:") || data.starts_with("h:")
    {
        "Storico"
    } else if data.starts_with("container:") || data.starts_with("contenitore:") {
        "Oggetti › Contenitori"
    } else if data.starts_with("location:")
        || data.starts_with("luogo:")
        || data.starts_with("loc:")
    {
        "Luoghi"
    } else if data.starts_with("object:")
        || data.starts_with("oggetto:")
        || data.starts_with("oggetti:")
    {
        "Oggetti"
    } else if data.starts_with("identity:") {
        "Profilo e spazi"
    } else if data.starts_with("admin:") || data.starts_with("system:") {
        "Amministrazione"
    } else {
        return None;
    };
    Some(section.to_string())
}

fn section_for_command(text: &str) -> Option<String> {
    let command = text
        .split_whitespace()
        .next()?
        .split('@')
        .next()?
        .to_lowercase();
    let section = match command.as_str() {
        "/start" | "/menu" => "Menù principale",
        "/miglioramenti"
        | "/miglioramenti_tutti"
        | "/miglioramenti_da_approvare"
        | "/miglioramenti_da_fare"
        | "/miglioramenti_fatti"
        | "/miglioramenti_archivio" => "Miglioramenti",
        "/alimentazione" | "/alimenti" => "Alimentazione › Alimenti",
        "/ricette" | "/ricetta_nuova" | "/ricette_ingredienti" => "Alimentazione › Ricette",
        "/oggetti" | "/oggetto_nuovo" => "Oggetti",
        "/storico" => "Storico",
        "/profilo" | "/spazi" => "Profilo e spazi",
        _ => return None,
    };
    Some(section.to_string())
}

async fn persist_active_ui(
    pool: &SqlitePool,
    chat_id: i64,
    message_id: MessageId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO telegram_ui_state (chat_id, active_message_id, aggiornato_il)
        VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ON CONFLICT(chat_id) DO UPDATE SET
            active_message_id = excluded.active_message_id,
            aggiornato_il = excluded.aggiornato_il
        "#,
    )
    .bind(chat_id)
    .bind(i64::from(message_id.0))
    .execute(pool)
    .await?;
    Ok(())
}

fn screen_title(text: &str) -> String {
    let title = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Schermata del gestionale");
    truncate(title, MAX_SCREEN_TITLE_CHARS)
}

fn humanize_command(text: &str) -> String {
    let command = text
        .split_whitespace()
        .next()
        .unwrap_or(text)
        .trim_start_matches('/');
    let label = command.replace('_', " ");
    format!("hai aperto/inviato «{label}»")
}

fn humanize_callback(data: &str) -> String {
    let label = if data == "menu:main" {
        "hai premuto «🏠 Menù principale»"
    } else if data.starts_with("food:") {
        "hai usato un pulsante della sezione Alimenti"
    } else if data.starts_with("recipe:") {
        "hai usato un pulsante della sezione Ricette"
    } else if data.starts_with("improve:") {
        "hai usato un pulsante della sezione Miglioramenti"
    } else if data.starts_with("history:") || data.starts_with("storico:") {
        "hai usato un pulsante dello Storico"
    } else if data.starts_with("container:") || data.starts_with("contenitore:") {
        "hai usato un pulsante dei Contenitori"
    } else if data.starts_with("location:") || data.starts_with("luogo:") {
        "hai usato un pulsante dei Luoghi"
    } else if data.starts_with("object:") || data.starts_with("oggetto:") {
        "hai usato un pulsante degli Oggetti"
    } else if data.starts_with("admin:") {
        "hai usato un pulsante dell'Amministrazione"
    } else {
        "hai premuto un pulsante del gestionale"
    };
    label.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titolo_schermata_usa_la_prima_riga_non_vuota() {
        assert_eq!(
            screen_title("\n\n🥕 Alimenti\nTotale: 10"),
            "🥕 Alimenti".to_string()
        );
    }

    #[test]
    fn callback_tecnici_non_vengono_esposti_nel_contesto() {
        assert!(!humanize_callback("food:detail:42").contains("food:detail"));
        assert!(!humanize_callback("recipe:menu").contains("recipe:menu"));
    }

    #[test]
    fn snapshot_indica_anche_la_sezione_corrente() {
        let store = ImproveContextStore::default();
        let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
            "⬅️ Indietro".to_string(),
            "recipe:menu".to_string(),
        )]]);
        let token = store.create_snapshot(1, "🍳 Dettaglio ricetta", Some(keyboard));
        let snapshot = store.get_snapshot(1, token).unwrap();
        assert_eq!(snapshot.section, "Alimentazione › Ricette");
        assert!(snapshot
            .summary()
            .contains("Sezione: Alimentazione › Ricette"));
    }

    #[test]
    fn menu_principale_non_viene_scambiato_per_il_primo_modulo_della_tastiera() {
        let store = ImproveContextStore::default();
        let keyboard = InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback(
                "📜 Storico".to_string(),
                "history:global:0".to_string(),
            )],
            vec![InlineKeyboardButton::callback(
                "🏷️ Oggetti".to_string(),
                "oggetti:menu".to_string(),
            )],
        ]);
        let token = store.create_snapshot(
            1,
            "🏠 Gestionale Casa\n\nScegli una sezione.",
            Some(keyboard),
        );
        let snapshot = store.get_snapshot(1, token).unwrap();
        assert_eq!(snapshot.section, "Menù principale");
    }

    #[test]
    fn azione_callback_indica_la_sezione_di_destinazione() {
        let store = ImproveContextStore::default();
        let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
            "🏠 Menù principale".to_string(),
            "menu:main".to_string(),
        )]]);
        store.remember_callback_labels(1, &keyboard);
        store.record_callback(1, "menu:main");
        let token = store.create_snapshot(
            1,
            "🏠 Gestionale Casa\n\nScegli una sezione.",
            Some(keyboard),
        );
        let snapshot = store.get_snapshot(1, token).unwrap();
        assert_eq!(
            snapshot.recent_actions[0],
            "hai premuto «🏠 Menù principale» → Menù principale"
        );
    }

    #[test]
    fn snapshot_mostra_prima_l_azione_piu_recente() {
        let store = ImproveContextStore::default();
        store.record_text(1, Some("prima"));
        store.record_text(1, Some("seconda"));
        let token = store.create_snapshot(1, "Test", None);
        let snapshot = store.get_snapshot(1, token).unwrap();
        assert_eq!(snapshot.recent_actions[0], "hai scritto «seconda»");
        assert_eq!(snapshot.recent_actions[1], "hai scritto «prima»");
    }
}
