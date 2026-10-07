-- Quando e' stato premuto "✅ Fatto" o "⏰ Rimanda" su un avviso (collaudo
-- di bc29b7b, 7 ottobre 2026): un promemoria del pasto e' sparito per un
-- "Fatto" premuto con ogni probabilita' per sbaglio, e senza l'ora non si
-- poteva dire quando. Ora locale, 'AAAA-MM-GG HH:MM', come inviato_il.
ALTER TABLE promemoria_invii ADD COLUMN esito_il TEXT;
