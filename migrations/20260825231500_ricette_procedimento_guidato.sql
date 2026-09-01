-- Step 7.2F.1 - Ricette operative: procedimento guidato a step e media per step.
--
-- Obiettivi:
-- - il procedimento non e' piu' un singolo testo libero;
-- - ogni ricetta ha step ordinati e numerati;
-- - ogni step puo' avere zero o piu' foto/video;
-- - visualizzazione completa e modalita' guidata usano gli stessi dati;
-- - eventuali ricette legacy con `procedimento` testuale vengono migrate in uno step #1.
--
-- Nota: la colonna legacy `ricette.procedimento` resta nello schema per
-- compatibilita' con le migration storiche, ma il codice applicativo da questo
-- step usa `ricetta_step` come fonte autorevole.

CREATE TABLE ricetta_step (
    id INTEGER PRIMARY KEY,
    ricetta_id INTEGER NOT NULL
        REFERENCES ricette(id) ON DELETE CASCADE,
    numero INTEGER NOT NULL CHECK (numero > 0),
    testo TEXT NOT NULL,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(testo)) > 0),
    UNIQUE (ricetta_id, numero)
);

CREATE INDEX idx_ricetta_step_ordine
    ON ricetta_step (ricetta_id, numero, id);

CREATE TABLE ricetta_step_media (
    id INTEGER PRIMARY KEY,
    ricetta_step_id INTEGER NOT NULL
        REFERENCES ricetta_step(id) ON DELETE CASCADE,
    tipo_media TEXT NOT NULL CHECK (tipo_media IN ('foto', 'video')),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    ordinamento INTEGER NOT NULL DEFAULT 0 CHECK (ordinamento >= 0),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(percorso_file)) > 0)
);

CREATE INDEX idx_ricetta_step_media_ordine
    ON ricetta_step_media (ricetta_step_id, tipo_media, ordinamento, id);

-- Migrazione conservativa del vecchio procedimento testuale, se presente.
INSERT INTO ricetta_step (ricetta_id, numero, testo, creato_il, aggiornato_il)
SELECT
    r.id,
    1,
    trim(r.procedimento),
    r.creato_il,
    r.aggiornato_il
FROM ricette r
WHERE r.procedimento IS NOT NULL
  AND length(trim(r.procedimento)) > 0
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step rs WHERE rs.ricetta_id = r.id
  );

-- Vista utile per dettaglio/lista e per la modalita' guidata.
CREATE VIEW v_ricetta_step_con_media AS
SELECT
    rs.id AS ricetta_step_id,
    rs.ricetta_id,
    rs.numero,
    rs.testo,
    SUM(CASE WHEN rsm.tipo_media = 'foto' THEN 1 ELSE 0 END) AS foto_count,
    SUM(CASE WHEN rsm.tipo_media = 'video' THEN 1 ELSE 0 END) AS video_count
FROM ricetta_step rs
LEFT JOIN ricetta_step_media rsm ON rsm.ricetta_step_id = rs.id
GROUP BY rs.id, rs.ricetta_id, rs.numero, rs.testo;
