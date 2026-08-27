-- Step 7.2G.5 — rifiniture finali prima della documentazione.
-- Migration append-only: tutte le migration precedenti sono già applicate e IMMUTABILI.
--
-- Chiude il miglioramento riaperto #30 e i nuovi #31-#32:
-- - contesto Migliora con sezione reale e destinazione delle azioni;
-- - persistenza del messaggio UI Telegram tra riavvii;
-- - spegnimento controllato dal pannello Amministrazione.
-- #7 resta volutamente Da fare per uno step dedicato a reset/eliminazione account.

CREATE TABLE telegram_ui_state (
    chat_id INTEGER PRIMARY KEY,
    active_message_id INTEGER NOT NULL CHECK (active_message_id > 0),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (30, 31, 32)
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
       'Migliora usa la sezione reale e le destinazioni',
       'Dal Menù principale premi Migliora: Sezione deve essere Menù principale, non Storico. Poi naviga in Oggetti, Ricette e Miglioramenti e premi Migliora. Ogni azione recente deve usare il nome reale del pulsante e indicare la sezione di destinazione, per esempio hai premuto Menù principale → Menù principale.',
       '🏠 Apri Menù principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 30 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Schermata singola sopravvive al riavvio',
       'Avvia il bot, naviga fino a una schermata qualsiasi e arrestalo in modo controllato. Deve restare una sola schermata offline. Al riavvio il vecchio messaggio offline deve essere rimosso automaticamente e deve rimanere una sola nuova schermata del gestionale; nessuna vecchia tastiera deve restare utilizzabile.',
       '🏠 Apri Menù principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 31 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Spegnimento controllato da Amministrazione',
       'Apri Amministrazione come amministratore principale. Deve esserci Spegni gestionale con seconda conferma. Confermando, il dispatcher deve terminare senza Ctrl+C e deve restare una sola schermata Gestionale Casa è offline. Riavvia il processo: il messaggio offline deve sparire ed essere sostituito dalla schermata online. Verifica anche che un utente non amministratore non possa eseguire lo spegnimento.',
       '🛠️ Apri Amministrazione',
       'admin:menu',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 32 AND stato = 'fatto';
