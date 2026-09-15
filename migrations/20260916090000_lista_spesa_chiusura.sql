-- Chiusura della spesa e intervallo che parte da oggi: chiesto da Alessio il
-- 16 settembre 2026, prima del collaudo delle correzioni del 10 settembre.
--
-- Mancava il momento "spesa fatta": la lista e' una sola e le voci comprate
-- restavano li' per sempre, quindi la lista non tornava mai pulita per il
-- giro successivo. Chiudere la spesa archivia cio' che e' stato comprato
-- (non lo cancella: l'archivio e' anche la base su cui si appoggera' la
-- futura dispensa, vedi docs/previsto/dispensa.md) e lo toglie dalla lista
-- attiva.

-- La lista si sposta da sola in avanti per non restare indietro rispetto a
-- oggi. L'utente puo' pero' scegliere deliberatamente un inizio nel passato
-- (es. per recuperare i pasti di ieri): in quel caso la scelta e' sua e va
-- rispettata, quindi lo spostamento automatico si disattiva finche' non
-- sceglie di nuovo un inizio da oggi in avanti.
ALTER TABLE liste_spesa
    ADD COLUMN inizio_manuale INTEGER NOT NULL DEFAULT 0
        CHECK (inizio_manuale IN (0, 1));

-- Una spesa chiusa: lo scontrino di un giro di spesa.
CREATE TABLE liste_spesa_chiusure (
    id INTEGER PRIMARY KEY,
    lista_id INTEGER NOT NULL
        REFERENCES liste_spesa(id) ON DELETE CASCADE,
    -- ON DELETE SET NULL, non RESTRICT: l'archivio di una spesa gia' fatta
    -- non deve impedire di eliminare un utente in futuro.
    chiusa_da_utente_id INTEGER
        REFERENCES utenti(id) ON DELETE SET NULL,
    -- Intervallo della lista al momento della chiusura: serve a rileggere
    -- l'archivio senza dipendere dall'intervallo attuale, che intanto si e'
    -- gia' spostato in avanti.
    data_inizio TEXT NOT NULL,
    data_fine TEXT NOT NULL,
    voci_totali INTEGER NOT NULL DEFAULT 0 CHECK (voci_totali >= 0),
    chiusa_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (date(data_inizio) IS NOT NULL),
    CHECK (date(data_fine) IS NOT NULL)
);

CREATE INDEX idx_lista_spesa_chiusure_lista
    ON liste_spesa_chiusure (lista_id, id);

-- Le voci comprate spostate qui dalla chiusura. Stesse colonne di
-- liste_spesa_voci, meno quelle che qui non hanno piu' senso (comprato, che
-- e' sempre vero per costruzione, e ordinamento, che riguarda solo la lista
-- attiva).
CREATE TABLE liste_spesa_voci_archiviate (
    id INTEGER PRIMARY KEY,
    chiusura_id INTEGER NOT NULL
        REFERENCES liste_spesa_chiusure(id) ON DELETE CASCADE,
    origine TEXT NOT NULL CHECK (origine IN ('generato', 'manuale')),
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE SET NULL,
    prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE SET NULL,
    descrizione TEXT NOT NULL,
    quantita REAL CHECK (quantita IS NULL OR quantita > 0),
    unita_simbolo TEXT,
    comprato_il TEXT,
    archiviata_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(descrizione)) > 0)
);

CREATE INDEX idx_lista_spesa_voci_archiviate_chiusura
    ON liste_spesa_voci_archiviate (chiusura_id, id);

-- Servira' alla futura dispensa per sapere cosa e' entrato in casa e
-- quando, senza dover rileggere tutta la tabella.
CREATE INDEX idx_lista_spesa_voci_archiviate_alimento
    ON liste_spesa_voci_archiviate (alimento_id, chiusura_id);

-- Una voce archiviata e' storia: non si modifica piu', stesso principio del
-- congelamento di una voce comprata
-- (trg_lista_spesa_voce_comprata_immutabile) e di un pasto completato del
-- planner. Qui il blocco e' totale, non solo su alcune colonne: dopo la
-- chiusura non esiste nessuna modifica legittima.
CREATE TRIGGER trg_lista_spesa_voce_archiviata_immutabile
BEFORE UPDATE ON liste_spesa_voci_archiviate
BEGIN
    SELECT RAISE(ABORT, 'voce archiviata non modificabile');
END;
