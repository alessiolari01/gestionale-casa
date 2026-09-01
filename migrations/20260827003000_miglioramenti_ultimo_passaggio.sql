-- Step 7.2G.3 — ultimo passaggio Miglioramenti prima della documentazione.
-- Migration append-only: le migration precedenti sono già applicate e IMMUTABILI.
--
-- Implementa i miglioramenti #22-#28 dell'handoff del 27/08/2026.
-- #7 resta volutamente Da fare: reset/eliminazione account richiede una progettazione
-- separata su proprietà, membership, storico e permessi.

UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (22, 23, 24, 25, 26, 27, 28)
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
       'Indietro conserva lista e pagina',
       'Apri Tutti i miglioramenti, entra in un elemento da una pagina qualsiasi e premi Indietro. Devi tornare esattamente alla stessa lista e alla stessa pagina, non al menu generico Miglioramenti.',
       '🗂 Tutti i miglioramenti',
       'improve:list:all:0',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 22 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Annulla Migliora torna al contesto',
       'Apri una sezione qualsiasi, premi Migliora e poi Annulla. Deve ricomparire la schermata da cui avevi premuto Migliora, con la sua tastiera aggiornata.',
       '🏠 Apri Menù principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 23 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Azioni recenti precise e ordinate',
       'Premi almeno tre pulsanti diversi e poi Migliora. Nel contesto l’azione più recente deve essere in cima e devono comparire i nomi reali dei pulsanti, non frasi generiche come hai usato un pulsante.',
       '🏠 Apri Menù principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 24 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Barra navigazione e accento Menù',
       'Naviga in più sezioni. Dove possibile la riga finale deve essere Indietro | Menù principale | Migliora; senza Indietro, Menù principale | Migliora. Verifica inoltre che il pulsante scriva sempre Menù con accento.',
       '🏠 Apri Menù principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 25 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Interfaccia Telegram a schermata singola',
       'Naviga tra più schermate: la schermata UI precedente del bot deve sparire. Prova un vecchio pulsante o un doppio tocco: non deve duplicare l’azione. In un wizard invia testo e una foto/video: dopo acquisizione riuscita l’input dell’utente deve essere eliminato; i media temporanei del bot devono sparire cambiando schermata.',
       '🏠 Apri Menù principale',
       'menu:main',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 26 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Descrizione lunga multimessaggio',
       'Crea un miglioramento inviando almeno due messaggi di descrizione e termina con Fine descrizione. Verifica che vengano uniti in ordine. Prova anche una descrizione molto lunga: il dettaglio deve mostrare un’anteprima e il pulsante Leggi descrizione completa con paginazione, senza limite normale di 2000 caratteri.',
       '💡 Nuovo miglioramento',
       'improve:new',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 27 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Categoria come filtro ingredienti',
       'Apri Cerca per ingredienti. Il pulsante categoria deve essere un filtro, non un selettore alternativo di ingredienti: scegli una categoria, poi digita un alimento e verifica che i risultati siano limitati a quella categoria. Deve essere possibile rimuovere il filtro.',
       '🥕 Cerca per ingredienti',
       'recipe:find',
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti WHERE id = 28 AND stato = 'fatto';
