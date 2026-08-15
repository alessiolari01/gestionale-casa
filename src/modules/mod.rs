//! Moduli funzionali del gestionale.
//!
//! Ogni modulo espone i propri comandi Telegram e la logica di dominio
//! specifica. Lo schema dati di ciascuno è documentato in
//! `docs/moduli/<nome>.md` prima di essere implementato qui.
//!
//! Se un modulo cresce troppo per stare in un solo file, verrà convertito
//! in una sotto-cartella (es. `modules/ricette/`) senza cambiare il resto
//! della struttura del progetto.

pub mod foto;
pub mod luoghi;
pub mod oggetti;
pub mod ricette;
pub mod storico;
pub mod veicoli;
pub mod vestiti;
