-- Step 7.1 - Fondazioni condivise: utenti, spazi, membership e audit autore.
--
-- Obiettivi della migration:
-- - introdurre identita' interne separate da Telegram;
-- - creare lo spazio bootstrap a cui associare i dati pre-Step 7;
-- - predisporre ruoli, preferenze e inviti;
-- - aggiungere il confine di spazio alle principali entita' radice;
-- - estendere lo storico con autore/origine/spazio senza inventare autori
--   retroattivi per gli eventi Step 6B/6C.
--
-- Compatibilita': il codice Step 6 continua a creare dati nello spazio #1
-- grazie al DEFAULT 1. L'isolamento operativo tra piu' spazi verra' attivato
-- quando le query CRUD saranno rese space-aware; fino ad allora lo spazio #1
-- resta lo spazio runtime di compatibilita' del database di sviluppo.

CREATE TABLE utenti (
    id INTEGER PRIMARY KEY,
    nome_visualizzato TEXT NOT NULL,
    stato TEXT NOT NULL DEFAULT 'attivo'
        CHECK (stato IN ('attivo', 'disabilitato')),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome_visualizzato)) > 0)
);

CREATE TABLE spazi (
    id INTEGER PRIMARY KEY,
    nome TEXT NOT NULL,
    tipo TEXT NOT NULL
        CHECK (tipo IN ('personale', 'famiglia', 'condiviso')),
    bootstrap_legacy INTEGER NOT NULL DEFAULT 0
        CHECK (bootstrap_legacy IN (0, 1)),
    creato_da_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0)
);

-- Lo spazio tecnico #1 riceve tutti i dati gia' presenti. Non viene creato
-- alcun utente fittizio: il primo account Telegram autorizzato che interagisce
-- con il bot verra' collegato a questo spazio dal codice runtime.
INSERT INTO spazi (id, nome, tipo, bootstrap_legacy)
VALUES (1, 'Spazio principale', 'personale', 1);

CREATE UNIQUE INDEX idx_spazi_bootstrap_unico
    ON spazi (bootstrap_legacy)
    WHERE bootstrap_legacy = 1;

CREATE TABLE membri_spazio (
    spazio_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE CASCADE,
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    ruolo TEXT NOT NULL
        CHECK (ruolo IN ('proprietario', 'amministratore', 'membro', 'lettura')),
    aggiunto_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (spazio_id, utente_id)
);

CREATE INDEX idx_membri_spazio_utente
    ON membri_spazio (utente_id, spazio_id);

CREATE TABLE account_telegram (
    id INTEGER PRIMARY KEY,
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    telegram_user_id INTEGER NOT NULL UNIQUE,
    chat_id INTEGER NOT NULL,
    username_snapshot TEXT,
    nome_snapshot TEXT NOT NULL,
    cognome_snapshot TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_account_telegram_utente
    ON account_telegram (utente_id);

CREATE INDEX idx_account_telegram_chat
    ON account_telegram (chat_id);

CREATE TABLE preferenze_utente (
    utente_id INTEGER PRIMARY KEY REFERENCES utenti(id) ON DELETE CASCADE,
    spazio_attivo_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE RESTRICT,
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TRIGGER trg_preferenze_spazio_membro_insert
BEFORE INSERT ON preferenze_utente
WHEN NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.utente_id = NEW.utente_id
      AND ms.spazio_id = NEW.spazio_attivo_id
)
BEGIN
    SELECT RAISE(ABORT, 'spazio attivo non appartenente all utente');
END;

CREATE TRIGGER trg_preferenze_spazio_membro_update
BEFORE UPDATE OF spazio_attivo_id ON preferenze_utente
WHEN NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.utente_id = NEW.utente_id
      AND ms.spazio_id = NEW.spazio_attivo_id
)
BEGIN
    SELECT RAISE(ABORT, 'spazio attivo non appartenente all utente');
END;

CREATE TABLE inviti_spazio (
    id INTEGER PRIMARY KEY,
    spazio_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE CASCADE,
    creato_da_utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    ruolo_proposto TEXT NOT NULL DEFAULT 'membro'
        CHECK (ruolo_proposto IN ('amministratore', 'membro', 'lettura')),
    scade_il TEXT,
    utilizzi_massimi INTEGER NOT NULL DEFAULT 1 CHECK (utilizzi_massimi > 0),
    utilizzi INTEGER NOT NULL DEFAULT 0 CHECK (utilizzi >= 0),
    revocato_il TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (utilizzi <= utilizzi_massimi)
);

CREATE INDEX idx_inviti_spazio_attivi
    ON inviti_spazio (spazio_id, scade_il)
    WHERE revocato_il IS NULL;

-- Confine di spazio sulle entita' radice gia' esistenti. SQLite non consente
-- di aggiungere in modo portabile una colonna REFERENCES NOT NULL con default
-- non nullo tramite ALTER TABLE: il riferimento a spazi viene quindi
-- garantito da trigger fino al futuro rebuild space-aware delle tabelle.
ALTER TABLE items ADD COLUMN spazio_id INTEGER NOT NULL DEFAULT 1;
ALTER TABLE abitazioni ADD COLUMN spazio_id INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tag ADD COLUMN spazio_id INTEGER NOT NULL DEFAULT 1;
ALTER TABLE storico_entita ADD COLUMN spazio_id INTEGER NOT NULL DEFAULT 1;
ALTER TABLE storico_eventi ADD COLUMN spazio_id INTEGER NOT NULL DEFAULT 1;

CREATE INDEX idx_items_spazio ON items (spazio_id, tipo, nome);
CREATE INDEX idx_abitazioni_spazio ON abitazioni (spazio_id, nome);
CREATE INDEX idx_tag_spazio ON tag (spazio_id, nome);
CREATE INDEX idx_storico_entita_spazio ON storico_entita (spazio_id, tipo_entita);
CREATE INDEX idx_storico_eventi_spazio_data
    ON storico_eventi (spazio_id, avvenuto_il DESC, id DESC);

CREATE TRIGGER trg_items_spazio_valido_insert
BEFORE INSERT ON items
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio item inesistente');
END;

CREATE TRIGGER trg_items_spazio_valido_update
BEFORE UPDATE OF spazio_id ON items
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio item inesistente');
END;

CREATE TRIGGER trg_abitazioni_spazio_valido_insert
BEFORE INSERT ON abitazioni
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio abitazione inesistente');
END;

CREATE TRIGGER trg_abitazioni_spazio_valido_update
BEFORE UPDATE OF spazio_id ON abitazioni
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio abitazione inesistente');
END;

CREATE TRIGGER trg_tag_spazio_valido_insert
BEFORE INSERT ON tag
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio tag inesistente');
END;

CREATE TRIGGER trg_tag_spazio_valido_update
BEFORE UPDATE OF spazio_id ON tag
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio tag inesistente');
END;

CREATE TRIGGER trg_storico_entita_spazio_valido_insert
BEFORE INSERT ON storico_entita
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio storico entita inesistente');
END;

CREATE TRIGGER trg_storico_entita_spazio_valido_update
BEFORE UPDATE OF spazio_id ON storico_entita
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio storico entita inesistente');
END;

-- Coerenza minima cross-space per i collegamenti gia' operativi nello Step 6.
CREATE TRIGGER trg_item_luogo_spazio_insert
BEFORE INSERT ON item_luogo
WHEN NEW.abitazione_id IS NOT NULL
AND EXISTS (
    SELECT 1
    FROM items i
    JOIN abitazioni a ON a.id = NEW.abitazione_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> a.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e abitazione appartengono a spazi diversi');
END;

CREATE TRIGGER trg_item_luogo_spazio_update
BEFORE UPDATE OF item_id, abitazione_id ON item_luogo
WHEN NEW.abitazione_id IS NOT NULL
AND EXISTS (
    SELECT 1
    FROM items i
    JOIN abitazioni a ON a.id = NEW.abitazione_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> a.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e abitazione appartengono a spazi diversi');
END;

CREATE TRIGGER trg_item_tag_spazio_insert
BEFORE INSERT ON item_tag
WHEN EXISTS (
    SELECT 1
    FROM items i
    JOIN tag t ON t.id = NEW.tag_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> t.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e tag appartengono a spazi diversi');
END;

CREATE TRIGGER trg_item_tag_spazio_update
BEFORE UPDATE OF item_id, tag_id ON item_tag
WHEN EXISTS (
    SELECT 1
    FROM items i
    JOIN tag t ON t.id = NEW.tag_id
    WHERE i.id = NEW.item_id
      AND i.spazio_id <> t.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'item e tag appartengono a spazi diversi');
END;

-- Audit autore/origine. Gli eventi vecchi restano senza autore: non sappiamo
-- con certezza chi li abbia eseguiti e non va inventata questa informazione.
ALTER TABLE storico_eventi
ADD COLUMN attore_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL;

ALTER TABLE storico_eventi
ADD COLUMN attore_nome_snapshot TEXT;

ALTER TABLE storico_eventi
ADD COLUMN spazio_nome_snapshot TEXT;

ALTER TABLE storico_eventi
ADD COLUMN origine_azione TEXT NOT NULL DEFAULT 'legacy'
    CHECK (origine_azione IN ('legacy', 'telegram', 'sistema', 'google', 'automazione'));

ALTER TABLE storico_eventi
ADD COLUMN automatico INTEGER NOT NULL DEFAULT 0
    CHECK (automatico IN (0, 1));

UPDATE storico_eventi
SET spazio_nome_snapshot = 'Spazio principale',
    automatico = CASE WHEN evento_padre_id IS NULL THEN 0 ELSE 1 END
WHERE spazio_nome_snapshot IS NULL;

CREATE INDEX idx_storico_eventi_attore
    ON storico_eventi (attore_utente_id, avvenuto_il DESC, id DESC);

CREATE INDEX idx_storico_eventi_origine
    ON storico_eventi (origine_azione, automatico, avvenuto_il DESC);

CREATE TRIGGER trg_storico_eventi_spazio_valido_insert
BEFORE INSERT ON storico_eventi
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio evento storico inesistente');
END;

CREATE TRIGGER trg_storico_eventi_spazio_valido_update
BEFORE UPDATE OF spazio_id ON storico_eventi
WHEN NOT EXISTS (SELECT 1 FROM spazi s WHERE s.id = NEW.spazio_id)
BEGIN
    SELECT RAISE(ABORT, 'spazio evento storico inesistente');
END;

CREATE TRIGGER trg_storico_eventi_entita_stesso_spazio_insert
BEFORE INSERT ON storico_eventi
WHEN EXISTS (
    SELECT 1
    FROM storico_entita se
    WHERE se.id = NEW.entita_storico_id
      AND se.spazio_id <> NEW.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'evento ed entita storica appartengono a spazi diversi');
END;

CREATE TRIGGER trg_storico_eventi_entita_stesso_spazio_update
BEFORE UPDATE OF entita_storico_id, spazio_id ON storico_eventi
WHEN EXISTS (
    SELECT 1
    FROM storico_entita se
    WHERE se.id = NEW.entita_storico_id
      AND se.spazio_id <> NEW.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'evento ed entita storica appartengono a spazi diversi');
END;

CREATE TRIGGER trg_storico_eventi_attore_membro_insert
BEFORE INSERT ON storico_eventi
WHEN NEW.attore_utente_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.utente_id = NEW.attore_utente_id
      AND ms.spazio_id = NEW.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'attore non membro dello spazio dell evento');
END;

CREATE TRIGGER trg_storico_eventi_attore_membro_update
BEFORE UPDATE OF attore_utente_id, spazio_id ON storico_eventi
WHEN NEW.attore_utente_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM membri_spazio ms
    WHERE ms.utente_id = NEW.attore_utente_id
      AND ms.spazio_id = NEW.spazio_id
)
BEGIN
    SELECT RAISE(ABORT, 'attore non membro dello spazio dell evento');
END;
