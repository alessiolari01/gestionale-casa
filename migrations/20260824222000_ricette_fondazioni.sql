-- Step 7.2C - Fondazioni Ricette.
--
-- Obiettivi:
-- - proprieta' della ricetta separata dalla visibilita' negli spazi;
-- - ingredienti referenziati agli alimenti esistenti, senza duplicarli;
-- - quantita' e unita' strutturate per il futuro ridimensionamento delle dosi;
-- - indici predisposti per la ricerca OR per ingredienti richiesti e ranking
--   per numero di ingredienti richiesti effettivamente presenti;
-- - riuso di inviti_risorsa / permessi_risorsa introdotti nella migration 14.

CREATE TABLE ricette (
    id INTEGER PRIMARY KEY,
    proprietario_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    nome TEXT NOT NULL,
    nome_normalizzato TEXT NOT NULL,
    descrizione TEXT,
    procedimento TEXT,
    porzioni_base INTEGER NOT NULL DEFAULT 1 CHECK (porzioni_base > 0),
    tempo_preparazione_minuti INTEGER CHECK (
        tempo_preparazione_minuti IS NULL OR tempo_preparazione_minuti >= 0
    ),
    tempo_cottura_minuti INTEGER CHECK (
        tempo_cottura_minuti IS NULL OR tempo_cottura_minuti >= 0
    ),
    catalogo_globale INTEGER NOT NULL DEFAULT 0 CHECK (catalogo_globale IN (0, 1)),
    archiviata INTEGER NOT NULL DEFAULT 0 CHECK (archiviata IN (0, 1)),
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0),
    CHECK (length(trim(nome_normalizzato)) > 0),
    CHECK (catalogo_globale = 0 OR proprietario_utente_id IS NULL),
    CHECK (catalogo_globale = 1 OR proprietario_utente_id IS NOT NULL)
);

CREATE UNIQUE INDEX idx_ricette_personali_nome
    ON ricette (proprietario_utente_id, nome_normalizzato)
    WHERE catalogo_globale = 0 AND archiviata = 0;

CREATE UNIQUE INDEX idx_ricette_globali_nome
    ON ricette (nome_normalizzato)
    WHERE catalogo_globale = 1 AND archiviata = 0;

CREATE INDEX idx_ricette_proprietario
    ON ricette (proprietario_utente_id, archiviata, nome_normalizzato, id);

CREATE TABLE ricetta_spazi (
    ricetta_id INTEGER NOT NULL REFERENCES ricette(id) ON DELETE CASCADE,
    spazio_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE CASCADE,
    condivisa_da_utente_id INTEGER REFERENCES utenti(id) ON DELETE SET NULL,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (ricetta_id, spazio_id)
);

CREATE INDEX idx_ricetta_spazi_spazio
    ON ricetta_spazi (spazio_id, ricetta_id);

CREATE TABLE ricetta_ingredienti (
    id INTEGER PRIMARY KEY,
    ricetta_id INTEGER NOT NULL REFERENCES ricette(id) ON DELETE CASCADE,
    alimento_id INTEGER NOT NULL REFERENCES alimenti(id) ON DELETE RESTRICT,
    quantita REAL NOT NULL CHECK (quantita > 0),
    unita_misura_id INTEGER NOT NULL REFERENCES unita_misura(id) ON DELETE RESTRICT,
    note TEXT,
    opzionale INTEGER NOT NULL DEFAULT 0 CHECK (opzionale IN (0, 1)),
    ordinamento INTEGER NOT NULL DEFAULT 0,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (ricetta_id, alimento_id)
);

-- Questo indice e' il cuore della futura ricerca per ingredienti:
-- WHERE alimento_id IN (...) -> GROUP BY ricetta_id -> COUNT(DISTINCT alimento_id).
CREATE INDEX idx_ricetta_ingredienti_ricerca
    ON ricetta_ingredienti (alimento_id, ricetta_id);

CREATE INDEX idx_ricetta_ingredienti_ordine
    ON ricetta_ingredienti (ricetta_id, ordinamento, id);

-- Evita due ricette omonime nello stesso spazio condiviso.
CREATE TRIGGER trg_ricetta_spazio_nome_unico_insert
BEFORE INSERT ON ricetta_spazi
WHEN EXISTS (
    SELECT 1
    FROM ricette nuova
    JOIN ricetta_spazi altra_share ON altra_share.spazio_id = NEW.spazio_id
    JOIN ricette altra ON altra.id = altra_share.ricetta_id
    WHERE nuova.id = NEW.ricetta_id
      AND altra.id <> nuova.id
      AND altra.archiviata = 0
      AND altra.nome_normalizzato = nuova.nome_normalizzato
)
BEGIN
    SELECT RAISE(ABORT, 'ricetta con nome duplicato nello spazio');
END;

CREATE TRIGGER trg_ricetta_nome_unico_spazi_update
BEFORE UPDATE OF nome_normalizzato ON ricette
WHEN NEW.archiviata = 0
AND EXISTS (
    SELECT 1
    FROM ricetta_spazi mia_share
    JOIN ricetta_spazi altra_share ON altra_share.spazio_id = mia_share.spazio_id
    JOIN ricette altra ON altra.id = altra_share.ricetta_id
    WHERE mia_share.ricetta_id = NEW.id
      AND altra.id <> NEW.id
      AND altra.archiviata = 0
      AND altra.nome_normalizzato = NEW.nome_normalizzato
)
BEGIN
    SELECT RAISE(ABORT, 'ricetta con nome duplicato in uno spazio condiviso');
END;

-- Solo proprietario o collaboratore con gestione esplicita puo' condividere;
-- inoltre deve poter scrivere nello spazio di destinazione.
CREATE TRIGGER trg_ricetta_spazio_condivisore_insert
BEFORE INSERT ON ricetta_spazi
WHEN NEW.condivisa_da_utente_id IS NOT NULL
AND (
    NOT EXISTS (
        SELECT 1
        FROM membri_spazio ms
        WHERE ms.spazio_id = NEW.spazio_id
          AND ms.utente_id = NEW.condivisa_da_utente_id
          AND ms.ruolo IN ('proprietario', 'amministratore', 'membro')
    )
    OR (
        NOT EXISTS (
            SELECT 1 FROM ricette r
            WHERE r.id = NEW.ricetta_id
              AND r.proprietario_utente_id = NEW.condivisa_da_utente_id
        )
        AND NOT EXISTS (
            SELECT 1
            FROM permessi_risorsa pr
            WHERE pr.tipo_risorsa = 'ricetta'
              AND pr.risorsa_id = NEW.ricetta_id
              AND pr.utente_id = NEW.condivisa_da_utente_id
              AND pr.puo_gestire_permessi = 1
              AND EXISTS (
                  SELECT 1
                  FROM ricetta_spazi rs0
                  JOIN membri_spazio ms0 ON ms0.spazio_id = rs0.spazio_id
                  WHERE rs0.ricetta_id = NEW.ricetta_id
                    AND ms0.utente_id = NEW.condivisa_da_utente_id
              )
        )
    )
)
BEGIN
    SELECT RAISE(ABORT, 'utente non autorizzato a condividere la ricetta');
END;

-- Inviti/permessi generici: specializzazione per tipo_risorsa = ricetta.
CREATE TRIGGER trg_invito_ricetta_valida
BEFORE INSERT ON inviti_risorsa
WHEN NEW.tipo_risorsa = 'ricetta'
AND NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.id = NEW.risorsa_id
      AND r.archiviata = 0
      AND r.catalogo_globale = 0
      AND r.proprietario_utente_id IS NOT NULL
)
BEGIN
    SELECT RAISE(ABORT, 'ricetta non invitabile');
END;

CREATE TRIGGER trg_invito_ricetta_destinatario_visibile
BEFORE INSERT ON inviti_risorsa
WHEN NEW.tipo_risorsa = 'ricetta'
AND NOT EXISTS (
    SELECT 1
    FROM ricetta_spazi rs
    JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id
    WHERE rs.ricetta_id = NEW.risorsa_id
      AND ms.utente_id = NEW.invitato_utente_id
)
BEGIN
    SELECT RAISE(ABORT, 'destinatario senza visibilita ricetta');
END;

CREATE TRIGGER trg_invito_ricetta_autore_autorizzato
BEFORE INSERT ON inviti_risorsa
WHEN NEW.tipo_risorsa = 'ricetta'
AND NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.id = NEW.risorsa_id
      AND r.proprietario_utente_id = NEW.creato_da_utente_id
)
AND NOT EXISTS (
    SELECT 1
    FROM permessi_risorsa pr
    WHERE pr.tipo_risorsa = 'ricetta'
      AND pr.risorsa_id = NEW.risorsa_id
      AND pr.utente_id = NEW.creato_da_utente_id
      AND pr.puo_gestire_permessi = 1
      AND EXISTS (
          SELECT 1
          FROM ricetta_spazi rs
          JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id
          WHERE rs.ricetta_id = NEW.risorsa_id
            AND ms.utente_id = NEW.creato_da_utente_id
      )
)
BEGIN
    SELECT RAISE(ABORT, 'autore invito ricetta non autorizzato');
END;

CREATE TRIGGER trg_permesso_ricetta_destinatario_visibile_insert
BEFORE INSERT ON permessi_risorsa
WHEN NEW.tipo_risorsa = 'ricetta'
AND NOT EXISTS (
    SELECT 1
    FROM ricetta_spazi rs
    JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id
    WHERE rs.ricetta_id = NEW.risorsa_id
      AND ms.utente_id = NEW.utente_id
)
BEGIN
    SELECT RAISE(ABORT, 'utente senza visibilita ricetta');
END;

CREATE TRIGGER trg_permesso_ricetta_concedente_autorizzato_insert
BEFORE INSERT ON permessi_risorsa
WHEN NEW.tipo_risorsa = 'ricetta'
AND NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.id = NEW.risorsa_id
      AND r.proprietario_utente_id = NEW.concesso_da_utente_id
)
AND NOT EXISTS (
    SELECT 1
    FROM permessi_risorsa pr
    WHERE pr.tipo_risorsa = 'ricetta'
      AND pr.risorsa_id = NEW.risorsa_id
      AND pr.utente_id = NEW.concesso_da_utente_id
      AND pr.puo_gestire_permessi = 1
      AND EXISTS (
          SELECT 1
          FROM ricetta_spazi rs
          JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id
          WHERE rs.ricetta_id = NEW.risorsa_id
            AND ms.utente_id = NEW.concesso_da_utente_id
      )
)
BEGIN
    SELECT RAISE(ABORT, 'concedente permesso ricetta non autorizzato');
END;
