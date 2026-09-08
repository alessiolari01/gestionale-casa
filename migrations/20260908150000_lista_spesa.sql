-- Lista della spesa: prossimo macro-step deciso l'8 settembre 2026
-- (docs/previsto/lista-della-spesa.md).
--
-- Principi, stessi del planner (vedi
-- migrations/20260831113000_planner_alimentare_fondazioni.sql):
-- - una sola lista attiva per spazio condiviso, una per utente senza spazio;
-- - la lista ha un proprio intervallo di date, indipendente dalle settimane
--   del planner: aggrega su tutti i planner_alimentari non archiviati dello
--   stesso spazio/proprietario la cui data_pasto cade nell'intervallo;
-- - le voci comprate sono congelate a livello di database, non solo di UI.

CREATE TABLE liste_spesa (
    id INTEGER PRIMARY KEY,
    proprietario_utente_id INTEGER NOT NULL
        REFERENCES utenti(id) ON DELETE RESTRICT,
    spazio_id INTEGER
        REFERENCES spazi(id) ON DELETE CASCADE,
    data_inizio TEXT NOT NULL,
    data_fine TEXT NOT NULL,
    aggiornata_il TEXT,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (date(data_inizio) IS NOT NULL),
    CHECK (date(data_fine) IS NOT NULL),
    CHECK (date(data_fine) >= date(data_inizio))
);

-- Una sola lista attiva per spazio condiviso, e una per utente senza spazio
-- -- stesso schema degli indici unique parziali di planner_alimentari.
CREATE UNIQUE INDEX idx_lista_spesa_personale
    ON liste_spesa (proprietario_utente_id)
    WHERE spazio_id IS NULL;

CREATE UNIQUE INDEX idx_lista_spesa_spazio
    ON liste_spesa (spazio_id)
    WHERE spazio_id IS NOT NULL;

CREATE INDEX idx_lista_spesa_periodo
    ON liste_spesa (data_inizio, data_fine, id);

CREATE TABLE liste_spesa_voci (
    id INTEGER PRIMARY KEY,
    lista_id INTEGER NOT NULL
        REFERENCES liste_spesa(id) ON DELETE CASCADE,
    origine TEXT NOT NULL CHECK (origine IN ('generato', 'manuale')),
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE SET NULL,
    descrizione TEXT NOT NULL,
    quantita REAL CHECK (quantita IS NULL OR quantita > 0),
    unita_simbolo TEXT,
    comprato INTEGER NOT NULL DEFAULT 0 CHECK (comprato IN (0, 1)),
    comprato_il TEXT,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(descrizione)) > 0),
    CHECK (
        (comprato = 0 AND comprato_il IS NULL)
        OR (comprato = 1 AND comprato_il IS NOT NULL)
    ),
    -- Una voce 'generato' ha sempre quantita' e unita' (viene dall'aggregazione
    -- del planner); una voce 'manuale' puo' restare senza, per un prodotto
    -- commerciale non modellato ("Detersivo piatti", senza quantita').
    CHECK (
        origine = 'manuale'
        OR (quantita IS NOT NULL AND unita_simbolo IS NOT NULL)
    )
);

CREATE INDEX idx_lista_spesa_voci_lista
    ON liste_spesa_voci (lista_id, comprato, id);

CREATE INDEX idx_lista_spesa_voci_alimento
    ON liste_spesa_voci (alimento_id, lista_id);

-- Congelamento a livello database, stesso principio di
-- trg_planner_pasto_completato_immutabile: una volta comprato=1, i campi
-- funzionali non si toccano piu'. Il toggle comprato stesso (tornare a 0)
-- resta permesso -- a differenza del pasto completato del planner, qui
-- l'utente puo' "scomprare" per errore e la voce torna disponibile al
-- prossimo refresh esplicito (se era 'generato').
CREATE TRIGGER trg_lista_spesa_voce_comprata_immutabile
BEFORE UPDATE ON liste_spesa_voci
WHEN OLD.comprato = 1
AND (
    NEW.lista_id <> OLD.lista_id
    OR NEW.origine <> OLD.origine
    OR NEW.alimento_id IS NOT OLD.alimento_id
    OR NEW.descrizione <> OLD.descrizione
    OR NEW.quantita IS NOT OLD.quantita
    OR NEW.unita_simbolo IS NOT OLD.unita_simbolo
)
BEGIN
    SELECT RAISE(ABORT, 'voce comprata non modificabile, solo il toggle comprato e'' permesso');
END;
