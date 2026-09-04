//! Sotto-step 5c del punto 6 del ciclo di automazione
//! (`docs/previsto/automazione-ciclo-sviluppo.md`): riepilogo, checklist e
//! conferma/rifiuto del collaudo guidato dopo uno swap.
//!
//! Deciso insieme ad Alessio il 4 settembre 2026: a pilotare questo
//! messaggio è il bot nuovo stesso, con bottoni normali gestiti dal suo
//! dispatcher — non l'agente orchestratore via API diretta come il
//! countdown. A differenza del countdown (che deve continuare ad
//! aggiornarsi anche col vecchio processo fermo), qui il bot nuovo è già
//! acceso e in ascolto su Telegram: non serve altro.
//!
//! Il contenuto (cosa è stato implementato + passi da provare) arriva da
//! un file scritto dall'agente prima dello swap, stesso canale a file già
//! usato per `RISERVATO` e per lo stato delle sessioni attive: niente
//! parametro nuovo per ogni pezzo.

use teloxide::types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup, MessageId};

/// Scritto dall'agente prima dello swap. Letto una sola volta all'avvio,
/// solo quando `RISERVATO=1`: un avvio normale non ha nulla da collaudare.
pub const FILE_RIEPILOGO: &str = "data/run/riepilogo_deploy.txt";

/// Scritto dal bot quando l'amministratore principale conferma o rifiuta,
/// letto dall'agente (sotto-step 5d) per sapere come proseguire: se
/// procedere al merge o innescare il rollback del sotto-step 5b.
pub const FILE_ESITO: &str = "data/run/esito_collaudo.txt";

const SEPARATORE_CHECKLIST: &str = "---CHECKLIST---";
const MAX_ETICHETTA_BOTTONE: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Riepilogo {
    pub testo: String,
    pub checklist: Vec<String>,
}

/// Separa riepilogo e checklist dal contenuto del file. Pura e testabile
/// senza filesystem: chi la chiama gestisce l'I/O (`leggi_riepilogo`).
/// `None` se il file non ha la forma attesa (separatore mancante,
/// riepilogo vuoto, o nessuna voce di checklist) — un file scritto male
/// non deve far crashare l'avvio del bot.
pub fn interpreta_riepilogo(contenuto: &str) -> Option<Riepilogo> {
    let (testo, checklist_grezza) = contenuto.split_once(SEPARATORE_CHECKLIST)?;
    let testo = testo.trim().to_string();
    let checklist: Vec<String> = checklist_grezza
        .lines()
        .map(str::trim)
        .filter(|riga| !riga.is_empty())
        .map(|riga| riga.trim_start_matches('-').trim().to_string())
        .filter(|riga| !riga.is_empty())
        .collect();

    if testo.is_empty() || checklist.is_empty() {
        return None;
    }
    Some(Riepilogo { testo, checklist })
}

pub async fn leggi_riepilogo(percorso: &str) -> Option<Riepilogo> {
    let contenuto = tokio::fs::read_to_string(percorso).await.ok()?;
    interpreta_riepilogo(&contenuto)
}

#[derive(Debug, Clone)]
pub struct StatoCollaudo {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub riepilogo: String,
    pub checklist: Vec<(String, bool)>,
}

impl StatoCollaudo {
    pub fn nuovo(chat_id: ChatId, message_id: MessageId, riepilogo: Riepilogo) -> Self {
        Self {
            chat_id,
            message_id,
            riepilogo: riepilogo.testo,
            checklist: riepilogo
                .checklist
                .into_iter()
                .map(|voce| (voce, false))
                .collect(),
        }
    }

    /// Spunta/despunta una voce per indice. `false` se l'indice non esiste
    /// (bottone di un messaggio vecchio, callback non più valido).
    pub fn alterna(&mut self, indice: usize) -> bool {
        match self.checklist.get_mut(indice) {
            Some((_, fatto)) => {
                *fatto = !*fatto;
                true
            }
            None => false,
        }
    }

    pub fn tutto_fatto(&self) -> bool {
        self.checklist.iter().all(|(_, fatto)| *fatto)
    }

    pub fn testo_messaggio(&self) -> String {
        let mut righe = vec![
            "🚀 Aggiornamento completato".to_string(),
            String::new(),
            self.riepilogo.clone(),
            String::new(),
            "Prova questi passi, poi spunta ogni voce:".to_string(),
        ];
        if !self.tutto_fatto() {
            righe.push(String::new());
            righe.push("Spunta tutti i passi per sbloccare la conferma.".to_string());
        }
        righe.join("\n")
    }

    pub fn tastiera(&self) -> InlineKeyboardMarkup {
        let mut righe: Vec<Vec<InlineKeyboardButton>> = self
            .checklist
            .iter()
            .enumerate()
            .map(|(indice, (voce, fatto))| {
                let segno = if *fatto { "✅" } else { "☐" };
                vec![InlineKeyboardButton::callback(
                    format!("{segno} {}", tronca(voce, MAX_ETICHETTA_BOTTONE)),
                    format!("collaudo:toggle:{indice}"),
                )]
            })
            .collect();
        if self.tutto_fatto() {
            righe.push(vec![
                InlineKeyboardButton::callback(
                    "✅ Confermo, funziona".to_string(),
                    "collaudo:conferma".to_string(),
                ),
                InlineKeyboardButton::callback(
                    "❌ Non funziona".to_string(),
                    "collaudo:rifiuta".to_string(),
                ),
            ]);
        }
        InlineKeyboardMarkup::new(righe)
    }
}

fn tronca(testo: &str, massimo: usize) -> String {
    if testo.chars().count() <= massimo {
        testo.to_string()
    } else {
        let troncato: String = testo.chars().take(massimo.saturating_sub(1)).collect();
        format!("{troncato}…")
    }
}

pub fn testo_confermato() -> &'static str {
    "✅ Collaudo confermato\n\nIl gestionale è di nuovo online per tutti."
}

pub fn testo_rifiutato() -> &'static str {
    "❌ Collaudo rifiutato\n\nResta in modalità manutenzione finché non arriva una versione corretta."
}

/// Scrive l'esito ("confermato" o "rifiutato") per l'agente orchestratore
/// (sotto-step 5d), sullo stesso schema a file già usato per lo stato
/// delle sessioni attive.
pub async fn scrivi_esito(esito: &str) -> std::io::Result<()> {
    if let Some(cartella) = std::path::Path::new(FILE_ESITO).parent() {
        tokio::fs::create_dir_all(cartella).await?;
    }
    tokio::fs::write(FILE_ESITO, esito).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_riepilogo_separa_testo_e_checklist() {
        let contenuto = "Aggiunta la schermata X.\nSeconda riga.\n---CHECKLIST---\n- primo passo\n- secondo passo\n";
        let riepilogo = interpreta_riepilogo(contenuto).expect("deve interpretarsi");
        assert_eq!(riepilogo.testo, "Aggiunta la schermata X.\nSeconda riga.");
        assert_eq!(riepilogo.checklist, vec!["primo passo", "secondo passo"]);
    }

    #[test]
    fn interpreta_riepilogo_ignora_righe_vuote_nella_checklist() {
        let contenuto = "Riepilogo.\n---CHECKLIST---\n\n- passo unico\n\n";
        let riepilogo = interpreta_riepilogo(contenuto).expect("deve interpretarsi");
        assert_eq!(riepilogo.checklist, vec!["passo unico"]);
    }

    #[test]
    fn interpreta_riepilogo_rifiuta_senza_separatore() {
        assert!(interpreta_riepilogo("solo testo, nessuna checklist").is_none());
    }

    #[test]
    fn interpreta_riepilogo_rifiuta_checklist_vuota() {
        assert!(interpreta_riepilogo("Riepilogo.\n---CHECKLIST---\n\n").is_none());
    }

    #[test]
    fn interpreta_riepilogo_rifiuta_riepilogo_vuoto() {
        assert!(interpreta_riepilogo("\n---CHECKLIST---\n- passo").is_none());
    }

    fn stato_di_prova() -> StatoCollaudo {
        StatoCollaudo::nuovo(
            ChatId(1),
            MessageId(1),
            Riepilogo {
                testo: "Test".to_string(),
                checklist: vec!["primo".to_string(), "secondo".to_string()],
            },
        )
    }

    #[test]
    fn tutto_fatto_diventa_vero_solo_quando_ogni_voce_e_spuntata() {
        let mut stato = stato_di_prova();
        assert!(!stato.tutto_fatto());
        stato.alterna(0);
        assert!(!stato.tutto_fatto());
        stato.alterna(1);
        assert!(stato.tutto_fatto());
    }

    #[test]
    fn alterna_su_indice_inesistente_non_fa_nulla_e_torna_falso() {
        let mut stato = stato_di_prova();
        assert!(!stato.alterna(99));
        assert!(!stato.tutto_fatto());
    }

    #[test]
    fn tastiera_offre_conferma_e_rifiuto_solo_a_checklist_completa() {
        let mut stato = stato_di_prova();
        assert_eq!(stato.tastiera().inline_keyboard.len(), 2);
        stato.alterna(0);
        stato.alterna(1);
        assert_eq!(stato.tastiera().inline_keyboard.len(), 3);
    }

    #[test]
    fn tronca_lunghi_taglia_e_lascia_stare_i_corti() {
        assert_eq!(tronca("corto", 40), "corto");
        let lungo = "a".repeat(50);
        let risultato = tronca(&lungo, 40);
        assert_eq!(risultato.chars().count(), 40);
        assert!(risultato.ends_with('…'));
    }
}
