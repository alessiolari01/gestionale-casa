-- Step 7.2G.2 — contesto Migliora globale e rifiniture backlog.
-- Migration append-only: le migration precedenti sono già applicate e IMMUTABILI.

-- Il contesto prodotto dal pulsante globale 💡 Migliora viene salvato separato
-- dalla descrizione dell'utente, così resta consultabile anche dopo l'archivio.
ALTER TABLE miglioramenti ADD COLUMN contesto TEXT;
ALTER TABLE miglioramenti_archivio ADD COLUMN contesto TEXT;

-- Questo aggiornamento implementa i miglioramenti amministrativi 14-21 del
-- nuovo handoff. Restano come "fatto" finché l'amministratore principale non
-- li collauda e li archivia manualmente.
UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (14, 15, 16, 17, 18, 19, 20, 21)
  AND EXISTS (
      SELECT 1
      FROM utenti u
      WHERE u.id = miglioramenti.autore_utente_id
        AND u.ruolo_sistema = 'admin'
        AND u.amministratore_principale = 1
  )
  AND instr(lower(descrizione), 'prova') = 0;

-- I piani usano solo testo leggibile e pulsanti reali: nessun /comando viene
-- esposto nell'interfaccia Telegram.
INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Aggiornamento automatico Alimenti',
       'Apri Alimenti e verifica che il pulsante Aggiorna non esista più. Entra e torna nell’elenco dopo una modifica: i dati devono essere riletti automaticamente.',
       '🥕 Apri Alimenti',
       'food:foods',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 14 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Prima scelta ingrediente',
       'Apri la ricerca ricette per ingredienti. Quando non hai ancora selezionato nulla deve comparire Categorie, ma non il pulsante Aggiungi ingrediente. Dopo la prima selezione il pulsante può comparire.',
       '🥕 Cerca per ingredienti',
       'recipe:find',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 15 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Ricette: ricerca per categorie',
       'Nel menu Ricette verifica che il vecchio Cerca sia sostituito da Cerca per categorie, affiancato a Cerca per ingredienti. La selezione per categorie deve mantenere la logica esistente.',
       '🍳 Apri Ricette',
       'recipe:menu',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 16 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Stato dopo il collaudo',
       'Verifica un miglioramento Fatto. Dopo la conferma deve risultare Verificato · da archiviare e non deve più mostrare il pulsante Verifica miglioramento.',
       '✅ Apri Fatti da verificare',
       'improve:list:done:0',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 17 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Modifica riapre il lavoro',
       'Su un miglioramento Fatto modifica il testo oppure aggiungi/rimuovi uno screenshot. Deve tornare Da fare e perdere l’eventuale esito di collaudo. Verifica e archiviazione non devono causare questo reset.',
       '✅ Apri Fatti da verificare',
       'improve:list:done:0',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 18 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Niente comandi tecnici a video',
       'Apri Miglioramenti e controlla che l’interfaccia usi pulsanti leggibili e non mostri stringhe tecniche con slash. Controlla anche i nuovi messaggi di verifica.',
       '💡 Apri Miglioramenti',
       'improve:menu',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 19 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Pulsante Migliora contestuale',
       'Naviga in più sezioni e verifica che ogni messaggio del bot abbia 💡 Migliora. Premilo dopo alcuni tocchi: il messaggio successivo deve descrivere schermata e azioni recenti, e deve essere l’unico senza un altro pulsante Migliora. Poi invia testo e, facoltativamente, screenshot.',
       '🏠 Apri Menu principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 20 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Elimina tutti gli scartati',
       'Apri Scartati. Deve esserci Elimina tutti gli scartati; premendolo deve comparire una seconda conferma. Annulla una volta, poi ripeti e conferma solo se vuoi davvero svuotare la lista.',
       '❌ Apri Scartati',
       'improve:list:discarded:0',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 21 AND stato = 'fatto';
