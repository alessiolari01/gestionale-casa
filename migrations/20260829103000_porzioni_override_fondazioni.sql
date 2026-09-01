-- Step 7.2I.0 - Fondazioni Porzioni e override.
--
-- Decisioni:
-- - la ricetta conserva le quantità base e il numero di porzioni;
-- - il profilo alimentare conserva solo la propria personalizzazione;
-- - la porzione personale è un fattore rispetto alla porzione standard della ricetta;
-- - l'override riguarda una specifica riga ingrediente della ricetta;
-- - l'esclusione è distinta da una quantità pari a zero;
-- - nessuna riga esistente viene modificata o popolata retroattivamente.

CREATE TABLE profilo_ricetta_porzioni (
    profilo_alimentare_id INTEGER NOT NULL
        REFERENCES profili_alimentari(id) ON DELETE CASCADE,
    ricetta_id INTEGER NOT NULL
        REFERENCES ricette(id) ON DELETE CASCADE,

    fattore_porzione REAL NOT NULL DEFAULT 1.0
        CHECK (fattore_porzione > 0),

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (profilo_alimentare_id, ricetta_id)
);

CREATE INDEX idx_profilo_ricetta_porzioni_ricetta
    ON profilo_ricetta_porzioni (ricetta_id, profilo_alimentare_id);

CREATE TABLE profilo_ricetta_ingredienti_override (
    profilo_alimentare_id INTEGER NOT NULL
        REFERENCES profili_alimentari(id) ON DELETE CASCADE,
    ricetta_ingrediente_id INTEGER NOT NULL
        REFERENCES ricetta_ingredienti(id) ON DELETE CASCADE,

    tipo_override TEXT NOT NULL
        CHECK (tipo_override IN ('quantita', 'escluso')),

    quantita_override REAL,

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (profilo_alimentare_id, ricetta_ingrediente_id),

    CHECK (
        (tipo_override = 'quantita'
            AND quantita_override IS NOT NULL
            AND quantita_override > 0)
        OR
        (tipo_override = 'escluso'
            AND quantita_override IS NULL)
    )
);

CREATE INDEX idx_profilo_ricetta_override_ingrediente
    ON profilo_ricetta_ingredienti_override (
        ricetta_ingrediente_id,
        profilo_alimentare_id
    );
