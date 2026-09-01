//! Step 7.2I.2A - Ingredienti personalizzati per profilo.
//!
//! Ripartenza pulita: nessuna modifica al dispatcher principale.
//! Tutti i callback restano nel namespace `foodprof:*`.

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

const PAGE_SIZE: i64 = 5;

#[derive(Debug, Clone, FromRow)]
struct IngredientRow {
    id: i64,
    name: String,
    recipe_quantity: f64,
    unit: String,
    servings: i64,
    factor: f64,
    override_kind: Option<String>,
    override_quantity: Option<f64>,
}

#[derive(Debug, Clone)]
struct IngredientView {
    row: IngredientRow,
    calculated_quantity: f64,
    final_quantity: Option<f64>,
}

#[derive(Debug)]
struct OverrideHistoryContext {
    profile_id: i64,
    profile_name: String,
    recipe_name: String,
    ingredient_name: String,
    unit: String,
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    if let Some((profile_id, recipe_id, page)) = parse_list_callback(data) {
        show_list(bot, chat_id, pool, profile_id, recipe_id, page).await?;
        return Ok(true);
    }

    if let Some((profile_id, recipe_id, ingredient_id)) =
        parse_detail_callback(data, "foodprof:ing:v:")
    {
        show_detail(bot, chat_id, pool, profile_id, recipe_id, ingredient_id).await?;
        return Ok(true);
    }

    if let Some((profile_id, recipe_id, ingredient_id)) =
        parse_detail_callback(data, "foodprof:ing:x:")
    {
        match set_excluded(pool, profile_id, recipe_id, ingredient_id).await {
            Ok(changed) => {
                let page = ingredient_page(pool, recipe_id, ingredient_id)
                    .await
                    .unwrap_or(0);
                let ingredient_name = load_one(pool, profile_id, recipe_id, ingredient_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|view| view.row.name)
                    .unwrap_or_else(|| "Ingrediente".to_string());
                let notice = if changed {
                    format!("✅ {ingredient_name} escluso per questo profilo.")
                } else {
                    format!("ℹ️ {ingredient_name} era già escluso.")
                };
                show_list_with_notice(
                    bot,
                    chat_id,
                    pool,
                    profile_id,
                    recipe_id,
                    page,
                    Some(&notice),
                )
                .await?;
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    profile_id,
                    recipe_id,
                    ingredient_id,
                    "Esclusione ingrediente rifiutata"
                );
                send_error(bot, chat_id, profile_id, recipe_id, &error.to_string()).await?;
            }
        }
        return Ok(true);
    }

    if let Some((profile_id, recipe_id, ingredient_id)) =
        parse_detail_callback(data, "foodprof:ing:r:")
    {
        match reset_override(pool, profile_id, recipe_id, ingredient_id).await {
            Ok(changed) => {
                let page = ingredient_page(pool, recipe_id, ingredient_id)
                    .await
                    .unwrap_or(0);
                let ingredient_name = load_one(pool, profile_id, recipe_id, ingredient_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|view| view.row.name)
                    .unwrap_or_else(|| "Ingrediente".to_string());
                let notice = if changed {
                    format!("✅ {ingredient_name} ripristinato alla quantità calcolata.")
                } else {
                    format!("ℹ️ {ingredient_name} usa già la quantità calcolata.")
                };
                show_list_with_notice(
                    bot,
                    chat_id,
                    pool,
                    profile_id,
                    recipe_id,
                    page,
                    Some(&notice),
                )
                .await?;
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    profile_id,
                    recipe_id,
                    ingredient_id,
                    "Reset override rifiutato"
                );
                send_error(bot, chat_id, profile_id, recipe_id, &error.to_string()).await?;
            }
        }
        return Ok(true);
    }

    bot.send_message(chat_id, "⚠️ Azione ingrediente non valida.")
        .reply_markup(main_return_keyboard())
        .await?;
    Ok(true)
}

pub async fn handle_quantity_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
    text: &str,
) -> ResponseResult<()> {
    let Some(quantity) = parse_quantity(text) else {
        bot.send_message(
            msg.chat.id,
            "⚠️ Scrivi una quantità positiva.\n\nEsempi: 90 oppure 90,5",
        )
        .reply_markup(detail_keyboard(profile_id, recipe_id, ingredient_id, true))
        .await?;
        return Ok(());
    };

    match set_quantity(pool, profile_id, recipe_id, ingredient_id, quantity).await {
        Ok(changed) => {
            let updated = match load_one(pool, profile_id, recipe_id, ingredient_id).await {
                Ok(Some(view)) => view,
                Ok(None) => {
                    bot.send_message(msg.chat.id, "⚠️ Ingrediente non più disponibile.")
                        .reply_markup(recipe_return_keyboard(profile_id, recipe_id))
                        .await?;
                    return Ok(());
                }
                Err(error) => {
                    tracing::error!(
                        ?error,
                        profile_id,
                        recipe_id,
                        ingredient_id,
                        "Errore rilettura ingrediente dopo override"
                    );
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Quantità salvata, ma non riesco a rileggere l'ingrediente.",
                    )
                    .reply_markup(recipe_return_keyboard(profile_id, recipe_id))
                    .await?;
                    return Ok(());
                }
            };
            let page = ingredient_page(pool, recipe_id, ingredient_id)
                .await
                .unwrap_or(0);
            let message = if changed {
                format!(
                    "✅ Quantità aggiornata per {}: {} {}.",
                    updated.row.name,
                    format_quantity(quantity),
                    updated.row.unit
                )
            } else {
                format!(
                    "ℹ️ {} usa già {} {}.",
                    updated.row.name,
                    format_quantity(quantity),
                    updated.row.unit
                )
            };
            show_list_with_notice(
                bot,
                msg.chat.id,
                pool,
                profile_id,
                recipe_id,
                page,
                Some(&message),
            )
            .await?;
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                profile_id,
                recipe_id,
                ingredient_id,
                quantity,
                "Quantità ingrediente rifiutata"
            );
            send_error(bot, msg.chat.id, profile_id, recipe_id, &error.to_string()).await?;
        }
    }
    Ok(())
}

pub fn input_context_from_callback(data: &str) -> Option<(i64, i64, i64)> {
    parse_detail_callback(data, "foodprof:ing:v:")
}

pub fn quantity_input_is_valid(text: &str) -> bool {
    parse_quantity(text).is_some()
}

async fn show_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    requested_page: i64,
) -> ResponseResult<()> {
    show_list_with_notice(
        bot,
        chat_id,
        pool,
        profile_id,
        recipe_id,
        requested_page,
        None,
    )
    .await
}

async fn show_list_with_notice(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    requested_page: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let context = match context_names(pool, profile_id, recipe_id).await {
        Ok(Some(context)) => context,
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Profilo o ricetta non disponibili.")
                .reply_markup(main_return_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, profile_id, recipe_id, "Errore contesto ingredienti");
            bot.send_message(chat_id, "⚠️ Non riesco a verificare profilo e ricetta.")
                .reply_markup(main_return_keyboard())
                .await?;
            return Ok(());
        }
    };

    match load_page(pool, profile_id, recipe_id, requested_page).await {
        Ok((items, total, page)) => {
            let pages = page_count(total);
            let text = format!(
                "{}🥕 Ingredienti personalizzati\n\n👤 Profilo: {}\n🍳 Ricetta: {}\n\nScegli un ingrediente.\n\nTotale: {total}\nPagina {}/{}",
                notice
                    .map(|value| format!("{value}\n\n"))
                    .unwrap_or_default(),
                context.0,
                context.1,
                page + 1,
                pages
            );
            bot.send_message(chat_id, text)
                .reply_markup(list_keyboard(profile_id, recipe_id, &items, page, pages))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, profile_id, recipe_id, "Errore lista ingredienti");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere gli ingredienti.")
                .reply_markup(recipe_return_keyboard(profile_id, recipe_id))
                .await?;
        }
    }
    Ok(())
}

async fn show_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
) -> ResponseResult<()> {
    let context = match context_names(pool, profile_id, recipe_id).await {
        Ok(Some(context)) => context,
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Profilo o ricetta non disponibili.")
                .reply_markup(main_return_keyboard())
                .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, profile_id, recipe_id, "Errore contesto ingrediente");
            bot.send_message(chat_id, "⚠️ Non riesco a verificare profilo e ricetta.")
                .reply_markup(main_return_keyboard())
                .await?;
            return Ok(());
        }
    };

    match load_one(pool, profile_id, recipe_id, ingredient_id).await {
        Ok(Some(view)) => {
            let calculated = format!(
                "{} {}",
                format_quantity(view.calculated_quantity),
                view.row.unit
            );
            let (final_value, mode) = match (view.row.override_kind.as_deref(), view.final_quantity)
            {
                (Some("escluso"), None) => ("ingrediente escluso".to_string(), "escluso"),
                (Some("quantita"), Some(quantity)) => (
                    format!("{} {}", format_quantity(quantity), view.row.unit),
                    "quantità personalizzata",
                ),
                _ => (calculated.clone(), "quantità calcolata"),
            };

            bot.send_message(
                chat_id,
                format!(
                    "🥕 Ingrediente personalizzato\n\n👤 Profilo: {}\n🍳 Ricetta: {}\n🥕 Ingrediente: {}\n\n📐 Quantità calcolata: {calculated}\n🎯 Quantità finale: {final_value}\n📌 Modalità: {mode}\n\n⌨️ Scrivi direttamente una quantità, ad esempio 90 oppure 90,5.",
                    context.0, context.1, view.row.name
                ),
            )
            .reply_markup(detail_keyboard(
                profile_id,
                recipe_id,
                ingredient_id,
                view.row.override_kind.is_some(),
            ))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Ingrediente non disponibile.")
                .reply_markup(recipe_return_keyboard(profile_id, recipe_id))
                .await?;
        }
        Err(error) => {
            tracing::error!(
                ?error,
                profile_id,
                recipe_id,
                ingredient_id,
                "Errore dettaglio ingrediente"
            );
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questo ingrediente.")
                .reply_markup(recipe_return_keyboard(profile_id, recipe_id))
                .await?;
        }
    }
    Ok(())
}

async fn context_names(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
) -> Result<Option<(String, String)>> {
    let user_id = current_user_id()?;
    let profile_name: Option<String> = sqlx::query_scalar(
        "SELECT nome FROM profili_alimentari \
         WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile verificare il profilo")?;

    let Some(profile_name) = profile_name else {
        return Ok(None);
    };

    if !recipe_is_visible(pool, recipe_id, user_id).await? {
        return Ok(None);
    }

    let recipe_name: Option<String> =
        sqlx::query_scalar("SELECT nome FROM ricette WHERE id = ? AND archiviata = 0")
            .bind(recipe_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere la ricetta")?;

    Ok(recipe_name.map(|name| (profile_name, name)))
}

async fn recipe_is_visible(pool: &SqlitePool, recipe_id: i64, user_id: i64) -> Result<bool> {
    let actor = identity::current_actor();
    let visible = if actor.view_all {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ricette r WHERE r.id = ? AND r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS( \
                SELECT 1 FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
                WHERE rs.ricetta_id = r.id AND ms.utente_id = ?)))",
        )
        .bind(recipe_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .context("Impossibile verificare la visibilità della ricetta")?
    } else {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ricette r WHERE r.id = ? AND r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS( \
                SELECT 1 FROM ricetta_spazi rs WHERE rs.ricetta_id = r.id AND rs.spazio_id = ?)))",
        )
        .bind(recipe_id)
        .bind(user_id)
        .bind(actor.spazio_id)
        .fetch_one(pool)
        .await
        .context("Impossibile verificare la visibilità della ricetta")?
    };
    Ok(visible)
}

async fn load_page(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    requested_page: i64,
) -> Result<(Vec<IngredientView>, i64, i64)> {
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ricetta_ingredienti WHERE ricetta_id = ?")
            .bind(recipe_id)
            .fetch_one(pool)
            .await
            .context("Impossibile contare gli ingredienti")?;

    let pages = page_count(total);
    let page = requested_page.max(0).min(pages.saturating_sub(1));
    let rows = sqlx::query_as::<_, IngredientRow>(
        "SELECT ri.id, a.nome AS name, ri.quantita AS recipe_quantity, \
                um.simbolo AS unit, r.porzioni_base AS servings, \
                COALESCE(prp.fattore_porzione, 1.0) AS factor, \
                o.tipo_override AS override_kind, o.quantita_override AS override_quantity \
         FROM ricetta_ingredienti ri \
         JOIN ricette r ON r.id = ri.ricetta_id \
         JOIN alimenti a ON a.id = ri.alimento_id \
         JOIN unita_misura um ON um.id = ri.unita_misura_id \
         LEFT JOIN profilo_ricetta_porzioni prp \
           ON prp.profilo_alimentare_id = ? AND prp.ricetta_id = r.id \
         LEFT JOIN profilo_ricetta_ingredienti_override o \
           ON o.profilo_alimentare_id = ? AND o.ricetta_ingrediente_id = ri.id \
         WHERE ri.ricetta_id = ? \
         ORDER BY ri.ordinamento, ri.id LIMIT ? OFFSET ?",
    )
    .bind(profile_id)
    .bind(profile_id)
    .bind(recipe_id)
    .bind(PAGE_SIZE)
    .bind(page * PAGE_SIZE)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli ingredienti")?;

    Ok((
        rows.into_iter().map(to_view).collect::<Result<Vec<_>>>()?,
        total,
        page,
    ))
}

async fn load_one(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
) -> Result<Option<IngredientView>> {
    let row = sqlx::query_as::<_, IngredientRow>(
        "SELECT ri.id, a.nome AS name, ri.quantita AS recipe_quantity, \
                um.simbolo AS unit, r.porzioni_base AS servings, \
                COALESCE(prp.fattore_porzione, 1.0) AS factor, \
                o.tipo_override AS override_kind, o.quantita_override AS override_quantity \
         FROM ricetta_ingredienti ri \
         JOIN ricette r ON r.id = ri.ricetta_id \
         JOIN alimenti a ON a.id = ri.alimento_id \
         JOIN unita_misura um ON um.id = ri.unita_misura_id \
         LEFT JOIN profilo_ricetta_porzioni prp \
           ON prp.profilo_alimentare_id = ? AND prp.ricetta_id = r.id \
         LEFT JOIN profilo_ricetta_ingredienti_override o \
           ON o.profilo_alimentare_id = ? AND o.ricetta_ingrediente_id = ri.id \
         WHERE ri.ricetta_id = ? AND ri.id = ?",
    )
    .bind(profile_id)
    .bind(profile_id)
    .bind(recipe_id)
    .bind(ingredient_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'ingrediente")?;

    row.map(to_view).transpose()
}

fn to_view(row: IngredientRow) -> Result<IngredientView> {
    if row.servings <= 0 || !row.recipe_quantity.is_finite() || row.recipe_quantity <= 0.0 {
        bail!("Dati ricetta non validi per il calcolo");
    }
    if !row.factor.is_finite() || row.factor <= 0.0 {
        bail!("Fattore porzione non valido");
    }

    let calculated_quantity = row.recipe_quantity / row.servings as f64 * row.factor;
    let final_quantity = match row.override_kind.as_deref() {
        Some("escluso") => None,
        Some("quantita") => Some(
            row.override_quantity
                .context("Override quantità privo del valore")?,
        ),
        _ => Some(calculated_quantity),
    };

    Ok(IngredientView {
        row,
        calculated_quantity,
        final_quantity,
    })
}

async fn ingredient_page(pool: &SqlitePool, recipe_id: i64, ingredient_id: i64) -> Result<i64> {
    let position: Option<i64> = sqlx::query_scalar(
        "SELECT posizione FROM (\
            SELECT id, ROW_NUMBER() OVER (ORDER BY ordinamento, id) - 1 AS posizione \
            FROM ricetta_ingredienti WHERE ricetta_id = ?\
         ) WHERE id = ?",
    )
    .bind(recipe_id)
    .bind(ingredient_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile determinare la pagina dell'ingrediente")?;

    Ok(position.unwrap_or(0) / PAGE_SIZE)
}

async fn set_quantity(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
    quantity: f64,
) -> Result<bool> {
    if !quantity.is_finite() || quantity <= 0.0 {
        bail!("La quantità deve essere positiva");
    }
    ensure_write_context(pool, profile_id, recipe_id, ingredient_id).await?;

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica dell'ingrediente")?;
    let history = override_history_context(&mut tx, profile_id, recipe_id, ingredient_id).await?;
    let before = current_override_conn(&mut tx, profile_id, ingredient_id).await?;

    if matches!(&before, Some((kind, Some(value))) if kind == "quantita" && (*value - quantity).abs() < f64::EPSILON)
    {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO profilo_ricetta_ingredienti_override \
           (profilo_alimentare_id, ricetta_ingrediente_id, tipo_override, quantita_override) \
         VALUES (?, ?, 'quantita', ?) \
         ON CONFLICT(profilo_alimentare_id, ricetta_ingrediente_id) DO UPDATE SET \
           tipo_override = 'quantita', quantita_override = excluded.quantita_override, \
           aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(profile_id)
    .bind(ingredient_id)
    .bind(quantity)
    .execute(&mut *tx)
    .await
    .context("Impossibile salvare la quantità personalizzata")?;

    record_override_history(
        &mut tx,
        &history,
        before.as_ref(),
        Some(("quantita", Some(quantity))),
    )
    .await?;
    tx.commit()
        .await
        .context("Impossibile completare la modifica dell'ingrediente")?;
    Ok(true)
}

async fn set_excluded(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
) -> Result<bool> {
    ensure_write_context(pool, profile_id, recipe_id, ingredient_id).await?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare l'esclusione dell'ingrediente")?;
    let history = override_history_context(&mut tx, profile_id, recipe_id, ingredient_id).await?;
    let before = current_override_conn(&mut tx, profile_id, ingredient_id).await?;

    if matches!(&before, Some((kind, _)) if kind == "escluso") {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO profilo_ricetta_ingredienti_override \
           (profilo_alimentare_id, ricetta_ingrediente_id, tipo_override, quantita_override) \
         VALUES (?, ?, 'escluso', NULL) \
         ON CONFLICT(profilo_alimentare_id, ricetta_ingrediente_id) DO UPDATE SET \
           tipo_override = 'escluso', quantita_override = NULL, \
           aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(profile_id)
    .bind(ingredient_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile escludere l'ingrediente")?;

    record_override_history(&mut tx, &history, before.as_ref(), Some(("escluso", None))).await?;
    tx.commit()
        .await
        .context("Impossibile completare l'esclusione dell'ingrediente")?;
    Ok(true)
}

async fn reset_override(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
) -> Result<bool> {
    ensure_write_context(pool, profile_id, recipe_id, ingredient_id).await?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare il ripristino dell'ingrediente")?;
    let history = override_history_context(&mut tx, profile_id, recipe_id, ingredient_id).await?;
    let before = current_override_conn(&mut tx, profile_id, ingredient_id).await?;

    let Some(before_value) = before else {
        return Ok(false);
    };

    sqlx::query(
        "DELETE FROM profilo_ricetta_ingredienti_override \
         WHERE profilo_alimentare_id = ? AND ricetta_ingrediente_id = ?",
    )
    .bind(profile_id)
    .bind(ingredient_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile ripristinare la quantità calcolata")?;

    record_override_history(&mut tx, &history, Some(&before_value), None).await?;
    tx.commit()
        .await
        .context("Impossibile completare il ripristino dell'ingrediente")?;
    Ok(true)
}

async fn ensure_write_context(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
) -> Result<()> {
    let user_id = current_user_id()?;
    let managed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM profili_alimentari \
         WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0)",
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il profilo")?;
    if !managed {
        bail!("Non hai il permesso di modificare questo profilo");
    }

    if !recipe_is_visible(pool, recipe_id, user_id).await? {
        bail!("Ricetta non disponibile nel contesto corrente");
    }

    let belongs: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ricetta_ingredienti WHERE id = ? AND ricetta_id = ?)",
    )
    .bind(ingredient_id)
    .bind(recipe_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare l'ingrediente")?;
    if !belongs {
        bail!("Ingrediente non disponibile per questa ricetta");
    }
    Ok(())
}

async fn override_history_context(
    conn: &mut SqliteConnection,
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
) -> Result<OverrideHistoryContext> {
    sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT p.nome, r.nome, a.nome, um.simbolo \
         FROM profili_alimentari p \
         JOIN ricette r ON r.id = ? \
         JOIN ricetta_ingredienti ri ON ri.id = ? AND ri.ricetta_id = r.id \
         JOIN alimenti a ON a.id = ri.alimento_id \
         JOIN unita_misura um ON um.id = ri.unita_misura_id \
         WHERE p.id = ?",
    )
    .bind(recipe_id)
    .bind(ingredient_id)
    .bind(profile_id)
    .fetch_optional(&mut *conn)
    .await
    .context("Impossibile preparare il contesto storico dell'ingrediente")?
    .map(
        |(profile_name, recipe_name, ingredient_name, unit)| OverrideHistoryContext {
            profile_id,
            profile_name,
            recipe_name,
            ingredient_name,
            unit,
        },
    )
    .context("Contesto storico dell'ingrediente non disponibile")
}

async fn current_override_conn(
    conn: &mut SqliteConnection,
    profile_id: i64,
    ingredient_id: i64,
) -> Result<Option<(String, Option<f64>)>> {
    sqlx::query_as(
        "SELECT tipo_override, quantita_override \
         FROM profilo_ricetta_ingredienti_override \
         WHERE profilo_alimentare_id = ? AND ricetta_ingrediente_id = ?",
    )
    .bind(profile_id)
    .bind(ingredient_id)
    .fetch_optional(&mut *conn)
    .await
    .context("Impossibile leggere l'override corrente")
}

async fn record_override_history(
    conn: &mut SqliteConnection,
    context: &OverrideHistoryContext,
    before: Option<&(String, Option<f64>)>,
    after: Option<(&str, Option<f64>)>,
) -> Result<()> {
    let entity_id = storico::ensure_entity(
        conn,
        "profilo_alimentare",
        context.profile_id,
        &context.profile_name,
    )
    .await
    .context("Impossibile preparare lo storico del profilo")?;
    let event_id = storico::record_event(
        conn,
        &NewHistoryEvent {
            entita_storico_id: entity_id,
            modulo: "alimentazione",
            componente: "ingrediente_profilo",
            operazione: "modifica",
            nome_entita_snapshot: &context.profile_name,
            abitazione_storico_id: None,
            abitazione_nome_snapshot: None,
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await
    .context("Impossibile registrare lo storico dell'ingrediente")?;

    storico::record_field_changes(
        conn,
        event_id,
        &[NewFieldChange {
            campo: "override_ingrediente",
            tipo_valore: "testo",
            valore_prima: Some(history_override_value(
                &context.recipe_name,
                &context.ingredient_name,
                &context.unit,
                before.map(|(kind, quantity)| (kind.as_str(), *quantity)),
            )),
            valore_dopo: Some(history_override_value(
                &context.recipe_name,
                &context.ingredient_name,
                &context.unit,
                after,
            )),
        }],
    )
    .await
    .context("Impossibile registrare il cambiamento dell'ingrediente")?;
    Ok(())
}

fn history_override_value(
    recipe_name: &str,
    ingredient_name: &str,
    unit: &str,
    value: Option<(&str, Option<f64>)>,
) -> String {
    let state = match value {
        Some(("escluso", _)) => "escluso".to_string(),
        Some(("quantita", Some(quantity))) => format!("{} {}", format_quantity(quantity), unit),
        _ => "quantità calcolata".to_string(),
    };
    format!("{recipe_name} · {ingredient_name}: {state}")
}

fn ingredient_marker(override_kind: Option<&str>) -> &'static str {
    match override_kind {
        Some("escluso") => "🚫 ",
        Some("quantita") => "⚙️ ",
        _ => "",
    }
}

fn list_keyboard(
    profile_id: i64,
    recipe_id: i64,
    items: &[IngredientView],
    page: i64,
    pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows = items
        .iter()
        .map(|item| {
            let marker = ingredient_marker(item.row.override_kind.as_deref());
            let value = match item.final_quantity {
                Some(quantity) => format!("{} {}", format_quantity(quantity), item.row.unit),
                None => "escluso".to_string(),
            };
            vec![button(
                format!("{marker}{} · {value}", item.row.name),
                detail_callback("v", profile_id, recipe_id, item.row.id),
            )]
        })
        .collect::<Vec<_>>();

    if pages > 1 {
        let mut pagination = Vec::new();
        if page > 0 {
            pagination.push(button(
                "⬅️ Pagina precedente",
                list_callback(profile_id, recipe_id, page - 1),
            ));
        }
        pagination.push(button(format!("{}/{}", page + 1, pages), "foodprof:noop"));
        if page + 1 < pages {
            pagination.push(button(
                "Pagina successiva ➡️",
                list_callback(profile_id, recipe_id, page + 1),
            ));
        }
        rows.push(pagination);
    }

    rows.push(vec![
        button(
            "⬅️ Indietro",
            format!("foodprof:portion:view:{profile_id}:{recipe_id}"),
        ),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn detail_keyboard(
    profile_id: i64,
    recipe_id: i64,
    ingredient_id: i64,
    has_override: bool,
) -> InlineKeyboardMarkup {
    let mut rows = vec![vec![button(
        "🚫 Escludi ingrediente",
        detail_callback("x", profile_id, recipe_id, ingredient_id),
    )]];

    if has_override {
        rows.push(vec![button(
            "♻️ Usa quantità calcolata",
            detail_callback("r", profile_id, recipe_id, ingredient_id),
        )]);
    }

    rows.push(vec![
        button("⬅️ Indietro", list_callback(profile_id, recipe_id, 0)),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn recipe_return_keyboard(profile_id: i64, recipe_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button(
            "⬅️ Torna alla ricetta",
            format!("foodprof:portion:view:{profile_id}:{recipe_id}"),
        ),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn main_return_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![button("🏠 Menù principale", "menu:main")]])
}

async fn send_error(
    bot: &Bot,
    chat_id: ChatId,
    profile_id: i64,
    recipe_id: i64,
    error: &str,
) -> ResponseResult<()> {
    bot.send_message(chat_id, format!("⚠️ {error}"))
        .reply_markup(recipe_return_keyboard(profile_id, recipe_id))
        .await?;
    Ok(())
}

pub fn list_callback_for_recipe(profile_id: i64, recipe_id: i64) -> String {
    list_callback(profile_id, recipe_id, 0)
}

fn list_callback(profile_id: i64, recipe_id: i64, page: i64) -> String {
    format!(
        "foodprof:ing:l:{}:{}:{page}",
        to_base36(profile_id),
        to_base36(recipe_id)
    )
}

fn detail_callback(action: &str, profile_id: i64, recipe_id: i64, ingredient_id: i64) -> String {
    format!(
        "foodprof:ing:{action}:{}:{}:{}",
        to_base36(profile_id),
        to_base36(recipe_id),
        to_base36(ingredient_id)
    )
}

fn parse_list_callback(data: &str) -> Option<(i64, i64, i64)> {
    let rest = data.strip_prefix("foodprof:ing:l:")?;
    let mut parts = rest.split(':');
    let profile_id = from_base36(parts.next()?)?;
    let recipe_id = from_base36(parts.next()?)?;
    let page = parts.next()?.parse::<i64>().ok()?;
    if profile_id <= 0 || recipe_id <= 0 || page < 0 || parts.next().is_some() {
        return None;
    }
    Some((profile_id, recipe_id, page))
}

fn parse_detail_callback(data: &str, prefix: &str) -> Option<(i64, i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let profile_id = from_base36(parts.next()?)?;
    let recipe_id = from_base36(parts.next()?)?;
    let ingredient_id = from_base36(parts.next()?)?;
    if profile_id <= 0 || recipe_id <= 0 || ingredient_id <= 0 || parts.next().is_some() {
        return None;
    }
    Some((profile_id, recipe_id, ingredient_id))
}

fn to_base36(mut value: i64) -> String {
    debug_assert!(value > 0);
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buffer = [0_u8; 13];
    let mut index = buffer.len();
    while value > 0 {
        index -= 1;
        buffer[index] = DIGITS[(value % 36) as usize];
        value /= 36;
    }
    String::from_utf8(buffer[index..].to_vec()).expect("base36 ASCII valido")
}

fn from_base36(value: &str) -> Option<i64> {
    i64::from_str_radix(value, 36).ok()
}

fn parse_quantity(text: &str) -> Option<f64> {
    let value = text.trim().replace(',', ".").parse::<f64>().ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}

fn format_quantity(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        let mut rendered = format!("{value:.2}");
        while rendered.ends_with('0') {
            rendered.pop();
        }
        if rendered.ends_with('.') {
            rendered.pop();
        }
        rendered.replace('.', ",")
    }
}

fn page_count(total: i64) -> i64 {
    ((total + PAGE_SIZE - 1) / PAGE_SIZE).max(1)
}

fn current_user_id() -> Result<i64> {
    identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")
}

fn button(text: impl Into<String>, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), data.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_roundtrip_e_limite_telegram() {
        let max = i64::MAX;
        let callbacks = [
            list_callback(max, max, 999),
            detail_callback("v", max, max, max),
            detail_callback("x", max, max, max),
            detail_callback("r", max, max, max),
        ];
        assert!(callbacks.iter().all(|value| value.len() <= 64));

        let detail = detail_callback("v", 12, 34, 56);
        assert_eq!(
            parse_detail_callback(&detail, "foodprof:ing:v:"),
            Some((12, 34, 56))
        );
        let list = list_callback(12, 34, 2);
        assert_eq!(parse_list_callback(&list), Some((12, 34, 2)));
    }

    #[test]
    fn quantita_manual_accetta_formato_italiano() {
        assert_eq!(parse_quantity("90"), Some(90.0));
        assert_eq!(parse_quantity("90,5"), Some(90.5));
        assert_eq!(parse_quantity("0"), None);
        assert_eq!(parse_quantity("-2"), None);
        assert_eq!(parse_quantity("ciao"), None);
    }

    #[test]
    fn elenco_non_aggiunge_carota_agli_ingredienti_normali() {
        assert_eq!(ingredient_marker(None), "");
        assert_eq!(ingredient_marker(Some("quantita")), "⚙️ ");
        assert_eq!(ingredient_marker(Some("escluso")), "🚫 ");
    }

    #[test]
    fn storico_override_ingrediente_e_leggibile() {
        assert_eq!(
            history_override_value("Pasta test", "Pasta", "g", None),
            "Pasta test · Pasta: quantità calcolata"
        );
        assert_eq!(
            history_override_value("Pasta test", "Pasta", "g", Some(("quantita", Some(90.5)))),
            "Pasta test · Pasta: 90,5 g"
        );
        assert_eq!(
            history_override_value("Pasta test", "Pasta", "g", Some(("escluso", None))),
            "Pasta test · Pasta: escluso"
        );
    }

    #[test]
    fn pagina_ingredienti_massimo_cinque() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(5), 1);
        assert_eq!(page_count(6), 2);
    }

    #[test]
    fn base36_roundtrip() {
        for value in [1_i64, 35, 36, 1_000_000, i64::MAX] {
            assert_eq!(from_base36(&to_base36(value)), Some(value));
        }
    }
}
