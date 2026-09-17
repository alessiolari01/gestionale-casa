-- Consegna A del 17 settembre 2026, dal collaudo dal vivo di Alessio.
-- Dettagli in STATO.md (sezione 2undecies), docs/moduli/planner.md e
-- docs/moduli/lista-spesa.md.

-- ---------------------------------------------------------------------------
-- 1. Un pasto saltato o consumato si può riportare a pianificato.
-- ---------------------------------------------------------------------------
--
-- Fino a qui l'esito di un pasto era definitivo (Step 7.3B, 31 agosto 2026):
-- Alessio ha segnato saltata la colazione di oggi e non poteva piu' tornare
-- indietro. Il blocco resta su tutto il resto -- ricetta, quantita', giorno,
-- partecipanti non si cambiano per sbaglio -- ma si apre l'unico passaggio
-- richiesto: tornare a "pianificato".

DROP TRIGGER trg_planner_pasto_saltato_immutabile;

CREATE TRIGGER trg_planner_pasto_saltato_immutabile
BEFORE UPDATE ON planner_pasti
WHEN OLD.saltato_il IS NOT NULL
AND (
    NEW.planner_id <> OLD.planner_id
    OR NEW.data_pasto <> OLD.data_pasto
    OR NEW.tipo_pasto <> OLD.tipo_pasto
    OR NEW.ordinamento <> OLD.ordinamento
    OR NEW.ricetta_id IS NOT OLD.ricetta_id
    OR NEW.ricetta_nome_snapshot <> OLD.ricetta_nome_snapshot
    OR NEW.ricetta_porzione_base_snapshot <> OLD.ricetta_porzione_base_snapshot
    OR NEW.ricetta_aggiornato_il_snapshot IS NOT OLD.ricetta_aggiornato_il_snapshot
    OR NEW.stato <> OLD.stato
    OR NEW.completato_il IS NOT OLD.completato_il
    -- L'unico cambiamento ammesso su saltato_il e' toglierlo.
    OR (NEW.saltato_il IS NOT NULL AND NEW.saltato_il IS NOT OLD.saltato_il)
)
BEGIN
    SELECT RAISE(ABORT, 'pasto saltato non modificabile');
END;

DROP TRIGGER trg_planner_pasto_completato_immutabile;

-- Rispetto alla versione del 31 agosto manca solo la riga
-- `NEW.stato <> OLD.stato`: gli stati possibili sono due, quindi da
-- "completato" l'unico cambio di stato e' tornare a "pianificato" (il CHECK
-- della tabella impone di togliere anche completato_il).
CREATE TRIGGER trg_planner_pasto_completato_immutabile
BEFORE UPDATE ON planner_pasti
WHEN OLD.stato = 'completato'
AND (
    NEW.planner_id <> OLD.planner_id
    OR NEW.data_pasto <> OLD.data_pasto
    OR NEW.tipo_pasto <> OLD.tipo_pasto
    OR NEW.ordinamento <> OLD.ordinamento
    OR NEW.ricetta_id IS NOT OLD.ricetta_id
    OR NEW.ricetta_nome_snapshot <> OLD.ricetta_nome_snapshot
    OR NEW.ricetta_porzione_base_snapshot <> OLD.ricetta_porzione_base_snapshot
    OR NEW.ricetta_aggiornato_il_snapshot IS NOT OLD.ricetta_aggiornato_il_snapshot
)
BEGIN
    SELECT RAISE(ABORT, 'pasto completato non modificabile');
END;

-- ---------------------------------------------------------------------------
-- 2. Cosa mancava in casa quando un pasto ha preso le sue scorte.
-- ---------------------------------------------------------------------------
--
-- Testo gia' pronto da mostrare ("Pasta brisée 200 g, Sovracosce 125 g"),
-- NULL se c'era tutto. Serve soprattutto per lo scarico automatico a orario
-- passato, dove nessuno puo' confermare l'eccezione sul momento.
ALTER TABLE planner_pasti ADD COLUMN scorte_mancanti TEXT;

-- ---------------------------------------------------------------------------
-- 3. Lista della spesa: cosa si e' preso davvero.
-- ---------------------------------------------------------------------------
--
-- "Mi servono 250 g ma la confezione e' da 300 g": la voce ricorda la
-- quantita' presa e, se scelta, la confezione. Chiudendo la spesa entra in
-- casa quella, non la quantita' che serviva. Le colonne non sono nel
-- trigger di congelamento delle voci comprate, di proposito: si segnano
-- proprio dopo aver spuntato.
ALTER TABLE liste_spesa_voci
    ADD COLUMN quantita_presa REAL CHECK (quantita_presa IS NULL OR quantita_presa > 0);
ALTER TABLE liste_spesa_voci ADD COLUMN unita_presa TEXT;
ALTER TABLE liste_spesa_voci
    ADD COLUMN prodotto_preso_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE SET NULL;

-- Nell'archivio, quanto serviva quando si e' preso altro: la storia di
-- "servivano 250 g, ne ho presi 300".
ALTER TABLE liste_spesa_voci_archiviate
    ADD COLUMN quantita_richiesta REAL
        CHECK (quantita_richiesta IS NULL OR quantita_richiesta > 0);
