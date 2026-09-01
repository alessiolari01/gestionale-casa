-- Step 7.3B - Planner: esito pasto saltato.
ALTER TABLE planner_pasti ADD COLUMN saltato_il TEXT;

CREATE INDEX idx_planner_pasti_saltato
ON planner_pasti (saltato_il, planner_id, data_pasto, id);

CREATE TRIGGER trg_planner_pasto_esito_coerente_update
BEFORE UPDATE ON planner_pasti
WHEN NEW.saltato_il IS NOT NULL
AND (NEW.stato <> 'pianificato' OR NEW.completato_il IS NOT NULL)
BEGIN
    SELECT RAISE(ABORT, 'pasto saltato incompatibile con consumato');
END;

CREATE TRIGGER trg_planner_pasto_saltato_immutabile
BEFORE UPDATE ON planner_pasti
WHEN OLD.saltato_il IS NOT NULL
AND (
    NEW.planner_id <> OLD.planner_id
    OR NEW.data_pasto <> OLD.data_pasto
    OR NEW.tipo_pasto <> OLD.tipo_pasto
    OR NEW.ordinamento <> OLD.ordinamento
    OR NEW.ricetta_id IS NOT OLD.ricetta_id
    OR NEW.ricetta_nome_snapshot <> OLD.ricetta_nome_snapshot
    OR NEW.ricetta_porzione_base_snapshot <> OLD.ricetta_porzione_base_snapshot
    OR NEW.ricetta_aggiornato_il_snapshot IS NOT OLD.ricetta_aggiornato_il_snapshot
    OR NEW.stato <> OLD.stato
    OR NEW.completato_il IS NOT OLD.completato_il
    OR NEW.saltato_il IS NOT OLD.saltato_il
)
BEGIN
    SELECT RAISE(ABORT, 'pasto saltato non modificabile');
END;

CREATE TRIGGER trg_planner_pasto_saltato_non_eliminabile
BEFORE DELETE ON planner_pasti
WHEN OLD.saltato_il IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'pasto saltato non eliminabile');
END;
