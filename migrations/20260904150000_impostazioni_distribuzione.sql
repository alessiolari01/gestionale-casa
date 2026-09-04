-- Impostazioni della schermata admin 🚀 Distribuzione (sotto-step 3/5 del
-- punto 6 del ciclo di automazione, docs/previsto/automazione-ciclo-sviluppo.md).
--
-- Riga singola (id fisso a 1, come preferenze_utente ha una riga per utente):
-- qui il "soggetto" e' il gestionale intero, non c'e' bisogno di piu' righe.
--
-- Due gruppi di colonne:
-- 1. il default (tipo_default + il parametro che gli serve), che l'admin
--    configura da questa schermata;
-- 2. la scelta puntuale per il prossimo deploy (scelta_puntuale_*), che
--    l'agente orchestratore leggera' via SSH prima dello swap (sotto-step 5)
--    e azzerera' una volta applicata. Nessuna UI la scrive ancora in questo
--    sotto-step: le colonne esistono gia' per non dover fare una seconda
--    migration quando arrivera' quella parte.
--
-- I CHECK garantiscono che tipo e parametro restino coerenti: un 'subito'
-- non porta ne' minuti ne' orario, un 'countdown' richiede i minuti, un
-- 'programmato' richiede l'orario. La validazione del formato orario
-- (HH:MM, ore 00-23) resta in Rust: un CHECK con GLOB non basterebbe a
-- escludere valori come '29:00'.

CREATE TABLE impostazioni_distribuzione (
    id INTEGER PRIMARY KEY CHECK (id = 1),

    tipo_default TEXT NOT NULL
        CHECK (tipo_default IN ('subito', 'countdown', 'programmato')),
    minuti_countdown_default INTEGER
        CHECK (minuti_countdown_default IS NULL OR minuti_countdown_default > 0),
    orario_programmato_default TEXT,

    scelta_puntuale_tipo TEXT
        CHECK (scelta_puntuale_tipo IS NULL
            OR scelta_puntuale_tipo IN ('subito', 'countdown', 'programmato')),
    scelta_puntuale_minuti INTEGER
        CHECK (scelta_puntuale_minuti IS NULL OR scelta_puntuale_minuti > 0),
    scelta_puntuale_orario TEXT,

    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (
        (tipo_default = 'subito'
            AND minuti_countdown_default IS NULL
            AND orario_programmato_default IS NULL)
        OR (tipo_default = 'countdown'
            AND minuti_countdown_default IS NOT NULL
            AND orario_programmato_default IS NULL)
        OR (tipo_default = 'programmato'
            AND minuti_countdown_default IS NULL
            AND orario_programmato_default IS NOT NULL)
    ),
    CHECK (
        scelta_puntuale_tipo IS NULL
        OR (scelta_puntuale_tipo = 'subito'
            AND scelta_puntuale_minuti IS NULL
            AND scelta_puntuale_orario IS NULL)
        OR (scelta_puntuale_tipo = 'countdown'
            AND scelta_puntuale_minuti IS NOT NULL
            AND scelta_puntuale_orario IS NULL)
        OR (scelta_puntuale_tipo = 'programmato'
            AND scelta_puntuale_minuti IS NULL
            AND scelta_puntuale_orario IS NOT NULL)
    )
);

-- Default operativo scelto nella specifica: countdown standard di 5 minuti.
INSERT INTO impostazioni_distribuzione (id, tipo_default, minuti_countdown_default)
VALUES (1, 'countdown', 5);
