-- Il codice a barre dello Spaghetti n.5 Barilla, scansionato da Alessio nel
-- bot vero (settembre 2026). Dal 6 ottobre 2026 il catalogo comune vive solo
-- nelle migration: il database reale è stato ricostruito pulito, e quello di
-- prova nasce dalle migration, quindi un dato del catalogo che non sta qui
-- andrebbe perso.
--
-- Non sovrascrive un codice già presente, e non lo assegna se un altro
-- prodotto attivo lo usa già (l'indice unico sui codici attivi).
UPDATE prodotti_alimentari
SET codice_ean = '8076800195057',
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE marca = 'Barilla'
  AND nome_commerciale = 'Spaghetti n.5'
  AND creato_da_utente_id IS NULL
  AND codice_ean IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM prodotti_alimentari WHERE codice_ean = '8076800195057'
  );
