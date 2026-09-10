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
        assert_eq!(formatta_quantita(133.33), "133.33");
        assert_eq!(formatta_quantita(0.3), "0.3");
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
    #[allow(dead_code)]
    pub ordinamento: i64,
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

/// Ordine indipendente dallo stato comprato (deciso con Alessio il 9
/// settembre 2026, dopo averlo visto dal vivo): prima l'ordine dipendeva da
/// `comprato ASC`, quindi spuntare una voce la faceva saltare in fondo alla
/// lista -- ora `ordinamento` è la sola chiave, e spuntare/deselezionare
/// non sposta più nulla. Vedi anche `sposta_voce`.
pub async fn carica_voci(pool: &SqlitePool, lista_id: i64) -> anyhow::Result<Vec<VoceListaSpesa>> {
    sqlx::query_as(
        "SELECT id, origine, alimento_id, descrizione, quantita, unita_simbolo, comprato, \
                ordinamento \
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
            // rifonde subito, senza aspettare.
            if let Some(lista) = trova_per_id(pool, lista_id).await? {
                aggiorna_lista(pool, &lista).await?;
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
         FROM liste_spesa_voci WHERE id = ? AND origine = 'generato' AND comprato = 1",
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
           AND id <> ? AND unita_simbolo = ?",
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
    fn identita(&self) -> IdentitaCatalogo {
        match self {
            Self::Alimento { id, .. } => IdentitaCatalogo::Alimento(*id),
            Self::Prodotto { id, .. } => IdentitaCatalogo::Prodotto(*id),
        }
    }

    fn etichetta(&self) -> String {
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
async fn cerca_nel_catalogo(
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
async fn alimento_visibile_per_id(
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
async fn prodotto_visibile_per_id(
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

/// Calcola il fresco dell'aggregazione, meno quanto già coperto da voci
/// comprate, senza scrivere nulla -- condiviso da `aggiorna_lista` (che lo
/// scrive per davvero) e da `serve_aggiornamento` (che lo confronta
/// soltanto con quanto già in lista, per decidere se mostrare "🔄 Aggiorna
/// lista").
async fn calcola_fresche(
    pool: &SqlitePool,
    lista: &ListaSpesa,
) -> anyhow::Result<Vec<VoceGenerata>> {
    let fresche = fresche_grezze(pool, lista).await?;
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
    let fresche = fresche_grezze(pool, lista).await?;
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

    #[tokio::test]
    async fn eccessi_comprati_segnala_un_pasto_tolto_dal_planner() {
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
            if let Err(errore) = aggiorna_lista(pool, &lista).await {
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
            show_lista(bot, chat_id, pool, Some("✅ Voce aggiunta.")).await?;
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
        show_lista(bot, chat_id, pool, None).await?;
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
    if let Some(raw_id) = data.strip_prefix("lista_spesa:remove:manuale:") {
        let Some(voce_id) = raw_id.parse::<i64>().ok().filter(|value| *value > 0) else {
            invalid(bot, chat_id).await?;
            return Ok(true);
        };
        match rimuovi_voce_manuale(pool, voce_id).await {
            Ok(()) => {
                mostra_dopo_rimozione(bot, chat_id, pool, "✅ Voce rimossa.").await?;
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
    if let Some(raw_id) = data.strip_prefix("lista_spesa:remove:catalogo:") {
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
                    Ok(lista) => aggiorna_lista(pool, &lista).await,
                    Err(errore) => Err(errore),
                };
                if let Err(errore) = esito_refresh {
                    tracing::warn!(
                        ?errore,
                        aggiunta_id,
                        "Aggiornamento lista dopo rimozione aggiunta catalogo fallito"
                    );
                }
                mostra_dopo_rimozione(bot, chat_id, pool, "✅ Voce rimossa.").await?;
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
                .reply_markup(annulla_keyboard())
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
                .reply_markup(annulla_keyboard())
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
                .reply_markup(nav_markup("food:menu"))
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
    rows.push(vec![button("✅ Fine riordino", "lista_spesa:menu")]);

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
                .reply_markup(nav_markup("food:menu"))
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
                format!("lista_spesa:remove:{prefisso}:{}", voce.id),
            )]
        })
        .collect();
    rows.push(vec![button("⬅️ Indietro", "lista_spesa:menu")]);

    bot.send_message(chat_id, testo)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await?;
    Ok(())
}

/// Mostra la lista intera, senza paginazione (eccezione esplicita a C6,
/// deciso con Alessio dopo un collaudo dal vivo): a differenza di ogni
/// altra lista del bot, qui l'utente deve vedere tutte le voci insieme per
/// decidere cosa prendere prima e cosa dopo al supermercato -- spezzarla in
/// pagine da cinque negherebbe proprio lo scopo della schermata.
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
                .reply_markup(nav_markup("food:menu"))
                .await?;
            return Ok(());
        }
    };
    let voci = carica_voci(pool, lista.id).await.unwrap_or_default();
    let totale = voci.len();
    let comprate = voci.iter().filter(|voce| voce.comprato != 0).count();
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
        testo.push_str("\n⚠️ Hai già segnato più di quanto serve ora (vedi i pulsanti sotto).\n");
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for voce in &voci {
        let icona = if voce.comprato != 0 { "✅" } else { "☐" };
        let quantita = match (voce.quantita, &voce.unita_simbolo) {
            (Some(valore), Some(unita)) => format!(" · {} {unita}", formatta_quantita(valore)),
            _ => String::new(),
        };
        let eccesso = eccessi
            .iter()
            .find(|eccesso| {
                Some(&eccesso.unita_simbolo) == voce.unita_simbolo.as_ref()
                    && eccesso.nome.trim().to_lowercase() == voce.descrizione.trim().to_lowercase()
            })
            .map(|eccesso| {
                // A capo, non " · ": su una riga sola Telegram tronca il
                // testo con "…" invece di andare a capo da solo (visto da
                // Alessio dal vivo con "125 in ecc…") -- un "\n" fa
                // occupare al pulsante una riga in più invece di tagliare.
                format!(
                    "\n⚠️ {} in eccesso",
                    formatta_quantita(eccesso.quantita_eccesso)
                )
            })
            .unwrap_or_default();
        rows.push(vec![button(
            format!(
                "{icona} {}{quantita}{eccesso}",
                liste::tronca(&voce.descrizione, 40)
            ),
            format!("lista_spesa:toggle:{}", voce.id),
        )]);
    }
    if serve_refresh {
        rows.push(vec![button("🔄 Aggiorna lista", "lista_spesa:refresh")]);
    }
    rows.push(vec![button("➕ Aggiungi voce manuale", "lista_spesa:add")]);
    if voci.len() > 1 {
        rows.push(vec![button("↕️ Riordina lista", "lista_spesa:reorder")]);
    }
    if ci_sono_voci_rimovibili {
        rows.push(vec![button("🗑️ Rimuovi voci", "lista_spesa:remove")]);
    }
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
