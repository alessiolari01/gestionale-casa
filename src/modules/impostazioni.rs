//! ⚙️ Impostazioni: quali funzioni del gestionale sono accese.
//!
//! Chiesto da Alessio il 24 settembre 2026: «dai la possibilità all'utente di
//! disattivare le funzioni, tipo scorta/dispensa perché magari non vuole
//! tracciare costantemente il cibo».
//!
//! Il principio è che **spegnere una funzione non nasconde soltanto un
//! pulsante**: spegne anche quello che quella funzione fa da sola. Senza le
//! Scorte non si vede la sezione, la spesa chiusa non entra in casa, i pasti
//! non scalano niente e la lista della spesa smette di sottrarre quello che
//! c'è in dispensa — altrimenti resterebbe una lista dimezzata da scorte che
//! nessuno aggiorna più.
//!
//! I dati non si toccano mai: spegnere è reversibile, e riaccendendo si
//! ritrova tutto dov'era. Per questo non c'è nessuna conferma di
//! eliminazione qui (C16 non si applica: non si cancella niente).
//!
//! Dove sta il sì/no. Le funzioni nuove stanno in `funzioni_spente`, una riga
//! per ogni funzione spenta di quell'utente (l'assenza vuol dire accesa).
//! Le tre preferenze che esistevano già prima — l'ingresso automatico in
//! dispensa, l'aggiornamento automatico della lista e la legenda dei simboli
//! — restano nelle loro colonne di `preferenze_utente`: questa schermata
//! legge e scrive quelle, invece di tenerne una seconda copia.

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::collections::HashSet;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

/// Il wrapper che tiene una sola schermata attiva per chat e aggiunge da
/// solo `💡 Migliora`: qui dentro `Bot` è sempre quello, mai quello di
/// teloxide.
type Bot = crate::context_bot::ContextBot;

/// Una funzione che si può spegnere.
///
/// L'ordine è quello in cui compaiono nelle impostazioni: prima le sezioni,
/// poi quello che il bot fa da solo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Funzione {
    Alimentazione,
    Ricette,
    ProfiliAlimentari,
    Planner,
    ListaSpesa,
    Scorte,
    Turni,
    Prezzi,
    Oggetti,
    Luoghi,
    Storico,
    ScorteIngresso,
    ScorteScaricoPasti,
    ListaAggiornamento,
    Legenda,
}

/// Le sezioni, nell'ordine dei menù.
pub const SEZIONI: [Funzione; 11] = [
    Funzione::Alimentazione,
    Funzione::Ricette,
    Funzione::ProfiliAlimentari,
    Funzione::Planner,
    Funzione::ListaSpesa,
    Funzione::Scorte,
    Funzione::Turni,
    Funzione::Prezzi,
    Funzione::Oggetti,
    Funzione::Luoghi,
    Funzione::Storico,
];

/// Quello che il bot fa da solo, senza che nessuno lo chieda.
pub const AUTOMATISMI: [Funzione; 4] = [
    Funzione::ScorteIngresso,
    Funzione::ScorteScaricoPasti,
    Funzione::ListaAggiornamento,
    Funzione::Legenda,
];

/// Dove vive il sì/no di questa funzione.
enum Dove {
    /// Una riga in `funzioni_spente` quando è spenta.
    Tabella,
    /// Una colonna di `preferenze_utente`, che esisteva già prima delle
    /// impostazioni: 1 acceso, 0 spento.
    Colonna(&'static str),
}

impl Funzione {
    /// La chiave scritta a database. Non cambia mai: cambiarla riaccenderebbe
    /// di colpo una funzione che qualcuno aveva spento.
    pub fn chiave(self) -> &'static str {
        match self {
            Funzione::Alimentazione => "alimentazione",
            Funzione::Ricette => "ricette",
            Funzione::ProfiliAlimentari => "profili_alimentari",
            Funzione::Planner => "planner",
            Funzione::ListaSpesa => "lista_spesa",
            Funzione::Scorte => "scorte",
            Funzione::Turni => "turni",
            Funzione::Prezzi => "prezzi",
            Funzione::Oggetti => "oggetti",
            Funzione::Luoghi => "luoghi",
            Funzione::Storico => "storico",
            Funzione::ScorteIngresso => "scorte_ingresso",
            Funzione::ScorteScaricoPasti => "scorte_scarico_pasti",
            Funzione::ListaAggiornamento => "lista_aggiornamento",
            Funzione::Legenda => "legenda",
        }
    }

    pub fn da_chiave(chiave: &str) -> Option<Funzione> {
        SEZIONI
            .iter()
            .chain(AUTOMATISMI.iter())
            .copied()
            .find(|funzione| funzione.chiave() == chiave)
    }

    /// L'etichetta è quella del pulsante che la funzione ha nei menù: chi
    /// spegne "🥫 Scorte" deve riconoscere subito cosa sparirà (C10).
    pub fn etichetta(self) -> &'static str {
        match self {
            Funzione::Alimentazione => "🍽️ Alimentazione",
            Funzione::Ricette => "🍳 Ricette",
            Funzione::ProfiliAlimentari => "👥 Profili alimentari",
            Funzione::Planner => "📅 Planner alimentare",
            Funzione::ListaSpesa => "🛒 Lista della spesa",
            Funzione::Scorte => "🥫 Scorte",
            Funzione::Turni => "📋 Turni e routine",
            Funzione::Prezzi => "💶 Prezzi e negozi",
            Funzione::Oggetti => "🏷️ Oggetti",
            Funzione::Luoghi => "🏠 Case, stanze e contenitori",
            Funzione::Storico => "📜 Storico",
            Funzione::ScorteIngresso => "📥 La spesa chiusa entra in casa",
            Funzione::ScorteScaricoPasti => "🍲 I pasti scalano le scorte",
            Funzione::ListaAggiornamento => "🔄 La lista si aggiorna da sola",
            Funzione::Legenda => "💡 Legenda dei simboli",
        }
    }

    /// Una riga che dice cosa cambia davvero spegnendola. Senza, "spegni
    /// Scorte" sembra solo "nascondi un pulsante".
    pub fn spiegazione(self) -> &'static str {
        match self {
            Funzione::Alimentazione => "Alimenti, ricette, planner, spesa, scorte e turni: tutto insieme.",
            Funzione::Ricette => "Le ricette e il loro procedimento. I pasti del planner restano, senza ricetta collegata.",
            Funzione::ProfiliAlimentari => "Chi mangia quanto. Senza, i pasti valgono per una persona sola.",
            Funzione::Planner => "I pasti pianificati. Senza, la lista della spesa la scrivi tu.",
            Funzione::ListaSpesa => "La lista della spesa e il suo archivio.",
            Funzione::Scorte => "Dispensa, frigo e freezer. Senza, la lista non sottrae più quello che hai in casa.",
            Funzione::Turni => "I modelli di settimana e le loro assegnazioni.",
            Funzione::Prezzi => "Negozi, prezzi visti e confronto della spesa.",
            Funzione::Oggetti => "Gli oggetti di casa con marca, prezzo e garanzia.",
            Funzione::Luoghi => "Case, stanze e contenitori dove sta la roba.",
            Funzione::Storico => "L'elenco di tutto quello che è successo.",
            Funzione::ScorteIngresso => "Chiudendo la spesa la roba comprata entra da sola nel suo posto.",
            Funzione::ScorteScaricoPasti => "Un pasto preparato o consumato toglie i suoi ingredienti dalle scorte.",
            Funzione::ListaAggiornamento => "La lista si ricalcola da sola quando cambia qualcosa.",
            Funzione::Legenda => "La riga che spiega i simboli sotto gli elenchi.",
        }
    }

    /// La funzione che la contiene: spenta quella, questa non si raggiunge
    /// comunque. Serve a non far promettere a una schermata quello che un
    /// interruttore più in alto ha già spento.
    pub fn padre(self) -> Option<Funzione> {
        match self {
            Funzione::Ricette
            | Funzione::ProfiliAlimentari
            | Funzione::Planner
            | Funzione::ListaSpesa
            | Funzione::Scorte
            | Funzione::Turni => Some(Funzione::Alimentazione),
            Funzione::Prezzi => Some(Funzione::ListaSpesa),
            Funzione::ScorteIngresso | Funzione::ScorteScaricoPasti => Some(Funzione::Scorte),
            Funzione::ListaAggiornamento => Some(Funzione::ListaSpesa),
            _ => None,
        }
    }

    fn dove(self) -> Dove {
        match self {
            Funzione::ScorteIngresso => Dove::Colonna("dispensa_ingresso_automatico"),
            Funzione::ListaAggiornamento => Dove::Colonna("lista_spesa_aggiornamento_automatico"),
            Funzione::Legenda => Dove::Colonna("mostra_legenda"),
            _ => Dove::Tabella,
        }
    }

    /// I callback che appartengono a questa funzione. Un pulsante di una
    /// schermata vecchia resta cliccabile per sempre, quindi non basta
    /// nascondere la voce nel menù: il click va fermato qui.
    fn prefissi(self) -> &'static [&'static str] {
        match self {
            Funzione::Alimentazione => &["food:"],
            Funzione::Ricette => &["recipe:"],
            Funzione::ProfiliAlimentari => &["foodprof:"],
            Funzione::Planner => &["planner:"],
            Funzione::ListaSpesa => &["lista_spesa:"],
            Funzione::Scorte => &["dispensa:"],
            Funzione::Turni => &["turni:"],
            Funzione::Prezzi => &["mercato:"],
            Funzione::Oggetti => &["oggetti:"],
            Funzione::Luoghi => &["loc:"],
            Funzione::Storico => &["history:"],
            _ => &[],
        }
    }
}

/// Vero se questo callback appartiene a una funzione che si può spegnere.
/// Serve a non leggere le preferenze a ogni singolo click: la stragrande
/// maggioranza dei callback (menù principale, profilo, miglioramenti,
/// amministrazione) non ha niente da controllare.
pub fn puo_essere_spento(data: &str) -> bool {
    SEZIONI.iter().any(|funzione| {
        funzione
            .prefissi()
            .iter()
            .any(|prefisso| data.starts_with(prefisso))
    })
}

/// Le funzioni accese di chi sta usando il bot adesso, lette in una volta
/// sola: disegnare un menù non deve costare una query per pulsante.
#[derive(Debug, Clone, Default)]
pub struct Funzioni {
    spente: HashSet<String>,
}

impl Funzioni {
    /// Tutto acceso: è quello che si usa quando non c'è un utente vero
    /// (notifica di avvio, attore di sistema) e quando la lettura fallisce —
    /// un problema nel leggere le preferenze non deve nascondere il
    /// gestionale.
    pub fn tutte_accese() -> Self {
        Self::default()
    }

    pub fn attiva(&self, funzione: Funzione) -> bool {
        if self.spente.contains(funzione.chiave()) {
            return false;
        }
        match funzione.padre() {
            Some(padre) => self.attiva(padre),
            None => true,
        }
    }

    /// Spenta da sé, senza guardare chi la contiene: serve alla schermata
    /// delle impostazioni, dove l'interruttore deve mostrare la scelta fatta
    /// su *questa* funzione anche quando il padre è spento.
    pub fn spenta_di_suo(&self, funzione: Funzione) -> bool {
        self.spente.contains(funzione.chiave())
    }

    /// La funzione spenta che blocca questo callback, se ce n'è una.
    pub fn blocca(&self, data: &str) -> Option<Funzione> {
        SEZIONI
            .iter()
            .copied()
            .find(|funzione| {
                !self.attiva(*funzione)
                    && funzione
                        .prefissi()
                        .iter()
                        .any(|prefisso| data.starts_with(prefisso))
            })
            .map(|funzione| {
                // Si nomina il pulsante che si è davvero spento: se le Scorte
                // sono spente perché è spenta l'Alimentazione, dire "Scorte"
                // manderebbe a cercare un interruttore già a posto.
                let mut colpevole = funzione;
                while let Some(padre) = colpevole.padre() {
                    if self.spenta_di_suo(colpevole) {
                        break;
                    }
                    colpevole = padre;
                }
                colpevole
            })
    }
}

/// Legge in un colpo solo tutte le funzioni spente di questo utente.
pub async fn funzioni(pool: &SqlitePool) -> Funzioni {
    let Some(utente_id) = crate::identity::current_actor().utente_id else {
        return Funzioni::tutte_accese();
    };
    let mut spente: HashSet<String> =
        sqlx::query_scalar::<_, String>("SELECT funzione FROM funzioni_spente WHERE utente_id = ?")
            .bind(utente_id)
            .fetch_all(pool)
            .await
            .unwrap_or_else(|errore| {
                tracing::warn!(?errore, "Impossibile leggere le funzioni spente");
                Vec::new()
            })
            .into_iter()
            .collect();

    // Le tre preferenze più vecchie stanno nelle loro colonne.
    let colonne: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT dispensa_ingresso_automatico, lista_spesa_aggiornamento_automatico, \
                mostra_legenda \
         FROM preferenze_utente WHERE utente_id = ?",
    )
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    if let Some((ingresso, aggiornamento, legenda)) = colonne {
        for (valore, funzione) in [
            (ingresso, Funzione::ScorteIngresso),
            (aggiornamento, Funzione::ListaAggiornamento),
            (legenda, Funzione::Legenda),
        ] {
            if valore == 0 {
                spente.insert(funzione.chiave().to_string());
            }
        }
    }
    Funzioni { spente }
}

/// Accende o spegne una funzione.
pub async fn imposta(pool: &SqlitePool, funzione: Funzione, accesa: bool) -> Result<()> {
    let utente_id = crate::identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    match funzione.dove() {
        Dove::Colonna(colonna) => {
            let sql = format!("UPDATE preferenze_utente SET {colonna} = ? WHERE utente_id = ?");
            sqlx::query(&sql)
                .bind(i64::from(accesa))
                .bind(utente_id)
                .execute(pool)
                .await
                .context("Impossibile salvare la preferenza")?;
        }
        Dove::Tabella if accesa => {
            sqlx::query("DELETE FROM funzioni_spente WHERE utente_id = ? AND funzione = ?")
                .bind(utente_id)
                .bind(funzione.chiave())
                .execute(pool)
                .await
                .context("Impossibile riaccendere la funzione")?;
        }
        Dove::Tabella => {
            sqlx::query(
                "INSERT INTO funzioni_spente (utente_id, funzione) VALUES (?, ?) \
                 ON CONFLICT (utente_id, funzione) DO NOTHING",
            )
            .bind(utente_id)
            .bind(funzione.chiave())
            .execute(pool)
            .await
            .context("Impossibile spegnere la funzione")?;
        }
    }
    Ok(())
}

fn button(testo: impl Into<String>, data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(testo.into(), data.into())
}

fn riga_navigazione(indietro: &str) -> Vec<InlineKeyboardButton> {
    vec![
        button("⬅️ Indietro", indietro),
        button("🏠 Menù principale", "menu:main"),
    ]
}

/// Il menù delle impostazioni: due elenchi, perché una lista sola di quindici
/// interruttori non si legge (C6).
pub async fn mostra_menu(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let funzioni = funzioni(pool).await;
    let spente = SEZIONI
        .iter()
        .chain(AUTOMATISMI.iter())
        .filter(|funzione| funzioni.spenta_di_suo(**funzione))
        .count();
    let stato = if spente == 0 {
        "Adesso è acceso tutto.".to_string()
    } else if spente == 1 {
        "Adesso c'è 1 cosa spenta.".to_string()
    } else {
        format!("Adesso ci sono {spente} cose spente.")
    };
    bot.send_message(
        chat_id,
        format!(
            "⚙️ Impostazioni\n\nQui scegli cosa usare del gestionale: quello che spegni sparisce dai menù e smette di lavorare da solo, ma i dati restano dove sono e riaccendendolo ritrovi tutto.\n\n{stato}"
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button("🧩 Sezioni", "settings:sezioni")],
        vec![button("🤖 Cosa fa da solo", "settings:automatismi")],
        riga_navigazione("menu:main"),
    ]))
    .await?;
    Ok(())
}

async fn mostra_elenco(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    automatismi: bool,
) -> ResponseResult<()> {
    let funzioni = funzioni(pool).await;
    let elenco: &[Funzione] = if automatismi { &AUTOMATISMI } else { &SEZIONI };
    let titolo = if automatismi {
        "🤖 Cosa fa da solo"
    } else {
        "🧩 Sezioni"
    };

    // C1: il testo non ripete i nomi, che stanno sui pulsanti. Spiega solo
    // come si legge la riga e cosa fa un tocco.
    let mut testo = format!("{titolo}\n\n✅ acceso · ⬜ spento — toccane uno per cambiarlo.");
    for funzione in elenco {
        if funzioni.spenta_di_suo(*funzione) {
            continue;
        }
        // La spiegazione la merita quello che è ancora acceso: è lì che serve
        // sapere cosa si perde spegnendolo.
        testo.push_str(&format!(
            "\n\n{} {}",
            funzione.etichetta(),
            funzione.spiegazione()
        ));
    }

    let mut tastiera: Vec<Vec<InlineKeyboardButton>> = elenco
        .iter()
        .map(|funzione| {
            let segno = if funzioni.spenta_di_suo(*funzione) {
                "⬜"
            } else {
                "✅"
            };
            vec![button(
                format!("{segno} {}", funzione.etichetta()),
                format!("settings:toggle:{}", funzione.chiave()),
            )]
        })
        .collect();
    tastiera.push(riga_navigazione("settings:menu"));
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(tastiera))
        .await?;
    Ok(())
}

/// La schermata che compare premendo il pulsante di una funzione spenta.
pub async fn avvisa_spenta(bot: &Bot, chat_id: ChatId, funzione: Funzione) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        format!(
            "⚙️ {} è spento.\n\n{}\n\nI dati sono ancora tutti lì: riaccendendolo ritrovi tutto dov'era.",
            funzione.etichetta(),
            funzione.spiegazione()
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![button("⚙️ Impostazioni", "settings:sezioni")],
        vec![button("🏠 Menù principale", "menu:main")],
    ]))
    .await?;
    Ok(())
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    data: &str,
) -> ResponseResult<bool> {
    if data == "settings:menu" {
        mostra_menu(bot, chat_id, pool).await?;
        return Ok(true);
    }
    if data == "settings:sezioni" {
        mostra_elenco(bot, chat_id, pool, false).await?;
        return Ok(true);
    }
    if data == "settings:automatismi" {
        mostra_elenco(bot, chat_id, pool, true).await?;
        return Ok(true);
    }
    if let Some(chiave) = data.strip_prefix("settings:toggle:") {
        let Some(funzione) = Funzione::da_chiave(chiave) else {
            mostra_menu(bot, chat_id, pool).await?;
            return Ok(true);
        };
        let accesa = funzioni(pool).await.spenta_di_suo(funzione);
        if let Err(errore) = imposta(pool, funzione, accesa).await {
            tracing::warn!(?errore, chiave, "Impossibile cambiare la funzione");
        }
        let automatismi = AUTOMATISMI.contains(&funzione);
        mostra_elenco(bot, chat_id, pool, automatismi).await?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use crate::modules::dispensa;
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

    async fn utente_e_spazio(pool: &SqlitePool) -> (i64, i64) {
        let utente_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Alessio')")
            .execute(pool)
            .await
            .expect("utente")
            .last_insert_rowid();
        let spazio_id = sqlx::query("INSERT INTO spazi (nome, tipo) VALUES ('Casa', 'condiviso')")
            .execute(pool)
            .await
            .expect("spazio")
            .last_insert_rowid();
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
        )
        .bind(spazio_id)
        .bind(utente_id)
        .execute(pool)
        .await
        .expect("membership");
        sqlx::query("INSERT INTO preferenze_utente (utente_id, spazio_attivo_id) VALUES (?, ?)")
            .bind(utente_id)
            .bind(spazio_id)
            .execute(pool)
            .await
            .expect("preferenze");
        (utente_id, spazio_id)
    }

    fn attore(utente_id: i64, spazio_id: i64) -> crate::identity::AuditActor {
        crate::identity::AuditActor {
            utente_id: Some(utente_id),
            nome_snapshot: "Alessio".to_string(),
            spazio_id,
            spazio_nome_snapshot: "Casa".to_string(),
            view_all: false,
            origine: "telegram",
            telegram_user_id: Some(utente_id),
            telegram_username: None,
        }
    }

    /// Spegnere e riaccendere, e il caso che conta: spegnere le Scorte deve
    /// spegnere anche l'ingresso automatico, che sta in un'altra tabella.
    #[tokio::test]
    async fn spegnere_le_scorte_spegne_anche_quello_che_facevano_da_sole() {
        let pool = test_pool().await;
        let (utente_id, spazio_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(attore(utente_id, spazio_id), async {
            // Di partenza è tutto acceso, senza nemmeno una riga scritta.
            assert!(funzioni(&pool).await.attiva(Funzione::Scorte));
            assert!(dispensa::ingresso_automatico(&pool).await);

            imposta(&pool, Funzione::Scorte, false)
                .await
                .expect("spegne le scorte");
            let spente = funzioni(&pool).await;
            assert!(!spente.attiva(Funzione::Scorte));
            assert!(
                !spente.attiva(Funzione::ScorteIngresso),
                "l'ingresso automatico dipende dalle scorte"
            );
            assert!(
                !dispensa::ingresso_automatico(&pool).await,
                "la spesa chiusa non entra piu' in casa da sola"
            );
            // Le altre sezioni non c'entrano niente.
            assert!(spente.attiva(Funzione::Oggetti));

            // Riaccendendole si ritrova tutto com'era: spegnere non cancella.
            imposta(&pool, Funzione::Scorte, true)
                .await
                .expect("riaccende");
            assert!(funzioni(&pool).await.attiva(Funzione::ScorteIngresso));
            assert!(dispensa::ingresso_automatico(&pool).await);
        })
        .await;
    }

    /// I tre interruttori piu' vecchi vivono ancora nelle loro colonne di
    /// `preferenze_utente`: le impostazioni scrivono li', non in una seconda
    /// copia che poi direbbe il contrario.
    #[tokio::test]
    async fn le_preferenze_vecchie_restano_nella_loro_colonna() {
        let pool = test_pool().await;
        let (utente_id, spazio_id) = utente_e_spazio(&pool).await;

        crate::identity::with_actor(attore(utente_id, spazio_id), async {
            imposta(&pool, Funzione::Legenda, false)
                .await
                .expect("spegne la legenda");
            let colonna: i64 = sqlx::query_scalar(
                "SELECT mostra_legenda FROM preferenze_utente WHERE utente_id = ?",
            )
            .bind(utente_id)
            .fetch_one(&pool)
            .await
            .expect("colonna");
            assert_eq!(colonna, 0);
            let righe: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM funzioni_spente")
                .fetch_one(&pool)
                .await
                .expect("conteggio");
            assert_eq!(righe, 0, "nessuna riga doppia");
            assert!(!funzioni(&pool).await.attiva(Funzione::Legenda));
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn con_spente(chiavi: &[&str]) -> Funzioni {
        Funzioni {
            spente: chiavi.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn niente_di_spento_vuol_dire_tutto_acceso() {
        let funzioni = Funzioni::tutte_accese();
        for funzione in SEZIONI.iter().chain(AUTOMATISMI.iter()) {
            assert!(funzioni.attiva(*funzione), "{}", funzione.chiave());
        }
    }

    #[test]
    fn spegnere_il_padre_spegne_anche_i_figli() {
        let funzioni = con_spente(&["alimentazione"]);
        assert!(!funzioni.attiva(Funzione::Scorte));
        assert!(!funzioni.attiva(Funzione::ScorteIngresso), "nipote");
        // Ma la scelta fatta su di loro resta quella di prima: riaccendendo
        // l'Alimentazione si ritrovano accese.
        assert!(!funzioni.spenta_di_suo(Funzione::Scorte));
        // Le sezioni fuori da Alimentazione non c'entrano niente.
        assert!(funzioni.attiva(Funzione::Oggetti));
    }

    #[test]
    fn un_callback_di_una_funzione_spenta_viene_fermato() {
        let funzioni = con_spente(&["scorte"]);
        assert_eq!(funzioni.blocca("dispensa:menu"), Some(Funzione::Scorte));
        assert_eq!(funzioni.blocca("oggetti:menu"), None);

        // Spenta l'Alimentazione, il messaggio deve nominare l'interruttore
        // che si e' davvero spostato, non la sezione che ne dipende.
        let funzioni = con_spente(&["alimentazione"]);
        assert_eq!(
            funzioni.blocca("dispensa:menu"),
            Some(Funzione::Alimentazione)
        );
    }

    #[test]
    fn ogni_chiave_va_e_torna_e_nessuna_e_doppia() {
        let mut viste = HashSet::new();
        for funzione in SEZIONI.iter().chain(AUTOMATISMI.iter()) {
            assert!(viste.insert(funzione.chiave()), "{}", funzione.chiave());
            assert_eq!(Funzione::da_chiave(funzione.chiave()), Some(*funzione));
            // Una chiave finisce dentro un callback da 64 byte.
            assert!(format!("settings:toggle:{}", funzione.chiave()).len() <= 64);
        }
        assert_eq!(Funzione::da_chiave("inventata"), None);
    }
}
