-- Step 7.3B - Versione contenuto della ricetta e pasti liberi nel planner.
--
-- Decisioni approvate prima dell'implementazione:
-- - `ricette.aggiornato_il` diventa una vera versione del contenuto, cosi'
--   `planner_alimentare::recipe_update_available` rileva anche la modifica di un
--   ingrediente e non solo rinomina/porzioni/archiviazione;
-- - un pasto del planner puo' essere una voce libera ("cena fuori", "avanzi")
--   senza ricetta collegata e senza ingredienti.
--
-- Migration append-only: non modifica ne' riscrive nulla di gia' applicato e non
-- popola retroattivamente alcun dato.

-- ---------------------------------------------------------------------------
-- 1. Versione contenuto della ricetta
-- ---------------------------------------------------------------------------
-- Il codice aggiorna `ricette.aggiornato_il` solo su rinomina, cambio
-- `porzioni_base` e archiviazione. Gli ingredienti, che sono cio' che il planner
-- deve davvero seguire, non lo toccavano.
--
-- I trigger agiscono solo sulla colonna `aggiornato_il`: non fanno scattare
-- `trg_ricetta_nome_unico_spazi_update`, che e' un BEFORE UPDATE OF
-- nome_normalizzato.
--
-- Il procedimento (`ricetta_step`) e' escluso di proposito: cambiarlo non
-- modifica le quantita' e farebbe comparire `Aggiorna` senza motivo.

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

-- Su eliminazione della ricetta la cascata rimuove prima la riga padre, quindi
-- questa UPDATE non trova nulla ed e' un no-op: corretto, non va evitato.
CREATE TRIGGER trg_ricetta_versione_ingrediente_delete
AFTER DELETE ON ricetta_ingredienti
BEGIN
    UPDATE ricette
       SET aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
     WHERE id = OLD.ricetta_id;
END;

-- ---------------------------------------------------------------------------
-- 2. Pasti liberi
-- ---------------------------------------------------------------------------
-- `tipo_voce` distingue un pasto basato su ricetta da una voce libera. Serve
-- perche' `ricetta_id` puo' diventare NULL anche per una ricetta eliminata
-- (ON DELETE SET NULL): senza discriminante i due casi sarebbero indistinguibili.
--
-- Per una voce libera:
-- - `ricetta_id` resta NULL;
-- - `ricetta_nome_snapshot` contiene il titolo scritto dall'utente;
-- - `ricetta_porzione_base_snapshot` vale 1, valore neutro imposto dal CHECK
--   originale della tabella, che non e' modificabile da ALTER TABLE;
-- - non esistono righe in `planner_pasto_ingredienti_snapshot`, quindi la voce
--   non contribuira' alla futura lista della spesa.
--
-- I pasti gia' esistenti restano 'ricetta' grazie al DEFAULT.

ALTER TABLE planner_pasti
    ADD COLUMN tipo_voce TEXT NOT NULL DEFAULT 'ricetta'
        CHECK (tipo_voce IN ('ricetta', 'libero'));

-- Una voce libera non puo' essere collegata a una ricetta.
CREATE TRIGGER trg_planner_pasto_libero_senza_ricetta_insert
BEFORE INSERT ON planner_pasti
WHEN NEW.tipo_voce = 'libero' AND NEW.ricetta_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'una voce libera non puo essere collegata a una ricetta');
END;

CREATE TRIGGER trg_planner_pasto_libero_senza_ricetta_update
BEFORE UPDATE OF tipo_voce, ricetta_id ON planner_pasti
WHEN NEW.tipo_voce = 'libero' AND NEW.ricetta_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'una voce libera non puo essere collegata a una ricetta');
END;

-- Un pasto basato su ricetta deve nascere con una ricetta reale. Puo' poi
-- restare orfano se la ricetta viene eliminata, e in quel caso sopravvive con il
-- solo snapshot.
CREATE TRIGGER trg_planner_pasto_ricetta_obbligatoria_insert
BEFORE INSERT ON planner_pasti
WHEN NEW.tipo_voce = 'ricetta' AND NEW.ricetta_id IS NULL
BEGIN
    SELECT RAISE(ABORT, 'un pasto su ricetta richiede una ricetta esistente');
END;

-- Una voce libera non puo' avere snapshot di ingredienti.
CREATE TRIGGER trg_planner_snapshot_solo_su_ricetta
BEFORE INSERT ON planner_pasto_ingredienti_snapshot
WHEN EXISTS (
    SELECT 1
    FROM planner_pasti p
    WHERE p.id = NEW.pasto_id
      AND p.tipo_voce = 'libero'
)
BEGIN
    SELECT RAISE(ABORT, 'una voce libera non ha ingredienti');
END;

-- Il congelamento del pasto completato copre anche il nuovo campo: il trigger
-- originale e' stato scritto prima che `tipo_voce` esistesse.
CREATE TRIGGER trg_planner_pasto_completato_tipo_voce
BEFORE UPDATE OF tipo_voce ON planner_pasti
WHEN OLD.stato = 'completato' AND NEW.tipo_voce <> OLD.tipo_voce
BEGIN
    SELECT RAISE(ABORT, 'pasto completato non modificabile');
END;

-- ---------------------------------------------------------------------------
-- 3. Partecipanti storici
-- ---------------------------------------------------------------------------
-- `planner_pasto_profili` ha PRIMARY KEY (pasto_id, profilo_alimentare_id) con la
-- colonna profilo ON DELETE SET NULL. SQLite ammette NULL nelle primary key
-- composite, quindi due profili eliminati produrrebbero due righe (pasto, NULL)
-- non piu' distinguibili. L'indice parziale sotto tiene univoco il partecipante
-- storico usando il nome snapshot, che resta sempre valorizzato.

CREATE UNIQUE INDEX idx_planner_pasto_profili_storici
    ON planner_pasto_profili (pasto_id, profilo_nome_snapshot)
    WHERE profilo_alimentare_id IS NULL;
