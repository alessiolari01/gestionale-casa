-- Versione del contenuto della ricetta e partecipanti storici del planner.
--
-- Due correzioni indipendenti dalla UI, emerse rileggendo il planner:
--
-- 1. `ricette.aggiornato_il` viene scritto solo da rinomina, cambio
--    `porzioni_base` e archiviazione. Modificare un ingrediente non lo tocca e
--    nessun trigger lo faceva (verificato su tutte le migration e su
--    `ricette.rs`). Il planner confronta proprio quel campo per decidere se
--    mostrare l'avviso di ricetta cambiata, quindi l'avviso non sarebbe mai
--    comparso nel caso piu' importante: quello in cui cambiano le quantita'.
--
-- 2. `planner_pasto_profili` ha PRIMARY KEY (pasto_id, profilo_alimentare_id)
--    con la colonna profilo ON DELETE SET NULL. SQLite ammette NULL nelle
--    primary key composite, quindi due profili eliminati produrrebbero due
--    righe (pasto, NULL) non piu' distinguibili fra loro.
--
-- Migration append-only: non modifica nulla di gia' applicato e non popola
-- dati retroattivi.

-- ---------------------------------------------------------------------------
-- 1. Versione del contenuto della ricetta
-- ---------------------------------------------------------------------------
-- I trigger scrivono soltanto la colonna `aggiornato_il`, quindi non fanno
-- scattare `trg_ricetta_nome_unico_spazi_update`, che e' un
-- BEFORE UPDATE OF nome_normalizzato.
--
-- Il procedimento (`ricetta_step`) resta escluso di proposito: cambiarlo non
-- modifica le quantita' e farebbe comparire l'avviso senza motivo.

CREATE TRIGGER trg_ricetta_versione_ingrediente_insert
AFTER INSERT ON ricetta_ingredienti
BEGIN
    UPDATE ricette
       SET aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
     WHERE id = NEW.ricetta_id;
END;

CREATE TRIGGER trg_ricetta_versione_ingrediente_update
AFTER UPDATE ON ricetta_ingredienti
BEGIN
    UPDATE ricette
       SET aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
     WHERE id IN (NEW.ricetta_id, OLD.ricetta_id);
END;

-- Quando si elimina una ricetta la cascata rimuove prima la riga padre, quindi
-- questa UPDATE non trova nulla ed e' un no-op: e' il comportamento voluto.
CREATE TRIGGER trg_ricetta_versione_ingrediente_delete
AFTER DELETE ON ricetta_ingredienti
BEGIN
    UPDATE ricette
       SET aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
     WHERE id = OLD.ricetta_id;
END;

-- ---------------------------------------------------------------------------
-- 2. Partecipanti storici del pasto
-- ---------------------------------------------------------------------------
-- Il nome snapshot resta sempre valorizzato, quindi tiene distinti i
-- partecipanti anche dopo l'eliminazione dei profili.

CREATE UNIQUE INDEX idx_planner_pasto_profili_storici
    ON planner_pasto_profili (pasto_id, profilo_nome_snapshot)
    WHERE profilo_alimentare_id IS NULL;
