-- Step 7.2A - Fondazioni Alimentazione: alimenti, alias e unita' strutturate.
-- Le 10 migration precedenti restano immutate.

CREATE TABLE unita_misura (
    id INTEGER PRIMARY KEY,
    codice TEXT NOT NULL UNIQUE,
    nome TEXT NOT NULL,
    simbolo TEXT NOT NULL,
    famiglia_conversione TEXT
        CHECK (
            famiglia_conversione IS NULL
            OR famiglia_conversione IN ('massa', 'volume')
        ),
    fattore_base_num INTEGER,
    fattore_base_den INTEGER,
    ordinamento INTEGER NOT NULL DEFAULT 0,
    attiva INTEGER NOT NULL DEFAULT 1 CHECK (attiva IN (0, 1)),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(codice)) > 0),
    CHECK (length(trim(nome)) > 0),
    CHECK (length(trim(simbolo)) > 0),
    CHECK (
        (
            famiglia_conversione IS NULL
            AND fattore_base_num IS NULL
            AND fattore_base_den IS NULL
        )
        OR
        (
            famiglia_conversione IS NOT NULL
            AND fattore_base_num IS NOT NULL
            AND fattore_base_num > 0
            AND fattore_base_den IS NOT NULL
            AND fattore_base_den > 0
        )
    )
);

CREATE INDEX idx_unita_misura_famiglia
    ON unita_misura (famiglia_conversione, attiva, ordinamento, id);

INSERT INTO unita_misura (
    codice, nome, simbolo, famiglia_conversione,
    fattore_base_num, fattore_base_den, ordinamento
) VALUES
    ('g', 'grammo', 'g', 'massa', 1, 1, 10),
    ('kg', 'chilogrammo', 'kg', 'massa', 1000, 1, 20),
    ('ml', 'millilitro', 'ml', 'volume', 1, 1, 30),
    ('l', 'litro', 'l', 'volume', 1000, 1, 40),
    ('pz', 'pezzo', 'pz', NULL, NULL, NULL, 50),
    ('cucchiaio', 'cucchiaio', 'cucchiaio', NULL, NULL, NULL, 60),
    ('cucchiaino', 'cucchiaino', 'cucchiaino', NULL, NULL, NULL, 70),
    ('qb', 'quanto basta', 'q.b.', NULL, NULL, NULL, 80);

CREATE TABLE alimenti (
    id INTEGER PRIMARY KEY,
    spazio_id INTEGER REFERENCES spazi(id) ON DELETE CASCADE,
    nome TEXT NOT NULL,
    nome_normalizzato TEXT NOT NULL,
    descrizione TEXT,
    unita_predefinita_id INTEGER
        REFERENCES unita_misura(id) ON DELETE SET NULL,
    creato_da_utente_id INTEGER
        REFERENCES utenti(id) ON DELETE SET NULL,
    archiviato INTEGER NOT NULL DEFAULT 0 CHECK (archiviato IN (0, 1)),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0),
    CHECK (length(trim(nome_normalizzato)) > 0)
);

CREATE UNIQUE INDEX idx_alimenti_globali_nome
    ON alimenti (nome_normalizzato)
    WHERE spazio_id IS NULL;

CREATE UNIQUE INDEX idx_alimenti_spazio_nome
    ON alimenti (spazio_id, nome_normalizzato)
    WHERE spazio_id IS NOT NULL;

CREATE INDEX idx_alimenti_spazio_ricerca
    ON alimenti (spazio_id, archiviato, nome_normalizzato, id);

CREATE INDEX idx_alimenti_unita_predefinita
    ON alimenti (unita_predefinita_id);

CREATE TABLE alimento_alias (
    id INTEGER PRIMARY KEY,
    alimento_id INTEGER NOT NULL
        REFERENCES alimenti(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    alias_normalizzato TEXT NOT NULL,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(alias)) > 0),
    CHECK (length(trim(alias_normalizzato)) > 0),
    UNIQUE (alimento_id, alias_normalizzato)
);

CREATE INDEX idx_alimento_alias_ricerca
    ON alimento_alias (alias_normalizzato, alimento_id);

CREATE TRIGGER trg_alimenti_creatore_membro_insert
BEFORE INSERT ON alimenti
WHEN NEW.spazio_id IS NOT NULL
AND NEW.creato_da_utente_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.spazio_id = NEW.spazio_id
      AND ms.utente_id = NEW.creato_da_utente_id
      AND ms.ruolo IN ('proprietario', 'amministratore', 'membro')
)
BEGIN
    SELECT RAISE(
        ABORT,
        'creatore alimento senza permesso di scrittura nello spazio'
    );
END;

CREATE TRIGGER trg_alimenti_creatore_membro_update
BEFORE UPDATE OF spazio_id, creato_da_utente_id ON alimenti
WHEN NEW.spazio_id IS NOT NULL
AND NEW.creato_da_utente_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.spazio_id = NEW.spazio_id
      AND ms.utente_id = NEW.creato_da_utente_id
      AND ms.ruolo IN ('proprietario', 'amministratore', 'membro')
)
BEGIN
    SELECT RAISE(
        ABORT,
        'creatore alimento senza permesso di scrittura nello spazio'
    );
END;
