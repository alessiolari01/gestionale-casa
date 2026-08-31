-- Step 7.2I.2A — rifiniture ingredienti + export miglioramenti.
-- Segna come implementati, ma ancora da verificare, i miglioramenti #39, #40 e #41.

UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = COALESCE(fatto_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (39, 40, 41)
  AND stato = 'da_fare';
