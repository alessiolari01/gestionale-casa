-- Ogni prodotto commerciale deve avere il suo formato base anche in
-- `formati_prodotto_alimentare`.
--
-- Quando un prodotto lo crea il bot (`create_product_association`), oltre
-- alla riga in `prodotti_alimentari` viene inserito il formato iniziale nella
-- tabella dei formati. Le due migration che hanno seminato il catalogo
-- (`20260918120000_prodotti_marche_note.sql` e
-- `20260918140000_catalogo_esteso.sql`) inserivano solo la prima: la scheda
-- del prodotto diceva "Formati disponibili: 0 · Nessun formato disponibile"
-- mentre l'elenco e il `📦` della lista mostravano "500 g" letto dal
-- prodotto. Trovato da Alessio nel collaudo del 23 settembre 2026 (punto 3).
--
-- Qui si riallineano i dati: un formato base per ogni prodotto attivo che non
-- ne ha nessuno.

INSERT INTO formati_prodotto_alimentare (
    prodotto_alimentare_id, quantita_confezione, unita_confezione_id, codice_ean
)
SELECT p.id, p.quantita_confezione, p.unita_confezione_id, p.codice_ean
FROM prodotti_alimentari p
WHERE p.attivo = 1
  AND NOT EXISTS (
      SELECT 1 FROM formati_prodotto_alimentare f
      WHERE f.prodotto_alimentare_id = p.id
  );
