-- Unita' predefinite sbagliate su sei alimenti del catalogo.
--
-- Il latte e la panna si comprano a millilitri (una bottiglia da 1 l, una
-- confezione da 200 ml), ma il catalogo li proponeva in grammi perche' stanno
-- nella categoria dei latticini, che per il resto si pesa. Il miele, al
-- contrario, si compra a peso (un vasetto da 500 g) e il catalogo lo
-- proponeva in millilitri perche' sta con gli oli.
--
-- Trovato da Alessio nel collaudo del 24 settembre 2026: scrivendo "1,5 l" di
-- latte la lista rispondeva in grammi.
--
-- Solo gli alimenti del catalogo globale e solo se l'unita' e' ancora quella
-- sbagliata: un alimento che qualcuno ha gia' corretto a mano resta com'e'.

UPDATE alimenti
SET unita_predefinita_id = (SELECT id FROM unita_misura WHERE simbolo = 'ml')
WHERE catalogo_globale = 1
  AND nome_normalizzato IN (
      'latte intero', 'latte parzialmente scremato', 'latte scremato',
      'panna fresca', 'panna da cucina'
  )
  AND unita_predefinita_id = (SELECT id FROM unita_misura WHERE simbolo = 'g');

UPDATE alimenti
SET unita_predefinita_id = (SELECT id FROM unita_misura WHERE simbolo = 'g')
WHERE catalogo_globale = 1
  AND nome_normalizzato = 'miele'
  AND unita_predefinita_id = (SELECT id FROM unita_misura WHERE simbolo = 'ml');
