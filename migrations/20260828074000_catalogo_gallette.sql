-- Step 7.2H.3 - Completa il catalogo globale con le gallette.
--
-- Migration append-only: non modifica le migration gia' applicate.
-- Le compatibilita' alimentari vengono inizializzate a "verificare": il tipo,
-- la marca e la contaminazione possono cambiare le caratteristiche reali del
-- prodotto commerciale, quindi il catalogo generico non deve inventare
-- certificazioni alimentari o sanitarie.

WITH seed(nome, nome_normalizzato) AS (
    VALUES
        ('🌾 Gallette', 'gallette'),
        ('🌾 Gallette di riso', 'gallette di riso'),
        ('🌾 Gallette di mais', 'gallette di mais')
)
INSERT INTO alimenti (
    spazio_id,
    nome,
    nome_normalizzato,
    descrizione,
    unita_predefinita_id,
    creato_da_utente_id,
    proprietario_utente_id,
    catalogo_globale,
    archiviato
)
SELECT
    NULL,
    seed.nome,
    seed.nome_normalizzato,
    'Alimento base del catalogo globale',
    um.id,
    NULL,
    NULL,
    1,
    0
FROM seed
JOIN unita_misura um ON um.codice = 'g'
WHERE NOT EXISTS (
    SELECT 1
    FROM alimenti a
    WHERE a.catalogo_globale = 1
      AND a.nome_normalizzato = seed.nome_normalizzato
);

-- Il trigger di creazione assegna inizialmente "Altro": per queste voci la
-- categoria corretta e' Cereali e derivati.
DELETE FROM alimento_categorie
WHERE alimento_id IN (
    SELECT id
    FROM alimenti
    WHERE catalogo_globale = 1
      AND nome_normalizzato IN ('gallette', 'gallette di riso', 'gallette di mais')
);

INSERT OR IGNORE INTO alimento_categorie (
    alimento_id,
    categoria_id,
    assegnata_da_utente_id
)
SELECT a.id, c.id, NULL
FROM alimenti a
JOIN categorie_alimento c ON c.codice = 'cereali'
WHERE a.catalogo_globale = 1
  AND a.nome_normalizzato IN ('gallette', 'gallette di riso', 'gallette di mais');

-- Manteniamo completa la matrice di compatibilita' del catalogo globale senza
-- attribuire proprieta' non verificate al prodotto generico.
INSERT OR IGNORE INTO alimento_compatibilita (
    alimento_id,
    etichetta_id,
    stato,
    fonte,
    nota
)
SELECT
    a.id,
    e.id,
    'verificare',
    'catalogo_gallette_2026_08',
    'Verificare etichetta, marca e possibili contaminazioni del prodotto commerciale.'
FROM alimenti a
CROSS JOIN etichette_alimentari e
WHERE a.catalogo_globale = 1
  AND a.nome_normalizzato IN ('gallette', 'gallette di riso', 'gallette di mais');
