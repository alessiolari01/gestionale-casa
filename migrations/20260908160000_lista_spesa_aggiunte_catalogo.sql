-- Aggiunta manuale con ricerca nel catalogo, merge con l'aggregazione del
-- planner: secondo miglioramento deciso con Alessio dopo il primo collaudo
-- dal vivo della lista della spesa (8 settembre 2026), continuazione dello
-- stesso lavoro (vedi migrations/20260908150000_lista_spesa.sql).
--
-- Le aggiunte dal catalogo (alimento generico o prodotto commerciale
-- specifico) non sono uno snapshot di un pasto: restano vive in una
-- tabella propria e partecipano di nuovo ogni volta al ricalcolo di
-- "🔄 Aggiorna lista", a differenza delle vecchie righe 'generato'
-- pure-planner che vengono cancellate e rigenerate da zero a ogni
-- refresh. Un alimento generico (es. "Pasta") si somma al fabbisogno già
-- calcolato dal planner sullo stesso alimento -- un'unica riga, non due;
-- un prodotto commerciale specifico (es. "Pasta De Cecco") resta sempre
-- una riga separata e distinta, anche se collegato allo stesso alimento
-- generico richiesto altrove -- decisione esplicita presa con Alessio per
-- non confondere "mi serve della pasta" con "voglio comprare proprio
-- quella marca".
--
-- Niente CHECK "tipo='alimento' => alimento_id NOT NULL" (o l'equivalente
-- per 'prodotto'): ON DELETE SET NULL esegue un UPDATE che violerebbe quel
-- CHECK quando l'alimento o il prodotto referenziato viene cancellato dal
-- catalogo in seguito -- stesso motivo per cui
-- planner_pasto_ingredienti_snapshot.alimento_id non ha un vincolo simile.
-- descrizione_snapshot basta a restare leggibile anche in quel caso.
CREATE TABLE liste_spesa_aggiunte_catalogo (
    id INTEGER PRIMARY KEY,
    lista_id INTEGER NOT NULL
        REFERENCES liste_spesa(id) ON DELETE CASCADE,
    tipo TEXT NOT NULL CHECK (tipo IN ('alimento', 'prodotto')),
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE SET NULL,
    prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE SET NULL,
    descrizione_snapshot TEXT NOT NULL,
    quantita REAL NOT NULL CHECK (quantita > 0),
    unita_simbolo TEXT NOT NULL,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(descrizione_snapshot)) > 0)
);

CREATE INDEX idx_lista_spesa_aggiunte_catalogo_lista
    ON liste_spesa_aggiunte_catalogo (lista_id);

-- Le voci generate ('generato') in liste_spesa_voci possono ora venire
-- anche da un prodotto commerciale specifico aggiunto dal catalogo: serve
-- una colonna dedicata, perché alimento_id resta una FK verso alimenti e
-- non può ospitare un id di prodotti_alimentari.
ALTER TABLE liste_spesa_voci
    ADD COLUMN prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE SET NULL;

CREATE INDEX idx_lista_spesa_voci_prodotto
    ON liste_spesa_voci (prodotto_alimentare_id, lista_id);

-- Il trigger di congelamento (migrations/20260908150000_lista_spesa.sql) va
-- ricreato per includere anche la nuova colonna nel controllo di
-- immutabilità: una voce comprata resta congelata nello stesso modo,
-- indipendentemente dal fatto che venga dal planner o da un'aggiunta dal
-- catalogo.
DROP TRIGGER trg_lista_spesa_voce_comprata_immutabile;

CREATE TRIGGER trg_lista_spesa_voce_comprata_immutabile
BEFORE UPDATE ON liste_spesa_voci
WHEN OLD.comprato = 1
AND (
    NEW.lista_id <> OLD.lista_id
    OR NEW.origine <> OLD.origine
    OR NEW.alimento_id IS NOT OLD.alimento_id
    OR NEW.prodotto_alimentare_id IS NOT OLD.prodotto_alimentare_id
    OR NEW.descrizione <> OLD.descrizione
    OR NEW.quantita IS NOT OLD.quantita
    OR NEW.unita_simbolo IS NOT OLD.unita_simbolo
)
BEGIN
    SELECT RAISE(ABORT, 'voce comprata non modificabile, solo il toggle comprato e'' permesso');
END;
