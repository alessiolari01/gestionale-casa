-- Step 7.2F.0 - separazione tra prodotto commerciale e formati di vendita.
--
-- Un prodotto commerciale descrive l'identità stabile del prodotto
-- (es. Philadelphia · Original). I formati descrivono invece le confezioni
-- acquistabili reali (es. 175 g, 200 g, 350 g) e diventano il punto corretto
-- per barcode/EAN, disponibilità e futuri prezzi per punto vendita.
--
-- Le colonne quantita_confezione, unita_confezione_id e codice_ean presenti
-- in prodotti_alimentari restano temporaneamente per compatibilità con le
-- migration già applicate, ma da questo step non sono più la fonte
-- autorevole per i formati. Ogni valore esistente viene migrato nella nuova
-- tabella e il codice applicativo usa formati_prodotto_alimentare.

CREATE TABLE formati_prodotto_alimentare (
    id INTEGER PRIMARY KEY,
    prodotto_alimentare_id INTEGER NOT NULL
        REFERENCES prodotti_alimentari(id) ON DELETE CASCADE,
    quantita_confezione REAL NOT NULL CHECK (quantita_confezione > 0),
    unita_confezione_id INTEGER NOT NULL
        REFERENCES unita_misura(id) ON DELETE RESTRICT,
    codice_ean TEXT,
    attivo INTEGER NOT NULL DEFAULT 1 CHECK (attivo IN (0, 1)),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (codice_ean IS NULL OR length(trim(codice_ean)) > 0)
);

CREATE INDEX idx_formati_prodotto_alimentare_prodotto
    ON formati_prodotto_alimentare (
        prodotto_alimentare_id,
        attivo,
        quantita_confezione,
        unita_confezione_id,
        id
    );

CREATE UNIQUE INDEX idx_formati_prodotto_alimentare_unico
    ON formati_prodotto_alimentare (
        prodotto_alimentare_id,
        quantita_confezione,
        unita_confezione_id
    )
    WHERE attivo = 1;

CREATE UNIQUE INDEX idx_formati_prodotto_alimentare_ean
    ON formati_prodotto_alimentare (codice_ean)
    WHERE codice_ean IS NOT NULL AND attivo = 1;

-- Ogni prodotto esistente diventa un prodotto con un primo formato.
INSERT INTO formati_prodotto_alimentare (
    prodotto_alimentare_id,
    quantita_confezione,
    unita_confezione_id,
    codice_ean,
    attivo,
    creato_il,
    aggiornato_il
)
SELECT
    id,
    quantita_confezione,
    unita_confezione_id,
    codice_ean,
    attivo,
    creato_il,
    aggiornato_il
FROM prodotti_alimentari;

-- Da questo momento il barcode appartiene al formato e non al prodotto.
-- Lasciamo la vecchia colonna a NULL per compatibilità con lo schema storico.
DROP INDEX IF EXISTS idx_prodotti_alimentari_ean;
UPDATE prodotti_alimentari SET codice_ean = NULL WHERE codice_ean IS NOT NULL;

-- Vista pronta per Ricette/Lista spesa/prezzi: una riga per formato attivo.
CREATE VIEW v_prodotti_formati_attivi AS
SELECT
    p.id AS prodotto_alimentare_id,
    p.alimento_id,
    p.marca,
    p.nome_commerciale,
    f.id AS formato_id,
    f.quantita_confezione,
    f.unita_confezione_id,
    um.simbolo AS unita_simbolo,
    f.codice_ean
FROM prodotti_alimentari p
JOIN formati_prodotto_alimentare f
  ON f.prodotto_alimentare_id = p.id
 AND f.attivo = 1
JOIN unita_misura um ON um.id = f.unita_confezione_id
WHERE p.attivo = 1;
