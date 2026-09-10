-- Turni e routine: prima fetta (10 settembre 2026), decisa con Alessio l'8
-- settembre 2026 come prossimo blocco della sequenza Alimentazione dopo la
-- lista della spesa, con design di dettaglio dell'10 settembre 2026.
--
-- Principi, stesso schema di planner_alimentare/lista_spesa:
-- - il modello (turno_modelli/turno_modello_pasti) contiene i default,
--   scelti dall'utente;
-- - l'assegnazione a una data (turno_assegnazioni/turno_assegnazione_pasti)
--   COPIA i pasti del modello al momento dell'assegnazione: modificare o
--   rimuovere un pasto assegnato non tocca mai il modello, e modificare il
--   modello dopo non cambia le assegnazioni già fatte;
-- - un'assegnazione è sempre riferita a un profilo alimentare (la persona),
--   non all'account -- stesso concetto di partecipante già usato dal
--   planner (vedi docs/previsto/turni-e-routine.md, "le assegnazioni
--   giornaliere restano riferite alla persona/profilo");
-- - nessun reminder: l'infrastruttura non esiste ancora nel progetto,
--   arriverà in un blocco successivo (vedi docs/moduli/turni-e-routine.md).

CREATE TABLE turno_modelli (
    id INTEGER PRIMARY KEY,
    proprietario_utente_id INTEGER NOT NULL
        REFERENCES utenti(id) ON DELETE RESTRICT,
    spazio_id INTEGER
        REFERENCES spazi(id) ON DELETE CASCADE,
    nome TEXT NOT NULL,
    nome_normalizzato TEXT NOT NULL,
    archiviato INTEGER NOT NULL DEFAULT 0
        CHECK (archiviato IN (0, 1)),
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0),
    CHECK (length(trim(nome_normalizzato)) > 0)
);

CREATE UNIQUE INDEX idx_turno_modelli_personale_nome
    ON turno_modelli (proprietario_utente_id, nome_normalizzato)
    WHERE spazio_id IS NULL AND archiviato = 0;

CREATE UNIQUE INDEX idx_turno_modelli_spazio_nome
    ON turno_modelli (spazio_id, nome_normalizzato)
    WHERE spazio_id IS NOT NULL AND archiviato = 0;

CREATE INDEX idx_turno_modelli_spazio
    ON turno_modelli (spazio_id, archiviato, id);

CREATE INDEX idx_turno_modelli_proprietario
    ON turno_modelli (proprietario_utente_id, archiviato, id);

-- Un modello condiviso può essere creato soltanto in uno spazio dove il
-- proprietario possiede una membership scrivibile -- stesso vincolo di
-- trg_planner_spazio_membership_insert.
CREATE TRIGGER trg_turno_modelli_spazio_membership_insert
BEFORE INSERT ON turno_modelli
WHEN NEW.spazio_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.spazio_id = NEW.spazio_id
      AND ms.utente_id = NEW.proprietario_utente_id
      AND ms.ruolo IN ('proprietario', 'amministratore', 'membro')
)
BEGIN
    SELECT RAISE(ABORT, 'utente senza permesso di creare un modello turno nello spazio');
END;

CREATE TABLE turno_modello_pasti (
    id INTEGER PRIMARY KEY,
    modello_id INTEGER NOT NULL
        REFERENCES turno_modelli(id) ON DELETE CASCADE,
    tipo_pasto TEXT NOT NULL CHECK (
        tipo_pasto IN (
            'colazione',
            'spuntino_mattina',
            'pranzo',
            'spuntino_pomeriggio',
            'cena',
            'altro'
        )
    ),
    orario TEXT
        CHECK (orario IS NULL OR (length(orario) = 5 AND substr(orario, 3, 1) = ':')),
    situazione TEXT NOT NULL CHECK (
        situazione IN ('casa', 'lavoro', 'fuori', 'saltato', 'altro')
    ),
    preparazione_anticipata INTEGER NOT NULL DEFAULT 0
        CHECK (preparazione_anticipata IN (0, 1)),
    preparazione_note TEXT,
    nota TEXT,
    ordinamento INTEGER NOT NULL DEFAULT 0,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- La nota di preparazione ha senso solo se il pasto è segnato come "da
    -- preparare in anticipo": senza, resterebbe un campo orfano.
    CHECK (preparazione_anticipata = 1 OR preparazione_note IS NULL)
);

CREATE INDEX idx_turno_modello_pasti_modello
    ON turno_modello_pasti (modello_id, ordinamento, id);

CREATE TABLE turno_assegnazioni (
    id INTEGER PRIMARY KEY,
    modello_id INTEGER
        REFERENCES turno_modelli(id) ON DELETE SET NULL,
    -- Snapshot del nome del modello al momento dell'assegnazione: resta
    -- leggibile anche se il modello viene poi rinominato, archiviato o
    -- cancellato del tutto -- stesso principio di
    -- planner_pasto_profili.profilo_nome_snapshot.
    modello_nome_snapshot TEXT NOT NULL,
    proprietario_utente_id INTEGER NOT NULL
        REFERENCES utenti(id) ON DELETE RESTRICT,
    spazio_id INTEGER
        REFERENCES spazi(id) ON DELETE CASCADE,
    profilo_alimentare_id INTEGER
        REFERENCES profili_alimentari(id) ON DELETE SET NULL,
    profilo_nome_snapshot TEXT NOT NULL,
    data TEXT NOT NULL,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (date(data) IS NOT NULL),
    CHECK (length(trim(modello_nome_snapshot)) > 0),
    CHECK (length(trim(profilo_nome_snapshot)) > 0)
);

-- Un solo turno assegnato per profilo e data: due modelli sulla stessa
-- persona nello stesso giorno sarebbero ambigui per il suggerimento nel
-- planner. Parziale su profilo non nullo: se il profilo viene cancellato
-- (ON DELETE SET NULL) l'assegnazione storica resta, ma non partecipa più
-- al vincolo di unicità.
CREATE UNIQUE INDEX idx_turno_assegnazioni_profilo_data
    ON turno_assegnazioni (profilo_alimentare_id, data)
    WHERE profilo_alimentare_id IS NOT NULL;

CREATE INDEX idx_turno_assegnazioni_data
    ON turno_assegnazioni (data, id);

CREATE INDEX idx_turno_assegnazioni_spazio
    ON turno_assegnazioni (spazio_id, data, id);

CREATE TABLE turno_assegnazione_pasti (
    id INTEGER PRIMARY KEY,
    assegnazione_id INTEGER NOT NULL
        REFERENCES turno_assegnazioni(id) ON DELETE CASCADE,
    tipo_pasto TEXT NOT NULL CHECK (
        tipo_pasto IN (
            'colazione',
            'spuntino_mattina',
            'pranzo',
            'spuntino_pomeriggio',
            'cena',
            'altro'
        )
    ),
    orario TEXT
        CHECK (orario IS NULL OR (length(orario) = 5 AND substr(orario, 3, 1) = ':')),
    situazione TEXT NOT NULL CHECK (
        situazione IN ('casa', 'lavoro', 'fuori', 'saltato', 'altro')
    ),
    preparazione_anticipata INTEGER NOT NULL DEFAULT 0
        CHECK (preparazione_anticipata IN (0, 1)),
    preparazione_note TEXT,
    nota TEXT,
    ordinamento INTEGER NOT NULL DEFAULT 0,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (preparazione_anticipata = 1 OR preparazione_note IS NULL)
);

CREATE INDEX idx_turno_assegnazione_pasti_assegnazione
    ON turno_assegnazione_pasti (assegnazione_id, ordinamento, id);
