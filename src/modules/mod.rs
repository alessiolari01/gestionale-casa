//! Moduli funzionali del gestionale.
//!
//! Ogni modulo espone i propri comandi Telegram e la logica di dominio
//! specifica. Lo schema dati di ciascuno è documentato in
//! `docs/moduli/<nome>.md` prima di essere implementato qui.
//!
//! Se un modulo cresce troppo per stare in un solo file, verrà convertito
//! in una sotto-cartella (es. `modules/ricette/`) senza cambiare il resto
//! della struttura del progetto.

// Step 6C.3A: contenitori integrati nella navigazione dei luoghi e nella creazione oggetti.
// Alcune API backend restano riservate ai successivi sotto-step 6C.3/6C.4.
pub mod alimentazione;
#[allow(dead_code)]
pub mod contenitori;
pub mod foto;
pub mod luoghi;
pub mod miglioramenti;
pub mod oggetti;
pub mod planner_alimentare;
pub mod planner_elenco;
pub mod porzioni;
pub mod porzioni_ingredienti;
pub mod porzioni_profili;
pub mod profili_alimentari;
pub mod ricette;
pub mod spazi_membri;
pub mod storico;
pub mod veicoli;
pub mod vestiti;
