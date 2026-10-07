//! 📅 Scadenze e 📝 cose da fare, dentro ⏰ Promemoria (7 ottobre 2026).
//!
//! Una **scadenza** è una data entro cui fare qualcosa: gli avvisi arrivano
//! nei giorni che decide la priorità, più fitti man mano che la data si
//! avvicina, e smettono con `✅ Fatto`. Una **cosa da fare** non ha né data
//! né priorità: sta in una lista e si spunta.

use anyhow::Context as _;
use chrono::{Duration, NaiveDate, NaiveDateTime};
use sqlx::SqlitePool;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use super::{
    acceso, adesso_locale, annulla_row, aspetta, attesa, button, chiudi_attesa, leggi_quando,
    manda, nav_row, primo_maiuscolo, regole, salva_regole, scrivi_ora_db, utente_corrente, Attesa,
    Avviso, Bot,
};
use crate::modules::{calendario, liste};

// ===========================================================================
// Dominio puro.
// ===========================================================================

/// Quanto conta una scadenza: decide quante volte e quando avvisare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priorita {
    Bassa,
    Media,
    Alta,
}

pub const PRIORITA: [Priorita; 3] = [Priorita::Alta, Priorita::Media, Priorita::Bassa];

impl Priorita {
    pub fn token(self) -> &'static str {
        match self {
            Priorita::Bassa => "bassa",
            Priorita::Media => "media",
            Priorita::Alta => "alta",
        }
    }

    pub fn da_token(valore: &str) -> Option<Self> {
        PRIORITA.iter().copied().find(|p| p.token() == valore)
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Priorita::Bassa => "🟢",
            Priorita::Media => "🟡",
            Priorita::Alta => "🔴",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Priorita::Bassa => "Bassa",
            Priorita::Media => "Media",
            Priorita::Alta => "Alta",
        }
    }

    /// I giorni prima della data in cui arriva un avviso (0 = il giorno
    /// stesso).
    pub fn giorni_prima(self) -> &'static [i64] {
        match self {
            Priorita::Bassa => &[7, 0],
            Priorita::Media => &[30, 7, 1, 0],
            Priorita::Alta => &[30, 14, 7, 3, 2, 1, 0],
        }
    }

    /// Per quanti giorni dopo la data si continua ad avvisare, se non è
    /// fatta.
    pub fn giorni_dopo(self) -> i64 {
        match self {
            Priorita::Bassa => 0,
            Priorita::Media => 1,
            Priorita::Alta => 7,
        }
    }

    /// "30, 7 e 1 giorno prima, il giorno stesso, e il giorno dopo se non è
    /// fatta".
    pub fn spiegazione(self) -> String {
        let prima: Vec<String> = self
            .giorni_prima()
            .iter()
            .filter(|giorni| **giorni > 0)
            .map(|giorni| giorni.to_string())
            .collect();
        let prima = match prima.as_slice() {
            [] => String::new(),
            [uno] if uno == "1" => "1 giorno prima".to_string(),
            [uno] => format!("{uno} giorni prima"),
            [resto @ .., ultimo] => format!("{} e {ultimo} giorni prima", resto.join(", ")),
        };
        match self.giorni_dopo() {
            0 => format!("avvisi {prima} e il giorno stesso"),
            1 => format!("avvisi {prima}, il giorno stesso e il giorno dopo se non è fatta"),
            giorni => format!(
                "avvisi {prima}, il giorno stesso, poi ogni giorno per {giorni} giorni finché non è fatta"
            ),
        }
    }
}

/// Se oggi tocca un avviso, a `giorni` dalla data (negativi: già passata).
pub fn tocca_oggi(priorita: Priorita, giorni: i64) -> bool {
    if giorni >= 0 {
        priorita.giorni_prima().contains(&giorni)
    } else {
        -giorni <= priorita.giorni_dopo()
    }
}

/// "fra 7 giorni", "domani", "oggi", "scaduta ieri", "scaduta da 3 giorni".
pub fn quanto_manca(giorni: i64) -> String {
    match giorni {
        0 => "oggi".to_string(),
        1 => "domani".to_string(),
        g if g > 1 => format!("fra {g} giorni"),
        -1 => "scaduta ieri".to_string(),
        g => format!("scaduta da {} giorni", -g),
    }
}

/// La data di una scadenza scritta a mano: tutto quello che capisce
/// `leggi_quando` ("15/10", "fra 2 settimane", "venerdì"), di cui conta il
/// giorno.
pub fn leggi_data(testo: &str, adesso: NaiveDateTime) -> Option<NaiveDate> {
    leggi_quando(testo, adesso).map(|quando| quando.date())
}

// ===========================================================================
// Database.
// ===========================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Scadenza {
    pub id: i64,
    pub testo: String,
    pub data: String,
    pub priorita: String,
    pub stato: String,
}

impl Scadenza {
    pub fn priorita(&self) -> Priorita {
        Priorita::da_token(&self.priorita).unwrap_or(Priorita::Media)
    }

    fn giorni(&self, oggi: NaiveDate) -> i64 {
        calendario::parse_date(&self.data)
            .map(|data| (data - oggi).num_days())
            .unwrap_or(0)
    }
}

pub async fn crea_scadenza(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: Option<i64>,
    testo: &str,
    data: NaiveDate,
    priorita: Priorita,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO scadenze (utente_id, spazio_id, testo, data, priorita) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(utente_id)
    .bind(spazio_id)
    .bind(testo.trim())
    .bind(calendario::format_date(data))
    .bind(priorita.token())
    .fetch_one(pool)
    .await
    .context("Impossibile salvare la scadenza")
}

async fn leggi_scadenza(pool: &SqlitePool, utente_id: i64, id: i64) -> Option<Scadenza> {
    sqlx::query_as(
        "SELECT id, testo, data, priorita, stato FROM scadenze WHERE id = ? AND utente_id = ?",
    )
    .bind(id)
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn scadenze_aperte(pool: &SqlitePool, utente_id: i64) -> Vec<Scadenza> {
    sqlx::query_as(
        "SELECT id, testo, data, priorita, stato FROM scadenze \
         WHERE utente_id = ? AND stato = 'aperta' ORDER BY data, id",
    )
    .bind(utente_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// `✅ Fatto` su una scadenza: niente più avvisi.
pub async fn segna_fatta(
    pool: &SqlitePool,
    utente_id: i64,
    id: i64,
    adesso: NaiveDateTime,
) -> bool {
    sqlx::query(
        "UPDATE scadenze SET stato = 'fatta', fatta_il = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND utente_id = ? AND stato = 'aperta'",
    )
    .bind(scrivi_ora_db(adesso))
    .bind(id)
    .bind(utente_id)
    .execute(pool)
    .await
    .map(|esito| esito.rows_affected() > 0)
    .unwrap_or(false)
}

/// Cambia un campo della scadenza. Una data nuova la riapre: rinnovare un
/// documento vuol dire una scadenza nuova da rispettare.
async fn aggiorna_scadenza(
    pool: &SqlitePool,
    utente_id: i64,
    id: i64,
    colonna: &str,
    valore: &str,
) -> bool {
    let riapri = if colonna == "data" {
        ", stato = 'aperta', fatta_il = NULL"
    } else {
        ""
    };
    let colonna = match colonna {
        "testo" | "data" | "priorita" => colonna,
        _ => return false,
    };
    sqlx::query(&format!(
        "UPDATE scadenze SET {colonna} = ?{riapri}, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND utente_id = ?"
    ))
    .bind(valore.trim())
    .bind(id)
    .bind(utente_id)
    .execute(pool)
    .await
    .map(|esito| esito.rows_affected() > 0)
    .unwrap_or(false)
}

async fn elimina_scadenza(pool: &SqlitePool, utente_id: i64, id: i64) -> bool {
    sqlx::query("DELETE FROM scadenze WHERE id = ? AND utente_id = ?")
        .bind(id)
        .bind(utente_id)
        .execute(pool)
        .await
        .map(|esito| esito.rows_affected() > 0)
        .unwrap_or(false)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DaFare {
    pub id: i64,
    pub testo: String,
    pub fatta_il: Option<String>,
}

/// Le voci, una per riga: "latte\npane" sono due voci (C20: si scrive come
/// viene).
pub fn voci_scritte(testo: &str) -> Vec<String> {
    testo
        .lines()
        .map(|riga| riga.trim().trim_start_matches(['-', '•', '*']).trim())
        .filter(|riga| !riga.is_empty())
        .map(str::to_string)
        .collect()
}

pub async fn aggiungi_da_fare(
    pool: &SqlitePool,
    utente_id: i64,
    spazio_id: Option<i64>,
    testo: &str,
) -> anyhow::Result<usize> {
    let voci = voci_scritte(testo);
    for voce in &voci {
        sqlx::query("INSERT INTO cose_da_fare (utente_id, spazio_id, testo) VALUES (?, ?, ?)")
            .bind(utente_id)
            .bind(spazio_id)
            .bind(voce)
            .execute(pool)
            .await
            .context("Impossibile salvare la cosa da fare")?;
    }
    Ok(voci.len())
}

/// Prima quelle da fare (nell'ordine in cui sono state scritte), poi le
/// fatte.
pub async fn cose_da_fare(pool: &SqlitePool, utente_id: i64) -> Vec<DaFare> {
    sqlx::query_as(
        "SELECT id, testo, fatta_il FROM cose_da_fare WHERE utente_id = ? \
         ORDER BY fatta_il IS NOT NULL, id",
    )
    .bind(utente_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

async fn leggi_da_fare(pool: &SqlitePool, utente_id: i64, id: i64) -> Option<DaFare> {
    sqlx::query_as("SELECT id, testo, fatta_il FROM cose_da_fare WHERE id = ? AND utente_id = ?")
        .bind(id)
        .bind(utente_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn inverti_da_fare(pool: &SqlitePool, utente_id: i64, id: i64, adesso: NaiveDateTime) {
    let _ = sqlx::query(
        "UPDATE cose_da_fare SET fatta_il = CASE WHEN fatta_il IS NULL THEN ? ELSE NULL END \
         WHERE id = ? AND utente_id = ?",
    )
    .bind(scrivi_ora_db(adesso))
    .bind(id)
    .bind(utente_id)
    .execute(pool)
    .await;
}

async fn elimina_da_fare(pool: &SqlitePool, utente_id: i64, id: i64) -> bool {
    sqlx::query("DELETE FROM cose_da_fare WHERE id = ? AND utente_id = ?")
        .bind(id)
        .bind(utente_id)
        .execute(pool)
        .await
        .map(|esito| esito.rows_affected() > 0)
        .unwrap_or(false)
}

async fn togli_le_fatte(pool: &SqlitePool, utente_id: i64) -> u64 {
    sqlx::query("DELETE FROM cose_da_fare WHERE utente_id = ? AND fatta_il IS NOT NULL")
        .bind(utente_id)
        .execute(pool)
        .await
        .map(|esito| esito.rows_affected())
        .unwrap_or(0)
}

async fn cambia_testo_da_fare(pool: &SqlitePool, utente_id: i64, id: i64, testo: &str) -> bool {
    sqlx::query("UPDATE cose_da_fare SET testo = ? WHERE id = ? AND utente_id = ?")
        .bind(testo.trim())
        .bind(id)
        .bind(utente_id)
        .execute(pool)
        .await
        .map(|esito| esito.rows_affected() > 0)
        .unwrap_or(false)
}

// ===========================================================================
// Il motore.
// ===========================================================================

/// Gli avvisi delle scadenze di oggi: uno al giorno al massimo per
/// scadenza, all'ora degli avvisi di chi l'ha scritta.
pub async fn controlla_scadenze(
    bot: &Bot,
    pool: &SqlitePool,
    adesso: NaiveDateTime,
) -> anyhow::Result<()> {
    let oggi = adesso.date();
    let lontano = calendario::format_date(oggi + Duration::days(31));
    let tutte: Vec<(i64, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, utente_id, testo, data, priorita FROM scadenze \
         WHERE stato = 'aperta' AND data <= ? ORDER BY utente_id, data, id",
    )
    .bind(&lontano)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le scadenze")?;
    for (id, utente_id, testo, data, priorita) in tutte {
        let Some(giorno) = calendario::parse_date(&data) else {
            continue;
        };
        let priorita = Priorita::da_token(&priorita).unwrap_or(Priorita::Media);
        let giorni = (giorno - oggi).num_days();
        if !tocca_oggi(priorita, giorni) {
            continue;
        }
        let ora = regole(pool, utente_id).await.ora_avvisi_scadenze;
        if scrivi_ora_db(adesso)[11..] < ora[..] {
            continue;
        }
        if !acceso(pool, utente_id, None).await {
            continue;
        }
        let quando = quanto_manca(giorni);
        let stato = if giorni >= 0 {
            format!(
                "{} Scade {quando}, {}.",
                priorita.emoji(),
                calendario::display_date(&data)
            )
        } else {
            format!(
                "⚠️ {} {}: era {}.",
                priorita.emoji(),
                primo_maiuscolo(&quando),
                calendario::display_date(&data)
            )
        };
        manda(
            bot,
            pool,
            Avviso {
                utente_id,
                chiave: format!("scadenza:{id}:{}", calendario::format_date(oggi)),
                messaggio: format!(
                    "📅 {testo}\n\n{stato}\n\n✅ Fatto la toglie: niente più avvisi."
                ),
                testo: format!("📅 {testo}"),
            },
        )
        .await;
    }
    Ok(())
}

/// `✅ Fatto` su un avviso di scadenza.
pub async fn fatto_da_avviso(
    pool: &SqlitePool,
    utente_id: i64,
    chiave: &str,
    adesso: NaiveDateTime,
) {
    let id = chiave
        .strip_prefix("scadenza:")
        .and_then(|resto| resto.split(':').next())
        .and_then(|id| id.parse::<i64>().ok());
    if let Some(id) = id {
        segna_fatta(pool, utente_id, id, adesso).await;
    }
}

// ===========================================================================
// Schermate.
// ===========================================================================

fn con_avviso(avviso: Option<&str>, testo: &str) -> String {
    match avviso {
        Some(avviso) => format!("{avviso}\n\n{testo}"),
        None => testo.to_string(),
    }
}

/// "Che cos'è?" dopo il testo di un promemoria nuovo.
pub async fn chiedi_tipo(bot: &Bot, chat_id: ChatId, testo: &str) -> ResponseResult<()> {
    aspetta(
        chat_id.0,
        Attesa::NuovoTipo {
            testo: testo.to_string(),
        },
    );
    bot.send_message(
        chat_id,
        format!(
            "Che cos'è?\n\n«{testo}»\n\n⏰ A un'ora precisa: ti scrivo a quell'ora.\n📅 Una scadenza: una data entro cui farlo, con gli avvisi che si infittiscono avvicinandosi.\n📝 Da fare: senza data, nella lista delle cose da fare."
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button("⏰ A un'ora precisa", "remind:tipo:orario")],
        vec![button("📅 Una scadenza", "remind:tipo:scadenza")],
        vec![button("📝 Da fare, senza data", "remind:tipo:dafare")],
        annulla_row(),
    ]))
    .await?;
    Ok(())
}

async fn chiedi_data(
    bot: &Bot,
    chat_id: ChatId,
    testo: &str,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        con_avviso(
            avviso,
            &format!(
                "📅 Entro quando?\n\n«{testo}»\n\nScegli qui sotto, oppure scrivilo: 15/10, 31 dicembre, fra 2 settimane, venerdì."
            ),
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![
            button("Fra una settimana", "remind:scad:entro:7"),
            button("Fra un mese", "remind:scad:entro:30"),
        ],
        annulla_row(),
    ]))
    .await?;
    Ok(())
}

async fn chiedi_priorita(
    bot: &Bot,
    chat_id: ChatId,
    testo: &str,
    data: &str,
    destinazione: &str,
) -> ResponseResult<()> {
    let righe: Vec<String> = PRIORITA
        .iter()
        .map(|p| format!("{} {}: {}.", p.emoji(), p.nome(), p.spiegazione()))
        .collect();
    let mut rows: Vec<Vec<InlineKeyboardButton>> = PRIORITA
        .iter()
        .map(|p| {
            vec![button(
                format!("{} {}", p.emoji(), p.nome()),
                format!("remind:scad:prio:{destinazione}:{}", p.token()),
            )]
        })
        .collect();
    rows.push(annulla_row());
    bot.send_message(
        chat_id,
        format!(
            "🎚 Quanto è importante?\n\n«{testo}», entro {}.\n\n{}",
            calendario::display_date(data),
            righe.join("\n")
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

pub async fn mostra_scadenze(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    pagina: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let oggi = adesso_locale(pool).await.date();
    let tutte = scadenze_aperte(pool, utente_id).await;
    let totale = tutte.len() as i64;
    let pagina = liste::pagina_valida(pagina, totale);
    let mut testo = liste::intestazione("📅 Scadenze", totale, pagina);
    if tutte.is_empty() {
        testo.push_str(
            "\n\nNessuna scadenza aperta.\nCreane una con ➕ Nuovo promemoria → 📅 Una scadenza.",
        );
    } else {
        testo.push_str("\n\nPrima quelle più vicine. 🔴 alta · 🟡 media · 🟢 bassa.");
    }
    let mut rows: Vec<Vec<InlineKeyboardButton>> = tutte
        .iter()
        .skip(liste::scarto(pagina) as usize)
        .take(liste::VOCI_PER_PAGINA)
        .map(|scadenza| {
            vec![button(
                format!(
                    "{} {} · {}",
                    scadenza.priorita().emoji(),
                    liste::tronca(&scadenza.testo, 24),
                    quanto_manca(scadenza.giorni(oggi))
                ),
                format!("remind:scad:view:{}", scadenza.id),
            )]
        })
        .collect();
    if let Some(riga) = liste::riga_paginazione_da_totale(pagina, totale, "remind:noop", |p| {
        format!("remind:scad:list:{p}")
    }) {
        rows.push(riga);
    }
    rows.push(nav_row("remind:menu"));
    bot.send_message(chat_id, con_avviso(avviso, &testo))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_scadenza(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let Some(scadenza) = leggi_scadenza(pool, utente_id, id).await else {
        return mostra_scadenze(
            bot,
            chat_id,
            pool,
            0,
            Some("⚠️ Questa scadenza non c'è più."),
        )
        .await;
    };
    let oggi = adesso_locale(pool).await.date();
    let priorita = scadenza.priorita();
    let stato = if scadenza.stato == "fatta" {
        "\n✅ Fatta: niente più avvisi.".to_string()
    } else {
        String::new()
    };
    let testo = format!(
        "📅 {}\n\n🗓 Entro {} ({}){stato}\n{} Priorità {}: {}.",
        scadenza.testo,
        calendario::display_date(&scadenza.data),
        quanto_manca(scadenza.giorni(oggi)),
        priorita.emoji(),
        priorita.nome().to_lowercase(),
        priorita.spiegazione()
    );
    let mut rows = Vec::new();
    if scadenza.stato == "aperta" {
        rows.push(vec![button("✅ Fatta", format!("remind:scad:done:{id}"))]);
    }
    rows.push(vec![
        button("✏️ Testo", format!("remind:scad:text:{id}")),
        button("🗓 Data", format!("remind:scad:date:{id}")),
    ]);
    rows.push(vec![
        button("🎚 Priorità", format!("remind:scad:prioask:{id}")),
        button("🗑️ Elimina", format!("remind:scad:del:ask:{id}")),
    ]);
    rows.push(nav_row("remind:scad:list:0"));
    bot.send_message(chat_id, con_avviso(avviso, &testo))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

pub async fn mostra_da_fare(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let voci = cose_da_fare(pool, utente_id).await;
    let fatte = voci.iter().filter(|voce| voce.fatta_il.is_some()).count();
    let mut testo = "📝 Da fare".to_string();
    if voci.is_empty() {
        testo.push_str("\n\nNon c'è niente da fare. Aggiungi qualcosa con ➕ Aggiungi.");
    } else {
        testo.push_str(&format!(
            "\n\n{} da fare, {fatte} fatte. Tocca una voce per spuntarla o toglierle la spunta.",
            voci.len() - fatte
        ));
    }
    let mut rows: Vec<Vec<InlineKeyboardButton>> = voci
        .iter()
        .take(30)
        .map(|voce| {
            let segno = if voce.fatta_il.is_some() {
                "✅"
            } else {
                "☐"
            };
            vec![button(
                format!("{segno} {}", liste::tronca(&voce.testo, 34)),
                format!("remind:todo:flip:{}", voce.id),
            )]
        })
        .collect();
    let mut azioni = vec![button("➕ Aggiungi", "remind:todo:add")];
    if !voci.is_empty() {
        azioni.push(button("✏️ Modifica", "remind:todo:pick"));
    }
    rows.push(azioni);
    if fatte > 0 {
        rows.push(vec![button(
            format!("🧹 Togli le fatte ({fatte})"),
            "remind:todo:clean:ask",
        )]);
    }
    rows.push(nav_row("remind:menu"));
    bot.send_message(chat_id, con_avviso(avviso, &testo))
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn chiedi_voci(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    aspetta(chat_id.0, Attesa::DaFareNuove);
    bot.send_message(
        chat_id,
        "📝 Cosa c'è da fare?\n\nScrivilo qui sotto. Più cose insieme: una per riga.",
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", "remind:todo:list"),
        button("🏠 Menù principale", "menu:main"),
    ]]))
    .await?;
    Ok(())
}

async fn mostra_scelta_da_modificare(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
) -> ResponseResult<()> {
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let voci = cose_da_fare(pool, utente_id).await;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = voci
        .iter()
        .take(30)
        .map(|voce| {
            vec![button(
                format!("✏️ {}", liste::tronca(&voce.testo, 34)),
                format!("remind:todo:view:{}", voce.id),
            )]
        })
        .collect();
    rows.push(nav_row("remind:todo:list"));
    bot.send_message(chat_id, "✏️ Quale voce?")
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_voce(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    id: i64,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, _)) = utente_corrente() else {
        return Ok(());
    };
    let Some(voce) = leggi_da_fare(pool, utente_id, id).await else {
        return mostra_da_fare(bot, chat_id, pool, Some("⚠️ Questa voce non c'è più.")).await;
    };
    let stato = if voce.fatta_il.is_some() {
        "✅ fatta"
    } else {
        "☐ da fare"
    };
    let rows = vec![
        vec![button("✏️ Testo", format!("remind:todo:text:{id}"))],
        vec![button(
            "📅 Dagli una scadenza",
            format!("remind:todo:toscad:{id}"),
        )],
        vec![button("🗑️ Elimina", format!("remind:todo:del:ask:{id}"))],
        nav_row("remind:todo:list"),
    ];
    bot.send_message(
        chat_id,
        con_avviso(avviso, &format!("📝 {}\n\n{stato}", voce.testo)),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

fn conferma(testo: String, si: String, annulla: String) -> (String, InlineKeyboardMarkup) {
    (
        testo,
        InlineKeyboardMarkup::new(vec![
            vec![button("✅ Sì, elimina", si)],
            vec![
                button("❌ Annulla", annulla),
                button("🏠 Menù principale", "menu:main"),
            ],
        ]),
    )
}

// ===========================================================================
// Ingressi.
// ===========================================================================

/// Salva la scadenza della bozza e mostra la sua scheda.
async fn salva_bozza(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    testo: &str,
    data: &str,
    da_fare: Option<i64>,
    priorita: Priorita,
) -> ResponseResult<()> {
    chiudi_attesa(chat_id.0);
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(());
    };
    let Some(giorno) = calendario::parse_date(data) else {
        return mostra_scadenze(bot, chat_id, pool, 0, None).await;
    };
    match crea_scadenza(pool, utente_id, spazio_id, testo, giorno, priorita).await {
        Ok(id) => {
            if let Some(voce) = da_fare {
                elimina_da_fare(pool, utente_id, voce).await;
            }
            let avviso = format!("✅ Scadenza salvata: {}.", priorita.spiegazione());
            mostra_scadenza(bot, chat_id, pool, id, Some(&avviso)).await
        }
        Err(errore) => {
            tracing::warn!(?errore, "Scadenza non salvata");
            mostra_scadenze(
                bot,
                chat_id,
                pool,
                0,
                Some("⚠️ Non sono riuscito a salvarla."),
            )
            .await
        }
    }
}

/// Una data scelta per la bozza o per una scadenza che c'è già.
async fn data_scelta(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    giorno: NaiveDate,
) -> ResponseResult<()> {
    let oggi = adesso_locale(pool).await.date();
    match attesa(chat_id.0) {
        Some(Attesa::NuovaScadenzaData { testo, da_fare }) => {
            if giorno < oggi {
                return chiedi_data(
                    bot,
                    chat_id,
                    &testo,
                    Some("⚠️ È già passata: scegli una data che deve ancora venire."),
                )
                .await;
            }
            let data = calendario::format_date(giorno);
            aspetta(
                chat_id.0,
                Attesa::NuovaScadenzaPriorita {
                    testo: testo.clone(),
                    data: data.clone(),
                    da_fare,
                },
            );
            chiedi_priorita(bot, chat_id, &testo, &data, "new").await
        }
        Some(Attesa::ScadenzaData { id }) => {
            let Some((utente_id, _)) = utente_corrente() else {
                return Ok(());
            };
            let avviso = if aggiorna_scadenza(
                pool,
                utente_id,
                id,
                "data",
                &calendario::format_date(giorno),
            )
            .await
            {
                "✅ Data cambiata."
            } else {
                "⚠️ Non sono riuscito a cambiarla."
            };
            mostra_scadenza(bot, chat_id, pool, id, Some(avviso)).await
        }
        _ => mostra_scadenze(bot, chat_id, pool, 0, None).await,
    }
}

/// Il testo scritto per una delle attese di questo file. `false` se
/// l'attesa non è di qui.
pub(super) async fn gestisci_testo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    attesa_aperta: &Attesa,
    scritto: &str,
) -> ResponseResult<bool> {
    let Some((utente_id, spazio_id)) = utente_corrente() else {
        return Ok(false);
    };
    match attesa_aperta {
        Attesa::NuovoTipo { testo } => {
            bot.send_message(chat_id, "Scegli con i pulsanti che cos'è.")
                .await?;
            chiedi_tipo(bot, chat_id, testo).await?;
        }
        Attesa::NuovaScadenzaData { testo, .. } => {
            let adesso = adesso_locale(pool).await;
            match leggi_data(scritto, adesso) {
                Some(giorno) => data_scelta(bot, chat_id, pool, giorno).await?,
                None => chiedi_data(bot, chat_id, testo, Some("⚠️ Non ho capito la data.")).await?,
            }
        }
        Attesa::ScadenzaData { id } => {
            let adesso = adesso_locale(pool).await;
            match leggi_data(scritto, adesso) {
                Some(giorno) => data_scelta(bot, chat_id, pool, giorno).await?,
                None => {
                    let testo = leggi_scadenza(pool, utente_id, *id)
                        .await
                        .map(|s| s.testo)
                        .unwrap_or_default();
                    chiedi_data(bot, chat_id, &testo, Some("⚠️ Non ho capito la data.")).await?
                }
            }
        }
        Attesa::NuovaScadenzaPriorita { testo, data, .. } => {
            bot.send_message(chat_id, "🎚 Scegli con i pulsanti quanto è importante.")
                .await?;
            chiedi_priorita(bot, chat_id, testo, data, "new").await?;
        }
        Attesa::ScadenzaTesto { id } => {
            let avviso = if aggiorna_scadenza(pool, utente_id, *id, "testo", scritto).await {
                "✅ Testo cambiato."
            } else {
                "⚠️ Non sono riuscito a cambiarlo."
            };
            mostra_scadenza(bot, chat_id, pool, *id, Some(avviso)).await?;
        }
        Attesa::DaFareNuove => {
            let avviso = match aggiungi_da_fare(pool, utente_id, spazio_id, scritto).await {
                Ok(0) => "⚠️ Non c'era niente da aggiungere.".to_string(),
                Ok(1) => "✅ Aggiunta.".to_string(),
                Ok(n) => format!("✅ Aggiunte {n} voci."),
                Err(_) => "⚠️ Non sono riuscito a salvarle.".to_string(),
            };
            mostra_da_fare(bot, chat_id, pool, Some(&avviso)).await?;
        }
        Attesa::DaFareTesto { id } => {
            let avviso = if cambia_testo_da_fare(pool, utente_id, *id, scritto).await {
                "✅ Testo cambiato."
            } else {
                "⚠️ Non sono riuscito a cambiarlo."
            };
            mostra_voce(bot, chat_id, pool, *id, Some(avviso)).await?;
        }
        Attesa::OraAvvisiScadenze => match crate::modules::turni::valida_orario(scritto) {
            Ok(ora) => imposta_ora_avvisi(bot, chat_id, pool, utente_id, &ora).await?,
            Err(_) => {
                chiedi_ora_avvisi(
                    bot,
                    chat_id,
                    Some("⚠️ Non è un'ora: scrivila così, 7:30 oppure 18."),
                )
                .await?
            }
        },
        _ => return Ok(false),
    }
    Ok(true)
}

pub async fn chiedi_ora_avvisi(
    bot: &Bot,
    chat_id: ChatId,
    avviso: Option<&str>,
) -> ResponseResult<()> {
    aspetta(chat_id.0, Attesa::OraAvvisiScadenze);
    let ore: Vec<InlineKeyboardButton> = ["08:00", "09:00", "12:00", "18:00"]
        .iter()
        .map(|ora| button(*ora, format!("remind:scad:ora:{}", ora.replace(':', ""))))
        .collect();
    bot.send_message(
        chat_id,
        con_avviso(
            avviso,
            "📅 Avvisi delle scadenze\n\nA che ora ti scrivo, nei giorni in cui tocca? Scegli o scrivi l'ora (es. 7:30).",
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![ore, nav_row("remind:auto")]))
    .await?;
    Ok(())
}

async fn imposta_ora_avvisi(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    utente_id: i64,
    ora: &str,
) -> ResponseResult<()> {
    let mut nuove = regole(pool, utente_id).await;
    nuove.ora_avvisi_scadenze = ora.to_string();
    let avviso = match salva_regole(pool, utente_id, &nuove).await {
        Ok(()) => format!("✅ Gli avvisi delle scadenze arrivano alle {ora}."),
        Err(_) => "⚠️ Non sono riuscito a salvarlo.".to_string(),
    };
    super::mostra_automatici(bot, chat_id, pool, Some(&avviso)).await
}

/// I pulsanti `remind:tipo:`, `remind:scad:`, `remind:todo:`. `false` se
/// non sono di qui.
pub async fn gestisci_pulsante(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    utente_id: i64,
    resto: &str,
) -> ResponseResult<bool> {
    if let Some(tipo) = resto.strip_prefix("tipo:") {
        let Some(Attesa::NuovoTipo { testo }) = attesa(chat_id.0) else {
            super::mostra_menu(
                bot,
                chat_id,
                pool,
                Some("⚠️ Questo promemoria non era più in preparazione: ricomincia da ➕ Nuovo promemoria."),
            )
            .await?;
            return Ok(true);
        };
        match tipo {
            "orario" => {
                aspetta(
                    chat_id.0,
                    Attesa::NuovoQuando {
                        testo: testo.clone(),
                    },
                );
                super::chiedi_quando(bot, chat_id, pool, &testo, None).await?;
            }
            "scadenza" => {
                aspetta(
                    chat_id.0,
                    Attesa::NuovaScadenzaData {
                        testo: testo.clone(),
                        da_fare: None,
                    },
                );
                chiedi_data(bot, chat_id, &testo, None).await?;
            }
            "dafare" => {
                let (_, spazio_id) = utente_corrente().unwrap_or((utente_id, None));
                chiudi_attesa(chat_id.0);
                let avviso = match aggiungi_da_fare(pool, utente_id, spazio_id, &testo).await {
                    Ok(_) => "✅ Aggiunta alle cose da fare.",
                    Err(_) => "⚠️ Non sono riuscito a salvarla.",
                };
                mostra_da_fare(bot, chat_id, pool, Some(avviso)).await?;
            }
            _ => {}
        }
        return Ok(true);
    }
    if let Some(resto) = resto.strip_prefix("scad:") {
        gestisci_scadenze(bot, chat_id, pool, utente_id, resto).await?;
        return Ok(true);
    }
    if let Some(resto) = resto.strip_prefix("todo:") {
        gestisci_da_fare(bot, chat_id, pool, utente_id, resto).await?;
        return Ok(true);
    }
    Ok(false)
}

async fn gestisci_scadenze(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    utente_id: i64,
    resto: &str,
) -> ResponseResult<()> {
    let id_dopo = |prefisso: &str| -> Option<i64> { resto.strip_prefix(prefisso)?.parse().ok() };
    if let Some(pagina) = resto.strip_prefix("list:") {
        return mostra_scadenze(bot, chat_id, pool, pagina.parse().unwrap_or(0), None).await;
    }
    if let Some(giorni) = resto.strip_prefix("entro:") {
        let Ok(giorni) = giorni.parse::<i64>() else {
            return Ok(());
        };
        let oggi = adesso_locale(pool).await.date();
        return data_scelta(bot, chat_id, pool, oggi + Duration::days(giorni)).await;
    }
    if let Some(valori) = resto.strip_prefix("prio:") {
        let Some((destinazione, token)) = valori.split_once(':') else {
            return Ok(());
        };
        let Some(priorita) = Priorita::da_token(token) else {
            return Ok(());
        };
        if destinazione == "new" {
            return match attesa(chat_id.0) {
                Some(Attesa::NuovaScadenzaPriorita {
                    testo,
                    data,
                    da_fare,
                }) => salva_bozza(bot, chat_id, pool, &testo, &data, da_fare, priorita).await,
                _ => mostra_scadenze(bot, chat_id, pool, 0, None).await,
            };
        }
        let Ok(id) = destinazione.parse::<i64>() else {
            return Ok(());
        };
        let avviso = if aggiorna_scadenza(pool, utente_id, id, "priorita", priorita.token()).await {
            "✅ Priorità cambiata."
        } else {
            "⚠️ Non sono riuscito a cambiarla."
        };
        return mostra_scadenza(bot, chat_id, pool, id, Some(avviso)).await;
    }
    if let Some(ora) = resto.strip_prefix("ora:") {
        if ora.len() == 4 && ora.bytes().all(|b| b.is_ascii_digit()) {
            let ora = format!("{}:{}", &ora[..2], &ora[2..]);
            return imposta_ora_avvisi(bot, chat_id, pool, utente_id, &ora).await;
        }
        return Ok(());
    }
    if resto == "orask" {
        return chiedi_ora_avvisi(bot, chat_id, None).await;
    }
    if let Some(id) = id_dopo("view:") {
        return mostra_scadenza(bot, chat_id, pool, id, None).await;
    }
    if let Some(id) = id_dopo("done:") {
        let adesso = adesso_locale(pool).await;
        let avviso = if segna_fatta(pool, utente_id, id, adesso).await {
            "✅ Fatta: niente più avvisi."
        } else {
            "⚠️ Non sono riuscito a segnarla."
        };
        return mostra_scadenze(bot, chat_id, pool, 0, Some(avviso)).await;
    }
    if let Some(id) = id_dopo("text:") {
        let Some(scadenza) = leggi_scadenza(pool, utente_id, id).await else {
            return mostra_scadenze(bot, chat_id, pool, 0, None).await;
        };
        aspetta(chat_id.0, Attesa::ScadenzaTesto { id });
        bot.send_message(
            chat_id,
            format!(
                "✏️ Testo della scadenza\n\nAdesso: «{}»\n\nScrivi quello nuovo.",
                scadenza.testo
            ),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button("❌ Annulla", format!("remind:scad:view:{id}")),
            button("🏠 Menù principale", "menu:main"),
        ]]))
        .await?;
        return Ok(());
    }
    if let Some(id) = id_dopo("date:") {
        let Some(scadenza) = leggi_scadenza(pool, utente_id, id).await else {
            return mostra_scadenze(bot, chat_id, pool, 0, None).await;
        };
        aspetta(chat_id.0, Attesa::ScadenzaData { id });
        return chiedi_data(bot, chat_id, &scadenza.testo, None).await;
    }
    if let Some(id) = id_dopo("prioask:") {
        let Some(scadenza) = leggi_scadenza(pool, utente_id, id).await else {
            return mostra_scadenze(bot, chat_id, pool, 0, None).await;
        };
        return chiedi_priorita(
            bot,
            chat_id,
            &scadenza.testo,
            &scadenza.data,
            &id.to_string(),
        )
        .await;
    }
    if let Some(id) = id_dopo("del:ask:") {
        let Some(scadenza) = leggi_scadenza(pool, utente_id, id).await else {
            return mostra_scadenze(bot, chat_id, pool, 0, None).await;
        };
        let (testo, tastiera) = conferma(
            format!(
                "⚠️ Eliminare la scadenza «{}» definitivamente? Non si può recuperare.",
                scadenza.testo
            ),
            format!("remind:scad:del:yes:{id}"),
            format!("remind:scad:view:{id}"),
        );
        bot.send_message(chat_id, testo)
            .reply_markup(tastiera)
            .await?;
        return Ok(());
    }
    if let Some(id) = id_dopo("del:yes:") {
        let avviso = if elimina_scadenza(pool, utente_id, id).await {
            "✅ Scadenza eliminata."
        } else {
            "⚠️ Non sono riuscito a eliminarla."
        };
        return mostra_scadenze(bot, chat_id, pool, 0, Some(avviso)).await;
    }
    Ok(())
}

async fn gestisci_da_fare(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    utente_id: i64,
    resto: &str,
) -> ResponseResult<()> {
    let id_dopo = |prefisso: &str| -> Option<i64> { resto.strip_prefix(prefisso)?.parse().ok() };
    match resto {
        "list" => return mostra_da_fare(bot, chat_id, pool, None).await,
        "add" => return chiedi_voci(bot, chat_id).await,
        "pick" => return mostra_scelta_da_modificare(bot, chat_id, pool).await,
        "clean:ask" => {
            let fatte = cose_da_fare(pool, utente_id)
                .await
                .iter()
                .filter(|voce| voce.fatta_il.is_some())
                .count();
            let (testo, tastiera) = conferma(
                format!(
                    "⚠️ Togliere {} definitivamente? Non si può recuperare.",
                    if fatte == 1 {
                        "la voce fatta".to_string()
                    } else {
                        format!("le {fatte} voci fatte")
                    }
                ),
                "remind:todo:clean:yes".to_string(),
                "remind:todo:list".to_string(),
            );
            bot.send_message(chat_id, testo)
                .reply_markup(tastiera)
                .await?;
            return Ok(());
        }
        "clean:yes" => {
            let tolte = togli_le_fatte(pool, utente_id).await;
            let avviso = match tolte {
                0 => "Non c'erano voci fatte.".to_string(),
                1 => "✅ Tolta la voce fatta.".to_string(),
                n => format!("✅ Tolte {n} voci fatte."),
            };
            return mostra_da_fare(bot, chat_id, pool, Some(&avviso)).await;
        }
        _ => {}
    }
    if let Some(id) = id_dopo("flip:") {
        let adesso = adesso_locale(pool).await;
        inverti_da_fare(pool, utente_id, id, adesso).await;
        return mostra_da_fare(bot, chat_id, pool, None).await;
    }
    if let Some(id) = id_dopo("view:") {
        return mostra_voce(bot, chat_id, pool, id, None).await;
    }
    if let Some(id) = id_dopo("text:") {
        let Some(voce) = leggi_da_fare(pool, utente_id, id).await else {
            return mostra_da_fare(bot, chat_id, pool, None).await;
        };
        aspetta(chat_id.0, Attesa::DaFareTesto { id });
        bot.send_message(
            chat_id,
            format!("✏️ Adesso: «{}»\n\nScrivi il testo nuovo.", voce.testo),
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button("❌ Annulla", format!("remind:todo:view:{id}")),
            button("🏠 Menù principale", "menu:main"),
        ]]))
        .await?;
        return Ok(());
    }
    if let Some(id) = id_dopo("toscad:") {
        let Some(voce) = leggi_da_fare(pool, utente_id, id).await else {
            return mostra_da_fare(bot, chat_id, pool, None).await;
        };
        aspetta(
            chat_id.0,
            Attesa::NuovaScadenzaData {
                testo: voce.testo.clone(),
                da_fare: Some(id),
            },
        );
        return chiedi_data(bot, chat_id, &voce.testo, None).await;
    }
    if let Some(id) = id_dopo("del:ask:") {
        let Some(voce) = leggi_da_fare(pool, utente_id, id).await else {
            return mostra_da_fare(bot, chat_id, pool, None).await;
        };
        let (testo, tastiera) = conferma(
            format!(
                "⚠️ Eliminare «{}» definitivamente? Non si può recuperare.",
                voce.testo
            ),
            format!("remind:todo:del:yes:{id}"),
            format!("remind:todo:view:{id}"),
        );
        bot.send_message(chat_id, testo)
            .reply_markup(tastiera)
            .await?;
        return Ok(());
    }
    if let Some(id) = id_dopo("del:yes:") {
        let avviso = if elimina_da_fare(pool, utente_id, id).await {
            "✅ Voce eliminata."
        } else {
            "⚠️ Non sono riuscito a eliminarla."
        };
        return mostra_da_fare(bot, chat_id, pool, Some(avviso)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piu_e_importante_piu_avvisi_arrivano() {
        let quanti = |p: Priorita| (-10..=40).filter(|g| tocca_oggi(p, *g)).count();
        assert_eq!(quanti(Priorita::Bassa), 2);
        assert_eq!(quanti(Priorita::Media), 5);
        assert_eq!(quanti(Priorita::Alta), 14);
        assert!(tocca_oggi(Priorita::Alta, 2));
        assert!(!tocca_oggi(Priorita::Media, 2));
        assert!(tocca_oggi(Priorita::Media, -1));
        assert!(!tocca_oggi(Priorita::Media, -2));
        assert!(tocca_oggi(Priorita::Alta, -7));
        assert!(!tocca_oggi(Priorita::Alta, -8));
        assert!(!tocca_oggi(Priorita::Bassa, -1));
    }

    #[test]
    fn la_priorita_si_spiega_da_sola() {
        assert_eq!(
            Priorita::Bassa.spiegazione(),
            "avvisi 7 giorni prima e il giorno stesso"
        );
        assert_eq!(
            Priorita::Media.spiegazione(),
            "avvisi 30, 7 e 1 giorni prima, il giorno stesso e il giorno dopo se non è fatta"
        );
        assert_eq!(
            Priorita::Alta.spiegazione(),
            "avvisi 30, 14, 7, 3, 2 e 1 giorni prima, il giorno stesso, poi ogni giorno per 7 giorni finché non è fatta"
        );
    }

    #[test]
    fn quanto_manca_si_dice_corto() {
        assert_eq!(quanto_manca(0), "oggi");
        assert_eq!(quanto_manca(1), "domani");
        assert_eq!(quanto_manca(12), "fra 12 giorni");
        assert_eq!(quanto_manca(-1), "scaduta ieri");
        assert_eq!(quanto_manca(-4), "scaduta da 4 giorni");
    }

    #[test]
    fn piu_voci_una_per_riga() {
        assert_eq!(
            voci_scritte("latte\n- pane\n\n• chiamare il idraulico  "),
            vec!["latte", "pane", "chiamare il idraulico"]
        );
        assert!(voci_scritte("  \n ").is_empty());
    }
}
