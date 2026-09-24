-- Chiudendo la spesa, un'aggiunta dal catalogo di cui si è comprato **meno**
-- di quanto chiedeva non sparisce più in silenzio: resta in lista con quel
-- che manca, e il bot chiede se tenerla ("ne restano 300 g: le lascio in
-- lista?"). Scelta di Alessio il 24 settembre 2026, sul punto 7 del
-- collaudo.
--
-- Questa colonna dice quale chiusura l'ha ridotta: serve al pulsante
-- "🗑 No, toglile", che deve togliere esattamente quelle e nessun'altra.
-- Resta anche dopo la risposta, come traccia di cosa è successo a
-- quell'aggiunta.

ALTER TABLE liste_spesa_aggiunte_catalogo
    ADD COLUMN ridotta_chiusura_id INTEGER
    REFERENCES liste_spesa_chiusure(id) ON DELETE SET NULL;
