-- 📄 Documenti (7 ottobre 2026, docs/moduli/documenti.md).
--
-- Una tabella propria e non `items`: `items.tipo` ha un vincolo che non
-- ammette 'documento', e cambiarlo vuol dire ricostruire la tabella di tutti
-- gli oggetti. La posizione usa le stesse case, stanze e contenitori.

-- Le cartelle sono dello spazio, annidabili. Nascono con quelle gia' pronte
-- alla prima apertura della sezione in uno spazio (`documenti_iniziati`):
-- eliminarne una non la fa ricomparire.
CREATE TABLE cartelle_documenti (
    id INTEGER PRIMARY KEY,
    spazio_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE CASCADE,
    padre_id INTEGER REFERENCES cartelle_documenti(id) ON DELETE CASCADE,
    nome TEXT NOT NULL COLLATE NOCASE,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(nome)) > 0),
    CHECK (padre_id IS NULL OR padre_id <> id)
);

CREATE UNIQUE INDEX idx_cartelle_documenti_nome
    ON cartelle_documenti (spazio_id, ifnull(padre_id, 0), nome);

CREATE TABLE documenti_iniziati (
    spazio_id INTEGER PRIMARY KEY REFERENCES spazi(id) ON DELETE CASCADE,
    iniziato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE documenti (
    id INTEGER PRIMARY KEY,
    spazio_id INTEGER NOT NULL REFERENCES spazi(id) ON DELETE CASCADE,
    proprietario_utente_id INTEGER NOT NULL REFERENCES utenti(id) ON DELETE CASCADE,
    -- 1 = "solo mio": nessun altro membro dello spazio lo vede, da nessuna
    -- parte. 0 = dello spazio.
    privato INTEGER NOT NULL DEFAULT 0 CHECK (privato IN (0, 1)),
    titolo TEXT NOT NULL,
    cartella_id INTEGER REFERENCES cartelle_documenti(id) ON DELETE SET NULL,
    numero TEXT,
    ente TEXT,
    -- 'AAAA-MM-GG'.
    rilasciato_il TEXT,
    note TEXT,
    link TEXT,
    abitazione_id INTEGER REFERENCES abitazioni(id) ON DELETE SET NULL,
    stanza_id INTEGER REFERENCES stanze(id) ON DELETE SET NULL,
    contenitore_id INTEGER REFERENCES contenitori(id) ON DELETE SET NULL,
    posizione_dettaglio TEXT,
    creato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(titolo)) > 0),
    CHECK (rilasciato_il IS NULL OR date(rilasciato_il) IS NOT NULL)
);

CREATE INDEX idx_documenti_spazio ON documenti (spazio_id, cartella_id, titolo);

-- La copia digitale: foto e PDF, in data/media/documenti/<documento>/.
CREATE TABLE documenti_file (
    id INTEGER PRIMARY KEY,
    documento_id INTEGER NOT NULL REFERENCES documenti(id) ON DELETE CASCADE,
    tipo TEXT NOT NULL CHECK (tipo IN ('foto', 'pdf')),
    percorso TEXT NOT NULL,
    nome_originale TEXT,
    -- Per rimandarlo senza ricaricarlo; se Telegram non lo riconosce piu',
    -- si manda il file conservato.
    telegram_file_id TEXT,
    caricato_il TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_documenti_file ON documenti_file (documento_id, id);

-- Le scadenze di un documento sono scadenze dei Promemoria.
ALTER TABLE scadenze ADD COLUMN documento_id INTEGER REFERENCES documenti(id) ON DELETE CASCADE;
CREATE INDEX idx_scadenze_documento ON scadenze (documento_id);
