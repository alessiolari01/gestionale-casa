-- Step 7.1B - Vista multi-spazio e fondazioni di condivisione trasversale.
--
-- Lo spazio attivo resta lo spazio PREDEFINITO di creazione, ma l'utente puo'
-- scegliere di visualizzare contemporaneamente tutti gli spazi di cui e' membro.
-- Gli item mantengono il proprio spazio proprietario anche quando sono collocati
-- fisicamente in un'abitazione appartenente a un altro spazio accessibile.

ALTER TABLE preferenze_utente
ADD COLUMN vista_spazi TEXT NOT NULL DEFAULT 'predefinito'
    CHECK (vista_spazi IN ('predefinito', 'tutti'));

CREATE TABLE item_condivisioni (
    item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    spazio_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE CASCADE,
    permesso TEXT NOT NULL DEFAULT 'lettura'
        CHECK (permesso IN ('lettura', 'modifica')),
    condiviso_da_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (item_id, spazio_id)
);

CREATE INDEX idx_item_condivisioni_spazio
    ON item_condivisioni (spazio_id, permesso, item_id);

-- I trigger Step 7.1A vietavano qualsiasi posizione cross-space. Da Step 7.1B
-- la proprieta' dell'item e la sua posizione fisica sono concetti distinti.
-- I permessi di accesso ai due spazi vengono verificati dal livello applicativo.
DROP TRIGGER IF EXISTS trg_item_luogo_spazio_insert;
DROP TRIGGER IF EXISTS trg_item_luogo_spazio_update;
