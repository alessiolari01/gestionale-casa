//! Primitive comuni a ogni lista paginata del bot.
//!
//! Esiste per la stessa ragione per cui esiste un solo `calendario.rs`: prima
//! di questo modulo la riga di paginazione era riscritta a mano in sei posti
//! (alimenti, ricette, storico, miglioramenti, archivio miglioramenti, e le
//! liste del planner), con quattro etichette diverse per lo stesso pulsante.
//! Una convenzione ricopiata a mano in sei posti non e' una convenzione: e'
//! sei posti che prima o poi divergono, ed e' esattamente quello che era
//! successo.
//!
//! Le regole applicate qui sono C6 e C7 di `docs/convenzioni-telegram.md`.

use teloxide::types::InlineKeyboardButton;

/// Voci per pagina. C6: cinque, ovunque.
pub const VOCI_PER_PAGINA: usize = 5;

/// Sopra questa soglia una lista si cerca invece di sfogliarla, quindi
/// `🔎 Cerca` diventa la prima azione della sezione (C6).
pub const SOGLIA_RICERCA: i64 = 20;

/// Numero di pagine per un totale di voci. Una lista vuota ha comunque una
/// pagina, quella che spiega che non c'e' niente (C8).
pub fn totale_pagine(totale: i64) -> i64 {
    if totale <= 0 {
        1
    } else {
        // Il conto si fa senza segno: `div_ceil` sugli interi con segno e'
        // ancora instabile sulla toolchain dell'S9, e la divisione scritta a
        // mano fa scattare `manual_div_ceil` sulla Clippy della CI, che e'
        // piu' recente (punto 1 dei punti aperti di STATO.md). Qui `totale`
        // e' gia' positivo, quindi la conversione e' sicura.
        (totale as u64).div_ceil(VOCI_PER_PAGINA as u64) as i64
    }
}

/// Riporta una pagina dentro i limiti reali della lista.
pub fn pagina_valida(pagina: i64, totale: i64) -> i64 {
    pagina.max(0).min(totale_pagine(totale) - 1)
}

/// Offset SQL della pagina.
pub fn scarto(pagina: i64) -> i64 {
    pagina.max(0).saturating_mul(VOCI_PER_PAGINA as i64)
}

/// Intestazione di una lista paginata (C1 + C6).
///
/// Il titolo porta sempre il totale, cosi' l'utente sa quanto e' grande la
/// lista prima di sfogliarla; sotto sta la posizione. Non c'e' altro: le voci
/// stanno sui pulsanti e il testo non le ripete.
///
/// ```text
/// 📋 Elenco alimenti · 422
/// Pagina 1/85
/// ```
pub fn intestazione(titolo: &str, totale: i64, pagina: i64) -> String {
    format!(
        "{titolo} · {totale}\nPagina {}/{}",
        pagina + 1,
        totale_pagine(totale)
    )
}

/// Riga di paginazione unica di tutto il bot (C6).
///
/// `⬅️ Precedente | n/tot | Successiva ➡️`, dove `n/tot` non e' premibile.
/// Restituisce `None` quando c'e' una pagina sola: una riga di navigazione fra
/// una pagina e se stessa e' rumore.
///
/// Prende **il numero di pagine**, non il totale delle voci, ed e' una scelta
/// pagata: la prima versione prendeva solo il totale e dava per scontate cinque
/// voci per pagina, cosi' i chiamanti che contano in pagine — il selettore dei
/// filtri dello storico ne mostra sette, la descrizione lunga di un
/// miglioramento e' spezzata a caratteri — non potevano usarla e sono rimasti
/// con la loro riga scritta a mano. Una primitiva che non entra dove serve non
/// unifica niente.
///
/// `callback_pagina` costruisce il callback della pagina richiesta, perche'
/// ogni modulo ha il suo formato; `callback_inerte` e' il no-op del modulo.
pub fn riga_paginazione(
    pagina: i64,
    pagine: i64,
    callback_inerte: &str,
    callback_pagina: impl Fn(i64) -> String,
) -> Option<Vec<InlineKeyboardButton>> {
    if pagine <= 1 {
        return None;
    }

    let mut riga = Vec::new();
    if pagina > 0 {
        riga.push(InlineKeyboardButton::callback(
            "⬅️ Precedente".to_string(),
            callback_pagina(pagina - 1),
        ));
    }
    riga.push(InlineKeyboardButton::callback(
        format!("{}/{}", pagina + 1, pagine),
        callback_inerte.to_string(),
    ));
    if pagina + 1 < pagine {
        riga.push(InlineKeyboardButton::callback(
            "Successiva ➡️".to_string(),
            callback_pagina(pagina + 1),
        ));
    }
    Some(riga)
}

/// Come `riga_paginazione`, per chi conosce il totale delle voci invece del
/// numero di pagine, con le cinque voci per pagina della regola.
pub fn riga_paginazione_da_totale(
    pagina: i64,
    totale: i64,
    callback_inerte: &str,
    callback_pagina: impl Fn(i64) -> String,
) -> Option<Vec<InlineKeyboardButton>> {
    riga_paginazione(
        pagina,
        totale_pagine(totale),
        callback_inerte,
        callback_pagina,
    )
}

/// Etichetta di un pulsante che porta a una lista, con il conteggio (C7).
///
/// `📋 Elenco alimenti · 422`. Un pulsante che porta a una lista vuota lo
/// dichiara prima di essere premuto, invece di farlo scoprire premendolo.
pub fn etichetta_con_conteggio(etichetta: &str, totale: i64) -> String {
    format!("{etichetta} · {totale}")
}

/// Vero quando la lista e' abbastanza lunga da richiedere la ricerca come
/// azione principale (C6).
pub fn si_cerca_invece_di_sfogliare(totale: i64) -> bool {
    totale > SOGLIA_RICERCA
}

/// Taglia un testo a `massimo` caratteri, aggiungendo il puntino di
/// sospensione solo se ha davvero tagliato.
pub fn tronca(valore: &str, massimo: usize) -> String {
    let mut caratteri = valore.chars();
    let inizio: String = caratteri.by_ref().take(massimo).collect();
    if caratteri.next().is_some() {
        format!("{inizio}…")
    } else {
        inizio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn etichette(riga: &Option<Vec<InlineKeyboardButton>>) -> Vec<String> {
        riga.as_ref()
            .map(|riga| riga.iter().map(|b| b.text.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn totale_pagine_arrotonda_per_eccesso() {
        assert_eq!(totale_pagine(0), 1);
        assert_eq!(totale_pagine(1), 1);
        assert_eq!(totale_pagine(5), 1);
        assert_eq!(totale_pagine(6), 2);
        assert_eq!(totale_pagine(422), 85);
    }

    #[test]
    fn pagina_valida_rientra_nei_limiti() {
        assert_eq!(pagina_valida(-3, 422), 0);
        assert_eq!(pagina_valida(900, 422), 84);
        assert_eq!(pagina_valida(0, 0), 0);
    }

    #[test]
    fn intestazione_porta_totale_e_posizione() {
        assert_eq!(
            intestazione("📋 Elenco alimenti", 422, 0),
            "📋 Elenco alimenti · 422\nPagina 1/85"
        );
    }

    #[test]
    fn una_pagina_sola_non_ha_riga_di_navigazione() {
        let riga = riga_paginazione_da_totale(0, 4, "x:noop", |p| format!("x:{p}"));
        assert!(riga.is_none());
    }

    #[test]
    fn prima_pagina_non_offre_precedente() {
        let riga = riga_paginazione_da_totale(0, 422, "x:noop", |p| format!("x:{p}"));
        assert_eq!(etichette(&riga), vec!["1/85", "Successiva ➡️"]);
    }

    #[test]
    fn ultima_pagina_non_offre_successiva() {
        let riga = riga_paginazione_da_totale(84, 422, "x:noop", |p| format!("x:{p}"));
        assert_eq!(etichette(&riga), vec!["⬅️ Precedente", "85/85"]);
    }

    #[test]
    fn pagina_intermedia_ha_entrambe_le_frecce() {
        let riga = riga_paginazione_da_totale(1, 422, "x:noop", |p| format!("x:{p}"));
        assert_eq!(
            etichette(&riga),
            vec!["⬅️ Precedente", "2/85", "Successiva ➡️"]
        );
    }

    #[test]
    fn il_contatore_non_e_premibile() {
        let riga =
            riga_paginazione_da_totale(1, 422, "food:noop", |p| format!("food:list:page:{p}"))
                .unwrap();
        let callback = |b: &InlineKeyboardButton| match &b.kind {
            teloxide::types::InlineKeyboardButtonKind::CallbackData(data) => data.clone(),
            _ => String::new(),
        };
        assert_eq!(callback(&riga[1]), "food:noop");
        assert_eq!(callback(&riga[0]), "food:list:page:0");
        assert_eq!(callback(&riga[2]), "food:list:page:2");
    }

    #[test]
    fn la_riga_accetta_anche_un_numero_di_pagine_diverso_da_cinque_per_pagina() {
        // Il selettore dei filtri dello storico mostra sette voci per pagina:
        // se la riga sapesse contare solo a cinque, resterebbe fuori.
        let riga = riga_paginazione(1, 3, "history:noop", |p| format!("h:p:{p}"));
        assert_eq!(
            etichette(&riga),
            vec!["⬅️ Precedente", "2/3", "Successiva ➡️"]
        );
    }

    #[test]
    fn il_conteggio_sta_sul_pulsante() {
        assert_eq!(
            etichetta_con_conteggio("📋 Elenco alimenti", 422),
            "📋 Elenco alimenti · 422"
        );
        assert_eq!(
            etichetta_con_conteggio("🟡 Da approvare", 0),
            "🟡 Da approvare · 0"
        );
    }

    #[test]
    fn la_soglia_della_ricerca_e_venti() {
        assert!(!si_cerca_invece_di_sfogliare(20));
        assert!(si_cerca_invece_di_sfogliare(21));
    }

    #[test]
    fn tronca_solo_quando_serve() {
        assert_eq!(tronca("Avena", 22), "Avena");
        assert_eq!(tronca("abcdef", 3), "abc…");
        // Il taglio conta i caratteri, non i byte: gli accenti non lo rompono.
        assert_eq!(tronca("caffè", 5), "caffè");
    }
}
