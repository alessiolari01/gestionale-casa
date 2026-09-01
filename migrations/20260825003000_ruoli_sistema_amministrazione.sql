-- Step 7.2C - Ruoli di sistema e interfaccia amministrativa.
--
-- Obiettivi:
-- - separare il ruolo globale nel gestionale dai ruoli negli spazi e dai
--   permessi sulle singole risorse;
-- - introdurre soltanto i due ruoli necessari oggi: utente e admin;
-- - rendere amministratore il proprietario dello spazio bootstrap, oppure il
--   primo utente interno esistente come fallback per database gia' avviati;
-- - lasciare tutti gli utenti successivi come utenti normali per default.

ALTER TABLE utenti
ADD COLUMN ruolo_sistema TEXT NOT NULL DEFAULT 'utente'
    CHECK (ruolo_sistema IN ('utente', 'admin'));

UPDATE utenti
SET ruolo_sistema = 'admin',
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = COALESCE(
    (
        SELECT creato_da_utente_id
        FROM spazi
        WHERE bootstrap_legacy = 1
          AND creato_da_utente_id IS NOT NULL
        LIMIT 1
    ),
    (
        SELECT id
        FROM utenti
        ORDER BY creato_il, id
        LIMIT 1
    )
)
AND NOT EXISTS (
    SELECT 1 FROM utenti WHERE ruolo_sistema = 'admin'
);

CREATE INDEX idx_utenti_ruolo_sistema
    ON utenti (ruolo_sistema, stato, nome_visualizzato);
