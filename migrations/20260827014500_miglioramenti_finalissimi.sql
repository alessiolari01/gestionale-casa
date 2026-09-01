-- Step 7.2G.4 — ultimi due miglioramenti prima della documentazione.
-- Migration append-only: tutte le migration precedenti sono già applicate e IMMUTABILI.
--
-- Implementa #29 e #30 dell'handoff FINALISSIMO del 27/08/2026.
-- #7 resta volutamente Da fare per uno step separato su reset/eliminazione account,
-- proprietà, membership, storico e permessi.

UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (29, 30)
  AND EXISTS (
      SELECT 1
      FROM utenti u
      WHERE u.id = miglioramenti.autore_utente_id
        AND u.ruolo_sistema = 'admin'
        AND u.amministratore_principale = 1
  )
  AND instr(lower(descrizione), 'prova') = 0;

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Annulla modifica testo conserva la sezione',
       'Apri Fatti da verificare o un’altra lista Miglioramenti, entra in un dettaglio, premi Modifica testo e poi Annulla. Devi tornare alla stessa sezione/lista e pagina da cui provenivi, non al menu generico Miglioramenti.',
       '✅ Fatti da verificare',
       'improve:list:done:0',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 29 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Migliora mostra la sezione corrente',
       'Naviga in più aree del gestionale e premi Migliora. Nel contesto deve comparire esplicitamente la voce Sezione, oltre alla schermata e alle azioni recenti; per esempio Alimentazione › Ricette, Miglioramenti, Oggetti o Storico.',
       '🏠 Apri Menù principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 30 AND stato = 'fatto';
