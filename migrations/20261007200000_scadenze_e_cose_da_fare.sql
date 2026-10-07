-- 📅 Scadenze e 📝 cose da fare, dentro ⏰ Promemoria (7 ottobre 2026,
-- docs/moduli/promemoria.md).
--
-- Alessio: "dove c'e' una scadenza vorrei dare dei livelli di priorita':
-- piu' si avvicina alla scadenza e piu' questo dovra' essere avvisato piu'
-- volte. Un promemoria puo' anche non avere scadenze, magari la facciamo
-- rientrare in una to do list".

-- Una data entro cui fare qualcosa. I giorni degli avvisi li decide la
-- priorita', nel codice (`scadenze::Priorita`). Le scadenze dei Documenti
-- saranno righe di questa tabella, con un riferimento al documento.
CREATE TABLE scadenze (
    id INTEGER PRIMARY KEY,
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    spazio_id INTEGER REFERENCES spazi(id) ON DELETE SET NULL,
    testo TEXT NOT NULL,
    -- 'AAAA-MM-GG'.
    data TEXT NOT NULL,
    priorita TEXT NOT NULL DEFAULT 'media' CHECK (priorita IN ('bassa', 'media', 'alta')),
    stato TEXT NOT NULL DEFAULT 'aperta' CHECK (stato IN ('aperta', 'fatta')),
    -- Ora locale 'AAAA-MM-GG HH:MM'.
    fatta_il TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(testo)) > 0),
    CHECK (date(data) IS NOT NULL)
);

CREATE INDEX idx_scadenze_aperte ON scadenze (stato, data);
CREATE INDEX idx_scadenze_utente ON scadenze (utente_id, stato, data);

-- Le cose da fare senza data ne' priorita'.
CREATE TABLE cose_da_fare (
    id INTEGER PRIMARY KEY,
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    spazio_id INTEGER REFERENCES spazi(id) ON DELETE SET NULL,
    testo TEXT NOT NULL,
    -- Ora locale 'AAAA-MM-GG HH:MM', NULL finche' non e' fatta.
    fatta_il TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(testo)) > 0)
);

CREATE INDEX idx_cose_da_fare_utente ON cose_da_fare (utente_id, fatta_il, id);

-- L'ora a cui arrivano gli avvisi delle scadenze, per utente.
ALTER TABLE promemoria_regole
    ADD COLUMN ora_avvisi_scadenze TEXT NOT NULL DEFAULT '09:00'
        CHECK (length(ora_avvisi_scadenze) = 5);
