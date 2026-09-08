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
    pub nome: String,
    pub unita_simbolo: String,
    pub quantita: f64,
}

/// Una voce generata dall'aggregazione, pronta per `liste_spesa_voci`.
#[derive(Debug, Clone, PartialEq)]
pub struct VoceGenerata {
    pub alimento_id: Option<i64>,
    pub nome: String,
    pub quantita: f64,
    pub unita_simbolo: String,
}

/// Identità di aggregazione: l'alimento del catalogo se presente, altrimenti
/// il nome normalizzato -- due righe con lo stesso `alimento_id` restano
/// insieme anche se il nome congelato differisce (rinominato nel frattempo),
/// due righe senza `alimento_id` si aggregano per nome uguale a meno di
/// maiuscole/spazi.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Identita {
    Alimento(i64),
    Nome(String),
}

fn identita_riga(riga: &RigaIngrediente) -> Identita {
    match riga.alimento_id {
        Some(id) => Identita::Alimento(id),
        None => Identita::Nome(riga.nome.trim().to_lowercase()),
    }
}

fn identita_voce(voce: &VoceGenerata) -> Identita {
    match voce.alimento_id {
        Some(id) => Identita::Alimento(id),
        None => Identita::Nome(voce.nome.trim().to_lowercase()),
    }
}

/// Arrotonda a due decimali: la somma di quantità in virgola mobile
/// altrimenti mostrerebbe cifre che nessuna ricetta ha mai scritto
/// (es. `133.33000000000001 g`).
pub fn arrotonda(quantita: f64) -> f64 {
    (quantita * 100.0).round() / 100.0
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

        let conversione = info_unita(&riga.unita_simbolo)
            .and_then(|info| info.famiglia.map(|famiglia| (famiglia, info)));
        let (unita_out, quantita_out) = match conversione {
            Some((famiglia, info)) if info.fattore_den != 0.0 => (
                famiglia.unita_base().to_string(),
                riga.quantita * info.fattore_num / info.fattore_den,
            ),
            _ => (riga.unita_simbolo.clone(), riga.quantita),
        };

        let identita = identita_riga(riga);
        if let Some(voce) = risultato
            .iter_mut()
            .find(|voce| identita_voce(voce) == identita && voce.unita_simbolo == unita_out)
        {
            voce.quantita += quantita_out;
        } else {
            risultato.push(VoceGenerata {
                alimento_id: riga.alimento_id,
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

/// Formatta una quantità per la UI: interi senza decimali, il resto con al
/// più due cifre senza zeri superflui (`500` invece di `500.00`, `133.33`
/// invece di `133.330000000001`).
fn formatta_quantita(valore: f64) -> String {
    if (valore.fract()).abs() < 1e-9 {
        format!("{valore:.0}")
    } else {
        let testo = format!("{valore:.2}");
        testo
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
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
                nome: "Farina".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 200.0,
            },
            RigaIngrediente {
                alimento_id: Some(1),
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
                nome: "Farina".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 100.0,
            },
            RigaIngrediente {
                alimento_id: Some(2),
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
                nome: "Yogurt".to_string(),
                unita_simbolo: "confezione".to_string(),
                quantita: 2.0,
            },
            RigaIngrediente {
                alimento_id: Some(3),
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
                nome: "Uova".to_string(),
                unita_simbolo: "pz".to_string(),
                quantita: 2.0,
            },
            RigaIngrediente {
                alimento_id: Some(4),
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
                nome: "Sale".to_string(),
                unita_simbolo: "cucchiaio".to_string(),
                quantita: 1.0,
            },
            RigaIngrediente {
                alimento_id: Some(5),
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
                nome: "Zero".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 0.0,
            },
            RigaIngrediente {
                alimento_id: Some(6),
                nome: "Negativo".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: -5.0,
            },
            RigaIngrediente {
                alimento_id: Some(6),
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
                nome: "Pane".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 100.0,
            },
            RigaIngrediente {
                alimento_id: None,
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
                nome: "Passata".to_string(),
                unita_simbolo: "g".to_string(),
                quantita: 200.0,
            },
            RigaIngrediente {
                alimento_id: Some(7),
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
    fn formattazione_quantita_evita_decimali_inventati() {
        assert_eq!(formatta_quantita(500.0), "500");
        assert_eq!(formatta_quantita(133.33), "133.33");
        assert_eq!(formatta_quantita(0.3), "0.3");
    }

    #[test]
    fn sottrai_gia_comprato_lascia_solo_la_differenza() {
        let fresche = vec![VoceGenerata {
            alimento_id: Some(1),
            nome: "Farina".to_string(),
            quantita: 350.0,
            unita_simbolo: "g".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(1),
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
            nome: "Farina".to_string(),
            quantita: 200.0,
            unita_simbolo: "g".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(1),
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
            nome: "Latte".to_string(),
            quantita: 500.0,
            unita_simbolo: "ml".to_string(),
        }];
        let gia_comprato = vec![VoceGenerata {
            alimento_id: Some(2),
            nome: "Latte".to_string(),
            quantita: 1.0,
            unita_simbolo: "l".to_string(),
        }];
        let residuo = sottrai_gia_comprato(fresche, &gia_comprato);
        assert_eq!(residuo.len(), 1);
        assert_eq!(residuo[0].quantita, 500.0);
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
}

async fn trova_per_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<ListaSpesa>> {
    sqlx::query_as(
        "SELECT id, proprietario_utente_id, spazio_id, data_inizio, data_fine \
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
        "SELECT id, proprietario_utente_id, spazio_id, data_inizio, data_fine \
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
        return Ok(lista);
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

/// Cambia l'intervallo di una lista esistente. Non tocca le voci: il
/// ricalcolo è sempre un'azione esplicita separata (`aggiorna_lista`).
pub async fn cambia_intervallo(
    pool: &SqlitePool,
    lista_id: i64,
    data_inizio: &str,
    data_fine: &str,
) -> anyhow::Result<()> {
    if !calendario::valid_date(data_inizio) || !calendario::valid_date(data_fine) {
        anyhow::bail!("Data non valida");
    }
    if data_fine < data_inizio {
        anyhow::bail!("La data di fine non può precedere quella di inizio");
    }
    sqlx::query(
        "UPDATE liste_spesa SET data_inizio = ?, data_fine = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(data_inizio)
    .bind(data_fine)
    .bind(lista_id)
    .execute(pool)
    .await
    .context("Impossibile cambiare l'intervallo della lista")?;
    Ok(())
}

pub async fn carica_voci(pool: &SqlitePool, lista_id: i64) -> anyhow::Result<Vec<VoceListaSpesa>> {
    sqlx::query_as(
        "SELECT id, origine, alimento_id, descrizione, quantita, unita_simbolo, comprato \
         FROM liste_spesa_voci WHERE lista_id = ? \
         ORDER BY comprato ASC, origine DESC, descrizione COLLATE NOCASE, id",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci della lista")
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
    let id = sqlx::query(
        "INSERT INTO liste_spesa_voci (lista_id, origine, descrizione, quantita, unita_simbolo) \
         VALUES (?, 'manuale', ?, ?, ?)",
    )
    .bind(lista_id)
    .bind(descrizione)
    .bind(quantita)
    .bind(unita_simbolo)
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
        sqlx::query(
            "UPDATE liste_spesa_voci SET comprato = 0, comprato_il = NULL, \
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
    let attuale: Option<i64> =
        sqlx::query_scalar("SELECT comprato FROM liste_spesa_voci WHERE id = ?")
            .bind(voce_id)
            .fetch_optional(pool)
            .await
            .context("Impossibile leggere la voce")?;
    let attuale = attuale.context("Voce non trovata")?;
    imposta_comprato(pool, voce_id, attuale == 0).await
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
/// Riga grezza di `liste_spesa_voci`: alimento, descrizione, quantità e
/// unità -- opzionali solo perché la query li rilegge così com'è la
/// colonna, non perché possano mancare davvero su una voce generata.
type RigaVoceGenerataGrezza = (Option<i64>, String, Option<f64>, Option<String>);

async fn voci_generate_comprate(
    pool: &SqlitePool,
    lista_id: i64,
) -> anyhow::Result<Vec<VoceGenerata>> {
    let righe: Vec<RigaVoceGenerataGrezza> = sqlx::query_as(
        "SELECT alimento_id, descrizione, quantita, unita_simbolo \
         FROM liste_spesa_voci \
         WHERE lista_id = ? AND origine = 'generato' AND comprato = 1",
    )
    .bind(lista_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere le voci generate già comprate")?;

    Ok(righe
        .into_iter()
        .filter_map(|(alimento_id, nome, quantita, unita_simbolo)| {
            Some(VoceGenerata {
                alimento_id,
                nome,
                quantita: quantita?,
                unita_simbolo: unita_simbolo?,
            })
        })
        .collect())
}

/// Riga grezza di `unita_misura`: simbolo, famiglia (testo o assente), e il
/// fattore base num/den quando la famiglia è presente.
type RigaUnitaMisuraGrezza = (String, Option<String>, Option<i64>, Option<i64>);

async fn carica_mappa_unita(pool: &SqlitePool) -> anyhow::Result<HashMap<String, InfoUnita>> {
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

/// Aggiornamento esplicito (mai automatico): ricalcola SOLO le voci
/// `origine = 'generato' AND comprato = 0` -- le cancella e re-inserisce da
/// zero il risultato fresco dell'aggregazione. Le voci comprate (generate o
/// manuali) e le voci manuali non comprate restano congelate esattamente
/// come sono. Ritorna il numero di voci generate dopo il refresh.
pub async fn aggiorna_lista(pool: &SqlitePool, lista: &ListaSpesa) -> anyhow::Result<usize> {
    let righe = righe_da_aggregare(pool, lista).await?;
    let mappa_unita = carica_mappa_unita(pool).await?;
    let fresche = aggrega_ingredienti(&righe, |simbolo| mappa_unita.get(simbolo).copied());
    // Le voci già comprate restano intoccate (mai cancellate qui sotto): si
    // sottrae quello che coprono già dal fresco, così non si duplica mai
    // una quantità già segnata come acquistata.
    let gia_comprato = voci_generate_comprate(pool, lista.id).await?;
    let voci = sottrai_gia_comprato(fresche, &gia_comprato);

    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la transazione")?;
    sqlx::query(
        "DELETE FROM liste_spesa_voci \
         WHERE lista_id = ? AND origine = 'generato' AND comprato = 0",
    )
    .bind(lista.id)
    .execute(&mut *tx)
    .await
    .context("Impossibile ripulire le voci generate")?;
    for voce in &voci {
        sqlx::query(
            "INSERT INTO liste_spesa_voci \
             (lista_id, origine, alimento_id, descrizione, quantita, unita_simbolo) \
             VALUES (?, 'generato', ?, ?, ?, ?)",
        )
        .bind(lista.id)
        .bind(voce.alimento_id)
        .bind(&voce.nome)
        .bind(voce.quantita)
        .bind(&voce.unita_simbolo)
        .execute(&mut *tx)
        .await
        .context("Impossibile inserire una voce generata")?;
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
            let esito = cambia_intervallo(&pool, lista.id, "2026-09-10", "2026-09-05").await;
            assert!(esito.is_err());
            let ok = cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07").await;
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
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07")
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
            cambia_intervallo(&pool, lista.id, "2026-09-01", "2026-09-07")
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
}

// ===========================================================================
// UI Telegram.
// ===========================================================================

#[derive(Debug, Clone, Default)]
pub struct ListaSpesaSessionStore {
    inner: Arc<Mutex<HashMap<i64, ListaSpesaConversationState>>>,
}

#[derive(Debug, Clone)]
enum ListaSpesaConversationState {
    AwaitingDescrizione,
    AwaitingQuantita { descrizione: String },
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

fn annulla_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        button("❌ Annulla", "lista_spesa:add:cancel"),
        button("🏠 Menù principale", "menu:main"),
    ]])
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

fn testo_scelta_descrizione() -> &'static str {
    "➕ Aggiungi voce manuale\n\nScrivi la descrizione (es. \"Detersivo piatti\")."
}

fn testo_scelta_quantita(descrizione: &str) -> String {
    format!(
        "➕ {descrizione}\n\nScrivi quantità e unità (es. \"500 g\"), oppure scegli senza quantità."
    )
}

async fn invalid(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(chat_id, "⚠️ Pulsante non valido o non più disponibile.")
        .reply_markup(nav_markup("lista_spesa:menu"))
        .await?;
    Ok(())
}

async fn expired(bot: &Bot, chat_id: ChatId) -> ResponseResult<()> {
    bot.send_message(
        chat_id,
        "ℹ️ Questa operazione non è più attiva. Riapri la lista.",
    )
    .reply_markup(nav_markup("lista_spesa:menu"))
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
    if novita::serve_badge(NOVITA_CHIAVE, &viste) {
        novita::tutorial_per(NOVITA_CHIAVE).map(str::to_string)
    } else {
        None
    }
}

async fn segna_vista_novita(pool: &SqlitePool) {
    if let Some(utente_id) = crate::identity::current_actor().utente_id {
        let _ = novita::segna_vista(pool, utente_id, NOVITA_CHIAVE).await;
    }
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
        show_lista(bot, chat_id, pool, None).await?;
        return Ok(true);
    }
    if let Some(raw) = data.strip_prefix("lista_spesa:page:") {
        let pagina = raw.parse::<i64>().unwrap_or(0).max(0);
        show_lista_pagina(bot, chat_id, pool, pagina, None).await?;
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
    if data == "lista_spesa:refresh" {
        match aggiorna_lista_attiva(pool).await {
            Ok(numero) => {
                let messaggio = format!(
                    "🔄 Lista aggiornata: {numero} {}.",
                    if numero == 1 {
                        "voce generata"
                    } else {
                        "voci generate"
                    }
                );
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
        sessions.set(chat_id.0, ListaSpesaConversationState::AwaitingDescrizione);
        bot.send_message(chat_id, testo_scelta_descrizione())
            .reply_markup(annulla_keyboard())
            .await?;
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
        match cambia_intervallo(pool, lista.id, &inizio, date).await {
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
    rows.push(nav_row("lista_spesa:menu"));

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

async fn show_lista(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    notice: Option<&str>,
) -> ResponseResult<()> {
    show_lista_pagina(bot, chat_id, pool, 0, notice).await
}

async fn show_lista_pagina(
    bot: &Bot,
    chat_id: ChatId,
    pool: &SqlitePool,
    pagina_richiesta: i64,
    notice: Option<&str>,
) -> ResponseResult<()> {
    let lista = match trova_o_crea_lista_attiva(pool).await {
        Ok(lista) => lista,
        Err(errore) => {
            tracing::warn!(?errore, "Impossibile aprire la lista della spesa");
            bot.send_message(chat_id, "⚠️ Non riesco ad aprire la lista della spesa.")
                .reply_markup(nav_markup("food:menu"))
                .await?;
            return Ok(());
        }
    };
    let voci = carica_voci(pool, lista.id).await.unwrap_or_default();
    let totale = voci.len() as i64;
    let pagina = liste::pagina_valida(pagina_richiesta, totale);
    let scarto = liste::scarto(pagina) as usize;
    let fine_slice = (scarto + liste::VOCI_PER_PAGINA).min(voci.len());
    let pagina_voci = if scarto < voci.len() {
        &voci[scarto..fine_slice]
    } else {
        &[]
    };
    let comprate = voci.iter().filter(|voce| voce.comprato != 0).count();

    let tutorial = tutorial_da_mostrare(pool).await;

    let mut testo = String::new();
    if let Some(notice) = notice {
        testo.push_str(notice);
        testo.push_str("\n\n");
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
    if totale == 0 {
        testo.push_str(
            "\nNessuna voce nella lista.\nUsa 🔄 Aggiorna lista per generarla dai pasti pianificati, oppure aggiungine una manuale.\n",
        );
    } else {
        testo.push_str(&format!("\n{comprate}/{totale} comprate\n"));
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for voce in pagina_voci {
        let icona = if voce.comprato != 0 { "✅" } else { "☐" };
        let quantita = match (voce.quantita, &voce.unita_simbolo) {
            (Some(valore), Some(unita)) => format!(" · {} {unita}", formatta_quantita(valore)),
            _ => String::new(),
        };
        rows.push(vec![button(
            format!("{icona} {}{quantita}", liste::tronca(&voce.descrizione, 40)),
            format!("lista_spesa:toggle:{}", voce.id),
        )]);
    }
    if let Some(riga) = liste::riga_paginazione_da_totale(pagina, totale, "lista_spesa:noop", |p| {
        format!("lista_spesa:page:{p}")
    }) {
        rows.push(riga);
    }
    rows.push(vec![button("🔄 Aggiorna lista", "lista_spesa:refresh")]);
    rows.push(vec![button("➕ Aggiungi voce manuale", "lista_spesa:add")]);
    rows.push(vec![button(
        "🗓️ Cambia intervallo",
        "lista_spesa:range:start",
    )]);
    rows.push(nav_row("food:menu"));

    // Solo dopo aver davvero costruito la schermata: chi la raggiunge la sta
    // vedendo per davvero, non solo aprendo un menù intermedio (C14).
    segna_vista_novita(pool).await;

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}
