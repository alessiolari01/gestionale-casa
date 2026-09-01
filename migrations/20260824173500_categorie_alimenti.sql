-- Step 7.2B - struttura categorie alimentari.
--
-- Le categorie sono globali ed estendibili.
-- alimento_categorie e' molti-a-molti per permettere in futuro
-- piu' categorie per alimento senza cambiare schema.

CREATE TABLE categorie_alimento (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codice TEXT NOT NULL UNIQUE
        CHECK (length(trim(codice)) > 0),
    nome TEXT NOT NULL
        CHECK (length(trim(nome)) > 0),
    emoji TEXT NOT NULL DEFAULT '🏷️',
    attiva INTEGER NOT NULL DEFAULT 1
        CHECK (attiva IN (0, 1)),
    ordinamento INTEGER NOT NULL DEFAULT 0,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO categorie_alimento (codice, nome, emoji, ordinamento) VALUES
    ('verdura',     'Verdure',              '🥬', 10),
    ('frutta',      'Frutta',               '🍎', 20),
    ('carne',       'Carne',                '🥩', 30),
    ('pesce',       'Pesce',                '🐟', 40),
    ('latticini',   'Latticini',            '🥛', 50),
    ('uova',        'Uova',                 '🥚', 60),
    ('cereali',     'Cereali e derivati',   '🌾', 70),
    ('legumi',      'Legumi',               '🫘', 80),
    ('condimenti',  'Condimenti e salse',   '🧂', 90),
    ('bevande',     'Bevande',              '🥤', 100),
    ('dolci',       'Dolci',                '🍰', 110),
    ('altro',       'Altro',                '🏷️', 999);

CREATE TABLE alimento_categorie (
    alimento_id INTEGER NOT NULL
        REFERENCES alimenti(id) ON DELETE CASCADE,
    categoria_id INTEGER NOT NULL
        REFERENCES categorie_alimento(id) ON DELETE RESTRICT,
    assegnata_da_utente_id INTEGER
        REFERENCES utenti(id) ON DELETE SET NULL,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (alimento_id, categoria_id)
);

CREATE INDEX idx_alimento_categorie_categoria
    ON alimento_categorie (categoria_id, alimento_id);

CREATE TRIGGER trg_alimento_categoria_proprietario_insert
BEFORE INSERT ON alimento_categorie
WHEN NEW.assegnata_da_utente_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1
    FROM alimenti a
    WHERE a.id = NEW.alimento_id
      AND a.proprietario_utente_id = NEW.assegnata_da_utente_id
)
BEGIN
    SELECT RAISE(
        ABORT,
        'solo il proprietario puo assegnare la categoria alimento'
    );
END;

-- Backfill: tutti gli alimenti gia' presenti partono da Altro.
INSERT OR IGNORE INTO alimento_categorie (
    alimento_id,
    categoria_id,
    assegnata_da_utente_id
)
SELECT
    a.id,
    c.id,
    a.proprietario_utente_id
FROM alimenti a
JOIN categorie_alimento c ON c.codice = 'altro';

-- Anche ogni nuovo alimento parte automaticamente da Altro.
CREATE TRIGGER trg_alimenti_categoria_default_insert
AFTER INSERT ON alimenti
BEGIN
    INSERT OR IGNORE INTO alimento_categorie (
        alimento_id,
        categoria_id,
        assegnata_da_utente_id
    )
    SELECT
        NEW.id,
        c.id,
        NEW.proprietario_utente_id
    FROM categorie_alimento c
    WHERE c.codice = 'altro'
      AND c.attiva = 1;
END;
