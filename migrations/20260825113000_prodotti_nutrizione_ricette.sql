-- Step 7.2D.0.3 - valori nutrizionali dei prodotti e scelta prodotto nelle ricette.
--
-- Obiettivi:
-- - mantenere facoltativi i valori nutrizionali dei prodotti commerciali;
-- - normalizzarli per 100 g oppure 100 ml;
-- - predisporre la provenienza dei dati per future importazioni/barcode;
-- - permettere a un ingrediente ricetta di riferirsi opzionalmente a un
--   prodotto commerciale senza perdere il riferimento all'alimento generico.

CREATE TABLE valori_nutrizionali_prodotto (
    prodotto_alimentare_id INTEGER PRIMARY KEY
        REFERENCES prodotti_alimentari(id) ON DELETE CASCADE,
    riferimento_quantita REAL NOT NULL DEFAULT 100
        CHECK (riferimento_quantita = 100),
    riferimento_unita_id INTEGER NOT NULL
        REFERENCES unita_misura(id) ON DELETE RESTRICT,
    energia_kcal REAL CHECK (energia_kcal IS NULL OR energia_kcal >= 0),
    energia_kj REAL CHECK (energia_kj IS NULL OR energia_kj >= 0),
    grassi_g REAL CHECK (grassi_g IS NULL OR grassi_g >= 0),
    saturi_g REAL CHECK (saturi_g IS NULL OR saturi_g >= 0),
    carboidrati_g REAL CHECK (carboidrati_g IS NULL OR carboidrati_g >= 0),
    zuccheri_g REAL CHECK (zuccheri_g IS NULL OR zuccheri_g >= 0),
    fibre_g REAL CHECK (fibre_g IS NULL OR fibre_g >= 0),
    proteine_g REAL CHECK (proteine_g IS NULL OR proteine_g >= 0),
    sale_g REAL CHECK (sale_g IS NULL OR sale_g >= 0),
    fonte_tipo TEXT NOT NULL DEFAULT 'manuale'
        CHECK (fonte_tipo IN ('manuale', 'etichetta', 'importazione')),
    fonte_note TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (
        energia_kcal IS NOT NULL OR energia_kj IS NOT NULL OR
        grassi_g IS NOT NULL OR saturi_g IS NOT NULL OR
        carboidrati_g IS NOT NULL OR zuccheri_g IS NOT NULL OR
        fibre_g IS NOT NULL OR proteine_g IS NOT NULL OR sale_g IS NOT NULL
    )
);

CREATE TRIGGER trg_valori_nutrizionali_unita_insert
BEFORE INSERT ON valori_nutrizionali_prodotto
WHEN NOT EXISTS (
    SELECT 1
    FROM unita_misura um
    WHERE um.id = NEW.riferimento_unita_id
      AND um.attiva = 1
      AND um.simbolo IN ('g', 'ml')
)
BEGIN
    SELECT RAISE(ABORT, 'unita nutrizionale non valida');
END;

CREATE TRIGGER trg_valori_nutrizionali_unita_update
BEFORE UPDATE OF riferimento_unita_id ON valori_nutrizionali_prodotto
WHEN NOT EXISTS (
    SELECT 1
    FROM unita_misura um
    WHERE um.id = NEW.riferimento_unita_id
      AND um.attiva = 1
      AND um.simbolo IN ('g', 'ml')
)
BEGIN
    SELECT RAISE(ABORT, 'unita nutrizionale non valida');
END;

ALTER TABLE ricetta_ingredienti
ADD COLUMN prodotto_alimentare_id INTEGER
    REFERENCES prodotti_alimentari(id) ON DELETE SET NULL;

CREATE INDEX idx_ricetta_ingredienti_prodotto
    ON ricetta_ingredienti (prodotto_alimentare_id, ricetta_id);

-- L'alimento generico rimane sempre il riferimento principale. Se viene
-- scelto un prodotto specifico, questo deve appartenere allo stesso alimento.
CREATE TRIGGER trg_ricetta_ingrediente_prodotto_coerente_insert
BEFORE INSERT ON ricetta_ingredienti
WHEN NEW.prodotto_alimentare_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM prodotti_alimentari p
    WHERE p.id = NEW.prodotto_alimentare_id
      AND p.alimento_id = NEW.alimento_id
      AND p.attivo = 1
)
BEGIN
    SELECT RAISE(ABORT, 'prodotto non associato all alimento della ricetta');
END;

CREATE TRIGGER trg_ricetta_ingrediente_prodotto_coerente_update
BEFORE UPDATE OF alimento_id, prodotto_alimentare_id ON ricetta_ingredienti
WHEN NEW.prodotto_alimentare_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM prodotti_alimentari p
    WHERE p.id = NEW.prodotto_alimentare_id
      AND p.alimento_id = NEW.alimento_id
      AND p.attivo = 1
)
BEGIN
    SELECT RAISE(ABORT, 'prodotto non associato all alimento della ricetta');
END;
