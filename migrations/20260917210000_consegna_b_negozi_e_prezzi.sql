-- Consegna B del 17 settembre 2026, concordata con Alessio dopo la consegna A.
-- Dettagli in STATO.md (sezione 2duodecies), docs/moduli/mercato.md e
-- docs/moduli/lista-spesa.md.
--
-- Cosa aggiunge: i negozi (le catene fra cui confrontare), i prezzi visti
-- davvero durante la spesa, il totale dello scontrino, i prodotti preferiti.
-- Tutto facoltativo: chi non registra niente vede la lista di prima.

-- ---------------------------------------------------------------------------
-- 1. Negozi.
-- ---------------------------------------------------------------------------
--
-- `spazio_id NULL` = catena del catalogo comune, uguale per tutti (come gli
-- alimenti globali). Un negozio creato da un utente appartiene al suo spazio.
CREATE TABLE negozi (
    id INTEGER PRIMARY KEY,
    spazio_id INTEGER REFERENCES spazi(id) ON DELETE CASCADE,
    nome TEXT NOT NULL,
    nome_normalizzato TEXT NOT NULL,
    attivo INTEGER NOT NULL DEFAULT 1 CHECK (attivo IN (0, 1)),
    creato_da_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0),
    CHECK (length(trim(nome_normalizzato)) > 0)
);

CREATE UNIQUE INDEX idx_negozi_globali_nome
    ON negozi (nome_normalizzato) WHERE spazio_id IS NULL;
CREATE UNIQUE INDEX idx_negozi_spazio_nome
    ON negozi (spazio_id, nome_normalizzato) WHERE spazio_id IS NOT NULL;

-- Le catene diffuse in Italia. Alessio ne ha nominate sette; le altre sono
-- quelle piu' comuni, cosi' chi apre la lista trova gia' il suo supermercato
-- invece di doverlo scrivere.
INSERT INTO negozi (nome, nome_normalizzato) VALUES
    ('Conad', 'conad'),
    ('Coop', 'coop'),
    ('Famila', 'famila'),
    ('Esselunga', 'esselunga'),
    ('Lidl', 'lidl'),
    ('Penny', 'penny'),
    ('Eurospin', 'eurospin'),
    ('Carrefour', 'carrefour'),
    ('Pam Panorama', 'pam panorama'),
    ('Despar', 'despar'),
    ('Crai', 'crai'),
    ('Sigma', 'sigma'),
    ('Bennet', 'bennet'),
    ('Iper La grande i', 'iper la grande i'),
    ('Il Gigante', 'il gigante'),
    ('MD', 'md'),
    ('Aldi', 'aldi'),
    ('Todis', 'todis'),
    ('In''s Mercato', 'in''s mercato'),
    ('Tigros', 'tigros'),
    ('Unes', 'unes'),
    ('Basko', 'basko'),
    ('Deco', 'deco'),
    ('Sisa', 'sisa'),
    ('NaturaSi', 'naturasi');

-- Su quali negozi l'utente vuole il confronto (scelta sua, chiesta
-- esplicitamente: confrontare venticinque catene non serve a nessuno).
CREATE TABLE negozi_confronto (
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    negozio_id INTEGER NOT NULL REFERENCES negozi(id) ON DELETE CASCADE,
    scelto_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (utente_id, negozio_id)
);

-- ---------------------------------------------------------------------------
-- 2. Prezzi visti.
-- ---------------------------------------------------------------------------
--
-- Il prezzo non appartiene al prodotto ma al prodotto *in un negozio, in un
-- giorno* (gia' scritto in docs/previsto/acquisti.md). In centesimi, mai
-- REAL: i soldi non si arrotondano per sbaglio.
--
-- `quantita`/`unita_simbolo` sono quelle della confezione pagata, cosi' si
-- puo' confrontare il prezzo al chilo fra confezioni diverse.
CREATE TABLE prezzi_osservati (
    id INTEGER PRIMARY KEY,
    negozio_id INTEGER NOT NULL REFERENCES negozi(id) ON DELETE CASCADE,
    alimento_id INTEGER REFERENCES alimenti(id) ON DELETE CASCADE,
    prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE CASCADE,
    descrizione TEXT NOT NULL,
    prezzo_centesimi INTEGER NOT NULL CHECK (prezzo_centesimi > 0),
    quantita REAL CHECK (quantita IS NULL OR quantita > 0),
    unita_simbolo TEXT,
    -- 'spesa' = registrato da me durante la spesa; 'open_prices' = suggerito
    -- da Open Prices, che e' un dato di altri e va detto.
    fonte TEXT NOT NULL DEFAULT 'spesa' CHECK (fonte IN ('spesa', 'open_prices')),
    utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    rilevato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(descrizione)) > 0),
    CHECK (alimento_id IS NOT NULL OR prodotto_alimentare_id IS NOT NULL)
);

CREATE INDEX idx_prezzi_osservati_alimento
    ON prezzi_osservati (alimento_id, negozio_id, rilevato_il);
CREATE INDEX idx_prezzi_osservati_prodotto
    ON prezzi_osservati (prodotto_alimentare_id, negozio_id, rilevato_il);

-- ---------------------------------------------------------------------------
-- 3. La spesa: negozio, prezzo per voce, totale dello scontrino.
-- ---------------------------------------------------------------------------
--
-- Tutto facoltativo (opzione "c" scelta da Alessio): si puo' registrare il
-- prezzo voce per voce, solo il totale, tutti e due o niente.
ALTER TABLE liste_spesa ADD COLUMN negozio_id INTEGER
    REFERENCES negozi(id) ON DELETE SET NULL;
ALTER TABLE liste_spesa_voci ADD COLUMN prezzo_centesimi INTEGER
    CHECK (prezzo_centesimi IS NULL OR prezzo_centesimi > 0);
ALTER TABLE liste_spesa_voci_archiviate ADD COLUMN prezzo_centesimi INTEGER
    CHECK (prezzo_centesimi IS NULL OR prezzo_centesimi > 0);
ALTER TABLE liste_spesa_chiusure ADD COLUMN negozio_id INTEGER
    REFERENCES negozi(id) ON DELETE SET NULL;
-- Il totale dello scontrino: diventera' una transazione del modulo Soldi
-- (approvato da Alessio), per questo si conserva anche quando le voci hanno
-- gia' i loro prezzi.
ALTER TABLE liste_spesa_chiusure ADD COLUMN totale_centesimi INTEGER
    CHECK (totale_centesimi IS NULL OR totale_centesimi > 0);

-- ---------------------------------------------------------------------------
-- 4. Prodotti preferiti.
-- ---------------------------------------------------------------------------
--
-- "Di questo alimento compro sempre quello": il preferito si propone per
-- primo. Per utente, non globale: il catalogo dei prodotti e' condiviso.
CREATE TABLE prodotti_preferiti (
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    alimento_id INTEGER NOT NULL REFERENCES alimenti(id) ON DELETE CASCADE,
    prodotto_alimentare_id INTEGER NOT NULL
        REFERENCES prodotti_alimentari(id) ON DELETE CASCADE,
    scelto_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (utente_id, alimento_id)
);

CREATE INDEX idx_prodotti_preferiti_prodotto
    ON prodotti_preferiti (prodotto_alimentare_id);
