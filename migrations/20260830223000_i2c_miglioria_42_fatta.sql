-- Step 7.2I.2C
UPDATE miglioramenti
SET stato = 'fatto',
    fatto_il = COALESCE(fatto_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    verifica_esito = NULL,
    verifica_note = NULL,
    verificato_il = NULL,
    verificato_da_utente_id = NULL,
    aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 42 AND stato = 'da_fare';
