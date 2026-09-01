-- Step 7.2H.4D — rifiniture finali inviti e workflow Miglioramenti.
-- Migration append-only: non modifica migration precedenti.
--
-- Porta in Fatto · da verificare soltanto le correzioni appena implementate
-- in questo step e associa una verifica guidata a ciascuna.

UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = COALESCE(fatto_il, strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ','now')
WHERE id IN (33, 39, 40, 41)
  AND stato = 'da_fare';

INSERT OR REPLACE INTO miglioramento_piani_verifica
(miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Input errato sempre recuperabile',
       'Da Spazi apri un invito e prova un orario errato oppure un limite utilizzi errato. Il bot deve spiegare l’errore, restare in attesa di un nuovo valore e offrire Indietro e Menù principale.',
       '👥 Apri Spazi',
       'identity:spaces',
       strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM miglioramenti
WHERE id = 33 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
(miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Dopo archiviazione torna ai Fatti',
       'Apri un miglioramento già verificato, archivialo e controlla che il bot torni automaticamente alla lista Fatti da verificare invece di restare nei Verificati da archiviare.',
       '✅ Apri Fatti',
       'improve:list:done:0',
       strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM miglioramenti
WHERE id = 39 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
(miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Orario digitabile subito',
       'Da Spazi crea o modifica un invito con scadenza. Nella schermata Orario scrivi direttamente un valore come 12:43 senza premere Inserisci orario: deve essere accettato e applicato subito. Verifica anche che i pulsanti rapidi continuino a funzionare.',
       '👥 Apri Spazi',
       'identity:spaces',
       strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM miglioramenti
WHERE id = 40 AND stato = 'fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica
(miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il)
SELECT id,
       'Limite utilizzi digitabile',
       'Da Spazi apri un invito attivo, scegli Modifica utilizzi e scrivi direttamente un numero non presente nei pulsanti, per esempio 37. Il dettaglio deve mostrare il nuovo limite. Verifica anche 1 = monouso e un input non valido recuperabile.',
       '👥 Apri Spazi',
       'identity:spaces',
       strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM miglioramenti
WHERE id = 41 AND stato = 'fatto';
