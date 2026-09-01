-- Step 7.2H.4C v2 — rifiniture inviti/orari e stati Miglioramenti coerenti col collaudo reale.
-- Migration append-only: NON modificare migration già applicate.
--
-- Regola:
-- - ciò che l’amministratore ha già confermato come funzionante viene archiviato;
-- - solo le funzioni nuove/non ancora provate restano Fatto · da verificare;
-- - i collaudi che richiedono un secondo account restano esplicitamente pendenti.

-- 1) Archivia i miglioramenti già collaudati e confermati.
INSERT INTO miglioramenti_archivio (
    miglioramento_origine_id, autore_utente_id, descrizione, modulo, contesto, creato_il,
    completato_il, archiviato_il, archiviato_da_utente_id, verifica_esito, verifica_note,
    verificato_il, verificato_da_utente_id
)
SELECT
    m.id, m.autore_utente_id, m.descrizione, m.modulo, m.contesto, m.creato_il,
    COALESCE(m.fatto_il, m.aggiornato_il), '2026-08-28T20:25:00.000Z',
    (SELECT u.id FROM utenti u WHERE u.ruolo_sistema='admin' AND u.amministratore_principale=1 ORDER BY u.id LIMIT 1),
    'ok', 'Collaudo confermato dall’amministratore durante gli Step 7.2H.3/H.4A/H.4B.', '2026-08-28T20:25:00.000Z',
    (SELECT u.id FROM utenti u WHERE u.ruolo_sistema='admin' AND u.amministratore_principale=1 ORDER BY u.id LIMIT 1)
FROM miglioramenti m
WHERE m.id IN (11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26)
  AND m.stato IN ('da_fare','fatto')
  AND NOT EXISTS (
      SELECT 1 FROM miglioramenti_archivio a
      WHERE a.miglioramento_origine_id=m.id AND a.archiviato_il='2026-08-28T20:25:00.000Z'
  );

INSERT INTO miglioramento_archivio_allegati
(miglioramento_archivio_id,tipo,percorso_file,descrizione,creato_il)
SELECT ar.id, al.tipo, al.percorso_file, al.descrizione, al.creato_il
FROM miglioramento_allegati al
JOIN miglioramenti m ON m.id=al.miglioramento_id
JOIN miglioramenti_archivio ar
  ON ar.miglioramento_origine_id=m.id AND ar.archiviato_il='2026-08-28T20:25:00.000Z'
WHERE m.id IN (11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26);

INSERT INTO miglioramento_archivio_verifica_allegati
(miglioramento_archivio_id,tipo,percorso_file,descrizione,creato_il)
SELECT ar.id, va.tipo, va.percorso_file, va.descrizione, va.creato_il
FROM miglioramento_verifica_allegati va
JOIN miglioramenti m ON m.id=va.miglioramento_id
JOIN miglioramenti_archivio ar
  ON ar.miglioramento_origine_id=m.id AND ar.archiviato_il='2026-08-28T20:25:00.000Z'
WHERE m.id IN (11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26);

-- Elimina esplicitamente i record figli prima del record attivo.
-- Le FK hanno ON DELETE CASCADE nel runtime, ma la migration deve restare
-- consistente anche quando viene verificata con sqlite3 e foreign_keys=OFF.
DELETE FROM miglioramento_piani_verifica
WHERE miglioramento_id IN (11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26);

DELETE FROM miglioramento_verifica_allegati
WHERE miglioramento_id IN (11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26);

DELETE FROM miglioramento_allegati
WHERE miglioramento_id IN (11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26);

DELETE FROM miglioramenti
WHERE id IN (11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26)
  AND stato IN ('da_fare','fatto');

-- 2) Solo ciò che non è ancora stato collaudato passa a Fatto · da verificare.
UPDATE miglioramenti
SET stato='fatto',
    fatto_il=COALESCE(fatto_il,strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    verifica_esito=NULL, verifica_note=NULL, verificato_il=NULL, verificato_da_utente_id=NULL,
    letto_admin_il=COALESCE(letto_admin_il,strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    aggiornato_il=strftime('%Y-%m-%dT%H:%M:%fZ','now')
WHERE id IN (17, 22, 27, 28, 29, 30, 31, 32, 33, 34) AND stato='da_fare';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Modifica ruolo membro con secondo account','Con un secondo account già membro, apri Membri dello spazio, selezionalo e modifica il ruolo. Verifica sia l’aggiornamento del dettaglio sia la notifica ricevuta dal secondo account, senza ID tecnici.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=17 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Notifica accettazione al creatore','Fai accettare un invito dal secondo account. Sul creatore verifica nome utente e spazio leggibili, nessun Telegram ID/UUID, navigazione utile e notifica non persistente.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=22 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Cambio vista Spazi senza schermata morta','Da Spazi premi Tutti i miei spazi e poi Solo predefinito. Ogni scelta deve mostrare un avviso temporaneo e mantenere visibile la schermata Spazi aggiornata.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=27 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Data creazione invito','Apri un invito attivo. Nel dettaglio deve comparire anche la data e ora di creazione in formato leggibile.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=28 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Indietro dall’orario al giorno','Crea un invito con scadenza, scegli il giorno e arriva alla scelta orario. Premi Indietro: deve tornare al calendario dello stesso mese/giorno, non ai tipi di invito.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=29 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Modifica limite utilizzi','Apri un invito attivo e modifica il numero massimo di utilizzi. 1 deve diventare monouso; valori maggiori devono mostrare il nuovo limite; Illimitati deve renderlo riutilizzabile.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=30 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Orario rapido applicato subito','Durante la scelta della scadenza premi un orario rapido. Deve essere applicato immediatamente senza una seconda conferma.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=31 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Inserimento manuale orario HH:MM','Durante la scelta dell’orario usa Inserisci orario e invia, per esempio, 12:43. Deve essere accettato in formato 24h e applicato subito.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=32 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Input orario errato recuperabile','Durante Inserisci orario prova valori errati come 2:43, 24:00 o 12.43. Il bot deve spiegare il formato HH:MM, mantenere l’attesa e offrire Indietro e Menù principale.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=33 AND stato='fatto';

INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il) SELECT id,'Riga navigazione Miglioramenti','Apri Da fare e altre liste Miglioramenti. La riga finale deve contenere Indietro, Menù principale e Migliora insieme quando la schermata prevede un ritorno.','🟢 Apri Da fare','improve:list:todo:0',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE id=34 AND stato='fatto';

-- 3) Collaudi espliciti che richiedono il secondo account.

INSERT INTO miglioramenti (autore_utente_id,descrizione,modulo,stato,letto_admin_il,fatto_il,creato_il,aggiornato_il)
SELECT u.id,'COLLAUDO PENDENTE H.4 — Secondo account: dopo aver accettato un invito, il destinatario deve vedere Apri spazio e Menù principale; Apri spazio deve portare direttamente allo spazio appena condiviso.','spazi','fatto',strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM utenti u
WHERE u.ruolo_sistema='admin' AND u.amministratore_principale=1
AND NOT EXISTS (SELECT 1 FROM miglioramenti m WHERE m.descrizione='COLLAUDO PENDENTE H.4 — Secondo account: dopo aver accettato un invito, il destinatario deve vedere Apri spazio e Menù principale; Apri spazio deve portare direttamente allo spazio appena condiviso.')
ORDER BY u.id LIMIT 1;
INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il)
SELECT id,'Accettazione invito dal secondo account','Con il secondo account apri un nuovo link, premi Accetta invito e verifica che compaiano Apri spazio e Menù principale. Apri spazio deve aprire direttamente lo spazio appena condiviso.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE descrizione='COLLAUDO PENDENTE H.4 — Secondo account: dopo aver accettato un invito, il destinatario deve vedere Apri spazio e Menù principale; Apri spazio deve portare direttamente allo spazio appena condiviso.' AND stato='fatto';

INSERT INTO miglioramenti (autore_utente_id,descrizione,modulo,stato,letto_admin_il,fatto_il,creato_il,aggiornato_il)
SELECT u.id,'COLLAUDO PENDENTE H.4 — Secondo account: il creatore deve ricevere la notifica di accettazione con nomi leggibili, nessun ID tecnico e navigazione utile.','spazi','fatto',strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM utenti u
WHERE u.ruolo_sistema='admin' AND u.amministratore_principale=1
AND NOT EXISTS (SELECT 1 FROM miglioramenti m WHERE m.descrizione='COLLAUDO PENDENTE H.4 — Secondo account: il creatore deve ricevere la notifica di accettazione con nomi leggibili, nessun ID tecnico e navigazione utile.')
ORDER BY u.id LIMIT 1;
INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il)
SELECT id,'Notifica accettazione al creatore','Fai accettare un invito dal secondo account. Sul creatore verifica nome utente e spazio leggibili, nessun Telegram ID/UUID e navigazione utile.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE descrizione='COLLAUDO PENDENTE H.4 — Secondo account: il creatore deve ricevere la notifica di accettazione con nomi leggibili, nessun ID tecnico e navigazione utile.' AND stato='fatto';

INSERT INTO miglioramenti (autore_utente_id,descrizione,modulo,stato,letto_admin_il,fatto_il,creato_il,aggiornato_il)
SELECT u.id,'COLLAUDO PENDENTE H.4 — Secondo account: dopo una modifica del ruolo, il destinatario deve ricevere una notifica leggibile con spazio e nuovo ruolo, senza ID tecnici.','spazi','fatto',strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM utenti u
WHERE u.ruolo_sistema='admin' AND u.amministratore_principale=1
AND NOT EXISTS (SELECT 1 FROM miglioramenti m WHERE m.descrizione='COLLAUDO PENDENTE H.4 — Secondo account: dopo una modifica del ruolo, il destinatario deve ricevere una notifica leggibile con spazio e nuovo ruolo, senza ID tecnici.')
ORDER BY u.id LIMIT 1;
INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il)
SELECT id,'Notifica cambio ruolo','Con il secondo account già membro cambia il suo ruolo da Membri dello spazio. Sul secondo account deve arrivare una notifica leggibile con spazio e nuovo ruolo, senza ID tecnici.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE descrizione='COLLAUDO PENDENTE H.4 — Secondo account: dopo una modifica del ruolo, il destinatario deve ricevere una notifica leggibile con spazio e nuovo ruolo, senza ID tecnici.' AND stato='fatto';

INSERT INTO miglioramenti (autore_utente_id,descrizione,modulo,stato,letto_admin_il,fatto_il,creato_il,aggiornato_il)
SELECT u.id,'COLLAUDO PENDENTE H.4 — Secondo account: dopo la rimozione dallo spazio, il destinatario deve ricevere una notifica chiara e lo spazio non deve più risultare accessibile.','spazi','fatto',strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now')
FROM utenti u
WHERE u.ruolo_sistema='admin' AND u.amministratore_principale=1
AND NOT EXISTS (SELECT 1 FROM miglioramenti m WHERE m.descrizione='COLLAUDO PENDENTE H.4 — Secondo account: dopo la rimozione dallo spazio, il destinatario deve ricevere una notifica chiara e lo spazio non deve più risultare accessibile.')
ORDER BY u.id LIMIT 1;
INSERT OR REPLACE INTO miglioramento_piani_verifica (miglioramento_id,titolo,istruzioni,azione_label,azione_callback,aggiornato_il)
SELECT id,'Notifica e perdita accesso dopo rimozione','Rimuovi il secondo account da uno spazio di test. Deve ricevere la notifica di rimozione; poi in Spazi quello spazio non deve più essere disponibile né apribile.','👥 Apri Spazi','identity:spaces',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM miglioramenti WHERE descrizione='COLLAUDO PENDENTE H.4 — Secondo account: dopo la rimozione dallo spazio, il destinatario deve ricevere una notifica chiara e lo spazio non deve più risultare accessibile.' AND stato='fatto';
