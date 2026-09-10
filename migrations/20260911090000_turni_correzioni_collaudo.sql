-- Correzioni al modulo Turni e routine emerse dal primo collaudo dal vivo su
-- Telegram (11 settembre 2026), tredici punti decisi con Alessio. Additiva:
-- la migration 20260910120000_turni_e_routine.sql e' gia' applicata al
-- database reale dell'S9 e non viene mai modificata.
--
-- Contenuto:
-- 1. un solo pasto per tipo in un modello / in un'assegnazione (indice
--    UNICO, punto 2 del collaudo);
-- 2. orario del pasto vero del planner (`planner_pasti.orario`, punto 4);
-- 3. il modello appartiene a un profilo, non l'assegnazione
--    (`turno_modelli.profilo_alimentare_id` + snapshot, punto 13), con
--    backfill dei modelli di test gia' esistenti sul database reale.

-- --- 1. Un solo pasto per tipo -------------------------------------------
--
-- Gia' naturalmente rispettato dal flusso guidato (i 5 tipi fissi non si
-- possono scegliere due volte), ma il vincolo vero sta qui: impedisce anche
-- un'incoerenza creata da un futuro bug applicativo o da un accesso diretto
-- al database.
CREATE UNIQUE INDEX idx_turno_modello_pasti_tipo_unico
    ON turno_modello_pasti (modello_id, tipo_pasto);

CREATE UNIQUE INDEX idx_turno_assegnazione_pasti_tipo_unico
    ON turno_assegnazione_pasti (assegnazione_id, tipo_pasto);

-- --- 2. Orario del pasto vero del planner ---------------------------------
--
-- Stesso formato e stesso CHECK gia' usato per turno_modello_pasti.orario:
-- opzionale, HH:MM. Il default (dal turno assegnato quel giorno, o dai
-- default fissi per tipo) e' calcolato in Rust e proposto come suggerimento
-- modificabile, mai scritto qui.
ALTER TABLE planner_pasti
ADD COLUMN orario TEXT
    CHECK (orario IS NULL OR (length(orario) = 5 AND substr(orario, 3, 1) = ':'));

-- --- 3. Il modello appartiene a un profilo, non l'assegnazione ------------
--
-- Resta nullable a livello di schema (non tutte le righe storiche possono
-- essere backfillate con certezza, vedi sotto): l'obbligatorieta' "un
-- modello ha sempre un profilo" e' imposta dal livello applicativo a ogni
-- nuova creazione (`turni::crea_modello` richiede sempre un profilo).
ALTER TABLE turno_modelli
ADD COLUMN profilo_alimentare_id INTEGER
    REFERENCES profili_alimentari(id) ON DELETE SET NULL;

ALTER TABLE turno_modelli
ADD COLUMN profilo_nome_snapshot TEXT;

CREATE INDEX idx_turno_modelli_profilo
    ON turno_modelli (profilo_alimentare_id);

-- Backfill dei modelli di test gia' creati da Alessio durante il collaudo
-- (es. "Gelateria") sul database reale: assegnati al profilo "se' stesso"
-- del proprietario, cioe' il profilo con utente_collegato_id uguale al
-- proprietario del modello. Se un proprietario non ha un profilo "se'
-- stesso" collegato (per qualunque motivo), il modello resta con
-- profilo_alimentare_id NULL invece di far fallire la migration: va
-- notato e sistemato a mano dall'interfaccia (un pulsante sul dettaglio
-- del modello permette di impostare il profilo mancante in un secondo
-- momento, vedi src/modules/turni.rs).
UPDATE turno_modelli
SET profilo_alimentare_id = (
        SELECT pa.id FROM profili_alimentari pa
        WHERE pa.utente_collegato_id = turno_modelli.proprietario_utente_id
          AND pa.archiviato = 0
        ORDER BY pa.id
        LIMIT 1
    ),
    profilo_nome_snapshot = (
        SELECT pa.nome FROM profili_alimentari pa
        WHERE pa.utente_collegato_id = turno_modelli.proprietario_utente_id
          AND pa.archiviato = 0
        ORDER BY pa.id
        LIMIT 1
    )
WHERE profilo_alimentare_id IS NULL;

-- --- 10. "Aggiorna assegnazione" quando il modello cambia dopo -----------
--
-- Snapshot di turno_modelli.aggiornato_il al momento dell'assegnazione:
-- confrontato con il valore attuale del modello dice se i suoi pasti sono
-- cambiati da allora, stesso principio di
-- planner_pasti.ricetta_aggiornato_il_snapshot per "🔄 Aggiorna planner".
-- NULL per le assegnazioni gia' esistenti sul database reale (create prima
-- di questa colonna): senza uno snapshot da confrontare, per loro
-- l'aggiornamento resta semplicemente non proposto, mai un falso positivo.
ALTER TABLE turno_assegnazioni
ADD COLUMN modello_aggiornato_il_snapshot TEXT;
