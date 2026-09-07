-- Meccanismo del badge "🆕" (docs/roadmap.md / decisione con Alessio del
-- 5 settembre 2026): da questo punto in avanti, ogni funzionalità nuova o
-- modificata in modo significativo si dichiara in src/modules/novita.rs
-- (chiave, genitore nel menù, tutorial opzionale) -- un registro statico
-- nel codice, non nel database. Qui serve tracciare solo una cosa: quali
-- chiavi ciascun utente ha già visto, per calcolare dove mostrare il
-- badge e farlo sparire -- per persona, non globalmente: più persone
-- usano lo stesso bot e ciascuna deve poter scoprire le novità ai propri
-- tempi.
CREATE TABLE novita_lette (
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    chiave TEXT NOT NULL CHECK (length(trim(chiave)) > 0),
    vista_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (utente_id, chiave)
);
