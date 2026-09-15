-- Dispensa, frigo e freezer: via libera di Alessio del 16 settembre 2026,
-- subito dopo la chiusura della spesa
-- (migrations/20260916090000_lista_spesa_chiusura.sql), che e' il punto in
-- cui la merce entra in casa. Specifica e decisioni in
-- docs/previsto/dispensa.md, modulo in docs/moduli/dispensa.md.
--
-- Scelta di struttura presa con Alessio ("scegli tu"): la conservazione e'
-- un'entita' propria, non un riuso di case/stanze/contenitori. Un
-- contenitore dice *dove sta un oggetto in casa*; una scorta dice *in che
-- condizione e' conservata una quantita' di alimento*, ed e' quella
-- condizione a decidere quanto dura. Due frigo in due stanze diverse si
-- comportano allo stesso modo; un freezer e una dispensa nella stessa
-- stanza no.

CREATE TABLE scorte (
    id INTEGER PRIMARY KEY,
    proprietario_utente_id INTEGER NOT NULL
        REFERENCES utenti(id) ON DELETE RESTRICT,
    spazio_id INTEGER
        REFERENCES spazi(id) ON DELETE CASCADE,
    -- I tre posti standard, senza configurazione: chi non definisce niente
    -- ce li ha comunque.
    conservazione TEXT NOT NULL
        CHECK (conservazione IN ('dispensa', 'frigo', 'freezer')),
    -- Stesso schema delle voci della lista della spesa: alimento generico o
    -- prodotto commerciale specifico, entrambi opzionali (una scorta puo'
    -- essere anche testo libero), ON DELETE SET NULL perche' la descrizione
    -- basta a restare leggibile se il catalogo cambia.
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE SET NULL,
    prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE SET NULL,
    descrizione TEXT NOT NULL,
    quantita REAL NOT NULL CHECK (quantita > 0),
    unita_simbolo TEXT NOT NULL,
    -- Opzionale, deciso da Alessio: una scorta nasce senza scadenza e chi
    -- vuole la aggiunge dopo. Un campo che si puo' lasciare vuoto non costa
    -- niente a chi non lo usa.
    scadenza TEXT,
    origine TEXT NOT NULL DEFAULT 'manuale'
        CHECK (origine IN ('spesa', 'manuale')),
    -- Da quale spesa chiusa e' entrata, quando e' entrata da li'.
    chiusura_id INTEGER
        REFERENCES liste_spesa_chiusure(id) ON DELETE SET NULL,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (length(trim(descrizione)) > 0),
    CHECK (scadenza IS NULL OR date(scadenza) IS NOT NULL)
);

CREATE INDEX idx_scorte_spazio_conservazione
    ON scorte (spazio_id, conservazione, id);

CREATE INDEX idx_scorte_alimento
    ON scorte (alimento_id, spazio_id);

-- Servira' agli avvisi di scadenza, quando esistera' l'infrastruttura dei
-- reminder (docs/previsto/reminder.md): oggi la scadenza si legge e basta.
CREATE INDEX idx_scorte_scadenza
    ON scorte (scadenza)
    WHERE scadenza IS NOT NULL;

-- Ingresso automatico della merce in dispensa alla chiusura della spesa:
-- acceso di default, deciso con Alessio. La maggior parte delle volte la
-- quantita' giusta e' quella che si e' comprata, e un passaggio obbligatorio
-- in piu' a fine spesa -- con le borse ancora da svuotare -- e' il modo piu'
-- sicuro per far smettere di usare la funzione. Chi preferisce controllare
-- prima lo spegne, e la merce resta da inserire a mano.
--
-- Preferenza per utente, come vista_spazi: piu' persone usano lo stesso bot
-- e ciascuna decide per se'.
ALTER TABLE preferenze_utente
    ADD COLUMN dispensa_ingresso_automatico INTEGER NOT NULL DEFAULT 1
        CHECK (dispensa_ingresso_automatico IN (0, 1));
