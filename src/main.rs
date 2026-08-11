//! Punto di ingresso del gestionale.
//!
//! Per ora questo file è solo uno scheletro: carica la configurazione e
//! prepara i punti di aggancio (database, bot, moduli) che verranno
//! implementati nei prossimi passi, a partire dallo schema dati core.

mod auth;
mod config;
mod db;
mod modules;

fn main() {
    // TODO: inizializzare il logging (tracing_subscriber).
    // TODO: caricare la configurazione da .env (config::Config::load()).
    // TODO: aprire la connessione al database e applicare le migrazioni
    //       (db::connect(&config.database_url)).
    // TODO: costruire il bot Telegram (teloxide::Bot::from_env()) e
    //       registrare i comandi dei singoli moduli.
    // TODO: avviare il dispatcher del bot (long polling).

    println!("Scheletro del progetto pronto. Prossimo passo: schema dati core.");
}
