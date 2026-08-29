-- Step 7.2H.4 - Inviti privati agli spazi e gestione del ciclo di vita.
--
-- La tabella inviti_spazio esiste dalle fondazioni Step 7.1 ma fino a H.3
-- non era esposta all'utente. H.4 aggiunge i dati necessari per ricostruire
-- il deep-link dall'elenco degli inviti attivi e per distinguere chiaramente
-- la modalità di utilizzo scelta dall'utente.
--
-- token_link è un bearer token casuale: non viene mai scritto nello Storico.
-- Gli inviti non più utilizzabili vengono cancellati dal runtime invece di
-- essere conservati come elenco storico.

ALTER TABLE inviti_spazio ADD COLUMN token_link TEXT;
ALTER TABLE inviti_spazio ADD COLUMN tipo_invito TEXT NOT NULL DEFAULT 'legacy'
    CHECK (tipo_invito IN ('legacy', 'monouso', 'riutilizzabile', 'limite', 'scadenza'));
ALTER TABLE inviti_spazio ADD COLUMN aggiornato_il TEXT;

CREATE UNIQUE INDEX idx_inviti_spazio_token_link
    ON inviti_spazio (token_link)
    WHERE token_link IS NOT NULL;

CREATE INDEX idx_inviti_spazio_runtime
    ON inviti_spazio (spazio_id, tipo_invito, scade_il, utilizzi, utilizzi_massimi)
    WHERE token_link IS NOT NULL;
