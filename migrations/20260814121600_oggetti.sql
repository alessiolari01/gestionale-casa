-- Step 5A: dati specifici del modulo Oggetti generici.
--
-- Ogni oggetto e' prima di tutto una riga in `items` con tipo = 'oggetto'.
-- Questa tabella contiene solo i dettagli specifici e usa lo stesso ID della
-- riga core. La cancellazione futura dell'item rimuovera' automaticamente i
-- dettagli grazie a ON DELETE CASCADE.
CREATE TABLE oggetti (
    item_id                  INTEGER PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
    descrizione              TEXT,
    marca                    TEXT,
    modello                  TEXT,
    numero_serie             TEXT,
    posizione                TEXT,
    data_acquisto            TEXT,
    prezzo_acquisto_centesimi INTEGER CHECK (prezzo_acquisto_centesimi IS NULL OR prezzo_acquisto_centesimi >= 0),
    venditore                TEXT,
    valore_stimato_centesimi INTEGER CHECK (valore_stimato_centesimi IS NULL OR valore_stimato_centesimi >= 0),
    condizione               TEXT CHECK (condizione IS NULL OR condizione IN ('ottimo', 'buono', 'usurato', 'da_riparare')),
    note                     TEXT
);

CREATE INDEX idx_oggetti_marca ON oggetti(marca);
CREATE INDEX idx_oggetti_modello ON oggetti(modello);
CREATE INDEX idx_oggetti_numero_serie ON oggetti(numero_serie);
CREATE INDEX idx_oggetti_posizione ON oggetti(posizione);

CREATE INDEX idx_items_tipo_nome ON items(tipo, nome COLLATE NOCASE);
