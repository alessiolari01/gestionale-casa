-- Step 7.2H.0 - Fondazioni Profili alimentari.
--
-- Decisioni architetturali:
-- - account Telegram, utente interno e profilo alimentare restano entità distinte;
-- - un profilo può rappresentare una persona senza account;
-- - il profilo non può essere globale: è privato oppure visibile tramite uno o più spazi;
-- - gli spazi sono contesti di collaborazione; abitazioni/stanze/contenitori restano luoghi fisici;
-- - proprietà/gestione, visibilità e permessi restano concetti separati;
-- - nessun dato esistente viene modificato o migrato in profili automaticamente.

CREATE TABLE profili_alimentari (
    id INTEGER PRIMARY KEY,

    gestore_utente_id INTEGER NOT NULL
        REFERENCES utenti(id) ON DELETE RESTRICT,

    utente_collegato_id INTEGER
        REFERENCES utenti(id) ON DELETE SET NULL,

    nome TEXT NOT NULL,
    nome_normalizzato TEXT NOT NULL,
    note TEXT,

    archiviato INTEGER NOT NULL DEFAULT 0
        CHECK (archiviato IN (0, 1)),

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(nome)) > 0),
    CHECK (length(trim(nome_normalizzato)) > 0),
    CHECK (note IS NULL OR length(trim(note)) > 0)
);

-- Un gestore non può avere due profili attivi con lo stesso nome normalizzato.
CREATE UNIQUE INDEX idx_profili_alimentari_gestore_nome_attivi
    ON profili_alimentari (gestore_utente_id, nome_normalizzato)
    WHERE archiviato = 0;

-- Un account può rappresentare al massimo un profilo alimentare attivo.
-- Archiviare un vecchio profilo consente, se davvero necessario, un nuovo collegamento.
CREATE UNIQUE INDEX idx_profili_alimentari_utente_collegato_attivo
    ON profili_alimentari (utente_collegato_id)
    WHERE utente_collegato_id IS NOT NULL AND archiviato = 0;

CREATE INDEX idx_profili_alimentari_gestore
    ON profili_alimentari (
        gestore_utente_id,
        archiviato,
        nome_normalizzato,
        id
    );

CREATE INDEX idx_profili_alimentari_utente_collegato
    ON profili_alimentari (utente_collegato_id, archiviato, id);

-- Visibilità del profilo nei contesti di collaborazione.
-- Nessuna riga = profilo privato del gestore (oltre all'eventuale account collegato,
-- secondo le regole applicative che verranno abilitate nel blocco UI).
CREATE TABLE profilo_alimentare_spazi (
    profilo_alimentare_id INTEGER NOT NULL
        REFERENCES profili_alimentari(id) ON DELETE CASCADE,

    spazio_id INTEGER NOT NULL
        REFERENCES spazi(id) ON DELETE CASCADE,

    condiviso_da_utente_id INTEGER NOT NULL
        REFERENCES utenti(id) ON DELETE RESTRICT,

    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (profilo_alimentare_id, spazio_id)
);

CREATE INDEX idx_profilo_alimentare_spazi_spazio
    ON profilo_alimentare_spazi (spazio_id, profilo_alimentare_id);

CREATE INDEX idx_profilo_alimentare_spazi_profilo
    ON profilo_alimentare_spazi (profilo_alimentare_id, spazio_id);

-- Non si può aggiungere a uno spazio un profilo già archiviato.
CREATE TRIGGER trg_profilo_alimentare_spazio_profilo_attivo_insert
BEFORE INSERT ON profilo_alimentare_spazi
WHEN NOT EXISTS (
    SELECT 1
    FROM profili_alimentari pa
    WHERE pa.id = NEW.profilo_alimentare_id
      AND pa.archiviato = 0
)
BEGIN
    SELECT RAISE(ABORT, 'profilo alimentare non condivisibile');
END;

-- Nel primo blocco operativo solo il gestore del profilo può condividerlo.
-- I collaboratori espliciti potranno essere aggiunti in un blocco successivo
-- riusando permessi_risorsa con controlli fail-closed dedicati.
CREATE TRIGGER trg_profilo_alimentare_spazio_gestore_insert
BEFORE INSERT ON profilo_alimentare_spazi
WHEN NOT EXISTS (
    SELECT 1
    FROM profili_alimentari pa
    WHERE pa.id = NEW.profilo_alimentare_id
      AND pa.gestore_utente_id = NEW.condiviso_da_utente_id
)
BEGIN
    SELECT RAISE(ABORT, 'solo il gestore può condividere il profilo alimentare');
END;

-- Il gestore deve avere diritto di scrittura nello spazio di destinazione.
CREATE TRIGGER trg_profilo_alimentare_spazio_membership_insert
BEFORE INSERT ON profilo_alimentare_spazi
WHEN NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.spazio_id = NEW.spazio_id
      AND ms.utente_id = NEW.condiviso_da_utente_id
      AND ms.ruolo IN ('proprietario', 'amministratore', 'membro')
)
BEGIN
    SELECT RAISE(ABORT, 'utente senza permesso di condividere il profilo nello spazio');
END;
