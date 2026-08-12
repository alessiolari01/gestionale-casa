-- Schema dati core: tabelle condivise da tutti i moduli (oggetti, vestiti,
-- veicoli, ricette). Spiegazione completa in docs/schema-core.md.
--
-- NOTA: SQLite non applica i vincoli di chiave esterna di default. Vanno
-- attivati per ogni connessione con "PRAGMA foreign_keys = ON;" nel codice
-- Rust (src/db.rs) — impostarlo qui nella migrazione non basta, va fatto
-- ad ogni apertura di connessione.

CREATE TABLE items (
    id            INTEGER PRIMARY KEY,
    tipo          TEXT NOT NULL CHECK (tipo IN ('oggetto', 'vestito', 'veicolo', 'ricetta')),
    nome          TEXT NOT NULL,
    creato_il     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE foto (
    id            INTEGER PRIMARY KEY,
    item_id       INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    percorso_file TEXT NOT NULL,
    ruolo         TEXT,
    descrizione   TEXT,
    caricato_il   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_foto_item_id ON foto(item_id);

CREATE TABLE tag (
    id   INTEGER PRIMARY KEY,
    nome TEXT NOT NULL UNIQUE
);

CREATE TABLE item_tag (
    item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, tag_id)
);

CREATE INDEX idx_item_tag_tag_id ON item_tag(tag_id);

CREATE TABLE promemoria (
    id                INTEGER PRIMARY KEY,
    item_id           INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    titolo            TEXT NOT NULL,
    descrizione       TEXT,
    scadenza          TEXT NOT NULL,
    ricorrenza_giorni INTEGER,
    notificato_il     TEXT,
    completato_il     TEXT,
    creato_il         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Indice parziale: le query dello scheduler cercano quasi sempre solo i
-- promemoria non ancora completati, quindi indicizziamo solo quelli.
CREATE INDEX idx_promemoria_scadenza ON promemoria(scadenza) WHERE completato_il IS NULL;
