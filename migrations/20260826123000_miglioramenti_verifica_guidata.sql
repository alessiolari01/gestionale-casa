-- Step 7.2G.1 — Miglioramenti verificabili, stato Fatto e collaudo guidato.
-- Migration append-only: NON modificare le migration precedenti già applicate.
--
-- Nuovo ciclo:
--   da_approvare -> da_fare -> fatto -> archivio
-- Lo stato "fatto" significa "implementato e in attesa di verifica admin".
-- L'archiviazione avviene solo dopo conferma esplicita dell'amministratore principale.

-- SQLite non consente di estendere il CHECK dello stato in-place: ricostruiamo
-- soltanto la tabella corrente mantenendo gli ID e i dati esistenti.
CREATE TABLE miglioramenti_step7g1 (
    id INTEGER PRIMARY KEY,
    autore_utente_id INTEGER NOT NULL,
    descrizione TEXT NOT NULL,
    modulo TEXT,
    stato TEXT NOT NULL DEFAULT 'da_approvare'
        CHECK (stato IN ('da_approvare', 'da_fare', 'fatto', 'scartato')),
    letto_admin_il TEXT,
    fatto_il TEXT,
    verifica_esito TEXT CHECK (verifica_esito IS NULL OR verifica_esito IN ('ok', 'problema')),
    verifica_note TEXT,
    verificato_il TEXT,
    verificato_da_utente_id INTEGER,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (autore_utente_id) REFERENCES utenti(id) ON DELETE RESTRICT,
    FOREIGN KEY (verificato_da_utente_id) REFERENCES utenti(id) ON DELETE SET NULL,
    CHECK (length(trim(descrizione)) > 0),
    CHECK (modulo IS NULL OR length(trim(modulo)) > 0),
    CHECK (verifica_note IS NULL OR length(trim(verifica_note)) > 0)
);

INSERT INTO miglioramenti_step7g1 (
    id,
    autore_utente_id,
    descrizione,
    modulo,
    stato,
    letto_admin_il,
    fatto_il,
    verifica_esito,
    verifica_note,
    verificato_il,
    verificato_da_utente_id,
    creato_il,
    aggiornato_il
)
SELECT
    m.id,
    m.autore_utente_id,
    m.descrizione,
    m.modulo,
    CASE
        -- I suggerimenti reali dell'amministratore principale sono requisiti
        -- da fare anche se uno stato storico li aveva classificati diversamente.
        WHEN u.ruolo_sistema = 'admin'
         AND u.amministratore_principale = 1
         AND instr(lower(m.descrizione), 'prova') = 0
            THEN 'da_fare'
        ELSE m.stato
    END,
    m.letto_admin_il,
    NULL,
    NULL,
    NULL,
    NULL,
    NULL,
    m.creato_il,
    m.aggiornato_il
FROM miglioramenti AS m
JOIN utenti AS u ON u.id = m.autore_utente_id;

-- Questo pacchetto implementa concretamente i miglioramenti 5, 6 e 8-13
-- presenti nel DB di handoff 54dc4dd. Restano attivi come "fatto" finché
-- l'amministratore principale non li verifica e li archivia manualmente.
UPDATE miglioramenti_step7g1
SET stato = 'fatto',
    fatto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    letto_admin_il = COALESCE(letto_admin_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (5, 6, 8, 9, 10, 11, 12, 13)
  AND EXISTS (
      SELECT 1
      FROM utenti u
      WHERE u.id = miglioramenti_step7g1.autore_utente_id
        AND u.ruolo_sistema = 'admin'
        AND u.amministratore_principale = 1
  )
  AND instr(lower(descrizione), 'prova') = 0;

-- Conserviamo anche gli allegati originali mentre ricostruiamo la tabella
-- padre, così non rompiamo la foreign key durante il DROP/RENAME.
CREATE TABLE miglioramento_allegati_step7g1 (
    id INTEGER PRIMARY KEY,
    miglioramento_id INTEGER NOT NULL,
    tipo TEXT NOT NULL DEFAULT 'foto' CHECK (tipo = 'foto'),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (miglioramento_id)
        REFERENCES miglioramenti_step7g1(id)
        ON DELETE CASCADE,
    CHECK (length(trim(percorso_file)) > 0)
);

INSERT INTO miglioramento_allegati_step7g1 (
    id, miglioramento_id, tipo, percorso_file, descrizione, creato_il
)
SELECT id, miglioramento_id, tipo, percorso_file, descrizione, creato_il
FROM miglioramento_allegati;

DROP TABLE miglioramento_allegati;
DROP TABLE miglioramenti;
ALTER TABLE miglioramenti_step7g1 RENAME TO miglioramenti;
ALTER TABLE miglioramento_allegati_step7g1 RENAME TO miglioramento_allegati;

CREATE INDEX idx_miglioramenti_autore_data
    ON miglioramenti(autore_utente_id, creato_il DESC, id DESC);
CREATE INDEX idx_miglioramenti_stato_data
    ON miglioramenti(stato, aggiornato_il DESC, id DESC);
CREATE INDEX idx_miglioramenti_non_letti_admin
    ON miglioramenti(letto_admin_il, stato, aggiornato_il DESC, id DESC);
CREATE INDEX idx_miglioramenti_verifica
    ON miglioramenti(stato, verifica_esito, verificato_il, aggiornato_il DESC, id DESC);
CREATE INDEX idx_miglioramento_allegati_miglioramento
    ON miglioramento_allegati(miglioramento_id, creato_il, id);

-- Piano di collaudo che ChatGPT può aggiornare/consegnare insieme al codice.
-- L'azione callback è opzionale: quando presente il bot mostra un pulsante
-- Telegram reale che porta direttamente alla schermata da testare.
CREATE TABLE miglioramento_piani_verifica (
    miglioramento_id INTEGER PRIMARY KEY,
    titolo TEXT NOT NULL,
    istruzioni TEXT NOT NULL,
    azione_label TEXT,
    azione_callback TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (miglioramento_id) REFERENCES miglioramenti(id) ON DELETE CASCADE,
    CHECK (length(trim(titolo)) > 0),
    CHECK (length(trim(istruzioni)) > 0),
    CHECK ((azione_label IS NULL AND azione_callback IS NULL)
        OR (length(trim(azione_label)) > 0 AND length(trim(azione_callback)) > 0))
);

-- Screenshot/video inviati durante il collaudo sono separati dagli allegati
-- originali del suggerimento e restano disponibili fino all'archiviazione.
CREATE TABLE miglioramento_verifica_allegati (
    id INTEGER PRIMARY KEY,
    miglioramento_id INTEGER NOT NULL,
    tipo TEXT NOT NULL CHECK (tipo IN ('foto', 'video')),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (miglioramento_id) REFERENCES miglioramenti(id) ON DELETE CASCADE,
    CHECK (length(trim(percorso_file)) > 0)
);
CREATE INDEX idx_miglioramento_verifica_allegati
    ON miglioramento_verifica_allegati(miglioramento_id, creato_il, id);

-- Conserviamo l'esito del collaudo anche nello storico archiviato.
ALTER TABLE miglioramenti_archivio ADD COLUMN verifica_esito TEXT;
ALTER TABLE miglioramenti_archivio ADD COLUMN verifica_note TEXT;
ALTER TABLE miglioramenti_archivio ADD COLUMN verificato_il TEXT;
ALTER TABLE miglioramenti_archivio ADD COLUMN verificato_da_utente_id INTEGER
    REFERENCES utenti(id) ON DELETE SET NULL;

CREATE TABLE miglioramento_archivio_verifica_allegati (
    id INTEGER PRIMARY KEY,
    miglioramento_archivio_id INTEGER NOT NULL,
    tipo TEXT NOT NULL CHECK (tipo IN ('foto', 'video')),
    percorso_file TEXT NOT NULL,
    descrizione TEXT,
    creato_il TEXT NOT NULL,
    FOREIGN KEY (miglioramento_archivio_id)
        REFERENCES miglioramenti_archivio(id)
        ON DELETE CASCADE,
    CHECK (length(trim(percorso_file)) > 0)
);
CREATE INDEX idx_miglioramento_archivio_verifica_allegati
    ON miglioramento_archivio_verifica_allegati(miglioramento_archivio_id, id);

-- Piani di verifica per i miglioramenti implementati in questo pacchetto.
INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Ritorno dopo cambio stato',
       '/miglioramenti_da_fare\nApri un miglioramento ancora Da fare, cambia stato e verifica che il bot torni alla lista Miglioramenti invece di restare nel dettaglio.',
       '💡 Apri Da fare',
       'improve:list:todo:0'
FROM miglioramenti WHERE id = 5 AND stato = 'fatto';

INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Eliminazione formato prodotto',
       '/alimenti\nApri un alimento con prodotto commerciale, entra nei formati, apri un formato e premi 🗑 Elimina formato. Conferma e verifica che il formato sparisca.',
       '🥕 Apri Alimenti',
       'food:foods'
FROM miglioramenti WHERE id = 6 AND stato = 'fatto';

INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Unità ingrediente prima della quantità',
       '/ricetta_nuova\nAggiungi un ingrediente: dopo alimento/prodotto deve comparire l’unità suggerita e il pulsante 📏 Cambia unità prima di inserire la quantità.',
       '➕ Nuova ricetta',
       'recipe:new'
FROM miglioramenti WHERE id = 8 AND stato = 'fatto';

INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Paginazione Miglioramenti',
       '/miglioramenti_tutti\nVerifica massimo 5 elementi per pagina, indicatore X/Y e pulsanti pagina precedente/successiva. Il totale deve includere tutti gli attivi.',
       '🗂 Tutti i miglioramenti',
       'improve:list:all:0'
FROM miglioramenti WHERE id = 9 AND stato = 'fatto';

INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Menu Alimentazione',
       '/alimentazione\nLa schermata deve mostrare Alimenti, Ricette e poi Indietro/Menu principale. /alimenti deve aprire le opzioni elenco, cerca e filtra.',
       '🍽 Alimentazione',
       'food:menu'
FROM miglioramenti WHERE id = 10 AND stato = 'fatto';

INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Eliminazione definitiva ricetta',
       '/ricette\nApri una tua ricetta → Modifica → 🗑 Elimina definitivamente. Verifica la doppia conferma e che la ricetta non compaia più.',
       '🍳 Apri Ricette',
       'recipe:menu'
FROM miglioramenti WHERE id = 11 AND stato = 'fatto';

INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Fine procedura guidata',
       '/ricette\nApri una ricetta con procedimento guidato, raggiungi l’ultimo step e premi ✅ Termina. Deve apparire una conferma esplicita che la ricetta è terminata.',
       '🍳 Apri Ricette',
       'recipe:menu'
FROM miglioramenti WHERE id = 12 AND stato = 'fatto';

INSERT INTO miglioramento_piani_verifica
    (miglioramento_id, titolo, istruzioni, azione_label, azione_callback)
SELECT id,
       'Ricerca ricette per ingredienti e categoria',
       '/ricette_ingredienti\nDigita subito un alimento senza premere Aggiungi ingrediente. Verifica anche il pulsante 🏷 Categorie e la selezione degli alimenti della categoria.',
       '🥕 Cerca per ingredienti',
       'recipe:find'
FROM miglioramenti WHERE id = 13 AND stato = 'fatto';
