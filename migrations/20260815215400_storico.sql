-- Step 6B - Storico trasversale.
--
-- Questa migration NON modifica le migration precedenti e NON cancella dati.
-- Le entita' gia' presenti vengono soltanto registrate in storico_entita;
-- non vengono creati eventi retroattivi/fittizi.

CREATE TABLE storico_entita (
    id INTEGER PRIMARY KEY,
    tipo_entita TEXT NOT NULL,
    id_origine INTEGER,
    nome_ultimo TEXT NOT NULL,
    creato_il TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    eliminato_il TEXT
);

-- Un ID applicativo puo' essere riutilizzato in futuro solo dopo che la vecchia
-- entita' storica e' stata marcata come eliminata.
CREATE UNIQUE INDEX idx_storico_entita_origine_attiva
    ON storico_entita (tipo_entita, id_origine)
    WHERE id_origine IS NOT NULL AND eliminato_il IS NULL;

CREATE INDEX idx_storico_entita_tipo
    ON storico_entita (tipo_entita);

CREATE TABLE storico_eventi (
    id INTEGER PRIMARY KEY,
    entita_storico_id INTEGER NOT NULL
        REFERENCES storico_entita(id) ON DELETE RESTRICT,
    modulo TEXT NOT NULL,
    componente TEXT NOT NULL,
    operazione TEXT NOT NULL,
    nome_entita_snapshot TEXT NOT NULL,

    abitazione_storico_id INTEGER
        REFERENCES storico_entita(id) ON DELETE SET NULL,
    abitazione_nome_snapshot TEXT,

    stanza_storico_id INTEGER
        REFERENCES storico_entita(id) ON DELETE SET NULL,
    stanza_nome_snapshot TEXT,

    evento_padre_id INTEGER
        REFERENCES storico_eventi(id) ON DELETE SET NULL,

    avvenuto_il TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_storico_eventi_entita_data
    ON storico_eventi (entita_storico_id, avvenuto_il DESC, id DESC);

CREATE INDEX idx_storico_eventi_data
    ON storico_eventi (avvenuto_il DESC, id DESC);

CREATE INDEX idx_storico_eventi_modulo_operazione
    ON storico_eventi (modulo, operazione, avvenuto_il DESC);

CREATE INDEX idx_storico_eventi_abitazione
    ON storico_eventi (abitazione_storico_id, avvenuto_il DESC);

CREATE INDEX idx_storico_eventi_stanza
    ON storico_eventi (stanza_storico_id, avvenuto_il DESC);

CREATE TABLE storico_cambiamenti (
    id INTEGER PRIMARY KEY,
    evento_id INTEGER NOT NULL
        REFERENCES storico_eventi(id) ON DELETE CASCADE,
    campo TEXT NOT NULL,
    tipo_valore TEXT NOT NULL,
    valore_prima TEXT,
    valore_dopo TEXT,
    ordine INTEGER NOT NULL DEFAULT 0,

    CHECK (valore_prima IS NOT NULL OR valore_dopo IS NOT NULL)
);

CREATE INDEX idx_storico_cambiamenti_evento
    ON storico_cambiamenti (evento_id, ordine, id);

CREATE TABLE storico_cambi_luogo (
    evento_id INTEGER PRIMARY KEY
        REFERENCES storico_eventi(id) ON DELETE CASCADE,

    abitazione_prima_id INTEGER
        REFERENCES storico_entita(id) ON DELETE SET NULL,
    abitazione_prima_nome TEXT,
    stanza_prima_id INTEGER
        REFERENCES storico_entita(id) ON DELETE SET NULL,
    stanza_prima_nome TEXT,

    abitazione_dopo_id INTEGER
        REFERENCES storico_entita(id) ON DELETE SET NULL,
    abitazione_dopo_nome TEXT,
    stanza_dopo_id INTEGER
        REFERENCES storico_entita(id) ON DELETE SET NULL,
    stanza_dopo_nome TEXT,

    CHECK (
        abitazione_prima_id IS NOT NULL
        OR abitazione_prima_nome IS NOT NULL
        OR stanza_prima_id IS NOT NULL
        OR stanza_prima_nome IS NOT NULL
        OR abitazione_dopo_id IS NOT NULL
        OR abitazione_dopo_nome IS NOT NULL
        OR stanza_dopo_id IS NOT NULL
        OR stanza_dopo_nome IS NOT NULL
    )
);

-- Backfill: rende tracciabili da ORA gli elementi gia' presenti.
-- Nessun INSERT viene fatto in storico_eventi, quindi non viene inventata
-- alcuna cronologia precedente all'introduzione dello Step 6B.
INSERT INTO storico_entita (tipo_entita, id_origine, nome_ultimo)
SELECT tipo, id, nome
FROM items;

INSERT INTO storico_entita (tipo_entita, id_origine, nome_ultimo)
SELECT 'abitazione', id, nome
FROM abitazioni;

INSERT INTO storico_entita (tipo_entita, id_origine, nome_ultimo)
SELECT 'stanza', id, nome
FROM stanze;
