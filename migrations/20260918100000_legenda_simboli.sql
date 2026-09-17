-- Legenda dei simboli, chiesta da Alessio nel collaudo del 18 settembre 2026:
-- "metti una legenda dove pensi sia utile, e falla decidere all'utente se
-- vederla o no -- all'inizio serve, poi insomma".
--
-- Accesa di default (serve proprio a chi comincia), per utente, come
-- dispensa_ingresso_automatico e lista_spesa_aggiornamento_automatico.
-- Comportamento in docs/convenzioni-telegram.md (C4) e nei documenti dei
-- moduli planner, lista-spesa e dispensa.
ALTER TABLE preferenze_utente
    ADD COLUMN mostra_legenda INTEGER NOT NULL DEFAULT 1
        CHECK (mostra_legenda IN (0, 1));
