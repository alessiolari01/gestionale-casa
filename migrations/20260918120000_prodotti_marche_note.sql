-- Prodotti delle marche piu' diffuse in Italia, chiesti da Alessio il 18
-- settembre 2026: "inserisci piu' prodotti possibili delle marche piu' famose
-- in italia (anche se non italiane)".
--
-- COSA C'E' E COSA NON C'E', detto chiaro:
-- - marca, nome e formato vengono dalla mia memoria dei prodotti in
--   commercio, non da un listino ufficiale: un formato puo' essere cambiato
--   o non essere piu' in vendita. Sono modificabili come qualunque prodotto.
-- - NESSUN codice a barre: un EAN inventato sarebbe un dato falso, e il bot
--   lo userebbe per cercare su Open Food Facts. Chi vuole il codice lo
--   aggiunge fotografando la confezione (📦 -> 🏷 Codice a barre), e da li'
--   arrivano anche i dati veri.
-- - NESSUN prezzo: i prezzi cambiano per negozio e per settimana, e si
--   segnano facendo la spesa.
--
-- `verificato = 0` (il default) dice proprio questo: nessuno li ha ancora
-- confermati.
--
-- I prodotti si agganciano agli alimenti del catalogo globale per
-- `nome_normalizzato`: se un alimento non c'e', la sua riga semplicemente non
-- produce nessun prodotto (JOIN), senza far fallire la migration.

WITH seed(alimento, marca, nome_commerciale, quantita, unita) AS (
    VALUES
    -- Pasta
    ('pasta', 'Barilla', 'Spaghetti n.5', 500, 'g'),
    ('pasta', 'Barilla', 'Penne rigate n.73', 500, 'g'),
    ('pasta', 'Barilla', 'Fusilli n.98', 500, 'g'),
    ('pasta', 'Barilla', 'Farfalle n.65', 500, 'g'),
    ('pasta', 'De Cecco', 'Spaghetti n.12', 500, 'g'),
    ('pasta', 'De Cecco', 'Penne rigate n.41', 500, 'g'),
    ('pasta', 'Divella', 'Spaghetti n.8', 500, 'g'),
    ('pasta', 'Rummo', 'Mezzi rigatoni n.51', 500, 'g'),
    ('pasta', 'Garofalo', 'Casarecce', 500, 'g'),
    ('pasta', 'Voiello', 'Spaghettoni n.104', 500, 'g'),
    ('pasta', 'Barilla', 'Spaghetti n.5 formato famiglia', 1, 'kg'),
    -- Riso
    ('riso', 'Scotti', 'Riso Roma', 1, 'kg'),
    ('riso', 'Gallo', 'Riso Carnaroli', 1, 'kg'),
    ('riso arborio', 'Curtiriso', 'Arborio', 1, 'kg'),
    ('riso basmati', 'Scotti', 'Basmati', 500, 'g'),
    -- Farine
    ('farina 00', 'Barilla', 'Farina 00', 1, 'kg'),
    ('farina 00', 'Molino Spadoni', 'Farina 00', 1, 'kg'),
    ('farina manitoba', 'Molino Spadoni', 'Manitoba', 1, 'kg'),
    ('farina di semola', 'De Cecco', 'Semola rimacinata', 1, 'kg'),
    -- Pomodoro e sughi
    ('passata di pomodoro', 'Mutti', 'Passata di pomodoro', 700, 'g'),
    ('passata di pomodoro', 'Cirio', 'Passata rustica', 700, 'g'),
    ('passata di pomodoro', 'Pomi', 'Passata', 700, 'g'),
    ('pelati', 'Mutti', 'Pelati', 400, 'g'),
    ('pelati', 'Cirio', 'Pelati', 400, 'g'),
    ('pesto', 'Barilla', 'Pesto alla genovese', 190, 'g'),
    ('pesto', 'Sacla', 'Pesto alla genovese', 190, 'g'),
    -- Latte e derivati
    ('latte intero', 'Granarolo', 'Latte intero UHT', 1, 'l'),
    ('latte intero', 'Parmalat', 'Latte intero UHT', 1, 'l'),
    ('latte parzialmente scremato', 'Granarolo', 'Latte parzialmente scremato UHT', 1, 'l'),
    ('latte parzialmente scremato', 'Parmalat', 'Zymil senza lattosio', 1, 'l'),
    ('latte scremato', 'Parmalat', 'Latte scremato UHT', 1, 'l'),
    ('panna da cucina', 'Chef', 'Panna da cucina', 200, 'ml'),
    ('panna fresca', 'Granarolo', 'Panna fresca', 250, 'ml'),
    ('burro', 'Santa Lucia', 'Burro', 250, 'g'),
    ('burro', 'Beppino Occelli', 'Burro', 250, 'g'),
    ('yogurt bianco', 'Muller', 'Yogurt bianco cremoso', 500, 'g'),
    ('yogurt bianco', 'Danone', 'Yogurt bianco', 125, 'g'),
    ('yogurt greco', 'Fage', 'Total 0%', 170, 'g'),
    ('mozzarella', 'Santa Lucia', 'Mozzarella', 125, 'g'),
    ('mozzarella di bufala', 'Garofalo', 'Mozzarella di bufala campana', 125, 'g'),
    ('ricotta', 'Santa Lucia', 'Ricotta', 250, 'g'),
    ('formaggio spalmabile', 'Philadelphia', 'Formaggio spalmabile', 150, 'g'),
    ('parmigiano reggiano', 'Parmareggio', 'Parmigiano Reggiano grattugiato', 100, 'g'),
    -- Salumi e carne
    ('prosciutto cotto', 'Rovagnati', 'Gran Biscotto a fette', 100, 'g'),
    ('prosciutto crudo', 'Levoni', 'Crudo di Parma a fette', 100, 'g'),
    ('salsiccia', 'Aia', 'Salsiccia fresca', 300, 'g'),
    ('pollo intero', 'Amadori', 'Pollo intero', 1, 'kg'),
    -- Pesce
    ('tonno in scatola', 'Rio Mare', 'Tonno all''olio d''oliva', 160, 'g'),
    ('tonno in scatola', 'Nostromo', 'Tonno all''olio d''oliva', 160, 'g'),
    ('tonno in scatola', 'Mareblu', 'Tonno all''olio di oliva', 160, 'g'),
    ('merluzzo', 'Findus', 'Bastoncini di merluzzo', 250, 'g'),
    -- Legumi in scatola
    ('fagioli borlotti', 'Valfrutta', 'Borlotti lessati', 400, 'g'),
    ('fagioli cannellini', 'Bonduelle', 'Cannellini', 400, 'g'),
    ('ceci', 'Valfrutta', 'Ceci lessati', 400, 'g'),
    ('lenticchie', 'Bonduelle', 'Lenticchie', 400, 'g'),
    ('mais dolce', 'Bonduelle', 'Mais dolce', 300, 'g'),
    -- Colazione e dolci
    ('cioccolato al latte', 'Milka', 'Cioccolato al latte', 100, 'g'),
    ('cioccolato fondente', 'Lindt', 'Excellence 70%', 100, 'g'),
    ('cioccolato fondente', 'Novi', 'Nero Novi 72%', 100, 'g'),
    ('miele', 'Ambrosoli', 'Miele millefiori', 500, 'g'),
    ('zucchero semolato', 'Eridania', 'Zucchero semolato', 1, 'kg'),
    ('zucchero di canna', 'Eridania', 'Zucchero di canna', 500, 'g'),
    ('fette biscottate', 'Mulino Bianco', 'Fette biscottate', 315, 'g'),
    ('cracker', 'Gran Pavesi', 'Crackers salati', 560, 'g'),
    ('cracker', 'Ritz', 'Crackers', 200, 'g'),
    ('grissini', 'Grissinbon', 'Grissini torinesi', 250, 'g'),
    -- Bevande
    ('acqua', 'San Benedetto', 'Acqua naturale', 1.5, 'l'),
    ('acqua', 'Levissima', 'Acqua naturale', 1.5, 'l'),
    ('acqua frizzante', 'San Pellegrino', 'Acqua frizzante', 1, 'l'),
    ('succo d’arancia', 'Santal', 'Succo d''arancia', 1, 'l'),
    ('succo di mela', 'Yoga', 'Succo di mela', 1, 'l'),
    ('caffè', 'Lavazza', 'Qualita Rossa macinato', 250, 'g'),
    ('caffè', 'Illy', 'Classico macinato', 250, 'g'),
    ('caffè solubile', 'Nescafe', 'Classic solubile', 100, 'g'),
    ('tè nero', 'Twinings', 'The nero filtri', 25, 'pz'),
    ('acqua tonica', 'Schweppes', 'Tonica', 1, 'l'),
    -- Condimenti
    ('olio extravergine di oliva', 'Monini', 'Classico extravergine', 1, 'l'),
    ('olio extravergine di oliva', 'De Cecco', 'Classico extravergine', 1, 'l'),
    ('olio di semi di girasole', 'Cuore', 'Olio di semi di girasole', 1, 'l'),
    ('aceto balsamico', 'Ponti', 'Aceto balsamico di Modena', 250, 'ml'),
    ('aceto di vino bianco', 'Ponti', 'Aceto di vino bianco', 1, 'l'),
    ('ketchup', 'Heinz', 'Tomato Ketchup', 342, 'g'),
    ('maionese', 'Calve', 'Maionese', 225, 'ml'),
    ('dado vegetale', 'Star', 'Dado vegetale', 10, 'pz'),
    ('dado di carne', 'Knorr', 'Dado di carne', 10, 'pz'),
    ('sale fino', 'Italkali', 'Sale fino da tavola', 1, 'kg'),
    -- Uova e pane
    ('uova', 'Coccodi', 'Uova fresche medie', 6, 'pz'),
    ('pane in cassetta', 'Mulino Bianco', 'Pan bauletto bianco', 400, 'g'),
    ('pane in cassetta', 'Bauli', 'Pan bauletto integrale', 400, 'g')
)
INSERT INTO prodotti_alimentari (
    alimento_id, marca, marca_normalizzata, nome_commerciale,
    nome_commerciale_normalizzato, quantita_confezione, unita_confezione_id,
    codice_ean, creato_da_utente_id, verificato, attivo
)
SELECT
    a.id,
    seed.marca,
    lower(seed.marca),
    seed.nome_commerciale,
    lower(seed.nome_commerciale),
    seed.quantita,
    um.id,
    NULL,
    NULL,
    0,
    1
FROM seed
JOIN alimenti a
  ON a.catalogo_globale = 1
 AND a.nome_normalizzato = seed.alimento
JOIN unita_misura um ON um.simbolo = seed.unita
WHERE NOT EXISTS (
    SELECT 1 FROM prodotti_alimentari p
    WHERE p.alimento_id = a.id
      AND p.marca_normalizzata = lower(seed.marca)
      AND p.nome_commerciale_normalizzato = lower(seed.nome_commerciale)
      AND p.quantita_confezione = seed.quantita
      AND p.unita_confezione_id = um.id
);
