-- Step 7.2B - fondazione permessi espliciti per risorse condivise.
--
-- Visibilita' e diritto di modifica sono separati.
-- La coppia (tipo_risorsa, risorsa_id) permette di riusare il modello per
-- alimenti, ricette e future entita' condivisibili.

CREATE TABLE inviti_risorsa (
    id INTEGER PRIMARY KEY,
    tipo_risorsa TEXT NOT NULL CHECK (length(trim(tipo_risorsa)) > 0),
    risorsa_id INTEGER NOT NULL CHECK (risorsa_id > 0),
    invitato_utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    creato_da_utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    puo_modificare INTEGER NOT NULL DEFAULT 1 CHECK (puo_modificare IN (0, 1)),
    puo_gestire_permessi INTEGER NOT NULL DEFAULT 0
        CHECK (puo_gestire_permessi IN (0, 1)),
    stato TEXT NOT NULL DEFAULT 'pendente'
        CHECK (stato IN ('pendente', 'accettato', 'rifiutato', 'revocato')),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    risposto_il TEXT,
    CHECK (puo_gestire_permessi = 0 OR puo_modificare = 1),
    CHECK (invitato_utente_id <> creato_da_utente_id)
);

CREATE UNIQUE INDEX idx_inviti_risorsa_pendente_unico
    ON inviti_risorsa (tipo_risorsa, risorsa_id, invitato_utente_id)
    WHERE stato = 'pendente';

CREATE INDEX idx_inviti_risorsa_invitato
    ON inviti_risorsa (invitato_utente_id, stato, creato_il DESC);

CREATE INDEX idx_inviti_risorsa_risorsa
    ON inviti_risorsa (tipo_risorsa, risorsa_id, stato);

CREATE TABLE permessi_risorsa (
    tipo_risorsa TEXT NOT NULL CHECK (length(trim(tipo_risorsa)) > 0),
    risorsa_id INTEGER NOT NULL CHECK (risorsa_id > 0),
    utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    puo_modificare INTEGER NOT NULL DEFAULT 0 CHECK (puo_modificare IN (0, 1)),
    puo_gestire_permessi INTEGER NOT NULL DEFAULT 0
        CHECK (puo_gestire_permessi IN (0, 1)),
    concesso_da_utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (tipo_risorsa, risorsa_id, utente_id),
    CHECK (puo_gestire_permessi = 0 OR puo_modificare = 1),
    CHECK (utente_id <> concesso_da_utente_id)
);

CREATE INDEX idx_permessi_risorsa_utente
    ON permessi_risorsa (utente_id, tipo_risorsa, risorsa_id);

-- Regole fail-closed del primo tipo operativo: alimento.
CREATE TRIGGER trg_invito_alimento_valido
BEFORE INSERT ON inviti_risorsa
WHEN NEW.tipo_risorsa = 'alimento'
AND NOT EXISTS (
    SELECT 1 FROM alimenti a
    WHERE a.id = NEW.risorsa_id
      AND a.archiviato = 0
      AND a.catalogo_globale = 0
      AND a.proprietario_utente_id IS NOT NULL
)
BEGIN
    SELECT RAISE(ABORT, 'alimento non invitabile');
END;

CREATE TRIGGER trg_invito_alimento_destinatario_visibile
BEFORE INSERT ON inviti_risorsa
WHEN NEW.tipo_risorsa = 'alimento'
AND NOT EXISTS (
    SELECT 1
    FROM alimento_spazi asp
    JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id
    WHERE asp.alimento_id = NEW.risorsa_id
      AND ms.utente_id = NEW.invitato_utente_id
)
BEGIN
    SELECT RAISE(ABORT, 'destinatario senza visibilita alimento');
END;

CREATE TRIGGER trg_invito_alimento_autore_autorizzato
BEFORE INSERT ON inviti_risorsa
WHEN NEW.tipo_risorsa = 'alimento'
AND NOT EXISTS (
    SELECT 1 FROM alimenti a
    WHERE a.id = NEW.risorsa_id
      AND a.proprietario_utente_id = NEW.creato_da_utente_id
)
AND NOT EXISTS (
    SELECT 1
    FROM permessi_risorsa pr
    WHERE pr.tipo_risorsa = 'alimento'
      AND pr.risorsa_id = NEW.risorsa_id
      AND pr.utente_id = NEW.creato_da_utente_id
      AND pr.puo_gestire_permessi = 1
      AND EXISTS (
          SELECT 1
          FROM alimento_spazi asp
          JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id
          WHERE asp.alimento_id = NEW.risorsa_id
            AND ms.utente_id = NEW.creato_da_utente_id
      )
)
BEGIN
    SELECT RAISE(ABORT, 'autore invito alimento non autorizzato');
END;

CREATE TRIGGER trg_permesso_alimento_visibile_insert
BEFORE INSERT ON permessi_risorsa
WHEN NEW.tipo_risorsa = 'alimento'
AND NOT EXISTS (
    SELECT 1
    FROM alimento_spazi asp
    JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id
    WHERE asp.alimento_id = NEW.risorsa_id
      AND ms.utente_id = NEW.utente_id
)
BEGIN
    SELECT RAISE(ABORT, 'utente senza visibilita alimento');
END;

CREATE TRIGGER trg_permesso_alimento_concedente_insert
BEFORE INSERT ON permessi_risorsa
WHEN NEW.tipo_risorsa = 'alimento'
AND NOT EXISTS (
    SELECT 1 FROM alimenti a
    WHERE a.id = NEW.risorsa_id
      AND a.proprietario_utente_id = NEW.concesso_da_utente_id
)
AND NOT EXISTS (
    SELECT 1
    FROM permessi_risorsa pr
    WHERE pr.tipo_risorsa = 'alimento'
      AND pr.risorsa_id = NEW.risorsa_id
      AND pr.utente_id = NEW.concesso_da_utente_id
      AND pr.puo_gestire_permessi = 1
      AND EXISTS (
          SELECT 1
          FROM alimento_spazi asp
          JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id
          WHERE asp.alimento_id = NEW.risorsa_id
            AND ms.utente_id = NEW.concesso_da_utente_id
      )
)
BEGIN
    SELECT RAISE(ABORT, 'concedente permesso alimento non autorizzato');
END;

-- La migration 13 limitava il cambio categoria al proprietario.
-- Ora puo' farlo anche un collaboratore con permesso esplicito di modifica,
-- purche' conservi la visibilita' dell'alimento.
DROP TRIGGER IF EXISTS trg_alimento_categoria_proprietario_insert;

CREATE TRIGGER trg_alimento_categoria_autorizzato_insert
BEFORE INSERT ON alimento_categorie
WHEN NEW.assegnata_da_utente_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM alimenti a
    WHERE a.id = NEW.alimento_id
      AND a.proprietario_utente_id = NEW.assegnata_da_utente_id
)
AND NOT EXISTS (
    SELECT 1
    FROM permessi_risorsa pr
    WHERE pr.tipo_risorsa = 'alimento'
      AND pr.risorsa_id = NEW.alimento_id
      AND pr.utente_id = NEW.assegnata_da_utente_id
      AND pr.puo_modificare = 1
      AND EXISTS (
          SELECT 1
          FROM alimento_spazi asp
          JOIN membri_spazio ms ON ms.spazio_id = asp.spazio_id
          WHERE asp.alimento_id = NEW.alimento_id
            AND ms.utente_id = NEW.assegnata_da_utente_id
      )
)
BEGIN
    SELECT RAISE(ABORT, 'utente non autorizzato a modificare categoria alimento');
END;
