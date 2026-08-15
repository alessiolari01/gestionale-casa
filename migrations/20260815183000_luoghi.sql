-- Step 6A: case, stanze e posizione strutturata condivisa.
--
-- `abitazioni` e `stanze` descrivono la gerarchia fisica. `item_luogo`
-- collega qualsiasi riga di `items` a una casa e, opzionalmente, a una stanza.
-- Il campo libero `oggetti.posizione` resta invariato ed e' usato come
-- dettaglio della posizione (es. "scaffale 2", "cassetto alto").

CREATE TABLE abitazioni (
    id            INTEGER PRIMARY KEY,
    nome          TEXT NOT NULL COLLATE NOCASE UNIQUE,
    descrizione   TEXT,
    creato_il     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE stanze (
    id            INTEGER PRIMARY KEY,
    abitazione_id INTEGER NOT NULL REFERENCES abitazioni(id) ON DELETE CASCADE,
    nome          TEXT NOT NULL COLLATE NOCASE,
    descrizione   TEXT,
    creato_il     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (abitazione_id, nome)
);

CREATE INDEX idx_stanze_abitazione_id ON stanze(abitazione_id);

CREATE TABLE item_luogo (
    item_id       INTEGER PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
    abitazione_id INTEGER REFERENCES abitazioni(id) ON DELETE CASCADE,
    stanza_id     INTEGER REFERENCES stanze(id) ON DELETE SET NULL,
    CHECK (stanza_id IS NULL OR abitazione_id IS NOT NULL)
);

CREATE INDEX idx_item_luogo_abitazione_id ON item_luogo(abitazione_id);
CREATE INDEX idx_item_luogo_stanza_id ON item_luogo(stanza_id);

-- SQLite non puo' esprimere con una semplice FK il vincolo "la stanza deve
-- appartenere alla casa selezionata" mantenendo, allo stesso tempo, la casa
-- quando una stanza viene eliminata. I trigger rendono esplicita la regola.
CREATE TRIGGER trg_item_luogo_insert_coerente
BEFORE INSERT ON item_luogo
WHEN NEW.stanza_id IS NOT NULL
BEGIN
    SELECT CASE
        WHEN NOT EXISTS (
            SELECT 1
            FROM stanze s
            WHERE s.id = NEW.stanza_id
              AND s.abitazione_id = NEW.abitazione_id
        )
        THEN RAISE(ABORT, 'stanza non appartenente alla casa selezionata')
    END;
END;

CREATE TRIGGER trg_item_luogo_update_coerente
BEFORE UPDATE OF abitazione_id, stanza_id ON item_luogo
WHEN NEW.stanza_id IS NOT NULL
BEGIN
    SELECT CASE
        WHEN NOT EXISTS (
            SELECT 1
            FROM stanze s
            WHERE s.id = NEW.stanza_id
              AND s.abitazione_id = NEW.abitazione_id
        )
        THEN RAISE(ABORT, 'stanza non appartenente alla casa selezionata')
    END;
END;

-- Se in futuro una stanza viene spostata programmaticamente in un'altra casa,
-- gli item gia' assegnati seguono la nuova casa e non possono restare incoerenti.
CREATE TRIGGER trg_stanza_cambio_abitazione
AFTER UPDATE OF abitazione_id ON stanze
BEGIN
    UPDATE item_luogo
    SET abitazione_id = NEW.abitazione_id
    WHERE stanza_id = NEW.id;
END;
