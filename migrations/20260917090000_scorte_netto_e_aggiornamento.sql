-- Secondo giro del 16-17 settembre 2026, dal collaudo dal vivo di Alessio
-- della chiusura della spesa e delle scorte. Dettagli in STATO.md (sezione
-- 2decies) e in docs/moduli/lista-spesa.md, docs/moduli/dispensa.md.
--
-- Il principio, parole di Alessio: fare tutto nel modo piu' automatico
-- possibile, ma l'utente deve poter aggiustare a mano qualunque cosa.

-- ---------------------------------------------------------------------------
-- 1. Dove va a finire ogni alimento una volta comprato.
-- ---------------------------------------------------------------------------

-- Il default per categoria. Le eccezioni per nome (surgelati in freezer,
-- patate e cipolle fuori dal frigo, frutti di bosco in frigo...) stanno nel
-- codice (`dispensa::conservazione_da_nome`), dove si possono testare e
-- valgono anche per gli alimenti creati in futuro dagli utenti.
ALTER TABLE categorie_alimento
    ADD COLUMN conservazione_predefinita TEXT NOT NULL DEFAULT 'dispensa'
        CHECK (conservazione_predefinita IN ('dispensa', 'frigo', 'freezer'));

UPDATE categorie_alimento SET conservazione_predefinita = 'frigo'
WHERE codice IN ('verdura', 'carne', 'pesce', 'latticini', 'uova');

-- La frutta resta in dispensa di default (banane, agrumi, frutta da far
-- maturare); quella che sta meglio al fresco (frutti di bosco, ciliegie, uva,
-- mele, prugne) ci va per nome.

-- La scelta fatta a mano ("📌 Qui d'ora in poi") vale per lo spazio, non per
-- tutti: il catalogo globale e' condiviso, e cambiare li' il posto della
-- pasta lo cambierebbe a ogni utente del bot.
CREATE TABLE scorte_destinazioni (
    id INTEGER PRIMARY KEY,
    spazio_id INTEGER NOT NULL
        REFERENCES spazi(id) ON DELETE CASCADE,
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE CASCADE,
    prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE CASCADE,
    conservazione TEXT NOT NULL
        CHECK (conservazione IN ('dispensa', 'frigo', 'freezer')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (
        (alimento_id IS NOT NULL AND prodotto_alimentare_id IS NULL)
        OR (alimento_id IS NULL AND prodotto_alimentare_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_scorte_destinazioni_alimento
    ON scorte_destinazioni (spazio_id, alimento_id)
    WHERE alimento_id IS NOT NULL;

CREATE UNIQUE INDEX idx_scorte_destinazioni_prodotto
    ON scorte_destinazioni (spazio_id, prodotto_alimentare_id)
    WHERE prodotto_alimentare_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 2. Aggiornamento automatico della lista, con il resoconto dei cambiamenti.
-- ---------------------------------------------------------------------------

-- Acceso di default, per utente, come dispensa_ingresso_automatico.
ALTER TABLE preferenze_utente
    ADD COLUMN lista_spesa_aggiornamento_automatico INTEGER NOT NULL DEFAULT 1
        CHECK (lista_spesa_aggiornamento_automatico IN (0, 1));

-- Ogni aggiornamento che ha cambiato davvero qualcosa. Salvato a database e
-- non tenuto in memoria (deciso con Alessio): "avvisare sempre" regge male
-- se l'avviso puo' svanire con un riavvio del bot.
CREATE TABLE liste_spesa_aggiornamenti (
    id INTEGER PRIMARY KEY,
    lista_id INTEGER NOT NULL
        REFERENCES liste_spesa(id) ON DELETE CASCADE,
    utente_id INTEGER
        REFERENCES utenti(id) ON DELETE SET NULL,
    automatico INTEGER NOT NULL CHECK (automatico IN (0, 1)),
    -- Ora locale del telefono, gia' pronta da mostrare: e' l'unico posto che
    -- conosce il fuso orario (vedi STATO.md, "Aritmetica delle date").
    avvenuto_il_locale TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%d %H:%M', 'now', 'localtime')),
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_liste_spesa_aggiornamenti_lista
    ON liste_spesa_aggiornamenti (lista_id, id);

CREATE TABLE liste_spesa_modifiche (
    id INTEGER PRIMARY KEY,
    aggiornamento_id INTEGER NOT NULL
        REFERENCES liste_spesa_aggiornamenti(id) ON DELETE CASCADE,
    tipo TEXT NOT NULL
        CHECK (tipo IN ('aggiunta', 'tolta', 'aumentata', 'ridotta')),
    descrizione TEXT NOT NULL,
    unita_simbolo TEXT NOT NULL,
    quantita_prima REAL,
    quantita_dopo REAL,
    -- Perche' una voce e' sparita o calata: "ce l'hai gia' in casa" oppure
    -- "non piu' pianificata". NULL per aggiunte e aumenti.
    motivo TEXT,
    CHECK (length(trim(descrizione)) > 0)
);

CREATE INDEX idx_liste_spesa_modifiche_aggiornamento
    ON liste_spesa_modifiche (aggiornamento_id, id);

-- Storia, come l'archivio della spesa chiusa: non si riscrive.
CREATE TRIGGER trg_lista_spesa_modifica_immutabile
BEFORE UPDATE ON liste_spesa_modifiche
BEGIN
    SELECT RAISE(ABORT, 'modifica registrata non modificabile');
END;

-- ---------------------------------------------------------------------------
-- 3. Scarico delle scorte quando un pasto viene preparato o consumato.
-- ---------------------------------------------------------------------------

-- `preparato_il` non e' un nuovo stato del pasto: il pasto resta
-- 'pianificato' finche' non lo si consuma, cosi' non si tocca la logica di
-- congelamento gia' collaudata (i trigger sui pasti completati e saltati
-- non elencano queste colonne, quindi restano aggiornabili).
ALTER TABLE planner_pasti ADD COLUMN preparato_il TEXT;

-- Quando le scorte sono gia' state scalate per questo pasto: una volta sola,
-- qualunque sia la strada (preparato, consumato, orario passato).
ALTER TABLE planner_pasti ADD COLUMN scorte_scalate_il TEXT;

-- 1 se lo scarico e' avvenuto da solo, a orario passato: solo quello viene
-- restituito se poi il pasto viene segnato saltato. Uno scarico fatto a mano
-- (preparato) resta: il cibo e' stato usato comunque.
ALTER TABLE planner_pasti
    ADD COLUMN scorte_scalate_automaticamente INTEGER NOT NULL DEFAULT 0
        CHECK (scorte_scalate_automaticamente IN (0, 1));

-- I pasti dei giorni passati non devono svuotare le scorte di oggi al primo
-- avvio: sono storia, precedente a questa funzione. Si considerano gia'
-- "scaricati".
UPDATE planner_pasti
SET scorte_scalate_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE date(data_pasto) < date('now', 'localtime');

CREATE INDEX idx_planner_pasti_scorte_da_scalare
    ON planner_pasti (scorte_scalate_il, data_pasto)
    WHERE scorte_scalate_il IS NULL;

-- Ogni prelievo dalle scorte, per poterlo restituire (pasto saltato dopo uno
-- scarico automatico) e per sapere in futuro cosa e' stato usato e quando.
CREATE TABLE scorte_movimenti (
    id INTEGER PRIMARY KEY,
    spazio_id INTEGER
        REFERENCES spazi(id) ON DELETE CASCADE,
    pasto_id INTEGER
        REFERENCES planner_pasti(id) ON DELETE SET NULL,
    -- La scorta toccata, se esiste ancora (una scorta finita viene
    -- eliminata: per restituirla la si ricrea con questi dati).
    scorta_id INTEGER
        REFERENCES scorte(id) ON DELETE SET NULL,
    conservazione TEXT NOT NULL
        CHECK (conservazione IN ('dispensa', 'frigo', 'freezer')),
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE SET NULL,
    prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE SET NULL,
    descrizione TEXT NOT NULL,
    quantita REAL NOT NULL CHECK (quantita > 0),
    unita_simbolo TEXT NOT NULL,
    scadenza TEXT,
    automatico INTEGER NOT NULL CHECK (automatico IN (0, 1)),
    restituito_il TEXT,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_scorte_movimenti_pasto
    ON scorte_movimenti (pasto_id, restituito_il);

-- ---------------------------------------------------------------------------
-- 4. Ordine della lista: numeri di posizione doppi.
-- ---------------------------------------------------------------------------

-- Trovato dal collaudo: sul database reale due voci avevano lo stesso
-- `ordinamento`, e scambiare due posizioni uguali non sposta niente -- le
-- frecce del riordino sembravano morte. Il ricalcolo li aveva prodotti
-- riusando il numero di una voce rigenerata mentre contava le nuove solo da
-- quelle rimaste. Da qui in avanti `aggiorna_lista` rinumera a ogni giro;
-- qui si ripuliscono i dati gia' esistenti, mantenendo l'ordine visibile.
UPDATE liste_spesa_voci
SET ordinamento = (
    SELECT numerate.posizione
    FROM (
        SELECT id,
               ROW_NUMBER() OVER (
                   PARTITION BY lista_id ORDER BY ordinamento, id
               ) AS posizione
        FROM liste_spesa_voci
    ) AS numerate
    WHERE numerate.id = liste_spesa_voci.id
);
