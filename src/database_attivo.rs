//! Quale database usa il bot: quello reale o quello di prova.
//!
//! Deciso con Alessio il 6 ottobre 2026: i collaudi non si fanno più sui
//! dati veri, che poi andavano ripristinati da un backup (e ogni ripristino
//! si portava dietro qualcosa di vecchio, come il messaggio "offline"). Il
//! bot è uno solo e i database due: `gestionale.db` per l'uso di tutti i
//! giorni, `prova.db` per i collaudi. Il catalogo (alimenti, prodotti,
//! ricette comuni) nasce dalle migration, quindi è uguale in tutti e due.
//!
//! La scelta sta in un file, letto all'avvio: per cambiarla il bot scrive il
//! file, si spegne e il guardiano lo riaccende (pulsante in 🛠️
//! Amministrazione).

use std::path::Path;
use std::sync::OnceLock;

/// Il file con la scelta: contiene `prova`, altrimenti vale il reale.
pub const FILE_SCELTA: &str = "data/run/database_attivo";
/// Il database di prova, accanto a quello reale.
pub const URL_PROVA_PREDEFINITO: &str = "sqlite://data/db/prova.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Database {
    Reale,
    Prova,
}

static ATTIVO: OnceLock<Database> = OnceLock::new();

impl Database {
    /// Legge la scelta. Un file che manca, vuoto o con altro dentro vale
    /// il reale: nel dubbio si lavora sui dati veri, come prima.
    pub fn leggi(file: &Path) -> Self {
        match std::fs::read_to_string(file) {
            Ok(testo) if testo.trim().eq_ignore_ascii_case("prova") => Database::Prova,
            _ => Database::Reale,
        }
    }

    pub fn scrivi(self, file: &Path) -> std::io::Result<()> {
        if let Some(cartella) = file.parent() {
            std::fs::create_dir_all(cartella)?;
        }
        let testo = match self {
            Database::Reale => "reale\n",
            Database::Prova => "prova\n",
        };
        std::fs::write(file, testo)
    }

    pub fn altro(self) -> Self {
        match self {
            Database::Reale => Database::Prova,
            Database::Prova => Database::Reale,
        }
    }

    /// L'indirizzo del database da aprire. `reale` è quello della
    /// configurazione (`DATABASE_URL`). La prova, se `DATABASE_PROVA_URL`
    /// non dice altro, è `prova.db` nella stessa cartella del reale: così
    /// segue il reale ovunque sia configurato (sull'S9 è
    /// `./data/db/gestionale.db`, senza `sqlite://`).
    pub fn url(self, reale: &str) -> String {
        match self {
            Database::Reale => reale.to_string(),
            Database::Prova => std::env::var("DATABASE_PROVA_URL")
                .ok()
                .filter(|url| !url.trim().is_empty())
                .unwrap_or_else(|| match reale.rfind('/') {
                    Some(fine_cartella) => format!("{}prova.db", &reale[..=fine_cartella]),
                    None => URL_PROVA_PREDEFINITO.to_string(),
                }),
        }
    }

    /// Il pulsante che porta all'**altro** database (Alessio, 6 ottobre
    /// 2026: dice dove ti porta, non dove sei).
    pub fn pulsante_per_cambiare(self) -> &'static str {
        match self {
            Database::Reale => "🧪 Carica database di prova",
            Database::Prova => "🏠 Carica database reale",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Database::Reale => "reale",
            Database::Prova => "di prova",
        }
    }

    /// Il database scelto all'avvio di questo processo.
    pub fn attivo() -> Self {
        *ATTIVO.get().unwrap_or(&Database::Reale)
    }

    pub fn imposta_attivo(self) {
        let _ = ATTIVO.set(self);
    }
}

/// Prepara il cambio: la scelta nuova e il segnale per il guardiano,
/// `riavvio.richiesto`, che dice che lo spegnimento è voluto: va riacceso
/// subito e non conta come una caduta (`scripts/guardiano-bot.sh`). Lo
/// spegnimento lo chiede poi chi chiama.
pub fn prepara_cambio(cartella_run: &Path, verso: Database) -> std::io::Result<()> {
    verso.scrivi(&cartella_run.join("database_attivo"))?;
    std::fs::write(cartella_run.join("riavvio.richiesto"), verso.nome())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cartella(nome: &str) -> std::path::PathBuf {
        let cartella = std::env::temp_dir().join(format!(
            "gestionale_db_attivo_{nome}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cartella);
        std::fs::create_dir_all(&cartella).expect("cartella");
        cartella
    }

    #[test]
    fn senza_file_si_usa_il_reale() {
        let cartella = cartella("senza");
        assert_eq!(
            Database::leggi(&cartella.join("database_attivo")),
            Database::Reale
        );
    }

    #[test]
    fn la_scelta_scritta_si_rilegge() {
        let cartella = cartella("rilegge");
        let file = cartella.join("database_attivo");
        Database::Prova.scrivi(&file).expect("scrivo prova");
        assert_eq!(Database::leggi(&file), Database::Prova);
        Database::Reale.scrivi(&file).expect("scrivo reale");
        assert_eq!(Database::leggi(&file), Database::Reale);
    }

    #[test]
    fn un_file_strano_vale_il_reale() {
        let cartella = cartella("strano");
        let file = cartella.join("database_attivo");
        std::fs::write(&file, "boh").expect("file");
        assert_eq!(Database::leggi(&file), Database::Reale);
        std::fs::write(&file, " Prova\n").expect("file");
        assert_eq!(Database::leggi(&file), Database::Prova);
    }

    #[test]
    fn la_prova_ha_il_suo_file_accanto_al_reale() {
        assert_eq!(
            Database::Reale.url("./data/db/gestionale.db"),
            "./data/db/gestionale.db"
        );
        let prova = Database::Prova.url("./data/db/gestionale.db");
        assert!(prova.ends_with("prova.db"), "{prova}");
        assert_ne!(prova, "./data/db/gestionale.db");
    }

    #[test]
    fn il_pulsante_dice_dove_porta() {
        assert_eq!(Database::Reale.altro(), Database::Prova);
        assert_eq!(Database::Prova.altro(), Database::Reale);
        assert!(Database::Reale
            .pulsante_per_cambiare()
            .contains("Carica database di prova"));
        assert!(Database::Prova
            .pulsante_per_cambiare()
            .contains("Carica database reale"));
    }

    #[test]
    fn il_cambio_lascia_la_scelta_e_il_segnale_al_guardiano() {
        let cartella = cartella("cambio");
        prepara_cambio(&cartella, Database::Prova).expect("cambio");
        assert_eq!(
            Database::leggi(&cartella.join("database_attivo")),
            Database::Prova
        );
        assert!(cartella.join("riavvio.richiesto").exists());
    }
}
