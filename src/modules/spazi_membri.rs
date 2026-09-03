//! Membri e inviti privati degli spazi condivisi - Step 7.2H.4.
//!
//! L'accesso al gestionale e la membership di uno spazio restano concetti
//! separati. Gli utenti normali non ricevono mai un elenco degli account
//! registrati: proprietario/amministratore genera un deep-link privato e il
//! destinatario sceglie esplicitamente se accettare.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    prelude::*,
    types::{CopyTextButton, InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::identity;
use crate::modules::calendario;
use crate::modules::liste;

type Bot = crate::context_bot::ContextBot;

const PAGE_SIZE: i64 = liste::VOCI_PER_PAGINA as i64;
const UNLIMITED_USES: i64 = 2_147_483_647;
const INVITE_PREFIX: &str = "spazio_";

#[derive(Debug, Clone)]
enum ManualTimeTarget {
    New { role: String, date: String },
    Existing { invite_id: i64 },
    NewMaxUses { role: String },
    ExistingMaxUses { invite_id: i64 },
}

fn manual_time_states() -> &'static Mutex<HashMap<i64, ManualTimeTarget>> {
    static STATES: OnceLock<Mutex<HashMap<i64, ManualTimeTarget>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn set_manual_time(chat_id: i64, target: ManualTimeTarget) {
    if let Ok(mut states) = manual_time_states().lock() {
        states.insert(chat_id, target);
    }
}

fn peek_manual_time(chat_id: i64) -> Option<ManualTimeTarget> {
    manual_time_states().lock().ok()?.get(&chat_id).cloned()
}

fn clear_manual_time(chat_id: i64) {
    if let Ok(mut states) = manual_time_states().lock() {
        states.remove(&chat_id);
    }
}

pub fn clear_pending_input(chat_id: i64) {
    clear_manual_time(chat_id);
}

#[derive(Debug, Clone, FromRow)]
struct SpaceContext {
    id: i64,
    nome: String,
    tipo: String,
    ruolo: String,
}

#[derive(Debug, Clone, FromRow)]
struct MemberRow {
    user_id: i64,
    nome: String,
    ruolo: String,
    telegram_username: Option<String>,
    chat_id: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct InviteRow {
    id: i64,
    spazio_id: i64,
    spazio_nome: String,
    creato_da_utente_id: i64,
    creatore_nome: String,
    token_link: String,
    ruolo_proposto: String,
    tipo_invito: String,
    scade_il: Option<String>,
    scadenza_locale: Option<String>,
    utilizzi_massimi: i64,
    utilizzi: i64,
    creazione_locale: String,
}

pub async fn active_space_supports_members(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> Result<bool> {
    let Some(user_id) = actor.utente_id else {
        return Ok(false);
    };
    let value: Option<String> = sqlx::query_scalar(
        "SELECT s.tipo FROM spazi s JOIN membri_spazio ms ON ms.spazio_id = s.id \
         WHERE s.id = ? AND ms.utente_id = ?",
    )
    .bind(actor.spazio_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile verificare il tipo dello spazio attivo")?;
    Ok(matches!(value.as_deref(), Some("famiglia" | "condiviso")))
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;
    let Some(target) = peek_manual_time(chat_id) else {
        return Ok(false);
    };
    let Some(text) = msg.text() else {
        return Ok(false);
    };
    if text.starts_with('/') {
        clear_manual_time(chat_id);
        return Ok(false);
    }
    let value = text.trim();

    if matches!(
        target,
        ManualTimeTarget::New { .. } | ManualTimeTarget::Existing { .. }
    ) && !valid_time_strict(value)
    {
        bot.send_message(
            msg.chat.id,
            "⚠️ Orario non valido.\n\nScrivilo esattamente nel formato 24 ore HH:MM, per esempio 12:43. Puoi riprovare subito oppure usare Indietro.",
        )
        .reply_markup(manual_time_back_keyboard(&target))
        .await?;
        return Ok(true);
    }

    match target {
        ManualTimeTarget::New { role, date } => {
            match local_expiry_to_utc(pool, &date, value).await {
                Ok(expiry) => {
                    clear_manual_time(chat_id);
                    create_and_show_invite(
                        bot,
                        msg.chat.id,
                        pool,
                        actor,
                        (&role, "scadenza", UNLIMITED_USES, Some(expiry)),
                    )
                    .await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nInserisci di nuovo l'orario nel formato HH:MM."),
                    )
                    .reply_markup(manual_time_back_keyboard(&ManualTimeTarget::New {
                        role,
                        date,
                    }))
                    .await?;
                }
            }
        }
        ManualTimeTarget::Existing { invite_id } => {
            match update_invite_time(pool, actor, invite_id, value).await {
                Ok(()) => {
                    clear_manual_time(chat_id);
                    show_invite_detail(bot, msg.chat.id, pool, actor, invite_id).await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nInserisci di nuovo l'orario nel formato HH:MM."),
                    )
                    .reply_markup(manual_time_back_keyboard(&ManualTimeTarget::Existing {
                        invite_id,
                    }))
                    .await?;
                }
            }
        }
        ManualTimeTarget::NewMaxUses { role } => {
            let Ok(max_uses) = value.parse::<i64>() else {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Numero di utilizzi non valido.\n\nScrivi un numero intero da 1 a 9999 oppure usa uno dei pulsanti rapidi.",
                )
                .reply_markup(max_uses_back_keyboard_new(&role))
                .await?;
                return Ok(true);
            };
            if !(1..=9_999).contains(&max_uses) {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Numero di utilizzi non valido.\n\nIl limite deve essere compreso tra 1 e 9999.",
                )
                .reply_markup(max_uses_back_keyboard_new(&role))
                .await?;
                return Ok(true);
            }
            clear_manual_time(chat_id);
            let kind = if max_uses == 1 { "monouso" } else { "limite" };
            create_and_show_invite(bot, msg.chat.id, pool, actor, (&role, kind, max_uses, None))
                .await?;
        }
        ManualTimeTarget::ExistingMaxUses { invite_id } => {
            let Ok(max_uses) = value.parse::<i64>() else {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Numero di utilizzi non valido.\n\nScrivi un numero intero da 1 a 9999 oppure usa uno dei pulsanti rapidi.",
                )
                .reply_markup(max_uses_back_keyboard_existing(invite_id))
                .await?;
                return Ok(true);
            };
            match update_invite_max_uses(pool, actor, invite_id, max_uses).await {
                Ok(()) => {
                    clear_manual_time(chat_id);
                    show_invite_detail(bot, msg.chat.id, pool, actor, invite_id).await?;
                }
                Err(error) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {error}\n\nInserisci un nuovo limite oppure torna indietro."),
                    )
                    .reply_markup(max_uses_back_keyboard_existing(invite_id))
                    .await?;
                }
            }
        }
    }
    Ok(true)
}

pub async fn handle_start_payload(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    payload: &str,
) -> ResponseResult<bool> {
    let Some(token) = payload.strip_prefix(INVITE_PREFIX) else {
        return Ok(false);
    };
    if !valid_token(token) {
        bot.send_message(chat_id, "⚠️ Il link d'invito non è valido.")
            .await?;
        return Ok(true);
    }
    let _ = cleanup_inactive_invites(pool).await;
    match load_active_invite_by_token(pool, token).await {
        Ok(Some(invite)) => {
            let already_member = actor
                .utente_id
                .is_some_and(|user_id| user_id == invite.creato_da_utente_id)
                || is_member(pool, actor.utente_id, invite.spazio_id)
                    .await
                    .unwrap_or(false);
            if already_member {
                bot.send_message(
                    chat_id,
                    format!(
                        "👥 Fai già parte dello spazio {}.\n\nL'invito non è stato utilizzato e non ha consumato alcun accesso.",
                        invite.spazio_nome
                    ),
                )
                .reply_markup(already_member_keyboard())
                .await?;
                return Ok(true);
            }
            bot.send_message(
                chat_id,
                format!(
                    "👥 Invito a uno spazio\n\n{} ti invita a:\n🏠 {}\n\nRuolo proposto: {} {}\n{}\n\nL'accesso allo spazio verrà creato soltanto se premi ✅ Accetta invito.",
                    invite.creatore_nome,
                    invite.spazio_nome,
                    role_icon(&invite.ruolo_proposto),
                    role_label(&invite.ruolo_proposto),
                    invite_validity_line(&invite),
                ),
            )
            .reply_markup(invite_accept_keyboard(&invite.token_link))
            .await?;
        }
        Ok(None) => {
            bot.send_message(
                chat_id,
                "⌛ Questo invito non è più disponibile: può essere scaduto, revocato o aver raggiunto il limite di utilizzi.",
            )
            .await?;
        }
        Err(error) => {
            tracing::warn!(?error, "Lettura invito da deep-link fallita");
            bot.send_message(chat_id, "⚠️ Non riesco a verificare questo invito.")
                .await?;
        }
    }
    Ok(true)
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    data: &str,
) -> ResponseResult<bool> {
    if !data.starts_with("space-members:invite:manual-time") {
        clear_manual_time(chat_id.0);
    }
    if data == "space-members:menu" {
        let _ = cleanup_inactive_invites(pool).await;
        show_members(bot, chat_id, pool, actor, 0).await?;
        return Ok(true);
    }
    if let Some(page) = parse_page(data, "space-members:list:") {
        show_members(bot, chat_id, pool, actor, page).await?;
        return Ok(true);
    }
    if let Some(user_id) = parse_positive(data, "space-members:view:") {
        show_member_detail(bot, chat_id, pool, actor, user_id).await?;
        return Ok(true);
    }
    if let Some(user_id) = parse_positive(data, "space-members:remove:") {
        show_remove_confirmation(bot, chat_id, pool, actor, user_id).await?;
        return Ok(true);
    }
    if let Some(user_id) = parse_positive(data, "space-members:remove-confirm:") {
        match remove_member(pool, actor, user_id).await {
            Ok(member) => {
                bot.send_message(
                    chat_id,
                    format!("✅ {} rimosso dallo spazio. Il suo account e il suo spazio personale restano invariati.", member.nome),
                )
                .await?;
                notify_member_removed(bot, &member, &actor.spazio_nome_snapshot).await;
                show_members(bot, chat_id, pool, actor, 0).await?;
            }
            Err(error) => {
                tracing::warn!(?error, user_id, "Rimozione membro spazio rifiutata");
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(back_to_members_keyboard())
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(user_id) = parse_positive(data, "space-members:edit-role:") {
        show_member_role_picker(bot, chat_id, pool, actor, user_id).await?;
        return Ok(true);
    }
    if let Some((user_id, role)) = parse_member_role_set(data) {
        match update_member_role(pool, actor, user_id, role).await {
            Ok((member, old_role)) => {
                bot.send_message(
                    chat_id,
                    format!(
                        "✅ Ruolo di {} aggiornato: {} → {}.",
                        member.nome,
                        role_label(&old_role),
                        role_label(role)
                    ),
                )
                .await?;
                notify_role_changed(bot, &member, &actor.spazio_nome_snapshot, &old_role, role)
                    .await;
                show_member_detail(bot, chat_id, pool, actor, user_id).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(back_to_members_keyboard())
                    .await?;
            }
        }
        return Ok(true);
    }

    if data == "space-members:invite:new" {
        show_invite_role_picker(bot, chat_id, pool, actor).await?;
        return Ok(true);
    }
    if let Some(role) = parse_invite_role(data, "space-members:invite:role:") {
        show_invite_type_picker(bot, chat_id, pool, actor, role).await?;
        return Ok(true);
    }
    if let Some((role, kind)) = parse_invite_kind(data) {
        match kind {
            "one" => {
                create_and_show_invite(bot, chat_id, pool, actor, (role, "monouso", 1, None))
                    .await?
            }
            "free" => {
                create_and_show_invite(
                    bot,
                    chat_id,
                    pool,
                    actor,
                    (role, "riutilizzabile", UNLIMITED_USES, None),
                )
                .await?
            }
            "max" => show_max_uses_picker(bot, chat_id, pool, actor, role).await?,
            "exp" => show_expiry_calendar(bot, chat_id, pool, actor, role, None, None).await?,
            _ => {}
        }
        return Ok(true);
    }
    if let Some((role, max_uses)) = parse_invite_max(data) {
        create_and_show_invite(bot, chat_id, pool, actor, (role, "limite", max_uses, None)).await?;
        return Ok(true);
    }
    if let Some((role, year, month)) = parse_new_calendar_nav(data) {
        show_expiry_calendar(bot, chat_id, pool, actor, role, None, Some((year, month))).await?;
        return Ok(true);
    }
    if let Some((role, date)) = parse_new_expiry_date(data) {
        show_new_time_picker(bot, chat_id, pool, actor, role, &date, "23:59").await?;
        return Ok(true);
    }
    if let Some((role, date, time)) = parse_new_time_pick(data) {
        match local_expiry_to_utc(pool, &date, &time).await {
            Ok(expiry) => {
                create_and_show_invite(
                    bot,
                    chat_id,
                    pool,
                    actor,
                    (role, "scadenza", UNLIMITED_USES, Some(expiry)),
                )
                .await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(calendar_back_keyboard_new(role, &date))
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some((role, date)) = parse_manual_time_new(data) {
        set_manual_time(
            chat_id.0,
            ManualTimeTarget::New {
                role: role.to_string(),
                date: date.clone(),
            },
        );
        bot.send_message(
            chat_id,
            format!(
                "⌨️ Inserisci orario\n\nData: {}\n\nScrivi l'orario esattamente nel formato 24 ore HH:MM, per esempio 12:43.",
                human_date(&date)
            ),
        )
        .reply_markup(manual_time_back_keyboard(&ManualTimeTarget::New {
            role: role.to_string(),
            date,
        }))
        .await?;
        return Ok(true);
    }
    if let Some((role, date, time)) = parse_new_expiry_time(data) {
        match local_expiry_to_utc(pool, &date, &time).await {
            Ok(expiry) => {
                create_and_show_invite(
                    bot,
                    chat_id,
                    pool,
                    actor,
                    (role, "scadenza", UNLIMITED_USES, Some(expiry)),
                )
                .await?
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }

    if data == "space-members:invite:list" {
        let _ = cleanup_inactive_invites(pool).await;
        show_active_invites(bot, chat_id, pool, actor, 0).await?;
        return Ok(true);
    }
    if let Some(page) = parse_page(data, "space-members:invite:list:") {
        let _ = cleanup_inactive_invites(pool).await;
        show_active_invites(bot, chat_id, pool, actor, page).await?;
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:view:") {
        let _ = cleanup_inactive_invites(pool).await;
        show_invite_detail(bot, chat_id, pool, actor, invite_id).await?;
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:revoke:") {
        match delete_manageable_invite(pool, actor, invite_id).await {
            Ok(()) => {
                bot.send_message(chat_id, "❌ Invito revocato ed eliminato.")
                    .await?;
                show_active_invites(bot, chat_id, pool, actor, 0).await?;
            }
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:edit-role:") {
        show_invite_edit_role(bot, chat_id, pool, actor, invite_id).await?;
        return Ok(true);
    }
    if let Some((invite_id, role)) = parse_invite_set_role(data) {
        match update_invite_role(pool, actor, invite_id, role).await {
            Ok(()) => show_invite_detail(bot, chat_id, pool, actor, invite_id).await?,
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:edit-date:") {
        show_existing_expiry_calendar(bot, chat_id, pool, actor, invite_id, None).await?;
        return Ok(true);
    }
    if let Some((invite_id, year, month)) = parse_existing_calendar_nav(data) {
        show_existing_expiry_calendar(bot, chat_id, pool, actor, invite_id, Some((year, month)))
            .await?;
        return Ok(true);
    }
    if let Some((invite_id, date)) = parse_existing_expiry_date(data) {
        match update_invite_date(pool, actor, invite_id, &date).await {
            Ok(()) => show_invite_detail(bot, chat_id, pool, actor, invite_id).await?,
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:edit-time:") {
        show_existing_time_picker(bot, chat_id, pool, actor, invite_id).await?;
        return Ok(true);
    }
    if let Some((invite_id, time)) = parse_existing_expiry_time(data) {
        match update_invite_time(pool, actor, invite_id, &time).await {
            Ok(()) => show_invite_detail(bot, chat_id, pool, actor, invite_id).await?,
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(back_to_invites_keyboard())
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:manual-time-existing:") {
        set_manual_time(chat_id.0, ManualTimeTarget::Existing { invite_id });
        bot.send_message(
            chat_id,
            "⌨️ Inserisci orario\n\nScrivi il nuovo orario esattamente nel formato 24 ore HH:MM, per esempio 12:43.",
        )
        .reply_markup(manual_time_back_keyboard(&ManualTimeTarget::Existing { invite_id }))
        .await?;
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:edit-max:") {
        show_existing_max_uses_picker(bot, chat_id, pool, actor, invite_id).await?;
        return Ok(true);
    }
    if let Some((invite_id, max_uses)) = parse_existing_max_uses(data) {
        match update_invite_max_uses(pool, actor, invite_id, max_uses).await {
            Ok(()) => show_invite_detail(bot, chat_id, pool, actor, invite_id).await?,
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(back_to_invites_keyboard())
                    .await?;
            }
        }
        return Ok(true);
    }
    if let Some(invite_id) = parse_positive(data, "space-members:invite:remove-expiry:") {
        match remove_invite_expiry(pool, actor, invite_id).await {
            Ok(()) => show_invite_detail(bot, chat_id, pool, actor, invite_id).await?,
            Err(error) => {
                bot.send_message(chat_id, format!("⚠️ {error}")).await?;
            }
        }
        return Ok(true);
    }

    if let Some(token) = data.strip_prefix("space-members:invite:accept:") {
        if valid_token(token) {
            match accept_invite(pool, actor, token).await {
                Ok(AcceptResult::Joined {
                    invite,
                    member_name,
                }) => {
                    bot.send_message(
                        chat_id,
                        format!(
                            "✅ Sei entrato nello spazio {} come {} {}.",
                            invite.spazio_nome,
                            role_icon(&invite.ruolo_proposto),
                            role_label(&invite.ruolo_proposto)
                        ),
                    )
                    .reply_markup(open_space_keyboard(invite.spazio_id))
                    .await?;
                    notify_invite_creator(bot, pool, &invite, &member_name).await;
                }
                Ok(AcceptResult::AlreadyMember { invite }) => {
                    bot.send_message(
                        chat_id,
                        format!("👥 Fai già parte dello spazio {}.", invite.spazio_nome),
                    )
                    .reply_markup(open_space_keyboard(invite.spazio_id))
                    .await?;
                }
                Err(error) => {
                    bot.send_message(chat_id, format!("⚠️ {error}")).await?;
                }
            }
        }
        return Ok(true);
    }
    if let Some(token) = data.strip_prefix("space-members:invite:reject:") {
        if valid_token(token) {
            bot.send_message(
                chat_id,
                "❌ Invito rifiutato. Nessuna membership è stata creata.",
            )
            .await?;
        }
        return Ok(true);
    }

    if data == "space-members:noop" {
        return Ok(true);
    }
    Ok(false)
}

async fn show_members(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    requested_page: i64,
) -> ResponseResult<()> {
    let context = match load_space_context(pool, actor).await {
        Ok(value) => value,
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(spaces_back_keyboard())
                .await?;
            return Ok(());
        }
    };
    if context.tipo == "personale" {
        bot.send_message(chat_id, "🔒 Lo spazio personale non può avere altri membri. Crea o seleziona uno spazio condiviso per collaborare.")
            .reply_markup(spaces_back_keyboard())
            .await?;
        return Ok(());
    }
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM membri_spazio WHERE spazio_id = ?")
        .bind(context.id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let pages = liste::totale_pagine(total);
    let page = requested_page.max(0).min(pages.saturating_sub(1));
    let members = sqlx::query_as::<_, MemberRow>(
        "SELECT u.id AS user_id, u.nome_visualizzato AS nome, ms.ruolo, \
                (SELECT at.username_snapshot FROM account_telegram at WHERE at.utente_id = u.id ORDER BY at.id LIMIT 1) AS telegram_username, \
                (SELECT at.chat_id FROM account_telegram at WHERE at.utente_id = u.id ORDER BY at.id LIMIT 1) AS chat_id \
         FROM membri_spazio ms JOIN utenti u ON u.id = ms.utente_id \
         WHERE ms.spazio_id = ? \
         ORDER BY CASE ms.ruolo WHEN 'proprietario' THEN 0 WHEN 'amministratore' THEN 1 WHEN 'membro' THEN 2 ELSE 3 END, u.nome_visualizzato COLLATE NOCASE, u.id \
         LIMIT ? OFFSET ?",
    )
    .bind(context.id)
    .bind(PAGE_SIZE)
    .bind(page * PAGE_SIZE)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let can_manage = can_manage_role(&context.ruolo);
    let mut rows = members
        .iter()
        .map(|member| {
            vec![InlineKeyboardButton::callback(
                format!("{} {}", role_icon(&member.ruolo), member.nome),
                format!("space-members:view:{}", member.user_id),
            )]
        })
        .collect::<Vec<_>>();
    if let Some(row) = liste::riga_paginazione(page, pages, "space-members:noop", |p| {
        format!("space-members:list:{p}")
    }) {
        rows.push(row);
    }
    if can_manage {
        rows.push(vec![InlineKeyboardButton::callback(
            "➕ Invita membro".to_string(),
            "space-members:invite:new".to_string(),
        )]);
        rows.push(vec![InlineKeyboardButton::callback(
            "🔗 Inviti attivi".to_string(),
            "space-members:invite:list".to_string(),
        )]);
    }
    rows.push(vec![
        InlineKeyboardButton::callback("⬅️ Spazi".to_string(), "identity:spaces".to_string()),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]);
    bot.send_message(chat_id, format!(
        "👥 Membri · {}\n\nTotale: {}\nPagina {}/{}\n\n{}",
        context.nome, total, page + 1, pages,
        if can_manage { "Puoi gestire ruoli, rimuovere membri e creare inviti privati. Gli account del gestionale non vengono mostrati in elenco." } else { "Puoi consultare i membri, ma non modificarli." }
    ))
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

async fn show_member_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    user_id: i64,
) -> ResponseResult<()> {
    let context = match load_space_context(pool, actor).await {
        Ok(value) if value.tipo != "personale" => value,
        Ok(_) => {
            bot.send_message(chat_id, "🔒 Lo spazio personale non gestisce altri membri.")
                .reply_markup(spaces_back_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(spaces_back_keyboard())
                .await?;
            return Ok(());
        }
    };
    let member = match member_by_id(pool, context.id, user_id).await {
        Ok(Some(value)) => value,
        _ => {
            bot.send_message(chat_id, "⚠️ Questo account non appartiene più allo spazio.")
                .reply_markup(back_to_members_keyboard())
                .await?;
            return Ok(());
        }
    };
    let username_line = member
        .telegram_username
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(|v| format!("\nTelegram: @{v}"))
        .unwrap_or_default();
    let actor_user_id = actor.utente_id.unwrap_or_default();
    let mut rows = Vec::new();
    if can_manage_role(&context.ruolo)
        && member.user_id != actor_user_id
        && member.ruolo != "proprietario"
    {
        rows.push(vec![InlineKeyboardButton::callback(
            "✏️ Modifica ruolo".to_string(),
            format!("space-members:edit-role:{}", member.user_id),
        )]);
        rows.push(vec![InlineKeyboardButton::callback(
            "🚪 Rimuovi dallo spazio".to_string(),
            format!("space-members:remove:{}", member.user_id),
        )]);
    }
    rows.push(nav_row("⬅️ Membri", "space-members:menu"));
    bot.send_message(chat_id, format!(
        "👤 {}\n\nRuolo: {} {}{}\nSpazio: {}\n\nLa rimozione non elimina l'account né il suo spazio personale.",
        member.nome, role_icon(&member.ruolo), role_label(&member.ruolo), username_line, context.nome
    )).reply_markup(InlineKeyboardMarkup::new(rows)).await?;
    Ok(())
}

async fn show_member_role_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    user_id: i64,
) -> ResponseResult<()> {
    let context = match load_manageable_space(pool, actor).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    let Some(member) = member_by_id(pool, context.id, user_id).await.ok().flatten() else {
        bot.send_message(chat_id, "⚠️ Membro non trovato.").await?;
        return Ok(());
    };
    if member.ruolo == "proprietario" || actor.utente_id == Some(user_id) {
        bot.send_message(
            chat_id,
            "⚠️ Questo ruolo non può essere modificato da questa schermata.",
        )
        .await?;
        return Ok(());
    }
    bot.send_message(
        chat_id,
        format!(
            "✏️ Modifica ruolo\n\nMembro: {}\nRuolo attuale: {} {}",
            member.nome,
            role_icon(&member.ruolo),
            role_label(&member.ruolo)
        ),
    )
    .reply_markup(role_keyboard_for_member(user_id))
    .await?;
    Ok(())
}

async fn show_remove_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    user_id: i64,
) -> ResponseResult<()> {
    let context = match load_manageable_space(pool, actor).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    let member = match removable_member(pool, actor, context.id, user_id).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    bot.send_message(chat_id, format!("🚪 Rimuovere {} dallo spazio {}?\n\nPerderà l'accesso alle risorse visibili soltanto tramite questo spazio. Il suo account e il suo spazio personale non verranno eliminati.", member.nome, context.nome))
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback("🚪 Conferma rimozione".to_string(), format!("space-members:remove-confirm:{user_id}"))],
            nav_row("⬅️ Indietro", &format!("space-members:view:{user_id}")),
        ])).await?;
    Ok(())
}

async fn show_invite_role_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> ResponseResult<()> {
    let context = match load_manageable_space(pool, actor).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    bot.send_message(chat_id, format!("➕ Invita membro · {}\n\nScegli il ruolo che verrà proposto a chi userà il link. Non viene mostrato alcun elenco degli utenti registrati.", context.nome))
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback("👤 Membro".to_string(), "space-members:invite:role:m".to_string())],
            vec![InlineKeyboardButton::callback("👁️ Sola lettura".to_string(), "space-members:invite:role:r".to_string())],
            vec![InlineKeyboardButton::callback("🛡️ Amministratore".to_string(), "space-members:invite:role:a".to_string())],
            nav_row("⬅️ Membri", "space-members:menu"),
        ])).await?;
    Ok(())
}

async fn show_invite_type_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    role: &str,
) -> ResponseResult<()> {
    let context = match load_manageable_space(pool, actor).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    let r = role_code(role);
    bot.send_message(
        chat_id,
        format!(
            "🔗 Tipo di invito · {}\n\nRuolo: {} {}\n\nScegli come deve funzionare il link.",
            context.nome,
            role_icon(role),
            role_label(role)
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "1️⃣ Monouso".to_string(),
            format!("space-members:invite:type:{r}:one"),
        )],
        vec![InlineKeyboardButton::callback(
            "♾️ Riutilizzabile".to_string(),
            format!("space-members:invite:type:{r}:free"),
        )],
        vec![InlineKeyboardButton::callback(
            "🔢 Numero massimo di utilizzi".to_string(),
            format!("space-members:invite:type:{r}:max"),
        )],
        vec![InlineKeyboardButton::callback(
            "⏳ Con scadenza".to_string(),
            format!("space-members:invite:type:{r}:exp"),
        )],
        nav_row("⬅️ Ruolo", "space-members:invite:new"),
    ]))
    .await?;
    Ok(())
}

async fn show_max_uses_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    role: &str,
) -> ResponseResult<()> {
    if load_manageable_space(pool, actor).await.is_err() {
        return Ok(());
    }
    let r = role_code(role);
    set_manual_time(
        chat_id.0,
        ManualTimeTarget::NewMaxUses {
            role: role.to_string(),
        },
    );
    bot.send_message(chat_id, "🔢 Numero massimo di utilizzi\n\nQuando il limite viene raggiunto, l'invito viene eliminato automaticamente.\n\nPuoi scegliere un pulsante rapido oppure scrivere direttamente un numero da 1 a 9999.")
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![2_i64, 5, 10].into_iter().map(|n| InlineKeyboardButton::callback(n.to_string(), format!("space-members:invite:max:{r}:{n}"))).collect::<Vec<_>>(),
            vec![InlineKeyboardButton::callback("20 utilizzi".to_string(), format!("space-members:invite:max:{r}:20"))],
            nav_row("⬅️ Tipo invito", &format!("space-members:invite:role:{r}")),
        ])).await?;
    Ok(())
}

async fn show_existing_max_uses_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
) -> ResponseResult<()> {
    let invite = match load_manageable_invite(pool, actor, invite_id).await {
        Ok(value) => value,
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(back_to_invites_keyboard())
                .await?;
            return Ok(());
        }
    };
    set_manual_time(chat_id.0, ManualTimeTarget::ExistingMaxUses { invite_id });
    bot.send_message(
        chat_id,
        format!(
            "🔢 Modifica utilizzi\n\nConfigurazione attuale: {}\n\nCon 1 utilizzo l'invito diventa monouso; con un valore maggiore diventa a utilizzi; con ♾️ diventa riutilizzabile. L'eventuale scadenza resta invariata.\n\nPuoi anche scrivere direttamente un numero da 1 a 9999.",
            invite_usage_line(&invite)
        ),
    )
    .reply_markup(existing_max_uses_keyboard(invite_id))
    .await?;
    Ok(())
}

async fn show_expiry_calendar(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    role: &str,
    invite_id: Option<i64>,
    requested_month: Option<(i32, u32)>,
) -> ResponseResult<()> {
    if load_manageable_space(pool, actor).await.is_err() {
        return Ok(());
    }
    let current = current_local_date(pool).await.unwrap_or((2026, 1, 1));
    let requested = requested_month.unwrap_or((current.0, current.1));
    let (year, month) = if (requested.0, requested.1) < (current.0, current.1) {
        (current.0, current.1)
    } else {
        requested
    };
    let rows = calendar_rows(year, month, role, invite_id, current);
    bot.send_message(chat_id, format!("📅 Seleziona la data di scadenza\n\n{} {}\nL'orario predefinito sarà 23:59 e potrai modificarlo anche dopo la creazione.", calendario::month_name(month), year))
        .reply_markup(InlineKeyboardMarkup::new(rows)).await?;
    Ok(())
}

async fn show_existing_expiry_calendar(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
    requested_month: Option<(i32, u32)>,
) -> ResponseResult<()> {
    let invite = match load_manageable_invite(pool, actor, invite_id).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    show_expiry_calendar(
        bot,
        chat_id,
        pool,
        actor,
        &invite.ruolo_proposto,
        Some(invite_id),
        requested_month,
    )
    .await
}

async fn show_new_time_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    role: &str,
    date: &str,
    time: &str,
) -> ResponseResult<()> {
    if load_manageable_space(pool, actor).await.is_err() {
        return Ok(());
    }
    let r = role_code(role);
    set_manual_time(
        chat_id.0,
        ManualTimeTarget::New {
            role: role.to_string(),
            date: date.to_string(),
        },
    );
    bot.send_message(chat_id, format!("🕒 Orario di scadenza\n\nData: {}\n\nScegli un orario rapido oppure inseriscilo manualmente. La scelta viene applicata subito.", human_date(date)))
        .reply_markup(time_keyboard_new(r, date, time)).await?;
    Ok(())
}

async fn create_and_show_invite(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    (role, kind, max_uses, expiry): (&str, &str, i64, Option<String>),
) -> ResponseResult<()> {
    match create_invite(pool, actor, role, kind, max_uses, expiry.as_deref()).await {
        Ok(invite) => show_invite_created(bot, chat_id, &invite).await?,
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(back_to_members_keyboard())
                .await?;
        }
    }
    Ok(())
}

async fn show_invite_created(bot: &Bot, chat_id: ChatId, invite: &InviteRow) -> ResponseResult<()> {
    let link = match invite_link(bot, &invite.token_link).await {
        Ok(v) => v,
        Err(error) => {
            tracing::warn!(?error, "Impossibile costruire deep-link invito");
            bot.send_message(
                chat_id,
                "⚠️ Invito creato, ma non riesco a costruire il link Telegram.",
            )
            .await?;
            return Ok(());
        }
    };
    bot.send_message(chat_id, format!(
        "🔗 Invito creato\n\nSpazio: {}\nRuolo: {} {}\nTipo: {}\n{}\n\nInvia questo link soltanto alla persona o al gruppo che vuoi autorizzare.",
        invite.spazio_nome, role_icon(&invite.ruolo_proposto), role_label(&invite.ruolo_proposto), invite_type_label(&invite.tipo_invito), invite_validity_line(invite)
    )).reply_markup(invite_detail_keyboard(invite, &link)).await?;
    Ok(())
}

async fn show_active_invites(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    requested_page: i64,
) -> ResponseResult<()> {
    let context = match load_manageable_space(pool, actor).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inviti_spazio WHERE spazio_id = ? AND token_link IS NOT NULL AND revocato_il IS NULL AND utilizzi < utilizzi_massimi AND (scade_il IS NULL OR julianday(scade_il) > julianday('now'))")
        .bind(context.id).fetch_one(pool).await.unwrap_or(0);
    if total == 0 {
        bot.send_message(
            chat_id,
            format!(
                "🔗 Inviti attivi · {}\n\nNon ci sono inviti utilizzabili in questo momento.",
                context.nome
            ),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback(
                "➕ Nuovo invito".to_string(),
                "space-members:invite:new".to_string(),
            )],
            nav_row("⬅️ Membri", "space-members:menu"),
        ]))
        .await?;
        return Ok(());
    }
    let pages = liste::totale_pagine(total);
    let page = requested_page.max(0).min(pages - 1);
    let invites =
        sqlx::query_as::<_, InviteRow>(&invite_select_sql("i.spazio_id = ?", "LIMIT ? OFFSET ?"))
            .bind(context.id)
            .bind(PAGE_SIZE)
            .bind(page * PAGE_SIZE)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
    let mut rows = invites
        .iter()
        .map(|invite| {
            vec![InlineKeyboardButton::callback(
                format!(
                    "🔗 {} · {}",
                    invite_type_short(&invite.tipo_invito),
                    role_label(&invite.ruolo_proposto)
                ),
                format!("space-members:invite:view:{}", invite.id),
            )]
        })
        .collect::<Vec<_>>();
    if let Some(row) = liste::riga_paginazione(page, pages, "space-members:noop", |p| {
        format!("space-members:invite:list:{p}")
    }) {
        rows.push(row);
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "➕ Nuovo invito".to_string(),
        "space-members:invite:new".to_string(),
    )]);
    rows.push(nav_row("⬅️ Membri", "space-members:menu"));
    bot.send_message(chat_id, format!("🔗 Inviti attivi · {}\n\nTotale: {}\nPagina {}/{}\n\nQui compaiono soltanto link ancora utilizzabili.", context.nome, total, page + 1, pages))
        .reply_markup(InlineKeyboardMarkup::new(rows)).await?;
    Ok(())
}

async fn show_invite_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
) -> ResponseResult<()> {
    let invite = match load_manageable_invite(pool, actor, invite_id).await {
        Ok(v) => v,
        Err(error) => {
            bot.send_message(chat_id, format!("⚠️ {error}"))
                .reply_markup(back_to_invites_keyboard())
                .await?;
            return Ok(());
        }
    };
    let link = match invite_link(bot, &invite.token_link).await {
        Ok(v) => v,
        Err(error) => {
            tracing::warn!(?error);
            bot.send_message(chat_id, "⚠️ Non riesco a costruire il link.")
                .await?;
            return Ok(());
        }
    };
    bot.send_message(chat_id, format!(
        "🔗 Dettaglio invito\n\nSpazio: {}\nRuolo: {} {}\nTipo: {}\nCreato il: {}\n{}\nUtilizzi: {}\nStato: ✅ Attivo",
        invite.spazio_nome, role_icon(&invite.ruolo_proposto), role_label(&invite.ruolo_proposto), invite_type_label(&invite.tipo_invito), human_created_at(&invite.creazione_locale), invite_validity_line(&invite), invite_usage_line(&invite)
    )).reply_markup(invite_detail_keyboard(&invite, &link)).await?;
    Ok(())
}

async fn show_invite_edit_role(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
) -> ResponseResult<()> {
    let invite = match load_manageable_invite(pool, actor, invite_id).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    bot.send_message(chat_id, format!("👤 Modifica ruolo futuro\n\nRuolo attuale: {} {}\nLa modifica vale soltanto per i prossimi utilizzi del link; i membri già entrati non cambiano ruolo.", role_icon(&invite.ruolo_proposto), role_label(&invite.ruolo_proposto)))
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback("👤 Membro".to_string(), format!("space-members:invite:set-role:{invite_id}:m"))],
            vec![InlineKeyboardButton::callback("👁️ Sola lettura".to_string(), format!("space-members:invite:set-role:{invite_id}:r"))],
            vec![InlineKeyboardButton::callback("🛡️ Amministratore".to_string(), format!("space-members:invite:set-role:{invite_id}:a"))],
            nav_row("⬅️ Invito", &format!("space-members:invite:view:{invite_id}")),
        ])).await?;
    Ok(())
}

async fn show_existing_time_picker(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
) -> ResponseResult<()> {
    let invite = match load_manageable_invite(pool, actor, invite_id).await {
        Ok(v) => v,
        Err(e) => {
            bot.send_message(chat_id, format!("⚠️ {e}")).await?;
            return Ok(());
        }
    };
    if invite.scade_il.is_none() {
        bot.send_message(
            chat_id,
            "⚠️ Questo invito non ha una scadenza. Usa 📅 Modifica data per aggiungerla.",
        )
        .await?;
        return Ok(());
    }
    let current: String = sqlx::query_scalar("SELECT strftime('%H:%M', ?, 'localtime')")
        .bind(invite.scade_il.as_deref())
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| "23:59".to_string());
    set_manual_time(chat_id.0, ManualTimeTarget::Existing { invite_id });
    bot.send_message(
        chat_id,
        format!("🕒 Modifica orario\n\nOrario attuale: {current}\n\nScegli un orario rapido oppure inseriscilo manualmente. La scelta viene applicata subito."),
    )
    .reply_markup(time_keyboard_existing(invite_id, &current))
    .await?;
    Ok(())
}

async fn create_invite(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    role: &str,
    kind: &str,
    max_uses: i64,
    expiry: Option<&str>,
) -> Result<InviteRow> {
    let context = load_manageable_space(pool, actor).await?;
    let creator = actor.utente_id.context("Identità utente non disponibile")?;
    if !matches!(role, "amministratore" | "membro" | "lettura") {
        bail!("Ruolo invito non valido");
    }
    if !matches!(kind, "monouso" | "riutilizzabile" | "limite" | "scadenza") {
        bail!("Tipo invito non valido");
    }
    if max_uses <= 0 {
        bail!("Numero massimo di utilizzi non valido");
    }
    cleanup_inactive_invites(pool).await?;
    let token: String = sqlx::query_scalar("SELECT lower(hex(randomblob(12)))")
        .fetch_one(pool)
        .await
        .context("Generazione token invito")?;
    let hash_marker: String = sqlx::query_scalar("SELECT lower(hex(randomblob(32)))")
        .fetch_one(pool)
        .await
        .context("Generazione marcatore invito")?;
    let result = sqlx::query("INSERT INTO inviti_spazio (spazio_id, creato_da_utente_id, token_hash, token_link, ruolo_proposto, tipo_invito, scade_il, utilizzi_massimi, utilizzi) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)")
        .bind(context.id).bind(creator).bind(hash_marker).bind(&token).bind(role).bind(kind).bind(expiry).bind(max_uses)
        .execute(pool).await.context("Impossibile creare l'invito")?;
    load_manageable_invite(pool, actor, result.last_insert_rowid()).await
}

#[derive(Debug)]
enum AcceptResult {
    Joined {
        invite: InviteRow,
        member_name: String,
    },
    AlreadyMember {
        invite: InviteRow,
    },
}

async fn accept_invite(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    token: &str,
) -> Result<AcceptResult> {
    let user_id = actor
        .utente_id
        .context("Devi avere un account autorizzato per accettare l'invito")?;
    cleanup_inactive_invites(pool).await?;
    let invite = load_active_invite_by_token(pool, token)
        .await?
        .context("Invito non più disponibile")?;
    if is_member(pool, Some(user_id), invite.spazio_id).await? {
        return Ok(AcceptResult::AlreadyMember { invite });
    }
    let name: String = sqlx::query_scalar(
        "SELECT nome_visualizzato FROM utenti WHERE id = ? AND stato = 'attivo'",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Lettura account")?
    .context("Account non attivo")?;
    let mut tx = pool
        .begin()
        .await
        .context("Transazione accettazione invito")?;
    let still_active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inviti_spazio WHERE id = ? AND token_link = ? AND revocato_il IS NULL AND utilizzi < utilizzi_massimi AND (scade_il IS NULL OR julianday(scade_il) > julianday('now'))")
        .bind(invite.id).bind(token).fetch_one(&mut *tx).await.context("Verifica invito")?;
    if still_active != 1 {
        bail!("Invito non più disponibile");
    }
    sqlx::query("INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, ?)")
        .bind(invite.spazio_id)
        .bind(user_id)
        .bind(&invite.ruolo_proposto)
        .execute(&mut *tx)
        .await
        .context("Impossibile entrare nello spazio")?;
    sqlx::query("UPDATE inviti_spazio SET utilizzi = utilizzi + 1, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
        .bind(invite.id).execute(&mut *tx).await.context("Aggiornamento utilizzi invito")?;
    if invite.tipo_invito == "monouso" || invite.utilizzi + 1 >= invite.utilizzi_massimi {
        sqlx::query("DELETE FROM inviti_spazio WHERE id = ?")
            .bind(invite.id)
            .execute(&mut *tx)
            .await
            .context("Pulizia invito consumato")?;
    }
    tx.commit()
        .await
        .context("Salvataggio accettazione invito")?;
    Ok(AcceptResult::Joined {
        invite,
        member_name: name,
    })
}

pub async fn cleanup_inactive_invites(pool: &SqlitePool) -> Result<u64> {
    let result = sqlx::query("DELETE FROM inviti_spazio WHERE token_link IS NOT NULL AND (revocato_il IS NOT NULL OR utilizzi >= utilizzi_massimi OR (scade_il IS NOT NULL AND julianday(scade_il) <= julianday('now')))")
        .execute(pool).await.context("Pulizia inviti non più attivi")?;
    Ok(result.rows_affected())
}

async fn update_member_role(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    user_id: i64,
    role: &str,
) -> Result<(MemberRow, String)> {
    if !matches!(role, "amministratore" | "membro" | "lettura") {
        bail!("Ruolo non valido");
    }
    let context = load_manageable_space(pool, actor).await?;
    let current = removable_member(pool, actor, context.id, user_id).await?;
    let old = current.ruolo.clone();
    if old == role {
        return Ok((current, old));
    }
    sqlx::query("UPDATE membri_spazio SET ruolo = ? WHERE spazio_id = ? AND utente_id = ? AND ruolo <> 'proprietario'")
        .bind(role).bind(context.id).bind(user_id).execute(pool).await.context("Impossibile modificare il ruolo")?;
    let updated = member_by_id(pool, context.id, user_id)
        .await?
        .context("Membro non trovato dopo modifica")?;
    Ok((updated, old))
}

async fn remove_member(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    user_id: i64,
) -> Result<MemberRow> {
    let context = load_manageable_space(pool, actor).await?;
    let member = removable_member(pool, actor, context.id, user_id).await?;
    let affected = sqlx::query("DELETE FROM membri_spazio WHERE spazio_id = ? AND utente_id = ? AND ruolo <> 'proprietario'")
        .bind(context.id).bind(user_id).execute(pool).await.context("Impossibile rimuovere il membro")?.rows_affected();
    if affected != 1 {
        bail!("Membro non più rimovibile");
    }
    Ok(member)
}

async fn update_invite_role(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
    role: &str,
) -> Result<()> {
    if !matches!(role, "amministratore" | "membro" | "lettura") {
        bail!("Ruolo non valido");
    }
    let invite = load_manageable_invite(pool, actor, invite_id).await?;
    sqlx::query("UPDATE inviti_spazio SET ruolo_proposto = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
        .bind(role).bind(invite.id).execute(pool).await.context("Impossibile aggiornare il ruolo dell'invito")?;
    Ok(())
}

async fn update_invite_date(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
    date: &str,
) -> Result<()> {
    let invite = load_manageable_invite(pool, actor, invite_id).await?;
    let time = if let Some(expiry) = &invite.scade_il {
        sqlx::query_scalar::<_, String>("SELECT strftime('%H:%M', ?, 'localtime')")
            .bind(expiry)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| "23:59".to_string())
    } else {
        "23:59".to_string()
    };
    let expiry = local_expiry_to_utc(pool, date, &time).await?;
    sqlx::query("UPDATE inviti_spazio SET scade_il = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
        .bind(expiry).bind(invite.id).execute(pool).await.context("Impossibile aggiornare la data")?;
    Ok(())
}

async fn update_invite_time(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
    time: &str,
) -> Result<()> {
    let invite = load_manageable_invite(pool, actor, invite_id).await?;
    let expiry = invite.scade_il.context("L'invito non ha una scadenza")?;
    let date: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%d', ?, 'localtime')")
        .bind(expiry)
        .fetch_one(pool)
        .await
        .context("Lettura data scadenza")?;
    let new_expiry = local_expiry_to_utc(pool, &date, time).await?;
    sqlx::query("UPDATE inviti_spazio SET scade_il = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
        .bind(new_expiry).bind(invite.id).execute(pool).await.context("Impossibile aggiornare l'orario")?;
    Ok(())
}

async fn update_invite_max_uses(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
    max_uses: i64,
) -> Result<()> {
    if max_uses != UNLIMITED_USES && !(1..=9_999).contains(&max_uses) {
        bail!("Numero massimo di utilizzi non valido");
    }
    let invite = load_manageable_invite(pool, actor, invite_id).await?;
    if max_uses < invite.utilizzi.max(1) {
        bail!("Il nuovo limite non può essere inferiore agli utilizzi già effettuati");
    }
    let kind = if max_uses == 1 {
        "monouso"
    } else if max_uses >= UNLIMITED_USES {
        if invite.scade_il.is_some() {
            "scadenza"
        } else {
            "riutilizzabile"
        }
    } else {
        "limite"
    };
    sqlx::query(
        "UPDATE inviti_spazio SET utilizzi_massimi = ?, tipo_invito = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(max_uses)
    .bind(kind)
    .bind(invite.id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare il limite utilizzi")?;
    Ok(())
}

async fn remove_invite_expiry(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
) -> Result<()> {
    let invite = load_manageable_invite(pool, actor, invite_id).await?;
    if invite.tipo_invito == "scadenza" {
        sqlx::query("UPDATE inviti_spazio SET scade_il = NULL, tipo_invito = 'riutilizzabile', utilizzi_massimi = ?, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
            .bind(UNLIMITED_USES).bind(invite.id).execute(pool).await.context("Impossibile rimuovere la scadenza")?;
    } else {
        sqlx::query("UPDATE inviti_spazio SET scade_il = NULL, aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
            .bind(invite.id).execute(pool).await.context("Impossibile rimuovere la scadenza")?;
    }
    Ok(())
}

async fn delete_manageable_invite(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
) -> Result<()> {
    let invite = load_manageable_invite(pool, actor, invite_id).await?;
    sqlx::query("DELETE FROM inviti_spazio WHERE id = ?")
        .bind(invite.id)
        .execute(pool)
        .await
        .context("Impossibile revocare l'invito")?;
    Ok(())
}

async fn load_space_context(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> Result<SpaceContext> {
    let user_id = actor
        .utente_id
        .context("Gestione membri non disponibile per un attore di sistema")?;
    sqlx::query_as::<_, SpaceContext>("SELECT s.id, s.nome, s.tipo, ms.ruolo FROM spazi s JOIN membri_spazio ms ON ms.spazio_id = s.id WHERE s.id = ? AND ms.utente_id = ?")
        .bind(actor.spazio_id).bind(user_id).fetch_optional(pool).await.context("Impossibile leggere lo spazio attivo")?.context("Non appartieni allo spazio attivo")
}

async fn load_manageable_space(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
) -> Result<SpaceContext> {
    let context = load_space_context(pool, actor).await?;
    if context.tipo == "personale" {
        bail!("Lo spazio personale non può avere altri membri");
    }
    if !can_manage_role(&context.ruolo) {
        bail!("Solo proprietario o amministratore possono gestire i membri dello spazio");
    }
    Ok(context)
}

async fn member_by_id(pool: &SqlitePool, space_id: i64, user_id: i64) -> Result<Option<MemberRow>> {
    sqlx::query_as::<_, MemberRow>("SELECT u.id AS user_id, u.nome_visualizzato AS nome, ms.ruolo, (SELECT at.username_snapshot FROM account_telegram at WHERE at.utente_id = u.id ORDER BY at.id LIMIT 1) AS telegram_username, (SELECT at.chat_id FROM account_telegram at WHERE at.utente_id = u.id ORDER BY at.id LIMIT 1) AS chat_id FROM membri_spazio ms JOIN utenti u ON u.id = ms.utente_id WHERE ms.spazio_id = ? AND u.id = ?")
        .bind(space_id).bind(user_id).fetch_optional(pool).await.context("Impossibile leggere il membro")
}

async fn removable_member(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    space_id: i64,
    user_id: i64,
) -> Result<MemberRow> {
    let actor_user_id = actor.utente_id.context("Identità utente non disponibile")?;
    if actor_user_id == user_id {
        bail!("Non puoi modificare o rimuovere te stesso da questa schermata");
    }
    let member = member_by_id(pool, space_id, user_id)
        .await?
        .context("Membro non trovato nello spazio")?;
    if member.ruolo == "proprietario" {
        bail!("Il proprietario dello spazio non può essere modificato o rimosso");
    }
    Ok(member)
}

async fn load_manageable_invite(
    pool: &SqlitePool,
    actor: &identity::AuditActor,
    invite_id: i64,
) -> Result<InviteRow> {
    let context = load_manageable_space(pool, actor).await?;
    cleanup_inactive_invites(pool).await?;
    sqlx::query_as::<_, InviteRow>(&invite_select_sql("i.id = ? AND i.spazio_id = ?", ""))
        .bind(invite_id)
        .bind(context.id)
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere l'invito")?
        .context("Invito non più attivo")
}

async fn load_active_invite_by_token(pool: &SqlitePool, token: &str) -> Result<Option<InviteRow>> {
    sqlx::query_as::<_, InviteRow>(&invite_select_sql("i.token_link = ?", ""))
        .bind(token)
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere l'invito")
}

fn invite_select_sql(where_clause: &str, suffix: &str) -> String {
    format!("SELECT i.id, i.spazio_id, s.nome AS spazio_nome, i.creato_da_utente_id, u.nome_visualizzato AS creatore_nome, i.token_link, i.ruolo_proposto, i.tipo_invito, i.scade_il, CASE WHEN i.scade_il IS NULL THEN NULL ELSE strftime('%d/%m/%Y %H:%M', i.scade_il, 'localtime') END AS scadenza_locale, i.utilizzi_massimi, i.utilizzi, strftime('%d/%m/%Y %H:%M', i.creato_il, 'localtime') AS creazione_locale FROM inviti_spazio i JOIN spazi s ON s.id = i.spazio_id JOIN utenti u ON u.id = i.creato_da_utente_id WHERE {where_clause} AND i.token_link IS NOT NULL AND i.revocato_il IS NULL AND i.utilizzi < i.utilizzi_massimi AND (i.scade_il IS NULL OR julianday(i.scade_il) > julianday('now')) ORDER BY i.id DESC {suffix}")
}

async fn is_member(pool: &SqlitePool, user_id: Option<i64>, space_id: i64) -> Result<bool> {
    let Some(user_id) = user_id else {
        return Ok(false);
    };
    let value: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM membri_spazio WHERE spazio_id = ? AND utente_id = ?",
    )
    .bind(space_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Verifica membership")?;
    Ok(value > 0)
}

async fn invite_link(bot: &Bot, token: &str) -> Result<String> {
    let me = bot
        .get_me()
        .await
        .context("Impossibile leggere username del bot")?;
    let username = me
        .user
        .username
        .as_deref()
        .context("Il bot non ha uno username Telegram")?;
    Ok(format!(
        "https://t.me/{username}?start={INVITE_PREFIX}{token}"
    ))
}

async fn current_local_date(pool: &SqlitePool) -> Result<(i32, u32, u32)> {
    let raw: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%d', 'now', 'localtime')")
        .fetch_one(pool)
        .await
        .context("Lettura data locale")?;
    parse_date(&raw).context("Data locale non valida")
}

async fn local_expiry_to_utc(pool: &SqlitePool, date: &str, time: &str) -> Result<String> {
    if parse_date(date).is_none() || !valid_time(time) {
        bail!("Data o orario non validi");
    }
    let local = format!("{date} {time}:00");
    let utc: Option<String> = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', ?, 'utc')")
        .bind(local)
        .fetch_one(pool)
        .await
        .context("Conversione scadenza")?;
    let utc = utc.context("Scadenza non valida")?;
    let future: i64 =
        sqlx::query_scalar("SELECT CASE WHEN julianday(?) > julianday('now') THEN 1 ELSE 0 END")
            .bind(&utc)
            .fetch_one(pool)
            .await
            .context("Verifica scadenza")?;
    if future != 1 {
        bail!("La scadenza deve essere futura");
    }
    Ok(utc)
}

async fn notify_invite_creator(
    bot: &Bot,
    pool: &SqlitePool,
    invite: &InviteRow,
    member_name: &str,
) {
    let chat: Option<i64> = sqlx::query_scalar(
        "SELECT chat_id FROM account_telegram WHERE utente_id = ? ORDER BY id LIMIT 1",
    )
    .bind(invite.creato_da_utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    if let Some(chat) = chat {
        if let Ok(message) = bot
            .send_message_untracked(
                ChatId(chat),
                format!(
                    "🔔 Hai una nuova notifica\n\n✅ Invito accettato\n\n{} è entrato nello spazio {} come {} {}.",
                    member_name,
                    invite.spazio_nome,
                    role_icon(&invite.ruolo_proposto),
                    role_label(&invite.ruolo_proposto)
                ),
            )
            .reply_markup(notification_keyboard())
            .await
        {
            bot.mark_transient_message(chat, message.id);
        }
    }
}

async fn notify_member_removed(bot: &Bot, member: &MemberRow, space_name: &str) {
    if let Some(chat) = member.chat_id {
        if let Ok(message) = bot
            .send_message_untracked(
                ChatId(chat),
                format!(
                    "🔔 Hai una nuova notifica\n\n🚪 Sei stato rimosso dallo spazio\n\n🏠 {space_name}\n\nIl tuo account e il tuo spazio personale non sono stati modificati."
                ),
            )
            .reply_markup(notification_keyboard())
            .await
        {
            bot.mark_transient_message(chat, message.id);
        }
    }
}

async fn notify_role_changed(
    bot: &Bot,
    member: &MemberRow,
    space_name: &str,
    old_role: &str,
    new_role: &str,
) {
    if let Some(chat) = member.chat_id {
        if let Ok(message) = bot
            .send_message_untracked(
                ChatId(chat),
                format!(
                    "🔔 Hai una nuova notifica\n\n🔄 Il tuo ruolo è stato modificato\n\nSpazio: {space_name}\nPrima: {} {}\nOra: {} {}",
                    role_icon(old_role),
                    role_label(old_role),
                    role_icon(new_role),
                    role_label(new_role)
                ),
            )
            .reply_markup(notification_keyboard())
            .await
        {
            bot.mark_transient_message(chat, message.id);
        }
    }
}

fn notification_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "👥 I miei spazi".to_string(),
            "identity:spaces".to_string(),
        ),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}

fn invite_accept_keyboard(token: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✅ Accetta invito".to_string(),
            format!("space-members:invite:accept:{token}"),
        )],
        vec![InlineKeyboardButton::callback(
            "❌ Rifiuta".to_string(),
            format!("space-members:invite:reject:{token}"),
        )],
    ])
}

fn invite_detail_keyboard(invite: &InviteRow, link: &str) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![InlineKeyboardButton::copy_text_button(
            "📋 Copia link d'invito".to_string(),
            CopyTextButton {
                text: link.to_string(),
            },
        )],
        vec![InlineKeyboardButton::callback(
            "👤 Modifica ruolo".to_string(),
            format!("space-members:invite:edit-role:{}", invite.id),
        )],
        vec![InlineKeyboardButton::callback(
            "🔢 Modifica utilizzi".to_string(),
            format!("space-members:invite:edit-max:{}", invite.id),
        )],
    ];
    if invite.scade_il.is_some() {
        rows.push(vec![
            InlineKeyboardButton::callback(
                "📅 Modifica data".to_string(),
                format!("space-members:invite:edit-date:{}", invite.id),
            ),
            InlineKeyboardButton::callback(
                "🕒 Modifica orario".to_string(),
                format!("space-members:invite:edit-time:{}", invite.id),
            ),
        ]);
        rows.push(vec![InlineKeyboardButton::callback(
            "♾️ Rimuovi scadenza".to_string(),
            format!("space-members:invite:remove-expiry:{}", invite.id),
        )]);
    } else {
        rows.push(vec![InlineKeyboardButton::callback(
            "⏳ Aggiungi scadenza".to_string(),
            format!("space-members:invite:edit-date:{}", invite.id),
        )]);
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "❌ Revoca invito".to_string(),
        format!("space-members:invite:revoke:{}", invite.id),
    )]);
    rows.push(nav_row("⬅️ Inviti attivi", "space-members:invite:list"));
    InlineKeyboardMarkup::new(rows)
}

fn role_keyboard_for_member(user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "👤 Membro".to_string(),
            format!("space-members:set-role:{user_id}:m"),
        )],
        vec![InlineKeyboardButton::callback(
            "👁️ Sola lettura".to_string(),
            format!("space-members:set-role:{user_id}:r"),
        )],
        vec![InlineKeyboardButton::callback(
            "🛡️ Amministratore".to_string(),
            format!("space-members:set-role:{user_id}:a"),
        )],
        nav_row("⬅️ Membro", &format!("space-members:view:{user_id}")),
    ])
}

fn time_keyboard_new(role: &str, date: &str, _current: &str) -> InlineKeyboardMarkup {
    let presets = ["08:00", "12:00", "18:00", "20:00", "21:30", "23:59"];
    let mut rows = Vec::new();
    for chunk in presets.chunks(2) {
        rows.push(
            chunk
                .iter()
                .map(|time| {
                    InlineKeyboardButton::callback(
                        (*time).to_string(),
                        format!("space-members:invite:pick-time:{role}:{date}:{time}"),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
    rows.push(calendar_back_row_new(role, date));
    InlineKeyboardMarkup::new(rows)
}

fn time_keyboard_existing(invite_id: i64, _current: &str) -> InlineKeyboardMarkup {
    let presets = ["08:00", "12:00", "18:00", "20:00", "21:30", "23:59"];
    let mut rows = Vec::new();
    for chunk in presets.chunks(2) {
        rows.push(
            chunk
                .iter()
                .map(|time| {
                    InlineKeyboardButton::callback(
                        (*time).to_string(),
                        format!("space-members:invite:set-time:{invite_id}:{time}"),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
    rows.push(nav_row(
        "⬅️ Invito",
        &format!("space-members:invite:view:{invite_id}"),
    ));
    InlineKeyboardMarkup::new(rows)
}

fn calendar_back_row_new(role: &str, date: &str) -> Vec<InlineKeyboardButton> {
    let callback = parse_date(date)
        .map(|(year, month, _)| {
            format!(
                "space-members:invite:cal-nav:{}:{year}:{month:02}",
                role_code(role)
            )
        })
        .unwrap_or_else(|| format!("space-members:invite:type:{}:exp", role_code(role)));
    vec![
        InlineKeyboardButton::callback("⬅️ Giorno".to_string(), callback),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]
}

fn calendar_back_keyboard_new(role: &str, date: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![calendar_back_row_new(role, date)])
}

fn manual_time_back_keyboard(target: &ManualTimeTarget) -> InlineKeyboardMarkup {
    let back = match target {
        ManualTimeTarget::New { role, date } => parse_date(date)
            .map(|(year, month, _)| {
                format!(
                    "space-members:invite:cal-nav:{}:{year}:{month:02}",
                    role_code(role)
                )
            })
            .unwrap_or_else(|| format!("space-members:invite:type:{}:exp", role_code(role))),
        ManualTimeTarget::Existing { invite_id } => {
            format!("space-members:invite:view:{invite_id}")
        }
        ManualTimeTarget::NewMaxUses { role } => {
            format!("space-members:invite:role:{}", role_code(role))
        }
        ManualTimeTarget::ExistingMaxUses { invite_id } => {
            format!("space-members:invite:view:{invite_id}")
        }
    };
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("⬅️ Indietro".to_string(), back),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}

fn max_uses_back_keyboard_new(role: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![nav_row(
        "⬅️ Tipo invito",
        &format!("space-members:invite:role:{}", role_code(role)),
    )])
}

fn max_uses_back_keyboard_existing(invite_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![nav_row(
        "⬅️ Invito",
        &format!("space-members:invite:view:{invite_id}"),
    )])
}

fn existing_max_uses_keyboard(invite_id: i64) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![1_i64, 2, 5, 10]
            .into_iter()
            .map(|value| {
                InlineKeyboardButton::callback(
                    value.to_string(),
                    format!("space-members:invite:set-max:{invite_id}:{value}"),
                )
            })
            .collect::<Vec<_>>(),
        vec![InlineKeyboardButton::callback(
            "20 utilizzi".to_string(),
            format!("space-members:invite:set-max:{invite_id}:20"),
        )],
        vec![InlineKeyboardButton::callback(
            "♾️ Illimitati".to_string(),
            format!("space-members:invite:set-max:{invite_id}:{UNLIMITED_USES}"),
        )],
    ];
    rows.push(nav_row(
        "⬅️ Invito",
        &format!("space-members:invite:view:{invite_id}"),
    ));
    InlineKeyboardMarkup::new(rows)
}

fn calendar_rows(
    year: i32,
    month: u32,
    role: &str,
    invite_id: Option<i64>,
    current: (i32, u32, u32),
) -> Vec<Vec<InlineKeyboardButton>> {
    // La griglia del mese e le regole sulle date stanno in `modules::calendario`.
    // Qui restava una seconda implementazione del calendario gregoriano —
    // congruenza di Zeller, bisestili, giorni del mese — scritta a mano accanto
    // a quella basata su `chrono` del planner.
    let nav_prefix = if let Some(id) = invite_id {
        format!("space-members:invite:cal-nav:{id}")
    } else {
        format!("space-members:invite:cal-nav:{}", role_code(role))
    };
    let oggi = format!("{:04}-{:02}-{:02}", current.0, current.1, current.2);

    // Un invito puo' scadere solo nel futuro: le date passate restano visibili,
    // marcate, ma non premibili.
    let giorno = |data: &str| {
        if data < oggi.as_str() {
            calendario::Giorno {
                stato: calendario::GiornoStato::Bloccato,
                marcatore: Some("❌"),
            }
        } else {
            calendario::Giorno::default()
        }
    };
    let callback_giorno = |data: &str| {
        if let Some(id) = invite_id {
            format!("space-members:invite:set-date:{id}:{data}")
        } else {
            format!("space-members:invite:date:{}:{data}", role_code(role))
        }
    };
    let callback_mese = |anno: i32, mese: u32| format!("{nav_prefix}:{anno}:{mese:02}");

    let config = calendario::Calendario {
        year,
        month,
        oggi: &oggi,
        callback_giorno: &callback_giorno,
        callback_mese: &callback_mese,
        callback_inerte: "space-members:noop",
        giorno: &giorno,
        mese_minimo: Some((current.0, current.1)),
    };
    let mut rows = calendario::righe(&config);

    let back_callback = if let Some(id) = invite_id {
        format!("space-members:invite:view:{id}")
    } else {
        format!("space-members:invite:role:{}", role_code(role))
    };
    rows.push(nav_row("⬅️ Indietro", &back_callback));
    rows
}

fn open_space_keyboard(spazio_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "👥 Apri spazio".to_string(),
            format!("identity:space:{spazio_id}"),
        ),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}

fn already_member_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "👥 I miei spazi".to_string(),
            "identity:spaces".to_string(),
        ),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}

fn nav_row(back_label: &str, back_callback: &str) -> Vec<InlineKeyboardButton> {
    vec![
        InlineKeyboardButton::callback(back_label.to_string(), back_callback.to_string()),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]
}

fn spaces_back_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("⬅️ Spazi".to_string(), "identity:spaces".to_string()),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}
fn back_to_members_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("⬅️ Membri".to_string(), "space-members:menu".to_string()),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}
fn back_to_invites_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "⬅️ Inviti attivi".to_string(),
            "space-members:invite:list".to_string(),
        ),
        InlineKeyboardButton::callback("🏠 Menù principale".to_string(), "menu:main".to_string()),
    ]])
}

fn invite_validity_line(invite: &InviteRow) -> String {
    let base = match invite.tipo_invito.as_str() {
        "monouso" => "Validità: 1 utilizzo".to_string(),
        "riutilizzabile" => "Validità: utilizzi illimitati".to_string(),
        "limite" => format!("Validità: massimo {} utilizzi", invite.utilizzi_massimi),
        "scadenza" => "Validità: utilizzi illimitati".to_string(),
        _ => "Validità: configurazione precedente".to_string(),
    };
    match invite.scadenza_locale.as_deref() {
        Some(value) => format!("{base}\nValido fino al: {}", human_local_expiry(value)),
        None => format!("{base}\nScadenza: nessuna"),
    }
}
fn human_local_expiry(value: &str) -> String {
    let Some((date, time)) = value.split_once(' ') else {
        return value.to_string();
    };
    let mut parts = date.split('/');
    let day = parts.next().and_then(|v| v.parse::<u32>().ok());
    let month = parts.next().and_then(|v| v.parse::<u32>().ok());
    let year = parts.next().and_then(|v| v.parse::<i32>().ok());
    if parts.next().is_some() {
        return value.to_string();
    }
    match (day, month, year) {
        (Some(day), Some(month), Some(year)) if (1..=12).contains(&month) => {
            format!(
                "{} {} {}, {}",
                day,
                calendario::month_name(month).to_lowercase(),
                year,
                time
            )
        }
        _ => value.to_string(),
    }
}

fn invite_usage_line(invite: &InviteRow) -> String {
    if invite.utilizzi_massimi >= UNLIMITED_USES {
        invite.utilizzi.to_string()
    } else {
        format!("{}/{}", invite.utilizzi, invite.utilizzi_massimi)
    }
}
fn invite_type_label(value: &str) -> &'static str {
    match value {
        "monouso" => "1️⃣ Monouso",
        "riutilizzabile" => "♾️ Riutilizzabile",
        "limite" => "🔢 Limite utilizzi",
        "scadenza" => "⏳ Con scadenza",
        _ => "Invito",
    }
}
fn invite_type_short(value: &str) -> &'static str {
    match value {
        "monouso" => "Monouso",
        "riutilizzabile" => "Riutilizzabile",
        "limite" => "A utilizzi",
        "scadenza" => "Con scadenza",
        _ => "Invito",
    }
}
fn can_manage_role(role: &str) -> bool {
    matches!(role, "proprietario" | "amministratore")
}
fn role_icon(role: &str) -> &'static str {
    match role {
        "proprietario" => "👑",
        "amministratore" => "🛡️",
        "membro" => "👤",
        "lettura" => "👁️",
        _ => "👤",
    }
}
fn role_label(role: &str) -> &'static str {
    match role {
        "proprietario" => "Proprietario",
        "amministratore" => "Amministratore",
        "membro" => "Membro",
        "lettura" => "Sola lettura",
        _ => "Membro",
    }
}
fn role_code(role: &str) -> &'static str {
    match role {
        "amministratore" | "a" => "a",
        "lettura" | "r" => "r",
        _ => "m",
    }
}
fn role_from_code(code: &str) -> Option<&'static str> {
    match code {
        "a" => Some("amministratore"),
        "m" => Some("membro"),
        "r" => Some("lettura"),
        _ => None,
    }
}
fn parse_page(data: &str, prefix: &str) -> Option<i64> {
    data.strip_prefix(prefix)?
        .parse::<i64>()
        .ok()
        .filter(|v| *v >= 0)
}
fn parse_positive(data: &str, prefix: &str) -> Option<i64> {
    data.strip_prefix(prefix)?
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
}
fn valid_token(token: &str) -> bool {
    token.len() == 24 && token.chars().all(|c| c.is_ascii_hexdigit())
}
fn valid_time(time: &str) -> bool {
    let Some((h, m)) = time.split_once(':') else {
        return false;
    };
    matches!((h.parse::<u32>(), m.parse::<u32>()), (Ok(h), Ok(m)) if h < 24 && m < 60)
}

fn valid_time_strict(time: &str) -> bool {
    time.len() == 5
        && time.as_bytes().get(2) == Some(&b':')
        && valid_time(time)
        && time
            .chars()
            .enumerate()
            .all(|(index, ch)| index == 2 || ch.is_ascii_digit())
}

fn human_created_at(value: &str) -> String {
    human_local_expiry(value)
}
fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut p = value.split('-');
    let y = p.next()?.parse().ok()?;
    let m = p.next()?.parse().ok()?;
    let d = p.next()?.parse().ok()?;
    if p.next().is_some() || !(1..=12).contains(&m) || d < 1 || d > calendario::days_in_month(y, m)
    {
        None
    } else {
        Some((y, m, d))
    }
}
fn human_date(value: &str) -> String {
    parse_date(value)
        .map(|(y, m, d)| format!("{} {} {}", d, calendario::month_name(m).to_lowercase(), y))
        .unwrap_or_else(|| value.to_string())
}
fn parse_invite_role(data: &str, prefix: &str) -> Option<&'static str> {
    role_from_code(data.strip_prefix(prefix)?)
}
fn parse_invite_kind(data: &str) -> Option<(&'static str, &str)> {
    let rest = data.strip_prefix("space-members:invite:type:")?;
    let mut p = rest.split(':');
    let role = role_from_code(p.next()?)?;
    let kind = p.next()?;
    if p.next().is_none() && matches!(kind, "one" | "free" | "max" | "exp") {
        Some((role, kind))
    } else {
        None
    }
}
fn parse_invite_max(data: &str) -> Option<(&'static str, i64)> {
    let rest = data.strip_prefix("space-members:invite:max:")?;
    let mut p = rest.split(':');
    let role = role_from_code(p.next()?)?;
    let n = p.next()?.parse().ok()?;
    if p.next().is_none() && matches!(n, 2 | 5 | 10 | 20) {
        Some((role, n))
    } else {
        None
    }
}
fn parse_new_calendar_nav(data: &str) -> Option<(&'static str, i32, u32)> {
    let rest = data.strip_prefix("space-members:invite:cal-nav:")?;
    let mut p = rest.split(':');
    let first = p.next()?;
    if first.parse::<i64>().is_ok() {
        return None;
    }
    let role = role_from_code(first)?;
    let y = p.next()?.parse().ok()?;
    let m = p.next()?.parse().ok()?;
    if p.next().is_none() {
        Some((role, y, m))
    } else {
        None
    }
}
fn parse_existing_calendar_nav(data: &str) -> Option<(i64, i32, u32)> {
    let rest = data.strip_prefix("space-members:invite:cal-nav:")?;
    let mut p = rest.split(':');
    let id = p.next()?.parse().ok()?;
    let y = p.next()?.parse().ok()?;
    let m = p.next()?.parse().ok()?;
    if p.next().is_none() {
        Some((id, y, m))
    } else {
        None
    }
}
fn parse_new_expiry_date(data: &str) -> Option<(&'static str, String)> {
    let rest = data.strip_prefix("space-members:invite:date:")?;
    let mut p = rest.split(':');
    let role = role_from_code(p.next()?)?;
    let date = p.next()?.to_string();
    if p.next().is_none() && parse_date(&date).is_some() {
        Some((role, date))
    } else {
        None
    }
}
fn parse_new_time_pick(data: &str) -> Option<(&'static str, String, String)> {
    let rest = data.strip_prefix("space-members:invite:pick-time:")?;
    let mut p = rest.split(':');
    let role = role_from_code(p.next()?)?;
    let date = p.next()?.to_string();
    let hour = p.next()?;
    let minute = p.next()?;
    if p.next().is_none() {
        Some((role, date, format!("{hour}:{minute}")))
    } else {
        None
    }
}
fn parse_new_expiry_time(data: &str) -> Option<(&'static str, String, String)> {
    let rest = data.strip_prefix("space-members:invite:confirm-time:")?;
    let mut p = rest.split(':');
    let role = role_from_code(p.next()?)?;
    let date = p.next()?.to_string();
    let hour = p.next()?;
    let minute = p.next()?;
    if p.next().is_none() {
        Some((role, date, format!("{hour}:{minute}")))
    } else {
        None
    }
}
fn parse_existing_expiry_date(data: &str) -> Option<(i64, String)> {
    let rest = data.strip_prefix("space-members:invite:set-date:")?;
    let (id, date) = rest.split_once(':')?;
    let id = id.parse().ok()?;
    if parse_date(date).is_some() {
        Some((id, date.to_string()))
    } else {
        None
    }
}
fn parse_existing_expiry_time(data: &str) -> Option<(i64, String)> {
    let rest = data.strip_prefix("space-members:invite:set-time:")?;
    let mut p = rest.split(':');
    let id = p.next()?.parse().ok()?;
    let h = p.next()?;
    let m = p.next()?;
    if p.next().is_none() {
        Some((id, format!("{h}:{m}")))
    } else {
        None
    }
}
fn parse_manual_time_new(data: &str) -> Option<(&'static str, String)> {
    let rest = data.strip_prefix("space-members:invite:manual-time-new:")?;
    let mut parts = rest.split(':');
    let role = role_from_code(parts.next()?)?;
    let date = parts.next()?.to_string();
    if parts.next().is_none() && parse_date(&date).is_some() {
        Some((role, date))
    } else {
        None
    }
}

fn parse_existing_max_uses(data: &str) -> Option<(i64, i64)> {
    let rest = data.strip_prefix("space-members:invite:set-max:")?;
    let (id, max_uses) = rest.split_once(':')?;
    let id = id.parse().ok()?;
    let max_uses = max_uses.parse().ok()?;
    if matches!(max_uses, 1 | 2 | 5 | 10 | 20) || max_uses == UNLIMITED_USES {
        Some((id, max_uses))
    } else {
        None
    }
}

fn parse_member_role_set(data: &str) -> Option<(i64, &'static str)> {
    let rest = data.strip_prefix("space-members:set-role:")?;
    let (id, role) = rest.split_once(':')?;
    Some((id.parse().ok()?, role_from_code(role)?))
}
fn parse_invite_set_role(data: &str) -> Option<(i64, &'static str)> {
    let rest = data.strip_prefix("space-members:invite:set-role:")?;
    let (id, role) = rest.split_once(':')?;
    Some((id.parse().ok()?, role_from_code(role)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn actor(user_id: i64, space_id: i64, name: &str) -> identity::AuditActor {
        identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: name.to_string(),
            spazio_id: space_id,
            spazio_nome_snapshot: "Condiviso".to_string(),
            view_all: false,
            origine: "telegram",
            telegram_user_id: Some(user_id),
            telegram_username: None,
        }
    }
    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }
    async fn insert_user(pool: &SqlitePool, name: &str, telegram_id: i64) -> i64 {
        let id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_rowid();
        sqlx::query("INSERT INTO account_telegram (utente_id, telegram_user_id, chat_id, nome_snapshot) VALUES (?, ?, ?, ?)").bind(id).bind(telegram_id).bind(telegram_id).bind(name).execute(pool).await.unwrap();
        id
    }
    async fn shared_space(pool: &SqlitePool, owner: i64) -> i64 {
        let id=sqlx::query("INSERT INTO spazi (nome, tipo, creato_da_utente_id) VALUES ('Condiviso', 'condiviso', ?)").bind(owner).execute(pool).await.unwrap().last_insert_rowid();
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
        )
        .bind(id)
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    #[tokio::test]
    async fn invito_monouso_si_elimina_dopo_accettazione() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 1001).await;
        let guest = insert_user(&p, "Guest", 1002).await;
        let space = shared_space(&p, owner).await;
        let a = actor(owner, space, "Owner");
        let invite = create_invite(&p, &a, "membro", "monouso", 1, None)
            .await
            .unwrap();
        let ga = actor(guest, 1, "Guest");
        identity::with_actor(ga.clone(), async {
            accept_invite(&p, &ga, &invite.token_link).await
        })
        .await
        .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inviti_spazio WHERE id = ?")
            .bind(invite.id)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn invito_a_limite_scompare_al_limite() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 2001).await;
        let g1 = insert_user(&p, "G1", 2002).await;
        let g2 = insert_user(&p, "G2", 2003).await;
        let space = shared_space(&p, owner).await;
        let a = actor(owner, space, "Owner");
        let invite = create_invite(&p, &a, "lettura", "limite", 2, None)
            .await
            .unwrap();
        for (id, name) in [(g1, "G1"), (g2, "G2")] {
            let ga = actor(id, 1, name);
            identity::with_actor(ga.clone(), async {
                accept_invite(&p, &ga, &invite.token_link).await
            })
            .await
            .unwrap();
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inviti_spazio WHERE id = ?")
            .bind(invite.id)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn ruolo_membro_si_puo_modificare() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 3001).await;
        let guest = insert_user(&p, "Guest", 3002).await;
        let space = shared_space(&p, owner).await;
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'membro')",
        )
        .bind(space)
        .bind(guest)
        .execute(&p)
        .await
        .unwrap();
        let a = actor(owner, space, "Owner");
        let (updated, old) = update_member_role(&p, &a, guest, "lettura").await.unwrap();
        assert_eq!(old, "membro");
        assert_eq!(updated.ruolo, "lettura");
    }

    #[tokio::test]
    async fn cleanup_elimina_invito_scaduto() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 4001).await;
        let space = shared_space(&p, owner).await;
        let a = actor(owner, space, "Owner");
        let invite = create_invite(
            &p,
            &a,
            "membro",
            "scadenza",
            UNLIMITED_USES,
            Some("2099-01-01T00:00:00Z"),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE inviti_spazio SET scade_il = '2000-01-01T00:00:00Z' WHERE id = ?")
            .bind(invite.id)
            .execute(&p)
            .await
            .unwrap();
        let removed = cleanup_inactive_invites(&p).await.unwrap();
        assert!(removed >= 1);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inviti_spazio WHERE id = ?")
            .bind(invite.id)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn spazio_personale_non_puo_generare_inviti() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 5001).await;
        let personal=sqlx::query("INSERT INTO spazi (nome, tipo, creato_da_utente_id) VALUES ('Personale test', 'personale', ?)").bind(owner).execute(&p).await.unwrap().last_insert_rowid();
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
        )
        .bind(personal)
        .bind(owner)
        .execute(&p)
        .await
        .unwrap();
        let a = actor(owner, personal, "Owner");
        assert!(create_invite(&p, &a, "membro", "monouso", 1, None)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn creatore_che_apre_il_proprio_invito_non_consuma_utilizzi() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 5501).await;
        let space = shared_space(&p, owner).await;
        let a = actor(owner, space, "Owner");
        let invite = create_invite(&p, &a, "membro", "monouso", 1, None)
            .await
            .unwrap();

        let result = identity::with_actor(a.clone(), async {
            accept_invite(&p, &a, &invite.token_link).await
        })
        .await
        .unwrap();

        assert!(matches!(result, AcceptResult::AlreadyMember { .. }));
        let uses: i64 = sqlx::query_scalar("SELECT utilizzi FROM inviti_spazio WHERE id = ?")
            .bind(invite.id)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(uses, 0);
    }

    #[tokio::test]
    async fn invito_riutilizzabile_resta_dopo_un_utilizzo() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 6001).await;
        let guest = insert_user(&p, "Guest", 6002).await;
        let space = shared_space(&p, owner).await;
        let a = actor(owner, space, "Owner");
        let invite = create_invite(&p, &a, "membro", "riutilizzabile", UNLIMITED_USES, None)
            .await
            .unwrap();
        let ga = actor(guest, 1, "Guest");
        identity::with_actor(ga.clone(), async {
            accept_invite(&p, &ga, &invite.token_link).await
        })
        .await
        .unwrap();
        let uses: i64 = sqlx::query_scalar("SELECT utilizzi FROM inviti_spazio WHERE id = ?")
            .bind(invite.id)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(uses, 1);
    }

    #[tokio::test]
    async fn scadenza_si_puo_modificare_sullo_stesso_invito() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 7001).await;
        let space = shared_space(&p, owner).await;
        let a = actor(owner, space, "Owner");
        let invite = create_invite(&p, &a, "membro", "monouso", 1, None)
            .await
            .unwrap();
        update_invite_date(&p, &a, invite.id, "2099-09-13")
            .await
            .unwrap();
        update_invite_time(&p, &a, invite.id, "21:37")
            .await
            .unwrap();
        let updated = load_manageable_invite(&p, &a, invite.id).await.unwrap();
        assert_eq!(updated.tipo_invito, "monouso");
        assert!(updated.scade_il.is_some());
        assert_eq!(updated.token_link, invite.token_link);
    }

    #[test]
    fn orario_manuale_richiede_formato_24h_esatto() {
        assert!(valid_time_strict("12:43"));
        assert!(valid_time_strict("00:00"));
        assert!(valid_time_strict("23:59"));
        assert!(!valid_time_strict("2:43"));
        assert!(!valid_time_strict("24:00"));
        assert!(!valid_time_strict("12:60"));
        assert!(!valid_time_strict("12.43"));
    }

    #[tokio::test]
    async fn modifica_limite_aggiorna_tipo_invito() {
        let p = pool().await;
        let owner = insert_user(&p, "Owner", 6001).await;
        let space = shared_space(&p, owner).await;
        let a = actor(owner, space, "Owner");
        let invite = create_invite(&p, &a, "membro", "riutilizzabile", UNLIMITED_USES, None)
            .await
            .unwrap();
        update_invite_max_uses(&p, &a, invite.id, 1).await.unwrap();
        let updated = load_manageable_invite(&p, &a, invite.id).await.unwrap();
        assert_eq!(updated.tipo_invito, "monouso");
        assert_eq!(updated.utilizzi_massimi, 1);
    }

    #[test]
    fn limite_utilizzi_personalizzato_accetta_valori_fuori_dai_preset() {
        assert!((1..=9_999).contains(&37));
        assert!((1..=9_999).contains(&250));
        assert!(!(1..=9_999).contains(&0));
        assert!(!(1..=9_999).contains(&10_000));
    }

    #[test]
    fn calendario_marca_chiaramente_le_date_passate() {
        let rows = calendar_rows(2026, 8, "membro", None, (2026, 8, 28));
        assert!(rows.iter().flatten().any(|button| button.text == "27 ❌"));
        assert!(!rows.iter().flatten().any(|button| button.text == "28 ❌"));
    }

    #[test]
    fn callback_inviti_restano_sotto_limite_telegram() {
        let samples = [
            format!("space-members:invite:accept:{}", "a".repeat(24)),
            format!("space-members:invite:set-time:{}:23:59", i64::MAX),
            format!("space-members:invite:set-date:{}:2026-09-13", i64::MAX),
        ];
        assert!(samples.iter().all(|v| v.len() <= 64));
    }

    #[test]
    fn calendario_gregoriano_base() {
        assert_eq!(calendario::days_in_month(2028, 2), 29);
        assert_eq!(calendario::weekday_monday_zero(2026, 9, 13), 6);
        assert_eq!(calendario::shift_month(2026, 1, -1), (2025, 12));
    }
}
