//! Step 7.2I.1 - Porzione ricetta per Profilo alimentare.

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

const RECIPE_PAGE_SIZE: i64 = 5;
const PRESET_PERCENTAGES: [i64; 4] = [80, 100, 120, 150];

#[derive(Debug, Clone, FromRow)]
struct RecipePortionRecord {
    id: i64,
    name: String,
    servings: i64,
    factor: Option<f64>,
}

#[derive(Debug, Clone)]
struct RecipePortionPage {
    items: Vec<RecipePortionRecord>,
    total: i64,
    page: i64,
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    if let Some((profile_id, page)) = parse_two_i64(data, "foodprof:portion:list:") {
        show_recipe_list(bot, chat_id, pool, profile_id, page).await?;
        return Ok(true);
    }

    if let Some((profile_id, recipe_id)) = parse_two_i64(data, "foodprof:portion:view:") {
        show_portion_detail(bot, chat_id, pool, profile_id, recipe_id).await?;
        return Ok(true);
    }

    if let Some((profile_id, recipe_id, percentage)) =
        parse_three_i64(data, "foodprof:portion:set:")
    {
        if !PRESET_PERCENTAGES.contains(&percentage) {
            send_invalid_action(bot, chat_id, profile_id).await?;
            return Ok(true);
        }

        match set_portion_percentage(pool, profile_id, recipe_id, percentage).await {
            Ok(changed) => {
                let message = if changed {
                    "✅ Porzione personale aggiornata."
                } else {
                    "ℹ️ La porzione era già impostata così."
                };
                bot.send_message(chat_id, message).await?;
                show_portion_detail(bot, chat_id, pool, profile_id, recipe_id).await?;
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    profile_id,
                    recipe_id,
                    percentage,
                    "Modifica porzione profilo rifiutata"
                );
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(portion_return_keyboard(profile_id))
                    .await?;
            }
        }
        return Ok(true);
    }

    if let Some((profile_id, recipe_id)) = parse_two_i64(data, "foodprof:portion:reset:") {
        match reset_portion(pool, profile_id, recipe_id).await {
            Ok(changed) => {
                let message = if changed {
                    "♻️ Ripristinata la porzione standard."
                } else {
                    "ℹ️ La ricetta usa già la porzione standard."
                };
                bot.send_message(chat_id, message).await?;
                show_portion_detail(bot, chat_id, pool, profile_id, recipe_id).await?;
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    profile_id,
                    recipe_id,
                    "Ripristino porzione profilo rifiutato"
                );
                bot.send_message(chat_id, format!("⚠️ {error}"))
                    .reply_markup(portion_return_keyboard(profile_id))
                    .await?;
            }
        }
        return Ok(true);
    }

    send_invalid_action(bot, chat_id, 0).await?;
    Ok(true)
}

async fn show_recipe_list(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
    requested_page: i64,
) -> ResponseResult<()> {
    let profile_name = match managed_profile_name(pool, profile_id).await {
        Ok(Some(name)) => name,
        Ok(None) => {
            bot.send_message(
                chat_id,
                "⚠️ Solo il gestore del profilo può modificare le sue porzioni.",
            )
            .reply_markup(main_profile_menu_keyboard())
            .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, profile_id, "Errore verifica profilo per porzioni");
            bot.send_message(chat_id, "⚠️ Non riesco a verificare questo profilo.")
                .reply_markup(main_profile_menu_keyboard())
                .await?;
            return Ok(());
        }
    };

    match list_visible_recipes(pool, profile_id, requested_page).await {
        Ok(page) => {
            let pages = page_count(page.total);
            let text = if page.total == 0 {
                format!(
                    "🍽️ Porzioni e preferenze · {profile_name}\n\nNessuna ricetta disponibile nel contesto corrente."
                )
            } else {
                format!(
                    "🍽️ Porzioni e preferenze · {profile_name}\n\nScegli una ricetta per impostare la porzione personale.\n\nTotale: {}\nPagina {}/{}\n\n💡 100% = porzione standard della ricetta.",
                    page.total,
                    page.page + 1,
                    pages
                )
            };
            bot.send_message(chat_id, text)
                .reply_markup(recipe_list_keyboard(profile_id, &page, pages))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, profile_id, "Errore elenco ricette per porzioni");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere le ricette disponibili.")
                .reply_markup(portion_return_keyboard(profile_id))
                .await?;
        }
    }

    Ok(())
}

async fn show_portion_detail(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
) -> ResponseResult<()> {
    let profile_name = match managed_profile_name(pool, profile_id).await {
        Ok(Some(name)) => name,
        Ok(None) => {
            bot.send_message(
                chat_id,
                "⚠️ Solo il gestore del profilo può modificare le sue porzioni.",
            )
            .reply_markup(main_profile_menu_keyboard())
            .await?;
            return Ok(());
        }
        Err(error) => {
            tracing::error!(?error, profile_id, "Errore verifica profilo");
            bot.send_message(chat_id, "⚠️ Non riesco a verificare questo profilo.")
                .reply_markup(main_profile_menu_keyboard())
                .await?;
            return Ok(());
        }
    };

    match visible_recipe(pool, profile_id, recipe_id).await {
        Ok(Some(recipe)) => {
            let percentage = factor_to_percentage(recipe.factor.unwrap_or(1.0));
            let custom = recipe.factor.is_some();
            let mode = if custom {
                "personalizzata"
            } else {
                "standard della ricetta"
            };

            bot.send_message(
                chat_id,
                format!(
                    "🍽️ Porzione personale\n\n👤 Profilo: {profile_name}\n🍳 Ricetta: {}\n👥 Ricetta base: {} porzioni\n\n⚖️ Porzione: {percentage}%\n📌 Modalità: {mode}\n\nLa percentuale scala proporzionalmente le quantità della singola porzione.\n\n⌨️ Puoi anche scrivere direttamente una percentuale in chat, ad esempio 125 oppure 125%.",
                    recipe.name, recipe.servings
                ),
            )
            .reply_markup(portion_detail_keyboard(
                profile_id,
                recipe_id,
                percentage,
                custom,
            ))
            .await?;
        }
        Ok(None) => {
            bot.send_message(chat_id, "⚠️ Ricetta non disponibile nel contesto corrente.")
                .reply_markup(portion_return_keyboard(profile_id))
                .await?;
        }
        Err(error) => {
            tracing::error!(?error, profile_id, recipe_id, "Errore dettaglio porzione");
            bot.send_message(chat_id, "⚠️ Non riesco a leggere questa ricetta.")
                .reply_markup(portion_return_keyboard(profile_id))
                .await?;
        }
    }

    Ok(())
}

async fn managed_profile_name(pool: &SqlitePool, profile_id: i64) -> Result<Option<String>> {
    let user_id = current_user_id()?;
    sqlx::query_scalar(
        "SELECT nome FROM profili_alimentari \
         WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile verificare il gestore del profilo")
}

async fn list_visible_recipes(
    pool: &SqlitePool,
    profile_id: i64,
    requested_page: i64,
) -> Result<RecipePortionPage> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Identità utente non disponibile per le porzioni")?;

    if managed_profile_name(pool, profile_id).await?.is_none() {
        bail!("Non hai il permesso di modificare questo profilo");
    }

    let (predicate, bind_space) = recipe_visibility_predicate("r", &actor);

    let count_sql =
        format!("SELECT COUNT(*) FROM ricette r WHERE r.archiviata = 0 AND ({predicate})");
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql).bind(user_id);
    if bind_space {
        count_query = count_query.bind(actor.spazio_id);
    } else {
        count_query = count_query.bind(user_id);
    }
    let total = count_query
        .fetch_one(pool)
        .await
        .context("Impossibile contare le ricette per il profilo")?;

    let pages = page_count(total);
    let page = requested_page.max(0).min(pages.saturating_sub(1));
    let offset = page * RECIPE_PAGE_SIZE;

    let list_sql = format!(
        "SELECT r.id, r.nome AS name, r.porzioni_base AS servings, \
                prp.fattore_porzione AS factor \
         FROM ricette r \
         LEFT JOIN profilo_ricetta_porzioni prp \
           ON prp.ricetta_id = r.id AND prp.profilo_alimentare_id = ? \
         WHERE r.archiviata = 0 AND ({predicate}) \
         ORDER BY r.nome COLLATE NOCASE, r.id \
         LIMIT ? OFFSET ?"
    );

    let mut list_query = sqlx::query_as::<_, RecipePortionRecord>(&list_sql)
        .bind(profile_id)
        .bind(user_id);
    if bind_space {
        list_query = list_query.bind(actor.spazio_id);
    } else {
        list_query = list_query.bind(user_id);
    }

    let items = list_query
        .bind(RECIPE_PAGE_SIZE)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere le ricette per il profilo")?;

    Ok(RecipePortionPage { items, total, page })
}

async fn visible_recipe(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
) -> Result<Option<RecipePortionRecord>> {
    let actor = identity::current_actor();
    let user_id = actor
        .utente_id
        .context("Identità utente non disponibile per le porzioni")?;

    if managed_profile_name(pool, profile_id).await?.is_none() {
        return Ok(None);
    }

    let (predicate, bind_space) = recipe_visibility_predicate("r", &actor);
    let sql = format!(
        "SELECT r.id, r.nome AS name, r.porzioni_base AS servings, \
                prp.fattore_porzione AS factor \
         FROM ricette r \
         LEFT JOIN profilo_ricetta_porzioni prp \
           ON prp.ricetta_id = r.id AND prp.profilo_alimentare_id = ? \
         WHERE r.id = ? AND r.archiviata = 0 AND ({predicate})"
    );

    let mut query = sqlx::query_as::<_, RecipePortionRecord>(&sql)
        .bind(profile_id)
        .bind(recipe_id)
        .bind(user_id);
    if bind_space {
        query = query.bind(actor.spazio_id);
    } else {
        query = query.bind(user_id);
    }

    query
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere la ricetta per il profilo")
}

pub async fn handle_percentage_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    text: &str,
) -> ResponseResult<()> {
    let Some(percentage) = parse_manual_percentage(text) else {
        bot.send_message(
            msg.chat.id,
            "⚠️ Scrivi una percentuale intera tra 1 e 500.\n\nEsempi: 125 oppure 125%.",
        )
        .reply_markup(portion_detail_keyboard(profile_id, recipe_id, 0, true))
        .await?;
        return Ok(());
    };

    match set_portion_percentage(pool, profile_id, recipe_id, percentage).await {
        Ok(changed) => {
            if changed {
                bot.send_message(
                    msg.chat.id,
                    format!("✅ Porzione personale impostata al {percentage}%."),
                )
                .await?;
            } else {
                bot.send_message(
                    msg.chat.id,
                    format!("ℹ️ La porzione era già impostata al {percentage}%."),
                )
                .await?;
            }
            show_portion_detail(bot, msg.chat.id, pool, profile_id, recipe_id).await?;
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                profile_id,
                recipe_id,
                percentage,
                "Inserimento manuale porzione rifiutato"
            );
            bot.send_message(msg.chat.id, format!("⚠️ {error}"))
                .reply_markup(portion_return_keyboard(profile_id))
                .await?;
        }
    }

    Ok(())
}

pub fn portion_context_from_callback(data: &str) -> Option<(i64, i64)> {
    let rest = data
        .strip_prefix("foodprof:portion:view:")
        .or_else(|| data.strip_prefix("foodprof:portion:set:"))
        .or_else(|| data.strip_prefix("foodprof:portion:reset:"))?;

    let mut parts = rest.split(':');
    let profile_id = parse_positive_i64(parts.next()?)?;
    let recipe_id = parse_positive_i64(parts.next()?)?;
    Some((profile_id, recipe_id))
}

async fn set_portion_percentage(
    pool: &SqlitePool,
    profile_id: i64,
    recipe_id: i64,
    percentage: i64,
) -> Result<bool> {
    if !(1..=500).contains(&percentage) {
        bail!("La percentuale deve essere compresa tra 1% e 500%");
    }
    if percentage == 100 {
        return reset_portion(pool, profile_id, recipe_id).await;
    }

    let user_id = current_user_id()?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare la modifica della porzione")?;

    let profile_name = managed_profile_name_conn(&mut tx, profile_id, user_id).await?;
    ensure_recipe_visible_conn(&mut tx, recipe_id, user_id).await?;

    let before: Option<f64> = sqlx::query_scalar(
        "SELECT fattore_porzione FROM profilo_ricetta_porzioni \
         WHERE profilo_alimentare_id = ? AND ricetta_id = ?",
    )
    .bind(profile_id)
    .bind(recipe_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile leggere la porzione corrente")?;

    let new_factor = percentage as f64 / 100.0;
    if before.is_some_and(|value| (value - new_factor).abs() < f64::EPSILON) {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO profilo_ricetta_porzioni \
           (profilo_alimentare_id, ricetta_id, fattore_porzione) \
         VALUES (?, ?, ?) \
         ON CONFLICT(profilo_alimentare_id, ricetta_id) DO UPDATE SET \
           fattore_porzione = excluded.fattore_porzione, \
           aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(profile_id)
    .bind(recipe_id)
    .bind(new_factor)
    .execute(&mut *tx)
    .await
    .context("Impossibile salvare la porzione personale")?;

    record_portion_history(
        &mut tx,
        profile_id,
        &profile_name,
        recipe_id,
        before,
        Some(new_factor),
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare la modifica della porzione")?;
    Ok(true)
}

async fn reset_portion(pool: &SqlitePool, profile_id: i64, recipe_id: i64) -> Result<bool> {
    let user_id = current_user_id()?;
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile iniziare il ripristino della porzione")?;

    let profile_name = managed_profile_name_conn(&mut tx, profile_id, user_id).await?;
    ensure_recipe_visible_conn(&mut tx, recipe_id, user_id).await?;

    let before: Option<f64> = sqlx::query_scalar(
        "SELECT fattore_porzione FROM profilo_ricetta_porzioni \
         WHERE profilo_alimentare_id = ? AND ricetta_id = ?",
    )
    .bind(profile_id)
    .bind(recipe_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile leggere la porzione corrente")?;

    let Some(before_factor) = before else {
        return Ok(false);
    };

    sqlx::query(
        "DELETE FROM profilo_ricetta_porzioni \
         WHERE profilo_alimentare_id = ? AND ricetta_id = ?",
    )
    .bind(profile_id)
    .bind(recipe_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile ripristinare la porzione standard")?;

    record_portion_history(
        &mut tx,
        profile_id,
        &profile_name,
        recipe_id,
        Some(before_factor),
        None,
    )
    .await?;

    tx.commit()
        .await
        .context("Impossibile completare il ripristino della porzione")?;
    Ok(true)
}

async fn managed_profile_name_conn(
    conn: &mut SqliteConnection,
    profile_id: i64,
    user_id: i64,
) -> Result<String> {
    sqlx::query_scalar(
        "SELECT nome FROM profili_alimentari \
         WHERE id = ? AND gestore_utente_id = ? AND archiviato = 0",
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await
    .context("Impossibile verificare il gestore del profilo")?
    .context("Non hai il permesso di modificare questo profilo")
}

async fn ensure_recipe_visible_conn(
    conn: &mut SqliteConnection,
    recipe_id: i64,
    user_id: i64,
) -> Result<()> {
    let actor = identity::current_actor();
    let visible: bool = if actor.view_all {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ricette r WHERE r.id = ? AND r.archiviata = 0 AND \
             (r.catalogo_globale = 1 OR r.proprietario_utente_id = ? OR EXISTS( \
                SELECT 1 FROM ricetta_spazi rs JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
                WHERE rs.ricetta_id = r.id AND ms.utente_id = ?)))",
        )
        .bind(recipe_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_one(&mut *conn)
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
        .fetch_one(&mut *conn)
        .await
        .context("Impossibile verificare la visibilità della ricetta")?
    };

    if !visible {
        bail!("Ricetta non disponibile nel contesto corrente");
    }
    Ok(())
}

async fn record_portion_history(
    conn: &mut SqliteConnection,
    profile_id: i64,
    profile_name: &str,
    recipe_id: i64,
    before: Option<f64>,
    after: Option<f64>,
) -> Result<()> {
    let recipe_name: String = sqlx::query_scalar("SELECT nome FROM ricette WHERE id = ?")
        .bind(recipe_id)
        .fetch_one(&mut *conn)
        .await
        .context("Impossibile leggere il nome della ricetta per lo storico")?;

    let entity_id = storico::ensure_entity(conn, "profilo_alimentare", profile_id, profile_name)
        .await
        .context("Impossibile preparare lo storico del profilo")?;

    let event_id = storico::record_event(
        conn,
        &NewHistoryEvent {
            entita_storico_id: entity_id,
            modulo: "alimentazione",
            componente: "porzione_profilo",
            operazione: "modifica",
            nome_entita_snapshot: profile_name,
            abitazione_storico_id: None,
            abitazione_nome_snapshot: None,
            stanza_storico_id: None,
            stanza_nome_snapshot: None,
            evento_padre_id: None,
        },
    )
    .await
    .context("Impossibile registrare lo storico della porzione")?;

    storico::record_field_changes(
        conn,
        event_id,
        &[NewFieldChange {
            campo: "porzione_ricetta",
            tipo_valore: "testo",
            valore_prima: Some(portion_history_value(&recipe_name, before)),
            valore_dopo: Some(portion_history_value(&recipe_name, after)),
        }],
    )
    .await
    .context("Impossibile registrare il cambiamento di porzione")?;
    Ok(())
}

fn portion_history_value(recipe_name: &str, factor: Option<f64>) -> String {
    format!(
        "{recipe_name}: {}%",
        factor_to_percentage(factor.unwrap_or(1.0))
    )
}

fn recipe_visibility_predicate(alias: &str, actor: &identity::AuditActor) -> (String, bool) {
    if actor.view_all {
        (
            format!(
                "{alias}.catalogo_globale = 1 OR {alias}.proprietario_utente_id = ? OR EXISTS (\
                    SELECT 1 FROM ricetta_spazi rvs JOIN membri_spazio rms ON rms.spazio_id = rvs.spazio_id \
                    WHERE rvs.ricetta_id = {alias}.id AND rms.utente_id = ?)"
            ),
            false,
        )
    } else {
        (
            format!(
                "{alias}.catalogo_globale = 1 OR {alias}.proprietario_utente_id = ? OR EXISTS (\
                    SELECT 1 FROM ricetta_spazi rvs \
                    WHERE rvs.ricetta_id = {alias}.id AND rvs.spazio_id = ?)"
            ),
            true,
        )
    }
}

fn recipe_list_keyboard(
    profile_id: i64,
    page: &RecipePortionPage,
    pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows = page
        .items
        .iter()
        .map(|recipe| {
            let percentage = factor_to_percentage(recipe.factor.unwrap_or(1.0));
            let marker = if recipe.factor.is_some() {
                "⚙️"
            } else {
                "🍽️"
            };
            vec![button(
                format!("{marker} {} · {percentage}%", recipe.name),
                format!("foodprof:portion:view:{profile_id}:{}", recipe.id),
            )]
        })
        .collect::<Vec<_>>();

    if page.total > 0 {
        let mut pagination = Vec::new();
        if page.page > 0 {
            pagination.push(button(
                "⬅️ Pagina precedente",
                format!("foodprof:portion:list:{profile_id}:{}", page.page - 1),
            ));
        }
        pagination.push(button(
            format!("{}/{}", page.page + 1, pages),
            "foodprof:noop",
        ));
        if page.page + 1 < pages {
            pagination.push(button(
                "Pagina successiva ➡️",
                format!("foodprof:portion:list:{profile_id}:{}", page.page + 1),
            ));
        }
        rows.push(pagination);
    }

    rows.push(vec![
        button("⬅️ Indietro", format!("foodprof:view:{profile_id}")),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn portion_detail_keyboard(
    profile_id: i64,
    recipe_id: i64,
    current_percentage: i64,
    custom: bool,
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();

    rows.push(
        PRESET_PERCENTAGES
            .into_iter()
            .map(|percentage| {
                let prefix = if percentage == current_percentage {
                    "✅"
                } else {
                    "◻️"
                };
                button(
                    format!("{prefix} {percentage}%"),
                    format!("foodprof:portion:set:{profile_id}:{recipe_id}:{percentage}"),
                )
            })
            .collect(),
    );

    if custom {
        rows.push(vec![button(
            "♻️ Ripristina standard",
            format!("foodprof:portion:reset:{profile_id}:{recipe_id}"),
        )]);
    }

    rows.push(vec![
        button(
            "⬅️ Indietro",
            format!("foodprof:portion:list:{profile_id}:0"),
        ),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn portion_return_keyboard(profile_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button(
            "⬅️ Torna alle porzioni",
            format!("foodprof:portion:list:{profile_id}:0"),
        ),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

fn main_profile_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("👥 Profili alimentari", "foodprof:menu")],
        vec![button("🏠 Menù principale", "menu:main")],
    ])
}

async fn send_invalid_action(bot: &Bot, chat_id: ChatId, profile_id: i64) -> ResponseResult<()> {
    let keyboard = if profile_id > 0 {
        portion_return_keyboard(profile_id)
    } else {
        main_profile_menu_keyboard()
    };
    bot.send_message(chat_id, "⚠️ Azione porzione non valida.")
        .reply_markup(keyboard)
        .await?;
    Ok(())
}

fn parse_manual_percentage(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    let without_percent = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();
    let percentage = without_percent.parse::<i64>().ok()?;
    (1..=500).contains(&percentage).then_some(percentage)
}

fn current_user_id() -> Result<i64> {
    identity::current_actor()
        .utente_id
        .context("Identità utente non disponibile")
}

fn factor_to_percentage(factor: f64) -> i64 {
    (factor * 100.0).round() as i64
}

fn page_count(total: i64) -> i64 {
    ((total + RECIPE_PAGE_SIZE - 1) / RECIPE_PAGE_SIZE).max(1)
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|value| *value > 0)
}

fn parse_nonnegative_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|value| *value >= 0)
}

fn parse_two_i64(data: &str, prefix: &str) -> Option<(i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let first = parse_positive_i64(parts.next()?)?;
    let second = if prefix.ends_with("list:") {
        parse_nonnegative_i64(parts.next()?)?
    } else {
        parse_positive_i64(parts.next()?)?
    };
    (parts.next().is_none()).then_some((first, second))
}

fn parse_three_i64(data: &str, prefix: &str) -> Option<(i64, i64, i64)> {
    let rest = data.strip_prefix(prefix)?;
    let mut parts = rest.split(':');
    let first = parse_positive_i64(parts.next()?)?;
    let second = parse_positive_i64(parts.next()?)?;
    let third = parse_positive_i64(parts.next()?)?;
    (parts.next().is_none()).then_some((first, second, third))
}

fn button(text: impl Into<String>, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.into(), data.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fattore_diventa_percentuale_leggibile() {
        assert_eq!(factor_to_percentage(0.8), 80);
        assert_eq!(factor_to_percentage(1.0), 100);
        assert_eq!(factor_to_percentage(1.2), 120);
        assert_eq!(factor_to_percentage(1.5), 150);
    }

    #[test]
    fn paginazione_ricette_usa_cinque_elementi() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(5), 1);
        assert_eq!(page_count(6), 2);
        assert_eq!(page_count(11), 3);
    }

    #[test]
    fn callback_porzioni_restano_nel_limite_telegram() {
        let samples = [
            format!("foodprof:portion:list:{}:999999", i64::MAX),
            format!("foodprof:portion:view:{}:{}", i64::MAX, i64::MAX),
            format!("foodprof:portion:set:{}:{}:150", i64::MAX, i64::MAX),
            format!("foodprof:portion:reset:{}:{}", i64::MAX, i64::MAX),
        ];
        assert!(samples.iter().all(|value| value.len() <= 64));
    }

    #[test]
    fn percentuale_manual_accetta_numero_e_simbolo() {
        assert_eq!(parse_manual_percentage("125"), Some(125));
        assert_eq!(parse_manual_percentage("125%"), Some(125));
        assert_eq!(parse_manual_percentage(" 80% "), Some(80));
        assert_eq!(parse_manual_percentage("0"), None);
        assert_eq!(parse_manual_percentage("501"), None);
        assert_eq!(parse_manual_percentage("ciao"), None);
    }

    #[test]
    fn parser_callback_porzione_funzionante() {
        assert_eq!(
            parse_two_i64("foodprof:portion:list:12:3", "foodprof:portion:list:"),
            Some((12, 3))
        );
        assert_eq!(
            parse_two_i64("foodprof:portion:view:12:99", "foodprof:portion:view:"),
            Some((12, 99))
        );
        assert_eq!(
            parse_three_i64("foodprof:portion:set:12:99:120", "foodprof:portion:set:"),
            Some((12, 99, 120))
        );
    }
}
