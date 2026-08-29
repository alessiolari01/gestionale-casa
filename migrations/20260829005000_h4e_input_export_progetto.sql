-- Step 7.2H.4E — input inatteso non distruttivo + export tecnico progetto.
-- Append-only: non modificare migration già applicate.

-- #33 viene nuovamente implementato: qualsiasi testo casuale quando nessun
-- wizard è in attesa non sostituisce più la schermata UI persistente.
UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 33;

INSERT OR REPLACE INTO miglioramento_piani_verifica (
    miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il
)
SELECT
    id,
    'Testo casuale non deve sostituire la schermata',
    'Apri una schermata del gestionale senza wizard/input attivo, poi scrivi testo casuale. Il testo utente deve essere rimosso, la schermata corrente deve restare invariata e deve comparire solo un breve avviso temporaneo che sparirà alla successiva interazione.',
    '🏠 Apri Menù principale',
    'menu:main',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti
WHERE id = 33;

-- Registriamo anche la nuova funzione richiesta in questo step come Fatto · da verificare.
INSERT INTO miglioramenti (
    autore_utente_id,
    descrizione,
    modulo,
    stato,
    letto_admin_il,
    fatto_il,
    creato_il,
    aggiornato_il
)
SELECT
    u.id,
    'Esporta progetto: aggiungere un pulsante che produca uno ZIP tecnico completo per consegnare il progetto a chi non lo conosce, includendo sorgenti, migration, documentazione, script e metadati Git ma escludendo dati sensibili e runtime (.env, token, database, backup, allegati utente, data/, target/, .git/).',
    'miglioramenti',
    'fatto',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM utenti u
WHERE u.ruolo_sistema = 'admin'
  AND u.amministratore_principale = 1
  AND NOT EXISTS (
      SELECT 1
      FROM miglioramenti m
      WHERE m.descrizione LIKE 'Esporta progetto:%'
  )
ORDER BY u.id
LIMIT 1;

INSERT OR REPLACE INTO miglioramento_piani_verifica (
    miglioramento_id, titolo, istruzioni, azione_label, azione_callback, aggiornato_il
)
SELECT
    id,
    'Export tecnico progetto sanitizzato',
    'Da Miglioramenti premi Esporta progetto. Lo ZIP deve arrivare con sorgenti, migration, documentazione, script e _project_handoff. Aprilo e verifica che NON contenga .env, database, data/, token, backup, target/ o .git/. Dopo il download premi Ho scaricato il file e verifica la pulizia del documento temporaneo.',
    '💡 Apri Miglioramenti',
    'improve:menu',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM miglioramenti
WHERE descrizione LIKE 'Esporta progetto:%'
ORDER BY id DESC
LIMIT 1;
