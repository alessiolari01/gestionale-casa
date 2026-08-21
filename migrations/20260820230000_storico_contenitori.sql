-- Step 6C.4 - contenitori nello storico trasversale.
--
-- La migration estende gli snapshot esistenti senza riscrivere il passato.
-- I contenitori gia' presenti vengono registrati in storico_entita, ma NON
-- vengono creati eventi retroattivi o fittizi.

ALTER TABLE storico_eventi
ADD COLUMN contenitore_storico_id INTEGER
    REFERENCES storico_entita(id) ON DELETE SET NULL;

ALTER TABLE storico_eventi
ADD COLUMN contenitore_percorso_snapshot TEXT;

CREATE INDEX idx_storico_eventi_contenitore
    ON storico_eventi (contenitore_storico_id, avvenuto_il DESC);

ALTER TABLE storico_cambi_luogo
ADD COLUMN contenitore_prima_id INTEGER
    REFERENCES storico_entita(id) ON DELETE SET NULL;

ALTER TABLE storico_cambi_luogo
ADD COLUMN contenitore_prima_percorso TEXT;

ALTER TABLE storico_cambi_luogo
ADD COLUMN contenitore_dopo_id INTEGER
    REFERENCES storico_entita(id) ON DELETE SET NULL;

ALTER TABLE storico_cambi_luogo
ADD COLUMN contenitore_dopo_percorso TEXT;

-- Backfill delle sole identita': da questo momento i contenitori esistenti
-- sono tracciabili. Non viene inventata alcuna cronologia precedente.
INSERT INTO storico_entita (tipo_entita, id_origine, nome_ultimo)
SELECT 'contenitore', c.id, c.nome
FROM contenitori c
WHERE NOT EXISTS (
    SELECT 1
    FROM storico_entita se
    WHERE se.tipo_entita = 'contenitore'
      AND se.id_origine = c.id
      AND se.eliminato_il IS NULL
);
