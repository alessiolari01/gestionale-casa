-- Step 7.3A - Planner alimentare: fondazioni.
--
-- Principi:
-- - il planner appartiene a un utente e può essere personale o legato a uno spazio;
-- - i partecipanti sono Profili alimentari reali, non account Telegram;
-- - un pasto conserva snapshot leggibili e tecnici sufficienti a non cambiare
--   silenziosamente quando ricetta/porzioni/override vengono modificati;
-- - un pasto completato resta congelato;
-- - nessun evento/planner esistente viene inventato retroattivamente.

CREATE TABLE planner_alimentari (
    id INTEGER PRIMARY KEY,
    proprietario_utente_id INTEGER NOT NULL
        REFERENCES utenti(id) ON DELETE RESTRICT,
    spazio_id INTEGER
        REFERENCES spazi(id) ON DELETE CASCADE,
    nome TEXT NOT NULL,
    nome_normalizzato TEXT NOT NULL,
    data_inizio TEXT NOT NULL,
    data_fine TEXT NOT NULL,
    archiviato INTEGER NOT NULL DEFAULT 0
        CHECK (archiviato IN (0, 1)),
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0),
    CHECK (length(trim(nome_normalizzato)) > 0),
    CHECK (date(data_inizio) IS NOT NULL),
    CHECK (date(data_fine) IS NOT NULL),
    CHECK (date(data_fine) >= date(data_inizio))
);

CREATE UNIQUE INDEX idx_planner_personale_nome_periodo
    ON planner_alimentari (
        proprietario_utente_id,
        nome_normalizzato,
        data_inizio,
        data_fine
    )
    WHERE spazio_id IS NULL AND archiviato = 0;

CREATE UNIQUE INDEX idx_planner_spazio_nome_periodo
    ON planner_alimentari (
        spazio_id,
        nome_normalizzato,
        data_inizio,
        data_fine
    )
    WHERE spazio_id IS NOT NULL AND archiviato = 0;

CREATE INDEX idx_planner_periodo
    ON planner_alimentari (data_inizio, data_fine, archiviato, id);

CREATE INDEX idx_planner_spazio
    ON planner_alimentari (spazio_id, archiviato, data_inizio, id);

CREATE TABLE planner_pasti (
    id INTEGER PRIMARY KEY,
    planner_id INTEGER NOT NULL
        REFERENCES planner_alimentari(id) ON DELETE CASCADE,
    data_pasto TEXT NOT NULL,
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
    ordinamento INTEGER NOT NULL DEFAULT 0,
    ricetta_id INTEGER
        REFERENCES ricette(id) ON DELETE SET NULL,

    -- Snapshot minimo della ricetta assegnata.
    ricetta_nome_snapshot TEXT NOT NULL,
    ricetta_porzione_base_snapshot INTEGER NOT NULL
        CHECK (ricetta_porzione_base_snapshot > 0),
    ricetta_aggiornato_il_snapshot TEXT,

    stato TEXT NOT NULL DEFAULT 'pianificato'
        CHECK (stato IN ('pianificato', 'completato')),
    completato_il TEXT,

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (date(data_pasto) IS NOT NULL),
    CHECK (length(trim(ricetta_nome_snapshot)) > 0),
    CHECK (
        (stato = 'pianificato' AND completato_il IS NULL)
        OR
        (stato = 'completato' AND completato_il IS NOT NULL)
    )
);

CREATE INDEX idx_planner_pasti_giorno
    ON planner_pasti (planner_id, data_pasto, tipo_pasto, ordinamento, id);

CREATE INDEX idx_planner_pasti_ricetta
    ON planner_pasti (ricetta_id, stato, id);

CREATE TABLE planner_pasto_profili (
    pasto_id INTEGER NOT NULL
        REFERENCES planner_pasti(id) ON DELETE CASCADE,
    profilo_alimentare_id INTEGER
        REFERENCES profili_alimentari(id) ON DELETE SET NULL,

    -- Gli snapshot conservano il significato del pasto anche se il profilo
    -- viene rinominato/archiviato o le sue personalizzazioni cambiano.
    profilo_nome_snapshot TEXT NOT NULL,
    fattore_porzione_snapshot REAL NOT NULL DEFAULT 1.0
        CHECK (fattore_porzione_snapshot > 0),

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (pasto_id, profilo_alimentare_id),
    CHECK (length(trim(profilo_nome_snapshot)) > 0)
);

CREATE INDEX idx_planner_pasto_profili_profilo
    ON planner_pasto_profili (profilo_alimentare_id, pasto_id);

CREATE TABLE planner_pasto_ingredienti_snapshot (
    id INTEGER PRIMARY KEY,
    pasto_id INTEGER NOT NULL
        REFERENCES planner_pasti(id) ON DELETE CASCADE,
    profilo_alimentare_id INTEGER
        REFERENCES profili_alimentari(id) ON DELETE SET NULL,
    ricetta_ingrediente_id INTEGER
        REFERENCES ricetta_ingredienti(id) ON DELETE SET NULL,
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE SET NULL,

    alimento_nome_snapshot TEXT NOT NULL,
    unita_simbolo_snapshot TEXT NOT NULL,

    quantita_base_snapshot REAL NOT NULL
        CHECK (quantita_base_snapshot > 0),
    quantita_scalata_snapshot REAL NOT NULL
        CHECK (quantita_scalata_snapshot > 0),

    tipo_override_snapshot TEXT NOT NULL DEFAULT 'nessuno'
        CHECK (tipo_override_snapshot IN ('nessuno', 'quantita', 'escluso')),

    quantita_finale_snapshot REAL
        CHECK (
            quantita_finale_snapshot IS NULL
            OR quantita_finale_snapshot > 0
        ),

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(alimento_nome_snapshot)) > 0),
    CHECK (length(trim(unita_simbolo_snapshot)) > 0),

    CHECK (
        (tipo_override_snapshot = 'escluso' AND quantita_finale_snapshot IS NULL)
        OR
        (tipo_override_snapshot <> 'escluso' AND quantita_finale_snapshot IS NOT NULL)
    )
);

CREATE INDEX idx_planner_snapshot_pasto_profilo
    ON planner_pasto_ingredienti_snapshot (
        pasto_id,
        profilo_alimentare_id,
        id
    );

CREATE INDEX idx_planner_snapshot_alimento
    ON planner_pasto_ingredienti_snapshot (
        alimento_id,
        pasto_id,
        profilo_alimentare_id
    );

-- Un planner condiviso può essere creato soltanto in uno spazio dove il
-- proprietario possiede una membership scrivibile.
CREATE TRIGGER trg_planner_spazio_membership_insert
BEFORE INSERT ON planner_alimentari
WHEN NEW.spazio_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.spazio_id = NEW.spazio_id
      AND ms.utente_id = NEW.proprietario_utente_id
      AND ms.ruolo IN ('proprietario', 'amministratore', 'membro')
)
BEGIN
    SELECT RAISE(ABORT, 'utente senza permesso di creare planner nello spazio');
END;

-- La data del pasto deve rientrare nel periodo del planner.
CREATE TRIGGER trg_planner_pasto_periodo_insert
BEFORE INSERT ON planner_pasti
WHEN NOT EXISTS (
    SELECT 1
    FROM planner_alimentari p
    WHERE p.id = NEW.planner_id
      AND date(NEW.data_pasto) BETWEEN date(p.data_inizio) AND date(p.data_fine)
)
BEGIN
    SELECT RAISE(ABORT, 'data pasto fuori dal periodo del planner');
END;

CREATE TRIGGER trg_planner_pasto_periodo_update
BEFORE UPDATE OF planner_id, data_pasto ON planner_pasti
WHEN NOT EXISTS (
    SELECT 1
    FROM planner_alimentari p
    WHERE p.id = NEW.planner_id
      AND date(NEW.data_pasto) BETWEEN date(p.data_inizio) AND date(p.data_fine)
)
BEGIN
    SELECT RAISE(ABORT, 'data pasto fuori dal periodo del planner');
END;

-- Un pasto completato è congelato: i campi funzionali non possono più essere
-- modificati. In futuro potrà essere solo consultato/storicizzato.
CREATE TRIGGER trg_planner_pasto_completato_immutabile
BEFORE UPDATE ON planner_pasti
WHEN OLD.stato = 'completato'
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
)
BEGIN
    SELECT RAISE(ABORT, 'pasto completato non modificabile');
END;
