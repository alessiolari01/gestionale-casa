-- Step 7.1 - Spazi operativi: unicita' per spazio e vincoli SQLite definitivi.
--
-- Compatibilita' SQLx 0.8.6 / SQLite:
-- SQLx 0.8.6 esegue le migration SQLite dentro una transazione propria e non
-- supporta ancora `-- no-transaction` per questo driver. Per questo motivo la
-- migration non usa BEGIN/COMMIT e non prova a disabilitare foreign_keys.
-- I rebuild di abitazioni/tag vengono eseguiti ricostruendo temporaneamente
-- anche le tabelle figlie che li referenziano, mantenendo ID e relazioni.
-- `defer_foreign_keys` rinvia i controlli FK al commit della transazione SQLx.
--
-- La migration NON crea nuovi spazi e NON sposta dati fra spazi: i dati
-- preesistenti restano nello spazio bootstrap #1.

PRAGMA defer_foreign_keys = ON;

-- ---------------------------------------------------------------------------
-- membership / spazio attivo
-- ---------------------------------------------------------------------------
CREATE TRIGGER trg_membri_spazio_spazio_attivo_delete
AFTER DELETE ON membri_spazio
WHEN EXISTS (
    SELECT 1
    FROM preferenze_utente p
    WHERE p.utente_id = OLD.utente_id
      AND p.spazio_attivo_id = OLD.spazio_id
)
BEGIN
    UPDATE preferenze_utente
    SET spazio_attivo_id = (
            SELECT ms.spazio_id
            FROM membri_spazio ms
            WHERE ms.utente_id = OLD.utente_id
            ORDER BY ms.aggiunto_il, ms.spazio_id
            LIMIT 1
        ),
        aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    WHERE utente_id = OLD.utente_id
      AND EXISTS (
          SELECT 1
          FROM membri_spazio ms
          WHERE ms.utente_id = OLD.utente_id
      );

    DELETE FROM preferenze_utente
    WHERE utente_id = OLD.utente_id
      AND NOT EXISTS (
          SELECT 1
          FROM membri_spazio ms
          WHERE ms.utente_id = OLD.utente_id
      );
END;

CREATE TRIGGER trg_membri_spazio_identita_immutabile
BEFORE UPDATE OF spazio_id, utente_id ON membri_spazio
BEGIN
    SELECT RAISE(ABORT, 'identificatori membership non modificabili');
END;

-- ---------------------------------------------------------------------------
-- abitazioni + tabelle figlie
-- ---------------------------------------------------------------------------
-- SQLite non consente di rimuovere il vecchio UNIQUE(nome) senza rebuild.
-- Con foreign_keys attive non possiamo semplicemente droppare la tabella
-- padre: salviamo quindi le righe delle tabelle dipendenti, ricostruiamo la
-- gerarchia e reinseriamo tutto con gli stessi ID.
CREATE TEMP TABLE _mig7_abitazioni AS
SELECT id, nome, descrizione, creato_il, aggiornato_il, spazio_id
FROM abitazioni;

CREATE TEMP TABLE _mig7_stanze AS
SELECT id, abitazione_id, nome, descrizione, creato_il, aggiornato_il
FROM stanze;

CREATE TEMP TABLE _mig7_contenitori AS
SELECT id, abitazione_id, stanza_id, contenitore_padre_id, nome, descrizione,
       creato_il, aggiornato_il
FROM contenitori;

CREATE TEMP TABLE _mig7_item_luogo AS
SELECT item_id, abitazione_id, stanza_id, contenitore_id
FROM item_luogo;

DROP TABLE item_luogo;
DROP TABLE contenitori;
DROP TABLE stanze;
DROP TABLE abitazioni;

CREATE TABLE abitazioni (
    id            INTEGER PRIMARY KEY,
    nome          TEXT NOT NULL COLLATE NOCASE,
    descrizione   TEXT,
    creato_il     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    spazio_id     INTEGER NOT NULL REFERENCES spazi(id) ON DELETE RESTRICT,
    UNIQUE (spazio_id, nome)
);

INSERT INTO abitazioni (id, nome, descrizione, creato_il, aggiornato_il, spazio_id)
SELECT id, nome, descrizione, creato_il, aggiornato_il, spazio_id
FROM _mig7_abitazioni;

CREATE INDEX idx_abitazioni_spazio
    ON abitazioni (spazio_id, nome);

CREATE TRIGGER trg_abitazioni_spazio_valido_insert
BEFORE INSERT ON abitazioni
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio abitazione inesistente');
END;

CREATE TRIGGER trg_abitazioni_spazio_valido_update
BEFORE UPDATE OF spazio_id ON abitazioni
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio abitazione inesistente');
END;

CREATE TABLE stanze (
    id            INTEGER PRIMARY KEY,
    abitazione_id INTEGER NOT NULL REFERENCES abitazioni(id) ON DELETE CASCADE,
    nome          TEXT NOT NULL COLLATE NOCASE,
    descrizione   TEXT,
    creato_il     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (abitazione_id, nome)
);

INSERT INTO stanze (id, abitazione_id, nome, descrizione, creato_il, aggiornato_il)
SELECT id, abitazione_id, nome, descrizione, creato_il, aggiornato_il
FROM _mig7_stanze;

CREATE INDEX idx_stanze_abitazione_id ON stanze(abitazione_id);

CREATE TRIGGER trg_stanza_cambio_abitazione
AFTER UPDATE OF abitazione_id ON stanze
BEGIN
    UPDATE item_luogo
    SET abitazione_id = NEW.abitazione_id
    WHERE stanza_id = NEW.id;
END;

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

INSERT INTO contenitori (
    id, abitazione_id, stanza_id, contenitore_padre_id, nome, descrizione,
    creato_il, aggiornato_il
)
SELECT id, abitazione_id, stanza_id, contenitore_padre_id, nome, descrizione,
       creato_il, aggiornato_il
FROM _mig7_contenitori
ORDER BY id;

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

CREATE TABLE item_luogo (
    item_id       INTEGER PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
    abitazione_id INTEGER REFERENCES abitazioni(id) ON DELETE CASCADE,
    stanza_id     INTEGER REFERENCES stanze(id) ON DELETE SET NULL,
    contenitore_id INTEGER REFERENCES contenitori(id) ON DELETE SET NULL,
    CHECK (stanza_id IS NULL OR abitazione_id IS NOT NULL)
);

INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id)
SELECT item_id, abitazione_id, stanza_id, contenitore_id
FROM _mig7_item_luogo;

CREATE INDEX idx_item_luogo_abitazione_id ON item_luogo(abitazione_id);
CREATE INDEX idx_item_luogo_stanza_id ON item_luogo(stanza_id);
CREATE INDEX idx_item_luogo_contenitore ON item_luogo (contenitore_id);

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

CREATE TRIGGER trg_item_luogo_spazio_insert
BEFORE INSERT ON item_luogo
WHEN NEW.abitazione_id IS NOT NULL
AND EXISTS (
    SELECT 1
    FROM items i
    JOIN abitazioni a ON a.id = NEW.abitazione_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> a.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e abitazione appartengono a spazi diversi');
END;

CREATE TRIGGER trg_item_luogo_spazio_update
BEFORE UPDATE OF item_id, abitazione_id ON item_luogo
WHEN NEW.abitazione_id IS NOT NULL
AND EXISTS (
    SELECT 1
    FROM items i
    JOIN abitazioni a ON a.id = NEW.abitazione_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> a.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e abitazione appartengono a spazi diversi');
END;

DROP TABLE _mig7_item_luogo;
DROP TABLE _mig7_contenitori;
DROP TABLE _mig7_stanze;
DROP TABLE _mig7_abitazioni;

-- ---------------------------------------------------------------------------
-- tag + item_tag
-- ---------------------------------------------------------------------------
CREATE TEMP TABLE _mig7_tag AS
SELECT id, nome, spazio_id
FROM tag;

CREATE TEMP TABLE _mig7_item_tag AS
SELECT item_id, tag_id
FROM item_tag;

DROP TABLE item_tag;
DROP TABLE tag;

CREATE TABLE tag (
    id        INTEGER PRIMARY KEY,
    nome      TEXT NOT NULL COLLATE NOCASE,
    spazio_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE RESTRICT,
    UNIQUE (spazio_id, nome)
);

INSERT INTO tag (id, nome, spazio_id)
SELECT id, nome, spazio_id
FROM _mig7_tag;

CREATE INDEX idx_tag_spazio
    ON tag (spazio_id, nome);

CREATE TRIGGER trg_tag_spazio_valido_insert
BEFORE INSERT ON tag
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio tag inesistente');
END;

CREATE TRIGGER trg_tag_spazio_valido_update
BEFORE UPDATE OF spazio_id ON tag
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio tag inesistente');
END;

CREATE TABLE item_tag (
    item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, tag_id)
);

INSERT INTO item_tag (item_id, tag_id)
SELECT item_id, tag_id
FROM _mig7_item_tag;

CREATE INDEX idx_item_tag_tag_id ON item_tag(tag_id);

CREATE TRIGGER trg_item_tag_spazio_insert
BEFORE INSERT ON item_tag
WHEN EXISTS (
    SELECT 1
    FROM items i
    JOIN tag t ON t.id = NEW.tag_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> t.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e tag appartengono a spazi diversi');
END;

CREATE TRIGGER trg_item_tag_spazio_update
BEFORE UPDATE OF item_id, tag_id ON item_tag
WHEN EXISTS (
    SELECT 1
    FROM items i
    JOIN tag t ON t.id = NEW.tag_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> t.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e tag appartengono a spazi diversi');
END;

DROP TABLE _mig7_item_tag;
DROP TABLE _mig7_tag;
