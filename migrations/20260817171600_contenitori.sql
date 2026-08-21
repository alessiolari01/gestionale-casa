-- Step 6C.1 - Contenitori gerarchici e sotto-posizioni.
CREATE TABLE contenitori (
    id INTEGER PRIMARY KEY,
    abitazione_id INTEGER NOT NULL REFERENCES abitazioni(id) ON DELETE CASCADE,
    stanza_id INTEGER REFERENCES stanze(id) ON DELETE CASCADE,
    contenitore_padre_id INTEGER REFERENCES contenitori(id) ON DELETE SET NULL,
    nome TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0),
    CHECK (contenitore_padre_id IS NULL OR contenitore_padre_id <> id)
);

CREATE INDEX idx_contenitori_luogo ON contenitori (abitazione_id, stanza_id);
CREATE INDEX idx_contenitori_padre ON contenitori (contenitore_padre_id);

CREATE UNIQUE INDEX idx_contenitori_nome_fratelli
ON contenitori (
    abitazione_id,
    ifnull(stanza_id, 0),
    ifnull(contenitore_padre_id, 0),
    nome COLLATE NOCASE
);

CREATE TRIGGER contenitori_stanza_coerente_insert
BEFORE INSERT ON contenitori
WHEN NEW.stanza_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM stanze s
    WHERE s.id = NEW.stanza_id AND s.abitazione_id = NEW.abitazione_id
)
BEGIN
    SELECT RAISE(ABORT, 'stanza non appartenente alla casa del contenitore');
END;

CREATE TRIGGER contenitori_stanza_coerente_update
BEFORE UPDATE OF abitazione_id, stanza_id ON contenitori
WHEN NEW.stanza_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM stanze s
    WHERE s.id = NEW.stanza_id AND s.abitazione_id = NEW.abitazione_id
)
BEGIN
    SELECT RAISE(ABORT, 'stanza non appartenente alla casa del contenitore');
END;

CREATE TRIGGER contenitori_padre_coerente_insert
BEFORE INSERT ON contenitori
WHEN NEW.contenitore_padre_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM contenitori p
    WHERE p.id = NEW.contenitore_padre_id
      AND p.abitazione_id = NEW.abitazione_id
      AND p.stanza_id IS NEW.stanza_id
)
BEGIN
    SELECT RAISE(ABORT, 'contenitore padre in un luogo differente');
END;

CREATE TRIGGER contenitori_padre_coerente_update
BEFORE UPDATE OF contenitore_padre_id ON contenitori
WHEN NEW.contenitore_padre_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM contenitori p
    WHERE p.id = NEW.contenitore_padre_id
      AND p.abitazione_id = NEW.abitazione_id
      AND p.stanza_id IS NEW.stanza_id
)
BEGIN
    SELECT RAISE(ABORT, 'contenitore padre in un luogo differente');
END;

ALTER TABLE item_luogo
ADD COLUMN contenitore_id INTEGER REFERENCES contenitori(id) ON DELETE SET NULL;

CREATE INDEX idx_item_luogo_contenitore ON item_luogo (contenitore_id);

CREATE TRIGGER item_luogo_contenitore_coerente_insert
BEFORE INSERT ON item_luogo
WHEN NEW.contenitore_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM contenitori c
    WHERE c.id = NEW.contenitore_id
      AND c.abitazione_id = NEW.abitazione_id
      AND c.stanza_id IS NEW.stanza_id
)
BEGIN
    SELECT RAISE(ABORT, 'contenitore non appartenente al luogo dell item');
END;

CREATE TRIGGER item_luogo_contenitore_coerente_update
BEFORE UPDATE OF abitazione_id, stanza_id, contenitore_id ON item_luogo
WHEN NEW.contenitore_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM contenitori c
    WHERE c.id = NEW.contenitore_id
      AND c.abitazione_id = NEW.abitazione_id
      AND c.stanza_id IS NEW.stanza_id
)
BEGIN
    SELECT RAISE(ABORT, 'contenitore non appartenente al luogo dell item');
END;
