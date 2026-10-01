-- Le scorte che le richieste dal catalogo hanno già usato, in questo giro
-- di spesa (Miglioramento 15, 1 ottobre 2026).
--
-- Un'aggiunta dal catalogo è diventata **netta**: dice quanto c'è da
-- comprare, perché le scorte si guardano una volta sola, quando si aggiunge.
-- Ma due richieste di fila dello stesso alimento non devono contare due
-- volte la stessa scorta: con 200 g di Riso in casa, chiederne 200 g e poi
-- altri 170 deve mettere in lista 170 g, non zero (il caso di Alessio del
-- Miglioramento 16). Qui il bot si ricorda quanta scorta ogni richiesta ha
-- già preso per sé.
--
-- `aggiunta_id` è l'aggiunta che ne è nata, quando la scorta non bastava:
-- ritirando o togliendo l'aggiunta, la scorta che aveva usato si libera da
-- sola. Una richiesta coperta tutta dalle scorte non ha un'aggiunta
-- (`aggiunta_id` nullo), e la sua riga si libera chiudendo la spesa, che è
-- la fine del giro.

CREATE TABLE liste_spesa_scorte_usate (
    id INTEGER PRIMARY KEY,
    lista_id INTEGER NOT NULL
        REFERENCES liste_spesa(id) ON DELETE CASCADE,
    aggiunta_id INTEGER
        REFERENCES liste_spesa_aggiunte_catalogo(id) ON DELETE CASCADE,
    alimento_id INTEGER
        REFERENCES alimenti(id) ON DELETE CASCADE,
    prodotto_alimentare_id INTEGER
        REFERENCES prodotti_alimentari(id) ON DELETE CASCADE,
    quantita REAL NOT NULL CHECK (quantita > 0),
    unita_simbolo TEXT NOT NULL,
    creato_il TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now', 'localtime')),
    CHECK (alimento_id IS NOT NULL OR prodotto_alimentare_id IS NOT NULL)
);

CREATE INDEX idx_liste_spesa_scorte_usate_lista
    ON liste_spesa_scorte_usate (lista_id);
