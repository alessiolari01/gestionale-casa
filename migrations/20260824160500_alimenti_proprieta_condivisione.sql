-- Step 7.2B - proprieta' personale e condivisione alimenti.
--
-- Modello:
-- - catalogo_globale = 1: alimento di sistema, senza proprietario;
-- - proprietario_utente_id valorizzato: alimento personale;
-- - alimento_spazi: spazi nei quali l'alimento personale e' condiviso.
--
-- La membership determina chi puo' vedere gli alimenti condivisi.
-- La perdita di una membership NON elimina l'alimento posseduto dall'utente.

ALTER TABLE alimenti
    ADD COLUMN proprietario_utente_id INTEGER
        REFERENCES utenti(id) ON DELETE SET NULL;

ALTER TABLE alimenti
    ADD COLUMN catalogo_globale INTEGER NOT NULL DEFAULT 0
        CHECK (catalogo_globale IN (0, 1));

-- Gli alimenti che nella 7.2A avevano spazio_id NULL erano catalogo globale.
UPDATE alimenti
SET catalogo_globale = 1
WHERE spazio_id IS NULL;

-- Gli alimenti personalizzati creati dalla prima 7.2B diventano di proprieta'
-- della persona che li ha creati.
UPDATE alimenti
SET proprietario_utente_id = creato_da_utente_id
WHERE spazio_id IS NOT NULL
  AND creato_da_utente_id IS NOT NULL;

CREATE TABLE alimento_spazi (
    alimento_id INTEGER NOT NULL
        REFERENCES alimenti(id) ON DELETE CASCADE,

    spazio_id INTEGER NOT NULL
        REFERENCES spazi(id) ON DELETE CASCADE,

    condiviso_da_utente_id INTEGER
        REFERENCES utenti(id) ON DELETE SET NULL,

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (alimento_id, spazio_id)
);

-- La vecchia appartenenza allo spazio viene trasformata in una condivisione.
INSERT OR IGNORE INTO alimento_spazi (
    alimento_id,
    spazio_id,
    condiviso_da_utente_id
)
SELECT
    id,
    spazio_id,
    proprietario_utente_id
FROM alimenti
WHERE spazio_id IS NOT NULL
  AND proprietario_utente_id IS NOT NULL;

-- Da questo momento spazio_id non definisce piu' proprieta' o visibilita'.
-- Lo lasciamo fisicamente nello schema per compatibilita' append-only, ma il
-- runtime 7.2B non lo usera' piu'.
UPDATE alimenti
SET spazio_id = NULL
WHERE proprietario_utente_id IS NOT NULL;

DROP INDEX IF EXISTS idx_alimenti_globali_nome;
DROP INDEX IF EXISTS idx_alimenti_spazio_nome;
DROP INDEX IF EXISTS idx_alimenti_spazio_ricerca;

CREATE UNIQUE INDEX idx_alimenti_globali_nome_v2
    ON alimenti (nome_normalizzato)
    WHERE catalogo_globale = 1;

CREATE UNIQUE INDEX idx_alimenti_proprietario_nome
    ON alimenti (proprietario_utente_id, nome_normalizzato)
    WHERE proprietario_utente_id IS NOT NULL;

CREATE INDEX idx_alimenti_proprietario_ricerca
    ON alimenti (
        proprietario_utente_id,
        archiviato,
        nome_normalizzato,
        id
    );

CREATE INDEX idx_alimento_spazi_spazio
    ON alimento_spazi (
        spazio_id,
        alimento_id
    );

DROP TRIGGER IF EXISTS trg_alimenti_creatore_membro_insert;
DROP TRIGGER IF EXISTS trg_alimenti_creatore_membro_update;

-- Un utente puo' condividere un alimento solo in uno spazio dove possiede
-- diritto di scrittura. La condivisione puo' continuare a esistere anche se
-- quella membership viene rimossa in seguito.
CREATE TRIGGER trg_alimento_spazi_permesso_insert
BEFORE INSERT ON alimento_spazi
WHEN NEW.condiviso_da_utente_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.spazio_id = NEW.spazio_id
      AND ms.utente_id = NEW.condiviso_da_utente_id
      AND ms.ruolo IN ('proprietario', 'amministratore', 'membro')
)
BEGIN
    SELECT RAISE(
        ABORT,
        'utente senza permesso di condividere alimenti nello spazio'
    );
END;

-- Evita due alimenti con lo stesso nome normalizzato nello stesso spazio.
CREATE TRIGGER trg_alimento_spazi_nome_unico_insert
BEFORE INSERT ON alimento_spazi
WHEN EXISTS (
    SELECT 1
    FROM alimento_spazi altri
    JOIN alimenti esistente
      ON esistente.id = altri.alimento_id
    JOIN alimenti nuovo
      ON nuovo.id = NEW.alimento_id
    WHERE altri.spazio_id = NEW.spazio_id
      AND altri.alimento_id <> NEW.alimento_id
      AND esistente.archiviato = 0
      AND nuovo.archiviato = 0
      AND esistente.nome_normalizzato = nuovo.nome_normalizzato
)
BEGIN
    SELECT RAISE(
        ABORT,
        'alimento con lo stesso nome gia condiviso nello spazio'
    );
END;

CREATE TRIGGER trg_alimento_nome_unico_spazi_update
BEFORE UPDATE OF nome_normalizzato ON alimenti
WHEN EXISTS (
    SELECT 1
    FROM alimento_spazi propri
    JOIN alimento_spazi altri
      ON altri.spazio_id = propri.spazio_id
     AND altri.alimento_id <> propri.alimento_id
    JOIN alimenti esistente
      ON esistente.id = altri.alimento_id
    WHERE propri.alimento_id = NEW.id
      AND esistente.archiviato = 0
      AND NEW.archiviato = 0
      AND esistente.nome_normalizzato = NEW.nome_normalizzato
)
BEGIN
    SELECT RAISE(
        ABORT,
        'alimento con lo stesso nome gia condiviso nello spazio'
    );
END;
