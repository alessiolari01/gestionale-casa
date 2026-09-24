-- Ogni funzione del gestionale si puo' spegnere, per utente.
--
-- Chiesto da Alessio il 24 settembre 2026: "magari non voglio tracciare
-- costantemente il cibo". Un gestionale personale che obbliga a usare tutto
-- diventa un lavoro; spegnendo le Scorte spariscono la sezione, i suoi
-- automatismi e la sottrazione delle scorte dalla lista della spesa, e il
-- resto continua a funzionare.
--
-- Una riga per ogni funzione SPENTA: l'assenza vuol dire accesa. Cosi' non
-- serve popolare la tabella per gli utenti che ci sono gia', e una funzione
-- nuova nasce accesa per tutti senza altre migration.
--
-- Le tre preferenze che esistevano gia' (dispensa_ingresso_automatico,
-- lista_spesa_aggiornamento_automatico, mostra_legenda) restano dove sono:
-- la schermata delle impostazioni legge e scrive quelle colonne. Un fatto in
-- un posto solo.

CREATE TABLE funzioni_spente (
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    funzione TEXT NOT NULL,
    spenta_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (utente_id, funzione),
    CHECK (length(trim(funzione)) > 0)
);
