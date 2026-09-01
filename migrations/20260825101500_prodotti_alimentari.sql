-- Step 7.2D.0.2 - prodotti commerciali collegati agli alimenti.
--
-- Un alimento resta il concetto generico usato dalle Ricette (es. "Formaggio
-- spalmabile"). I prodotti commerciali rappresentano invece una referenza
-- acquistabile reale (es. marca Philadelphia, prodotto Original, confezione
-- 200 g). Il futuro storico prezzi dovrà referenziare prodotti_alimentari.id,
-- non duplicare il nome dell'alimento o del prodotto.

CREATE TABLE prodotti_alimentari (
    id INTEGER PRIMARY KEY,
    alimento_id INTEGER NOT NULL REFERENCES alimenti(id) ON DELETE RESTRICT,
    marca TEXT NOT NULL,
    marca_normalizzata TEXT NOT NULL,
    nome_commerciale TEXT NOT NULL,
    nome_commerciale_normalizzato TEXT NOT NULL,
    quantita_confezione REAL NOT NULL CHECK (quantita_confezione > 0),
    unita_confezione_id INTEGER NOT NULL REFERENCES unita_misura(id) ON DELETE RESTRICT,
    codice_ean TEXT,
    creato_da_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    verificato INTEGER NOT NULL DEFAULT 0 CHECK (verificato IN (0, 1)),
    attivo INTEGER NOT NULL DEFAULT 1 CHECK (attivo IN (0, 1)),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(marca)) > 0),
    CHECK (length(trim(marca_normalizzata)) > 0),
    CHECK (length(trim(nome_commerciale)) > 0),
    CHECK (length(trim(nome_commerciale_normalizzato)) > 0),
    CHECK (codice_ean IS NULL OR length(trim(codice_ean)) > 0)
);

CREATE INDEX idx_prodotti_alimentari_alimento
    ON prodotti_alimentari (alimento_id, attivo, marca_normalizzata, nome_commerciale_normalizzato, id);

CREATE INDEX idx_prodotti_alimentari_marca_nome
    ON prodotti_alimentari (marca_normalizzata, nome_commerciale_normalizzato, attivo, id);

CREATE UNIQUE INDEX idx_prodotti_alimentari_ean
    ON prodotti_alimentari (codice_ean)
    WHERE codice_ean IS NOT NULL AND attivo = 1;

-- Evita duplicati esatti della stessa confezione sotto lo stesso alimento.
CREATE UNIQUE INDEX idx_prodotti_alimentari_associazione_unica
    ON prodotti_alimentari (
        alimento_id,
        marca_normalizzata,
        nome_commerciale_normalizzato,
        quantita_confezione,
        unita_confezione_id
    )
    WHERE attivo = 1;

-- Non creiamo ancora negozi o rilevazioni_prezzo: il futuro modulo prezzi
-- potrà aggiungere una tabella append-only con FK a prodotti_alimentari.id e
-- al punto vendita, preservando lo storico del costo nel tempo.
