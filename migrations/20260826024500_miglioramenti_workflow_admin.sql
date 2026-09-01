-- Step 7.2G — Workflow Miglioramenti e coda amministrativa
-- Migration append-only: le migration precedenti sono considerate immutabili.

ALTER TABLE richieste_accesso
    ADD COLUMN letto_admin_il TEXT;

-- Le richieste già decise prima di Step 7.2G sono considerate già lette.
UPDATE richieste_accesso
SET letto_admin_il = COALESCE(decisa_il, richiesta_il)
WHERE stato IN ('approvata', 'rifiutata');

CREATE TABLE miglioramenti_archivio (
    id INTEGER PRIMARY KEY,
    miglioramento_origine_id INTEGER NOT NULL,
    autore_utente_id INTEGER NOT NULL,
    descrizione TEXT NOT NULL,
    modulo TEXT,
    creato_il TEXT NOT NULL,
    completato_il TEXT NOT NULL,
    archiviato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    archiviato_da_utente_id INTEGER,
    FOREIGN KEY (autore_utente_id) REFERENCES utenti(id) ON DELETE RESTRICT,
    FOREIGN KEY (archiviato_da_utente_id) REFERENCES utenti(id) ON DELETE SET NULL,
    CHECK (length(trim(descrizione)) > 0),
    CHECK (modulo IS NULL OR length(trim(modulo)) > 0)
);

CREATE INDEX idx_miglioramenti_archivio_data
    ON miglioramenti_archivio(archiviato_il DESC, id DESC);

CREATE INDEX idx_miglioramenti_archivio_autore
    ON miglioramenti_archivio(autore_utente_id, archiviato_il DESC);

CREATE TABLE miglioramento_archivio_allegati (
    id INTEGER PRIMARY KEY,
    miglioramento_archivio_id INTEGER NOT NULL,
    tipo TEXT NOT NULL CHECK (tipo IN ('foto')),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL,
    FOREIGN KEY (miglioramento_archivio_id)
        REFERENCES miglioramenti_archivio(id)
        ON DELETE CASCADE,
    CHECK (length(trim(percorso_file)) > 0)
);

CREATE INDEX idx_miglioramento_archivio_allegati
    ON miglioramento_archivio_allegati(miglioramento_archivio_id, id);

-- I vecchi elementi "fatto" escono dal backlog attivo e diventano storico.
INSERT INTO miglioramenti_archivio (
    miglioramento_origine_id,
    autore_utente_id,
    descrizione,
    modulo,
    creato_il,
    completato_il,
    archiviato_il,
    archiviato_da_utente_id
)
SELECT
    m.id,
    m.autore_utente_id,
    m.descrizione,
    m.modulo,
    m.creato_il,
    m.aggiornato_il,
    m.aggiornato_il,
    NULL
FROM miglioramenti AS m
WHERE m.stato = 'fatto';

INSERT INTO miglioramento_archivio_allegati (
    miglioramento_archivio_id,
    tipo,
    percorso_file,
    descrizione,
    creato_il
)
SELECT
    a2.id,
    a.tipo,
    a.percorso_file,
    a.descrizione,
    a.creato_il
FROM miglioramento_allegati AS a
JOIN miglioramenti AS m
    ON m.id = a.miglioramento_id
JOIN miglioramenti_archivio AS a2
    ON a2.miglioramento_origine_id = m.id
WHERE m.stato = 'fatto';

-- Il CHECK della tabella legacy non può essere modificato in-place in SQLite:
-- si ricostruisce la tabella mantenendo gli ID degli elementi ancora attivi.
CREATE TABLE miglioramenti_step7g (
    id INTEGER PRIMARY KEY,
    autore_utente_id INTEGER NOT NULL,
    descrizione TEXT NOT NULL,
    modulo TEXT,
    stato TEXT NOT NULL DEFAULT 'da_approvare'
        CHECK (stato IN ('da_approvare', 'da_fare', 'scartato')),
    letto_admin_il TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (autore_utente_id) REFERENCES utenti(id) ON DELETE RESTRICT,
    CHECK (length(trim(descrizione)) > 0),
    CHECK (modulo IS NULL OR length(trim(modulo)) > 0)
);

INSERT INTO miglioramenti_step7g (
    id,
    autore_utente_id,
    descrizione,
    modulo,
    stato,
    letto_admin_il,
    creato_il,
    aggiornato_il
)
SELECT
    m.id,
    m.autore_utente_id,
    m.descrizione,
    m.modulo,
    CASE
        WHEN m.stato = 'scartato' THEN 'scartato'
        WHEN u.ruolo_sistema = 'admin' THEN 'da_fare'
        ELSE 'da_approvare'
    END,
    CASE
        -- Uno "scartato" è già stato oggetto di una decisione amministrativa.
        WHEN m.stato = 'scartato' THEN COALESCE(m.aggiornato_il, m.creato_il)
        -- I miglioramenti legacy creati da un admin sono già approvati/da fare.
        WHEN u.ruolo_sistema = 'admin' THEN COALESCE(m.aggiornato_il, m.creato_il)
        -- I miglioramenti legacy di utenti normali richiedono ancora approvazione.
        ELSE NULL
    END,
    m.creato_il,
    m.aggiornato_il
FROM miglioramenti AS m
JOIN utenti AS u
    ON u.id = m.autore_utente_id
WHERE m.stato <> 'fatto';

CREATE TABLE miglioramento_allegati_step7g (
    id INTEGER PRIMARY KEY,
    miglioramento_id INTEGER NOT NULL,
    tipo TEXT NOT NULL DEFAULT 'foto' CHECK (tipo = 'foto'),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (miglioramento_id)
        REFERENCES miglioramenti_step7g(id)
        ON DELETE CASCADE,
    CHECK (length(trim(percorso_file)) > 0)
);

INSERT INTO miglioramento_allegati_step7g (
    id,
    miglioramento_id,
    tipo,
    percorso_file,
    descrizione,
    creato_il
)
SELECT
    a.id,
    a.miglioramento_id,
    a.tipo,
    a.percorso_file,
    a.descrizione,
    a.creato_il
FROM miglioramento_allegati AS a
JOIN miglioramenti AS m
    ON m.id = a.miglioramento_id
WHERE m.stato <> 'fatto';

DROP TABLE miglioramento_allegati;
DROP TABLE miglioramenti;

ALTER TABLE miglioramenti_step7g RENAME TO miglioramenti;
ALTER TABLE miglioramento_allegati_step7g RENAME TO miglioramento_allegati;

CREATE INDEX idx_miglioramenti_autore_data
    ON miglioramenti(autore_utente_id, creato_il DESC, id DESC);

CREATE INDEX idx_miglioramenti_stato_data
    ON miglioramenti(stato, aggiornato_il DESC, id DESC);

CREATE INDEX idx_miglioramenti_non_letti_admin
    ON miglioramenti(letto_admin_il, stato, aggiornato_il DESC, id DESC);

CREATE INDEX idx_miglioramento_allegati_miglioramento
    ON miglioramento_allegati(miglioramento_id, creato_il, id);
