-- ⏰ Promemoria (7 ottobre 2026, docs/previsto/promemoria.md).
--
-- Il primo modulo in cui il bot scrive da solo, a un orario. Le ore sono
-- testo in ora LOCALE, 'AAAA-MM-GG HH:MM', confrontate con
-- strftime('%Y-%m-%d %H:%M','now','localtime'): il fuso lo conosce solo
-- SQLite (Miglioramento 17).

-- I promemoria scritti da chi li vuole, e quelli nati da un "⏰ Rimanda".
-- "_liberi" perche' `promemoria` c'e' gia': e' del primo schema (12 agosto
-- 2026), legata agli oggetti, mai usata e vuota; `db::status` la conta fra
-- le tabelle dello schema di base, quindi resta dov'e'.
CREATE TABLE promemoria_liberi (
    id INTEGER PRIMARY KEY,
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    spazio_id INTEGER REFERENCES spazi(id) ON DELETE SET NULL,
    testo TEXT NOT NULL,
    -- La prossima volta che deve arrivare.
    prossimo_il TEXT NOT NULL,
    ripetizione TEXT NOT NULL DEFAULT 'mai'
        CHECK (ripetizione IN ('mai', 'giorno', 'feriali', 'settimana', 'mese', 'anno')),
    -- Il giorno scelto all'inizio, per 'mese' e 'anno': "ogni mese il 31"
    -- cade il 30 ad aprile e torna il 31 a maggio.
    giorno_scelto INTEGER CHECK (giorno_scelto BETWEEN 1 AND 31),
    stato TEXT NOT NULL DEFAULT 'attivo'
        CHECK (stato IN ('attivo', 'sospeso', 'concluso')),
    origine TEXT NOT NULL DEFAULT 'libero'
        CHECK (origine IN ('libero', 'rimandato')),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(testo)) > 0),
    CHECK (length(prossimo_il) = 16)
);

CREATE INDEX idx_promemoria_da_mandare ON promemoria_liberi (stato, prossimo_il);
CREATE INDEX idx_promemoria_utente ON promemoria_liberi (utente_id, stato, prossimo_il);

-- Ogni avviso mandato, con una chiave unica per utente: lo stesso avviso
-- non parte mai due volte, nemmeno con due controlli sovrapposti.
--   promemoria:<id>:<prossimo_il>   un promemoria libero
--   pasto:<pasto_id>                un pasto del planner
--   scadenze:<AAAA-MM-GG>           il riepilogo delle scorte di quel giorno
-- Un riepilogo senza niente da dire si segna lo stesso (chat_id NULL), per
-- non ricontrollarlo ogni trenta secondi.
CREATE TABLE promemoria_invii (
    id INTEGER PRIMARY KEY,
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    chiave TEXT NOT NULL,
    testo TEXT NOT NULL DEFAULT '',
    chat_id INTEGER,
    message_id INTEGER,
    esito TEXT CHECK (esito IN ('fatto', 'rimandato')),
    inviato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M', 'now', 'localtime')),
    UNIQUE (utente_id, chiave)
);

-- Le regole automatiche, una riga per utente. Nascono spente (NULL): un
-- messaggio non chiesto e' un'automazione inattesa.
CREATE TABLE promemoria_regole (
    utente_id INTEGER PRIMARY KEY REFERENCES utenti(id) ON DELETE CASCADE,
    -- Quanti minuti prima di un pasto del planner; 0 = all'ora del pasto.
    pasti_minuti_prima INTEGER CHECK (pasti_minuti_prima BETWEEN 0 AND 1440),
    -- A che ora arriva il riepilogo delle scorte che scadono, 'HH:MM'.
    scadenze_ora TEXT CHECK (scadenze_ora IS NULL OR length(scadenze_ora) = 5),
    scadenze_giorni INTEGER NOT NULL DEFAULT 3 CHECK (scadenze_giorni BETWEEN 0 AND 30),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- L'eccezione di un pasto alla regola generale: un altro anticipo, oppure
-- NULL per "non ricordarmelo".
CREATE TABLE promemoria_pasti (
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    pasto_id INTEGER NOT NULL REFERENCES planner_pasti(id) ON DELETE CASCADE,
    minuti_prima INTEGER CHECK (minuti_prima BETWEEN 0 AND 1440),
    PRIMARY KEY (utente_id, pasto_id)
);
