//! Lista della spesa: prossimo macro-step deciso l'8 settembre 2026
//! (`docs/previsto/lista-della-spesa.md`), appoggiato sul planner alimentare
//! già operativo.
//!
//! Una sola lista attiva per spazio (o personale, stesso pattern di
//! `planner_alimentare.rs`), con un proprio intervallo di date indipendente
//! dalle settimane del planner: la lista aggrega su tutti i
//! `planner_alimentari` non archiviati dello stesso spazio/proprietario la
//! cui `data_pasto` ricade nell'intervallo.
//!
//! Il modulo è diviso in tre parti, come `planner_alimentare.rs`:
//! dominio puro (aggregazione/conversione unità, testato senza database),
//! funzioni database (`sqlite::memory:` nei test), poi la UI Telegram.

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    sync::{Arc, Mutex, OnceLock},
};

use anyhow::Context as _;
use sqlx::{FromRow, SqlitePool};
use teloxide::{
    // `Download`: serve a scaricare la foto del codice a barre (consegna B,
    // 18 settembre 2026).
    net::Download,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::modules::{calendario, liste, novita};

type Bot = crate::context_bot::ContextBot;

// ===========================================================================
// Dominio puro: aggregazione degli ingredienti e conversione di unità.
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamigliaConversione {
    Massa,
    Volume,
}

impl FamigliaConversione {
    /// Unità-base in cui si somma: `g` per la massa, `ml` per il volume
    /// (fattore 1/1 nella tabella `unita_misura`, per definizione).
    fn unita_base(self) -> &'static str {
        match self {
            Self::Massa => "g",
            Self::Volume => "ml",
        }
    }

    fn from_db(value: &str) -> Option<Self> {
        match value {
            "massa" => Some(Self::Massa),
            "volume" => Some(Self::Volume),
            _ => None,
        }
    }
}

/// Come si converte un simbolo di `unita_misura`: se ha una famiglia, il
/// fattore per portarlo nell'unità-base di quella famiglia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoUnita {
    pub famiglia: Option<FamigliaConversione>,
    pub fattore_num: f64,
    pub fattore_den: f64,
}

/// Una riga di ingrediente pianificato, già filtrata a monte (solo pasti
/// pianificati non saltati/completati, solo righe con
/// `quantita_finale_snapshot` non nullo — vedi `righe_da_aggregare`).
#[derive(Debug, Clone, PartialEq)]
pub struct RigaIngrediente {
    pub alimento_id: Option<i64>,
    /// Prodotto commerciale specifico (es. "Pasta De Cecco"), se la riga
    /// viene da un'aggiunta dal catalogo su un prodotto e non su un
    /// alimento generico -- vedi `Identita::Prodotto`. `None` per tutte le
    /// righe del planner e per le aggiunte su un alimento generico.
    pub prodotto_id: Option<i64>,
    pub nome: String,
    pub unita_simbolo: String,
    pub quantita: f64,
}

/// Una voce generata dall'aggregazione, pronta per `liste_spesa_voci`.
#[derive(Debug, Clone, PartialEq)]
pub struct VoceGenerata {
    pub alimento_id: Option<i64>,
    pub prodotto_id: Option<i64>,
    pub nome: String,
    pub quantita: f64,
    pub unita_simbolo: String,
}

/// Identità di aggregazione: il prodotto commerciale specifico se presente
/// (mai fuso con l'alimento generico sottostante, anche a parità di
/// `alimento_id` -- decisione esplicita presa con Alessio per non
/// confondere "mi serve della pasta" con "voglio comprare proprio quella
/// marca"), altrimenti l'alimento del catalogo se presente, altrimenti il
/// nome normalizzato -- due righe con lo stesso `alimento_id` restano
/// insieme anche se il nome congelato differisce (rinominato nel frattempo),
/// due righe senza `alimento_id` né `prodotto_id` si aggregano per nome
/// uguale a meno di maiuscole/spazi.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Identita {
    Prodotto(i64),
    Alimento(i64),
    Nome(String),
}

fn identita_riga(riga: &RigaIngrediente) -> Identita {
    match (riga.prodotto_id, riga.alimento_id) {
        (Some(id), _) => Identita::Prodotto(id),
        (None, Some(id)) => Identita::Alimento(id),
        (None, None) => Identita::Nome(riga.nome.trim().to_lowercase()),
    }
}

fn identita_voce(voce: &VoceGenerata) -> Identita {
    match (voce.prodotto_id, voce.alimento_id) {
        (Some(id), _) => Identita::Prodotto(id),
        (None, Some(id)) => Identita::Alimento(id),
        (None, None) => Identita::Nome(voce.nome.trim().to_lowercase()),
    }
}

/// Arrotonda a due decimali: la somma di quantità in virgola mobile
/// altrimenti mostrerebbe cifre che nessuna ricetta ha mai scritto
/// (es. `133.33000000000001 g`).
pub fn arrotonda(quantita: f64) -> f64 {
    (quantita * 100.0).round() / 100.0
}

/// Porta una quantità nell'unità-base della sua famiglia (`g` per la massa,
/// `ml` per il volume). Un'unità senza famiglia, o sconosciuta (`info =
/// None`), resta com'è: `pz` non si converte in niente.
///
/// Estratta da `aggrega_ingredienti` il 17 settembre 2026 perché serve
/// anche alle scorte (`dispensa.rs`): la stessa regola di conversione in un
/// posto solo, non due copie che prima o poi divergono.
pub fn converti_in_base(quantita: f64, unita: &str, info: Option<InfoUnita>) -> (f64, String) {
    match info.and_then(|info| info.famiglia.map(|famiglia| (famiglia, info))) {
        Some((famiglia, info)) if info.fattore_den != 0.0 => (
            quantita * info.fattore_num / info.fattore_den,
            famiglia.unita_base().to_string(),
        ),
        _ => (quantita, unita.to_string()),
    }
}

/// L'inverso di `converti_in_base`: da una quantità nell'unità-base alla
/// stessa quantità nell'unità indicata da `info` (es. 250 g → 0,25 kg).
/// Senza famiglia la quantità non cambia.
pub fn converti_da_base(quantita_base: f64, info: Option<InfoUnita>) -> f64 {
    match info {
        Some(info) if info.famiglia.is_some() && info.fattore_num != 0.0 => {
            quantita_base * info.fattore_den / info.fattore_num
        }
        _ => quantita_base,
    }
}

/// Aggrega le righe di ingrediente in voci per la lista della spesa.
///
/// Stessa famiglia di conversione (massa o volume) sommata nell'unità-base
/// della famiglia; famiglie diverse, o unità senza famiglia (pz, cucchiaio,
/// qb, o un simbolo non presente in `unita_misura`), restano separate e si
/// aggregano per simbolo esatto, senza conversione. `info_unita` risolve un
/// simbolo di `unita_misura`; `None` (simbolo sconosciuto) equivale a
/// un'unità senza famiglia.
pub fn aggrega_ingredienti(
    righe: &[RigaIngrediente],
    info_unita: impl Fn(&str) -> Option<InfoUnita>,
) -> Vec<VoceGenerata> {
    let mut risultato: Vec<VoceGenerata> = Vec::new();

    for riga in righe {
        // Difensivo: le righe arrivano già filtrate dal database
        // (`quantita_finale_snapshot > 0` per CHECK), ma il dominio non deve
        // fidarsi ciecamente di chi lo chiama.
        if !riga.quantita.is_finite() || riga.quantita <= 0.0 {
            continue;
        }

        let (quantita_out, unita_out) = converti_in_base(
            riga.quantita,
            &riga.unita_simbolo,
            info_unita(&riga.unita_simbolo),
        );

        let identita = identita_riga(riga);
        if let Some(voce) = risultato
            .iter_mut()
            .find(|voce| identita_voce(voce) == identita && voce.unita_simbolo == unita_out)
        {
            voce.quantita += quantita_out;
        } else {
            risultato.push(VoceGenerata {
                alimento_id: riga.alimento_id,
                prodotto_id: riga.prodotto_id,
                nome: riga.nome.clone(),
                quantita: quantita_out,
                unita_simbolo: unita_out,
            });
        }
    }

    for voce in &mut risultato {
        voce.quantita = arrotonda(voce.quantita);
    }
    risultato
}

/// Sottrae dalle voci appena aggregate la quantità già coperta da voci
/// `generato` esistenti e già comprate, con la stessa identità (alimento o
/// nome) e la stessa unità. Una voce interamente coperta (residuo <= 0)
/// sparisce del tutto: non c'è nulla di nuovo da comprare per quella riga.
/// Non è mai un merge con la vecchia riga comprata -- quella resta
/// intoccata altrove (`aggiorna_lista` non la cancella mai); qui si calcola
/// solo quanto manca ancora.
pub fn sottrai_gia_comprato(
    fresche: Vec<VoceGenerata>,
    gia_comprato: &[VoceGenerata],
) -> Vec<VoceGenerata> {
    fresche
        .into_iter()
        .filter_map(|mut voce| {
            let identita = identita_voce(&voce);
            let coperto: f64 = gia_comprato
                .iter()
                .filter(|coperta| {
                    identita_voce(coperta) == identita
                        && coperta.unita_simbolo == voce.unita_simbolo
                })
                .map(|coperta| coperta.quantita)
                .sum();
            let residuo = arrotonda(voce.quantita - coperto);
            if residuo > 0.0 {
                voce.quantita = residuo;
                Some(voce)
            } else {
                None
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoceManualeError {
    DescrizioneVuota,
    DescrizioneTroppoLunga,
    QuantitaFormatoNonValido,
    QuantitaNonPositiva,
    UnitaMancante,
    UnitaTroppoLunga,
}

impl fmt::Display for VoceManualeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let messaggio = match self {
            Self::DescrizioneVuota => "Scrivi una descrizione, non può essere vuota.",
            Self::DescrizioneTroppoLunga => {
                "La descrizione è troppo lunga (massimo 120 caratteri)."
            }
            Self::QuantitaFormatoNonValido => {
                "Scrivi quantità e unità separate da uno spazio, es. \"500 g\"."
            }
            Self::QuantitaNonPositiva => "La quantità deve essere maggiore di zero.",
            Self::UnitaMancante => "Scrivi anche l'unità, es. \"500 g\".",
            Self::UnitaTroppoLunga => "L'unità è troppo lunga (massimo 20 caratteri).",
        };
        f.write_str(messaggio)
    }
}

impl Error for VoceManualeError {}

/// Sotto questa soglia un residuo non e' un residuo: e' l'errore dei numeri
/// a virgola mobile dopo le conversioni fra unita'.
const TOLLERANZA_QUANTITA: f64 = 1e-6;

const DESCRIZIONE_MAX_CARATTERI: usize = 120;
const UNITA_MAX_CARATTERI: usize = 20;

/// Valida la descrizione di una voce manuale (testo libero, obbligatoria).
pub fn valida_descrizione_manuale(testo: &str) -> Result<String, VoceManualeError> {
    let testo = testo.trim();
    if testo.is_empty() {
        return Err(VoceManualeError::DescrizioneVuota);
    }
    if testo.chars().count() > DESCRIZIONE_MAX_CARATTERI {
        return Err(VoceManualeError::DescrizioneTroppoLunga);
    }
    Ok(testo.to_string())
}

/// Interpreta una quantità scritta a mano ("500 g", "1,5 l", "2 pz"): il
/// primo token è il numero (virgola o punto come separatore decimale), il
/// resto è l'unità.
pub fn valida_quantita_manuale(testo: &str) -> Result<(f64, String), VoceManualeError> {
    let testo = testo.trim();
    let mut parti = testo.splitn(2, char::is_whitespace);
    let numero = parti.next().unwrap_or_default().trim();
    let unita = parti.next().unwrap_or_default().trim();

    if numero.is_empty() {
        return Err(VoceManualeError::QuantitaFormatoNonValido);
    }
    let quantita: f64 = numero
        .replace(',', ".")
        .parse()
        .map_err(|_| VoceManualeError::QuantitaFormatoNonValido)?;
    if unita.is_empty() {
        return Err(VoceManualeError::UnitaMancante);
    }
    if !quantita.is_finite() || quantita <= 0.0 {
        return Err(VoceManualeError::QuantitaNonPositiva);
    }
    if unita.chars().count() > UNITA_MAX_CARATTERI {
        return Err(VoceManualeError::UnitaTroppoLunga);
    }
    Ok((quantita, unita.to_string()))
}

/// Come `valida_quantita_manuale`, ma per un'aggiunta dal catalogo: se
/// l'unità non viene scritta usa `unita_default` (quella dell'alimento o
/// del prodotto scelto) invece di richiederla sempre -- chiesto da Alessio
/// dopo un collaudo dal vivo, per non dover riscrivere l'unità di un
/// alimento già noto al catalogo. Scrivere comunque un'unità la
/// sovrascrive. Senza un'unità predefinita (alimento senza
/// `unita_predefinita_id`), si comporta come `valida_quantita_manuale`:
/// la chiede.
pub fn valida_quantita_con_default(
    testo: &str,
    unita_default: Option<&str>,
) -> Result<(f64, String), VoceManualeError> {
    let testo = testo.trim();
    let mut parti = testo.splitn(2, char::is_whitespace);
    let numero = parti.next().unwrap_or_default().trim();
    let unita_scritta = parti.next().unwrap_or_default().trim();

    if numero.is_empty() {
        return Err(VoceManualeError::QuantitaFormatoNonValido);
    }
    let quantita: f64 = numero
        .replace(',', ".")
        .parse()
        .map_err(|_| VoceManualeError::QuantitaFormatoNonValido)?;
    if !quantita.is_finite() || quantita <= 0.0 {
        return Err(VoceManualeError::QuantitaNonPositiva);
    }

    let unita = if unita_scritta.is_empty() {
        unita_default
            .ok_or(VoceManualeError::UnitaMancante)?
            .to_string()
    } else {
        unita_scritta.to_string()
    };
    if unita.chars().count() > UNITA_MAX_CARATTERI {
        return Err(VoceManualeError::UnitaTroppoLunga);
    }
    Ok((quantita, unita))
}

/// Formatta una quantità per la UI: interi senza decimali, il resto con al
/// più due cifre senza zeri superflui (`500` invece di `500.00`, `133.33`
/// invece di `133.330000000001`).
pub fn formatta_quantita(valore: f64) -> String {
    if (valore.fract()).abs() < 1e-9 {
        return format!("{valore:.0}");
    }
    let testo = format!("{valore:.2}");
    // Virgola, non punto: è così che si scrive un numero in italiano, ed è
    // così che il bot lo accetta in entrata. Fino al 23 settembre 2026 usciva
    // "0.99" accanto a prezzi scritti "0,99 €" (Alessio, collaudo).
    testo
        .trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// La lista non deve restare indietro rispetto a oggi (chiesto da Alessio il
/// 16 settembre 2026): un intervallo scelto la settimana scorsa aggregherebbe
/// pasti ormai passati, che non si comprano più. Se l'inizio è prima di oggi
/// lo si porta a oggi; se anche la fine è ormai passata, l'intervallo riparte
/// come quello di default (oggi + 6 giorni, una settimana).
///
/// Non succede mai quando `inizio_manuale` è vero: lì l'inizio nel passato è
/// una scelta esplicita dell'utente (vedi `cambia_intervallo`), e una scelta
/// esplicita non viene corretta alle sue spalle -- viene segnalata.
///
/// Ritorna `None` quando non c'è niente da cambiare, così il chiamante non
/// scrive sul database a ogni apertura della lista.
pub fn intervallo_da_oggi(
    data_inizio: &str,
    data_fine: &str,
    oggi: &str,
    inizio_manuale: bool,
) -> Option<(String, String)> {
    if inizio_manuale || data_inizio >= oggi {
        return None;
    }
    let nuovo_inizio = oggi.to_string();
    let nuova_fine = if data_fine < nuovo_inizio.as_str() {
        calendario::shift_date(&nuovo_inizio, 6).unwrap_or_else(|| nuovo_inizio.clone())
    } else {
        data_fine.to_string()
    };
    Some((nuovo_inizio, nuova_fine))
}

/// Un'aggiunta dal catalogo con la sua riga già convertita nell'unità di
/// aggregazione -- vedi `aggiunte_coperte_dalla_spesa`.
#[derive(Debug, Clone, PartialEq)]
pub struct AggiuntaCatalogo {
    pub id: i64,
    pub riga: RigaIngrediente,
}

/// Cosa succede alle aggiunte dal catalogo quando la spesa si chiude.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EsitoAggiunte {
    /// Comprato almeno quanto chiedevano: la richiesta è servita, si tolgono.
    pub chiuse: Vec<i64>,
    /// Comprato meno di quanto chiedevano: restano con quel che manca
    /// (`id`, quantità residua nell'unità di aggregazione), e il bot chiede
    /// se tenerle.
    pub ridotte: Vec<(i64, f64)>,
}

/// Cosa fare di ogni aggiunta dal catalogo chiudendo la spesa.
///
/// Un'aggiunta dal catalogo resta viva attraverso ogni refresh (è il suo
/// scopo: "mi serve anche questo"), quindi chiudere la spesa senza toccarla
/// la farebbe ricomparire il giorno dopo come se non l'avessi mai comprata.
///
/// - **comprato ≥ chiesto**: la richiesta è servita, l'aggiunta si chiude;
/// - **comprato < chiesto**: l'aggiunta resta, ridotta a quel che manca, e
///   il bot chiede se lasciarla in lista;
/// - **comprato niente di quell'identità**: l'aggiunta resta com'è, è una
///   richiesta non ancora servita.
///
/// Prima si pretendeva la copertura intera e basta, ma la lista mostra il
/// **netto delle scorte**: chiedendo 500 g con 300 g già in casa se ne
/// comprano 200, l'aggiunta non risultava mai coperta e restava in lista per
/// sempre — invisibile (la lista era vuota), ma ancora contata nel
/// fabbisogno e ancora elencata in "🗑️ Rimuovi voci". Trovato da Alessio nel
/// collaudo del 23 settembre 2026 (punti 6 e 7); la domanda sul residuo è la
/// sua scelta del 24, al posto della chiusura silenziosa.
///
/// Le righe che vengono dal planner non compaiono qui: le gestisce il
/// planner (un pasto pianificato continua a servire finché non lo si
/// consuma o lo si toglie), esattamente come per "🗑️ Rimuovi voci".
pub fn aggiunte_coperte_dalla_spesa(
    aggiunte: &[AggiuntaCatalogo],
    comprate: &[VoceGenerata],
) -> EsitoAggiunte {
    let mut residuo: Vec<(Identita, String, f64)> = Vec::new();
    for voce in comprate {
        let identita = identita_voce(voce);
        match residuo
            .iter_mut()
            .find(|(i, unita, _)| *i == identita && unita == &voce.unita_simbolo)
        {
            Some((_, _, quantita)) => *quantita += voce.quantita,
            None => residuo.push((identita, voce.unita_simbolo.clone(), voce.quantita)),
        }
    }

    let mut ordinate: Vec<&AggiuntaCatalogo> = aggiunte.iter().collect();
    ordinate.sort_by_key(|aggiunta| aggiunta.id);

    let mut esito = EsitoAggiunte::default();
    for aggiunta in ordinate {
        let identita = identita_riga(&aggiunta.riga);
        let Some((_, _, disponibile)) = residuo
            .iter_mut()
            .find(|(i, unita, _)| *i == identita && *unita == aggiunta.riga.unita_simbolo)
        else {
            continue;
        };
        if *disponibile <= 0.0 {
            // Di questa identità si è già usato tutto il comprato per le
            // aggiunte precedenti: questa resta in piedi.
            continue;
        }
        // Si scala quello che si può, così due aggiunte dello stesso
        // alimento non si chiudono con una sola spesa piccola.
        let usato = disponibile.min(aggiunta.riga.quantita);
        *disponibile -= usato;
        let manca = aggiunta.riga.quantita - usato;
        if manca > TOLLERANZA_QUANTITA {
            esito.ridotte.push((aggiunta.id, manca));
        } else {
            esito.chiuse.push(aggiunta.id);
        }
    }
    esito
}

/// Una scorta vista dalla lista della spesa, già nell'unità-base: a quale
/// alimento appartiene (anche quando è un prodotto specifico), il prodotto
/// se c'è, e il nome per le scorte scritte a mano.
#[derive(Debug, Clone, PartialEq)]
pub struct ScortaDisponibile {
    pub alimento_id: Option<i64>,
    pub prodotto_id: Option<i64>,
    pub nome: String,
    pub quantita: f64,
    pub unita_simbolo: String,
}

/// Toglie dal fabbisogno quello che c'è già in casa (chiesto da Alessio il
/// 16 settembre 2026): la lista deve dire quanto serve comprare **al
/// netto** di dispensa, frigo e freezer. Una voce interamente coperta
/// sparisce.
///
/// Regole di corrispondenza, nell'ordine:
/// - una richiesta di un **prodotto specifico** si copre solo con scorte di
///   quel prodotto;
/// - una richiesta di un **alimento generico** si copre con le scorte
///   generiche di quell'alimento e poi con quelle dei suoi prodotti (se la
///   ricetta chiede pasta, la pasta De Cecco va bene) — il contrario no;
/// - una voce senza alimento si copre con una scorta scritta a mano con lo
///   stesso nome.
///
/// Stessa unità-base, sempre: `pz` non copre `g`.
pub fn sottrai_scorte(
    fresche: Vec<VoceGenerata>,
    scorte: &[ScortaDisponibile],
) -> Vec<VoceGenerata> {
    let mut residuo: Vec<f64> = scorte.iter().map(|scorta| scorta.quantita).collect();
    let mut prendi = |voce: &VoceGenerata, adatta: &dyn Fn(&ScortaDisponibile) -> bool| {
        let mut manca = voce.quantita;
        for (indice, scorta) in scorte.iter().enumerate() {
            if manca <= 0.0 {
                break;
            }
            if scorta.unita_simbolo != voce.unita_simbolo || !adatta(scorta) {
                continue;
            }
            let presa = manca.min(residuo[indice]);
            residuo[indice] -= presa;
            manca -= presa;
        }
        manca
    };

    // Prima i prodotti specifici, che possono usare solo le proprie scorte:
    // se l'alimento generico passasse per primo, potrebbe consumare la
    // scorta del prodotto che la richiesta specifica avrebbe dovuto usare.
    let mut risultato: Vec<Option<VoceGenerata>> = fresche.into_iter().map(Some).collect();
    for passata in 0..2 {
        for voce in risultato.iter_mut() {
            let Some(corrente) = voce.as_ref() else {
                continue;
            };
            let manca = match (passata, identita_voce(corrente)) {
                (0, Identita::Prodotto(prodotto)) => {
                    prendi(corrente, &|s| s.prodotto_id == Some(prodotto))
                }
                (1, Identita::Alimento(alimento)) => {
                    let generica = prendi(corrente, &|s| {
                        s.prodotto_id.is_none() && s.alimento_id == Some(alimento)
                    });
                    let parziale = VoceGenerata {
                        quantita: generica,
                        ..corrente.clone()
                    };
                    prendi(&parziale, &|s| {
                        s.prodotto_id.is_some() && s.alimento_id == Some(alimento)
                    })
                }
                (1, Identita::Nome(nome)) => prendi(corrente, &|s| {
                    s.alimento_id.is_none()
                        && s.prodotto_id.is_none()
                        && s.nome.trim().to_lowercase() == nome
                }),
                _ => continue,
            };
            let manca = arrotonda(manca);
            if manca > 0.0 {
                if let Some(voce) = voce.as_mut() {
                    voce.quantita = manca;
                }
            } else {
                *voce = None;
            }
        }
    }
    risultato.into_iter().flatten().collect()
}

/// Che cosa è cambiato in una voce dopo un aggiornamento della lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoModifica {
    Aggiunta,
    Tolta,
    Aumentata,
    Ridotta,
}

impl TipoModifica {
    pub fn token(self) -> &'static str {
        match self {
            Self::Aggiunta => "aggiunta",
            Self::Tolta => "tolta",
            Self::Aumentata => "aumentata",
            Self::Ridotta => "ridotta",
        }
    }

    pub fn da_token(valore: &str) -> Option<Self> {
        match valore {
            "aggiunta" => Some(Self::Aggiunta),
            "tolta" => Some(Self::Tolta),
            "aumentata" => Some(Self::Aumentata),
            "ridotta" => Some(Self::Ridotta),
            _ => None,
        }
    }

    pub fn icona(self) -> &'static str {
        match self {
            Self::Aggiunta => "➕",
            Self::Tolta => "➖",
            Self::Aumentata => "🔼",
            Self::Ridotta => "🔽",
        }
    }
}

/// Una riga del resoconto "📋 Cosa è cambiato".
#[derive(Debug, Clone, PartialEq)]
pub struct Modifica {
    pub tipo: TipoModifica,
    pub descrizione: String,
    pub unita_simbolo: String,
    pub quantita_prima: Option<f64>,
    pub quantita_dopo: Option<f64>,
    pub motivo: Option<String>,
}

/// Perché una voce è calata o sparita.
pub const MOTIVO_IN_CASA: &str = "ce l'hai già in casa";
pub const MOTIVO_NON_PIANIFICATA: &str = "non serve più per i pasti pianificati";

/// Confronta i totali per voce prima e dopo un aggiornamento e dice cosa è
/// cambiato. Si confrontano i **totali** di ogni alimento (voci comprate e
/// non comprate insieme), non le singole righe: così spuntare o togliere la
/// spunta, che fonde o separa righe dello stesso alimento senza cambiarne
/// il totale, non produce un resoconto inutile.
///
/// `in_casa` dice, per una voce calata o sparita, se il motivo è che
/// adesso c'è in dispensa/frigo/freezer.
pub fn confronta_totali(
    prima: &[VoceGenerata],
    dopo: &[VoceGenerata],
    in_casa: impl Fn(&VoceGenerata) -> bool,
) -> Vec<Modifica> {
    let stessa = |a: &VoceGenerata, b: &VoceGenerata| {
        identita_voce(a) == identita_voce(b) && a.unita_simbolo == b.unita_simbolo
    };
    let motivo = |voce: &VoceGenerata| {
        Some(
            if in_casa(voce) {
                MOTIVO_IN_CASA
            } else {
                MOTIVO_NON_PIANIFICATA
            }
            .to_string(),
        )
    };
    let mut modifiche = Vec::new();
    for vecchia in prima {
        match dopo.iter().find(|nuova| stessa(vecchia, nuova)) {
            None => modifiche.push(Modifica {
                tipo: TipoModifica::Tolta,
                descrizione: vecchia.nome.clone(),
                unita_simbolo: vecchia.unita_simbolo.clone(),
                quantita_prima: Some(vecchia.quantita),
                quantita_dopo: None,
                motivo: motivo(vecchia),
            }),
            Some(nuova) if arrotonda(nuova.quantita - vecchia.quantita) > 0.0 => {
                modifiche.push(Modifica {
                    tipo: TipoModifica::Aumentata,
                    descrizione: nuova.nome.clone(),
                    unita_simbolo: nuova.unita_simbolo.clone(),
                    quantita_prima: Some(vecchia.quantita),
                    quantita_dopo: Some(nuova.quantita),
                    motivo: None,
                })
            }
            Some(nuova) if arrotonda(vecchia.quantita - nuova.quantita) > 0.0 => {
                modifiche.push(Modifica {
                    tipo: TipoModifica::Ridotta,
                    descrizione: nuova.nome.clone(),
                    unita_simbolo: nuova.unita_simbolo.clone(),
                    quantita_prima: Some(vecchia.quantita),
                    quantita_dopo: Some(nuova.quantita),
                    motivo: motivo(nuova),
                })
            }
            Some(_) => {}
        }
    }
    for nuova in dopo {
        if !prima.iter().any(|vecchia| stessa(vecchia, nuova)) {
            modifiche.push(Modifica {
                tipo: TipoModifica::Aggiunta,
                descrizione: nuova.nome.clone(),
                unita_simbolo: nuova.unita_simbolo.clone(),
                quantita_prima: None,
                quantita_dopo: Some(nuova.quantita),
                motivo: None,
            });
        }
    }
    modifiche
}

/// `+1 nuova, 2 tolte, 1 ridotta` — il riepilogo in testa alla lista.
pub fn riepilogo_modifiche(modifiche: &[Modifica]) -> String {
    let conta = |tipo: TipoModifica| {
        modifiche
            .iter()
            .filter(|modifica| modifica.tipo == tipo)
            .count()
    };
    let parti: Vec<String> = [
        (TipoModifica::Aggiunta, "nuova", "nuove"),
        (TipoModifica::Tolta, "tolta", "tolte"),
        (TipoModifica::Aumentata, "aumentata", "aumentate"),
        (TipoModifica::Ridotta, "ridotta", "ridotte"),
    ]
    .iter()
    .filter_map(|(tipo, singolare, plurale)| {
        let quante = conta(*tipo);
        match quante {
            0 => None,
            1 => Some(format!("1 {singolare}")),
            _ => Some(format!("{quante} {plurale}")),
        }
    })
    .collect();
    let testo = parti.join(", ");
    if modifiche.iter().any(|m| m.tipo == TipoModifica::Aggiunta) {
        format!("+{testo}")
    } else {
        testo
    }
}

/// Una riga del dettaglio: `🔽 🥛 Latte · 500 → 300 ml — ce l'hai già in
/// casa`.
pub fn riga_modifica(modifica: &Modifica) -> String {
    let unita = &modifica.unita_simbolo;
    let quantita = match (modifica.quantita_prima, modifica.quantita_dopo) {
        (Some(prima), Some(dopo)) => format!(
            "{} → {} {unita}",
            formatta_quantita(prima),
            formatta_quantita(dopo)
        ),
        (None, Some(dopo)) => format!("{} {unita}", formatta_quantita(dopo)),
        (Some(prima), None) => format!("{} {unita}", formatta_quantita(prima)),
        (None, None) => String::new(),
    };
    let motivo = modifica
        .motivo
        .as_deref()
        .map(|motivo| format!(" — {motivo}"))
        .unwrap_or_default();
    format!(
        "{} {} · {quantita}{motivo}",
        modifica.tipo.icona(),
        modifica.descrizione
    )
}

#[cfg(test)]
mod domain_tests {
    use super::*;

    fn info_massa() -> InfoUnita {
        InfoUnita {
            famiglia: Some(FamigliaConversione::Massa),
            fattore_num: 1000.0,
            fattore_den: 1.0,
        }
    }

    fn info_grammo() -> InfoUnita {
        InfoUnita {
            famiglia: Some(FamigliaConversione::Massa),
            fattore_num: 1.0,
            fattore_den: 1.0,
        }
    }

    fn info_volume() -> InfoUnita {
        InfoUnita {
            famiglia: Some(FamigliaConversione::Volume),
            fattore_num: 1.0,
            fattore_den: 1.0,
        }
    }

    fn unita_standard(simbolo: &str) -> Option<InfoUnita> {
        match simbolo {
            "g" => Some(info_grammo()),
            "kg" => Some(info_massa()),
            "ml" => Some(info_volume()),
            _ => None,
        }
    }

    #[test]
    fn stessa_famiglia_convertita_e_sommata() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: None,
                nome: "Farina".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 200.0,
            },
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: None,
                nome: "Farina".to_string(),
                unita_simbolo: "kg".to_string(),
                quantita: 0.3,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].unita_simbolo, "g");
        assert_eq!(voci[0].quantita, 500.0);
    }

    #[test]
    fn famiglie_diverse_restano_separate() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: None,
                nome: "Farina".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 100.0,
            },
            RigaIngrediente {
                alimento_id: Some(2),
                prodotto_id: None,
                nome: "Latte".to_string(),
                unita_simbolo: "ml".to_string(),
                quantita: 200.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 2);
    }

    #[test]
    fn unita_sconosciuta_non_convertita_ma_aggregata_per_simbolo_esatto() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(3),
                prodotto_id: None,
                nome: "Yogurt".to_string(),
                unita_simbolo: "confezione".to_string(),
                quantita: 2.0,
            },
            RigaIngrediente {
                alimento_id: Some(3),
                prodotto_id: None,
                nome: "Yogurt".to_string(),
                unita_simbolo: "confezione".to_string(),
                quantita: 1.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].unita_simbolo, "confezione");
        assert_eq!(voci[0].quantita, 3.0);
    }

    #[test]
    fn unita_senza_famiglia_si_aggregano_per_simbolo_esatto() {
        let unita = |simbolo: &str| match simbolo {
            "pz" => Some(InfoUnita {
                famiglia: None,
                fattore_num: 1.0,
                fattore_den: 1.0,
            }),
            _ => None,
        };
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(4),
                prodotto_id: None,
                nome: "Uova".to_string(),
                unita_simbolo: "pz".to_string(),
                quantita: 2.0,
            },
            RigaIngrediente {
                alimento_id: Some(4),
                prodotto_id: None,
                nome: "Uova".to_string(),
                unita_simbolo: "pz".to_string(),
                quantita: 4.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].quantita, 6.0);
    }

    #[test]
    fn simboli_non_convertibili_diversi_restano_separati() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(5),
                prodotto_id: None,
                nome: "Sale".to_string(),
                unita_simbolo: "cucchiaio".to_string(),
                quantita: 1.0,
            },
            RigaIngrediente {
                alimento_id: Some(5),
                prodotto_id: None,
                nome: "Sale".to_string(),
                unita_simbolo: "cucchiaino".to_string(),
                quantita: 1.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 2);
    }

    #[test]
    fn righe_con_quantita_non_valida_vengono_ignorate() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(6),
                prodotto_id: None,
                nome: "Zero".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 0.0,
            },
            RigaIngrediente {
                alimento_id: Some(6),
                prodotto_id: None,
                nome: "Negativo".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: -5.0,
            },
            RigaIngrediente {
                alimento_id: Some(6),
                prodotto_id: None,
                nome: "NonFinito".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: f64::NAN,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert!(voci.is_empty());
    }

    #[test]
    fn senza_alimento_id_si_aggrega_per_nome_normalizzato() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: None,
                prodotto_id: None,
                nome: "Pane".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 100.0,
            },
            RigaIngrediente {
                alimento_id: None,
                prodotto_id: None,
                nome: "  pane  ".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 50.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].quantita, 150.0);
    }

    #[test]
    fn alimento_id_prevale_sul_nome_anche_se_e_cambiato() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(7),
                prodotto_id: None,
                nome: "Passata".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 200.0,
            },
            RigaIngrediente {
                alimento_id: Some(7),
                prodotto_id: None,
                nome: "Passata di pomodoro".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 300.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].quantita, 500.0);
    }

    #[test]
    fn arrotondamento_a_due_decimali() {
        assert_eq!(arrotonda(133.333_333), 133.33);
        assert_eq!(arrotonda(0.1 + 0.2), 0.3);
    }

    #[test]
    fn descrizione_manuale_valida_e_rifiuta_vuota() {
        assert_eq!(
            valida_descrizione_manuale("  Detersivo piatti  "),
            Ok("Detersivo piatti".to_string())
        );
        assert_eq!(
            valida_descrizione_manuale("   "),
            Err(VoceManualeError::DescrizioneVuota)
        );
        assert_eq!(
            valida_descrizione_manuale(&"x".repeat(121)),
            Err(VoceManualeError::DescrizioneTroppoLunga)
        );
    }

    #[test]
    fn quantita_manuale_interpreta_numero_e_unita() {
        assert_eq!(
            valida_quantita_manuale("500 g"),
            Ok((500.0, "g".to_string()))
        );
        assert_eq!(valida_quantita_manuale("1,5 l"), Ok((1.5, "l".to_string())));
    }

    #[test]
    fn quantita_manuale_rifiuta_formati_non_validi() {
        assert_eq!(
            valida_quantita_manuale("g"),
            Err(VoceManualeError::QuantitaFormatoNonValido)
        );
        assert_eq!(
            valida_quantita_manuale("500"),
            Err(VoceManualeError::UnitaMancante)
        );
        assert_eq!(
            valida_quantita_manuale("0 g"),
            Err(VoceManualeError::QuantitaNonPositiva)
        );
        assert_eq!(
            valida_quantita_manuale("-3 g"),
            Err(VoceManualeError::QuantitaNonPositiva)
        );
    }

    #[test]
    fn quantita_con_default_usa_l_unita_predefinita_se_non_scritta() {
        assert_eq!(
            valida_quantita_con_default("500", Some("g")),
            Ok((500.0, "g".to_string()))
        );
        assert_eq!(
            valida_quantita_con_default("1,5", Some("kg")),
            Ok((1.5, "kg".to_string()))
        );
    }

    #[test]
    fn quantita_con_default_si_puo_sovrascrivere() {
        // Scrivere comunque un'unità la sovrascrive, anche se ne esiste
        // una predefinita per l'alimento scelto.
        assert_eq!(
            valida_quantita_con_default("500 ml", Some("g")),
            Ok((500.0, "ml".to_string()))
        );
    }

    #[test]
    fn quantita_con_default_senza_predefinita_richiede_l_unita() {
        // Un alimento senza `unita_predefinita_id` si comporta come la
        // voce libera: l'unità va scritta.
        assert_eq!(
            valida_quantita_con_default("500", None),
            Err(VoceManualeError::UnitaMancante)
        );
        assert_eq!(
            valida_quantita_con_default("500 g", None),
            Ok((500.0, "g".to_string()))
        );
    }

    #[test]
    fn formattazione_quantita_evita_decimali_inventati() {
        assert_eq!(formatta_quantita(500.0), "500");
        // Virgola, come si scrive e come si legge in italiano (23 settembre
        // 2026): prima usciva "133.33" accanto a prezzi scritti "1,29 €".
        assert_eq!(formatta_quantita(133.33), "133,33");
        assert_eq!(formatta_quantita(0.3), "0,3");
        assert_eq!(formatta_quantita(1.5), "1,5");
    }

    #[test]
    fn sottrai_gia_comprato_lascia_solo_la_differenza() {
        let fresche = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 350.0,
            unita_simbolo: "g".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 200.0,
            unita_simbolo: "g".to_string(),
        }];
        let residuo = sottrai_gia_comprato(fresche, &gia_comprato);
        assert_eq!(residuo.len(), 1);
        assert_eq!(residuo[0].quantita, 150.0);
    }

    #[test]
    fn sottrai_gia_comprato_toglie_la_voce_se_gia_coperta_del_tutto() {
        let fresche = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 200.0,
            unita_simbolo: "g".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 300.0,
            unita_simbolo: "g".to_string(),
        }];
        assert!(sottrai_gia_comprato(fresche, &gia_comprato).is_empty());
    }

    #[test]
    fn sottrai_gia_comprato_ignora_unita_diverse() {
        let fresche = vec![VoceGenerata {
            alimento_id: Some(2),
            prodotto_id: None,
            nome: "Latte".to_string(),
            quantita: 500.0,
            unita_simbolo: "ml".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(2),
            prodotto_id: None,
            nome: "Latte".to_string(),
            quantita: 1.0,
            unita_simbolo: "l".to_string(),
        }];
        let residuo = sottrai_gia_comprato(fresche, &gia_comprato);
        assert_eq!(residuo.len(), 1);
        assert_eq!(residuo[0].quantita, 500.0);
    }

    #[test]
    fn prodotto_specifico_non_si_fonde_con_l_alimento_generico_anche_a_parita_di_alimento() {
        // Miglioramento richiesto da Alessio dopo il primo collaudo dal vivo:
        // 200 g di "Pasta" (alimento generico, dal planner) e 500 g di
        // "Pasta De Cecco" (prodotto specifico, alimento_id sottostante
        // uguale) devono restare due righe distinte, mai una sola voce.
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: None,
                nome: "Pasta".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 200.0,
            },
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: Some(50),
                nome: "Pasta De Cecco".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 500.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 2);
        let generico = voci.iter().find(|v| v.prodotto_id.is_none()).unwrap();
        assert_eq!(generico.quantita, 200.0);
        let specifico = voci.iter().find(|v| v.prodotto_id == Some(50)).unwrap();
        assert_eq!(specifico.quantita, 500.0);
    }

    #[test]
    fn alimento_generico_aggiunto_a_mano_si_somma_al_fabbisogno_del_planner() {
        // L'esempio esatto di Alessio: 200 g di pasta dal planner + 50 g
        // aggiunti a mano sullo stesso alimento generico devono dare 250 g
        // in un'unica riga, non due.
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: None,
                nome: "Pasta".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 200.0,
            },
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: None,
                nome: "Pasta".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 50.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].quantita, 250.0);
    }

    #[test]
    fn due_prodotti_specifici_uguali_si_sommano_tra_loro() {
        let righe = vec![
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: Some(50),
                nome: "Pasta De Cecco".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 500.0,
            },
            RigaIngrediente {
                alimento_id: Some(1),
                prodotto_id: Some(50),
                nome: "Pasta De Cecco".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 250.0,
            },
        ];
        let voci = aggrega_ingredienti(&righe, unita_standard);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].quantita, 750.0);
    }

    #[test]
    fn eccesso_quando_il_comprato_supera_il_fabbisogno_sceso() {
        // Un pasto tolto dal planner (o una ricetta ridotta) fa scendere il
        // fabbisogno reale sotto quanto già segnato comprato: l'eccesso è
        // la differenza, mai una correzione automatica della voce comprata.
        let fresche_grezze = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 200.0,
            unita_simbolo: "g".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 350.0,
            unita_simbolo: "g".to_string(),
        }];
        let eccessi = calcola_eccessi(&fresche_grezze, &gia_comprato);
        assert_eq!(eccessi.len(), 1);
        assert_eq!(eccessi[0].nome, "Farina");
        assert_eq!(eccessi[0].quantita_eccesso, 150.0);
    }

    #[test]
    fn nessun_eccesso_quando_il_fabbisogno_copre_ancora_il_comprato() {
        let fresche_grezze = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 400.0,
            unita_simbolo: "g".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 350.0,
            unita_simbolo: "g".to_string(),
        }];
        assert!(calcola_eccessi(&fresche_grezze, &gia_comprato).is_empty());
    }

    #[test]
    fn nessun_eccesso_quando_il_fabbisogno_e_sparito_del_tutto() {
        // Il pasto è stato tolto del tutto dal planner: nessuna riga fresca
        // per quella identità, tutto il comprato è in eccesso.
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(1),
            prodotto_id: None,
            nome: "Farina".to_string(),
            quantita: 200.0,
            unita_simbolo: "g".to_string(),
        }];
        let eccessi = calcola_eccessi(&[], &gia_comprato);
        assert_eq!(eccessi.len(), 1);
        assert_eq!(eccessi[0].quantita_eccesso, 200.0);
    }

    #[test]
    fn intervallo_rimasto_indietro_riparte_da_oggi() {
        // Fine ancora futura: si sposta solo l'inizio, la fine resta dov'è.
        let esito = intervallo_da_oggi("2026-09-10", "2026-09-20", "2026-09-16", false);
        assert_eq!(
            esito,
            Some(("2026-09-16".to_string(), "2026-09-20".to_string()))
        );
    }

    #[test]
    fn intervallo_tutto_passato_torna_alla_settimana_da_oggi() {
        let esito = intervallo_da_oggi("2026-09-01", "2026-09-07", "2026-09-16", false);
        assert_eq!(
            esito,
            Some(("2026-09-16".to_string(), "2026-09-22".to_string()))
        );
    }

    #[test]
    fn intervallo_gia_da_oggi_non_si_tocca() {
        assert_eq!(
            intervallo_da_oggi("2026-09-16", "2026-09-22", "2026-09-16", false),
            None
        );
        assert_eq!(
            intervallo_da_oggi("2026-09-20", "2026-09-27", "2026-09-16", false),
            None
        );
    }

    #[test]
    fn inizio_scelto_dall_utente_nel_passato_non_viene_spostato() {
        // La scelta esplicita vince: la schermata la segnala, il codice non
        // la corregge alle spalle dell'utente.
        assert_eq!(
            intervallo_da_oggi("2026-09-01", "2026-09-07", "2026-09-16", true),
            None
        );
    }

    fn aggiunta(id: i64, alimento_id: i64, quantita: f64, unita: &str) -> AggiuntaCatalogo {
        AggiuntaCatalogo {
            id,
            riga: RigaIngrediente {
                alimento_id: Some(alimento_id),
                prodotto_id: None,
                nome: "Pasta".to_string(),
                unita_simbolo: unita.to_string(),
                quantita,
            },
        }
    }

    fn comprata(alimento_id: i64, quantita: f64, unita: &str) -> VoceGenerata {
        VoceGenerata {
            alimento_id: Some(alimento_id),
            prodotto_id: None,
            nome: "Pasta".to_string(),
            quantita,
            unita_simbolo: unita.to_string(),
        }
    }

    #[test]
    fn chiusura_chiude_le_aggiunte_servite_e_riduce_quelle_a_meta() {
        // 100 g comprati per due aggiunte dello stesso alimento: la prima
        // (50 g) è servita e si chiude, la seconda (200 g) resta con i 150 g
        // che mancano -- e su quelli il bot chiede se lasciarli in lista
        // (scelta di Alessio, 24 settembre 2026).
        let aggiunte = vec![aggiunta(1, 7, 50.0, "g"), aggiunta(2, 7, 200.0, "g")];
        let esito = aggiunte_coperte_dalla_spesa(&aggiunte, &[comprata(7, 100.0, "g")]);
        assert_eq!(esito.chiuse, vec![1]);
        assert_eq!(esito.ridotte, vec![(2, 150.0)]);

        // Comprato tutto quello che si era chiesto: niente da chiedere.
        let esito = aggiunte_coperte_dalla_spesa(&aggiunte, &[comprata(7, 250.0, "g")]);
        assert_eq!(esito.chiuse, vec![1, 2]);
        assert!(esito.ridotte.is_empty());

        // Di quell'alimento non si è comprato niente: le aggiunte restano
        // com'erano, sono richieste non ancora servite.
        let esito = aggiunte_coperte_dalla_spesa(&aggiunte, &[comprata(9, 100.0, "g")]);
        assert!(esito.chiuse.is_empty());
        assert!(esito.ridotte.is_empty());
    }

    #[test]
    fn chiusura_non_tocca_un_aggiunta_di_un_altro_alimento_o_unita() {
        let aggiunte = vec![aggiunta(1, 7, 50.0, "g"), aggiunta(2, 9, 50.0, "g")];
        // Il comprato è di un altro alimento: nessuna aggiunta è toccata.
        let esito = aggiunte_coperte_dalla_spesa(&aggiunte, &[comprata(3, 500.0, "g")]);
        assert_eq!(esito, EsitoAggiunte::default());
        // Stesso alimento ma unità diversa: non si scala (`pz` e `g` non si
        // convertono fra loro).
        let esito = aggiunte_coperte_dalla_spesa(&aggiunte, &[comprata(7, 500.0, "pz")]);
        assert_eq!(esito, EsitoAggiunte::default());
    }

    fn in_casa(alimento: Option<i64>, prodotto: Option<i64>, q: f64, u: &str) -> ScortaDisponibile {
        ScortaDisponibile {
            alimento_id: alimento,
            prodotto_id: prodotto,
            nome: "Scorta".to_string(),
            quantita: q,
            unita_simbolo: u.to_string(),
        }
    }

    fn serve(alimento: Option<i64>, prodotto: Option<i64>, nome: &str, q: f64) -> VoceGenerata {
        VoceGenerata {
            alimento_id: alimento,
            prodotto_id: prodotto,
            nome: nome.to_string(),
            quantita: q,
            unita_simbolo: "g".to_string(),
        }
    }

    #[test]
    fn la_lista_dice_quanto_manca_al_netto_di_quello_che_c_e_in_casa() {
        // Servono 500 g di pasta, ne ho 300: ne restano 200.
        let risultato = sottrai_scorte(
            vec![serve(Some(7), None, "Pasta", 500.0)],
            &[in_casa(Some(7), None, 300.0, "g")],
        );
        assert_eq!(risultato.len(), 1);
        assert_eq!(risultato[0].quantita, 200.0);

        // Ne ho di più: la voce sparisce.
        assert!(sottrai_scorte(
            vec![serve(Some(7), None, "Pasta", 500.0)],
            &[in_casa(Some(7), None, 800.0, "g")],
        )
        .is_empty());

        // Unità diverse non si coprono.
        assert_eq!(
            sottrai_scorte(
                vec![serve(Some(7), None, "Pasta", 500.0)],
                &[in_casa(Some(7), None, 3.0, "pz")],
            )[0]
            .quantita,
            500.0
        );
    }

    #[test]
    fn un_prodotto_in_casa_copre_l_alimento_generico_ma_non_il_contrario() {
        // La pasta De Cecco (prodotto 70 dell'alimento 7) va bene quando la
        // ricetta chiede "pasta".
        assert!(sottrai_scorte(
            vec![serve(Some(7), None, "Pasta", 500.0)],
            &[in_casa(Some(7), Some(70), 500.0, "g")],
        )
        .is_empty());
        // Ma della pasta qualunque non copre chi vuole proprio la De Cecco.
        assert_eq!(
            sottrai_scorte(
                vec![serve(Some(7), Some(70), "De Cecco", 500.0)],
                &[in_casa(Some(7), None, 500.0, "g")],
            )[0]
            .quantita,
            500.0
        );
        // E la scorta del prodotto va prima a chi chiede il prodotto: il
        // generico prende solo quello che avanza.
        let risultato = sottrai_scorte(
            vec![
                serve(Some(7), None, "Pasta", 300.0),
                serve(Some(7), Some(70), "De Cecco", 400.0),
            ],
            &[in_casa(Some(7), Some(70), 500.0, "g")],
        );
        assert_eq!(risultato.len(), 1);
        assert_eq!(risultato[0].nome, "Pasta");
        assert_eq!(risultato[0].quantita, 200.0);
    }

    #[test]
    fn una_voce_senza_alimento_si_copre_con_una_scorta_con_lo_stesso_nome() {
        let mut scorta = in_casa(None, None, 1.0, "g");
        scorta.nome = "Detersivo".to_string();
        assert!(sottrai_scorte(vec![serve(None, None, "detersivo", 1.0)], &[scorta]).is_empty());
    }

    #[test]
    fn il_resoconto_dice_cosa_e_cambiato_e_perche() {
        let prima = vec![
            serve(Some(1), None, "Pasta", 250.0),
            serve(Some(2), None, "Sovracosce", 125.0),
            serve(Some(3), None, "Latte", 500.0),
            serve(Some(4), None, "Farina", 200.0),
        ];
        let dopo = vec![
            serve(Some(3), None, "Latte", 300.0),
            serve(Some(4), None, "Farina", 350.0),
            serve(Some(5), None, "Uova", 6.0),
        ];
        // Il latte è calato perché ce n'è in casa; il resto no.
        let modifiche = confronta_totali(&prima, &dopo, |voce| voce.alimento_id == Some(3));
        let tipi: Vec<(TipoModifica, &str)> = modifiche
            .iter()
            .map(|m| (m.tipo, m.descrizione.as_str()))
            .collect();
        assert_eq!(
            tipi,
            vec![
                (TipoModifica::Tolta, "Pasta"),
                (TipoModifica::Tolta, "Sovracosce"),
                (TipoModifica::Ridotta, "Latte"),
                (TipoModifica::Aumentata, "Farina"),
                (TipoModifica::Aggiunta, "Uova"),
            ]
        );
        assert_eq!(modifiche[0].motivo.as_deref(), Some(MOTIVO_NON_PIANIFICATA));
        assert_eq!(modifiche[2].motivo.as_deref(), Some(MOTIVO_IN_CASA));
        assert_eq!(modifiche[3].motivo, None);

        assert_eq!(
            riepilogo_modifiche(&modifiche),
            "+1 nuova, 2 tolte, 1 aumentata, 1 ridotta"
        );
        assert_eq!(
            riga_modifica(&modifiche[2]),
            "🔽 Latte · 500 → 300 g — ce l'hai già in casa"
        );
        // Nessun cambiamento, nessun resoconto.
        assert!(confronta_totali(&dopo, &dopo, |_| false).is_empty());
    }

    #[test]
    fn la_conversione_di_unita_va_e_torna() {
        let kg = Some(info_massa());
        let (base, unita) = converti_in_base(0.25, "kg", kg);
        assert_eq!((base, unita.as_str()), (250.0, "g"));
        assert_eq!(converti_da_base(250.0, kg), 0.25);
        // Senza famiglia non cambia niente.
        assert_eq!(converti_in_base(3.0, "pz", None), (3.0, "pz".to_string()));
        assert_eq!(converti_da_base(3.0, None), 3.0);
    }
}

// ===========================================================================
// Database (`sqlite::memory:` nei test).
// ===========================================================================

#[derive(Debug, Clone, FromRow)]
pub struct ListaSpesa {
    pub id: i64,
    pub proprietario_utente_id: i64,
    pub spazio_id: Option<i64>,
    pub data_inizio: String,
    pub data_fine: String,
    /// `1` quando l'utente ha scelto di proposito un inizio precedente a
    /// oggi: da quel momento la lista non si sposta più in avanti da sola
    /// (vedi `intervallo_da_oggi`), e la schermata lo segnala.
    pub inizio_manuale: i64,
    /// Il negozio di questa spesa (consegna B): i prezzi segnati mentre si
    /// è dentro al supermercato si legano a lui.
    pub negozio_id: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct VoceListaSpesa {
    pub id: i64,
    #[allow(dead_code)]
    pub origine: String,
    #[allow(dead_code)]
    pub alimento_id: Option<i64>,
    pub descrizione: String,
    pub quantita: Option<f64>,
    pub unita_simbolo: Option<String>,
    pub comprato: i64,
    #[allow(dead_code)]
    pub ordinamento: i64,
    #[allow(dead_code)]
    pub prodotto_alimentare_id: Option<i64>,
    /// Quanto si è preso davvero ("📦 Ho preso…"), se diverso da quanto
    /// serviva: è questo che entra in casa alla chiusura.
    pub quantita_presa: Option<f64>,
    pub unita_presa: Option<String>,
    #[allow(dead_code)]
    pub prodotto_preso_id: Option<i64>,
    /// Quanto è costata davvero questa voce (consegna B), se l'hai segnato.
    pub prezzo_centesimi: Option<i64>,
}

async fn trova_per_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<ListaSpesa>> {
    sqlx::query_as(
        "SELECT id, proprietario_utente_id, spazio_id, data_inizio, data_fine, inizio_manuale, \
                negozio_id \
         FROM liste_spesa WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere la lista della spesa")
}

/// Trova la lista attiva dello spazio/utente corrente, senza crearla.
pub async fn trova_lista_attiva(pool: &SqlitePool) -> anyhow::Result<Option<ListaSpesa>> {
    let actor = crate::identity::current_actor();
    sqlx::query_as(
        "SELECT id, proprietario_utente_id, spazio_id, data_inizio, data_fine, inizio_manuale, \
                negozio_id \
         FROM liste_spesa WHERE spazio_id = ? ORDER BY id LIMIT 1",
    )
    .bind(actor.spazio_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile cercare la lista della spesa")
}

/// Trova la lista attiva, creandola con un intervallo di default (oggi + 6
/// giorni, una settimana) se non esiste ancora -- stessa idea del planner:
/// nessuna creazione manuale, nasce alla prima apertura.
pub async fn trova_o_crea_lista_attiva(pool: &SqlitePool) -> anyhow::Result<ListaSpesa> {
    if let Some(lista) = trova_lista_attiva(pool).await? {
        // La lista esistente può essere rimasta indietro: si sposta in
        // avanti da sola prima di essere mostrata o usata (vedi
        // `intervallo_da_oggi`), così ogni apertura parte almeno da oggi.
        return applica_inizio_da_oggi(pool, lista).await;
    }
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let oggi: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(pool)
        .await
        .context("Impossibile leggere la data odierna")?;
    let fine = calendario::shift_date(&oggi, 6).unwrap_or_else(|| oggi.clone());
    let id = sqlx::query(
        "INSERT INTO liste_spesa (proprietario_utente_id, spazio_id, data_inizio, data_fine) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(actor.spazio_id)
    .bind(&oggi)
    .bind(&fine)
    .execute(pool)
    .await
    .context("Impossibile creare la lista della spesa")?
    .last_insert_rowid();
    trova_per_id(pool, id)
        .await?
        .context("Lista della spesa appena creata non trovata")
}

/// Porta l'inizio della lista a oggi quando è rimasto indietro, e con esso
/// la fine se anche quella è ormai passata. Non fa nulla (nessuna scrittura)
/// quando l'intervallo va già bene o quando l'inizio nel passato è una
/// scelta esplicita dell'utente -- vedi `intervallo_da_oggi`.
async fn applica_inizio_da_oggi(
    pool: &SqlitePool,
    lista: ListaSpesa,
) -> anyhow::Result<ListaSpesa> {
    let oggi = sqlx::query_scalar::<_, String>("SELECT date('now','localtime')")
        .fetch_one(pool)
        .await
        .context("Impossibile leggere la data odierna")?;
    let Some((nuovo_inizio, nuova_fine)) = intervallo_da_oggi(
        &lista.data_inizio,
        &lista.data_fine,
        &oggi,
        lista.inizio_manuale != 0,
    ) else {
        return Ok(lista);
    };
    sqlx::query(
        "UPDATE liste_spesa SET data_inizio = ?, data_fine = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(&nuovo_inizio)
    .bind(&nuova_fine)
    .bind(lista.id)
    .execute(pool)
    .await
    .context("Impossibile riportare l'intervallo della lista a oggi")?;
    Ok(ListaSpesa {
        data_inizio: nuovo_inizio,
        data_fine: nuova_fine,
        ..lista
    })
}

/// Cambia l'intervallo di una lista esistente. Non tocca le voci: il
/// ricalcolo è sempre un'azione esplicita separata (`aggiorna_lista`).
///
/// `inizio_manuale` registra se l'utente ha scelto di proposito un inizio
/// precedente a oggi: da quel momento lo spostamento automatico in avanti si
/// ferma, e riprende appena sceglie di nuovo un inizio da oggi in poi.
pub async fn cambia_intervallo(
    pool: &SqlitePool,
    lista_id: i64,
    data_inizio: &str,
    data_fine: &str,
    inizio_manuale: bool,
) -> anyhow::Result<()> {
    if !calendario::valid_date(data_inizio) || !calendario::valid_date(data_fine) {
        anyhow::bail!("Data non valida");
    }
    if data_fine < data_inizio {
        anyhow::bail!("La data di fine non può precedere quella di inizio");
    }
    sqlx::query(
        "UPDATE liste_spesa SET data_inizio = ?, data_fine = ?, inizio_manuale = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(data_inizio)
    .bind(data_fine)
    .bind(i64::from(inizio_manuale))
    .bind(lista_id)
    .execute(pool)
    .await
    .context("Impossibile cambiare l'intervallo della lista")?;
    Ok(())
}

/// Ordine indipendente dallo stato comprato (deciso con Alessio il 9
/// settembre 2026, dopo averlo visto dal vivo): prima l'ordine dipendeva da
/// `comprato ASC`, quindi spuntare una voce la faceva saltare in fondo alla
/// lista -- ora `ordinamento` è la sola chiave, e spuntare/deselezionare
/// non sposta più nulla. Vedi anche `sposta_voce`.
pub async fn carica_voci(pool: &SqlitePool, lista_id: i64) -> anyhow::Result<Vec<VoceListaSpesa>> {
    sqlx::query_as(
        "SELECT id, origine, alimento_id, descrizione, quantita, unita_simbolo, comprato, \
                ordinamento, prodotto_alimentare_id, quantita_presa, unita_presa, \
                prodotto_preso_id, prezzo_centesimi \
         FROM liste_spesa_voci WHERE lista_id = ? \
         ORDER BY ordinamento ASC, id ASC",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci della lista")
}

/// Il prossimo valore di `ordinamento` per una nuova voce di questa lista:
/// in coda a tutte le altre, così una voce appena creata non si inserisce
/// arbitrariamente in mezzo a un ordine che l'utente ha già sistemato a
/// mano con `sposta_voce`.
async fn prossimo_ordinamento(pool: &SqlitePool, lista_id: i64) -> anyhow::Result<i64> {
    let massimo: Option<i64> =
        sqlx::query_scalar("SELECT MAX(ordinamento) FROM liste_spesa_voci WHERE lista_id = ?")
            .bind(lista_id)
            .fetch_one(pool)
            .await
            .context("Impossibile leggere l'ordinamento massimo")?;
    Ok(massimo.unwrap_or(0) + 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direzione {
    Su,
    Giu,
}

/// Sposta una voce di una posizione su/giù nell'ordine visibile, scambiando
/// `ordinamento` con la voce vicina in quella direzione -- indipendente da
/// `comprato`, coerente con l'ordine unico di `carica_voci`. Non fa nulla
/// (senza errore) se la voce è già all'estremità.
pub async fn sposta_voce(
    pool: &SqlitePool,
    lista_id: i64,
    voce_id: i64,
    direzione: Direzione,
) -> anyhow::Result<()> {
    let voci = carica_voci(pool, lista_id).await?;
    let Some(posizione) = voci.iter().position(|voce| voce.id == voce_id) else {
        return Ok(());
    };
    let vicina = match direzione {
        Direzione::Su => posizione.checked_sub(1),
        Direzione::Giu => posizione.checked_add(1).filter(|&i| i < voci.len()),
    };
    let Some(vicina) = vicina else {
        return Ok(());
    };
    let (voce, voce_vicina) = (&voci[posizione], &voci[vicina]);

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    sqlx::query("UPDATE liste_spesa_voci SET ordinamento = ? WHERE id = ?")
        .bind(voce_vicina.ordinamento)
        .bind(voce.id)
        .execute(&mut *tx)
        .await
        .context("Impossibile spostare la voce")?;
    sqlx::query("UPDATE liste_spesa_voci SET ordinamento = ? WHERE id = ?")
        .bind(voce.ordinamento)
        .bind(voce_vicina.id)
        .execute(&mut *tx)
        .await
        .context("Impossibile spostare la voce vicina")?;
    tx.commit()
        .await
        .context("Impossibile salvare il nuovo ordine")?;
    Ok(())
}

/// Aggiunge una voce manuale: descrizione obbligatoria, quantità/unità
/// opzionali (ammesse `None` solo per `origine = 'manuale'`, vincolo anche a
/// database).
pub async fn aggiungi_voce_manuale(
    pool: &SqlitePool,
    lista_id: i64,
    descrizione: &str,
    quantita: Option<f64>,
    unita_simbolo: Option<&str>,
) -> anyhow::Result<i64> {
    let ordinamento = prossimo_ordinamento(pool, lista_id).await?;
    let id = sqlx::query(
        "INSERT INTO liste_spesa_voci \
         (lista_id, origine, descrizione, quantita, unita_simbolo, ordinamento) \
         VALUES (?, 'manuale', ?, ?, ?, ?)",
    )
    .bind(lista_id)
    .bind(descrizione)
    .bind(quantita)
    .bind(unita_simbolo)
    .bind(ordinamento)
    .execute(pool)
    .await
    .context("Impossibile aggiungere la voce")?
    .last_insert_rowid();
    Ok(id)
}

/// Un alimento o un prodotto del catalogo messo in lista **senza quantità**
/// (18 settembre 2026, chiesto da Alessio): "mi serve la pasta, non so
/// quanta". Diventa una voce a sé, legata al catalogo — così `📦 Ho preso…`
/// ne riconosce le confezioni — ma fuori dal calcolo del fabbisogno, che
/// senza un numero non saprebbe cosa sommare.
///
/// Alla chiusura non entra nelle scorte (non si sa quanta ne entra), a meno
/// che si segni con `📦` quanto se ne è preso: allora entra quella.
pub async fn aggiungi_catalogo_senza_quantita(
    pool: &SqlitePool,
    lista_id: i64,
    identita: IdentitaCatalogo,
    descrizione: &str,
) -> anyhow::Result<i64> {
    let (alimento_id, prodotto_id) = match identita {
        IdentitaCatalogo::Alimento(id) => (Some(id), None),
        IdentitaCatalogo::Prodotto(id) => {
            let alimento: Option<i64> =
                sqlx::query_scalar("SELECT alimento_id FROM prodotti_alimentari WHERE id = ?")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .context("Impossibile leggere il prodotto")?;
            (alimento, Some(id))
        }
    };
    let ordinamento = prossimo_ordinamento(pool, lista_id).await?;
    let id = sqlx::query(
        "INSERT INTO liste_spesa_voci \
         (lista_id, origine, alimento_id, prodotto_alimentare_id, descrizione, ordinamento) \
         VALUES (?, 'manuale', ?, ?, ?, ?)",
    )
    .bind(lista_id)
    .bind(alimento_id)
    .bind(prodotto_id)
    .bind(descrizione)
    .bind(ordinamento)
    .execute(pool)
    .await
    .context("Impossibile aggiungere la voce")?
    .last_insert_rowid();
    Ok(id)
}

/// Segna/toglie il flag comprato per una riga intera (non quantità
/// parziale, deciso con Alessio). Il trigger a database impedisce di
/// toccare gli altri campi mentre `comprato = 1`; il toggle stesso resta
/// sempre permesso.
pub async fn imposta_comprato(
    pool: &SqlitePool,
    voce_id: i64,
    comprato: bool,
) -> anyhow::Result<()> {
    if comprato {
        sqlx::query(
            "UPDATE liste_spesa_voci SET comprato = 1, \
             comprato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
    } else {
        // Togliere la spunta vuol dire "non l'ho preso": quanto era stato
        // segnato con 📦 non vale più.
        sqlx::query(
            "UPDATE liste_spesa_voci SET comprato = 0, comprato_il = NULL, \
             quantita_presa = NULL, unita_presa = NULL, prodotto_preso_id = NULL, \
             aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
    }
    .bind(voce_id)
    .execute(pool)
    .await
    .context("Impossibile aggiornare la voce")?;
    Ok(())
}

async fn toggle_comprato(pool: &SqlitePool, voce_id: i64) -> anyhow::Result<()> {
    let attuale: Option<(i64, String, i64)> =
        sqlx::query_as("SELECT comprato, origine, lista_id FROM liste_spesa_voci WHERE id = ?")
            .bind(voce_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere la voce")?;
    let (comprato, origine, lista_id) = attuale.context("Voce non trovata")?;
    let nuovo_comprato = comprato == 0;
    imposta_comprato(pool, voce_id, nuovo_comprato).await?;

    // Due casi eccezionali in cui il ricalcolo scatta da solo, entrambi
    // decisi con Alessio dopo un collaudo dal vivo -- solo per le voci
    // 'generato': una voce manuale non ha nulla con cui fondersi e non
    // viene mai toccata da un refresh, nemmeno indiretto.
    if origine == "generato" {
        if nuovo_comprato {
            // Spuntare una voce residua (creata da `sottrai_gia_comprato`
            // per la sola differenza, mentre un'altra riga dello stesso
            // alimento era già comprata) non deve lasciare due righe
            // comprate separate per sempre: si fondono in una sola.
            fondi_comprate_se_serve(pool, lista_id, voce_id).await?;
        } else {
            // Togliere la spunta la rende di nuovo disponibile al refresh
            // (`aggiorna_lista` non tocca mai le voci comprate), ma se nel
            // frattempo un altro pasto ha già prodotto una riga nuova per
            // la differenza, senza un ricalcolo l'utente vede due righe
            // frammentate finché non preme "Aggiorna lista" a mano. Qui si
            // rifonde subito, senza aspettare -- ma solo se l'utente vuole
            // che la lista si aggiorni da sola: con l'aggiornamento
            // automatico spento, la lista cambia solo quando lo chiede lui
            // (deciso con Alessio il 16 settembre 2026, dopo che una
            // despunta gli aveva svuotato la lista senza avvisare).
            if aggiornamento_automatico(pool).await {
                if let Some(lista) = trova_per_id(pool, lista_id).await? {
                    aggiorna_e_registra(pool, &lista, true).await?;
                }
            }
        }
    }
    Ok(())
}

/// Se un'altra voce `generato` già comprata condivide la stessa identità
/// (alimento, prodotto, o nome se nessuno dei due è disponibile) e la
/// stessa unità di `voce_id`, le fonde in una sola: somma le quantità nella
/// voce appena spuntata ed elimina l'altra. Il trigger di congelamento
/// impedisce di cambiare la quantità di una riga con `comprato = 1`, quindi
/// si passa da un giro comprato→0→1 (solo il toggle stesso, mai gli altri
/// campi, resta permesso mentre `comprato = 1`) dentro un'unica
/// transazione, cosa che nessun percorso normale dell'utente può fare.
type RigaFusioneGrezza = (Option<i64>, Option<i64>, String, f64, String);
type CandidatoFusioneGrezzo = (i64, Option<i64>, Option<i64>, String, f64);

async fn fondi_comprate_se_serve(
    pool: &SqlitePool,
    lista_id: i64,
    voce_id: i64,
) -> anyhow::Result<()> {
    let riga: Option<RigaFusioneGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione, quantita, unita_simbolo \
         FROM liste_spesa_voci WHERE id = ? AND origine = 'generato' AND comprato = 1 \
           AND quantita_presa IS NULL",
    )
    .bind(voce_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere la voce appena comprata")?;
    let Some((alimento_id, prodotto_id, nome, quantita, unita_simbolo)) = riga else {
        return Ok(());
    };
    let identita = identita_voce(&VoceGenerata {
        alimento_id,
        prodotto_id,
        nome: nome.clone(),
        quantita,
        unita_simbolo: unita_simbolo.clone(),
    });

    let candidate: Vec<CandidatoFusioneGrezzo> = sqlx::query_as(
        "SELECT id, alimento_id, prodotto_alimentare_id, descrizione, quantita \
         FROM liste_spesa_voci \
         WHERE lista_id = ? AND origine = 'generato' AND comprato = 1 \
           AND id <> ? AND unita_simbolo = ? AND quantita_presa IS NULL",
    )
    .bind(lista_id)
    .bind(voce_id)
    .bind(&unita_simbolo)
    .fetch_all(pool)
    .await
    .context("Impossibile cercare un'altra voce comprata da fondere")?;

    let Some((altro_id, _, _, _, quantita_altro)) =
        candidate.into_iter().find(|(_, aid, pid, nome_altro, _)| {
            identita_voce(&VoceGenerata {
                alimento_id: *aid,
                prodotto_id: *pid,
                nome: nome_altro.clone(),
                quantita: 0.0,
                unita_simbolo: unita_simbolo.clone(),
            }) == identita
        })
    else {
        return Ok(());
    };

    let totale = arrotonda(quantita + quantita_altro);
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    sqlx::query("UPDATE liste_spesa_voci SET comprato = 0, comprato_il = NULL WHERE id = ?")
        .bind(voce_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile preparare la fusione")?;
    sqlx::query(
        "UPDATE liste_spesa_voci SET quantita = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(totale)
    .bind(voce_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile sommare la quantità fusa")?;
    sqlx::query(
        "UPDATE liste_spesa_voci SET comprato = 1, \
         comprato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(voce_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile completare la fusione")?;
    sqlx::query("DELETE FROM liste_spesa_voci WHERE id = ?")
        .bind(altro_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile eliminare la voce fusa")?;
    tx.commit()
        .await
        .context("Impossibile salvare la fusione")?;
    Ok(())
}

/// "📦 Ho preso…": segna quanto si è preso davvero (una confezione da 300 g
/// quando ne servivano 250) e, se serve, spunta la voce. Una voce con la
/// presa segnata non si fonde più con altre righe comprate: la somma di due
/// prese diverse non avrebbe un senso chiaro.
pub async fn registra_presa(
    pool: &SqlitePool,
    voce_id: i64,
    quantita: f64,
    unita: &str,
    prodotto_id: Option<i64>,
) -> anyhow::Result<()> {
    let aggiornate = sqlx::query(
        "UPDATE liste_spesa_voci SET quantita_presa = ?, unita_presa = ?, \
         prodotto_preso_id = ?, comprato = 1, \
         comprato_il = COALESCE(comprato_il, strftime('%Y-%m-%dT%H:%M:%fZ','now')), \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(arrotonda(quantita))
    .bind(unita.trim())
    .bind(prodotto_id)
    .bind(voce_id)
    .execute(pool)
    .await
    .context("Impossibile segnare quanto preso")?
    .rows_affected();
    anyhow::ensure!(aggiornate == 1, "Voce non trovata");
    Ok(())
}

/// Torna a "presa la quantità che serviva": la voce resta spuntata.
pub async fn annulla_presa(pool: &SqlitePool, voce_id: i64) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE liste_spesa_voci SET quantita_presa = NULL, unita_presa = NULL, \
         prodotto_preso_id = NULL, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(voce_id)
    .execute(pool)
    .await
    .context("Impossibile togliere quanto preso")?;
    Ok(())
}

/// Una confezione proponibile in "📦 Ho preso…": il formato base di un
/// prodotto (`token` = `p{id}`) o uno dei suoi formati aggiuntivi
/// (`f{id}`).
#[derive(Debug, Clone, PartialEq)]
pub struct Confezione {
    pub token: String,
    pub prodotto_id: i64,
    /// Marca e nome commerciale, senza il formato: il formato va a capo sul
    /// pulsante, altrimenti su Telegram Desktop veniva tagliato.
    pub nome: String,
    pub etichetta: String,
    pub quantita: f64,
    pub unita: String,
}

#[derive(Debug, FromRow)]
struct ConfezioneGrezza {
    prodotto_id: i64,
    formato_id: Option<i64>,
    marca: String,
    nome_commerciale: String,
    quantita: f64,
    unita: String,
}

const CONFEZIONI_MAX: usize = 8;

/// Le confezioni registrate per l'alimento della voce. Se la voce è già
/// legata a un prodotto preciso, solo le sue; altrimenti quelle di tutti i
/// prodotti attivi dell'alimento. Senza doppioni (stessa quantità e unità
/// dello stesso prodotto).
pub async fn confezioni_per_voce(
    pool: &SqlitePool,
    alimento_id: i64,
    prodotto_id: Option<i64>,
) -> anyhow::Result<Vec<Confezione>> {
    let righe: Vec<ConfezioneGrezza> = sqlx::query_as(
        "SELECT p.id AS prodotto_id, NULL AS formato_id, p.marca AS marca, \
                p.nome_commerciale AS nome_commerciale, \
                p.quantita_confezione AS quantita, um.simbolo AS unita \
         FROM prodotti_alimentari p \
         JOIN unita_misura um ON um.id = p.unita_confezione_id \
         WHERE p.alimento_id = ? AND p.attivo = 1 AND (? IS NULL OR p.id = ?) \
         UNION ALL \
         SELECT p.id, f.id, p.marca, p.nome_commerciale, f.quantita_confezione, um.simbolo \
         FROM formati_prodotto_alimentare f \
         JOIN prodotti_alimentari p ON p.id = f.prodotto_alimentare_id \
         JOIN unita_misura um ON um.id = f.unita_confezione_id \
         WHERE p.alimento_id = ? AND p.attivo = 1 AND f.attivo = 1 \
           AND (? IS NULL OR p.id = ?) \
         ORDER BY marca, nome_commerciale, quantita, formato_id",
    )
    .bind(alimento_id)
    .bind(prodotto_id)
    .bind(prodotto_id)
    .bind(alimento_id)
    .bind(prodotto_id)
    .bind(prodotto_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le confezioni dell'alimento")?;

    let mut confezioni: Vec<Confezione> = Vec::new();
    for riga in righe {
        let doppione = confezioni.iter().any(|altra| {
            altra.prodotto_id == riga.prodotto_id
                && altra.unita == riga.unita
                && (altra.quantita - riga.quantita).abs() < 1e-9
        });
        if doppione {
            continue;
        }
        let token = match riga.formato_id {
            Some(formato_id) => format!("f{formato_id}"),
            None => format!("p{}", riga.prodotto_id),
        };
        confezioni.push(Confezione {
            token,
            prodotto_id: riga.prodotto_id,
            nome: format!("{} {}", riga.marca, riga.nome_commerciale),
            etichetta: format!(
                "{} {} · {} {}",
                riga.marca,
                riga.nome_commerciale,
                formatta_quantita(riga.quantita),
                riga.unita
            ),
            quantita: riga.quantita,
            unita: riga.unita,
        });
        if confezioni.len() == CONFEZIONI_MAX {
            break;
        }
    }
    Ok(confezioni)
}

/// Rilegge la confezione di un pulsante (`p{id}` o `f{id}`): quantità e
/// unità vengono dal database, non dal callback.
async fn confezione_da_token(
    pool: &SqlitePool,
    token: &str,
) -> anyhow::Result<Option<(i64, f64, String)>> {
    if let Some(id) = token.strip_prefix('p').and_then(|v| v.parse::<i64>().ok()) {
        return sqlx::query_as(
            "SELECT p.id, p.quantita_confezione, um.simbolo FROM prodotti_alimentari p \
             JOIN unita_misura um ON um.id = p.unita_confezione_id WHERE p.id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere la confezione");
    }
    if let Some(id) = token.strip_prefix('f').and_then(|v| v.parse::<i64>().ok()) {
        return sqlx::query_as(
            "SELECT f.prodotto_alimentare_id, f.quantita_confezione, um.simbolo \
             FROM formati_prodotto_alimentare f \
             JOIN unita_misura um ON um.id = f.unita_confezione_id WHERE f.id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Impossibile leggere il formato");
    }
    Ok(None)
}

/// Righe di ingrediente pianificate nell'intervallo della lista, già
/// filtrate secondo le regole decise: solo pasti pianificati (non saltati,
/// non completati), solo righe con `quantita_finale_snapshot` non nullo
/// (le righe nulle sono ingredienti esclusi per quel profilo/pasto e non
/// contribuiscono), stesso spazio se la lista è condivisa altrimenti stesso
/// proprietario.
async fn righe_da_aggregare(
    pool: &SqlitePool,
    lista: &ListaSpesa,
) -> anyhow::Result<Vec<RigaIngrediente>> {
    let righe: Vec<(Option<i64>, String, String, f64)> = sqlx::query_as(
        "SELECT s.alimento_id, s.alimento_nome_snapshot, s.unita_simbolo_snapshot, \
                s.quantita_finale_snapshot \
         FROM planner_pasto_ingredienti_snapshot s \
         JOIN planner_pasti pp ON pp.id = s.pasto_id \
         JOIN planner_alimentari p ON p.id = pp.planner_id \
         WHERE pp.stato = 'pianificato' \
           AND pp.saltato_il IS NULL \
           AND pp.completato_il IS NULL \
           AND pp.scorte_scalate_il IS NULL \
           AND s.quantita_finale_snapshot IS NOT NULL \
           AND p.archiviato = 0 \
           AND date(pp.data_pasto) BETWEEN date(?1) AND date(?2) \
           AND ( \
                 (?3 IS NOT NULL AND p.spazio_id = ?3) \
              OR (?3 IS NULL AND p.spazio_id IS NULL AND p.proprietario_utente_id = ?4) \
           )",
    )
    .bind(&lista.data_inizio)
    .bind(&lista.data_fine)
    .bind(lista.spazio_id)
    .bind(lista.proprietario_utente_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere gli ingredienti pianificati")?;

    Ok(righe
        .into_iter()
        .map(
            |(alimento_id, nome, unita_simbolo, quantita)| RigaIngrediente {
                alimento_id,
                prodotto_id: None,
                nome,
                unita_simbolo,
                quantita,
            },
        )
        .collect())
}

/// Identità di un'aggiunta dal catalogo: un alimento generico (si somma al
/// fabbisogno del planner sullo stesso alimento) o un prodotto commerciale
/// specifico (resta sempre una riga separata, anche se collegato allo
/// stesso alimento generico -- vedi `Identita::Prodotto`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitaCatalogo {
    Alimento(i64),
    Prodotto(i64),
}

/// Un risultato di ricerca nel catalogo: un alimento generico o un prodotto
/// commerciale specifico, con l'etichetta già pronta per il pulsante
/// (icona distinta, C4) e la descrizione da salvare come snapshot.
#[derive(Debug, Clone, PartialEq)]
pub enum RisultatoCatalogo {
    Alimento {
        id: i64,
        nome: String,
    },
    Prodotto {
        id: i64,
        marca: String,
        nome_commerciale: String,
    },
}

impl RisultatoCatalogo {
    pub fn identita(&self) -> IdentitaCatalogo {
        match self {
            Self::Alimento { id, .. } => IdentitaCatalogo::Alimento(*id),
            Self::Prodotto { id, .. } => IdentitaCatalogo::Prodotto(*id),
        }
    }

    pub fn etichetta(&self) -> String {
        match self {
            // Il nome dell'alimento porta già la sua icona di categoria
            // incorporata (es. "🌾 Pasta", "🏷️ Pasta sfoglia" -- vedi
            // `migrations/20260825014500_catalogo_alimenti_base.sql"):
            // aggiungerne una fissa qui sopra ("🥕") duplicava l'icona su
            // ogni risultato, notato da Alessio collaudando dal vivo.
            Self::Alimento { nome, .. } => nome.clone(),
            Self::Prodotto {
                marca,
                nome_commerciale,
                ..
            } => format!("🏷️ {marca} {nome_commerciale}"),
        }
    }
}

fn normalizza_ricerca(valore: &str) -> String {
    valore
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Clausola di visibilità comune a ogni ricerca nel catalogo: catalogo
/// globale, di proprietà dell'utente corrente, o condiviso nello spazio
/// visibile -- stesso schema di `ricette::search_food_choices`, non
/// riusabile direttamente da qui perché quella funzione non restituisce
/// anche i prodotti come risultati distinti (vedi la spiegazione nel
/// commit che introduce questo modulo).
fn clausola_visibilita_alimento(alias: &str, view_all: bool) -> String {
    if view_all {
        format!(
            "{alias}.catalogo_globale = 1 OR {alias}.proprietario_utente_id = ? OR EXISTS (\
                SELECT 1 FROM alimento_spazi asp JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id \
                WHERE asp.alimento_id = {alias}.id AND ms.utente_id = ?)"
        )
    } else {
        format!(
            "{alias}.catalogo_globale = 1 OR {alias}.proprietario_utente_id = ? OR EXISTS (\
                SELECT 1 FROM alimento_spazi asp WHERE asp.alimento_id = {alias}.id AND asp.spazio_id = ?)"
        )
    }
}

/// Cerca alimenti generici e prodotti commerciali nel catalogo per il
/// flusso "➕ Aggiungi voce manuale" → "🔎 Cerca nel catalogo". Nessuna
/// paginazione vera, un `LIMIT` per parte come "top N" -- stesso
/// approccio di `ricette::search_food_choices`.
pub async fn cerca_nel_catalogo(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> anyhow::Result<Vec<RisultatoCatalogo>> {
    let actor = crate::identity::current_actor();
    let user_id = actor.utente_id.context("Utente non disponibile")?;
    let normalizzata = normalizza_ricerca(query);
    let like = format!("%{normalizzata}%");
    let visibilita = clausola_visibilita_alimento("a", actor.view_all);
    let secondo_bind = if actor.view_all {
        user_id
    } else {
        actor.spazio_id
    };

    let alimenti: Vec<(i64, String)> = sqlx::query_as(&format!(
        "SELECT DISTINCT a.id, a.nome FROM alimenti a \
         WHERE a.archiviato = 0 AND ({visibilita}) AND (\
            a.nome_normalizzato LIKE ? \
            OR EXISTS (SELECT 1 FROM alimento_alias aa WHERE aa.alimento_id = a.id AND aa.alias_normalizzato LIKE ?)\
         ) ORDER BY CASE WHEN a.nome_normalizzato = ? THEN 0 ELSE 1 END, a.nome COLLATE NOCASE, a.id LIMIT ?"
    ))
    .bind(user_id)
    .bind(secondo_bind)
    .bind(&like)
    .bind(&like)
    .bind(&normalizzata)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Impossibile cercare gli alimenti nel catalogo")?;

    let prodotti: Vec<(i64, String, String)> = sqlx::query_as(&format!(
        "SELECT DISTINCT p.id, p.marca, p.nome_commerciale \
         FROM prodotti_alimentari p JOIN alimenti a ON a.id = p.alimento_id \
         WHERE p.attivo = 1 AND a.archiviato = 0 AND ({visibilita}) AND (\
            p.marca_normalizzata LIKE ? OR p.nome_commerciale_normalizzato LIKE ?\
         ) ORDER BY p.marca COLLATE NOCASE, p.nome_commerciale COLLATE NOCASE, p.id LIMIT ?"
    ))
    .bind(user_id)
    .bind(secondo_bind)
    .bind(&like)
    .bind(&like)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Impossibile cercare i prodotti nel catalogo")?;

    let mut risultati: Vec<RisultatoCatalogo> = alimenti
        .into_iter()
        .map(|(id, nome)| RisultatoCatalogo::Alimento { id, nome })
        .collect();
    risultati.extend(prodotti.into_iter().map(|(id, marca, nome_commerciale)| {
        RisultatoCatalogo::Prodotto {
            id,
            marca,
            nome_commerciale,
        }
    }));
    Ok(risultati)
}

/// Rilegge il nome di un alimento per id, rispettando la stessa visibilità
/// della ricerca -- usata dopo la scelta di un risultato (il pulsante porta
/// solo l'id per restare sotto il limite di `callback_data`, il nome va
/// riletto per mostrare la conferma e salvare lo snapshot).
/// Ritorna anche l'unità predefinita dell'alimento (`unita_predefinita_id`),
/// se ne ha una: chiesta da Alessio dopo un collaudo dal vivo, per non
/// dover riscrivere l'unità ogni volta che si aggiunge un alimento già
/// noto al catalogo -- resta comunque sovrascrivibile al momento
/// dell'inserimento (vedi `valida_quantita_con_default`).
pub async fn alimento_visibile_per_id(
    pool: &SqlitePool,
    id: i64,
) -> anyhow::Result<Option<(String, Option<String>)>> {
    let actor = crate::identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(None);
    };
    let visibilita = clausola_visibilita_alimento("a", actor.view_all);
    let secondo_bind = if actor.view_all {
        user_id
    } else {
        actor.spazio_id
    };
    sqlx::query_as(&format!(
        "SELECT a.nome, u.simbolo FROM alimenti a \
         LEFT JOIN unita_misura u ON u.id = a.unita_predefinita_id \
         WHERE a.id = ? AND a.archiviato = 0 AND ({visibilita})"
    ))
    .bind(id)
    .bind(user_id)
    .bind(secondo_bind)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere l'alimento scelto")
}

/// Come `alimento_visibile_per_id`, per un prodotto commerciale: la
/// visibilità è quella del suo alimento generico, l'unità predefinita è
/// quella della confezione (`unita_confezione_id`, sempre presente per un
/// prodotto -- a differenza di quella, opzionale, di un alimento generico).
pub async fn prodotto_visibile_per_id(
    pool: &SqlitePool,
    id: i64,
) -> anyhow::Result<Option<(String, String, String)>> {
    let actor = crate::identity::current_actor();
    let Some(user_id) = actor.utente_id else {
        return Ok(None);
    };
    let visibilita = clausola_visibilita_alimento("a", actor.view_all);
    let secondo_bind = if actor.view_all {
        user_id
    } else {
        actor.spazio_id
    };
    sqlx::query_as(&format!(
        "SELECT p.marca, p.nome_commerciale, u.simbolo \
         FROM prodotti_alimentari p \
         JOIN alimenti a ON a.id = p.alimento_id \
         JOIN unita_misura u ON u.id = p.unita_confezione_id \
         WHERE p.id = ? AND p.attivo = 1 AND a.archiviato = 0 AND ({visibilita})"
    ))
    .bind(id)
    .bind(user_id)
    .bind(secondo_bind)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere il prodotto scelto")
}

/// Registra un'aggiunta dal catalogo (alimento generico o prodotto
/// commerciale specifico). A differenza di una voce manuale in
/// `liste_spesa_voci`, questa non è mai uno snapshot: resta nella propria
/// tabella e partecipa di nuovo ogni volta al ricalcolo di `aggiorna_lista`
/// (vedi `righe_da_aggiunte_catalogo`), finché non viene coperta da una
/// voce generata segnata comprata.
pub async fn aggiungi_da_catalogo(
    pool: &SqlitePool,
    lista_id: i64,
    identita: IdentitaCatalogo,
    descrizione_snapshot: &str,
    quantita: f64,
    unita_simbolo: &str,
) -> anyhow::Result<i64> {
    let (tipo, alimento_id, prodotto_id): (&str, Option<i64>, Option<i64>) = match identita {
        IdentitaCatalogo::Alimento(id) => ("alimento", Some(id), None),
        IdentitaCatalogo::Prodotto(id) => ("prodotto", None, Some(id)),
    };
    let id = sqlx::query(
        "INSERT INTO liste_spesa_aggiunte_catalogo \
         (lista_id, tipo, alimento_id, prodotto_alimentare_id, descrizione_snapshot, \
          quantita, unita_simbolo) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(lista_id)
    .bind(tipo)
    .bind(alimento_id)
    .bind(prodotto_id)
    .bind(descrizione_snapshot)
    .bind(quantita)
    .bind(unita_simbolo)
    .execute(pool)
    .await
    .context("Impossibile registrare l'aggiunta dal catalogo")?
    .last_insert_rowid();
    Ok(id)
}

/// Una voce manuale o un'aggiunta dal catalogo, per la schermata
/// "🗑️ Rimuovi voci" (chiesto da Alessio dopo un collaudo dal vivo: prima
/// non si poteva rimuovere né l'una né l'altra). Le righe `generato`
/// pure-planner non compaiono mai qui: sono gestite dal planner stesso, non
/// da una rimozione manuale.
#[derive(Debug, Clone, PartialEq)]
pub struct VoceRimovibile {
    pub id: i64,
    pub origine: OrigineRimovibile,
    pub descrizione: String,
    pub quantita: Option<f64>,
    pub unita_simbolo: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigineRimovibile {
    Manuale,
    Catalogo,
}

/// Voci manuali e aggiunte dal catalogo di questa lista, insieme, pronte
/// per la schermata di rimozione.
pub async fn voci_rimovibili(
    pool: &SqlitePool,
    lista_id: i64,
) -> anyhow::Result<Vec<VoceRimovibile>> {
    let manuali: Vec<(i64, String, Option<f64>, Option<String>)> = sqlx::query_as(
        "SELECT id, descrizione, quantita, unita_simbolo FROM liste_spesa_voci \
         WHERE lista_id = ? AND origine = 'manuale' ORDER BY ordinamento, id",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci manuali")?;
    let catalogo: Vec<(i64, String, f64, String)> = sqlx::query_as(
        "SELECT id, descrizione_snapshot, quantita, unita_simbolo \
         FROM liste_spesa_aggiunte_catalogo WHERE lista_id = ? ORDER BY id",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le aggiunte dal catalogo")?;

    let mut voci: Vec<VoceRimovibile> = manuali
        .into_iter()
        .map(
            |(id, descrizione, quantita, unita_simbolo)| VoceRimovibile {
                id,
                origine: OrigineRimovibile::Manuale,
                descrizione,
                quantita,
                unita_simbolo,
            },
        )
        .collect();
    voci.extend(
        catalogo.into_iter().map(
            |(id, descrizione, quantita, unita_simbolo)| VoceRimovibile {
                id,
                origine: OrigineRimovibile::Catalogo,
                descrizione,
                quantita: Some(quantita),
                unita_simbolo: Some(unita_simbolo),
            },
        ),
    );
    Ok(voci)
}

/// Rimuove una voce manuale (testo libero) per sempre. A differenza di una
/// voce `generato`, non ha nulla che la rigeneri: la cancellazione è
/// definitiva, indipendentemente dal fatto che sia comprata o meno (il
/// congelamento protegge la *quantità* di una voce comprata, non
/// l'esistenza della riga).
pub async fn rimuovi_voce_manuale(pool: &SqlitePool, voce_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM liste_spesa_voci WHERE id = ? AND origine = 'manuale'")
        .bind(voce_id)
        .execute(pool)
        .await
        .context("Impossibile rimuovere la voce manuale")?;
    Ok(())
}

/// Toglie in un colpo solo tutto ciò che è stato messo in lista a mano: le
/// voci scritte a mano e le aggiunte dal catalogo (alimenti e prodotti).
/// Restano le righe dei pasti pianificati, che le gestisce il planner.
/// Chiesto da Alessio il 18 settembre 2026, per svuotare una lista fatta a
/// mano senza toccare voce per voce. Ritorna quante voci ha tolto.
pub async fn rimuovi_tutte_le_aggiunte(pool: &SqlitePool, lista_id: i64) -> anyhow::Result<usize> {
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    let manuali =
        sqlx::query("DELETE FROM liste_spesa_voci WHERE lista_id = ? AND origine = 'manuale'")
            .bind(lista_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile togliere le voci scritte a mano")?
            .rows_affected();
    let dal_catalogo = sqlx::query("DELETE FROM liste_spesa_aggiunte_catalogo WHERE lista_id = ?")
        .bind(lista_id)
        .execute(&mut *tx)
        .await
        .context("Impossibile togliere le aggiunte dal catalogo")?
        .rows_affected();
    tx.commit()
        .await
        .context("Impossibile salvare la rimozione")?;
    Ok((manuali + dal_catalogo) as usize)
}

/// Rimuove un'aggiunta dal catalogo: smette di contribuire ai refresh
/// successivi. Non tocca da sola la riga `generato` già in lista (che un
/// refresh esplicito potrebbe ridurre o far sparire, secondo il fabbisogno
/// rimasto) -- il chiamante decide se richiamare `aggiorna_lista` subito.
pub async fn rimuovi_aggiunta_catalogo(pool: &SqlitePool, aggiunta_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM liste_spesa_aggiunte_catalogo WHERE id = ?")
        .bind(aggiunta_id)
        .execute(pool)
        .await
        .context("Impossibile rimuovere l'aggiunta dal catalogo")?;
    Ok(())
}

/// Aggiunte dal catalogo di questa lista, convertite nella stessa forma
/// delle righe del planner così `aggiorna_lista` le aggrega insieme con la
/// stessa logica di conversione unità -- restano "vive" attraverso ogni
/// refresh, a differenza delle vecchie righe `generato` pure-planner.
/// Riga grezza di `liste_spesa_aggiunte_catalogo`: alimento, prodotto,
/// descrizione, quantità e unità.
type RigaAggiuntaCatalogoGrezza = (Option<i64>, Option<i64>, String, f64, String);

async fn righe_da_aggiunte_catalogo(
    pool: &SqlitePool,
    lista_id: i64,
) -> anyhow::Result<Vec<RigaIngrediente>> {
    let righe: Vec<RigaAggiuntaCatalogoGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione_snapshot, \
                quantita, unita_simbolo \
         FROM liste_spesa_aggiunte_catalogo WHERE lista_id = ?",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le aggiunte dal catalogo")?;

    Ok(righe
        .into_iter()
        .map(
            |(alimento_id, prodotto_id, nome, quantita, unita_simbolo)| RigaIngrediente {
                alimento_id,
                prodotto_id,
                nome,
                unita_simbolo,
                quantita,
            },
        )
        .collect())
}

/// Voci `generato` già comprate di questa lista, con quantità/unità non
/// nulle (per costruzione lo sono sempre, per una voce generata) --
/// servono a `aggiorna_lista` per non duplicare ciò che è già stato
/// acquistato quando ricalcola il fresco dell'aggregazione.
/// Riga grezza di `liste_spesa_voci`: alimento, prodotto, descrizione,
/// quantità e unità -- opzionali solo perché la query li rilegge così come
/// sono le colonne, non perché possano mancare davvero su una voce
/// generata.
type RigaVoceGenerataGrezza = (
    Option<i64>,
    Option<i64>,
    String,
    Option<f64>,
    Option<String>,
);

/// Riga grezza per rileggere l'ordinamento di una voce generata non
/// comprata prima di un refresh: alimento, prodotto, descrizione, unità,
/// ordinamento -- vedi `aggiorna_lista`.
type RigaOrdinePrecedenteGrezza = (Option<i64>, Option<i64>, String, Option<String>, i64);

async fn voci_generate_comprate(
    pool: &SqlitePool,
    lista_id: i64,
) -> anyhow::Result<Vec<VoceGenerata>> {
    let righe: Vec<RigaVoceGenerataGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione, quantita, unita_simbolo \
         FROM liste_spesa_voci \
         WHERE lista_id = ? AND origine = 'generato' AND comprato = 1",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci generate già comprate")?;

    Ok(righe
        .into_iter()
        .filter_map(
            |(alimento_id, prodotto_id, nome, quantita, unita_simbolo)| {
                Some(VoceGenerata {
                    alimento_id,
                    prodotto_id,
                    nome,
                    quantita: quantita?,
                    unita_simbolo: unita_simbolo?,
                })
            },
        )
        .collect())
}

/// Voci `generato` non ancora comprate di questa lista -- servono a
/// `serve_aggiornamento` per confrontare cosa c'è già in lista con cosa
/// produrrebbe un refresh, senza doverlo eseguire per davvero.
async fn voci_generate_non_comprate(
    pool: &SqlitePool,
    lista_id: i64,
) -> anyhow::Result<Vec<VoceGenerata>> {
    let righe: Vec<RigaVoceGenerataGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione, quantita, unita_simbolo \
         FROM liste_spesa_voci \
         WHERE lista_id = ? AND origine = 'generato' AND comprato = 0",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci generate non comprate")?;

    Ok(righe
        .into_iter()
        .filter_map(
            |(alimento_id, prodotto_id, nome, quantita, unita_simbolo)| {
                Some(VoceGenerata {
                    alimento_id,
                    prodotto_id,
                    nome,
                    quantita: quantita?,
                    unita_simbolo: unita_simbolo?,
                })
            },
        )
        .collect())
}

/// Ordina un vettore di voci per un confronto stabile che non dipende
/// dall'ordine di produzione -- non serve un `Ord` su `VoceGenerata`
/// intero (la quantità è un `f64`), basta ordinare per identità e unità.
fn ordina_per_confronto(voci: &mut [VoceGenerata]) {
    voci.sort_by(|a, b| {
        (a.alimento_id, a.prodotto_id, &a.nome, &a.unita_simbolo).cmp(&(
            b.alimento_id,
            b.prodotto_id,
            &b.nome,
            &b.unita_simbolo,
        ))
    });
}

/// Riga grezza di `unita_misura`: simbolo, famiglia (testo o assente), e il
/// fattore base num/den quando la famiglia è presente.
type RigaUnitaMisuraGrezza = (String, Option<String>, Option<i64>, Option<i64>);

pub async fn carica_mappa_unita(pool: &SqlitePool) -> anyhow::Result<HashMap<String, InfoUnita>> {
    let righe: Vec<RigaUnitaMisuraGrezza> = sqlx::query_as(
        "SELECT simbolo, famiglia_conversione, fattore_base_num, fattore_base_den \
         FROM unita_misura",
    )
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le unità di misura")?;

    Ok(righe
        .into_iter()
        .filter_map(|(simbolo, famiglia, num, den)| {
            let famiglia = famiglia.as_deref().and_then(FamigliaConversione::from_db);
            let (fattore_num, fattore_den) = match (famiglia, num, den) {
                (Some(_), Some(n), Some(d)) if d != 0 => (n as f64, d as f64),
                (Some(_), _, _) => return None,
                (None, _, _) => (1.0, 1.0),
            };
            Some((
                simbolo,
                InfoUnita {
                    famiglia,
                    fattore_num,
                    fattore_den,
                },
            ))
        })
        .collect())
}

/// Il fresco dell'aggregazione (planner + aggiunte dal catalogo), *prima*
/// di sottrarre quanto già comprato -- condiviso da `calcola_fresche` (che
/// sottrae) e da `eccessi_comprati` (che confronta il fabbisogno grezzo con
/// quanto è già segnato comprato, per trovare un eccesso).
async fn fresche_grezze(
    pool: &SqlitePool,
    lista: &ListaSpesa,
) -> anyhow::Result<Vec<VoceGenerata>> {
    let mut righe = righe_da_aggregare(pool, lista).await?;
    // Le aggiunte dal catalogo non sono uno snapshot: partecipano di nuovo
    // ogni volta al calcolo del fresco, insieme alle righe del planner --
    // un alimento generico si somma davvero, un prodotto specifico resta
    // nel suo bucket di identità separato (`identita_riga`).
    righe.extend(righe_da_aggiunte_catalogo(pool, lista.id).await?);
    let mappa_unita = carica_mappa_unita(pool).await?;
    Ok(aggrega_ingredienti(&righe, |simbolo| {
        mappa_unita.get(simbolo).copied()
    }))
}

/// Le scorte dello spazio della lista, nell'unità-base, pronte per
/// `sottrai_scorte`.
async fn scorte_disponibili(
    pool: &SqlitePool,
    lista: &ListaSpesa,
    mappa_unita: &HashMap<String, InfoUnita>,
) -> anyhow::Result<Vec<ScortaDisponibile>> {
    let scorte = crate::modules::dispensa::scorte_per_netto(pool, lista.spazio_id).await?;
    Ok(scorte
        .into_iter()
        .map(|scorta| {
            let (quantita, unita_simbolo) = converti_in_base(
                scorta.quantita,
                &scorta.unita_simbolo,
                mappa_unita.get(&scorta.unita_simbolo).copied(),
            );
            ScortaDisponibile {
                alimento_id: scorta.alimento_id,
                prodotto_id: scorta.prodotto_alimentare_id,
                nome: scorta.descrizione,
                quantita,
                unita_simbolo,
            }
        })
        .collect())
}

/// Il fabbisogno al netto di quello che c'è in casa, **prima** di togliere
/// quanto è già segnato comprato in lista.
///
/// L'ordine conta: una voce comprata e non ancora chiusa non è ancora in
/// casa, quindi le due sottrazioni non si sovrappongono; chiudendo la spesa
/// la voce comprata esce dalla lista ed entra nelle scorte, e il totale
/// sottratto resta lo stesso. È questo che impedisce alla chiusura di far
/// ricomparire la roba appena comprata (trovato da Alessio collaudando il
/// 16 settembre 2026).
async fn fabbisogno_al_netto_delle_scorte(
    pool: &SqlitePool,
    lista: &ListaSpesa,
) -> anyhow::Result<Vec<VoceGenerata>> {
    let grezze = fresche_grezze(pool, lista).await?;
    // Con le Scorte spente non si sottrae niente: quei numeri sarebbero
    // fermi all'ultima volta che qualcuno li ha aggiornati, e la lista
    // arriverebbe dimezzata senza un motivo visibile
    // (Alessio, 24 settembre 2026).
    if !crate::modules::impostazioni::funzioni(pool)
        .await
        .attiva(crate::modules::impostazioni::Funzione::Scorte)
    {
        return Ok(grezze);
    }
    let mappa_unita = carica_mappa_unita(pool).await?;
    let scorte = scorte_disponibili(pool, lista, &mappa_unita).await?;
    Ok(sottrai_scorte(grezze, &scorte))
}

/// Calcola il fresco dell'aggregazione, meno quello che c'è in casa e meno
/// quanto già coperto da voci comprate, senza scrivere nulla -- condiviso da
/// `aggiorna_lista` (che lo scrive per davvero) e da `serve_aggiornamento`
/// (che lo confronta soltanto con quanto già in lista, per decidere se
/// mostrare "🔄 Aggiorna lista").
async fn calcola_fresche(
    pool: &SqlitePool,
    lista: &ListaSpesa,
) -> anyhow::Result<Vec<VoceGenerata>> {
    let fresche = fabbisogno_al_netto_delle_scorte(pool, lista).await?;
    // Le voci già comprate restano intoccate (mai cancellate da chi scrive
    // questo risultato): si sottrae quello che coprono già dal fresco, così
    // non si duplica mai una quantità già segnata come acquistata.
    let gia_comprato = voci_generate_comprate(pool, lista.id).await?;
    Ok(sottrai_gia_comprato(fresche, &gia_comprato))
}

/// Una voce già segnata comprata la cui quantità supera ormai il fabbisogno
/// reale (un pasto rimosso dal planner, una ricetta ridotta...): non viene
/// mai corretta da sola (una voce comprata resta sempre congelata), ma
/// viene segnalata così l'utente sa di aver già preso più del necessario.
#[derive(Debug, Clone, PartialEq)]
pub struct Eccesso {
    pub nome: String,
    pub unita_simbolo: String,
    pub quantita_eccesso: f64,
}

/// Confronta il fabbisogno grezzo (prima di sottrarre il comprato) con
/// quanto è già segnato comprato: se per un'identità il comprato supera il
/// fabbisogno reale, quella è la quantità in eccesso. Dominio puro,
/// testabile senza database.
pub fn calcola_eccessi(
    fresche_grezze: &[VoceGenerata],
    gia_comprato: &[VoceGenerata],
) -> Vec<Eccesso> {
    gia_comprato
        .iter()
        .filter_map(|comprata| {
            let identita = identita_voce(comprata);
            let richiesto: f64 = fresche_grezze
                .iter()
                .filter(|voce| {
                    identita_voce(voce) == identita && voce.unita_simbolo == comprata.unita_simbolo
                })
                .map(|voce| voce.quantita)
                .sum();
            let eccesso = arrotonda(comprata.quantita - richiesto);
            (eccesso > 0.0).then_some(Eccesso {
                nome: comprata.nome.clone(),
                unita_simbolo: comprata.unita_simbolo.clone(),
                quantita_eccesso: eccesso,
            })
        })
        .collect()
}

/// Gli eccessi di questa lista, calcolati sullo stato reale del database --
/// vedi `calcola_eccessi` per la logica.
pub async fn eccessi_comprati(
    pool: &SqlitePool,
    lista: &ListaSpesa,
) -> anyhow::Result<Vec<Eccesso>> {
    // L'eccesso si misura su quello che serve davvero, cioè al netto di
    // quello che c'è già in casa.
    let fresche = fabbisogno_al_netto_delle_scorte(pool, lista).await?;
    let gia_comprato = voci_generate_comprate(pool, lista.id).await?;
    Ok(calcola_eccessi(&fresche, &gia_comprato))
}

/// Vero se un "🔄 Aggiorna lista" cambierebbe davvero il risultato --
/// confronta il fresco dell'aggregazione con le voci generate non ancora
/// comprate già in lista, senza eseguire il refresh. Deciso con Alessio
/// dopo un collaudo dal vivo in cui il bottone compariva sempre, anche a
/// lista già aggiornata (stesso principio di "🔄 Aggiorna planner").
pub async fn serve_aggiornamento(pool: &SqlitePool, lista: &ListaSpesa) -> anyhow::Result<bool> {
    let mut fresche = calcola_fresche(pool, lista).await?;
    let mut attuali = voci_generate_non_comprate(pool, lista.id).await?;
    ordina_per_confronto(&mut fresche);
    ordina_per_confronto(&mut attuali);
    Ok(fresche != attuali)
}

/// Aggiornamento esplicito (mai automatico): ricalcola SOLO le voci
/// `origine = 'generato' AND comprato = 0` -- le cancella e re-inserisce da
/// zero il risultato fresco dell'aggregazione. Le voci comprate (generate o
/// manuali) e le voci manuali non comprate restano congelate esattamente
/// come sono. Ritorna il numero di voci generate dopo il refresh.
pub async fn aggiorna_lista(pool: &SqlitePool, lista: &ListaSpesa) -> anyhow::Result<usize> {
    let voci = calcola_fresche(pool, lista).await?;

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    // L'ordine (deciso dall'utente con "↕️ Riordina lista") non deve mai
    // cambiare per effetto di un refresh -- deciso con Alessio dopo averlo
    // visto dal vivo: deselezionare una voce (che richiama `aggiorna_lista`
    // da sola, vedi `toggle_comprato`) rimescolava tutte le voci generate
    // non comprate. Si rilegge l'ordinamento di ciascuna PRIMA di
    // cancellare, e lo si riassegna alla stessa identità nel fresco appena
    // calcolato; solo un'identità davvero nuova (mai vista prima nelle
    // voci generate non comprate) prende un ordinamento nuovo, in coda.
    let precedenti: Vec<RigaOrdinePrecedenteGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione, unita_simbolo, ordinamento \
         FROM liste_spesa_voci WHERE lista_id = ? AND origine = 'generato' AND comprato = 0",
    )
    .bind(lista.id)
    .fetch_all(&mut *tx)
    .await
    .context("Impossibile rileggere l'ordine precedente")?;
    sqlx::query(
        "DELETE FROM liste_spesa_voci \
         WHERE lista_id = ? AND origine = 'generato' AND comprato = 0",
    )
    .bind(lista.id)
    .execute(&mut *tx)
    .await
    .context("Impossibile ripulire le voci generate")?;
    // Le voci davvero nuove vanno in coda all'ordine esistente (voci
    // comprate o manuali, che il refresh non tocca) -- non si inseriscono
    // in mezzo a un ordine che l'utente ha già sistemato a mano.
    let mut prossimo_ordinamento: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(ordinamento), 0) FROM liste_spesa_voci WHERE lista_id = ?",
    )
    .bind(lista.id)
    .fetch_one(&mut *tx)
    .await
    .context("Impossibile leggere l'ordinamento massimo")?;
    for voce in &voci {
        let ordinamento = precedenti
            .iter()
            .find(|(alimento_id, prodotto_id, nome, unita_simbolo, _)| {
                unita_simbolo.as_deref() == Some(voce.unita_simbolo.as_str())
                    && identita_voce(&VoceGenerata {
                        alimento_id: *alimento_id,
                        prodotto_id: *prodotto_id,
                        nome: nome.clone(),
                        quantita: 0.0,
                        unita_simbolo: voce.unita_simbolo.clone(),
                    }) == identita_voce(voce)
            })
            .map(|(_, _, _, _, ordinamento)| *ordinamento)
            .unwrap_or_else(|| {
                prossimo_ordinamento += 1;
                prossimo_ordinamento
            });
        sqlx::query(
            "INSERT INTO liste_spesa_voci \
             (lista_id, origine, alimento_id, prodotto_alimentare_id, descrizione, \
              quantita, unita_simbolo, ordinamento) \
             VALUES (?, 'generato', ?, ?, ?, ?, ?, ?)",
        )
        .bind(lista.id)
        .bind(voce.alimento_id)
        .bind(voce.prodotto_id)
        .bind(&voce.nome)
        .bind(voce.quantita)
        .bind(&voce.unita_simbolo)
        .bind(ordinamento)
        .execute(&mut *tx)
        .await
        .context("Impossibile inserire una voce generata")?;
    }
    // Rinumera tutte le voci mantenendo l'ordine che si vede (trovato dal
    // collaudo del 16 settembre 2026): riusare il numero di una voce
    // rigenerata mentre le nuove si contano solo da quelle rimaste poteva
    // dare lo stesso numero a due voci, e scambiare due posizioni uguali non
    // sposta niente — le frecce di "↕️ Riordina lista" sembravano morte.
    let ordine: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM liste_spesa_voci WHERE lista_id = ? ORDER BY ordinamento, id",
    )
    .bind(lista.id)
    .fetch_all(&mut *tx)
    .await
    .context("Impossibile rileggere l'ordine della lista")?;
    for (posizione, voce_id) in ordine.iter().enumerate() {
        sqlx::query("UPDATE liste_spesa_voci SET ordinamento = ? WHERE id = ?")
            .bind(posizione as i64 + 1)
            .bind(voce_id)
            .execute(&mut *tx)
            .await
            .context("Impossibile rinumerare la lista")?;
    }
    sqlx::query(
        "UPDATE liste_spesa SET \
         aggiornata_il = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(lista.id)
    .execute(&mut *tx)
    .await
    .context("Impossibile aggiornare il timestamp della lista")?;
    tx.commit()
        .await
        .context("Impossibile salvare l'aggiornamento della lista")?;
    Ok(voci.len())
}

/// Il totale per alimento di tutte le voci generate della lista, comprate e
/// non: la base del confronto di `confronta_totali`.
async fn totali_generati(pool: &SqlitePool, lista_id: i64) -> anyhow::Result<Vec<VoceGenerata>> {
    let righe: Vec<RigaVoceGenerataGrezza> = sqlx::query_as(
        "SELECT alimento_id, prodotto_alimentare_id, descrizione, quantita, unita_simbolo \
         FROM liste_spesa_voci WHERE lista_id = ? AND origine = 'generato' \
         ORDER BY ordinamento, id",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci generate")?;
    let mut totali: Vec<VoceGenerata> = Vec::new();
    for (alimento_id, prodotto_id, nome, quantita, unita_simbolo) in righe {
        let (Some(quantita), Some(unita_simbolo)) = (quantita, unita_simbolo) else {
            continue;
        };
        let voce = VoceGenerata {
            alimento_id,
            prodotto_id,
            nome,
            quantita,
            unita_simbolo,
        };
        match totali.iter_mut().find(|t| {
            identita_voce(t) == identita_voce(&voce) && t.unita_simbolo == voce.unita_simbolo
        }) {
            Some(totale) => totale.quantita = arrotonda(totale.quantita + voce.quantita),
            None => totali.push(voce),
        }
    }
    Ok(totali)
}

/// Aggiorna la lista e registra cosa è cambiato (chiesto da Alessio il 16
/// settembre 2026: l'aggiornamento automatico può aggiungere, togliere,
/// alzare e abbassare, ma deve **sempre** dire cosa ha fatto). Registra solo
/// se qualcosa è cambiato davvero. Ritorna le modifiche.
pub async fn aggiorna_e_registra(
    pool: &SqlitePool,
    lista: &ListaSpesa,
    automatico: bool,
) -> anyhow::Result<Vec<Modifica>> {
    let prima = totali_generati(pool, lista.id).await?;
    aggiorna_lista(pool, lista).await?;
    let dopo = totali_generati(pool, lista.id).await?;

    // Il motivo di una voce calata o sparita: se il fabbisogno grezzo la
    // chiede ancora (più di quanto resta), a coprirla sono le scorte.
    let grezze = fresche_grezze(pool, lista).await?;
    let modifiche = confronta_totali(&prima, &dopo, |voce| {
        let richiesta: f64 = grezze
            .iter()
            .filter(|g| {
                identita_voce(g) == identita_voce(voce) && g.unita_simbolo == voce.unita_simbolo
            })
            .map(|g| g.quantita)
            .sum();
        let rimasta = dopo
            .iter()
            .find(|d| {
                identita_voce(d) == identita_voce(voce) && d.unita_simbolo == voce.unita_simbolo
            })
            .map(|d| d.quantita)
            .unwrap_or(0.0);
        arrotonda(richiesta - rimasta) > 0.0
    });
    if modifiche.is_empty() {
        return Ok(modifiche);
    }

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    let aggiornamento_id = sqlx::query(
        "INSERT INTO liste_spesa_aggiornamenti (lista_id, utente_id, automatico) VALUES (?, ?, ?)",
    )
    .bind(lista.id)
    .bind(crate::identity::current_actor().utente_id)
    .bind(i64::from(automatico))
    .execute(&mut *tx)
    .await
    .context("Impossibile registrare l'aggiornamento")?
    .last_insert_rowid();
    for modifica in &modifiche {
        sqlx::query(
            "INSERT INTO liste_spesa_modifiche \
             (aggiornamento_id, tipo, descrizione, unita_simbolo, quantita_prima, \
              quantita_dopo, motivo) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(aggiornamento_id)
        .bind(modifica.tipo.token())
        .bind(&modifica.descrizione)
        .bind(&modifica.unita_simbolo)
        .bind(modifica.quantita_prima)
        .bind(modifica.quantita_dopo)
        .bind(&modifica.motivo)
        .execute(&mut *tx)
        .await
        .context("Impossibile registrare una modifica")?;
    }
    tx.commit()
        .await
        .context("Impossibile salvare il resoconto")?;
    Ok(modifiche)
}

/// Se la lista si aggiorna da sola all'apertura. Acceso di default, per
/// persona (deciso con Alessio).
pub async fn aggiornamento_automatico(pool: &SqlitePool) -> bool {
    let Some(utente_id) = crate::identity::current_actor().utente_id else {
        return false;
    };
    sqlx::query_scalar::<_, i64>(
        "SELECT lista_spesa_aggiornamento_automatico FROM preferenze_utente WHERE utente_id = ?",
    )
    .bind(utente_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|valore| valore != 0)
    .unwrap_or(true)
}

pub async fn imposta_aggiornamento_automatico(
    pool: &SqlitePool,
    attivo: bool,
) -> anyhow::Result<()> {
    let utente_id = crate::identity::current_actor()
        .utente_id
        .context("Utente non disponibile")?;
    sqlx::query(
        "UPDATE preferenze_utente SET lista_spesa_aggiornamento_automatico = ? \
         WHERE utente_id = ?",
    )
    .bind(i64::from(attivo))
    .bind(utente_id)
    .execute(pool)
    .await
    .context("Impossibile salvare la preferenza")?;
    Ok(())
}

/// L'ultimo aggiornamento che ha cambiato qualcosa: id, se era automatico,
/// e quando (ora locale già pronta da mostrare).
pub async fn ultimo_aggiornamento(
    pool: &SqlitePool,
    lista_id: i64,
) -> anyhow::Result<Option<(i64, bool, String)>> {
    let riga: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, automatico, avvenuto_il_locale FROM liste_spesa_aggiornamenti \
         WHERE lista_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(lista_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'ultimo aggiornamento")?;
    Ok(riga.map(|(id, automatico, quando)| (id, automatico != 0, quando)))
}

/// Le modifiche registrate da un aggiornamento, nell'ordine in cui sono
/// state trovate.
/// Riga grezza di `liste_spesa_modifiche`: tipo, descrizione, unità,
/// quantità prima e dopo, motivo.
type RigaModificaGrezza = (
    String,
    String,
    String,
    Option<f64>,
    Option<f64>,
    Option<String>,
);

pub async fn modifiche_di(
    pool: &SqlitePool,
    aggiornamento_id: i64,
) -> anyhow::Result<Vec<Modifica>> {
    let righe: Vec<RigaModificaGrezza> = sqlx::query_as(
        "SELECT tipo, descrizione, unita_simbolo, quantita_prima, quantita_dopo, motivo \
             FROM liste_spesa_modifiche WHERE aggiornamento_id = ? ORDER BY id",
    )
    .bind(aggiornamento_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le modifiche")?;
    Ok(righe
        .into_iter()
        .filter_map(
            |(tipo, descrizione, unita_simbolo, quantita_prima, quantita_dopo, motivo)| {
                Some(Modifica {
                    tipo: TipoModifica::da_token(&tipo)?,
                    descrizione,
                    unita_simbolo,
                    quantita_prima,
                    quantita_dopo,
                    motivo,
                })
            },
        )
        .collect())
}

/// Una spesa chiusa, per la schermata di sola lettura "🗄 Ultima spesa
/// chiusa".
#[derive(Debug, Clone, FromRow)]
pub struct ChiusuraSpesa {
    pub id: i64,
    pub data_inizio: String,
    pub data_fine: String,
    pub voci_totali: i64,
    pub chiusa_il: String,
}

/// Una voce archiviata da una chiusura: solo quello che serve a rileggerla,
/// non si modifica più (trigger `trg_lista_spesa_voce_archiviata_immutabile`).
#[derive(Debug, Clone, FromRow)]
pub struct VoceArchiviata {
    pub descrizione: String,
    pub quantita: Option<f64>,
    pub unita_simbolo: Option<String>,
}

/// Riga grezza di una voce comprata, letta prima di archiviarla.
#[derive(Debug, Clone, FromRow)]
struct VoceComprata {
    origine: String,
    alimento_id: Option<i64>,
    prodotto_alimentare_id: Option<i64>,
    descrizione: String,
    quantita: Option<f64>,
    unita_simbolo: Option<String>,
    comprato_il: Option<String>,
    quantita_presa: Option<f64>,
    unita_presa: Option<String>,
    prodotto_preso_id: Option<i64>,
    prezzo_centesimi: Option<i64>,
}

/// Quante voci sono segnate comprate: decide se "🧾 Chiudi la spesa" ha
/// senso di comparire, senza caricare tutte le voci per contarle.
pub async fn conta_comprate(pool: &SqlitePool, lista_id: i64) -> anyhow::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM liste_spesa_voci WHERE lista_id = ? AND comprato = 1")
        .bind(lista_id)
        .fetch_one(pool)
        .await
        .context("Impossibile contare le voci comprate")
}

/// Le aggiunte dal catalogo di questa lista, ciascuna già convertita
/// nell'unità di aggregazione (kg → g, l → ml) riusando `aggrega_ingredienti`
/// su una riga sola: senza la conversione un'aggiunta scritta in kg non
/// combacerebbe mai con la voce comprata in g che ne è nata.
/// Riga grezza di `liste_spesa_aggiunte_catalogo` con il proprio id: come
/// `RigaAggiuntaCatalogoGrezza`, ma l'id serve a sapere quale riga togliere
/// chiudendo la spesa.
type RigaAggiuntaConIdGrezza = (i64, Option<i64>, Option<i64>, String, f64, String);

async fn aggiunte_convertite(
    pool: &SqlitePool,
    lista_id: i64,
    mappa_unita: &HashMap<String, InfoUnita>,
) -> anyhow::Result<Vec<AggiuntaCatalogo>> {
    let righe: Vec<RigaAggiuntaConIdGrezza> = sqlx::query_as(
        "SELECT id, alimento_id, prodotto_alimentare_id, descrizione_snapshot, \
                quantita, unita_simbolo \
         FROM liste_spesa_aggiunte_catalogo WHERE lista_id = ?",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le aggiunte dal catalogo")?;

    Ok(righe
        .into_iter()
        .map(
            |(id, alimento_id, prodotto_id, nome, quantita, unita_simbolo)| {
                let originale = RigaIngrediente {
                    alimento_id,
                    prodotto_id,
                    nome,
                    unita_simbolo,
                    quantita,
                };
                let riga = aggrega_ingredienti(std::slice::from_ref(&originale), |simbolo| {
                    mappa_unita.get(simbolo).copied()
                })
                .into_iter()
                .next()
                .map(|voce| RigaIngrediente {
                    alimento_id: voce.alimento_id,
                    prodotto_id: voce.prodotto_id,
                    nome: voce.nome,
                    unita_simbolo: voce.unita_simbolo,
                    quantita: voce.quantita,
                })
                .unwrap_or(originale);
                AggiuntaCatalogo { id, riga }
            },
        )
        .collect())
}

/// Di quello che ogni aggiunta dal catalogo chiedeva, quanto ne manca
/// ancora **adesso**: la stessa sottrazione che fa la lista
/// (`sottrai_scorte`), sulle scorte di questo momento.
///
/// È il numero che l'utente legge in lista, e quindi l'unico che il bot puo'
/// dire senza contraddirsi. La prima versione faceva un conto suo -- chiesto
/// meno comprato, senza guardare la dispensa -- e usciva con "restano 590 g"
/// mentre la lista ne chiedeva 10, e con la domanda che compariva anche
/// quando non mancava niente (Alessio, collaudo del 24 settembre 2026, punti
/// C1, C3 e C5).
///
/// Ritorna `(id dell'aggiunta, quanto manca, unita')` nell'unita' con cui
/// l'aggiunta e' scritta -- "0,5 kg", non "500 g" -- e salta quelle che non
/// mancano piu'.
async fn residui_delle_aggiunte(
    pool: &SqlitePool,
    lista: &ListaSpesa,
    mappa_unita: &HashMap<String, InfoUnita>,
) -> anyhow::Result<Vec<(i64, f64, String)>> {
    let grezze: Vec<RigaAggiuntaConIdGrezza> = sqlx::query_as(
        "SELECT id, alimento_id, prodotto_alimentare_id, descrizione_snapshot, \
                quantita, unita_simbolo \
         FROM liste_spesa_aggiunte_catalogo WHERE lista_id = ? ORDER BY id",
    )
    .bind(lista.id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le aggiunte dal catalogo")?;

    // Ogni aggiunta diventa una richiesta come le altre, nell'unita' di
    // aggregazione: e' `sottrai_scorte` che sa dare la precedenza ai prodotti
    // specifici sulle scorte generiche, e rifarlo qui a mano vorrebbe dire
    // due regole diverse per la stessa cosa.
    let mut richieste = Vec::new();
    let mut origine = Vec::new();
    for (id, alimento_id, prodotto_id, nome, quantita, unita_simbolo) in grezze {
        let (in_base, unita_base) = converti_in_base(
            quantita,
            &unita_simbolo,
            mappa_unita.get(&unita_simbolo).copied(),
        );
        richieste.push(VoceGenerata {
            alimento_id,
            prodotto_id,
            nome,
            quantita: in_base,
            unita_simbolo: unita_base,
        });
        origine.push((id, quantita, unita_simbolo, in_base));
    }

    let scorte = scorte_disponibili(pool, lista, mappa_unita).await?;
    let rimaste = sottrai_scorte(richieste.clone(), &scorte);

    // `sottrai_scorte` tiene l'ordine e toglie solo le righe azzerate: si
    // cammina sulle due liste in parallelo.
    let mut residui = Vec::new();
    let mut prossima = rimaste.into_iter().peekable();
    for (richiesta, (id, quantita_scritta, unita_scritta, in_base)) in richieste.iter().zip(origine)
    {
        let manca = match prossima.peek() {
            Some(rimasta)
                if identita_voce(rimasta) == identita_voce(richiesta)
                    && rimasta.unita_simbolo == richiesta.unita_simbolo =>
            {
                let manca = rimasta.quantita;
                prossima.next();
                manca
            }
            _ => 0.0,
        };
        if manca <= TOLLERANZA_QUANTITA {
            continue;
        }
        let residuo = if in_base > 0.0 {
            quantita_scritta * manca / in_base
        } else {
            quantita_scritta
        };
        residui.push((id, residuo, unita_scritta));
    }
    Ok(residui)
}

/// Che fine fanno le aggiunte dal catalogo quando la spesa si chiude.
///
/// Quelle che non mancano piu' si chiudono in silenzio: la richiesta e'
/// servita. Quelle che mancano ancora restano **com'erano** -- la quantita'
/// non si tocca, ci pensa la lista a mostrarne il netto -- e vengono marcate
/// con questa chiusura, cosi' "🗑 No, toglile" toglie esattamente quelle.
///
/// Senza le Scorte accese non c'e' nessuna dispensa da guardare: allora vale
/// la regola vecchia, cioe' aver comprato qualcosa di quell'identita' chiude
/// la richiesta. Altrimenti nessuna aggiunta si chiuderebbe mai, e
/// tornerebbero le voci fantasma del collaudo del 23 settembre.
async fn sistema_le_aggiunte(
    pool: &SqlitePool,
    lista: &ListaSpesa,
    chiusura_id: i64,
    comprate_generate: &[VoceGenerata],
) -> anyhow::Result<Vec<AggiuntaRidotta>> {
    let mappa_unita = carica_mappa_unita(pool).await?;
    let scorte_accese = crate::modules::impostazioni::funzioni(pool)
        .await
        .attiva(crate::modules::impostazioni::Funzione::Scorte);

    let da_chiudere: Vec<i64>;
    let mut ridotte = Vec::new();
    if scorte_accese {
        let residui = residui_delle_aggiunte(pool, lista, &mappa_unita).await?;
        let ancora_vive: Vec<i64> = residui.iter().map(|(id, _, _)| *id).collect();
        let tutte: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, descrizione_snapshot FROM liste_spesa_aggiunte_catalogo \
             WHERE lista_id = ? ORDER BY id",
        )
        .bind(lista.id)
        .fetch_all(pool)
        .await
        .context("Impossibile leggere le aggiunte dal catalogo")?;
        da_chiudere = tutte
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !ancora_vive.contains(id))
            .collect();
        for (id, quanto, unita) in residui {
            let nome = tutte
                .iter()
                .find(|(altro, _)| *altro == id)
                .map(|(_, nome)| nome.clone())
                .unwrap_or_default();
            sqlx::query(
                "UPDATE liste_spesa_aggiunte_catalogo SET ridotta_chiusura_id = ? WHERE id = ?",
            )
            .bind(chiusura_id)
            .bind(id)
            .execute(pool)
            .await
            .context("Impossibile segnare un'aggiunta rimasta a meta'")?;
            ridotte.push(AggiuntaRidotta {
                nome,
                quantita: quanto,
                unita_simbolo: unita,
            });
        }
    } else {
        let aggiunte = aggiunte_convertite(pool, lista.id, &mappa_unita).await?;
        let esito = aggiunte_coperte_dalla_spesa(&aggiunte, comprate_generate);
        da_chiudere = esito
            .chiuse
            .into_iter()
            .chain(esito.ridotte.into_iter().map(|(id, _)| id))
            .collect();
    }

    for id in da_chiudere {
        sqlx::query("DELETE FROM liste_spesa_aggiunte_catalogo WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .context("Impossibile togliere un'aggiunta dal catalogo gia' comprata")?;
    }
    Ok(ridotte)
}

/// Cosa ha prodotto una chiusura: quante voci sono finite nell'archivio e
/// quante sono entrate in casa, per luogo (vuoto se l'ingresso automatico è
/// spento o se nessuna voce era collegata al catalogo).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EsitoChiusura {
    pub archiviate: usize,
    pub entrate: Vec<(crate::modules::dispensa::Conservazione, usize)>,
    /// La chiusura appena creata: serve a chiedere il totale dello
    /// scontrino, che è facoltativo (consegna B).
    pub chiusura_id: i64,
    /// La somma dei prezzi segnati voce per voce, se ce n'era almeno uno.
    pub speso_centesimi: Option<i64>,
    /// Le aggiunte dal catalogo di cui si e' comprato meno di quanto
    /// chiedevano: restano in lista con quel che manca, e il bot chiede se
    /// lasciarle (scelta di Alessio, 24 settembre 2026).
    pub ridotte: Vec<AggiuntaRidotta>,
}

/// Un'aggiunta dal catalogo rimasta a meta' dopo la chiusura, come la legge
/// chi deve decidere se tenerla.
#[derive(Debug, Clone, PartialEq)]
pub struct AggiuntaRidotta {
    pub nome: String,
    pub quantita: f64,
    pub unita_simbolo: String,
}

/// Chiude la spesa: le voci comprate (generate o manuali) escono dalla lista
/// attiva e finiscono nell'archivio, insieme alle aggiunte dal catalogo che
/// quelle voci hanno ormai coperto (`aggiunte_coperte_dalla_spesa`).
///
/// Chiesto da Alessio il 16 settembre 2026: mancava il momento "spesa
/// fatta". La lista è una sola e le voci comprate ci restavano per sempre,
/// quindi il giro dopo ripartiva sporco, e "comprato" finiva per voler dire
/// due cose diverse (l'ho preso adesso / l'avevo preso la settimana scorsa).
///
/// Le voci **non** vengono cancellate: l'archivio è la memoria della spesa,
/// e da lì la merce entra in casa (`dispensa::ingresso_da_chiusura`), ognuna
/// nel suo posto. Le voci non comprate restano in lista come sono.
///
/// `archiviate = 0` significa che non c'era niente di comprato e che non è
/// stata creata nessuna chiusura.
pub async fn chiudi_spesa(pool: &SqlitePool, lista: &ListaSpesa) -> anyhow::Result<EsitoChiusura> {
    let comprate: Vec<VoceComprata> = sqlx::query_as(
        "SELECT origine, alimento_id, prodotto_alimentare_id, descrizione, quantita, \
                unita_simbolo, comprato_il, quantita_presa, unita_presa, prodotto_preso_id, \
                prezzo_centesimi \
         FROM liste_spesa_voci WHERE lista_id = ? AND comprato = 1 \
         ORDER BY ordinamento ASC, id ASC",
    )
    .bind(lista.id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci comprate")?;

    if comprate.is_empty() {
        return Ok(EsitoChiusura::default());
    }

    // Solo le voci generate contano per coprire un'aggiunta dal catalogo:
    // una voce manuale libera non è collegata a nessuna identità del
    // catalogo, quindi non può coprire niente.
    // Per coprire le aggiunte conta quanto serviva (la quantità in lista),
    // non quanto si è preso: l'eccedenza di una confezione più grande entra
    // in casa e lì viene sottratta dal fabbisogno.
    let comprate_generate: Vec<VoceGenerata> = comprate
        .iter()
        .filter(|voce| voce.origine == "generato")
        .filter_map(|voce| {
            Some(VoceGenerata {
                alimento_id: voce.alimento_id,
                prodotto_id: voce.prodotto_alimentare_id,
                nome: voce.descrizione.clone(),
                quantita: voce.quantita?,
                unita_simbolo: voce.unita_simbolo.clone()?,
            })
        })
        .collect();
    let utente_id = crate::identity::current_actor().utente_id;

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    let chiusura_id = sqlx::query(
        "INSERT INTO liste_spesa_chiusure \
         (lista_id, chiusa_da_utente_id, data_inizio, data_fine, voci_totali, negozio_id) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(lista.id)
    .bind(utente_id)
    .bind(&lista.data_inizio)
    .bind(&lista.data_fine)
    .bind(comprate.len() as i64)
    .bind(lista.negozio_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile registrare la chiusura della spesa")?
    .last_insert_rowid();

    for voce in &comprate {
        // Se si è segnato cosa si è preso davvero ("📦"), in archivio -- e
        // quindi in casa -- va quello, con la quantità che serviva accanto.
        let (quantita, unita, richiesta) = match (voce.quantita_presa, &voce.unita_presa) {
            (Some(presa), Some(unita)) => (Some(presa), Some(unita.clone()), voce.quantita),
            _ => (voce.quantita, voce.unita_simbolo.clone(), None),
        };
        sqlx::query(
            "INSERT INTO liste_spesa_voci_archiviate \
             (chiusura_id, origine, alimento_id, prodotto_alimentare_id, descrizione, \
              quantita, unita_simbolo, comprato_il, quantita_richiesta, prezzo_centesimi) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(chiusura_id)
        .bind(&voce.origine)
        .bind(voce.alimento_id)
        .bind(voce.prodotto_preso_id.or(voce.prodotto_alimentare_id))
        .bind(&voce.descrizione)
        .bind(quantita)
        .bind(unita)
        .bind(&voce.comprato_il)
        .bind(richiesta)
        .bind(voce.prezzo_centesimi)
        .execute(&mut *tx)
        .await
        .context("Impossibile archiviare una voce comprata")?;
    }

    sqlx::query("DELETE FROM liste_spesa_voci WHERE lista_id = ? AND comprato = 1")
        .bind(lista.id)
        .execute(&mut *tx)
        .await
        .context("Impossibile togliere le voci comprate dalla lista")?;

    tx.commit()
        .await
        .context("Impossibile salvare la chiusura della spesa")?;

    // La merce comprata entra in dispensa, se l'utente non l'ha disattivato
    // (`docs/previsto/dispensa.md`, decisione del 16 settembre 2026: acceso
    // di default). Fuori dalla transazione dell'archivio: un problema qui
    // non deve far perdere la chiusura, che è già valida da sola.
    let entrate = if crate::modules::dispensa::ingresso_automatico(pool).await {
        match crate::modules::dispensa::ingresso_da_chiusura(pool, chiusura_id).await {
            Ok(entrate) => entrate,
            Err(errore) => {
                tracing::warn!(?errore, chiusura_id, "Ingresso in casa fallito");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Le aggiunte dal catalogo si sistemano **dopo** che la merce e' entrata
    // in casa: quel che manca ancora si legge sulle scorte di adesso,
    // esattamente come fa la lista.
    let ridotte = match sistema_le_aggiunte(pool, lista, chiusura_id, &comprate_generate).await {
        Ok(ridotte) => ridotte,
        Err(errore) => {
            tracing::warn!(?errore, "Aggiunte dal catalogo non sistemate");
            Vec::new()
        }
    };

    let speso: Option<i64> = comprate
        .iter()
        .filter_map(|voce| voce.prezzo_centesimi)
        .sum::<i64>()
        .into();
    Ok(EsitoChiusura {
        archiviate: comprate.len(),
        entrate,
        chiusura_id,
        speso_centesimi: speso.filter(|totale| *totale > 0),
        ridotte,
    })
}

/// L'ultima spesa chiusa di questa lista, se ce n'è una.
pub async fn ultima_chiusura(
    pool: &SqlitePool,
    lista_id: i64,
) -> anyhow::Result<Option<ChiusuraSpesa>> {
    sqlx::query_as(
        "SELECT id, data_inizio, data_fine, voci_totali, chiusa_il \
         FROM liste_spesa_chiusure WHERE lista_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(lista_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile leggere l'ultima spesa chiusa")
}

/// Le voci archiviate da una chiusura, nell'ordine in cui erano in lista.
pub async fn voci_archiviate(
    pool: &SqlitePool,
    chiusura_id: i64,
) -> anyhow::Result<Vec<VoceArchiviata>> {
    sqlx::query_as(
        "SELECT descrizione, quantita, unita_simbolo \
         FROM liste_spesa_voci_archiviate WHERE chiusura_id = ? ORDER BY id ASC",
    )
    .bind(chiusura_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci archiviate")
}

/// Dal 17 settembre 2026 "🔄 Aggiorna lista" passa da `aggiorna_e_registra`,
/// che dice anche cosa è cambiato: questa scorciatoia resta solo per il test
/// che verifica la creazione della lista al primo aggiornamento.
#[cfg(test)]
async fn aggiorna_lista_attiva(pool: &SqlitePool) -> anyhow::Result<usize> {
    let lista = trova_o_crea_lista_attiva(pool).await?;
    aggiorna_lista(pool, &lista).await
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn actor(user_id: i64, space_id: i64, name: &str) -> crate::identity::AuditActor {
        crate::identity::AuditActor {
            utente_id: Some(user_id),
            nome_snapshot: name.to_string(),
            spazio_id: space_id,
            spazio_nome_snapshot: format!("Spazio {space_id}"),
            view_all: false,
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

    async fn add_membership(pool: &SqlitePool, space_id: i64, user_id: i64) {
        sqlx::query(
            "INSERT INTO membri_spazio (spazio_id, utente_id, ruolo) VALUES (?, ?, 'proprietario')",
        )
        .bind(space_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("membership");
    }

    async fn unita_id(pool: &SqlitePool, simbolo: &str) -> i64 {
        sqlx::query_scalar("SELECT id FROM unita_misura WHERE simbolo = ?")
            .bind(simbolo)
            .fetch_one(pool)
            .await
            .expect("unità di misura")
    }

    /// Alimento generico di catalogo globale, come "Pasta" -- visibile a
    /// chiunque, indipendentemente dallo spazio attivo.
    /// Riusa l'alimento se il nome è già nel catalogo base seminato dalle
    /// migration (es. "Pasta", "Farina") -- evita un conflitto con
    /// `idx_alimenti_globali_nome` invece di forzare nomi di fantasia nei
    /// test.
    async fn create_alimento_globale(pool: &SqlitePool, nome: &str) -> i64 {
        let normalizzato = nome.to_lowercase();
        if let Some(id) = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM alimenti WHERE nome_normalizzato = ? AND spazio_id IS NULL",
        )
        .bind(&normalizzato)
        .fetch_optional(pool)
        .await
        .expect("ricerca alimento esistente")
        {
            return id;
        }
        sqlx::query(
            "INSERT INTO alimenti (nome, nome_normalizzato, catalogo_globale) \
             VALUES (?, ?, 1)",
        )
        .bind(nome)
        .bind(&normalizzato)
        .execute(pool)
        .await
        .expect("alimento")
        .last_insert_rowid()
    }

    /// Prodotto commerciale specifico collegato a un alimento generico,
    /// come "Pasta De Cecco" collegato a "Pasta".
    async fn create_prodotto(
        pool: &SqlitePool,
        alimento_id: i64,
        marca: &str,
        nome_commerciale: &str,
    ) -> i64 {
        let unita = unita_id(pool, "g").await;
        sqlx::query(
            "INSERT INTO prodotti_alimentari \
             (alimento_id, marca, marca_normalizzata, nome_commerciale, \
              nome_commerciale_normalizzato, quantita_confezione, unita_confezione_id) \
             VALUES (?, ?, ?, ?, ?, 500, ?)",
        )
        .bind(alimento_id)
        .bind(marca)
        .bind(marca.to_lowercase())
        .bind(nome_commerciale)
        .bind(nome_commerciale.to_lowercase())
        .bind(unita)
        .execute(pool)
        .await
        .expect("prodotto")
        .last_insert_rowid()
    }

    async fn create_planner(
        pool: &SqlitePool,
        owner_id: i64,
        space_id: i64,
        inizio: &str,
        fine: &str,
    ) -> i64 {
        sqlx::query(
            "INSERT INTO planner_alimentari \
             (proprietario_utente_id, spazio_id, nome, nome_normalizzato, data_inizio, data_fine) \
             VALUES (?, ?, 'Settimana', 'settimana', ?, ?)",
        )
        .bind(owner_id)
        .bind(space_id)
        .bind(inizio)
        .bind(fine)
        .execute(pool)
        .await
        .expect("planner")
        .last_insert_rowid()
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_meal_with_ingredient(
        pool: &SqlitePool,
        planner_id: i64,
        data_pasto: &str,
        stato: &str,
        saltato_il: Option<&str>,
        completato_il: Option<&str>,
        alimento_id: Option<i64>,
        nome: &str,
        unita: &str,
        quantita_finale: Option<f64>,
    ) -> i64 {
        let pasto_id: i64 = sqlx::query(
            "INSERT INTO planner_pasti \
             (planner_id, data_pasto, tipo_pasto, ricetta_nome_snapshot, \
              ricetta_porzione_base_snapshot, stato, completato_il, saltato_il) \
             VALUES (?, ?, 'cena', 'Ricetta test', 4, ?, ?, ?)",
        )
        .bind(planner_id)
        .bind(data_pasto)
        .bind(stato)
        .bind(completato_il)
        .bind(saltato_il)
        .execute(pool)
        .await
        .expect("pasto")
        .last_insert_rowid();

        sqlx::query(
            "INSERT INTO planner_pasto_ingredienti_snapshot \
             (pasto_id, alimento_id, alimento_nome_snapshot, unita_simbolo_snapshot, \
              quantita_base_snapshot, quantita_scalata_snapshot, tipo_override_snapshot, \
              quantita_finale_snapshot) \
             VALUES (?, ?, ?, ?, 1, 1, ?, ?)",
        )
        .bind(pasto_id)
        .bind(alimento_id)
        .bind(nome)
        .bind(unita)
        .bind(if quantita_finale.is_some() {
            "nessuno"
        } else {
            "escluso"
        })
        .bind(quantita_finale)
        .execute(pool)
        .await
        .expect("snapshot ingrediente");
        pasto_id
    }

    #[tokio::test]
    async fn trova_o_crea_e_idempotente() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let prima = trova_o_crea_lista_attiva(&pool).await.expect("prima");
            let seconda = trova_o_crea_lista_attiva(&pool).await.expect("seconda");
            assert_eq!(prima.id, seconda.id);
        })
        .await;
    }

    #[tokio::test]
    async fn cambia_intervallo_rifiuta_fine_prima_di_inizio() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let esito = cambia_intervallo(&pool, lista.id, "2026-09-10", "2026-09-05", true).await;
            assert!(esito.is_err());
            let ok = cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true).await;
            assert!(ok.is_ok());
            let rilettura = trova_lista_attiva(&pool)
                .await
                .expect("rilettura")
                .expect("presente");
            assert_eq!(rilettura.data_inizio, "2026-09-01");
            assert_eq!(rilettura.data_fine, "2026-09-07");
        })
        .await;
    }

    #[tokio::test]
    async fn aggiungi_voce_manuale_senza_quantita_e_ammessa() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let id = aggiungi_voce_manuale(&pool, lista.id, "Detersivo piatti", None, None)
                .await
                .expect("voce manuale");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            let voce = voci.iter().find(|v| v.id == id).expect("presente");
            assert_eq!(voce.origine, "manuale");
            assert_eq!(voce.quantita, None);
        })
        .await;
    }

    #[tokio::test]
    async fn toggle_comprato_va_avanti_e_indietro() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let id = aggiungi_voce_manuale(&pool, lista.id, "Pane", None, None)
                .await
                .expect("voce");

            toggle_comprato(&pool, id).await.expect("toggle a comprato");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.iter().find(|v| v.id == id).unwrap().comprato, 1);

            toggle_comprato(&pool, id)
                .await
                .expect("toggle a non comprato");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.iter().find(|v| v.id == id).unwrap().comprato, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn voce_comprata_non_si_puo_modificare_solo_il_toggle_e_permesso() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let id = aggiungi_voce_manuale(&pool, lista.id, "Pane", None, None)
                .await
                .expect("voce");
            imposta_comprato(&pool, id, true).await.expect("comprato");

            let esito =
                sqlx::query("UPDATE liste_spesa_voci SET descrizione = 'Altro' WHERE id = ?")
                    .bind(id)
                    .execute(&pool)
                    .await;
            assert!(esito.is_err(), "il trigger deve bloccare la modifica");

            // Il toggle resta permesso.
            imposta_comprato(&pool, id, false)
                .await
                .expect("decomprato");
        })
        .await;
    }

    #[tokio::test]
    async fn aggiorna_lista_esclude_pasti_saltati_e_ingredienti_esclusi() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();

            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;

            // Contribuisce: pianificato, quantità finale non nulla.
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;
            // Non contribuisce: saltato.
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-03",
                "pianificato",
                Some("2026-09-03T00:00:00.000Z"),
                None,
                Some(101),
                "Zucchero",
                "g",
                Some(50.0),
            )
            .await;
            // Non contribuisce: ingrediente escluso (quantità finale nulla).
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-04",
                "pianificato",
                None,
                None,
                Some(102),
                "Uova",
                "pz",
                None,
            )
            .await;
            // Non contribuisce: fuori intervallo. Serve un secondo planner,
            // perché il primo copre solo 01-07 e il trigger del planner
            // rifiuta un pasto fuori dal proprio periodo.
            let planner_fuori_intervallo =
                create_planner(&pool, user_id, space_id, "2026-09-08", "2026-09-14").await;
            create_meal_with_ingredient(
                &pool,
                planner_fuori_intervallo,
                "2026-09-10",
                "pianificato",
                None,
                None,
                Some(103),
                "Latte",
                "ml",
                Some(500.0),
            )
            .await;

            let numero = aggiorna_lista(&pool, &lista).await.expect("refresh");
            assert_eq!(numero, 1);
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 1);
            assert_eq!(voci[0].descrizione, "Farina");
            assert_eq!(voci[0].quantita, Some(200.0));
        })
        .await;
    }

    #[tokio::test]
    async fn refresh_non_tocca_voci_comprate_ne_manuali_non_comprate() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;

            aggiorna_lista(&pool, &lista).await.expect("primo refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            let generata_id = voci[0].id;
            imposta_comprato(&pool, generata_id, true)
                .await
                .expect("segna comprata");
            let manuale_id = aggiungi_voce_manuale(&pool, lista.id, "Detersivo", None, None)
                .await
                .expect("manuale");

            // Un secondo pasto pianificato aggiunge altra farina: dopo il
            // refresh la voce comprata resta intoccata e ne compare una
            // nuova per la sola differenza, non un merge.
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-05",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(150.0),
            )
            .await;
            let numero = aggiorna_lista(&pool, &lista)
                .await
                .expect("secondo refresh");
            assert_eq!(numero, 1);

            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("voci dopo refresh");
            assert_eq!(voci.len(), 3);
            let comprata = voci.iter().find(|v| v.id == generata_id).unwrap();
            assert_eq!(comprata.quantita, Some(200.0));
            assert_eq!(comprata.comprato, 1);
            let manuale = voci.iter().find(|v| v.id == manuale_id).unwrap();
            assert_eq!(manuale.origine, "manuale");
            let nuova_generata = voci
                .iter()
                .find(|v| v.id != generata_id && v.id != manuale_id)
                .unwrap();
            assert_eq!(nuova_generata.origine, "generato");
            assert_eq!(nuova_generata.quantita, Some(150.0));
        })
        .await;
    }

    #[tokio::test]
    async fn deselezionare_una_voce_generata_la_rifonde_subito_con_la_residua() {
        // Scenario reale segnalato da Alessio: farina comprata (200 g),
        // un secondo pasto crea una nuova riga residua (150 g) allo stesso
        // refresh -- togliendo la spunta alla prima, le due righe devono
        // tornare a essere una sola (350 g), senza dover premere
        // "🔄 Aggiorna lista" a mano.
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;
            aggiorna_lista(&pool, &lista).await.expect("primo refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            let comprata_id = voci[0].id;
            imposta_comprato(&pool, comprata_id, true)
                .await
                .expect("segna comprata");

            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-05",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(150.0),
            )
            .await;
            aggiorna_lista(&pool, &lista)
                .await
                .expect("secondo refresh, crea la riga residua");
            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("voci frammentate");
            assert_eq!(voci.len(), 2, "prima di deselezionare restano due righe");

            toggle_comprato(&pool, comprata_id)
                .await
                .expect("deseleziona e rifonde subito");

            let voci = carica_voci(&pool, lista.id).await.expect("voci rifuse");
            assert_eq!(voci.len(), 1, "le due righe tornano una sola");
            assert_eq!(voci[0].quantita, Some(350.0));
            assert_eq!(voci[0].comprato, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn deselezionare_una_voce_manuale_non_scatena_un_refresh() {
        // Il ricalcolo automatico è un'eccezione solo per le voci
        // 'generato': una voce manuale non ha nulla con cui fondersi, e non
        // deve mai essere toccata da un refresh, nemmeno indiretto.
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let manuale_id = aggiungi_voce_manuale(&pool, lista.id, "Detersivo", None, None)
                .await
                .expect("manuale");
            imposta_comprato(&pool, manuale_id, true)
                .await
                .expect("segna comprata");

            toggle_comprato(&pool, manuale_id)
                .await
                .expect("deseleziona voce manuale");

            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 1);
            assert_eq!(voci[0].comprato, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn spuntare_una_voce_residua_la_fonde_con_quella_gia_comprata() {
        // Secondo scenario reale segnalato da Alessio: dopo che un secondo
        // pasto crea una riga residua (150 g) accanto a quella già
        // comprata (200 g), spuntare anche la residua non deve lasciare
        // due righe "Farina" comprate separate per sempre -- si fondono in
        // una sola da 350 g.
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;
            aggiorna_lista(&pool, &lista).await.expect("primo refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            let comprata_id = voci[0].id;
            imposta_comprato(&pool, comprata_id, true)
                .await
                .expect("segna comprata");

            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-05",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(150.0),
            )
            .await;
            aggiorna_lista(&pool, &lista)
                .await
                .expect("secondo refresh, crea la riga residua");
            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("voci frammentate");
            assert_eq!(voci.len(), 2);
            let residua_id = voci.iter().find(|v| v.id != comprata_id).unwrap().id;

            toggle_comprato(&pool, residua_id)
                .await
                .expect("spunta la residua e fonde");

            let voci = carica_voci(&pool, lista.id).await.expect("voci fuse");
            assert_eq!(voci.len(), 1, "le due righe comprate tornano una sola");
            assert_eq!(voci[0].quantita, Some(350.0));
            assert_eq!(voci[0].comprato, 1);
        })
        .await;
    }

    #[tokio::test]
    async fn voci_manuali_comprate_non_si_fondono_mai() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let prima_id = aggiungi_voce_manuale(&pool, lista.id, "Sale", Some(1.0), Some("kg"))
                .await
                .expect("prima voce manuale");
            let seconda_id = aggiungi_voce_manuale(&pool, lista.id, "Sale", Some(1.0), Some("kg"))
                .await
                .expect("seconda voce manuale");

            imposta_comprato(&pool, prima_id, true)
                .await
                .expect("segna comprata");
            toggle_comprato(&pool, seconda_id)
                .await
                .expect("spunta la seconda, nessuna fusione");

            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 2, "due voci manuali restano sempre separate");
        })
        .await;
    }

    #[tokio::test]
    async fn sposta_voce_scambia_con_la_vicina_e_ignora_lo_stato_comprato() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let primo = aggiungi_voce_manuale(&pool, lista.id, "Uno", None, None)
                .await
                .expect("uno");
            let secondo = aggiungi_voce_manuale(&pool, lista.id, "Due", None, None)
                .await
                .expect("due");
            let terzo = aggiungi_voce_manuale(&pool, lista.id, "Tre", None, None)
                .await
                .expect("tre");
            // Spuntare "Uno" non deve più farlo saltare in fondo (deciso
            // con Alessio il 9 settembre 2026).
            imposta_comprato(&pool, primo, true)
                .await
                .expect("segna comprato");

            let voci = carica_voci(&pool, lista.id).await.expect("ordine iniziale");
            assert_eq!(
                voci.iter().map(|v| v.id).collect::<Vec<_>>(),
                vec![primo, secondo, terzo],
                "l'ordine resta quello di inserimento, comprato o no"
            );

            sposta_voce(&pool, lista.id, secondo, Direzione::Su)
                .await
                .expect("sposta su");
            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("dopo lo spostamento");
            assert_eq!(
                voci.iter().map(|v| v.id).collect::<Vec<_>>(),
                vec![secondo, primo, terzo]
            );

            // La prima voce non può salire oltre la cima: nessun errore, nessun effetto.
            sposta_voce(&pool, lista.id, secondo, Direzione::Su)
                .await
                .expect("già in cima, nessun effetto");
            let voci = carica_voci(&pool, lista.id).await.expect("invariata");
            assert_eq!(
                voci.iter().map(|v| v.id).collect::<Vec<_>>(),
                vec![secondo, primo, terzo]
            );
        })
        .await;
    }

    #[tokio::test]
    async fn un_refresh_non_rimescola_un_ordine_gia_sistemato_a_mano() {
        // Trovato da Alessio dal vivo: qualunque `aggiorna_lista` (manuale
        // o scatenato da sola dal deseleziona, vedi `toggle_comprato`)
        // riportava le voci generate non comprate all'ordine di
        // ricalcolo, perdendo un ordine già sistemato con "↕️ Riordina
        // lista" -- non solo nel caso specifico del deseleziona.
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-03",
                "pianificato",
                None,
                None,
                Some(101),
                "Zucchero",
                "g",
                Some(100.0),
            )
            .await;
            aggiorna_lista(&pool, &lista).await.expect("primo refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("ordine iniziale");
            assert_eq!(voci.len(), 2);
            let seconda_id = voci[1].id;

            // L'utente inverte l'ordine a mano.
            sposta_voce(&pool, lista.id, seconda_id, Direzione::Su)
                .await
                .expect("sposta su");
            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("ordine invertito");
            assert_eq!(
                voci.iter()
                    .map(|v| v.descrizione.clone())
                    .collect::<Vec<_>>(),
                vec!["Zucchero".to_string(), "Farina".to_string()]
            );

            // Un secondo refresh cancella e reinserisce le righe generate
            // (nuovi id), senza che nulla sia cambiato nel planner: non
            // deve comunque riportare l'ordine a quello di ricalcolo.
            aggiorna_lista(&pool, &lista)
                .await
                .expect("secondo refresh, nessun cambiamento reale");
            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("ordine dopo il secondo refresh");
            assert_eq!(
                voci.iter()
                    .map(|v| v.descrizione.clone())
                    .collect::<Vec<_>>(),
                vec!["Zucchero".to_string(), "Farina".to_string()],
                "l'ordine sistemato a mano deve sopravvivere a un refresh che non cambia nulla"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn rimuovi_voce_manuale_la_elimina_anche_se_comprata() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let voce_id = aggiungi_voce_manuale(&pool, lista.id, "Detersivo", None, None)
                .await
                .expect("manuale");
            imposta_comprato(&pool, voce_id, true)
                .await
                .expect("segna comprata");

            rimuovi_voce_manuale(&pool, voce_id)
                .await
                .expect("rimozione");

            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert!(voci.is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn rimuovi_aggiunta_catalogo_fa_tornare_la_riga_al_solo_fabbisogno_del_planner() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(alimento_pasta),
                "Pasta",
                "g",
                Some(200.0),
            )
            .await;
            let aggiunta_id = aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento_pasta),
                "Pasta",
                50.0,
                "g",
            )
            .await
            .expect("aggiunta catalogo");
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("voci con l'aggiunta");
            assert_eq!(voci[0].quantita, Some(250.0));

            rimuovi_aggiunta_catalogo(&pool, aggiunta_id)
                .await
                .expect("rimozione");
            aggiorna_lista(&pool, &lista)
                .await
                .expect("refresh dopo la rimozione");

            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("voci dopo la rimozione");
            assert_eq!(voci.len(), 1);
            assert_eq!(
                voci[0].quantita,
                Some(200.0),
                "resta solo il fabbisogno del planner"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn voci_rimovibili_include_manuali_e_catalogo_non_le_righe_del_planner() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(alimento_pasta),
                "Pasta",
                "g",
                Some(200.0),
            )
            .await;
            aggiungi_voce_manuale(&pool, lista.id, "Detersivo", None, None)
                .await
                .expect("manuale");
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento_pasta),
                "Pasta",
                50.0,
                "g",
            )
            .await
            .expect("aggiunta catalogo");
            aggiorna_lista(&pool, &lista).await.expect("refresh");

            let rimovibili = voci_rimovibili(&pool, lista.id).await.expect("rimovibili");
            assert_eq!(
                rimovibili.len(),
                2,
                "solo manuale + aggiunta, non la riga del planner"
            );
            assert!(rimovibili
                .iter()
                .any(|v| v.origine == OrigineRimovibile::Manuale && v.descrizione == "Detersivo"));
            assert!(rimovibili
                .iter()
                .any(|v| v.origine == OrigineRimovibile::Catalogo && v.descrizione == "Pasta"));
        })
        .await;
    }

    /// "🗑️ Rimuovi tutte" (18 settembre 2026): via le voci scritte a mano e
    /// le aggiunte dal catalogo, in un colpo; la riga del pasto pianificato
    /// resta, e torna al solo fabbisogno del planner.
    #[tokio::test]
    async fn rimuovi_tutte_toglie_le_aggiunte_e_lascia_i_pasti() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(alimento_pasta),
                "Pasta",
                "g",
                Some(200.0),
            )
            .await;
            aggiungi_voce_manuale(&pool, lista.id, "Detersivo", None, None)
                .await
                .expect("manuale");
            aggiungi_voce_manuale(&pool, lista.id, "Pane", Some(1.0), Some("pz"))
                .await
                .expect("manuale 2");
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento_pasta),
                "Pasta",
                50.0,
                "g",
            )
            .await
            .expect("aggiunta catalogo");
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            assert_eq!(voci_rimovibili(&pool, lista.id).await.unwrap().len(), 3);

            let tolte = rimuovi_tutte_le_aggiunte(&pool, lista.id)
                .await
                .expect("rimozione");
            assert_eq!(tolte, 3);
            assert!(voci_rimovibili(&pool, lista.id).await.unwrap().is_empty());

            aggiorna_lista(&pool, &lista).await.expect("refresh dopo");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 1, "resta solo la riga del pasto");
            assert_eq!(voci[0].quantita, Some(200.0), "senza più i 50 g aggiunti");
        })
        .await;
    }

    /// Voce del catalogo senza quantità (18 settembre 2026): resta fuori
    /// dalle scorte, a meno che con 📦 si segni quanto se ne è preso.
    #[tokio::test]
    async fn una_voce_del_catalogo_senza_quantita_entra_in_casa_solo_con_la_presa() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let pasta = create_alimento_globale(&pool, "Pasta").await;
        let farina = create_alimento_globale(&pool, "Farina").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let senza_presa = aggiungi_catalogo_senza_quantita(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(pasta),
                "Pasta",
            )
            .await
            .expect("pasta");
            let con_presa = aggiungi_catalogo_senza_quantita(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(farina),
                "Farina",
            )
            .await
            .expect("farina");

            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 2);
            assert!(voci.iter().all(|voce| voce.quantita.is_none()));
            assert!(
                voci.iter().all(|voce| voce.alimento_id.is_some()),
                "restano legate al catalogo"
            );
            // Un refresh non le tocca: non sono righe del planner.
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            assert_eq!(carica_voci(&pool, lista.id).await.unwrap().len(), 2);

            imposta_comprato(&pool, senza_presa, true)
                .await
                .expect("spunta");
            registra_presa(&pool, con_presa, 1.0, "kg", None)
                .await
                .expect("presa");
            chiudi_spesa(&pool, &lista).await.expect("chiusura");

            let scorte_pasta: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM scorte WHERE alimento_id = ?")
                    .bind(pasta)
                    .fetch_one(&pool)
                    .await
                    .expect("scorte pasta");
            assert_eq!(scorte_pasta, 0, "senza quantità non entra in casa");
            let scorte_farina: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM scorte WHERE alimento_id = ?")
                    .bind(farina)
                    .fetch_one(&pool)
                    .await
                    .expect("scorte farina");
            assert_eq!(scorte_farina, 1, "con la presa segnata sì");
        })
        .await;
    }

    /// Il caso del collaudo del 23 settembre 2026 (punti 6 e 7): si chiedono
    /// 500 g, in casa ce ne sono 300, quindi in lista ne compaiono 200. Presi
    /// quei 200 e chiusa la spesa, la richiesta e' servita per intero -- in
    /// casa ce ne sono 500 -- e l'aggiunta si chiude senza chiedere niente.
    ///
    /// Prima restava viva (il comprato non copriva i 500 richiesti),
    /// invisibile nella lista vuota ma ancora contata nel fabbisogno e ancora
    /// elencata in "Rimuovi voci".
    #[tokio::test]
    async fn una_aggiunta_servita_dalla_spesa_si_chiude_senza_chiedere() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let parmigiano = create_alimento_globale(&pool, "Parmigiano Reggiano").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            crate::modules::dispensa::aggiungi_scorta(
                &pool,
                crate::modules::dispensa::Conservazione::Frigo,
                Some(IdentitaCatalogo::Alimento(parmigiano)),
                "Parmigiano Reggiano",
                300.0,
                "g",
            )
            .await
            .expect("scorta");
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(parmigiano),
                "Parmigiano Reggiano",
                500.0,
                "g",
            )
            .await
            .expect("aggiunta");
            aggiorna_lista(&pool, &lista).await.expect("refresh");

            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 1);
            assert_eq!(voci[0].quantita, Some(200.0), "il netto delle scorte");
            imposta_comprato(&pool, voci[0].id, true)
                .await
                .expect("comprato");

            let esito = chiudi_spesa(&pool, &lista).await.expect("chiusura");
            assert!(
                esito.ridotte.is_empty(),
                "non manca piu' niente: niente da chiedere"
            );
            assert!(
                voci_rimovibili(&pool, lista.id).await.unwrap().is_empty(),
                "niente voci fantasma in Rimuovi voci"
            );
            aggiorna_lista(&pool, &lista).await.expect("refresh dopo");
            assert!(
                carica_voci(&pool, lista.id).await.unwrap().is_empty(),
                "l'aggiunta non pesa piu' sul fabbisogno"
            );
        })
        .await;
    }

    /// Un'aggiunta di cui non si è comprato niente resta: è una richiesta non
    /// ancora servita.
    #[tokio::test]
    async fn una_aggiunta_mai_comprata_sopravvive_alla_chiusura() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let pasta = create_alimento_globale(&pool, "Pasta").await;
        let farina = create_alimento_globale(&pool, "Farina").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(pasta),
                "Pasta",
                500.0,
                "g",
            )
            .await
            .expect("pasta");
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(farina),
                "Farina",
                1000.0,
                "g",
            )
            .await
            .expect("farina");
            aggiorna_lista(&pool, &lista).await.expect("refresh");

            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            let pasta_voce = voci
                .iter()
                .find(|voce| voce.descrizione.contains("Pasta"))
                .expect("voce pasta");
            imposta_comprato(&pool, pasta_voce.id, true)
                .await
                .expect("comprato");
            chiudi_spesa(&pool, &lista).await.expect("chiusura");

            aggiorna_lista(&pool, &lista).await.expect("refresh dopo");
            let rimaste = carica_voci(&pool, lista.id).await.expect("voci dopo");
            assert_eq!(rimaste.len(), 1, "resta solo la farina mai comprata");
            assert!(rimaste[0].descrizione.contains("Farina"));
        })
        .await;
    }

    #[tokio::test]
    async fn eccessi_comprati_segnala_un_pasto_tolto_dal_planner() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            let pasto_id = create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            imposta_comprato(&pool, voci[0].id, true)
                .await
                .expect("segna comprata");

            let eccessi = eccessi_comprati(&pool, &lista).await.expect("eccessi");
            assert!(eccessi.is_empty(), "il fabbisogno copre ancora il comprato");

            // Il pasto viene tolto del tutto dal planner (il pomodoro
            // usato dalla ricetta non serve più).
            sqlx::query("DELETE FROM planner_pasti WHERE id = ?")
                .bind(pasto_id)
                .execute(&pool)
                .await
                .expect("rimuovi il pasto");

            let eccessi = eccessi_comprati(&pool, &lista).await.expect("eccessi");
            assert_eq!(eccessi.len(), 1);
            assert_eq!(eccessi[0].nome, "Farina");
            assert_eq!(eccessi[0].quantita_eccesso, 200.0);
        })
        .await;
    }

    #[tokio::test]
    async fn serve_aggiornamento_e_falso_subito_dopo_un_refresh() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;

            assert!(
                serve_aggiornamento(&pool, &lista).await.expect("verifica"),
                "un pasto pianificato non ancora aggregato deve mostrare il bottone"
            );

            aggiorna_lista(&pool, &lista).await.expect("refresh");
            assert!(
                !serve_aggiornamento(&pool, &lista).await.expect("verifica"),
                "subito dopo un refresh non c'è nulla di nuovo da aggiornare"
            );

            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-05",
                "pianificato",
                None,
                None,
                Some(100),
                "Farina",
                "g",
                Some(50.0),
            )
            .await;
            assert!(
                serve_aggiornamento(&pool, &lista).await.expect("verifica"),
                "un nuovo pasto pianificato rende di nuovo utile il refresh"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn aggiorna_lista_attiva_trova_o_crea_da_sola() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let numero = aggiorna_lista_attiva(&pool).await.expect("refresh");
            assert_eq!(numero, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn aggiunta_alimento_generico_si_somma_al_fabbisogno_del_planner() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(alimento_pasta),
                "Pasta",
                "g",
                Some(200.0),
            )
            .await;

            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento_pasta),
                "Pasta",
                50.0,
                "g",
            )
            .await
            .expect("aggiunta catalogo");

            // L'esempio esatto di Alessio: 200 g dal planner + 50 g
            // aggiunti a mano sullo stesso alimento generico devono dare
            // 250 g in un'unica riga, non due.
            let numero = aggiorna_lista(&pool, &lista).await.expect("refresh");
            assert_eq!(numero, 1);
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 1);
            assert_eq!(voci[0].quantita, Some(250.0));
        })
        .await;
    }

    #[tokio::test]
    async fn aggiunta_prodotto_specifico_resta_riga_separata_dal_planner() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;
        let prodotto_de_cecco = create_prodotto(&pool, alimento_pasta, "De Cecco", "Pasta").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(alimento_pasta),
                "Pasta",
                "g",
                Some(200.0),
            )
            .await;

            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Prodotto(prodotto_de_cecco),
                "De Cecco Pasta",
                500.0,
                "g",
            )
            .await
            .expect("aggiunta catalogo");

            // Il prodotto specifico resta sempre una riga separata, anche
            // se collegato allo stesso alimento generico richiesto dal
            // planner -- mai un merge, decisione esplicita presa con
            // Alessio.
            let numero = aggiorna_lista(&pool, &lista).await.expect("refresh");
            assert_eq!(numero, 2);
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 2);
            let generico = voci.iter().find(|v| v.descrizione == "Pasta").unwrap();
            assert_eq!(generico.quantita, Some(200.0));
            let specifico = voci
                .iter()
                .find(|v| v.descrizione == "De Cecco Pasta")
                .unwrap();
            assert_eq!(specifico.quantita, Some(500.0));
        })
        .await;
    }

    #[tokio::test]
    async fn aggiunta_dal_catalogo_persiste_attraverso_un_secondo_refresh() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");

            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento_pasta),
                "Pasta",
                50.0,
                "g",
            )
            .await
            .expect("aggiunta catalogo");

            // A differenza delle righe 'generato' pure-planner, un'aggiunta
            // dal catalogo non e' uno snapshot: partecipa di nuovo a ogni
            // refresh, anche senza alcun pasto pianificato nel mezzo.
            aggiorna_lista(&pool, &lista).await.expect("primo refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 1);
            assert_eq!(voci[0].quantita, Some(50.0));

            aggiorna_lista(&pool, &lista)
                .await
                .expect("secondo refresh");
            let voci = carica_voci(&pool, lista.id)
                .await
                .expect("voci dopo il secondo refresh");
            assert_eq!(voci.len(), 1);
            assert_eq!(voci[0].quantita, Some(50.0));
        })
        .await;
    }

    #[tokio::test]
    async fn voce_comprata_da_prodotto_resta_congelata_anche_sulla_nuova_colonna() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;
        let prodotto_de_cecco = create_prodotto(&pool, alimento_pasta, "De Cecco", "Pasta").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Prodotto(prodotto_de_cecco),
                "De Cecco Pasta",
                500.0,
                "g",
            )
            .await
            .expect("aggiunta catalogo");
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            let voce_id = voci[0].id;
            imposta_comprato(&pool, voce_id, true)
                .await
                .expect("comprato");

            let esito = sqlx::query(
                "UPDATE liste_spesa_voci SET prodotto_alimentare_id = NULL WHERE id = ?",
            )
            .bind(voce_id)
            .execute(&pool)
            .await;
            assert!(
                esito.is_err(),
                "il trigger deve bloccare anche la modifica di prodotto_alimentare_id"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn ricerca_nel_catalogo_trova_alimenti_e_prodotti_distinti() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        let alimento_pasta = create_alimento_globale(&pool, "Pasta").await;
        create_prodotto(&pool, alimento_pasta, "De Cecco", "Pasta").await;
        create_alimento_globale(&pool, "Passata di pomodoro").await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let risultati = cerca_nel_catalogo(&pool, "pasta", 10)
                .await
                .expect("ricerca catalogo");
            // Il catalogo base seminato dalle migration contiene anche
            // "Pasta sfoglia", "Pasta brisée", "Pasta fillo": la ricerca
            // trova tutti gli alimenti che contengono "pasta", non solo
            // quello esatto -- qui basta verificare che l'alimento generico
            // "Pasta" e il prodotto "De Cecco Pasta" siano entrambi tra i
            // risultati, come tipi distinti.
            let trova_alimento_esatto = risultati.iter().any(
                |r| matches!(r, RisultatoCatalogo::Alimento { id, .. } if *id == alimento_pasta),
            );
            let prodotti: Vec<_> = risultati
                .iter()
                .filter(|r| matches!(r, RisultatoCatalogo::Prodotto { .. }))
                .collect();
            assert!(
                trova_alimento_esatto,
                "l'alimento generico Pasta deve essere tra i risultati"
            );
            assert_eq!(prodotti.len(), 1);
        })
        .await;
    }

    /// Il messaggio dell'aggiunta deve dire lo stesso numero che si legge in
    /// lista. Aggiungendo due volte lo stesso alimento la lista mostra due
    /// righe, e il bot ne leggeva una sola: diceva "ne restano 10 g" con 210
    /// g sotto gli occhi (Alessio, collaudo del 24 settembre 2026, C1).
    #[tokio::test]
    async fn il_messaggio_dell_aggiunta_dice_quanto_se_ne_vede_in_lista() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let alimento = create_alimento_globale(&pool, "Parmigiano").await;
            crate::modules::dispensa::aggiungi_scorta(
                &pool,
                crate::modules::dispensa::Conservazione::Frigo,
                Some(IdentitaCatalogo::Alimento(alimento)),
                "Parmigiano",
                590.0,
                "g",
            )
            .await
            .expect("scorta");

            // Primo giro: 800 g chiesti, 590 in casa, 210 da comprare.
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento),
                "Parmigiano",
                800.0,
                "g",
            )
            .await
            .expect("aggiunta");
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let messaggio = spiega_aggiunta(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento),
                800.0,
                "g",
            )
            .await;
            assert!(
                messaggio.contains("210 g"),
                "il messaggio deve dire 210 g: {messaggio}"
            );

            // Di un altro alimento in casa ce n'e' gia' abbastanza: la lista
            // non chiede niente, e il messaggio lo spiega invece di far
            // dubitare del salvataggio.
            let riso = create_alimento_globale(&pool, "Riso").await;
            crate::modules::dispensa::aggiungi_scorta(
                &pool,
                crate::modules::dispensa::Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(riso)),
                "Riso",
                1000.0,
                "g",
            )
            .await
            .expect("scorta di riso");
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(riso),
                "Riso",
                100.0,
                "g",
            )
            .await
            .expect("aggiunta riso");
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let messaggio = spiega_aggiunta(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(riso),
                100.0,
                "g",
            )
            .await;
            assert!(messaggio.contains("abbastanza"), "{messaggio}");
        })
        .await;
    }

    /// Scelta di Alessio del 24 settembre 2026: di quello che si e' chiesto
    /// e non si e' ancora comprato non si decide al posto suo. Il numero che
    /// il bot dice e' lo stesso che si legge in lista.
    #[tokio::test]
    async fn di_quel_che_manca_ancora_il_bot_chiede_cosa_farne() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            let alimento = create_alimento_globale(&pool, "Parmigiano").await;

            // Se ne chiedono 800 g con la casa vuota: la lista li chiede
            // tutti.
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento),
                "Parmigiano",
                800.0,
                "g",
            )
            .await
            .expect("aggiunta");
            aggiorna_lista(&pool, &lista).await.expect("refresh");

            // Al negozio se ne prendono solo 500 (il pulsante "Ho preso").
            let voce = carica_voci(&pool, lista.id)
                .await
                .expect("voci")
                .into_iter()
                .next()
                .expect("la riga del parmigiano");
            assert_eq!(voce.quantita, Some(800.0));
            registra_presa(&pool, voce.id, 500.0, "g", None)
                .await
                .expect("presa");

            let esito = chiudi_spesa(&pool, &lista).await.expect("chiusura");
            assert_eq!(esito.archiviate, 1);
            // I 500 g presi sono entrati in casa: dei combinati 800
            // richiesti ne mancano 300, ed e' quello che la lista continua a
            // chiedere.
            assert_eq!(esito.ridotte.len(), 1);
            assert_eq!(esito.ridotte[0].quantita, 300.0);
            assert_eq!(esito.ridotte[0].unita_simbolo, "g");

            // L'aggiunta resta intera -- la quantita' non si tocca, ci pensa
            // la lista a mostrarne il netto -- e porta il segno di questa
            // chiusura.
            let rimaste: Vec<(f64, Option<i64>)> = sqlx::query_as(
                "SELECT quantita, ridotta_chiusura_id FROM liste_spesa_aggiunte_catalogo",
            )
            .fetch_all(&pool)
            .await
            .expect("aggiunte rimaste");
            assert_eq!(rimaste.len(), 1);
            assert_eq!(rimaste[0], (800.0, Some(esito.chiusura_id)));

            // "No, toglile": sparisce, e nessun'altra con lei.
            let tolte = togli_aggiunte_ridotte(&pool, esito.chiusura_id)
                .await
                .expect("rimozione");
            assert_eq!(tolte, 1);
            let restano: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM liste_spesa_aggiunte_catalogo")
                    .fetch_one(&pool)
                    .await
                    .expect("conteggio");
            assert_eq!(restano, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn chiudi_spesa_archivia_le_comprate_e_lascia_le_altre() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();

            let alimento = create_alimento_globale(&pool, "Farina").await;
            let planner_id =
                create_planner(&pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
            create_meal_with_ingredient(
                &pool,
                planner_id,
                "2026-09-02",
                "pianificato",
                None,
                None,
                Some(alimento),
                "Farina",
                "g",
                Some(200.0),
            )
            .await;
            aggiorna_lista(&pool, &lista).await.expect("refresh");

            let manuale = aggiungi_voce_manuale(&pool, lista.id, "Detersivo piatti", None, None)
                .await
                .expect("voce manuale");
            let generata = carica_voci(&pool, lista.id)
                .await
                .expect("voci")
                .into_iter()
                .find(|voce| voce.origine == "generato")
                .expect("voce generata");
            imposta_comprato(&pool, generata.id, true)
                .await
                .expect("comprato");

            let esito = chiudi_spesa(&pool, &lista).await.expect("chiusura");
            assert_eq!(esito.archiviate, 1);
            // La voce era collegata al catalogo e l'ingresso automatico è
            // acceso di default: è entrata anche in dispensa.
            assert_eq!(
                esito.entrate,
                vec![(crate::modules::dispensa::Conservazione::Dispensa, 1)]
            );

            // In lista resta solo la voce manuale non comprata.
            let rimaste = carica_voci(&pool, lista.id).await.expect("voci dopo");
            assert_eq!(rimaste.len(), 1);
            assert_eq!(rimaste[0].id, manuale);

            let chiusura = ultima_chiusura(&pool, lista.id)
                .await
                .expect("ultima chiusura")
                .expect("presente");
            assert_eq!(chiusura.voci_totali, 1);
            assert_eq!(chiusura.data_inizio, "2026-09-01");
            let voci = voci_archiviate(&pool, chiusura.id).await.expect("archivio");
            assert_eq!(voci.len(), 1);
            assert_eq!(voci[0].descrizione, "Farina");
            assert_eq!(voci[0].quantita, Some(200.0));

            // Una voce archiviata è storia: il trigger a database impedisce
            // di modificarla, come per una voce comprata.
            let esito = sqlx::query(
                "UPDATE liste_spesa_voci_archiviate SET descrizione = 'Altro' WHERE id = ?",
            )
            .bind(1_i64)
            .execute(&pool)
            .await;
            assert!(esito.is_err(), "il trigger deve bloccare la modifica");

            // Il pasto è ancora pianificato, ma la farina comprata è entrata
            // in casa: il ricalcolo la sottrae e la voce non ricompare. È il
            // difetto trovato da Alessio collaudando il 16 settembre 2026 --
            // prima di questa correzione la voce tornava subito, identica,
            // come se non fosse mai stata comprata.
            aggiorna_lista(&pool, &lista).await.expect("refresh dopo");
            let dopo = carica_voci(&pool, lista.id).await.expect("voci finali");
            assert!(
                !dopo.iter().any(|voce| voce.origine == "generato"),
                "la voce comprata non deve ricomparire"
            );

            // Se in casa non c'è più (usata altrove), il fabbisogno torna.
            sqlx::query("DELETE FROM scorte")
                .execute(&pool)
                .await
                .expect("scorte consumate");
            aggiorna_lista(&pool, &lista).await.expect("refresh finale");
            let finale = carica_voci(&pool, lista.id).await.expect("voci finali");
            assert!(finale
                .iter()
                .any(|voce| voce.origine == "generato" && voce.quantita == Some(200.0)));
        })
        .await;
    }

    #[tokio::test]
    async fn chiudere_la_spesa_toglie_l_aggiunta_dal_catalogo_gia_comprata() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07", true)
                .await
                .expect("intervallo");
            let lista = trova_lista_attiva(&pool).await.unwrap().unwrap();

            let alimento = create_alimento_globale(&pool, "Pasta").await;
            aggiungi_da_catalogo(
                &pool,
                lista.id,
                IdentitaCatalogo::Alimento(alimento),
                "Pasta",
                50.0,
                "g",
            )
            .await
            .expect("aggiunta");
            aggiorna_lista(&pool, &lista).await.expect("refresh");

            let voce = carica_voci(&pool, lista.id)
                .await
                .expect("voci")
                .into_iter()
                .find(|voce| voce.origine == "generato")
                .expect("voce generata");
            imposta_comprato(&pool, voce.id, true)
                .await
                .expect("comprato");

            chiudi_spesa(&pool, &lista).await.expect("chiusura");
            aggiorna_lista(&pool, &lista).await.expect("refresh dopo");

            // L'aggiunta è stata comprata: non deve tornare da sola al primo
            // ricalcolo, altrimenti la lista ripartirebbe sporca.
            let rimaste = carica_voci(&pool, lista.id).await.expect("voci dopo");
            assert!(rimaste.is_empty(), "la lista deve restare pulita");
            let aggiunte: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM liste_spesa_aggiunte_catalogo WHERE lista_id = ?",
            )
            .bind(lista.id)
            .fetch_one(&pool)
            .await
            .expect("conteggio aggiunte");
            assert_eq!(aggiunte, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn la_lista_rimasta_indietro_riparte_da_oggi_alla_riapertura() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let oggi: String = sqlx::query_scalar("SELECT date('now','localtime')")
                .fetch_one(&pool)
                .await
                .expect("oggi");
            let dieci_giorni_fa = calendario::shift_date(&oggi, -10).expect("data");
            let tre_giorni_fa = calendario::shift_date(&oggi, -3).expect("data");

            let lista = trova_o_crea_lista_attiva(&pool).await.expect("lista");
            // Intervallo tutto nel passato, non scelto a mano: alla
            // riapertura la lista si sposta da sola.
            cambia_intervallo(&pool, lista.id, &dieci_giorni_fa, &tre_giorni_fa, false)
                .await
                .expect("intervallo");

            let riaperta = trova_o_crea_lista_attiva(&pool).await.expect("riapertura");
            assert_eq!(riaperta.data_inizio, oggi);
            assert_eq!(
                riaperta.data_fine,
                calendario::shift_date(&oggi, 6).expect("fine")
            );

            // Stesso intervallo, ma scelto dall'utente: resta dov'è.
            cambia_intervallo(&pool, lista.id, &dieci_giorni_fa, &tre_giorni_fa, true)
                .await
                .expect("intervallo manuale");
            let manuale = trova_o_crea_lista_attiva(&pool)
                .await
                .expect("riapertura manuale");
            assert_eq!(manuale.data_inizio, dieci_giorni_fa);
            assert_eq!(manuale.data_fine, tre_giorni_fa);
        })
        .await;
    }

    /// Lista con intervallo fisso e un pasto pianificato che chiede 200 g di
    /// farina: la base dei test sul netto e sul resoconto.
    async fn lista_con_farina(
        pool: &SqlitePool,
        user_id: i64,
        space_id: i64,
    ) -> (ListaSpesa, i64, i64) {
        let lista = trova_o_crea_lista_attiva(pool).await.expect("lista");
        cambia_intervallo(pool, lista.id, "2026-09-01", "2026-09-07", true)
            .await
            .expect("intervallo");
        let lista = trova_lista_attiva(pool).await.unwrap().unwrap();
        let farina = create_alimento_globale(pool, "Farina").await;
        let planner_id = create_planner(pool, user_id, space_id, "2026-09-01", "2026-09-07").await;
        let pasto = create_meal_with_ingredient(
            pool,
            planner_id,
            "2026-09-02",
            "pianificato",
            None,
            None,
            Some(farina),
            "Farina",
            "g",
            Some(200.0),
        )
        .await;
        (lista, farina, pasto)
    }

    #[tokio::test]
    async fn la_confezione_presa_entra_in_casa_al_posto_della_quantita_in_lista() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let (lista, farina, _) = lista_con_farina(&pool, user_id, space_id).await;
            let prodotto = create_prodotto(&pool, farina, "Molino", "Farina 00").await;
            let grammi = unita_id(&pool, "g").await;
            let formato: i64 = sqlx::query(
                "INSERT INTO formati_prodotto_alimentare \
                 (prodotto_alimentare_id, quantita_confezione, unita_confezione_id) \
                 VALUES (?, 1000, ?)",
            )
            .bind(prodotto)
            .bind(grammi)
            .execute(&pool)
            .await
            .expect("formato")
            .last_insert_rowid();
            // Un formato uguale a quello base non va proposto due volte.
            sqlx::query(
                "INSERT INTO formati_prodotto_alimentare \
                 (prodotto_alimentare_id, quantita_confezione, unita_confezione_id) \
                 VALUES (?, 500, ?)",
            )
            .bind(prodotto)
            .bind(grammi)
            .execute(&pool)
            .await
            .expect("formato doppione");

            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let voce = carica_voci(&pool, lista.id).await.expect("voci").remove(0);
            assert_eq!(voce.quantita, Some(200.0));

            let confezioni = confezioni_per_voce(&pool, farina, None)
                .await
                .expect("confezioni");
            let tokens: Vec<&str> = confezioni.iter().map(|c| c.token.as_str()).collect();
            assert_eq!(tokens, vec![format!("p{prodotto}"), format!("f{formato}")]);
            assert_eq!(confezioni[1].etichetta, "Molino Farina 00 · 1000 g");

            let (preso_da, quantita, unita) = confezione_da_token(&pool, &format!("f{formato}"))
                .await
                .expect("lettura")
                .expect("confezione");
            assert_eq!(preso_da, prodotto);
            registra_presa(&pool, voce.id, quantita, &unita, Some(preso_da))
                .await
                .expect("presa");

            let voce = carica_voci(&pool, lista.id).await.expect("voci").remove(0);
            assert_eq!(voce.comprato, 1, "segnare la presa spunta la voce");
            assert_eq!(voce.quantita_presa, Some(1000.0));
            assert_eq!(voce.quantita, Some(200.0), "quanto serviva resta");

            chiudi_spesa(&pool, &lista).await.expect("chiusura");
            let archiviata: (Option<f64>, Option<f64>, Option<i64>) = sqlx::query_as(
                "SELECT quantita, quantita_richiesta, prodotto_alimentare_id \
                 FROM liste_spesa_voci_archiviate",
            )
            .fetch_one(&pool)
            .await
            .expect("archivio");
            assert_eq!(archiviata, (Some(1000.0), Some(200.0), Some(prodotto)));
            let in_casa: f64 =
                sqlx::query_scalar("SELECT SUM(quantita) FROM scorte WHERE alimento_id = ?")
                    .bind(farina)
                    .fetch_one(&pool)
                    .await
                    .expect("scorte");
            assert_eq!(in_casa, 1000.0, "entra la confezione intera");
        })
        .await;
    }

    #[tokio::test]
    async fn togliere_la_spunta_dimentica_la_presa() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let (lista, _, _) = lista_con_farina(&pool, user_id, space_id).await;
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let voce_id = carica_voci(&pool, lista.id).await.expect("voci")[0].id;

            registra_presa(&pool, voce_id, 300.0, "g", None)
                .await
                .expect("presa");
            annulla_presa(&pool, voce_id).await.expect("annulla");
            let voce = carica_voci(&pool, lista.id).await.expect("voci").remove(0);
            assert_eq!(voce.comprato, 1, "annullare la presa lascia la spunta");
            assert_eq!(voce.quantita_presa, None);

            registra_presa(&pool, voce_id, 300.0, "g", None)
                .await
                .expect("presa di nuovo");
            imposta_comprato(&pool, voce_id, false)
                .await
                .expect("spunta tolta");
            let voce = carica_voci(&pool, lista.id).await.expect("voci").remove(0);
            assert_eq!(voce.quantita_presa, None);
            assert_eq!(voce.unita_presa, None);

            assert!(
                registra_presa(&pool, 999_999, 1.0, "g", None)
                    .await
                    .is_err(),
                "una voce che non esiste non si segna"
            );
        })
        .await;
    }

    /// Il prezzo segnato sulla voce: fino al 18 settembre 2026 questa strada
    /// non era coperta da nessun test, e infatti non funzionava — la query
    /// non leggeva una colonna che la struct pretendeva. Il test passa da
    /// `registra_prezzo_voce`, cioè esattamente da dove passa il bot.
    #[tokio::test]
    async fn il_prezzo_di_una_voce_si_salva_e_finisce_nello_storico_del_negozio() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let (lista, farina, _) = lista_con_farina(&pool, user_id, space_id).await;
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let voce_id = carica_voci(&pool, lista.id).await.expect("voci")[0].id;
            let negozio = crate::modules::mercato::negozi_visibili(&pool)
                .await
                .expect("negozi")
                .into_iter()
                .find(|negozio| negozio.nome == "Coop")
                .expect("Coop")
                .id;

            registra_prezzo_voce(&pool, voce_id, 249, Some(negozio))
                .await
                .expect("prezzo");

            let voce = carica_voci(&pool, lista.id).await.expect("voci").remove(0);
            assert_eq!(voce.prezzo_centesimi, Some(249));
            assert_eq!(voce.comprato, 1, "segnare il prezzo spunta la voce");
            let storico =
                crate::modules::mercato::ultimo_prezzo(&pool, negozio, Some(farina), None)
                    .await
                    .expect("storico")
                    .expect("prezzo nello storico");
            assert_eq!(storico.0, 249);
            assert_eq!(storico.1, Some(200.0), "la quantità della voce");
            assert_eq!(storico.2.as_deref(), Some("g"));

            // Senza negozio il prezzo resta comunque sulla voce: è la spesa
            // di chi non sceglie il negozio, e non deve rompersi.
            registra_prezzo_voce(&pool, voce_id, 300, None)
                .await
                .expect("prezzo senza negozio");
            let voce = carica_voci(&pool, lista.id).await.expect("voci").remove(0);
            assert_eq!(voce.prezzo_centesimi, Some(300));

            // Chiudendo la spesa il prezzo va in archivio con la voce.
            chiudi_spesa(&pool, &lista).await.expect("chiusura");
            let archiviato: Option<i64> =
                sqlx::query_scalar("SELECT prezzo_centesimi FROM liste_spesa_voci_archiviate")
                    .fetch_one(&pool)
                    .await
                    .expect("archivio");
            assert_eq!(archiviato, Some(300));
        })
        .await;
    }

    /// La legenda: accesa all'inizio, si spegne e resta spenta.
    #[tokio::test]
    async fn la_legenda_parte_accesa_e_si_spegne() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;
        sqlx::query("INSERT INTO preferenze_utente (utente_id, spazio_attivo_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(space_id)
            .execute(&pool)
            .await
            .expect("preferenze");

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            assert!(liste::legenda_attiva(&pool).await, "accesa all'inizio");
            assert!(!liste::cambia_legenda(&pool).await);
            assert!(!liste::legenda_attiva(&pool).await, "resta spenta");
            assert!(liste::cambia_legenda(&pool).await);
            assert!(liste::legenda_attiva(&pool).await);
        })
        .await;
    }

    #[tokio::test]
    async fn quello_che_c_e_in_casa_si_toglie_dalla_lista() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let (lista, farina, _) = lista_con_farina(&pool, user_id, space_id).await;
            crate::modules::dispensa::aggiungi_scorta(
                &pool,
                crate::modules::dispensa::Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(farina)),
                "Farina",
                150.0,
                "g",
            )
            .await
            .expect("scorta");

            aggiorna_lista(&pool, &lista).await.expect("refresh");
            let voci = carica_voci(&pool, lista.id).await.expect("voci");
            assert_eq!(voci.len(), 1);
            assert_eq!(
                voci[0].quantita,
                Some(50.0),
                "servono solo i 50 g che mancano"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn un_pasto_gia_scaricato_non_chiede_piu_niente_alla_lista() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let (lista, _, pasto) = lista_con_farina(&pool, user_id, space_id).await;
            // Preparato (o passato l'orario): gli ingredienti sono già stati
            // usati, non vanno ricomprati.
            crate::modules::dispensa::scala_scorte_per_pasto(&pool, pasto, false)
                .await
                .expect("scarico");
            aggiorna_lista(&pool, &lista).await.expect("refresh");
            assert!(carica_voci(&pool, lista.id).await.unwrap().is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn l_aggiornamento_registra_cosa_ha_cambiato() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let (lista, farina, _) = lista_con_farina(&pool, user_id, space_id).await;

            let modifiche = aggiorna_e_registra(&pool, &lista, true)
                .await
                .expect("primo");
            assert_eq!(modifiche.len(), 1);
            assert_eq!(modifiche[0].tipo, TipoModifica::Aggiunta);

            // Nessun cambiamento: niente di registrato.
            assert!(aggiorna_e_registra(&pool, &lista, true)
                .await
                .expect("secondo")
                .is_empty());

            // Arriva della farina in casa: la voce cala, e il motivo lo dice.
            crate::modules::dispensa::aggiungi_scorta(
                &pool,
                crate::modules::dispensa::Conservazione::Dispensa,
                Some(IdentitaCatalogo::Alimento(farina)),
                "Farina",
                120.0,
                "g",
            )
            .await
            .expect("scorta");
            let modifiche = aggiorna_e_registra(&pool, &lista, false)
                .await
                .expect("terzo");
            assert_eq!(modifiche.len(), 1);
            assert_eq!(modifiche[0].tipo, TipoModifica::Ridotta);
            assert_eq!(modifiche[0].quantita_dopo, Some(80.0));
            assert_eq!(modifiche[0].motivo.as_deref(), Some(MOTIVO_IN_CASA));

            let (id, automatico, _) = ultimo_aggiornamento(&pool, lista.id)
                .await
                .expect("lettura")
                .expect("presente");
            assert!(!automatico);
            let registrate = modifiche_di(&pool, id).await.expect("modifiche");
            assert_eq!(registrate, modifiche);
        })
        .await;
    }

    #[tokio::test]
    async fn dopo_un_aggiornamento_le_posizioni_non_si_ripetono_mai() {
        let pool = test_pool().await;
        let user_id = create_user(&pool, "Alessio").await;
        let space_id = create_space(&pool, "Casa").await;
        add_membership(&pool, space_id, user_id).await;

        crate::identity::with_actor(actor(user_id, space_id, "Alessio"), async {
            let (lista, _, _) = lista_con_farina(&pool, user_id, space_id).await;
            aggiungi_voce_manuale(&pool, lista.id, "Pane", None, None)
                .await
                .unwrap();
            aggiungi_voce_manuale(&pool, lista.id, "Latte", None, None)
                .await
                .unwrap();
            aggiorna_lista(&pool, &lista).await.unwrap();
            // Il caso trovato sul database reale: due voci con la stessa
            // posizione, che le frecce non riuscivano a scambiare.
            sqlx::query("UPDATE liste_spesa_voci SET ordinamento = 1 WHERE lista_id = ?")
                .bind(lista.id)
                .execute(&pool)
                .await
                .unwrap();

            aggiorna_lista(&pool, &lista).await.unwrap();
            let voci = carica_voci(&pool, lista.id).await.unwrap();
            let posizioni: Vec<i64> = voci.iter().map(|v| v.ordinamento).collect();
            assert_eq!(posizioni, vec![1, 2, 3]);

            // E ora lo spostamento si vede davvero.
            let prima = voci[0].id;
            sposta_voce(&pool, lista.id, prima, Direzione::Giu)
                .await
                .unwrap();
            let dopo = carica_voci(&pool, lista.id).await.unwrap();
            assert_eq!(dopo[1].id, prima);
        })
        .await;
    }
}

// ===========================================================================
// UI Telegram.
// ===========================================================================

#[derive(Debug, Clone, Default)]
pub struct ListaSpesaSessionStore {
    inner: Arc<Mutex<HashMap<i64, ListaSpesaConversationState>>>,
}

#[derive(Debug, Clone)]
// Il prefisso comune "Awaiting" descrive la natura di ogni stato (in attesa
// di un input testuale) e va tenuto per coerenza con le altre mappe di
// sessione del progetto -- non è un nome ripetuto per distrazione.
#[allow(clippy::enum_variant_names)]
enum ListaSpesaConversationState {
    AwaitingDescrizione,
    AwaitingQuantita {
        descrizione: String,
    },
    AwaitingCatalogoQuery,
    AwaitingQuantitaCatalogo {
        identita: IdentitaCatalogo,
        descrizione: String,
        unita_default: Option<String>,
    },
    /// Nella schermata "📦 Ho preso…": un testo è la quantità presa.
    AwaitingQuantitaPresa {
        voce_id: i64,
        unita_default: Option<String>,
    },
    /// Consegna B: il nome di un negozio nuovo.
    AwaitingNomeNegozio,
    /// Il nome nuovo di un negozio creato a mano (18 settembre 2026).
    AwaitingRinominaNegozio {
        negozio_id: i64,
    },
    /// Consegna B: quanto è costata questa voce.
    AwaitingPrezzo {
        voce_id: i64,
    },
    /// Consegna B: il codice a barre di quello che si è preso.
    AwaitingCodiceBarre {
        voce_id: i64,
    },
    /// Consegna B: il totale dello scontrino, dopo la chiusura.
    AwaitingTotaleSpesa {
        chiusura_id: i64,
    },
}

impl ListaSpesaSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, chat_id: i64) -> Option<ListaSpesaConversationState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&chat_id)
            .cloned()
    }

    fn set(&self, chat_id: i64, state: ListaSpesaConversationState) {
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
    /// pre-swap dell'automazione, come le altre mappe -- vedi
    /// `DistribuzioneSessionStore` in `main.rs`.
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

/// Chiave del registro delle novità (`novita::REGISTRO`): usata sia come
/// foglia (badge/tutorial di questa schermata) sia come genitore del
/// pulsante "🍽️ Alimentazione" del menù principale (`badge_alimentazione`
/// in `main.rs`), così il badge risale fino in cima, come richiede C14.
pub const NOVITA_CHIAVE: &str = "lista_spesa";

/// La foglia corrente del registro per questa schermata: dal 16 settembre
/// 2026 è la chiusura della spesa, non più `NOVITA_CHIAVE` (che resta come
/// nodo intermedio per far risalire il badge dal pulsante "🛒 Lista della
/// spesa" fino al menù principale). È questa la chiave che viene segnata
/// come vista e il cui tutorial viene mostrato: usare ancora quella vecchia
/// lascerebbe il badge acceso per sempre, perché `novita::serve_badge`
/// guarda solo le foglie.
const NOVITA_CHIAVE_CORRENTE: &str = "lista_spesa_chiusura";

/// Stato "cambio intervallo" in corso: la data di inizio già scelta, in
/// attesa della data di fine. Interno al modulo, come `PlannerDraft` in
/// `planner_alimentare.rs` -- non serve instradare testo libero per
/// questo passo (solo bottoni del calendario), quindi non serve una mappa
/// di sessione condivisa con `main.rs`.
static RANGE_DRAFTS: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();

fn range_drafts() -> &'static Mutex<HashMap<i64, String>> {
    RANGE_DRAFTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn range_set(chat_id: i64, data_inizio: String) {
    range_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(chat_id, data_inizio);
}

fn range_get(chat_id: i64) -> Option<String> {
    range_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&chat_id)
        .cloned()
}

fn range_clear(chat_id: i64) {
    range_drafts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&chat_id);
}

/// Da dove si è arrivati alla lista: "🍽️ Alimentazione" (il solito) oppure
/// il planner, che dal 16 settembre 2026 ha un suo pulsante per la lista
/// (chiesto da Alessio). Serve a `⬅️ Indietro`, che per C3 deve tornare alla
/// schermata da cui si è arrivati, non a una scelta fissa del codice.
static ORIGINI: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();

const ORIGINE_PREDEFINITA: &str = "food:menu";

fn origini() -> &'static Mutex<HashMap<i64, String>> {
    ORIGINI.get_or_init(|| Mutex::new(HashMap::new()))
}

fn origine_set(chat_id: i64, origine: &str) {
    origini()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(chat_id, origine.to_string());
}

fn origine_di(chat_id: i64) -> String {
    origini()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&chat_id)
        .cloned()
        .unwrap_or_else(|| ORIGINE_PREDEFINITA.to_string())
}

fn button(label: impl Into<String>, callback: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(label.into(), callback.into())
}

fn nav_row(back: &str) -> Vec<InlineKeyboardButton> {
    vec![
        button("⬅️ Indietro", back),
        button("🏠 Menù principale", "menu:main"),
    ]
}

fn nav_markup(back: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![nav_row(back)])
}

/// Schermata "sei sicuro?" per un'eliminazione definitiva (nuova
/// convenzione C16 di `docs/convenzioni-telegram.md`, punto 9 del collaudo
/// dell'11 settembre 2026 sul modulo Turni e routine): prima
/// `rimuovi_voce_manuale`/`rimuovi_aggiunta_catalogo` eseguivano subito al
/// primo tocco, senza nessuna conferma intermedia.
fn conferma_eliminazione_markup(
    cosa: &str,
    conferma_cb: &str,
    annulla_cb: &str,
) -> (String, InlineKeyboardMarkup) {
    let testo = format!("⚠️ Eliminare {cosa} definitivamente? Non si può recuperare.");
    let markup = InlineKeyboardMarkup::new(vec![
        vec![button("✅ Sì, elimina", conferma_cb.to_string())],
        vec![
            button("❌ Annulla", annulla_cb.to_string()),
            button("🏠 Menù principale", "menu:main"),
        ],
    ]);
    (testo, markup)
}

/// Conferma prima di chiudere la spesa. Non è un'eliminazione (le voci
/// restano nell'archivio, vedi `chiudi_spesa`), quindi non è il caso di C16
/// e il testo non promette che "non si può recuperare": dice cosa succede,
/// cioè che la lista resta con le sole voci non comprate.
fn conferma_chiusura_markup(comprate: i64) -> (String, InlineKeyboardMarkup) {
    let testo = format!(
        "🧾 Chiudere la spesa?\n\n{comprate} {} dalla lista. Le voci non comprate restano.",
        if comprate == 1 {
            "voce comprata va nell'archivio e sparisce"
        } else {
            "voci comprate vanno nell'archivio e spariscono"
        }
    );
    let markup = InlineKeyboardMarkup::new(vec![
        vec![button("✅ Sì, chiudi la spesa", "lista_spesa:close:yes")],
        vec![
            button("❌ Annulla", "lista_spesa:back"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ]);
    (testo, markup)
}

fn annulla_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", "lista_spesa:add:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]])
}

/// Come `quantita_keyboard`, ma per una voce del catalogo (18 settembre
/// 2026): anche lì si può non sapere quanta ne serve.
fn quantita_catalogo_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "➖ Senza quantità",
            "lista_spesa:add:catalogo:senza",
        )],
        vec![
            button("❌ Annulla", "lista_spesa:add:cancel"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn quantita_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("➖ Senza quantità", "lista_spesa:add:skip_qty")],
        vec![
            button("❌ Annulla", "lista_spesa:add:cancel"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

/// Scelta iniziale di "➕ Aggiungi voce manuale": cercare nel catalogo (si
/// somma o resta separata a seconda dell'identità, vedi `IdentitaCatalogo`)
/// oppure scrivere una voce libera (flusso testo-libero invariato).
fn scelta_add_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("🔎 Cerca nel catalogo", "lista_spesa:add:catalogo")],
        vec![button("📝 Voce libera", "lista_spesa:add:libera")],
        vec![
            button("❌ Annulla", "lista_spesa:add:cancel"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

fn testo_scelta_add() -> &'static str {
    "➕ Aggiungi voce manuale\n\nCerca un alimento o un prodotto nel catalogo -- si somma a quanto già serve o resta una riga a parte a seconda di cosa scegli -- oppure scrivi una voce libera."
}

fn testo_scelta_descrizione() -> &'static str {
    "📝 Voce libera\n\nScrivi la descrizione (es. \"Detersivo piatti\")."
}

fn testo_scelta_quantita(descrizione: &str) -> String {
    format!(
        "➕ {descrizione}\n\nScrivi quantità e unità (es. \"500 g\"), oppure scegli senza quantità."
    )
}

fn testo_scelta_query_catalogo() -> &'static str {
    "🔎 Cerca nel catalogo\n\nScrivi il nome di un alimento (es. \"pasta\") o di un prodotto commerciale (es. \"de cecco\")."
}

fn testo_scelta_quantita_catalogo(descrizione: &str, unita_default: Option<&str>) -> String {
    match unita_default {
        Some(unita) => format!(
            "➕ {descrizione}\n\nScrivi solo la quantità (es. \"500\") -- uso l'unità predefinita \"{unita}\", oppure scrivi anche l'unità (es. \"500 ml\") per usarne un'altra."
        ),
        None => format!("➕ {descrizione}\n\nScrivi quantità e unità (es. \"500 g\")."),
    }
}

fn risultati_catalogo_keyboard(risultati: &[RisultatoCatalogo]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = risultati
        .iter()
        .map(|risultato| {
            let callback = match risultato.identita() {
                IdentitaCatalogo::Alimento(id) => format!("lista_spesa:add:pick:alimento:{id}"),
                IdentitaCatalogo::Prodotto(id) => format!("lista_spesa:add:pick:prodotto:{id}"),
            };
            vec![button(risultato.etichetta(), callback)]
        })
        .collect();
    rows.push(vec![button("📝 Voce libera", "lista_spesa:add:libera")]);
    rows.push(vec![
        button("❌ Annulla", "lista_spesa:add:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

async fn invalid(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "⚠️ Pulsante non valido o non più disponibile.")
        .reply_markup(nav_markup("lista_spesa:back"))
        .await?;
    Ok(())
}

async fn expired(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "ℹ️ Questa operazione non è più attiva. Riapri la lista.",
    )
    .reply_markup(nav_markup("lista_spesa:back"))
    .await?;
    Ok(())
}

async fn today(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| "9999-12-31".to_string())
}

fn parse_mese(value: &str) -> Option<(i32, u32)> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[4] != b'-' {
        return None;
    }
    let anno: i32 = value[..4].parse().ok()?;
    let mese: u32 = value[5..7].parse().ok()?;
    (1..=12).contains(&mese).then_some((anno, mese))
}

fn mese_di(data: &str) -> (i32, u32) {
    let anno = data.get(..4).and_then(|v| v.parse().ok()).unwrap_or(1970);
    let mese = data.get(5..7).and_then(|v| v.parse().ok()).unwrap_or(1);
    (anno, mese)
}

/// Se, per l'utente corrente, questa è la prima vera visita alla schermata
/// -- il tutorial da anteporre secondo C14, letto ma non ancora segnato
/// come visto (lo segna `segna_vista_novita`, chiamato solo dopo aver
/// davvero mostrato la schermata).
async fn tutorial_da_mostrare(pool: &SqlitePool) -> Option<String> {
    let utente_id = crate::identity::current_actor().utente_id?;
    let viste = novita::viste_da_utente(pool, utente_id).await.ok()?;
    if novita::serve_badge(NOVITA_CHIAVE_CORRENTE, &viste) {
        novita::tutorial_per(NOVITA_CHIAVE_CORRENTE).map(str::to_string)
    } else {
        None
    }
}

async fn segna_vista_novita(pool: &SqlitePool) {
    if let Some(utente_id) = crate::identity::current_actor().utente_id {
        let _ = novita::segna_vista(pool, utente_id, NOVITA_CHIAVE_CORRENTE).await;
    }
}

/// Una foto mentre si aspetta un codice a barre (18 settembre 2026, chiesto
/// da Alessio: al supermercato si fotografa, non si copiano tredici cifre).
/// La lettura è tutta nel bot (`mercato::leggi_codice_da_jpeg`): la foto non
/// esce dal telefono.
pub async fn handle_photo(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &ListaSpesaSessionStore,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;
    let Some(ListaSpesaConversationState::AwaitingCodiceBarre { voce_id }) = sessions.get(chat_id)
    else {
        return Ok(false);
    };
    let Some(foto) = msg.photo().and_then(|formati| {
        formati
            .iter()
            .max_by_key(|f| u64::from(f.width) * u64::from(f.height))
    }) else {
        return Ok(false);
    };

    bot.send_message(msg.chat.id, "🔎 Guardo la foto…").await?;
    let dati = match scarica_foto(bot, foto.file.id.clone()).await {
        Ok(dati) => dati,
        Err(errore) => {
            tracing::warn!(?errore, "Download della foto del codice fallito");
            mostra_presa(
                bot,
                msg.chat.id,
                pool,
                sessions,
                voce_id,
                Some("⚠️ Non riesco a scaricare la foto. Riprova, oppure scrivi le cifre."),
            )
            .await?;
            return Ok(true);
        }
    };
    let letto = crate::modules::mercato::leggi_codice_da_jpeg(&dati).unwrap_or_else(|errore| {
        tracing::warn!(?errore, "Lettura del codice dalla foto fallita");
        None
    });
    match letto {
        Some(codice) => {
            sessions.clear_chat(chat_id);
            leggi_codice_a_barre(bot, msg.chat.id, pool, sessions, voce_id, &codice).await?;
        }
        None => {
            // Succede: sfocata, storta, o troppo lontana. Si dice come
            // rifarla invece di lasciare l'utente a indovinare.
            mostra_presa(
                bot,
                msg.chat.id,
                pool,
                sessions,
                voce_id,
                Some("🔎 Non sono riuscito a leggere il codice.
Riprova più da vicino, con le righe dritte e tutta l'etichetta dentro la foto — oppure scrivi le cifre."),
            )
            .await?;
        }
    }
    Ok(true)
}

/// Scarica una foto di Telegram in memoria: non serve tenerla su disco, si
/// guarda e si butta.
async fn scarica_foto(bot: &Bot, file_id: teloxide::types::FileId) -> anyhow::Result<Vec<u8>> {
    let file = bot
        .get_file(file_id)
        .await
        .context("Impossibile leggere il file da Telegram")?;
    let mut dati: Vec<u8> = Vec::new();
    bot.download_file(&file.path, &mut dati)
        .await
        .context("Download della foto fallito")?;
    Ok(dati)
}

pub async fn handle_message(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    sessions: &ListaSpesaSessionStore,
    text: &str,
) -> ResponseResult<bool> {
    let chat_id = msg.chat.id.0;

    if text.trim() == "/annulla" && sessions.get(chat_id).is_some() {
        sessions.clear_chat(chat_id);
        bot.annulla_e_avvisa(chat_id, "❌ Aggiunta annullata.");
        show_lista(bot, msg.chat.id, pool, None).await?;
        return Ok(true);
    }

    let Some(state) = sessions.get(chat_id) else {
        return Ok(false);
    };

    match state {
        ListaSpesaConversationState::AwaitingDescrizione => {
            match valida_descrizione_manuale(text) {
                Ok(descrizione) => {
                    sessions.set(
                        chat_id,
                        ListaSpesaConversationState::AwaitingQuantita {
                            descrizione: descrizione.clone(),
                        },
                    );
                    bot.send_message(msg.chat.id, testo_scelta_quantita(&descrizione))
                        .reply_markup(quantita_keyboard())
                        .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(annulla_keyboard())
                        .await?;
                }
            }
        }
        ListaSpesaConversationState::AwaitingQuantita { descrizione } => {
            match valida_quantita_manuale(text) {
                Ok((quantita, unita)) => {
                    sessions.clear_chat(chat_id);
                    salva_voce_manuale(
                        bot,
                        msg.chat.id,
                        pool,
                        &descrizione,
                        Some(quantita),
                        Some(&unita),
                    )
                    .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(quantita_keyboard())
                        .await?;
                }
            }
        }
        ListaSpesaConversationState::AwaitingCatalogoQuery => {
            let query = text.trim();
            if query.is_empty() || query.chars().count() > DESCRIZIONE_MAX_CARATTERI {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Scrivi un nome da cercare, non può essere vuoto.",
                )
                .reply_markup(annulla_keyboard())
                .await?;
                return Ok(true);
            }
            match cerca_nel_catalogo(pool, query, 10).await {
                Ok(risultati) if risultati.is_empty() => {
                    bot.send_message(
                        msg.chat.id,
                        format!("🔎 Nessun risultato per \"{query}\".\n\nProva un altro nome oppure passa a voce libera."),
                    )
                    .reply_markup(risultati_catalogo_keyboard(&[]))
                    .await?;
                }
                Ok(risultati) => {
                    bot.send_message(msg.chat.id, format!("🔎 Risultati per \"{query}\""))
                        .reply_markup(risultati_catalogo_keyboard(&risultati))
                        .await?;
                }
                Err(errore) => {
                    tracing::warn!(?errore, "Ricerca nel catalogo fallita");
                    bot.send_message(msg.chat.id, "⚠️ Non riesco a cercare nel catalogo.")
                        .reply_markup(annulla_keyboard())
                        .await?;
                }
            }
        }
        ListaSpesaConversationState::AwaitingQuantitaCatalogo {
            identita,
            descrizione,
            unita_default,
        } => match valida_quantita_con_default(text, unita_default.as_deref()) {
            Ok((quantita, unita)) => {
                sessions.clear_chat(chat_id);
                salva_voce_catalogo(
                    bot,
                    msg.chat.id,
                    pool,
                    identita,
                    &descrizione,
                    quantita,
                    &unita,
                )
                .await?;
            }
            Err(errore) => {
                bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                    .reply_markup(annulla_keyboard())
                    .await?;
            }
        },
        ListaSpesaConversationState::AwaitingNomeNegozio => {
            match crate::modules::mercato::crea_negozio(pool, text).await {
                Ok(negozio_id) => {
                    sessions.clear_chat(chat_id);
                    // Un negozio creato a mano serve subito: entra nel
                    // confronto senza doverlo anche spuntare.
                    if let Err(errore) =
                        crate::modules::mercato::cambia_scelta(pool, negozio_id).await
                    {
                        tracing::warn!(?errore, negozio_id, "Scelta del negozio nuovo fallita");
                    }
                    crate::modules::mercato::mostra_negozi(
                        bot,
                        msg.chat.id,
                        pool,
                        Some("✅ Negozio aggiunto e messo nel confronto."),
                    )
                    .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                            button("❌ Annulla", "mercato:negozi"),
                            button("🏠 Menù principale", "menu:main"),
                        ]]))
                        .await?;
                }
            }
        }
        ListaSpesaConversationState::AwaitingRinominaNegozio { negozio_id } => {
            match crate::modules::mercato::rinomina_negozio(pool, negozio_id, text).await {
                Ok(()) => {
                    sessions.clear_chat(chat_id);
                    crate::modules::mercato::mostra_negozio(
                        bot,
                        msg.chat.id,
                        pool,
                        negozio_id,
                        Some("✅ Nome cambiato."),
                    )
                    .await?;
                }
                Err(errore) => {
                    bot.send_message(msg.chat.id, format!("⚠️ {errore}"))
                        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                            button(
                                "❌ Annulla",
                                format!("mercato:negozio:gestisci:{negozio_id}"),
                            ),
                            button("🏠 Menù principale", "menu:main"),
                        ]]))
                        .await?;
                }
            }
        }
        ListaSpesaConversationState::AwaitingPrezzo { voce_id } => {
            let Some(centesimi) = crate::modules::mercato::interpreta_prezzo(text) else {
                mostra_presa(
                    bot,
                    msg.chat.id,
                    pool,
                    sessions,
                    voce_id,
                    Some("⚠️ Scrivi un prezzo, ad esempio 1,29."),
                )
                .await?;
                return Ok(true);
            };
            let negozio = trova_lista_attiva(pool)
                .await
                .ok()
                .flatten()
                .and_then(|lista| lista.negozio_id);
            let avviso = match registra_prezzo_voce(pool, voce_id, centesimi, negozio).await {
                Ok(()) => {
                    let euro = crate::modules::mercato::formatta_euro(centesimi);
                    let euro = match riferimento_voce(pool, voce_id, centesimi).await {
                        Some(riferimento) => format!("{euro} ({riferimento})"),
                        None => euro,
                    };
                    match negozio {
                        Some(_) => format!("✅ Segnato: {euro}."),
                        None => format!(
                            "✅ Segnato: {euro}.
Scegli 🏪 il negozio e i prossimi prezzi entrano anche nel confronto."
                        ),
                    }
                }
                Err(errore) => {
                    tracing::warn!(?errore, voce_id, "Registrazione prezzo fallita");
                    "⚠️ Non riesco a segnare il prezzo.".to_string()
                }
            };
            sessions.clear_chat(chat_id);
            show_lista(bot, msg.chat.id, pool, Some(&avviso)).await?;
        }
        ListaSpesaConversationState::AwaitingCodiceBarre { voce_id } => {
            sessions.clear_chat(chat_id);
            leggi_codice_a_barre(bot, msg.chat.id, pool, sessions, voce_id, text).await?;
        }
        ListaSpesaConversationState::AwaitingTotaleSpesa { chiusura_id } => {
            let Some(centesimi) = crate::modules::mercato::interpreta_prezzo(text) else {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Scrivi il totale, ad esempio 43,20. Oppure salta.",
                )
                .reply_markup(totale_keyboard())
                .await?;
                return Ok(true);
            };
            sessions.clear_chat(chat_id);
            let avviso = match registra_totale_chiusura(pool, chiusura_id, centesimi).await {
                Ok(()) => format!(
                    "✅ Totale della spesa: {}.
Quando ci sarà la sezione Soldi diventerà una spesa registrata.",
                    crate::modules::mercato::formatta_euro(centesimi)
                ),
                Err(errore) => {
                    tracing::warn!(?errore, chiusura_id, "Salvataggio totale fallito");
                    "⚠️ Non riesco a salvare il totale.".to_string()
                }
            };
            show_lista(bot, msg.chat.id, pool, Some(&avviso)).await?;
        }
        ListaSpesaConversationState::AwaitingQuantitaPresa {
            voce_id,
            unita_default,
        } => match valida_quantita_con_default(text, unita_default.as_deref()) {
            Ok((quantita, unita)) => {
                sessions.clear_chat(chat_id);
                salva_presa(bot, msg.chat.id, pool, voce_id, quantita, &unita, None).await?;
            }
            Err(errore) => {
                let avviso = format!("⚠️ {errore}");
                mostra_presa(bot, msg.chat.id, pool, sessions, voce_id, Some(&avviso)).await?;
            }
        },
    }
    Ok(true)
}

async fn salva_voce_manuale(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    descrizione: &str,
    quantita: Option<f64>,
    unita: Option<&str>,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(
                ?errore,
                "Lista della spesa non disponibile per la voce manuale"
            );
            show_lista(bot, chat_id, pool, Some("⚠️ Non riesco a salvare la voce.")).await?;
            return Ok(());
        }
    };
    match aggiungi_voce_manuale(pool, lista.id, descrizione, quantita, unita).await {
        Ok(_) => {
            show_lista(bot, chat_id, pool, Some("✅ Voce aggiunta.")).await?;
        }
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile salvare la voce manuale");
            show_lista(bot, chat_id, pool, Some("⚠️ Non riesco a salvare la voce.")).await?;
        }
    }
    Ok(())
}

/// Salva un'aggiunta dal catalogo (alimento generico o prodotto specifico)
/// e aggiorna subito la lista, così l'utente vede immediatamente la somma
/// con quanto già richiesto dal planner (per un alimento generico) o la
/// nuova riga separata (per un prodotto specifico) -- senza dover premere
/// "🔄 Aggiorna lista" a mano per vederlo.
/// Cosa dire dopo aver aggiunto qualcosa dal catalogo.
///
/// "✅ Voce aggiunta." seguito da "Nessuna voce nella lista" non si capisce:
/// la lista mostra il **netto delle scorte**, quindi chiedendo 100 g di
/// parmigiano con 290 g già in frigo non compare niente, ed è giusto — ma va
/// detto (Alessio, collaudo del 23 settembre 2026, punto 9).
async fn spiega_aggiunta(
    pool: &SqlitePool,
    lista_id: i64,
    identita: IdentitaCatalogo,
    quantita: f64,
    unita: &str,
) -> String {
    let voci = carica_voci(pool, lista_id).await.unwrap_or_default();
    // **Tutte** le righe di quell'alimento, non la prima: aggiungendo due
    // volte lo stesso alimento la lista mostra due righe, e leggendone una
    // sola il bot diceva "ne restano 10 g" mentre sotto se ne vedevano 210
    // (Alessio, collaudo del 24 settembre 2026, punto C1).
    let in_lista: Option<f64> = voci
        .iter()
        .filter(|voce| match identita {
            IdentitaCatalogo::Alimento(id) => voce.alimento_id == Some(id),
            IdentitaCatalogo::Prodotto(id) => voce.prodotto_alimentare_id == Some(id),
        })
        .filter(|voce| voce.unita_simbolo.as_deref() == Some(unita))
        .try_fold(0.0_f64, |somma, voce| Some(somma + voce.quantita?))
        .filter(|totale| *totale > 0.0);
    match in_lista {
        // C18: un testo mostrato all'utente sta su una riga sola nel codice,
        // per quanto lunga.
        None => "✅ Aggiunta, ma in casa ne hai già abbastanza: per ora non c'è niente da comprare.\nComparirà in lista appena te ne servirà davvero.".to_string(),
        Some(rimasta) if rimasta + 1e-9 < quantita => format!(
            "✅ Aggiunta: servivano {} {unita}, in lista ne restano {} {unita}.\nIl resto ce l'hai già in casa.",
            formatta_quantita(quantita),
            formatta_quantita(rimasta)
        ),
        Some(_) => "✅ Voce aggiunta.".to_string(),
    }
}

async fn salva_voce_catalogo(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    identita: IdentitaCatalogo,
    descrizione: &str,
    quantita: f64,
    unita: &str,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(
                ?errore,
                "Lista della spesa non disponibile per l'aggiunta dal catalogo"
            );
            show_lista(bot, chat_id, pool, Some("⚠️ Non riesco a salvare la voce.")).await?;
            return Ok(());
        }
    };
    match aggiungi_da_catalogo(pool, lista.id, identita, descrizione, quantita, unita).await {
        Ok(_) => {
            if let Err(errore) = aggiorna_e_registra(pool, &lista, true).await {
                tracing::warn!(
                    ?errore,
                    "Aggiornamento lista dopo aggiunta catalogo fallito"
                );
                show_lista(
                    bot,
                    chat_id,
                    pool,
                    Some("✅ Voce aggiunta, ma non sono riuscito ad aggiornare subito la lista. Premi 🔄 Aggiorna lista."),
                )
                .await?;
                return Ok(());
            }
            let avviso = spiega_aggiunta(pool, lista.id, identita, quantita, unita).await;
            show_lista(bot, chat_id, pool, Some(&avviso)).await?;
        }
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile salvare l'aggiunta dal catalogo");
            show_lista(bot, chat_id, pool, Some("⚠️ Non riesco a salvare la voce.")).await?;
        }
    }
    Ok(())
}

pub async fn handle_callback(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ListaSpesaSessionStore,
    data: &str,
) -> ResponseResult<bool> {
    if data == "lista_spesa:noop" {
        return Ok(true);
    }
    if data == "lista_spesa:menu" {
        sessions.clear_chat(chat_id.0);
        range_clear(chat_id.0);
        // Ingresso dal menù "🍽️ Alimentazione": è lì che deve tornare
        // `⬅️ Indietro` finché non si entra da un'altra strada.
        origine_set(chat_id.0, ORIGINE_PREDEFINITA);
        show_lista(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    // Ingresso dal planner (16 settembre 2026): stessa schermata, ma
    // `⬅️ Indietro` torna al planner -- C3 vuole che riporti dove si era, non
    // in un posto scelto dal codice.
    if data == "lista_spesa:menu:planner" {
        sessions.clear_chat(chat_id.0);
        range_clear(chat_id.0);
        origine_set(chat_id.0, "planner:menu");
        show_lista(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    // Ritorno alla lista da una sua schermata interna (riordino, rimozione,
    // archivio, calendario): non tocca la provenienza, altrimenti un giro
    // dentro la lista farebbe dimenticare di essere arrivati dal planner.
    if data == "lista_spesa:back" {
        sessions.clear_chat(chat_id.0);
        range_clear(chat_id.0);
        show_lista(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    if data == "lista_spesa:close:ask" {
        let lista = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => lista,
            Err(errore) => {
                tracing::warn!(?errore, "Lista non disponibile per la chiusura");
                invalid(bot, chat_id).await?;
                return Ok(true);
            }
        };
        let comprate = conta_comprate(pool, lista.id).await.unwrap_or(0);
        if comprate == 0 {
            show_lista(
                bot,
                chat_id,
                pool,
                Some("ℹ️ Non c'è ancora niente di comprato da archiviare."),
            )
            .await?;
            return Ok(true);
        }
        let (testo, markup) = conferma_chiusura_markup(comprate);
        bot.send_message(chat_id, testo)
            .reply_markup(markup)
            .await?;
        return Ok(true);
    }
    if data == "lista_spesa:close:yes" {
        let lista = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => lista,
            Err(errore) => {
                tracing::warn!(?errore, "Lista non disponibile per la chiusura");
                invalid(bot, chat_id).await?;
                return Ok(true);
            }
        };
        match chiudi_spesa(pool, &lista).await {
            Ok(esito) if esito.archiviate == 0 => {
                show_lista(
                    bot,
                    chat_id,
                    pool,
                    Some("ℹ️ Non c'è ancora niente di comprato da archiviare."),
                )
                .await?;
            }
            Ok(esito) => {
                let numero = esito.archiviate;
                // Tolte le voci comprate, il fabbisogno si ricalcola subito:
                // quello che è entrato in casa viene sottratto
                // (`sottrai_scorte`), quindi non ricompare.
                if let Err(errore) = aggiorna_e_registra(pool, &lista, true).await {
                    tracing::warn!(?errore, "Aggiornamento lista dopo la chiusura fallito");
                }
                let mut messaggio = format!(
                    "✅ Spesa chiusa: {numero} {} nell'archivio.",
                    if numero == 1 { "voce" } else { "voci" }
                );
                if let Some(entrate) = crate::modules::dispensa::riepilogo_ingresso(&esito.entrate)
                {
                    messaggio.push_str(&format!("\nEntrate in casa: {entrate}."));
                }
                if let Some(speso) = esito.speso_centesimi {
                    messaggio.push_str(&format!(
                        "\nPrezzi segnati: {}.",
                        crate::modules::mercato::formatta_euro(speso)
                    ));
                }
                // Di qualcosa si è comprato meno di quanto si era chiesto:
                // prima si decide che farne, poi si chiede il totale.
                if !esito.ridotte.is_empty() {
                    messaggio.push_str("\n\n🧺 Di questo ne avevi chiesto di più:");
                    for aggiunta in &esito.ridotte {
                        messaggio.push_str(&format!(
                            "\n• {} — restano {} {}",
                            aggiunta.nome,
                            formatta_quantita(aggiunta.quantita),
                            aggiunta.unita_simbolo
                        ));
                    }
                    messaggio.push_str("\n\nLe lascio in lista?");
                    bot.send_message(chat_id, messaggio)
                        .reply_markup(residuo_keyboard(esito.chiusura_id))
                        .await?;
                    return Ok(true);
                }
                chiedi_totale(bot, chat_id, sessions, esito.chiusura_id, messaggio).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, "Chiusura della spesa fallita");
                show_lista(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Non riesco a chiudere la spesa."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if let Some(resto) = data.strip_prefix("lista_spesa:resto:") {
        // "Le lascio in lista?" dopo una chiusura: le aggiunte ridotte sono
        // quelle marcate con questa chiusura, e nessun'altra.
        let (scelta, chiusura) = resto.split_once(':').unwrap_or(("keep", resto));
        let Ok(chiusura_id) = chiusura.parse::<i64>() else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let messaggio = if scelta == "drop" {
            match togli_aggiunte_ridotte(pool, chiusura_id).await {
                Ok(_) => {
                    if let Ok(lista) = trova_o_crea_lista_attiva(pool).await {
                        if let Err(errore) = aggiorna_e_registra(pool, &lista, true).await {
                            tracing::warn!(?errore, "Aggiornamento dopo il residuo fallito");
                        }
                    }
                    "🗑️ Tolte dalla lista.".to_string()
                }
                Err(errore) => {
                    tracing::warn!(?errore, "Rimozione delle aggiunte ridotte fallita");
                    "⚠️ Non riesco a toglierle: sono rimaste in lista.".to_string()
                }
            }
        } else {
            "✅ Restano in lista con quel che manca.".to_string()
        };
        chiedi_totale(bot, chat_id, sessions, chiusura_id, messaggio).await?;
        return Ok(true);
    }
    if data == "lista_spesa:archivio" {
        mostra_ultima_chiusura(bot, chat_id, pool).await?;
        return Ok(true);
    }
    if data == "lista_spesa:reorder" {
        show_lista_riordina(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:reorder:up:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        muovi_e_mostra_riordino(bot, chat_id, pool, voce_id, Direzione::Su).await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:reorder:down:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        muovi_e_mostra_riordino(bot, chat_id, pool, voce_id, Direzione::Giu).await?;
        return Ok(true);
    }
    if data == "lista_spesa:remove" {
        show_lista_rimuovi(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    // Punto 9 del collaudo dell'11 settembre 2026 / C16: conferma esplicita
    // prima di un'eliminazione definitiva -- prima si eseguiva subito al
    // primo tocco.
    if data == "lista_spesa:remove:all:ask" {
        let quante = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => voci_rimovibili(pool, lista.id)
                .await
                .map(|voci| voci.len())
                .unwrap_or(0),
            Err(_) => 0,
        };
        let (_, markup) =
            conferma_eliminazione_markup("", "lista_spesa:remove:all:yes", "lista_spesa:remove");
        // C16, con il conto: si vede quante voci spariscono, e che quelle
        // dei pasti restano.
        bot.send_message(
            chat_id,
            format!("⚠️ Eliminare tutte le {quante} voci aggiunte a mano o dal catalogo? Non si può recuperare.\n\nLe voci dei pasti pianificati restano."),
        )
        .reply_markup(markup)
        .await?;
        return Ok(true);
    }
    if data == "lista_spesa:remove:all:yes" {
        let esito = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => match rimuovi_tutte_le_aggiunte(pool, lista.id).await {
                Ok(quante) => {
                    // Le aggiunte dal catalogo contribuivano al fabbisogno:
                    // si ricalcola subito, come per la rimozione di una sola.
                    if let Err(errore) = aggiorna_e_registra(pool, &lista, true).await {
                        tracing::warn!(?errore, "Aggiornamento dopo la rimozione di tutte fallito");
                    }
                    Ok(quante)
                }
                Err(errore) => Err(errore),
            },
            Err(errore) => Err(errore),
        };
        match esito {
            Ok(quante) => {
                let avviso = format!(
                    "✅ Tolte {quante} {}.",
                    if quante == 1 { "voce" } else { "voci" }
                );
                show_lista(bot, chat_id, pool, Some(&avviso)).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, "Rimozione di tutte le voci fallita");
                show_lista_rimuovi(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Non riesco a togliere le voci."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:remove:ask:manuale:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let (testo, markup) = conferma_eliminazione_markup(
            "questa voce",
            &format!("lista_spesa:remove:yes:manuale:{voce_id}"),
            "lista_spesa:remove",
        );
        bot.send_message(chat_id, testo)
            .reply_markup(markup)
            .await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:remove:ask:catalogo:") {
        let Some(aggiunta_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let (testo, markup) = conferma_eliminazione_markup(
            "questa voce",
            &format!("lista_spesa:remove:yes:catalogo:{aggiunta_id}"),
            "lista_spesa:remove",
        );
        bot.send_message(chat_id, testo)
            .reply_markup(markup)
            .await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:remove:yes:manuale:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match rimuovi_voce_manuale(pool, voce_id).await {
            Ok(()) => {
                mostra_dopo_rimozione(bot, chat_id, pool, "✅ Voce eliminata.").await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, voce_id, "Rimozione voce manuale fallita");
                show_lista_rimuovi(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Non riesco a rimuovere la voce."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:remove:yes:catalogo:") {
        let Some(aggiunta_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match rimuovi_aggiunta_catalogo(pool, aggiunta_id).await {
            Ok(()) => {
                // L'aggiunta non è più una fonte per il fresco: si
                // ricalcola subito, così una riga generata dal catalogo si
                // riduce o sparisce senza dover premere "🔄 Aggiorna
                // lista" a mano -- stesso trattamento già riservato a
                // un'aggiunta appena inserita (`salva_voce_catalogo`).
                let esito_refresh = match trova_o_crea_lista_attiva(pool).await {
                    Ok(lista) => aggiorna_e_registra(pool, &lista, true).await.map(|_| ()),
                    Err(errore) => Err(errore),
                };
                if let Err(errore) = esito_refresh {
                    tracing::warn!(
                        ?errore,
                        aggiunta_id,
                        "Aggiornamento lista dopo rimozione aggiunta catalogo fallito"
                    );
                }
                mostra_dopo_rimozione(bot, chat_id, pool, "✅ Voce eliminata.").await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, aggiunta_id, "Rimozione aggiunta catalogo fallita");
                show_lista_rimuovi(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Non riesco a rimuovere la voce."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if data == "lista_spesa:negozio" {
        let attuale = trova_lista_attiva(pool)
            .await
            .ok()
            .flatten()
            .and_then(|lista| lista.negozio_id);
        crate::modules::mercato::mostra_scelta_negozio_spesa(bot, chat_id, pool, attuale).await?;
        return Ok(true);
    }
    if data == "lista_spesa:negozio:nuovo" {
        attendi_nome_negozio(bot, chat_id, sessions).await?;
        return Ok(true);
    }
    // La sessione di testo è qui, quindi anche la rinomina passa da questo
    // modulo anche se la schermata del negozio sta in `mercato`.
    if let Some(raw) = data.strip_prefix("lista_spesa:negozio:rinomina:") {
        let Some(negozio_id) = raw.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            ListaSpesaConversationState::AwaitingRinominaNegozio { negozio_id },
        );
        bot.send_message(chat_id, "✏️ Scrivi il nome nuovo del negozio.")
            .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                button(
                    "❌ Annulla",
                    format!("mercato:negozio:gestisci:{negozio_id}"),
                ),
                button("🏠 Menù principale", "menu:main"),
            ]]))
            .await?;
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("lista_spesa:negozio:set:") {
        let Some(negozio_id) = raw.parse::<i64>().ok() else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let lista = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => lista,
            Err(errore) => {
                tracing::warn!(?errore, "Lista non disponibile per il negozio");
                invalid(bot, chat_id).await?;
                return Ok(true);
            }
        };
        let scelto = (negozio_id > 0).then_some(negozio_id);
        let avviso = match imposta_negozio_lista(pool, lista.id, scelto).await {
            Ok(()) => match scelto {
                Some(id) => match crate::modules::mercato::negozio_per_id(pool, id).await {
                    Ok(Some(negozio)) => format!("🏪 Spesa da {}.", negozio.nome),
                    _ => "🏪 Negozio scelto.".to_string(),
                },
                None => "➖ Nessun negozio per questa spesa.".to_string(),
            },
            Err(errore) => {
                tracing::warn!(?errore, "Scelta del negozio fallita");
                "⚠️ Non riesco a scegliere il negozio.".to_string()
            }
        };
        show_lista(bot, chat_id, pool, Some(&avviso)).await?;
        return Ok(true);
    }
    if data == "lista_spesa:conviene" {
        let lista = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => lista,
            Err(errore) => {
                tracing::warn!(?errore, "Lista non disponibile per il confronto");
                invalid(bot, chat_id).await?;
                return Ok(true);
            }
        };
        let voci = carica_voci(pool, lista.id).await.unwrap_or_default();
        let (stime, senza_prezzo) = stime_dei_negozi(pool, &voci)
            .await
            .unwrap_or_else(|errore| {
                tracing::warn!(?errore, "Confronto fra negozi fallito");
                (Vec::new(), voci.len())
            });
        crate::modules::mercato::mostra_confronto(bot, chat_id, &stime, senza_prezzo).await?;
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("lista_spesa:prezzo:") {
        let Some(voce_id) = raw.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            ListaSpesaConversationState::AwaitingPrezzo { voce_id },
        );
        bot.send_message(
            chat_id,
            "💶 Quanto è costato?

Scrivi il prezzo pagato, ad esempio 1,29.",
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button("❌ Annulla", "lista_spesa:back"),
            button("🏠 Menù principale", "menu:main"),
        ]]))
        .await?;
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("lista_spesa:ean:") {
        let Some(voce_id) = raw.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.set(
            chat_id.0,
            ListaSpesaConversationState::AwaitingCodiceBarre { voce_id },
        );
        bot.send_message(
            chat_id,
            "🏷 Fotografa il codice a barre della confezione, oppure scrivi le cifre sotto le righe nere.

Lo leggo io, poi lo cerco su Open Food Facts e segno la confezione che hai preso.",
        )
        .reply_markup(InlineKeyboardMarkup::new(vec![vec![
            button("❌ Annulla", "lista_spesa:back"),
            button("🏠 Menù principale", "menu:main"),
        ]]))
        .await?;
        return Ok(true);
    }
    if data == "lista_spesa:totale:salta" {
        sessions.clear_chat(chat_id.0);
        show_lista(
            bot,
            chat_id,
            pool,
            Some("✅ Spesa chiusa senza totale: va benissimo."),
        )
        .await?;
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("lista_spesa:pref:") {
        let parti: Vec<&str> = raw.split(':').collect();
        let [voce, alimento, prodotto] = parti[..] else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let (Some(voce_id), Some(alimento_id), Some(prodotto_id)) = (
            voce.parse::<i64>().ok(),
            alimento.parse::<i64>().ok(),
            prodotto.parse::<i64>().ok(),
        ) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        let avviso =
            match crate::modules::mercato::cambia_preferito(pool, alimento_id, prodotto_id).await {
                Ok(true) => "⭐ Preferito: te lo propongo per primo.",
                Ok(false) => "➖ Non è più il preferito.",
                Err(errore) => {
                    tracing::warn!(?errore, "Cambio preferito fallito");
                    "⚠️ Non riesco a segnare il preferito."
                }
            };
        mostra_presa(bot, chat_id, pool, sessions, voce_id, Some(avviso)).await?;
        return Ok(true);
    }
    if let Some(resto) = data.strip_prefix("lista_spesa:presa:f:") {
        let Some((voce_id, token)) = resto
            .split_once(':')
            .and_then(|(voce, token)| Some((voce.parse::<i64>().ok()?, token)))
        else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        match confezione_da_token(pool, token).await {
            Ok(Some((prodotto_id, quantita, unita))) => {
                salva_presa(
                    bot,
                    chat_id,
                    pool,
                    voce_id,
                    quantita,
                    &unita,
                    Some(prodotto_id),
                )
                .await?;
            }
            Ok(None) => invalid(bot, chat_id).await?,
            Err(errore) => {
                tracing::warn!(?errore, voce_id, "Lettura confezione fallita");
                mostra_presa(
                    bot,
                    chat_id,
                    pool,
                    sessions,
                    voce_id,
                    Some("⚠️ Non riesco a leggere questa confezione."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:presa:reset:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        let avviso = match annulla_presa(pool, voce_id).await {
            Ok(()) => "✅ Torna a contare la quantità in lista.",
            Err(errore) => {
                tracing::warn!(?errore, voce_id, "Annullamento presa fallito");
                "⚠️ Non riesco ad aggiornare questa voce."
            }
        };
        show_lista(bot, chat_id, pool, Some(avviso)).await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:presa:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        mostra_presa(bot, chat_id, pool, sessions, voce_id, None).await?;
        return Ok(true);
    }
    if let Some(raw_id) = data.strip_prefix("lista_spesa:toggle:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match toggle_comprato(pool, voce_id).await {
            Ok(()) => {
                show_lista(bot, chat_id, pool, None).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, voce_id, "Toggle voce lista spesa fallito");
                show_lista(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Non riesco ad aggiornare questa voce."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if data == "lista_spesa:legenda" {
        liste::cambia_legenda(pool).await;
        show_lista(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    if data == "lista_spesa:auto" {
        let attuale = aggiornamento_automatico(pool).await;
        if let Err(errore) = imposta_aggiornamento_automatico(pool, !attuale).await {
            tracing::warn!(
                ?errore,
                "Salvataggio preferenza aggiornamento automatico fallito"
            );
        }
        let avviso = if attuale {
            "✅ Da ora la lista cambia solo quando premi 🔄 Aggiorna lista."
        } else {
            "✅ Da ora la lista si aggiorna da sola quando la apri, e ti dice cosa ha cambiato."
        };
        show_lista(bot, chat_id, pool, Some(avviso)).await?;
        return Ok(true);
    }
    if data == "lista_spesa:modifiche" {
        mostra_modifiche(bot, chat_id, pool).await?;
        return Ok(true);
    }
    if data == "lista_spesa:refresh" {
        let esito = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => aggiorna_e_registra(pool, &lista, false).await,
            Err(errore) => Err(errore),
        };
        match esito {
            Ok(modifiche) => {
                let messaggio = if modifiche.is_empty() {
                    "🔄 Lista già aggiornata.".to_string()
                } else {
                    format!("🔄 Lista aggiornata: {}.", riepilogo_modifiche(&modifiche))
                };
                show_lista(bot, chat_id, pool, Some(&messaggio)).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, "Aggiornamento lista spesa fallito");
                show_lista(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Non riesco ad aggiornare la lista."),
                )
                .await?;
            }
        }
        return Ok(true);
    }
    if data == "lista_spesa:add" {
        sessions.clear_chat(chat_id.0);
        bot.send_message(chat_id, testo_scelta_add())
            .reply_markup(scelta_add_keyboard())
            .await?;
        return Ok(true);
    }
    if data == "lista_spesa:add:libera" {
        sessions.set(chat_id.0, ListaSpesaConversationState::AwaitingDescrizione);
        bot.send_message(chat_id, testo_scelta_descrizione())
            .reply_markup(annulla_keyboard())
            .await?;
        return Ok(true);
    }
    if data == "lista_spesa:add:catalogo" {
        sessions.set(
            chat_id.0,
            ListaSpesaConversationState::AwaitingCatalogoQuery,
        );
        bot.send_message(chat_id, testo_scelta_query_catalogo())
            .reply_markup(annulla_keyboard())
            .await?;
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("lista_spesa:add:pick:alimento:") {
        let Some(id) = raw.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match alimento_visibile_per_id(pool, id).await {
            Ok(Some((nome, unita_default))) => {
                sessions.set(
                    chat_id.0,
                    ListaSpesaConversationState::AwaitingQuantitaCatalogo {
                        identita: IdentitaCatalogo::Alimento(id),
                        descrizione: nome.clone(),
                        unita_default: unita_default.clone(),
                    },
                );
                bot.send_message(
                    chat_id,
                    testo_scelta_quantita_catalogo(&nome, unita_default.as_deref()),
                )
                .reply_markup(quantita_catalogo_keyboard())
                .await?;
            }
            Ok(None) => {
                invalid(bot, chat_id).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, id, "Alimento del catalogo non rileggibile");
                invalid(bot, chat_id).await?;
            }
        }
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("lista_spesa:add:pick:prodotto:") {
        let Some(id) = raw.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match prodotto_visibile_per_id(pool, id).await {
            Ok(Some((marca, nome_commerciale, unita_default))) => {
                let descrizione = format!("{marca} {nome_commerciale}");
                sessions.set(
                    chat_id.0,
                    ListaSpesaConversationState::AwaitingQuantitaCatalogo {
                        identita: IdentitaCatalogo::Prodotto(id),
                        descrizione: descrizione.clone(),
                        unita_default: Some(unita_default.clone()),
                    },
                );
                bot.send_message(
                    chat_id,
                    testo_scelta_quantita_catalogo(&descrizione, Some(&unita_default)),
                )
                .reply_markup(quantita_catalogo_keyboard())
                .await?;
            }
            Ok(None) => {
                invalid(bot, chat_id).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, id, "Prodotto del catalogo non rileggibile");
                invalid(bot, chat_id).await?;
            }
        }
        return Ok(true);
    }
    if data == "lista_spesa:add:catalogo:senza" {
        let Some(ListaSpesaConversationState::AwaitingQuantitaCatalogo {
            identita,
            descrizione,
            ..
        }) = sessions.get(chat_id.0)
        else {
            expired(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        let avviso = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => {
                match aggiungi_catalogo_senza_quantita(pool, lista.id, identita, &descrizione)
                    .await
                {
                    Ok(_) => "✅ Voce aggiunta senza quantità.\nNon entra nelle scorte, a meno che con 📦 non segni quanto ne hai preso.".to_string(),
                    Err(errore) => {
                        tracing::warn!(?errore, "Aggiunta dal catalogo senza quantità fallita");
                        "⚠️ Non riesco a salvare la voce.".to_string()
                    }
                }
            }
            Err(errore) => {
                tracing::warn!(?errore, "Lista non disponibile");
                "⚠️ Non riesco a salvare la voce.".to_string()
            }
        };
        show_lista(bot, chat_id, pool, Some(&avviso)).await?;
        return Ok(true);
    }
    if data == "lista_spesa:add:cancel" {
        sessions.clear_chat(chat_id.0);
        bot.annulla_e_avvisa(chat_id.0, "❌ Aggiunta annullata.");
        show_lista(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    if data == "lista_spesa:add:skip_qty" {
        let Some(ListaSpesaConversationState::AwaitingQuantita { descrizione }) =
            sessions.get(chat_id.0)
        else {
            expired(bot, chat_id).await?;
            return Ok(true);
        };
        sessions.clear_chat(chat_id.0);
        salva_voce_manuale(bot, chat_id, pool, &descrizione, None, None).await?;
        return Ok(true);
    }
    if data == "lista_spesa:range:start" {
        range_clear(chat_id.0);
        let oggi = today(pool).await;
        let (anno, mese) = mese_di(&oggi);
        mostra_calendario_inizio(bot, chat_id, pool, anno, mese).await?;
        return Ok(true);
    }
    if let Some(mese_testo) = data.strip_prefix("lista_spesa:cal:start:") {
        match parse_mese(mese_testo) {
            Some((anno, mese)) => {
                mostra_calendario_inizio(bot, chat_id, pool, anno, mese).await?;
            }
            None => invalid(bot, chat_id).await?,
        }
        return Ok(true);
    }
    if let Some(mese_testo) = data.strip_prefix("lista_spesa:cal:end:") {
        let Some(inizio) = range_get(chat_id.0) else {
            expired(bot, chat_id).await?;
            return Ok(true);
        };
        match parse_mese(mese_testo) {
            Some((anno, mese)) => {
                mostra_calendario_fine(bot, chat_id, pool, anno, mese, &inizio).await?;
            }
            None => invalid(bot, chat_id).await?,
        }
        return Ok(true);
    }
    if let Some(date) = data.strip_prefix("lista_spesa:range:day:start:") {
        if !calendario::valid_date(date) {
            invalid(bot, chat_id).await?;
            return Ok(true);
        }
        // Un inizio precedente a oggi si segnala subito, al tocco, con la
        // possibilità di tenerlo o cambiarlo (chiesto da Alessio il 16
        // settembre 2026): prima l'avviso arrivava solo alla fine, dopo aver
        // scelto anche la data di fine, quando era ormai cosa fatta.
        let oggi = today(pool).await;
        if date < oggi.as_str() {
            bot.send_message(
                chat_id,
                format!(
                    "⚠️ {} è prima di oggi.\n\nLa lista resterà ferma su questo inizio finché non lo cambi, invece di spostarsi in avanti da sola.",
                    calendario::display_date(date)
                ),
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![button(
                    "✅ Tienila",
                    format!("lista_spesa:range:keep:{date}"),
                )],
                vec![button("📅 Scegli un'altra data", "lista_spesa:range:start")],
                nav_row("lista_spesa:back"),
            ]))
            .await?;
            return Ok(true);
        }
        range_set(chat_id.0, date.to_string());
        let (anno, mese) = mese_di(date);
        mostra_calendario_fine(bot, chat_id, pool, anno, mese, date).await?;
        return Ok(true);
    }
    if let Some(date) = data.strip_prefix("lista_spesa:range:keep:") {
        if !calendario::valid_date(date) {
            invalid(bot, chat_id).await?;
            return Ok(true);
        }
        range_set(chat_id.0, date.to_string());
        let (anno, mese) = mese_di(date);
        mostra_calendario_fine(bot, chat_id, pool, anno, mese, date).await?;
        return Ok(true);
    }
    if let Some(date) = data.strip_prefix("lista_spesa:range:day:end:") {
        if !calendario::valid_date(date) {
            invalid(bot, chat_id).await?;
            return Ok(true);
        }
        let Some(inizio) = range_get(chat_id.0) else {
            expired(bot, chat_id).await?;
            return Ok(true);
        };
        if date < inizio.as_str() {
            invalid(bot, chat_id).await?;
            return Ok(true);
        }
        let lista = match trova_o_crea_lista_attiva(pool).await {
            Ok(lista) => lista,
            Err(errore) => {
                tracing::warn!(
                    ?errore,
                    "Lista della spesa non disponibile per il cambio intervallo"
                );
                invalid(bot, chat_id).await?;
                return Ok(true);
            }
        };
        range_clear(chat_id.0);
        // Un inizio precedente a oggi è permesso (può servire a recuperare i
        // pasti di ieri), ma è una scelta esplicita, già confermata con
        // "✅ Tienila": da qui la lista smette di spostarsi in avanti da sola.
        // L'avviso non si ripete qui -- resta la riga fissa in testa alla
        // lista, che spiega perché non si sposta più.
        let oggi = today(pool).await;
        let inizio_nel_passato = inizio.as_str() < oggi.as_str();
        match cambia_intervallo(pool, lista.id, &inizio, date, inizio_nel_passato).await {
            Ok(()) => {
                show_lista(bot, chat_id, pool, Some("🗓️ Intervallo aggiornato.")).await?;
            }
            Err(errore) => {
                tracing::warn!(?errore, "Cambio intervallo lista spesa fallito");
                show_lista(
                    bot,
                    chat_id,
                    pool,
                    Some("⚠️ Non riesco a cambiare l'intervallo."),
                )
                .await?;
            }
        }
        return Ok(true);
    }

    Ok(false)
}

async fn mostra_calendario_inizio(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    year: i32,
    month: u32,
) -> ResponseResult<()> {
    let oggi = today(pool).await;
    let giorno = |_: &str| calendario::Giorno::default();
    let callback_giorno = |data: &str| format!("lista_spesa:range:day:start:{data}");
    let callback_mese = |anno: i32, mese: u32| format!("lista_spesa:cal:start:{anno:04}-{mese:02}");
    let config = calendario::Calendario {
        year,
        month,
        oggi: &oggi,
        callback_giorno: &callback_giorno,
        callback_mese: &callback_mese,
        callback_inerte: "lista_spesa:noop",
        giorno: &giorno,
        mese_minimo: None,
    };
    let mut rows = calendario::righe(&config);
    rows.push(nav_row("lista_spesa:back"));

    bot.send_message(chat_id, "🗓️ Cambia intervallo\n\nScegli la data di inizio.")
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn mostra_calendario_fine(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    year: i32,
    month: u32,
    inizio: &str,
) -> ResponseResult<()> {
    let oggi = today(pool).await;
    let inizio_owned = inizio.to_string();
    let giorno = |data: &str| calendario::Giorno {
        stato: if data < inizio_owned.as_str() {
            calendario::GiornoStato::Bloccato
        } else {
            calendario::GiornoStato::Libero
        },
        marcatore: None,
    };
    let callback_giorno = |data: &str| format!("lista_spesa:range:day:end:{data}");
    let callback_mese = |anno: i32, mese: u32| format!("lista_spesa:cal:end:{anno:04}-{mese:02}");
    let (anno_minimo, mese_minimo) = mese_di(inizio);
    let config = calendario::Calendario {
        year,
        month,
        oggi: &oggi,
        callback_giorno: &callback_giorno,
        callback_mese: &callback_mese,
        callback_inerte: "lista_spesa:noop",
        giorno: &giorno,
        mese_minimo: Some((anno_minimo, mese_minimo)),
    };
    let mut rows = calendario::righe(&config);
    rows.push(nav_row("lista_spesa:range:start"));

    bot.send_message(
        chat_id,
        format!(
            "🗓️ Cambia intervallo\n\nInizio: {}\nScegli la data di fine.",
            calendario::display_date(inizio)
        ),
    )
    .reply_markup(InlineKeyboardMarkup::new(rows))
    .await?;
    Ok(())
}

/// Riga di una voce in modalità riordino: sempre tre pulsanti nello stesso
/// ordine (freccia su, etichetta, freccia giù), mai due o uno -- prima le
/// frecce assenti alle estremità spostavano l'etichetta di colonna riga per
/// riga, un disallineamento visto da Alessio dal vivo. Alle estremità la
/// freccia resta premibile ma non fa nulla (`lista_spesa:noop`), stesso
/// trattamento già riservato al contatore di pagina non premibile altrove
/// nel bot.
fn riordina_row(
    voce: &VoceListaSpesa,
    posizione: usize,
    totale: usize,
) -> Vec<InlineKeyboardButton> {
    let callback_su = if posizione > 0 {
        format!("lista_spesa:reorder:up:{}", voce.id)
    } else {
        "lista_spesa:noop".to_string()
    };
    let callback_giu = if posizione + 1 < totale {
        format!("lista_spesa:reorder:down:{}", voce.id)
    } else {
        "lista_spesa:noop".to_string()
    };
    vec![
        button("⬆️", callback_su),
        button(liste::tronca(&voce.descrizione, 30), "lista_spesa:noop"),
        button("⬇️", callback_giu),
    ]
}

/// Modalità dedicata per riordinare la lista a piacere (deciso con Alessio
/// il 9 settembre 2026, dopo aver visto dal vivo che spuntare una voce la
/// faceva saltare in fondo): fuori da questa modalità l'ordine è ora
/// sempre lo stesso, indipendente da `comprato` (`carica_voci`); qui ogni
/// voce si sposta su o giù con `sposta_voce`, indipendentemente dal fatto
/// che sia comprata o meno.
async fn show_lista_riordina(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            bot.send_message(chat_id, "⚠️ Non riesco ad aprire la lista della spesa.")
                .reply_markup(nav_markup(&origine_di(chat_id.0)))
                .await?;
            return Ok(());
        }
    };
    let voci = carica_voci(pool, lista.id).await.unwrap_or_default();

    let mut testo = String::new();
    if let Some(notice) = notice {
        testo.push_str(notice);
        testo.push_str("\n\n");
    }
    testo.push_str("↕️ Riordina lista\n\nSposta le voci su o giù con le frecce.");

    let totale = voci.len();
    let mut rows: Vec<Vec<InlineKeyboardButton>> = voci
        .iter()
        .enumerate()
        .map(|(indice, voce)| riordina_row(voce, indice, totale))
        .collect();
    rows.push(vec![button("✅ Fine riordino", "lista_spesa:back")]);

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn muovi_e_mostra_riordino(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    voce_id: i64,
    direzione: Direzione,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            show_lista_riordina(
                bot,
                chat_id,
                pool,
                Some("⚠️ Non riesco a spostare la voce."),
            )
            .await?;
            return Ok(());
        }
    };
    if let Err(errore) = sposta_voce(pool, lista.id, voce_id, direzione).await {
        tracing::warn!(?errore, voce_id, "Spostamento voce lista spesa fallito");
        show_lista_riordina(
            bot,
            chat_id,
            pool,
            Some("⚠️ Non riesco a spostare la voce."),
        )
        .await?;
        return Ok(());
    }
    show_lista_riordina(bot, chat_id, pool, None).await
}

/// Etichetta di una voce rimovibile: `🗑️ descrizione · quantità unità`, o
/// senza quantità per una voce manuale libera che non ne ha (ammesso, vedi
/// `aggiungi_voce_manuale`).
fn etichetta_rimovibile(voce: &VoceRimovibile) -> String {
    let quantita = match (voce.quantita, &voce.unita_simbolo) {
        (Some(valore), Some(unita)) => format!(" · {} {unita}", formatta_quantita(valore)),
        _ => String::new(),
    };
    format!("🗑️ {}{quantita}", liste::tronca(&voce.descrizione, 40))
}

/// Modalità dedicata per rimuovere una voce manuale o un'aggiunta dal
/// catalogo (chiesto da Alessio dopo un collaudo dal vivo: prima non era
/// possibile rimuovere né l'una né l'altra). Le righe `generato`
/// pure-planner non compaiono: le gestisce il planner, non una rimozione
/// manuale.
/// Dopo una rimozione riuscita: se non resta più nulla da rimuovere, va
/// dritto alla lista principale invece di mostrare di nuovo "🗑️ Rimuovi
/// voci" ormai vuota (chiesto da Alessio dopo un collaudo dal vivo — quella
/// schermata vuota era un vicolo cieco che richiedeva comunque "⬅️
/// Indietro" per uscirne).
async fn mostra_dopo_rimozione(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    notice: &str,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            show_lista_rimuovi(bot, chat_id, pool, Some(notice)).await?;
            return Ok(());
        }
    };
    let resta_qualcosa = voci_rimovibili(pool, lista.id)
        .await
        .map(|voci| !voci.is_empty())
        .unwrap_or(true);
    if resta_qualcosa {
        show_lista_rimuovi(bot, chat_id, pool, Some(notice)).await
    } else {
        show_lista(bot, chat_id, pool, Some(notice)).await
    }
}

async fn show_lista_rimuovi(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            bot.send_message(chat_id, "⚠️ Non riesco ad aprire la lista della spesa.")
                .reply_markup(nav_markup(&origine_di(chat_id.0)))
                .await?;
            return Ok(());
        }
    };
    let voci = voci_rimovibili(pool, lista.id).await.unwrap_or_default();

    let mut testo = String::new();
    if let Some(notice) = notice {
        testo.push_str(notice);
        testo.push_str("\n\n");
    }
    testo.push_str("🗑️ Rimuovi voci\n\nSolo le voci aggiunte a mano o dal catalogo: quelle generate dai pasti pianificati le gestisce il planner.");
    // Qui si vede anche quello che in lista non compare, perche' in casa
    // ce n'e' gia' abbastanza: senza dirlo sembra una voce fantasma
    // (Alessio, collaudo del 24 settembre 2026).
    let in_lista = carica_voci(pool, lista.id).await.unwrap_or_default();
    let coperte: Vec<&VoceRimovibile> = voci
        .iter()
        .filter(|voce| {
            matches!(voce.origine, OrigineRimovibile::Catalogo)
                && !in_lista.iter().any(|riga| {
                    riga.descrizione.trim().to_lowercase() == voce.descrizione.trim().to_lowercase()
                })
        })
        .collect();
    if !coperte.is_empty() {
        testo.push_str(&format!(
            "\n\n🏠 Di queste in casa ne hai gia' abbastanza, quindi in lista non compaiono: {}.",
            coperte
                .iter()
                .map(|voce| liste::tronca(&voce.descrizione, 40))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if voci.is_empty() {
        testo.push_str("\n\nNessuna voce da rimuovere.");
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = voci
        .iter()
        .map(|voce| {
            let prefisso = match voce.origine {
                OrigineRimovibile::Manuale => "manuale",
                OrigineRimovibile::Catalogo => "catalogo",
            };
            vec![button(
                etichetta_rimovibile(voce),
                format!("lista_spesa:remove:ask:{prefisso}:{}", voce.id),
            )]
        })
        .collect();
    // Con una voce sola "tutte" e "questa" sono la stessa cosa: il pulsante
    // comparirebbe per niente (C8).
    if voci.len() > 1 {
        rows.push(vec![button(
            format!("🗑️ Rimuovi tutte ({})", voci.len()),
            "lista_spesa:remove:all:ask",
        )]);
    }
    rows.push(vec![
        button("⬅️ Indietro", "lista_spesa:back"),
        button("🏠 Menù principale", "menu:main"),
    ]);

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// L'ultima spesa chiusa, in sola lettura: cosa è stato comprato e quando.
///
/// Qui il testo elenca le voci invece di ripeterle su dei pulsanti: non c'è
/// niente da toccare (una voce archiviata non si modifica più, lo impedisce
/// anche il trigger a database), quindi dei pulsanti sarebbero finti — C1
/// vieta di ripetere sul testo ciò che è già sui pulsanti, non di scrivere
/// un elenco quando i pulsanti non esistono.
async fn mostra_ultima_chiusura(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            bot.send_message(chat_id, "⚠️ Non riesco ad aprire la lista della spesa.")
                .reply_markup(nav_markup(&origine_di(chat_id.0)))
                .await?;
            return Ok(());
        }
    };
    let chiusura = ultima_chiusura(pool, lista.id)
        .await
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Lettura dell'ultima spesa chiusa fallita");
            None
        });

    let mut testo = String::from("🗄 Ultima spesa chiusa\n");
    match chiusura {
        Some(chiusura) => {
            let voci = voci_archiviate(pool, chiusura.id).await.unwrap_or_default();
            testo.push_str(&format!(
                "\n📅 {} → {}\nChiusa il {} · {} {}\n\n",
                calendario::display_date(&chiusura.data_inizio),
                calendario::display_date(&chiusura.data_fine),
                calendario::display_date(chiusura.chiusa_il.get(..10).unwrap_or("")),
                chiusura.voci_totali,
                if chiusura.voci_totali == 1 {
                    "voce"
                } else {
                    "voci"
                },
            ));
            for voce in &voci {
                let quantita = match (voce.quantita, &voce.unita_simbolo) {
                    (Some(valore), Some(unita)) => {
                        format!(" · {} {unita}", formatta_quantita(valore))
                    }
                    _ => String::new(),
                };
                testo.push_str(&format!("✅ {}{quantita}\n", voce.descrizione));
            }
        }
        None => {
            testo.push_str("\nNessuna spesa chiusa finora.\n");
        }
    }

    let rows = vec![nav_row("lista_spesa:back")];
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Il resoconto dell'ultimo aggiornamento che ha cambiato qualcosa: voce
/// per voce, con il motivo quando una voce è calata o sparita. Solo
/// lettura, quindi l'elenco sta nel testo (non ci sono pulsanti che lo
/// ripeterebbero, C1).
async fn mostra_modifiche(bot: &Bot, chat_id: ChatId, pool: &SqlitePool) -> ResponseResult<()> {
    let ultimo = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => ultimo_aggiornamento(pool, lista.id).await.unwrap_or(None),
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            None
        }
    };
    let mut testo = String::from("📋 Ultimi cambiamenti della lista\n");
    match ultimo {
        Some((aggiornamento_id, automatico, quando)) => {
            let (data, ora) = quando.split_once(' ').unwrap_or((quando.as_str(), ""));
            testo.push_str(&format!(
                "\n{} {} alle {}\n\n",
                if automatico {
                    "Aggiornata da sola"
                } else {
                    "Aggiornata a mano"
                },
                calendario::display_date(data),
                ora
            ));
            let modifiche = modifiche_di(pool, aggiornamento_id)
                .await
                .unwrap_or_default();
            for modifica in &modifiche {
                testo.push_str(&riga_modifica(modifica));
                testo.push('\n');
            }
        }
        None => testo.push_str("\nNessun cambiamento registrato finora.\n"),
    }
    bot.send_message(chat_id, testo)
        .reply_markup(nav_markup("lista_spesa:back"))
        .await?;
    Ok(())
}

/// Mostra la lista intera, senza paginazione (eccezione esplicita a C6,
/// deciso con Alessio dopo un collaudo dal vivo): a differenza di ogni
/// altra lista del bot, qui l'utente deve vedere tutte le voci insieme per
/// decidere cosa prendere prima e cosa dopo al supermercato -- spezzarla in
/// pagine da cinque negherebbe proprio lo scopo della schermata.
#[derive(Debug, FromRow)]
struct VocePresa {
    descrizione: String,
    alimento_id: Option<i64>,
    prodotto_alimentare_id: Option<i64>,
    quantita: Option<f64>,
    unita_simbolo: Option<String>,
    quantita_presa: Option<f64>,
    unita_presa: Option<String>,
    prodotto_preso_id: Option<i64>,
    prezzo_centesimi: Option<i64>,
}

/// "📦 Ho preso…": al supermercato la confezione raramente è quella
/// giusta al grammo (servono 250 g, c'è quella da 300 g). Qui si sceglie
/// una confezione registrata o si scrive quanto si è preso; alla chiusura
/// entra in casa quello.
async fn mostra_presa(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ListaSpesaSessionStore,
    voce_id: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let voce: Option<VocePresa> = sqlx::query_as(
        "SELECT descrizione, alimento_id, prodotto_alimentare_id, quantita, unita_simbolo, \
                quantita_presa, unita_presa, prodotto_preso_id, prezzo_centesimi \
         FROM liste_spesa_voci WHERE id = ?",
    )
    .bind(voce_id)
    .fetch_optional(pool)
    .await
    .unwrap_or_else(|errore| {
        tracing::warn!(?errore, voce_id, "Lettura voce per la presa fallita");
        None
    });
    let Some(voce) = voce else {
        sessions.clear_chat(chat_id.0);
        show_lista(
            bot,
            chat_id,
            pool,
            Some("⚠️ Questa voce non c'è più nella lista."),
        )
        .await?;
        return Ok(());
    };
    let mut confezioni = match voce.alimento_id {
        Some(alimento_id) => confezioni_per_voce(pool, alimento_id, voce.prodotto_alimentare_id)
            .await
            .unwrap_or_else(|errore| {
                tracing::warn!(?errore, voce_id, "Lettura confezioni fallita");
                Vec::new()
            }),
        None => Vec::new(),
    };
    // Il preferito va in cima e si riconosce dalla stella (consegna B): "di
    // questo alimento compro sempre quello".
    let preferito = match voce.alimento_id {
        Some(alimento_id) => crate::modules::mercato::preferito_di(pool, alimento_id)
            .await
            .unwrap_or_else(|errore| {
                tracing::warn!(?errore, voce_id, "Lettura del preferito fallita");
                None
            }),
        None => None,
    };
    if preferito.is_some() {
        confezioni.sort_by_key(|confezione| Some(confezione.prodotto_id) != preferito);
    }
    sessions.set(
        chat_id.0,
        ListaSpesaConversationState::AwaitingQuantitaPresa {
            voce_id,
            unita_default: voce.unita_simbolo.clone(),
        },
    );

    let mut testo = String::new();
    if let Some(notice) = notice {
        testo.push_str(notice);
        testo.push_str("\n\n");
    }
    testo.push_str(&format!("📦 {}\n\n", voce.descrizione));
    if let (Some(valore), Some(unita)) = (voce.quantita, &voce.unita_simbolo) {
        testo.push_str(&format!(
            "Servono: {} {unita}.\n",
            formatta_quantita(valore)
        ));
    }
    if let (Some(valore), Some(unita)) = (voce.quantita_presa, &voce.unita_presa) {
        testo.push_str(&format!("Presi: {} {unita}.\n", formatta_quantita(valore)));
    }
    // L'ultimo prezzo visto per questa roba, con la data: al supermercato
    // serve proprio a sapere se quello sullo scaffale e' buono (18 settembre
    // 2026). Se e' vecchio, la riga lo dice.
    let storico = match (
        voce.prodotto_preso_id.or(voce.prodotto_alimentare_id),
        voce.alimento_id,
    ) {
        (Some(prodotto_id), _) => {
            crate::modules::mercato::storico_prezzi_prodotto(pool, prodotto_id, 1).await
        }
        (None, Some(alimento_id)) => {
            crate::modules::mercato::storico_prezzi_alimento(pool, alimento_id, 1).await
        }
        _ => Ok(Vec::new()),
    };
    if let Some(ultimo) = storico.unwrap_or_default().first() {
        let oggi = today(pool).await;
        testo.push_str(&format!(
            "
💶 Ultimo prezzo: {}
",
            crate::modules::mercato::riga_prezzo(
                ultimo,
                &oggi,
                crate::modules::calendario::anno_corrente()
            )
        ));
    }

    let esempio = match &voce.unita_simbolo {
        Some(unita) => format!("es. 300, in {unita}"),
        None => "es. 300 g".to_string(),
    };
    if confezioni.is_empty() {
        testo.push_str(&format!("\nScrivi quanto hai preso ({esempio})."));
    } else {
        testo.push_str(&format!(
            "\nTocca la confezione che hai preso, oppure scrivi quanto hai preso ({esempio})."
        ));
        // I nomi per esteso: sul pulsante ci sta il formato e poco altro,
        // e "Parmareggio Parmigiano Reggian…" non si legge (A3).
        let lunghi: Vec<String> = confezioni
            .iter()
            .filter(|confezione| confezione.nome.chars().count() > 24)
            .map(|confezione| {
                format!(
                    "\n• {} {} — {}",
                    formatta_quantita(confezione.quantita),
                    confezione.unita,
                    confezione.nome
                )
            })
            .collect();
        if !lunghi.is_empty() {
            testo.push_str(&lunghi.join(""));
            testo.push('\n');
        }
        // Il significato della stella, che da sola non si capiva (Alessio,
        // 19 settembre 2026).
        if voce.alimento_id.is_some() && confezioni.len() > 1 {
            testo.push_str(
                "\n☆ accanto a una confezione la rende la tua preferita: te la propongo per prima.",
            );
        }
    }

    let confezioni_multiple = confezioni.len() > 1;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = confezioni
        .iter()
        .map(|confezione| {
            let stella = if Some(confezione.prodotto_id) == preferito {
                "⭐ "
            } else {
                ""
            };
            // Il formato **prima** del nome, e il nome tagliato corto: due
            // confezioni dello stesso prodotto si distinguono per il
            // formato, ed è quello che non deve sparire. Mandare a capo
            // l'etichetta non serve -- Telegram non lo fa (punto A3 del
            // collaudo del 24 settembre 2026).
            let mut riga = vec![button(
                format!(
                    "{stella}{} {} · {}",
                    formatta_quantita(confezione.quantita),
                    confezione.unita,
                    liste::tronca(&confezione.nome, 24)
                ),
                format!("lista_spesa:presa:f:{voce_id}:{}", confezione.token),
            )];
            // Con una confezione sola non c'è niente da preferire: la stella
            // comparirebbe per non fare niente (C8, Alessio 23 settembre 2026).
            if let (Some(alimento_id), true) = (voce.alimento_id, confezioni_multiple) {
                // La stella dice lo stato: vuota se non è la preferita, piena
                // se lo è. Era sempre piena, e non si capiva a cosa servisse.
                riga.push(button(
                    if Some(confezione.prodotto_id) == preferito {
                        "⭐"
                    } else {
                        "☆"
                    },
                    format!(
                        "lista_spesa:pref:{voce_id}:{alimento_id}:{}",
                        confezione.prodotto_id
                    ),
                ));
            }
            riga
        })
        .collect();
    if voce.quantita_presa.is_some() {
        rows.push(vec![button(
            "↩️ Conta la quantità in lista",
            format!("lista_spesa:presa:reset:{voce_id}"),
        )]);
    }
    // Consegna B: il prezzo di questa voce e il codice a barre di quello che
    // si è preso davvero. Entrambi facoltativi.
    let etichetta_prezzo = match voce.prezzo_centesimi {
        Some(centesimi) => format!("💶 {}", crate::modules::mercato::formatta_euro(centesimi)),
        None => "💶 Prezzo".to_string(),
    };
    let mut riga_extra = vec![button(
        etichetta_prezzo,
        format!("lista_spesa:prezzo:{voce_id}"),
    )];
    if voce.alimento_id.is_some() {
        riga_extra.push(button(
            "🏷 Codice a barre",
            format!("lista_spesa:ean:{voce_id}"),
        ));
    }
    rows.push(riga_extra);
    rows.push(vec![
        button("⬅️ Indietro", "lista_spesa:back"),
        button("🏠 Menù principale", "menu:main"),
    ]);
    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

async fn salva_presa(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    voce_id: i64,
    quantita: f64,
    unita: &str,
    prodotto_id: Option<i64>,
) -> ResponseResult<()> {
    let avviso = match registra_presa(pool, voce_id, quantita, unita, prodotto_id).await {
        Ok(()) => format!(
            "✅ Segnato: presi {} {unita}. Chiudendo la spesa entra in casa questa quantità.",
            formatta_quantita(quantita)
        ),
        Err(errore) => {
            tracing::warn!(?errore, voce_id, "Registrazione presa fallita");
            "⚠️ Non riesco a segnare quanto hai preso.".to_string()
        }
    };
    show_lista(bot, chat_id, pool, Some(&avviso)).await
}

/// Tastiera del totale dello scontrino: si può sempre saltare.
/// "Le lascio in lista?" per le aggiunte comprate a metà.
fn residuo_keyboard(chiusura_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button(
            "✅ Sì, lasciale",
            format!("lista_spesa:resto:keep:{chiusura_id}"),
        )],
        vec![button(
            "🗑 No, toglile",
            format!("lista_spesa:resto:drop:{chiusura_id}"),
        )],
        // C3: la riga di navigazione ci vuole anche qui. L'avevo dimenticata
        // scrivendo questa schermata, e Alessio se n'è accorto subito
        // (collaudo del 24 settembre 2026). "⬅️ Indietro" vale come
        // "lasciale": la lista resta com'è.
        vec![
            button("⬅️ Indietro", "lista_spesa:menu"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

/// Il totale dello scontrino è facoltativo (consegna B, opzione "c"): si
/// chiede una volta sola, dopo la chiusura, e si può saltare.
async fn chiedi_totale(
    bot: &Bot,
    chat_id: ChatId,
    sessions: &ListaSpesaSessionStore,
    chiusura_id: i64,
    mut messaggio: String,
) -> Result<(), teloxide::RequestError> {
    sessions.set(
        chat_id.0,
        ListaSpesaConversationState::AwaitingTotaleSpesa { chiusura_id },
    );
    messaggio
        .push_str("\n\n🧾 Quant'è il totale dello scontrino? Scrivilo (es. 43,20), oppure salta.");
    bot.send_message(chat_id, messaggio)
        .reply_markup(totale_keyboard())
        .await?;
    Ok(())
}

/// Toglie le aggiunte che questa chiusura aveva ridotto: è la risposta "no"
/// alla domanda sul residuo.
pub async fn togli_aggiunte_ridotte(pool: &SqlitePool, chiusura_id: i64) -> anyhow::Result<u64> {
    let esito =
        sqlx::query("DELETE FROM liste_spesa_aggiunte_catalogo WHERE ridotta_chiusura_id = ?")
            .bind(chiusura_id)
            .execute(pool)
            .await
            .context("Impossibile togliere le aggiunte rimaste a metà")?;
    Ok(esito.rows_affected())
}

fn totale_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![button("➖ Salta il totale", "lista_spesa:totale:salta")],
        // C3: mancava "⬅️ Indietro", e da questa schermata si poteva solo
        // scrivere il totale o saltare (Alessio, 24 settembre 2026). Torna
        // alla lista senza segnare niente, come saltare.
        vec![
            button("⬅️ Indietro", "lista_spesa:menu"),
            button("🏠 Menù principale", "menu:main"),
        ],
    ])
}

/// Legge un codice a barre con Open Food Facts e lo usa per la presa: se il
/// catalogo non conosce quel prodotto lo crea sull'alimento della voce, poi
/// segna la confezione come quantità presa. Se Open Prices conosce un
/// prezzo, lo dice — come suggerimento di altri, non come prezzo visto.
async fn leggi_codice_a_barre(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    sessions: &ListaSpesaSessionStore,
    voce_id: i64,
    testo: &str,
) -> ResponseResult<()> {
    use crate::modules::mercato;
    let Some(ean) = mercato::codice_a_barre_valido(testo) else {
        mostra_presa(
            bot,
            chat_id,
            pool,
            sessions,
            voce_id,
            Some("⚠️ Non sembra un codice a barre: sono 8, 12, 13 o 14 cifre."),
        )
        .await?;
        return Ok(());
    };
    let alimento_id: Option<i64> =
        sqlx::query_scalar("SELECT alimento_id FROM liste_spesa_voci WHERE id = ?")
            .bind(voce_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten();
    let Some(alimento_id) = alimento_id else {
        mostra_presa(
            bot,
            chat_id,
            pool,
            sessions,
            voce_id,
            Some("⚠️ Questa voce non è legata a un alimento del catalogo."),
        )
        .await?;
        return Ok(());
    };

    // Il catalogo prima della rete: se il prodotto c'è già, non si chiede
    // niente a nessuno.
    let gia_noto = mercato::prodotto_per_ean(pool, &ean).await.unwrap_or(None);
    let (prodotto_id, descrizione, quantita, unita) = match gia_noto {
        Some(prodotto_id) => {
            let riga: Option<(String, String, f64, String)> = sqlx::query_as(
                "SELECT p.marca, p.nome_commerciale, p.quantita_confezione, um.simbolo \
                 FROM prodotti_alimentari p \
                 JOIN unita_misura um ON um.id = p.unita_confezione_id WHERE p.id = ?",
            )
            .bind(prodotto_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            let Some((marca, nome, quantita, unita)) = riga else {
                mostra_presa(
                    bot,
                    chat_id,
                    pool,
                    sessions,
                    voce_id,
                    Some("⚠️ Non riesco a leggere questo prodotto."),
                )
                .await?;
                return Ok(());
            };
            (prodotto_id, format!("{marca} {nome}"), quantita, unita)
        }
        None => {
            bot.send_message(chat_id, "🔎 Cerco il codice a barre…")
                .await?;
            let esterno = match mercato::cerca_su_open_food_facts(&ean).await {
                Ok(Some(prodotto)) => prodotto,
                Ok(None) => {
                    mostra_presa(
                        bot,
                        chat_id,
                        pool,
                        sessions,
                        voce_id,
                        Some("🔎 Open Food Facts non conosce questo codice.\nScrivi la quantità a mano."),
                    )
                    .await?;
                    return Ok(());
                }
                Err(errore) => {
                    tracing::warn!(?errore, ean, "Lettura da Open Food Facts fallita");
                    mostra_presa(
                        bot,
                        chat_id,
                        pool,
                        sessions,
                        voce_id,
                        Some("⚠️ Non riesco a raggiungere Open Food Facts adesso.\nScrivi la quantità a mano."),
                    )
                    .await?;
                    return Ok(());
                }
            };
            match mercato::crea_prodotto_da_esterno(pool, alimento_id, &esterno).await {
                Ok(prodotto_id) => (
                    prodotto_id,
                    format!("{} {}", esterno.marca, esterno.nome),
                    esterno.quantita,
                    esterno.unita.clone(),
                ),
                Err(errore) => {
                    tracing::warn!(?errore, ean, "Salvataggio prodotto da EAN fallito");
                    mostra_presa(
                        bot,
                        chat_id,
                        pool,
                        sessions,
                        voce_id,
                        Some("⚠️ Ho letto il prodotto ma non riesco a salvarlo."),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
    };

    let mut avviso = match registra_presa(pool, voce_id, quantita, &unita, Some(prodotto_id)).await
    {
        Ok(()) => format!(
            "✅ {descrizione}: presi {} {unita}.",
            formatta_quantita(quantita)
        ),
        Err(errore) => {
            tracing::warn!(?errore, voce_id, "Presa da codice a barre fallita");
            "⚠️ Non riesco a segnare quello che hai preso.".to_string()
        }
    };
    // Open Prices è un dato di altri: si propone, non si registra.
    if let Ok(Some(centesimi)) = mercato::prezzo_su_open_prices(&ean).await {
        avviso.push_str(&format!(
            // C1: il pulsante è già lì sotto, il testo non lo ripete.
            "\n💶 Su Open Prices altri l'hanno pagato {}: il tuo prezzo però lo segni tu.",
            mercato::formatta_euro(centesimi)
        ));
    }
    show_lista(bot, chat_id, pool, Some(&avviso)).await
}

/// Imposta (o toglie) il negozio di questa spesa.
async fn imposta_negozio_lista(
    pool: &SqlitePool,
    lista_id: i64,
    negozio_id: Option<i64>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE liste_spesa SET negozio_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(negozio_id)
    .bind(lista_id)
    .execute(pool)
    .await
    .context("Impossibile scegliere il negozio della spesa")?;
    Ok(())
}

/// Segna il prezzo pagato per una voce e lo registra nello storico del
/// negozio di oggi, così la prossima volta la stima sa quanto costa.
async fn registra_prezzo_voce(
    pool: &SqlitePool,
    voce_id: i64,
    prezzo_centesimi: i64,
    negozio_id: Option<i64>,
) -> anyhow::Result<()> {
    // Tutte le colonne di `VocePresa`: il 17 settembre 2026 qui mancava
    // `prezzo_centesimi`, la lettura falliva sempre e all'utente arrivava
    // solo "non riesco a segnare il prezzo". Stessa famiglia del difetto
    // degli ingredienti delle ricette — una colonna che la struct pretende
    // e la query non dà — trovata da Alessio nel collaudo del 18 settembre.
    let voce: Option<VocePresa> = sqlx::query_as(
        "SELECT descrizione, alimento_id, prodotto_alimentare_id, quantita, unita_simbolo, \
                quantita_presa, unita_presa, prodotto_preso_id, prezzo_centesimi \
         FROM liste_spesa_voci WHERE id = ?",
    )
    .bind(voce_id)
    .fetch_optional(pool)
    .await
    .context("Impossibile rileggere la voce")?;
    let voce = voce.context("Voce non trovata")?;
    sqlx::query(
        "UPDATE liste_spesa_voci SET prezzo_centesimi = ?, comprato = 1, \
         comprato_il = COALESCE(comprato_il, strftime('%Y-%m-%dT%H:%M:%fZ','now')), \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(prezzo_centesimi)
    .bind(voce_id)
    .execute(pool)
    .await
    .context("Impossibile segnare il prezzo")?;
    // Lo storico serve al confronto, e il confronto ha senso solo con un
    // negozio: senza, il prezzo resta comunque sulla voce e sullo scontrino.
    if let Some(negozio_id) = negozio_id {
        let (quantita, unita) = match (voce.quantita_presa, voce.unita_presa.clone()) {
            (Some(presa), Some(unita)) => (Some(presa), Some(unita)),
            _ => (voce.quantita, voce.unita_simbolo.clone()),
        };
        crate::modules::mercato::registra_prezzo(
            pool,
            negozio_id,
            voce.alimento_id,
            voce.prodotto_preso_id.or(voce.prodotto_alimentare_id),
            &voce.descrizione,
            prezzo_centesimi,
            quantita,
            unita.as_deref(),
            "spesa",
        )
        .await?;
    }
    Ok(())
}

/// Quantità in lista e quantità presa, con le loro unità.
type RigaQuantitaGrezza = (Option<f64>, Option<String>, Option<f64>, Option<String>);

/// `5,00 € al kg` per una confezione da 500 g pagata 2,50 €: serve a capire
/// se il prezzo è buono davvero. `None` quando la quantità non c'è o l'unità
/// non si converte.
async fn riferimento_voce(pool: &SqlitePool, voce_id: i64, centesimi: i64) -> Option<String> {
    let riga: Option<RigaQuantitaGrezza> = sqlx::query_as(
        "SELECT quantita, unita_simbolo, quantita_presa, unita_presa          FROM liste_spesa_voci WHERE id = ?",
    )
    .bind(voce_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let (quantita, unita, presa, unita_presa) = riga?;
    let (quantita, unita) = match (presa, unita_presa) {
        (Some(presa), Some(unita)) => (Some(presa), Some(unita)),
        _ => (quantita, unita),
    };
    let (per_riferimento, riferimento) =
        crate::modules::mercato::prezzo_al_riferimento(centesimi, quantita, unita.as_deref())?;
    Some(format!(
        "{} al {riferimento}",
        crate::modules::mercato::formatta_euro(per_riferimento)
    ))
}

/// Salva il totale dello scontrino sulla chiusura appena fatta. È
/// facoltativo (opzione "c" scelta da Alessio) e diventerà una transazione
/// del modulo Soldi.
async fn registra_totale_chiusura(
    pool: &SqlitePool,
    chiusura_id: i64,
    totale_centesimi: i64,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE liste_spesa_chiusure SET totale_centesimi = ? WHERE id = ?")
        .bind(totale_centesimi)
        .bind(chiusura_id)
        .execute(pool)
        .await
        .context("Impossibile salvare il totale della spesa")?;
    Ok(())
}

/// La stima della lista in ogni negozio scelto: per ogni voce l'ultimo
/// prezzo visto lì, moltiplicato per quante confezioni servono non si sa,
/// quindi si conta una volta sola — è una stima, e il testo lo dice.
async fn stime_dei_negozi(
    pool: &SqlitePool,
    voci: &[VoceListaSpesa],
) -> anyhow::Result<(Vec<crate::modules::mercato::StimaNegozio>, usize)> {
    use crate::modules::mercato::{ultimo_prezzo, StimaNegozio};
    let negozi = crate::modules::mercato::negozi_scelti(pool).await?;
    // Tutta la lista, non solo quello che resta da comprare: spuntando una
    // voce il confronto spariva e il bot diceva "non ho ancora prezzi" anche
    // con i prezzi segnati (Alessio, collaudo del 23 settembre 2026). Il
    // confronto serve a decidere dove fare la spesa, e una voce già presa
    // conta comunque nel totale di quella spesa.
    let da_comprare: Vec<&VoceListaSpesa> = voci.iter().collect();
    let mut stime = Vec::new();
    let mut coperte_da_qualcuno = vec![false; da_comprare.len()];
    for negozio in negozi {
        let mut totale = 0_i64;
        let mut con_prezzo = 0_usize;
        for (indice, voce) in da_comprare.iter().enumerate() {
            let prezzo = ultimo_prezzo(
                pool,
                negozio.id,
                voce.alimento_id,
                voce.prodotto_alimentare_id,
            )
            .await?;
            if let Some((centesimi, _, _)) = prezzo {
                totale += centesimi;
                con_prezzo += 1;
                coperte_da_qualcuno[indice] = true;
            }
        }
        if con_prezzo > 0 {
            stime.push(StimaNegozio {
                negozio_id: negozio.id,
                nome: negozio.nome,
                totale_centesimi: totale,
                voci_con_prezzo: con_prezzo,
                voci_totali: da_comprare.len(),
            });
        }
    }
    crate::modules::mercato::ordina_stime(&mut stime);
    let senza_prezzo = coperte_da_qualcuno.iter().filter(|c| !**c).count();
    Ok((stime, senza_prezzo))
}

/// Chiede il nome di un negozio nuovo. Vive qui, e non in `mercato`, perché
/// la sessione di testo è quella della lista della spesa.
async fn attendi_nome_negozio(
    bot: &Bot,
    chat_id: ChatId,
    sessions: &ListaSpesaSessionStore,
) -> ResponseResult<()> {
    sessions.set(chat_id.0, ListaSpesaConversationState::AwaitingNomeNegozio);
    bot.send_message(
        chat_id,
        "🏪 Come si chiama il negozio?\n\nAd esempio: Il fruttivendolo di via Roma.",
    )
    .reply_markup(InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", "mercato:negozi"),
        button("🏠 Menù principale", "menu:main"),
    ]]))
    .await?;
    Ok(())
}

async fn show_lista(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            bot.send_message(chat_id, "⚠️ Non riesco ad aprire la lista della spesa.")
                .reply_markup(nav_markup(&origine_di(chat_id.0)))
                .await?;
            return Ok(());
        }
    };
    // I pasti ormai passati si prendono le loro scorte prima di calcolare il
    // fabbisogno: è quello che la lista deve sapere per dire cosa manca.
    let scarichi = crate::modules::dispensa::scala_pasti_scaduti(pool)
        .await
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Scarico dei pasti passati fallito");
            Vec::new()
        });
    let avviso_scarichi = crate::modules::dispensa::avviso_scarichi(&scarichi);
    let automatico = aggiornamento_automatico(pool).await;
    // Il bottone "🔄 Aggiorna lista" compare solo se premerlo cambierebbe
    // davvero qualcosa (stesso principio di "🔄 Aggiorna planner" sulle
    // ricette cambiate) -- deciso con Alessio dopo un collaudo dal vivo in
    // cui il bottone c'era sempre, anche a lista già aggiornata.
    let serve_refresh = serve_aggiornamento(pool, &lista)
        .await
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Verifica aggiornamento lista spesa fallita");
            true
        });
    // Con l'aggiornamento automatico acceso, la lista si allinea da sola
    // all'apertura -- ma solo se c'è davvero qualcosa da cambiare, per non
    // riscrivere le voci a ogni tocco -- e dice sempre cosa ha fatto.
    let modifiche_automatiche = if automatico && serve_refresh {
        aggiorna_e_registra(pool, &lista, true)
            .await
            .unwrap_or_else(|errore| {
                tracing::warn!(?errore, "Aggiornamento automatico della lista fallito");
                Vec::new()
            })
    } else {
        Vec::new()
    };
    let serve_refresh = serve_refresh && !automatico;
    let voci = carica_voci(pool, lista.id).await.unwrap_or_default();
    let totale = voci.len();
    let comprate = voci.iter().filter(|voce| voce.comprato != 0).count();
    let ce_un_resoconto = ultimo_aggiornamento(pool, lista.id)
        .await
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Lettura dell'ultimo aggiornamento fallita");
            None
        })
        .is_some();
    // Una voce già comprata resta sempre congelata (mai corretta da sola),
    // ma se il fabbisogno reale è sceso sotto quanto già segnato -- un
    // pasto tolto dal planner, una ricetta ridotta -- l'utente deve saperlo
    // (deciso con Alessio il 9 settembre 2026): ogni voce coinvolta lo
    // mostra sul proprio pulsante.
    let eccessi = eccessi_comprati(pool, &lista)
        .await
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Verifica eccessi lista spesa fallita");
            Vec::new()
        });
    let ci_sono_voci_rimovibili = voci_rimovibili(pool, lista.id)
        .await
        .map(|voci| !voci.is_empty())
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Verifica voci rimovibili fallita");
            false
        });
    // "🗄 Ultima spesa chiusa" compare solo se una spesa è già stata chiusa:
    // un pulsante che porta a una schermata vuota è un vicolo cieco (stesso
    // principio già applicato a "🗑️ Rimuovi voci").
    let ce_un_archivio = ultima_chiusura(pool, lista.id)
        .await
        .unwrap_or_else(|errore| {
            tracing::warn!(?errore, "Lettura dell'ultima spesa chiusa fallita");
            None
        })
        .is_some();
    let oggi = today(pool).await;
    let negozio_nome = match lista.negozio_id {
        Some(negozio_id) => crate::modules::mercato::negozio_per_id(pool, negozio_id)
            .await
            .unwrap_or_default()
            .map(|negozio| negozio.nome),
        None => None,
    };

    let tutorial = tutorial_da_mostrare(pool).await;

    let mut testo = String::new();
    if let Some(notice) = notice {
        testo.push_str(notice);
        testo.push_str("\n\n");
    }
    if let Some(avviso) = &avviso_scarichi {
        testo.push_str(avviso);
        testo.push_str("\n\n");
    }
    if !modifiche_automatiche.is_empty() {
        testo.push_str(&format!(
            "🔄 Aggiornata da sola: {}.\nIl dettaglio è in 📋 Ultimi cambiamenti.\n\n",
            riepilogo_modifiche(&modifiche_automatiche)
        ));
    }
    if let Some(tutorial) = &tutorial {
        testo.push_str(tutorial);
        testo.push_str("\n\n");
    }
    testo.push_str(&format!(
        "🛒 Lista della spesa\n\n📅 {} → {}\n",
        calendario::display_date(&lista.data_inizio),
        calendario::display_date(&lista.data_fine)
    ));
    // La lista si sposta da sola in avanti (`applica_inizio_da_oggi`), quindi
    // un inizio precedente a oggi può solo essere una scelta esplicita
    // dell'utente: va detto, perché da lì in poi la lista resta ferma e
    // continua ad aggregare pasti ormai passati.
    if lista.data_inizio < oggi {
        testo.push_str(&format!(
            "\n⚠️ Parte dal {}, prima di oggi: l'hai scelto tu, quindi resta fermo finché non cambi l'intervallo.\n",
            calendario::display_date(&lista.data_inizio)
        ));
    }
    if totale == 0 {
        if serve_refresh {
            testo.push_str(
                "\nNessuna voce nella lista.\nUsa 🔄 Aggiorna lista per generarla dai pasti pianificati, oppure aggiungine una manuale.\n",
            );
        } else {
            testo.push_str(
                "\nNessuna voce nella lista.\nAggiungine una manuale, oppure pianifica qualche pasto nel planner.\n",
            );
        }
    } else {
        testo.push_str(&format!("\n{comprate}/{totale} comprate\n"));
    }
    if !eccessi.is_empty() {
        // Quali voci, non "vedi i pulsanti sotto": con venti voci in lista
        // quella riga faceva cercare (Alessio, collaudo del 24 settembre
        // 2026).
        let nomi: Vec<String> = eccessi
            .iter()
            .map(|eccesso| liste::tronca(&eccesso.nome, 40))
            .collect();
        testo.push_str(&format!(
            "\n⚠️ Hai già segnato più di quanto serve ora: {}.\n",
            nomi.join(", ")
        ));
    }
    // Consegna B: quanto è già stato segnato in questa spesa. Nessun prezzo
    // registrato, nessuna riga in più: chi non usa i prezzi non li vede.
    let segnato: i64 = voci.iter().filter_map(|voce| voce.prezzo_centesimi).sum();
    if segnato > 0 {
        testo.push_str(&format!(
            "\n💶 Segnato finora: {}\n",
            crate::modules::mercato::formatta_euro(segnato)
        ));
    }

    // Legenda dei simboli (18 settembre 2026), accesa finché non la spegni.
    let legenda = liste::legenda_attiva(pool).await;
    if legenda && totale > 0 {
        testo.push_str(&liste::blocco_legenda(liste::LEGENDA_LISTA_SPESA));
    }

    // Quello che non entra in un pulsante: la confezione presa, il prezzo
    // segnato e l'eccesso. Solo per le voci che ne hanno davvero -- se non
    // ne ha nessuna, questo blocco non compare (C8: niente righe vuote).
    let mut dettagli_voci: Vec<String> = Vec::new();
    for voce in &voci {
        let mut parti: Vec<String> = Vec::new();
        if let (Some(valore), Some(unita)) = (voce.quantita_presa, &voce.unita_presa) {
            parti.push(format!("📦 {} {unita}", formatta_quantita(valore)));
        }
        if let Some(centesimi) = voce.prezzo_centesimi {
            parti.push(format!(
                "💶 {}",
                crate::modules::mercato::formatta_euro(centesimi)
            ));
        }
        if let Some(eccesso) = eccessi.iter().find(|eccesso| {
            Some(&eccesso.unita_simbolo) == voce.unita_simbolo.as_ref()
                && eccesso.nome.trim().to_lowercase() == voce.descrizione.trim().to_lowercase()
        }) {
            parti.push(format!(
                "⚠️ {} {} in eccesso",
                formatta_quantita(eccesso.quantita_eccesso),
                eccesso.unita_simbolo
            ));
        }
        if !parti.is_empty() {
            dettagli_voci.push(format!(
                "• {} — {}",
                liste::tronca(&voce.descrizione, 40),
                parti.join(" · ")
            ));
        }
    }
    if !dettagli_voci.is_empty() {
        testo.push_str(&format!("\n{}\n", dettagli_voci.join("\n")));
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for voce in &voci {
        let icona = if voce.comprato != 0 { "✅" } else { "☐" };
        let quantita = match (voce.quantita, &voce.unita_simbolo) {
            // "1,5 l", non "1500 ml": la lista aggrega nell'unita' di base,
            // ma chi legge ha scritto "1,5 l" e in "Rimuovi voci" lo
            // ritrovava scritto cosi' -- due numeri diversi per la stessa
            // cosa (Alessio, collaudo del 24 settembre 2026).
            (Some(valore), Some(unita)) => format!(
                " · {}",
                crate::modules::dispensa::formatta_quantita_leggibile(valore, unita)
            ),
            _ => String::new(),
        };
        // Sul pulsante ci stanno il nome e la quantita', e basta.
        //
        // Il 23 settembre avevo provato a mandare a capo l'etichetta con un
        // "a capo": non funziona. Telegram **non manda a capo le etichette
        // dei pulsanti**, e su Desktop resta una riga sola che taglia la
        // fine -- cioe' proprio il prezzo e la confezione, il motivo per cui
        // quella voce era stata presa (Alessio, collaudo del 24 settembre
        // 2026, punto A1). Quindi il pulsante porta il minimo indispensabile
        // e il resto sta nel testo qui sopra, dove lo spazio non manca.
        let mut riga = vec![button(
            format!("{icona} {}{quantita}", liste::tronca(&voce.descrizione, 24)),
            format!("lista_spesa:toggle:{}", voce.id),
        )];
        // "📦 Ho preso…" dove c'è una quantità da correggere, e sulle voci del
        // catalogo senza quantità: è da lì che si dice quanto se ne è preso, e
        // solo così entrano nelle scorte (18 settembre 2026).
        if voce.quantita.is_some() || voce.alimento_id.is_some() {
            riga.push(button("📦", format!("lista_spesa:presa:{}", voce.id)));
        }
        rows.push(riga);
    }
    if serve_refresh {
        rows.push(vec![button("🔄 Aggiorna lista", "lista_spesa:refresh")]);
    }
    rows.push(vec![button("➕ Aggiungi voce manuale", "lista_spesa:add")]);
    // Le azioni secondarie vanno a coppie, per non allungare troppo una
    // schermata che già mostra tutte le voci (eccezione a C6).
    let mut accoppia = |uno: Option<InlineKeyboardButton>, due: Option<InlineKeyboardButton>| {
        let riga: Vec<InlineKeyboardButton> = [uno, due].into_iter().flatten().collect();
        if !riga.is_empty() {
            rows.push(riga);
        }
    };
    accoppia(
        (voci.len() > 1).then(|| button("↕️ Riordina", "lista_spesa:reorder")),
        ci_sono_voci_rimovibili.then(|| button("🗑️ Rimuovi voci", "lista_spesa:remove")),
    );
    // Il momento "spesa fatta": ha senso solo se qualcosa è stato comprato.
    accoppia(
        (comprate > 0).then(|| button("🧾 Chiudi la spesa", "lista_spesa:close:ask")),
        ce_un_archivio.then(|| button("🗄 Ultima spesa", "lista_spesa:archivio")),
    );
    // Dalla lista al planner (chiesto da Alessio il 16 settembre 2026). Se
    // alla lista si è arrivati proprio dal planner, il pulsante ci torna e
    // basta, invece di aprire un giro planner → lista → planner senza fine.
    let verso_planner = if origine_di(chat_id.0) == "planner:menu" {
        "planner:menu"
    } else {
        "planner:menu:lista"
    };
    accoppia(
        Some(button("🗓️ Cambia intervallo", "lista_spesa:range:start")),
        Some(button("📅 Planner", verso_planner)),
    );
    accoppia(
        ce_un_resoconto.then(|| button("📋 Ultimi cambiamenti", "lista_spesa:modifiche")),
        None,
    );
    // Consegna B: dove si sta facendo la spesa e dove converrebbe farla.
    accoppia(
        Some(button(
            match &negozio_nome {
                Some(nome) => format!("🏪 {nome}"),
                None => "🏪 Scegli negozio".to_string(),
            },
            "lista_spesa:negozio",
        )),
        (totale > 0).then(|| button("📊 Dove conviene", "lista_spesa:conviene")),
    );
    rows.push(vec![button(
        if automatico {
            "⚙️ Aggiornamento automatico: attivo"
        } else {
            "⚙️ Aggiornamento automatico: spento"
        },
        "lista_spesa:auto",
    )]);
    rows.push(vec![liste::pulsante_legenda(
        legenda,
        "lista_spesa:legenda",
    )]);
    rows.push(nav_row(&origine_di(chat_id.0)));

    // Solo dopo aver davvero costruito la schermata: chi la raggiunge la sta
    // vedendo per davvero, non solo aprendo un menù intermedio (C14).
    segna_vista_novita(pool).await;

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}
