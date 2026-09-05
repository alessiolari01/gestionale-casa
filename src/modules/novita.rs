//! Meccanismo del badge "🆕": da questa funzionalità in avanti, ogni pezzo
//! di bot nuovo o modificato in modo significativo si dichiara qui (una
//! [`VoceNovita`] nel [`REGISTRO`]), con l'eventuale genitore nel menù per
//! far salire il badge fino al menù principale, e un tutorial opzionale
//! mostrato la prima volta che l'utente arriva sulla schermata specifica.
//!
//! Deciso con Alessio il 5 settembre 2026: si applica solo da qui in
//! avanti, nessun retrofit delle schermate esistenti -- [`REGISTRO`] parte
//! vuoto. "Cosa è nuovo" vive nel codice, non nel database: nel database
//! (`novita_lette`) si tiene traccia solo di chi ha già visto cosa, per
//! utente -- non un flag globale, perché più persone usano lo stesso bot e
//! ciascuna deve scoprire le novità ai propri tempi.

use std::collections::HashSet;

use sqlx::SqlitePool;

/// Una funzionalità dichiarata come "nuova o cambiata". `chiave` è un
/// identificatore stabile e interno (mai mostrato all'utente), `genitore`
/// è la chiave del pulsante di menù superiore che deve mostrare il badge
/// finché questa non è stata vista, `tutorial` è il testo anteposto alla
/// schermata la prima volta che l'utente ci arriva davvero.
#[derive(Debug, Clone, Copy)]
pub struct VoceNovita {
    pub chiave: &'static str,
    pub genitore: Option<&'static str>,
    pub tutorial: Option<&'static str>,
}

/// Registro delle novità attualmente segnalate. Resta vuoto finché non
/// arriva la prima funzionalità reale da dichiarare qui.
///
/// Primo caso reale (5 settembre 2026): l'allegato di un miglioramento
/// accetta anche un video, non solo una foto. Genitore `"improve_menu"` è
/// il pulsante `📋 Miglioramenti` del menù principale (callback
/// `improve:menu` in `oggetti::main_menu_keyboard`) -- non serve una sua
/// voce separata nel registro: `ha_antenato_in` legge il campo `genitore`
/// direttamente sulla foglia, senza dover risalire oltre un livello.
pub const REGISTRO: &[VoceNovita] = &[VoceNovita {
    chiave: "miglioramenti_allegato_video",
    genitore: Some("improve_menu"),
    tutorial: Some(
        "🆕 Ora puoi allegare anche un video, non solo una foto. Prova a \
         mandarne uno di esempio: alla fine potrai scegliere se tenerlo o \
         eliminarlo.",
    ),
}];

/// Vero se nessun'altra voce del registro ha `chiave` come genitore: cioè
/// se `chiave` è una funzionalità vera e propria (una schermata che un
/// utente visita e su cui scatta [`segna_vista`]), non solo un nodo usato
/// per comporre la catena verso il menù superiore.
///
/// La distinzione conta: solo le foglie vengono mai segnate come "viste"
/// da qualcuno (nessuna schermata chiama `segna_vista` per un nodo
/// puramente intermedio come "ricette", usato solo per risalire a
/// "alimentazione"). Contare anche i nodi intermedi come "da vedere"
/// terrebbe il badge acceso per sempre, anche a foglie tutte viste --
/// trovato scrivendo il primo test di questo modulo, non a tavolino.
fn e_foglia(registro: &[VoceNovita], chiave: &str) -> bool {
    !registro.iter().any(|voce| voce.genitore == Some(chiave))
}

/// Tutte le foglie del registro che sono `chiave` stessa oppure hanno
/// `chiave` come antenato (genitore, o genitore del genitore, ...). Serve
/// a calcolare se un pulsante di menù, che spesso corrisponde a un nodo
/// intermedio e non a una singola funzionalità, deve mostrare il badge.
fn discendenti_o_se_stessa_in<'a>(registro: &'a [VoceNovita], chiave: &str) -> Vec<&'a str> {
    registro
        .iter()
        .filter(|voce| e_foglia(registro, voce.chiave))
        .filter(|voce| voce.chiave == chiave || ha_antenato_in(registro, voce.chiave, chiave))
        .map(|voce| voce.chiave)
        .collect()
}

fn ha_antenato_in(registro: &[VoceNovita], chiave: &str, antenato_cercato: &str) -> bool {
    let mut corrente = chiave;
    while let Some(voce) = registro.iter().find(|v| v.chiave == corrente) {
        match voce.genitore {
            Some(genitore) if genitore == antenato_cercato => return true,
            Some(genitore) => corrente = genitore,
            None => return false,
        }
    }
    false
}

/// Vero se, per l'insieme di chiavi già viste da un utente, almeno una
/// delle chiavi sotto `chiave` (o `chiave` stessa) non è ancora stata
/// vista -- cioè se quel pulsante di menù deve mostrare 🆕.
pub fn serve_badge(chiave: &str, viste: &HashSet<String>) -> bool {
    discendenti_o_se_stessa_in(REGISTRO, chiave)
        .into_iter()
        .any(|c| !viste.contains(c))
}

/// Antepone "🆕 " all'etichetta se il badge va mostrato.
pub fn etichetta_con_badge(base: &str, mostra: bool) -> String {
    if mostra {
        format!("🆕 {base}")
    } else {
        base.to_string()
    }
}

/// Il tutorial dichiarato per una chiave, se presente nel registro.
pub fn tutorial_per(chiave: &str) -> Option<&'static str> {
    REGISTRO
        .iter()
        .find(|voce| voce.chiave == chiave)
        .and_then(|voce| voce.tutorial)
}

/// Tutte le chiavi viste da un utente, lette in una sola query: chi
/// disegna un menù con più pulsanti la chiama una volta sola, non un
/// round-trip per pulsante.
pub async fn viste_da_utente(pool: &SqlitePool, utente_id: i64) -> sqlx::Result<HashSet<String>> {
    let righe: Vec<(String,)> =
        sqlx::query_as("SELECT chiave FROM novita_lette WHERE utente_id = ?")
            .bind(utente_id)
            .fetch_all(pool)
            .await?;
    Ok(righe.into_iter().map(|(chiave,)| chiave).collect())
}

/// Segna una chiave come vista da un utente. Ritorna `true` se è la prima
/// volta (la riga non esisteva già): serve a chi mostra la schermata per
/// sapere se deve anteporre il tutorial oppure no.
pub async fn segna_vista(pool: &SqlitePool, utente_id: i64, chiave: &str) -> sqlx::Result<bool> {
    let esito = sqlx::query(
        "INSERT INTO novita_lette (utente_id, chiave) VALUES (?, ?)
         ON CONFLICT (utente_id, chiave) DO NOTHING",
    )
    .bind(utente_id)
    .bind(chiave)
    .execute(pool)
    .await?;
    Ok(esito.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    const REGISTRO_TEST: &[VoceNovita] = &[
        VoceNovita {
            chiave: "alimentazione",
            genitore: None,
            tutorial: None,
        },
        VoceNovita {
            chiave: "ricette",
            genitore: Some("alimentazione"),
            tutorial: None,
        },
        VoceNovita {
            chiave: "ricette_procedimento",
            genitore: Some("ricette"),
            tutorial: Some("Tocca ogni passo per segnarlo fatto."),
        },
        VoceNovita {
            chiave: "planner_pasto_saltato",
            genitore: Some("alimentazione"),
            tutorial: None,
        },
    ];

    fn serve_badge_test(chiave: &str, viste: &HashSet<String>) -> bool {
        discendenti_o_se_stessa_in(REGISTRO_TEST, chiave)
            .into_iter()
            .any(|c| !viste.contains(c))
    }

    #[test]
    fn radice_mostra_badge_se_un_discendente_non_e_visto() {
        let viste = HashSet::new();
        assert!(serve_badge_test("alimentazione", &viste));
    }

    #[test]
    fn radice_resta_con_badge_finche_resta_un_ramo_non_visto() {
        let mut viste = HashSet::new();
        viste.insert("ricette_procedimento".to_string());
        // "planner_pasto_saltato" (altro ramo sotto "alimentazione") non è
        // ancora visto: il badge sulla radice deve restare.
        assert!(serve_badge_test("alimentazione", &viste));
    }

    #[test]
    fn radice_perde_il_badge_quando_ogni_foglia_e_vista() {
        let mut viste = HashSet::new();
        viste.insert("ricette_procedimento".to_string());
        viste.insert("planner_pasto_saltato".to_string());
        assert!(!serve_badge_test("alimentazione", &viste));
    }

    #[test]
    fn vedere_una_foglia_non_tocca_un_ramo_indipendente() {
        let mut viste = HashSet::new();
        viste.insert("planner_pasto_saltato".to_string());
        // "ricette" ha ancora "ricette_procedimento" non vista.
        assert!(serve_badge_test("ricette", &viste));
    }

    #[test]
    fn chiave_senza_voce_nel_registro_non_mostra_mai_badge() {
        let viste = HashSet::new();
        assert!(!serve_badge_test("modulo_senza_novita", &viste));
    }

    #[test]
    fn etichetta_antepone_il_badge_solo_se_richiesto() {
        assert_eq!(etichetta_con_badge("Ricette", true), "🆕 Ricette");
        assert_eq!(etichetta_con_badge("Ricette", false), "Ricette");
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
            .expect("foreign key");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration");
        pool
    }

    #[tokio::test]
    async fn segna_vista_e_idempotente_e_riportata_da_viste_da_utente() {
        let pool = test_pool().await;
        let utente_id = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Tester')")
            .execute(&pool)
            .await
            .expect("utente")
            .last_insert_rowid();

        let prima_volta = segna_vista(&pool, utente_id, "ricette_procedimento")
            .await
            .expect("segna vista");
        assert!(prima_volta);

        let seconda_volta = segna_vista(&pool, utente_id, "ricette_procedimento")
            .await
            .expect("segna vista di nuovo");
        assert!(!seconda_volta);

        let viste = viste_da_utente(&pool, utente_id).await.expect("viste");
        assert_eq!(viste.len(), 1);
        assert!(viste.contains("ricette_procedimento"));
    }

    #[tokio::test]
    async fn viste_da_utente_non_mischia_utenti_diversi() {
        let pool = test_pool().await;
        let utente_a = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Alessio')")
            .execute(&pool)
            .await
            .expect("utente a")
            .last_insert_rowid();
        let utente_b = sqlx::query("INSERT INTO utenti (nome_visualizzato) VALUES ('Altro')")
            .execute(&pool)
            .await
            .expect("utente b")
            .last_insert_rowid();

        segna_vista(&pool, utente_a, "ricette_procedimento")
            .await
            .expect("segna vista a");

        let viste_a = viste_da_utente(&pool, utente_a).await.expect("viste a");
        let viste_b = viste_da_utente(&pool, utente_b).await.expect("viste b");
        assert!(viste_a.contains("ricette_procedimento"));
        assert!(viste_b.is_empty());
    }
}
