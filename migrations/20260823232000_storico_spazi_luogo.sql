-- Step 7.1B - storico dei luoghi multi-spazio
-- Conserva lo spazio della posizione separatamente dallo spazio proprietario
-- dell'entita', sia nel contesto dell'evento sia nel prima/dopo degli spostamenti.

ALTER TABLE storico_eventi
ADD COLUMN luogo_spazio_id INTEGER REFERENCES spazi(id) ON DELETE SET NULL;

ALTER TABLE storico_eventi
ADD COLUMN luogo_spazio_nome_snapshot TEXT;

ALTER TABLE storico_cambi_luogo
ADD COLUMN spazio_prima_id INTEGER REFERENCES spazi(id) ON DELETE SET NULL;

ALTER TABLE storico_cambi_luogo
ADD COLUMN spazio_prima_nome TEXT;

ALTER TABLE storico_cambi_luogo
ADD COLUMN spazio_dopo_id INTEGER REFERENCES spazi(id) ON DELETE SET NULL;

ALTER TABLE storico_cambi_luogo
ADD COLUMN spazio_dopo_nome TEXT;

-- Backfill degli eventi esistenti. abitazione_storico_id punta all'identita'
-- storica della casa, che conserva a sua volta lo spazio proprietario.
UPDATE storico_eventi
SET luogo_spazio_id = (
        SELECT se.spazio_id
        FROM storico_entita se
        WHERE se.id = storico_eventi.abitazione_storico_id
    ),
    luogo_spazio_nome_snapshot = (
        SELECT s.nome
        FROM storico_entita se
        JOIN spazi s ON s.id = se.spazio_id
        WHERE se.id = storico_eventi.abitazione_storico_id
    )
WHERE abitazione_storico_id IS NOT NULL;

UPDATE storico_cambi_luogo
SET spazio_prima_id = (
        SELECT se.spazio_id
        FROM storico_entita se
        WHERE se.id = storico_cambi_luogo.abitazione_prima_id
    ),
    spazio_prima_nome = (
        SELECT s.nome
        FROM storico_entita se
        JOIN spazi s ON s.id = se.spazio_id
        WHERE se.id = storico_cambi_luogo.abitazione_prima_id
    )
WHERE abitazione_prima_id IS NOT NULL;

UPDATE storico_cambi_luogo
SET spazio_dopo_id = (
        SELECT se.spazio_id
        FROM storico_entita se
        WHERE se.id = storico_cambi_luogo.abitazione_dopo_id
    ),
    spazio_dopo_nome = (
        SELECT s.nome
        FROM storico_entita se
        JOIN spazi s ON s.id = se.spazio_id
        WHERE se.id = storico_cambi_luogo.abitazione_dopo_id
    )
WHERE abitazione_dopo_id IS NOT NULL;

CREATE INDEX idx_storico_eventi_luogo_spazio
    ON storico_eventi (luogo_spazio_id, avvenuto_il DESC, id DESC);
