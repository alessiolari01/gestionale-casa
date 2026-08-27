-- Step 7.2G.6 — esportazione amministrativa dei Miglioramenti dal bot.
-- Migration append-only: tutte le migration precedenti sono già applicate e IMMUTABILI.
--
-- Implementa il miglioramento attivo #8 dell'handoff DEFINITIVO:
-- - generazione ZIP sanitizzato direttamente da Telegram;
-- - invio del documento all'amministratore principale;
-- - cancellazione locale solo dopo conferma esplicita del download;
-- - pulizia di sicurezza degli export temporanei orfani.
--
-- #7 resta Da fare.
-- #9 è una funzione futura e resta Da fare per essere riportata nella roadmap finale.

UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 8
  AND instr(lower(descrizione), 'esporta miglioramenti') > 0
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
       'Esporta miglioramenti direttamente dal bot',
       'Apri Miglioramenti come amministratore principale e premi Esporta miglioramenti. Il bot deve creare e inviare uno ZIP sanitizzato con stato Git, miglioramenti attivi/archiviati, schema e allegati. Scarica il documento e verifica che si apra. Prima della conferma la copia locale deve restare disponibile; dopo Ho scaricato il file la copia locale deve essere eliminata e il documento Telegram deve essere rimosso alla navigazione successiva. Verifica inoltre che un utente normale non possa avviare l’esportazione.',
       '📦 Esporta miglioramenti',
       'improve:export',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti
WHERE id = 8
  AND stato = 'fatto';
