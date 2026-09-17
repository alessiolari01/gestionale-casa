//! Negozi, prezzi, prodotti preferiti e codice a barre.
//!
//! Consegna B del 17 settembre 2026, concordata con Alessio subito dopo la
//! consegna A. Comportamento in `docs/moduli/mercato.md`, schema in
//! `docs/database.md` (Step 7.4sexies).
//!
//! Il principio, deciso con lui: **tutto facoltativo**. Chi non registra un
//! prezzo vede esattamente la lista della spesa di prima; chi li registra
//! ottiene la stima della spesa e il confronto fra i supermercati che ha
//! scelto lui — non fra tutte le catene d'Italia.
//!
//! Due fonti esterne, approvate da Alessio:
//! - **Open Food Facts** per leggere un prodotto dal codice a barre;
//! - **Open Prices** per un prezzo di riferimento, presentato sempre come
//!   suggerimento di altri e mai come un prezzo visto da noi.
//!
//! Entrambe sono facoltative: se la rete non c'è, il bot lo dice e tutto il
//! resto continua a funzionare.
//!
//! Stessa divisione degli altri moduli: dominio puro, funzioni database, UI.

use anyhow::Context as _;
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

// Il wrapper che tiene una sola schermata attiva e aggiunge "💡 Migliora".
type Bot = crate::context_bot::ContextBot;

// ---------------------------------------------------------------------------
// Dominio puro
// ---------------------------------------------------------------------------

/// `1290` → `12,90 €`. I soldi stanno in centesimi interi: un `f64` di euro
/// perde i centesimi dopo qualche somma.
pub fn formatta_euro(centesimi: i64) -> String {
    let segno = if centesimi < 0 { "-" } else { "" };
    let valore = centesimi.abs();
    format!("{segno}{},{:02} €", valore / 100, valore % 100)
}

/// Legge un prezzo scritto a mano: `1,29`, `1.29`, `1`, `1,29 €`, `€ 1,29`.
/// Rifiuta zero, negativi e tutto ciò che non è un numero.
pub fn interpreta_prezzo(testo: &str) -> Option<i64> {
    let pulito: String = testo
        .trim()
        .replace('€', "")
        .replace("eur", "")
        .replace("EUR", "")
        .trim()
        .replace(',', ".");
    let valore: f64 = pulito.trim().parse().ok()?;
    if !valore.is_finite() || valore <= 0.0 {
        return None;
    }
    let centesimi = (valore * 100.0).round() as i64;
    (centesimi > 0).then_some(centesimi)
}

/// Il prezzo riportato a un'unità confrontabile: `2,50 € per 500 g` diventa
/// `5,00 € al kg`. `None` quando manca la quantità o l'unità non è fra
/// quelle note: meglio nessun confronto che un confronto sbagliato.
pub fn prezzo_al_riferimento(
    centesimi: i64,
    quantita: Option<f64>,
    unita: Option<&str>,
) -> Option<(i64, &'static str)> {
    let quantita = quantita?;
    if quantita <= 0.0 {
        return None;
    }
    let unita = unita?.trim().to_lowercase();
    let (per_unita_base, riferimento, fattore) = match unita.as_str() {
        "g" => (true, "kg", 1000.0),
        "kg" => (true, "kg", 1.0),
        "ml" => (true, "l", 1000.0),
        "l" => (true, "l", 1.0),
        "pz" | "conf" => (false, "pz", 1.0),
        _ => return None,
    };
    let quantita_in_riferimento = if per_unita_base {
        quantita / fattore
    } else {
        quantita
    };
    if quantita_in_riferimento <= 0.0 {
        return None;
    }
    let per_riferimento = (centesimi as f64 / quantita_in_riferimento).round() as i64;
    (per_riferimento > 0).then_some((per_riferimento, riferimento))
}

/// Una riga del confronto fra negozi.
#[derive(Debug, Clone, PartialEq)]
pub struct StimaNegozio {
    pub negozio_id: i64,
    pub nome: String,
    pub totale_centesimi: i64,
    /// Quante voci della lista hanno un prezzo noto in questo negozio, e
    /// quante sono in tutto: una stima su 3 voci di 20 non è una stima.
    pub voci_con_prezzo: usize,
    pub voci_totali: usize,
}

impl StimaNegozio {
    pub fn riga(&self) -> String {
        format!(
            "• {}: {} ({}/{} voci)",
            self.nome,
            formatta_euro(self.totale_centesimi),
            self.voci_con_prezzo,
            self.voci_totali
        )
    }
}

/// Ordina il confronto: prima chi copre più voci della lista, poi il totale
/// più basso. Un negozio che conosce due prodotti su venti non può "vincere"
/// solo perché la sua somma è piccola.
pub fn ordina_stime(stime: &mut [StimaNegozio]) {
    stime.sort_by(|a, b| {
        b.voci_con_prezzo
            .cmp(&a.voci_con_prezzo)
            .then(a.totale_centesimi.cmp(&b.totale_centesimi))
            .then(a.nome.cmp(&b.nome))
    });
}

/// La quantità dichiarata da Open Food Facts (`"300 g"`, `"1,5 l"`,
/// `"6 x 125 g"`) ridotta a numero e unità. `None` se non si capisce: meglio
/// chiedere all'utente che inventare una quantità.
pub fn interpreta_quantita_esterna(testo: &str) -> Option<(f64, String)> {
    let testo = testo.trim().to_lowercase().replace(',', ".");
    // "6 x 125 g" → il pezzo dopo la x è quello che interessa.
    let parte = testo.rsplit(['x', '×']).next()?.trim().to_string();
    let numero: String = parte
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if numero.is_empty() {
        return None;
    }
    let valore: f64 = numero.parse().ok()?;
    if !valore.is_finite() || valore <= 0.0 {
        return None;
    }
    let unita: String = parte[numero.len()..]
        .trim()
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect();
    let unita = match unita.as_str() {
        "g" | "gr" | "grammi" => "g",
        "kg" => "kg",
        "ml" => "ml",
        "l" | "lt" | "litri" => "l",
        "cl" => return Some((valore * 10.0, "ml".to_string())),
        "" => return None,
        _ => return None,
    };
    Some((valore, unita.to_string()))
}

/// Un codice a barre plausibile: 8, 12, 13 o 14 cifre (EAN-8, UPC, EAN-13,
/// ITF-14). Si controlla prima di chiamare Open Food Facts, per non fare una
/// richiesta di rete per un errore di battitura.
pub fn codice_a_barre_valido(testo: &str) -> Option<String> {
    let cifre: String = testo.chars().filter(|c| c.is_ascii_digit()).collect();
    matches!(cifre.len(), 8 | 12 | 13 | 14).then_some(cifre)
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, PartialEq)]
pub struct Negozio {
    pub id: i64,
    pub nome: String,
    /// `1` se l'utente l'ha scelto per il confronto.
    #[sqlx(default)]
    pub scelto: i64,
}

/// I negozi visibili: le catene comuni più quelli creati nello spazio
/// attivo. `scelto` dice se entrano nel confronto dell'utente.
pub async fn negozi_visibili(pool: &SqlitePool) -> anyhow::Result<Vec<Negozio>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    sqlx::query_as(
        "SELECT n.id, n.nome, \
                CASE WHEN nc.utente_id IS NULL THEN 0 ELSE 1 END AS scelto \
         FROM negozi n \
         LEFT JOIN negozi_confronto nc ON nc.negozio_id = n.id AND nc.utente_id = ? \
         WHERE n.attivo = 1 AND (n.spazio_id IS NULL OR n.spazio_id = ?) \
         ORDER BY scelto DESC, n.nome_normalizzato",
    )
    .bind(user_id)
    .bind(actor.spazio_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i negozi")
}

/// Solo quelli scelti per il confronto.
pub async fn negozi_scelti(pool: &SqlitePool) -> anyhow::Result<Vec<Negozio>> {
    Ok(negozi_visibili(pool)
        .await?
        .into_iter()
        .filter(|negozio| negozio.scelto == 1)
        .collect())
}

pub async fn negozio_per_id(pool: &SqlitePool, negozio_id: i64) -> anyhow::Result<Option<Negozio>> {
    sqlx::query_as("SELECT id, nome, 0 AS scelto FROM negozi WHERE id = ?")
        .bind(negozio_id)
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere il negozio")
}

/// Aggiunge o toglie un negozio dal confronto dell'utente. Ritorna lo stato
/// nuovo.
pub async fn cambia_scelta(pool: &SqlitePool, negozio_id: i64) -> anyhow::Result<bool> {
    let user_id = crate::identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    let gia: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM negozi_confronto WHERE utente_id = ? AND negozio_id = ?")
            .bind(user_id)
            .bind(negozio_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere la scelta")?;
    if gia.is_some() {
        sqlx::query("DELETE FROM negozi_confronto WHERE utente_id = ? AND negozio_id = ?")
            .bind(user_id)
            .bind(negozio_id)
            .execute(pool)
            .await
            .context("Impossibile togliere il negozio dal confronto")?;
        Ok(false)
    } else {
        sqlx::query("INSERT INTO negozi_confronto (utente_id, negozio_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(negozio_id)
            .execute(pool)
            .await
            .context("Impossibile aggiungere il negozio al confronto")?;
        Ok(true)
    }
}

/// Un negozio che non è fra le catene comuni ("Il fruttivendolo di via
/// Roma"): appartiene allo spazio attivo, e nasce già scelto.
pub async fn crea_negozio(pool: &SqlitePool, nome: &str) -> anyhow::Result<i64> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let nome = nome.trim();
    anyhow::ensure!(!nome.is_empty(), "Il nome non può essere vuoto");
    anyhow::ensure!(nome.chars().count() <= 60, "Il nome è troppo lungo");
    let normalizzato = nome.to_lowercase();
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM negozi WHERE nome_normalizzato = ? \
         AND (spazio_id IS NULL OR spazio_id = ?)",
    )
    .bind(&normalizzato)
    .bind(actor.spazio_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile cercare il negozio")?
    {
        return Ok(id);
    }
    let id = sqlx::query(
        "INSERT INTO negozi (spazio_id, nome, nome_normalizzato, creato_da_utente_id) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(actor.spazio_id)
    .bind(nome)
    .bind(&normalizzato)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile creare il negozio")?
    .last_insert_rowid();
    Ok(id)
}

/// Registra un prezzo visto. `quantita`/`unita` sono quelle della confezione
/// pagata, e servono al prezzo al chilo.
#[allow(clippy::too_many_arguments)]
pub async fn registra_prezzo(
    pool: &SqlitePool,
    negozio_id: i64,
    alimento_id: Option<i64>,
    prodotto_id: Option<i64>,
    descrizione: &str,
    prezzo_centesimi: i64,
    quantita: Option<f64>,
    unita: Option<&str>,
    fonte: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        prezzo_centesimi > 0,
        "Il prezzo deve essere maggiore di zero"
    );
    anyhow::ensure!(
        alimento_id.is_some() || prodotto_id.is_some(),
        "Un prezzo senza alimento né prodotto non si può confrontare"
    );
    let user_id = crate::identity::current_actor().utente_id;
    sqlx::query(
        "INSERT INTO prezzi_osservati \
         (negozio_id, alimento_id, prodotto_alimentare_id, descrizione, prezzo_centesimi, \
          quantita, unita_simbolo, fonte, utente_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(negozio_id)
    .bind(alimento_id)
    .bind(prodotto_id)
    .bind(descrizione.trim())
    .bind(prezzo_centesimi)
    .bind(quantita)
    .bind(unita)
    .bind(fonte)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile registrare il prezzo")?;
    Ok(())
}

/// L'ultimo prezzo visto per questo alimento (o prodotto) in questo negozio.
pub async fn ultimo_prezzo(
    pool: &SqlitePool,
    negozio_id: i64,
    alimento_id: Option<i64>,
    prodotto_id: Option<i64>,
) -> anyhow::Result<Option<(i64, Option<f64>, Option<String>)>> {
    sqlx::query_as(
        "SELECT prezzo_centesimi, quantita, unita_simbolo FROM prezzi_osservati \
         WHERE negozio_id = ? \
           AND ((? IS NOT NULL AND prodotto_alimentare_id = ?) \
                OR (? IS NOT NULL AND alimento_id = ?)) \
         ORDER BY rilevato_il DESC, id DESC LIMIT 1",
    )
    .bind(negozio_id)
    .bind(prodotto_id)
    .bind(prodotto_id)
    .bind(alimento_id)
    .bind(alimento_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'ultimo prezzo")
}

/// Il prodotto preferito per un alimento, se c'è.
pub async fn preferito_di(pool: &SqlitePool, alimento_id: i64) -> anyhow::Result<Option<i64>> {
    let Some(user_id) = crate::identity::current_actor().utente_id else {
        return Ok(None);
    };
    sqlx::query_scalar(
        "SELECT prodotto_alimentare_id FROM prodotti_preferiti \
         WHERE utente_id = ? AND alimento_id = ?",
    )
    .bind(user_id)
    .bind(alimento_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere il prodotto preferito")
}

/// Segna (o toglie) il preferito di un alimento. Ritorna `true` se da ora
/// è preferito.
pub async fn cambia_preferito(
    pool: &SqlitePool,
    alimento_id: i64,
    prodotto_id: i64,
) -> anyhow::Result<bool> {
    let user_id = crate::identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    if preferito_di(pool, alimento_id).await? == Some(prodotto_id) {
        sqlx::query("DELETE FROM prodotti_preferiti WHERE utente_id = ? AND alimento_id = ?")
            .bind(user_id)
            .bind(alimento_id)
            .execute(pool)
            .await
            .context("Impossibile togliere il preferito")?;
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO prodotti_preferiti (utente_id, alimento_id, prodotto_alimentare_id) \
         VALUES (?, ?, ?) \
         ON CONFLICT (utente_id, alimento_id) DO UPDATE SET \
           prodotto_alimentare_id = excluded.prodotto_alimentare_id, \
           scelto_il = excluded.scelto_il",
    )
    .bind(user_id)
    .bind(alimento_id)
    .bind(prodotto_id)
    .execute(pool)
    .await
    .context("Impossibile segnare il preferito")?;
    Ok(true)
}

/// Il prodotto con questo codice a barre, se il catalogo lo conosce già.
pub async fn prodotto_per_ean(pool: &SqlitePool, ean: &str) -> anyhow::Result<Option<i64>> {
    sqlx::query_scalar(
        "SELECT id FROM prodotti_alimentari WHERE codice_ean = ? AND attivo = 1 LIMIT 1",
    )
    .bind(ean)
    .fetch_optional(pool)
    .await
    .context("Impossibile cercare il codice a barre")
}

/// Crea il prodotto letto da Open Food Facts e lo lega all'alimento della
/// voce. L'unità arriva da `unita_misura`: se non c'è, il prodotto non si
/// crea (meglio niente che una confezione con l'unità sbagliata).
pub async fn crea_prodotto_da_esterno(
    pool: &SqlitePool,
    alimento_id: i64,
    prodotto: &ProdottoEsterno,
) -> anyhow::Result<i64> {
    let user_id = crate::identity::current_actor().utente_id;
    let unita_id: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = ?")
        .bind(&prodotto.unita)
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere l'unità di misura")?
        .context("Unità di misura sconosciuta")?;
    let marca = if prodotto.marca.trim().is_empty() {
        "Senza marca".to_string()
    } else {
        prodotto.marca.trim().to_string()
    };
    let id = sqlx::query(
        "INSERT INTO prodotti_alimentari \
         (alimento_id, marca, marca_normalizzata, nome_commerciale, \
          nome_commerciale_normalizzato, quantita_confezione, unita_confezione_id, \
          codice_ean, creato_da_utente_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(alimento_id)
    .bind(&marca)
    .bind(marca.to_lowercase())
    .bind(&prodotto.nome)
    .bind(prodotto.nome.to_lowercase())
    .bind(prodotto.quantita)
    .bind(unita_id)
    .bind(&prodotto.ean)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile salvare il prodotto letto dal codice a barre")?
    .last_insert_rowid();
    Ok(id)
}

// ---------------------------------------------------------------------------
// Fonti esterne (Open Food Facts, Open Prices)
// ---------------------------------------------------------------------------

/// Un prodotto letto da Open Food Facts.
#[derive(Debug, Clone, PartialEq)]
pub struct ProdottoEsterno {
    pub ean: String,
    pub nome: String,
    pub marca: String,
    pub quantita: f64,
    pub unita: String,
}

const TIMEOUT_RETE: std::time::Duration = std::time::Duration::from_secs(8);
const USER_AGENT_RETE: &str = "GestionaleCasa/1.0 (bot personale)";

/// Legge un prodotto da Open Food Facts. `Ok(None)` = codice sconosciuto;
/// `Err` = la rete non ha risposto (i due casi si dicono all'utente in modo
/// diverso).
pub async fn cerca_su_open_food_facts(ean: &str) -> anyhow::Result<Option<ProdottoEsterno>> {
    let url = format!("https://world.openfoodfacts.org/api/v2/product/{ean}.json?fields=product_name,product_name_it,brands,quantity");
    let risposta = client_rete()?
        .get(url)
        .send()
        .await
        .context("Open Food Facts non risponde")?;
    if risposta.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let corpo: serde_json::Value = risposta
        .json()
        .await
        .context("Risposta di Open Food Facts non leggibile")?;
    if corpo.get("status").and_then(|v| v.as_i64()) == Some(0) {
        return Ok(None);
    }
    let prodotto = corpo.get("product").unwrap_or(&serde_json::Value::Null);
    let nome = prodotto
        .get("product_name_it")
        .and_then(|v| v.as_str())
        .or_else(|| prodotto.get("product_name").and_then(|v| v.as_str()))
        .unwrap_or_default()
        .trim()
        .to_string();
    if nome.is_empty() {
        return Ok(None);
    }
    let marca = prodotto
        .get("brands")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let Some((quantita, unita)) = prodotto
        .get("quantity")
        .and_then(|v| v.as_str())
        .and_then(interpreta_quantita_esterna)
    else {
        return Ok(None);
    };
    Ok(Some(ProdottoEsterno {
        ean: ean.to_string(),
        nome,
        marca,
        quantita,
        unita,
    }))
}

/// L'ultimo prezzo che altri hanno segnato su Open Prices per questo codice
/// a barre. È un suggerimento, e come tale va sempre presentato.
pub async fn prezzo_su_open_prices(ean: &str) -> anyhow::Result<Option<i64>> {
    let url = format!(
        "https://prices.openfoodfacts.org/api/v1/prices?product_code={ean}&currency=EUR&order_by=-date&size=1"
    );
    let corpo: serde_json::Value = client_rete()?
        .get(url)
        .send()
        .await
        .context("Open Prices non risponde")?
        .json()
        .await
        .context("Risposta di Open Prices non leggibile")?;
    let prezzo = corpo
        .get("items")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("price"))
        .and_then(|v| v.as_f64());
    Ok(prezzo
        .map(|valore| (valore * 100.0).round() as i64)
        .filter(|centesimi| *centesimi > 0))
}

fn client_rete() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT_RETE)
        .user_agent(USER_AGENT_RETE)
        .build()
        .context("Impossibile preparare la connessione")
}

// ---------------------------------------------------------------------------
// UI Telegram
// ---------------------------------------------------------------------------

fn button(label: impl Into<String>, callback: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.into(), callback.into())
}

/// `🏪 Negozi`: quali entrano nel confronto. La scelta è dell'utente, come
/// chiesto: confrontare tutte le catene non serve.
pub async fn mostra_negozi(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let negozi = negozi_visibili(pool).await.unwrap_or_else(|errore| {
        tracing::warn!(?errore, "Lettura dei negozi fallita");
        Vec::new()
    });
    let scelti = negozi.iter().filter(|n| n.scelto == 1).count();
    let mut testo = String::new();
    if let Some(notice) = notice {
        testo.push_str(notice);
        testo.push_str("\n\n");
    }
    testo.push_str("🏪 Negozi\n\n");
    if negozi.is_empty() {
        testo.push_str("Non c'è nessun negozio.\nAggiungine uno con ➕ Altro negozio.");
    } else if scelti == 0 {
        testo.push_str(
            "Tocca i supermercati dove fai la spesa: solo quelli entrano nel confronto dei prezzi.",
        );
    } else {
        testo.push_str(&format!(
            "{scelti} nel confronto. Tocca per aggiungere o togliere."
        ));
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = negozi
        .iter()
        .map(|negozio| {
            let icona = if negozio.scelto == 1 { "✅" } else { "☐" };
            vec![button(
                format!("{icona} {}", negozio.nome),
                format!("mercato:negozio:{}", negozio.id),
            )]
        })
        .collect();
    rows.push(vec![button(
        "➕ Altro negozio",
        "lista_spesa:negozio:nuovo",
    )]);
    rows.push(vec![
        button("⬅️ Indietro", "lista_spesa:back"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// `📊 Dove conviene`: la stessa lista, valutata con i prezzi già visti in
/// ciascun negozio scelto.
pub async fn mostra_confronto(
    bot: &Bot,
    chat_id: ChatId,
    stime: &[StimaNegozio],
    senza_prezzo: usize,
) -> ResponseResult<()> {
    let mut testo = String::from("📊 Dove conviene\n\n");
    if stime.is_empty() {
        testo.push_str(
            "Non ho ancora prezzi per i negozi che hai scelto.\n\nSegna il prezzo mentre fai la spesa (📦 sulla voce → 💶 Prezzo): da lì in poi so stimare.",
        );
    } else {
        for stima in stime {
            testo.push_str(&stima.riga());
            testo.push('\n');
        }
        testo.push_str(
            "\nÈ una stima sui prezzi che hai già visto tu, non un listino: le voci senza prezzo non ci sono dentro.",
        );
        if senza_prezzo > 0 {
            testo.push_str(&format!(
                "\n\n{senza_prezzo} voci non hanno ancora nessun prezzo."
            ));
        }
    }
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(vec![
            vec![button("🏪 Negozi", "mercato:negozi")],
            vec![
                button("⬅️ Indietro", "lista_spesa:back"),
                button("🏠 Menù principale", "menu:main"),
            ],
        ]))
        .await?;
    Ok(())
}

/// Scelta del negozio di questa spesa: quello a cui si legano i prezzi
/// registrati mentre si è dentro al supermercato.
pub async fn mostra_scelta_negozio_spesa(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    attuale: Option<i64>,
) -> ResponseResult<()> {
    let negozi = negozi_visibili(pool).await.unwrap_or_default();
    let (scelti, altri): (Vec<&Negozio>, Vec<&Negozio>) =
        negozi.iter().partition(|negozio| negozio.scelto == 1);
    let elenco: Vec<&Negozio> = if scelti.is_empty() {
        altri.into_iter().take(10).collect()
    } else {
        scelti
    };
    let mut rows: Vec<Vec<InlineKeyboardButton>> = elenco
        .iter()
        .map(|negozio| {
            let icona = if Some(negozio.id) == attuale {
                "✅"
            } else {
                "🏪"
            };
            vec![button(
                format!("{icona} {}", negozio.nome),
                format!("lista_spesa:negozio:set:{}", negozio.id),
            )]
        })
        .collect();
    if attuale.is_some() {
        rows.push(vec![button(
            "➖ Nessun negozio",
            "lista_spesa:negozio:set:0",
        )]);
    }
    rows.push(vec![button("🏪 Gestisci negozi", "mercato:negozi")]);
    rows.push(vec![
        button("⬅️ Indietro", "lista_spesa:back"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(
        chat_id,
        "🏪 Dove stai facendo la spesa?\n\nI prezzi che segni finiscono su questo negozio.",
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

/// I callback `mercato:*`. Il negozio della spesa e i prezzi delle voci
/// restano invece in `lista_spesa`, dove sta la lista.
pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    if data == "mercato:negozi" {
        mostra_negozi(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("mercato:negozio:") {
        let Some(negozio_id) = raw.parse::<i64>().ok().filter(|v| *v > 0) else {
            return Ok(true);
        };
        let notice = match cambia_scelta(pool, negozio_id).await {
            Ok(true) => "✅ Aggiunto al confronto.",
            Ok(false) => "➖ Tolto dal confronto.",
            Err(errore) => {
                tracing::warn!(?errore, negozio_id, "Cambio scelta negozio fallito");
                "⚠️ Non riesco ad aggiornare questo negozio."
            }
        };
        mostra_negozi(bot, chat_id, pool, Some(notice)).await?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i_prezzi_si_leggono_e_si_scrivono_come_li_scrive_una_persona() {
        assert_eq!(interpreta_prezzo("1,29"), Some(129));
        assert_eq!(interpreta_prezzo("1.29 €"), Some(129));
        assert_eq!(interpreta_prezzo(" € 2 "), Some(200));
        assert_eq!(interpreta_prezzo("0"), None);
        assert_eq!(interpreta_prezzo("-3"), None);
        assert_eq!(interpreta_prezzo("caro"), None);
        assert_eq!(formatta_euro(129), "1,29 €");
        assert_eq!(formatta_euro(2000), "20,00 €");
        assert_eq!(formatta_euro(5), "0,05 €");
    }

    #[test]
    fn il_prezzo_si_riporta_al_chilo_o_al_litro() {
        assert_eq!(
            prezzo_al_riferimento(250, Some(500.0), Some("g")),
            Some((500, "kg"))
        );
        assert_eq!(
            prezzo_al_riferimento(150, Some(1.5), Some("l")),
            Some((100, "l"))
        );
        // Senza quantità o con un'unità che non si sa convertire, niente
        // confronto inventato.
        assert_eq!(prezzo_al_riferimento(250, None, Some("g")), None);
        assert_eq!(prezzo_al_riferimento(250, Some(2.0), Some("mazzo")), None);
    }

    #[test]
    fn il_confronto_premia_chi_copre_piu_voci() {
        let mut stime = vec![
            StimaNegozio {
                negozio_id: 1,
                nome: "Lidl".to_string(),
                totale_centesimi: 500,
                voci_con_prezzo: 1,
                voci_totali: 5,
            },
            StimaNegozio {
                negozio_id: 2,
                nome: "Conad".to_string(),
                totale_centesimi: 2000,
                voci_con_prezzo: 5,
                voci_totali: 5,
            },
            StimaNegozio {
                negozio_id: 3,
                nome: "Coop".to_string(),
                totale_centesimi: 1900,
                voci_con_prezzo: 5,
                voci_totali: 5,
            },
        ];
        ordina_stime(&mut stime);
        assert_eq!(
            stime.iter().map(|s| s.nome.as_str()).collect::<Vec<_>>(),
            vec!["Coop", "Conad", "Lidl"]
        );
        assert_eq!(stime[0].riga(), "• Coop: 19,00 € (5/5 voci)");
    }

    #[test]
    fn la_quantita_di_open_food_facts_si_capisce_o_si_lascia_stare() {
        assert_eq!(
            interpreta_quantita_esterna("300 g"),
            Some((300.0, "g".to_string()))
        );
        assert_eq!(
            interpreta_quantita_esterna("1,5 L"),
            Some((1.5, "l".to_string()))
        );
        assert_eq!(
            interpreta_quantita_esterna("6 x 125 g"),
            Some((125.0, "g".to_string()))
        );
        assert_eq!(
            interpreta_quantita_esterna("33 cl"),
            Some((330.0, "ml".to_string()))
        );
        assert_eq!(interpreta_quantita_esterna("una confezione"), None);
        assert_eq!(interpreta_quantita_esterna(""), None);
    }

    #[test]
    fn un_codice_a_barre_si_controlla_prima_di_chiamare_la_rete() {
        assert_eq!(
            codice_a_barre_valido(" 8001120000019 "),
            Some("8001120000019".to_string())
        );
        assert_eq!(
            codice_a_barre_valido("80011200"),
            Some("80011200".to_string())
        );
        assert_eq!(codice_a_barre_valido("123"), None);
        assert_eq!(codice_a_barre_valido("non un codice"), None);
    }
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn actor(user_id: i64, space_id: i64) -> crate::identity::AuditActor {
        crate::identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: "Alessio".to_string(),
            spazio_id: space_id,
            spazio_nome_snapshot: "Casa".to_string(),
            view_all: false,
            origine: "test",
            telegram_user_id: None,
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

    async fn utente_e_spazio(pool: &SqlitePool) -> (i64, i64) {
        let user_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Alessio')")
            .execute(pool)
            .await
            .expect("utente")
            .last_insert_rowid();
        let space_id = sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Casa', 'condiviso')")
            .execute(pool)
            .await
            .expect("spazio")
            .last_insert_rowid();
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
        )
        .bind(space_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("membership");
        (user_id, space_id)
    }

    async fn alimento(pool: &SqlitePool, nome: &str) -> i64 {
        let normalizzato = nome.to_lowercase();
        if let Some(id) = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM alimenti WHERE nome_normalizzato = ? AND catalogo_globale = 1",
        )
        .bind(&normalizzato)
        .fetch_optional(pool)
        .await
        .expect("ricerca alimento")
        {
            return id;
        }
        sqlx::query(
            "INSERT INTO alimenti (nome, nome_normalizzato, catalogo_globale) VALUES (?, ?, 1)",
        )
        .bind(nome)
        .bind(&normalizzato)
        .execute(pool)
        .await
        .expect("alimento")
        .last_insert_rowid()
    }

    #[tokio::test]
    async fn i_negozi_partono_dalle_catene_note_e_l_utente_sceglie_i_suoi() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let negozi = negozi_visibili(&pool).await.expect("negozi");
            assert!(
                negozi.iter().any(|n| n.nome == "Esselunga"),
                "le catene comuni ci sono già"
            );
            assert!(
                negozi.iter().all(|n| n.scelto == 0),
                "all'inizio nessuno è nel confronto"
            );
            assert!(negozi_scelti(&pool).await.expect("scelti").is_empty());

            let lidl = negozi.iter().find(|n| n.nome == "Lidl").expect("Lidl").id;
            assert!(cambia_scelta(&pool, lidl).await.expect("scelta"));
            let scelti = negozi_scelti(&pool).await.expect("scelti");
            assert_eq!(scelti.len(), 1);
            assert_eq!(scelti[0].nome, "Lidl");
            assert!(!cambia_scelta(&pool, lidl).await.expect("scelta"));
            assert!(negozi_scelti(&pool).await.expect("scelti").is_empty());

            // Un negozio non di catena: appartiene allo spazio, e non si
            // duplica se lo si riscrive uguale.
            let mio = crea_negozio(&pool, "Fruttivendolo di via Roma")
                .await
                .expect("negozio");
            assert_eq!(
                crea_negozio(&pool, "  fruttivendolo di via roma ")
                    .await
                    .expect("stesso negozio"),
                mio
            );
            assert!(crea_negozio(&pool, "   ").await.is_err());
        })
        .await;
    }

    #[tokio::test]
    async fn il_prezzo_si_ricorda_per_negozio_e_il_preferito_per_alimento() {
        let pool = test_pool().await;
        let (user_id, space_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(actor(user_id, space_id), async {
            let pasta = alimento(&pool, "Pasta").await;
            let unita: i64 = sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = 'g'")
                .fetch_one(&pool)
                .await
                .expect("unità");
            let prodotto = sqlx::query(
                "INSERT INTO prodotti_alimentari \
                 (alimento_id, marca, marca_normalizzata, nome_commerciale, \
                  nome_commerciale_normalizzato, quantita_confezione, unita_confezione_id) \
                 VALUES (?, 'Barilla', 'barilla', 'Spaghetti', 'spaghetti', 500, ?)",
            )
            .bind(pasta)
            .bind(unita)
            .execute(&pool)
            .await
            .expect("prodotto")
            .last_insert_rowid();
            let negozi = negozi_visibili(&pool).await.expect("negozi");
            let lidl = negozi.iter().find(|n| n.nome == "Lidl").expect("Lidl").id;
            let conad = negozi.iter().find(|n| n.nome == "Conad").expect("Conad").id;

            registra_prezzo(
                &pool,
                lidl,
                Some(pasta),
                None,
                "Pasta",
                99,
                Some(500.0),
                Some("g"),
                "spesa",
            )
            .await
            .expect("prezzo Lidl");
            registra_prezzo(
                &pool,
                conad,
                Some(pasta),
                None,
                "Pasta",
                129,
                Some(500.0),
                Some("g"),
                "spesa",
            )
            .await
            .expect("prezzo Conad");
            // L'ultimo prezzo dello stesso negozio vince su quello di prima.
            registra_prezzo(
                &pool,
                lidl,
                Some(pasta),
                None,
                "Pasta",
                109,
                Some(500.0),
                Some("g"),
                "spesa",
            )
            .await
            .expect("prezzo Lidl nuovo");

            let (centesimi, quantita, unita_prezzo) = ultimo_prezzo(&pool, lidl, Some(pasta), None)
                .await
                .expect("lettura")
                .expect("prezzo");
            assert_eq!(centesimi, 109);
            assert_eq!(quantita, Some(500.0));
            assert_eq!(unita_prezzo.as_deref(), Some("g"));
            assert_eq!(
                ultimo_prezzo(&pool, conad, Some(pasta), None)
                    .await
                    .expect("lettura")
                    .map(|(c, _, _)| c),
                Some(129),
                "ogni negozio ha il suo"
            );
            // Un prezzo che non si può attribuire a niente non si registra.
            assert!(
                registra_prezzo(&pool, lidl, None, None, "Boh", 100, None, None, "spesa")
                    .await
                    .is_err()
            );
            assert!(registra_prezzo(
                &pool,
                lidl,
                Some(pasta),
                None,
                "Pasta",
                0,
                None,
                None,
                "spesa"
            )
            .await
            .is_err());

            assert_eq!(preferito_di(&pool, pasta).await.expect("preferito"), None);
            assert!(cambia_preferito(&pool, pasta, prodotto)
                .await
                .expect("preferito"));
            assert_eq!(
                preferito_di(&pool, pasta).await.expect("preferito"),
                Some(prodotto)
            );
            assert!(!cambia_preferito(&pool, pasta, prodotto)
                .await
                .expect("preferito tolto"));
            assert_eq!(preferito_di(&pool, pasta).await.expect("preferito"), None);
        })
        .await;
    }
}
