-- Step 7.2E - Accesso controllato al bot e backlog Miglioramenti.
--
-- Obiettivi:
-- - mantenere ALLOWED_CHAT_IDS come bootstrap/emergenza, non come autorizzazione definitiva;
-- - permettere a un account Telegram sconosciuto di richiedere accesso;
-- - rendere l'amministratore principale l'unico decisore delle richieste;
-- - creare l'utente normale e il suo spazio personale soltanto dopo approvazione;
-- - introdurre un backlog interno di miglioramenti utilizzabile da tutti gli utenti approvati;
-- - consentire screenshot/allegati locali senza legare i miglioramenti agli spazi domestici.

ALTER TABLE utenti
ADD COLUMN amministratore_principale INTEGER NOT NULL DEFAULT 0
    CHECK (amministratore_principale IN (0, 1));

UPDATE utenti
SET amministratore_principale = 1,
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
        WHERE ruolo_sistema = 'admin'
        ORDER BY creato_il, id
        LIMIT 1
    )
)
AND NOT EXISTS (
    SELECT 1 FROM utenti WHERE amministratore_principale = 1
);

CREATE UNIQUE INDEX idx_utenti_amministratore_principale_unico
    ON utenti (amministratore_principale)
    WHERE amministratore_principale = 1;

CREATE TABLE richieste_accesso (
    id INTEGER PRIMARY KEY,
    telegram_user_id INTEGER NOT NULL UNIQUE,
    chat_id INTEGER NOT NULL,
    username_snapshot TEXT,
    nome_snapshot TEXT NOT NULL,
    cognome_snapshot TEXT,
    stato TEXT NOT NULL DEFAULT 'pendente'
        CHECK (stato IN ('pendente', 'approvata', 'rifiutata')),
    richiesta_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    decisa_il TEXT,
    decisa_da_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    CHECK (length(trim(nome_snapshot)) > 0),
    CHECK (
        (stato = 'pendente' AND decisa_il IS NULL AND decisa_da_utente_id IS NULL)
        OR
        (stato IN ('approvata', 'rifiutata') AND decisa_il IS NOT NULL AND decisa_da_utente_id IS NOT NULL)
    )
);

CREATE INDEX idx_richieste_accesso_stato_data
    ON richieste_accesso (stato, richiesta_il DESC, id DESC);

CREATE TABLE miglioramenti (
    id INTEGER PRIMARY KEY,
    autore_utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE RESTRICT,
    descrizione TEXT NOT NULL,
    modulo TEXT,
    stato TEXT NOT NULL DEFAULT 'aperto'
        CHECK (stato IN ('aperto', 'pianificato', 'fatto', 'scartato')),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(descrizione)) > 0),
    CHECK (modulo IS NULL OR length(trim(modulo)) > 0)
);

CREATE INDEX idx_miglioramenti_autore_data
    ON miglioramenti (autore_utente_id, creato_il DESC, id DESC);

CREATE INDEX idx_miglioramenti_stato_data
    ON miglioramenti (stato, aggiornato_il DESC, id DESC);

CREATE TABLE miglioramento_allegati (
    id INTEGER PRIMARY KEY,
    miglioramento_id INTEGER NOT NULL REFERENCES miglioramenti(id) ON DELETE CASCADE,
    tipo TEXT NOT NULL DEFAULT 'foto' CHECK (tipo = 'foto'),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(percorso_file)) > 0)
);

CREATE INDEX idx_miglioramento_allegati_miglioramento
    ON miglioramento_allegati (miglioramento_id, creato_il, id);
