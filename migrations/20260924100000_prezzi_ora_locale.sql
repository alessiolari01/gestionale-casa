-- I prezzi già registrati portavano l'ora UTC, che in Italia dopo le 22:00
-- (le 23:00 con l'ora solare) è ancora il giorno prima: un prezzo segnato
-- alle 00:48 del 23 settembre risultava "visto il 22 Set".
--
-- Il codice è stato corretto (`mercato::registra_prezzo` usa 'localtime'),
-- ma le righe già scritte restavano sbagliate: sul database dell'S9 erano
-- tre, tutte con il giorno indietro di uno. Le riporta all'ora locale.
-- Trovato da Alessio nel collaudo del 23 settembre 2026 (punto 5).
--
-- Solo le righe che finiscono per 'Z' (quelle scritte come UTC): se questa
-- migration girasse due volte, la seconda non troverebbe più niente da fare.

UPDATE prezzi_osservati
SET rilevato_il = strftime('%Y-%m-%dT%H:%M:%f', rilevato_il, 'localtime')
WHERE rilevato_il LIKE '%Z';
