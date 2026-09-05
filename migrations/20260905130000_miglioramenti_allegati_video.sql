-- Un miglioramento può ora avere anche un video come allegato originale,
-- non solo una foto (prima solo gli allegati di *verifica* lo
-- permettevano: miglioramento_verifica_allegati aveva già
-- CHECK (tipo IN ('foto','video')), miglioramento_allegati era fermo a
-- CHECK (tipo = 'foto')). SQLite non permette ALTER su un CHECK: si
-- ricostruisce la tabella, stesso schema, cambia solo il vincolo.
ALTER TABLE miglioramento_allegati RENAME TO miglioramento_allegati_old;

CREATE TABLE miglioramento_allegati (
    id INTEGER PRIMARY KEY,
    miglioramento_id INTEGER NOT NULL,
    tipo TEXT NOT NULL DEFAULT 'foto' CHECK (tipo IN ('foto', 'video')),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (miglioramento_id)
        REFERENCES miglioramenti(id)
        ON DELETE CASCADE,
    CHECK (length(trim(percorso_file)) > 0)
);

INSERT INTO miglioramento_allegati (id, miglioramento_id, tipo, percorso_file, descrizione, creato_il)
    SELECT id, miglioramento_id, tipo, percorso_file, descrizione, creato_il
      FROM miglioramento_allegati_old;

DROP TABLE miglioramento_allegati_old;

CREATE INDEX idx_miglioramento_allegati_miglioramento
    ON miglioramento_allegati(miglioramento_id, creato_il, id);

-- Stesso vincolo, stessa correzione, sulla copia archiviata: altrimenti un
-- miglioramento con un allegato video non potrebbe mai essere archiviato
-- (miglioramento_archivio_verifica_allegati già ammetteva 'video', questa
-- tabella no).
ALTER TABLE miglioramento_archivio_allegati RENAME TO miglioramento_archivio_allegati_old;

CREATE TABLE miglioramento_archivio_allegati (
    id INTEGER PRIMARY KEY,
    miglioramento_archivio_id INTEGER NOT NULL,
    tipo TEXT NOT NULL CHECK (tipo IN ('foto', 'video')),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL,
    FOREIGN KEY (miglioramento_archivio_id)
        REFERENCES miglioramenti_archivio(id)
        ON DELETE CASCADE,
    CHECK (length(trim(percorso_file)) > 0)
);

INSERT INTO miglioramento_archivio_allegati (id, miglioramento_archivio_id, tipo, percorso_file, descrizione, creato_il)
    SELECT id, miglioramento_archivio_id, tipo, percorso_file, descrizione, creato_il
      FROM miglioramento_archivio_allegati_old;

DROP TABLE miglioramento_archivio_allegati_old;

CREATE INDEX idx_miglioramento_archivio_allegati
    ON miglioramento_archivio_allegati(miglioramento_archivio_id, id);
