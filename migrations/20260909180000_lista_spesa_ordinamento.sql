-- Lista della spesa: ordine stabile e riordino manuale, deciso con Alessio
-- il 9 settembre 2026 dopo un collaudo dal vivo. Prima l'ordine dipendeva da
-- `comprato` (le voci comprate saltavano in fondo alla lista quando venivano
-- spuntate): ora l'ordine e' indipendente dallo stato comprato, e l'utente
-- puo' spostare una voce su/giu' a piacere ("↕️ Riordina lista").
ALTER TABLE liste_spesa_voci ADD COLUMN ordinamento INTEGER NOT NULL DEFAULT 0;

-- Le voci esistenti mantengono l'ordine che avevano di fatto (per id).
UPDATE liste_spesa_voci SET ordinamento = id;

CREATE INDEX idx_lista_spesa_voci_ordinamento
    ON liste_spesa_voci (lista_id, ordinamento, id);
