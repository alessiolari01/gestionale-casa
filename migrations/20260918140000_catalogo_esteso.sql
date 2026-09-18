-- Catalogo esteso: prodotti di marca in piu', anche non alimentari.
--
-- Chiesto da Alessio il 18 settembre 2026: "vorrei piu' prodotti e formati
-- possibili che vanno in qualsiasi categoria (cibo, lattine, succhi,
-- detersivi e oggetti a consumo abbastanza frequente)".
--
-- Valgono le stesse avvertenze della migration dei primi novanta prodotti:
-- marca, nome e formato vengono dalla memoria del modello e sono tutti
-- modificabili; NESSUN codice a barre inventato (si aggiunge fotografando la
-- confezione) e NESSUN prezzo (si segna facendo la spesa).
--
-- Le voci non alimentari entrano nello stesso catalogo, con due categorie
-- nuove: la lista della spesa e le scorte le gestiscono esattamente come il
-- cibo, e senza questo un detersivo poteva solo essere scritto a mano ogni
-- volta.

INSERT OR IGNORE INTO categorie_alimento (codice, nome, emoji, ordinamento, conservazione_predefinita)
VALUES
    ('casa', 'Casa e pulizia', '🧼', 130, 'dispensa'),
    ('igiene', 'Igiene personale', '🧴', 140, 'dispensa');


WITH seed(nome, nome_normalizzato, unita_codice, categoria_codice) AS (
    VALUES
    ('🥤 Bibita gassata', 'bibita gassata', 'ml', 'bevande'),
    ('🥤 Aranciata', 'aranciata', 'ml', 'bevande'),
    ('🥤 Cola', 'cola', 'ml', 'bevande'),
    ('🥤 Tè freddo', 'tè freddo', 'ml', 'bevande'),
    ('🥤 Energy drink', 'energy drink', 'ml', 'bevande'),
    ('🥤 Birra', 'birra', 'ml', 'bevande'),
    ('🥤 Vino rosso', 'vino rosso', 'ml', 'bevande'),
    ('🥤 Vino bianco', 'vino bianco', 'ml', 'bevande'),
    ('🥤 Latte di soia', 'latte di soia', 'ml', 'bevande'),
    ('🥤 Latte di mandorla', 'latte di mandorla', 'ml', 'bevande'),
    ('🥤 Latte di avena', 'latte di avena', 'ml', 'bevande'),
    ('🍰 Biscotti frollini', 'biscotti frollini', 'g', 'dolci'),
    ('🍰 Biscotti al cioccolato', 'biscotti al cioccolato', 'g', 'dolci'),
    ('🍰 Merendine', 'merendine', 'g', 'dolci'),
    ('🍰 Crema spalmabile alle nocciole', 'crema spalmabile alle nocciole', 'g', 'dolci'),
    ('🍰 Marmellata', 'marmellata', 'g', 'dolci'),
    ('🍰 Cereali da colazione', 'cereali da colazione', 'g', 'cereali'),
    ('🍰 Caramelle', 'caramelle', 'g', 'dolci'),
    ('🍰 Snack salati', 'snack salati', 'g', 'altro'),
    ('🍰 Patatine in busta', 'patatine in busta', 'g', 'altro'),
    ('🍰 Frutta secca', 'frutta secca', 'g', 'altro'),
    ('🌾 Pasta alluovo', 'pasta alluovo', 'g', 'cereali'),
    ('🌾 Gnocchi di patate', 'gnocchi di patate', 'g', 'cereali'),
    ('🌾 Cous cous', 'cous cous', 'g', 'cereali'),
    ('🌾 Pangrattato', 'pangrattato', 'g', 'cereali'),
    ('🌾 Lievito di birra', 'lievito di birra', 'g', 'cereali'),
    ('🌾 Lievito per dolci', 'lievito per dolci', 'g', 'cereali'),
    ('🧊 Pizza surgelata', 'pizza surgelata', 'g', 'cereali'),
    ('🧊 Verdure surgelate', 'verdure surgelate', 'g', 'verdura'),
    ('🧊 Patatine fritte surgelate', 'patatine fritte surgelate', 'g', 'verdura'),
    ('🧊 Gelato', 'gelato', 'ml', 'dolci'),
    ('🧼 Detersivo per piatti', 'detersivo per piatti', 'ml', 'casa'),
    ('🧼 Detersivo per lavastoviglie', 'detersivo per lavastoviglie', 'pz', 'casa'),
    ('🧼 Brillantante', 'brillantante', 'ml', 'casa'),
    ('🧼 Sale per lavastoviglie', 'sale per lavastoviglie', 'kg', 'casa'),
    ('🧼 Detersivo per lavatrice', 'detersivo per lavatrice', 'ml', 'casa'),
    ('🧼 Ammorbidente', 'ammorbidente', 'ml', 'casa'),
    ('🧼 Candeggina', 'candeggina', 'ml', 'casa'),
    ('🧼 Sgrassatore', 'sgrassatore', 'ml', 'casa'),
    ('🧼 Detergente per pavimenti', 'detergente per pavimenti', 'ml', 'casa'),
    ('🧼 Anticalcare', 'anticalcare', 'ml', 'casa'),
    ('🧼 Spugne per piatti', 'spugne per piatti', 'pz', 'casa'),
    ('🧼 Carta casa', 'carta casa', 'pz', 'casa'),
    ('🧼 Carta igienica', 'carta igienica', 'pz', 'casa'),
    ('🧼 Fazzoletti di carta', 'fazzoletti di carta', 'pz', 'casa'),
    ('🧼 Tovaglioli di carta', 'tovaglioli di carta', 'pz', 'casa'),
    ('🧼 Sacchi per la spazzatura', 'sacchi per la spazzatura', 'pz', 'casa'),
    ('🧼 Pellicola per alimenti', 'pellicola per alimenti', 'pz', 'casa'),
    ('🧼 Alluminio per alimenti', 'alluminio per alimenti', 'pz', 'casa'),
    ('🧼 Carta da forno', 'carta da forno', 'pz', 'casa'),
    ('🧼 Profumatore per ambienti', 'profumatore per ambienti', 'ml', 'casa'),
    ('🧼 Lampadine', 'lampadine', 'pz', 'casa'),
    ('🧼 Pile stilo', 'pile stilo', 'pz', 'casa'),
    ('🧴 Dentifricio', 'dentifricio', 'ml', 'igiene'),
    ('🧴 Spazzolino da denti', 'spazzolino da denti', 'pz', 'igiene'),
    ('🧴 Collutorio', 'collutorio', 'ml', 'igiene'),
    ('🧴 Filo interdentale', 'filo interdentale', 'pz', 'igiene'),
    ('🧴 Bagnoschiuma', 'bagnoschiuma', 'ml', 'igiene'),
    ('🧴 Shampoo', 'shampoo', 'ml', 'igiene'),
    ('🧴 Balsamo per capelli', 'balsamo per capelli', 'ml', 'igiene'),
    ('🧴 Sapone per le mani', 'sapone per le mani', 'ml', 'igiene'),
    ('🧴 Deodorante', 'deodorante', 'ml', 'igiene'),
    ('🧴 Schiuma da barba', 'schiuma da barba', 'ml', 'igiene'),
    ('🧴 Rasoi usa e getta', 'rasoi usa e getta', 'pz', 'igiene'),
    ('🧴 Salviette umidificate', 'salviette umidificate', 'pz', 'igiene'),
    ('🧴 Cotton fioc', 'cotton fioc', 'pz', 'igiene'),
    ('🧴 Crema idratante', 'crema idratante', 'ml', 'igiene')
)
INSERT INTO alimenti (
    spazio_id, nome, nome_normalizzato, descrizione, unita_predefinita_id,
    creato_da_utente_id, proprietario_utente_id, catalogo_globale, archiviato
)
SELECT NULL, seed.nome, seed.nome_normalizzato,
       'Voce del catalogo globale', um.id, NULL, NULL, 1, 0
FROM seed
JOIN unita_misura um ON um.codice = seed.unita_codice
WHERE NOT EXISTS (
    SELECT 1 FROM alimenti a
    WHERE a.nome_normalizzato = seed.nome_normalizzato AND a.spazio_id IS NULL
);


WITH seed(nome_normalizzato, categoria_codice) AS (
    VALUES
    ('bibita gassata', 'bevande'),
    ('aranciata', 'bevande'),
    ('cola', 'bevande'),
    ('tè freddo', 'bevande'),
    ('energy drink', 'bevande'),
    ('birra', 'bevande'),
    ('vino rosso', 'bevande'),
    ('vino bianco', 'bevande'),
    ('latte di soia', 'bevande'),
    ('latte di mandorla', 'bevande'),
    ('latte di avena', 'bevande'),
    ('biscotti frollini', 'dolci'),
    ('biscotti al cioccolato', 'dolci'),
    ('merendine', 'dolci'),
    ('crema spalmabile alle nocciole', 'dolci'),
    ('marmellata', 'dolci'),
    ('cereali da colazione', 'cereali'),
    ('caramelle', 'dolci'),
    ('snack salati', 'altro'),
    ('patatine in busta', 'altro'),
    ('frutta secca', 'altro'),
    ('pasta alluovo', 'cereali'),
    ('gnocchi di patate', 'cereali'),
    ('cous cous', 'cereali'),
    ('pangrattato', 'cereali'),
    ('lievito di birra', 'cereali'),
    ('lievito per dolci', 'cereali'),
    ('pizza surgelata', 'cereali'),
    ('verdure surgelate', 'verdura'),
    ('patatine fritte surgelate', 'verdura'),
    ('gelato', 'dolci'),
    ('detersivo per piatti', 'casa'),
    ('detersivo per lavastoviglie', 'casa'),
    ('brillantante', 'casa'),
    ('sale per lavastoviglie', 'casa'),
    ('detersivo per lavatrice', 'casa'),
    ('ammorbidente', 'casa'),
    ('candeggina', 'casa'),
    ('sgrassatore', 'casa'),
    ('detergente per pavimenti', 'casa'),
    ('anticalcare', 'casa'),
    ('spugne per piatti', 'casa'),
    ('carta casa', 'casa'),
    ('carta igienica', 'casa'),
    ('fazzoletti di carta', 'casa'),
    ('tovaglioli di carta', 'casa'),
    ('sacchi per la spazzatura', 'casa'),
    ('pellicola per alimenti', 'casa'),
    ('alluminio per alimenti', 'casa'),
    ('carta da forno', 'casa'),
    ('profumatore per ambienti', 'casa'),
    ('lampadine', 'casa'),
    ('pile stilo', 'casa'),
    ('dentifricio', 'igiene'),
    ('spazzolino da denti', 'igiene'),
    ('collutorio', 'igiene'),
    ('filo interdentale', 'igiene'),
    ('bagnoschiuma', 'igiene'),
    ('shampoo', 'igiene'),
    ('balsamo per capelli', 'igiene'),
    ('sapone per le mani', 'igiene'),
    ('deodorante', 'igiene'),
    ('schiuma da barba', 'igiene'),
    ('rasoi usa e getta', 'igiene'),
    ('salviette umidificate', 'igiene'),
    ('cotton fioc', 'igiene'),
    ('crema idratante', 'igiene')
)
DELETE FROM alimento_categorie
WHERE alimento_id IN (
    SELECT a.id FROM alimenti a JOIN seed ON seed.nome_normalizzato = a.nome_normalizzato
    WHERE a.catalogo_globale = 1
);


WITH seed(nome_normalizzato, categoria_codice) AS (
    VALUES
    ('bibita gassata', 'bevande'),
    ('aranciata', 'bevande'),
    ('cola', 'bevande'),
    ('tè freddo', 'bevande'),
    ('energy drink', 'bevande'),
    ('birra', 'bevande'),
    ('vino rosso', 'bevande'),
    ('vino bianco', 'bevande'),
    ('latte di soia', 'bevande'),
    ('latte di mandorla', 'bevande'),
    ('latte di avena', 'bevande'),
    ('biscotti frollini', 'dolci'),
    ('biscotti al cioccolato', 'dolci'),
    ('merendine', 'dolci'),
    ('crema spalmabile alle nocciole', 'dolci'),
    ('marmellata', 'dolci'),
    ('cereali da colazione', 'cereali'),
    ('caramelle', 'dolci'),
    ('snack salati', 'altro'),
    ('patatine in busta', 'altro'),
    ('frutta secca', 'altro'),
    ('pasta alluovo', 'cereali'),
    ('gnocchi di patate', 'cereali'),
    ('cous cous', 'cereali'),
    ('pangrattato', 'cereali'),
    ('lievito di birra', 'cereali'),
    ('lievito per dolci', 'cereali'),
    ('pizza surgelata', 'cereali'),
    ('verdure surgelate', 'verdura'),
    ('patatine fritte surgelate', 'verdura'),
    ('gelato', 'dolci'),
    ('detersivo per piatti', 'casa'),
    ('detersivo per lavastoviglie', 'casa'),
    ('brillantante', 'casa'),
    ('sale per lavastoviglie', 'casa'),
    ('detersivo per lavatrice', 'casa'),
    ('ammorbidente', 'casa'),
    ('candeggina', 'casa'),
    ('sgrassatore', 'casa'),
    ('detergente per pavimenti', 'casa'),
    ('anticalcare', 'casa'),
    ('spugne per piatti', 'casa'),
    ('carta casa', 'casa'),
    ('carta igienica', 'casa'),
    ('fazzoletti di carta', 'casa'),
    ('tovaglioli di carta', 'casa'),
    ('sacchi per la spazzatura', 'casa'),
    ('pellicola per alimenti', 'casa'),
    ('alluminio per alimenti', 'casa'),
    ('carta da forno', 'casa'),
    ('profumatore per ambienti', 'casa'),
    ('lampadine', 'casa'),
    ('pile stilo', 'casa'),
    ('dentifricio', 'igiene'),
    ('spazzolino da denti', 'igiene'),
    ('collutorio', 'igiene'),
    ('filo interdentale', 'igiene'),
    ('bagnoschiuma', 'igiene'),
    ('shampoo', 'igiene'),
    ('balsamo per capelli', 'igiene'),
    ('sapone per le mani', 'igiene'),
    ('deodorante', 'igiene'),
    ('schiuma da barba', 'igiene'),
    ('rasoi usa e getta', 'igiene'),
    ('salviette umidificate', 'igiene'),
    ('cotton fioc', 'igiene'),
    ('crema idratante', 'igiene')
)
INSERT INTO alimento_categorie (alimento_id, categoria_id, assegnata_da_utente_id)
SELECT a.id, c.id, NULL
FROM seed
JOIN alimenti a ON a.nome_normalizzato = seed.nome_normalizzato AND a.catalogo_globale = 1
JOIN categorie_alimento c ON c.codice = seed.categoria_codice;


WITH seed(alimento, marca, nome_commerciale, quantita, unita) AS (
    VALUES
    ('cola', 'Coca-Cola', 'Original Taste lattina', 330, 'ml'),
    ('cola', 'Coca-Cola', 'Zero lattina', 330, 'ml'),
    ('cola', 'Coca-Cola', 'Original Taste bottiglia', 1.5, 'l'),
    ('cola', 'Pepsi', 'Cola lattina', 330, 'ml'),
    ('aranciata', 'Fanta', 'Aranciata lattina', 330, 'ml'),
    ('aranciata', 'San Pellegrino', 'Aranciata lattina', 330, 'ml'),
    ('aranciata', 'Schweppes', 'Aranciata amara lattina', 330, 'ml'),
    ('bibita gassata', 'Sprite', 'Lattina', 330, 'ml'),
    ('bibita gassata', 'Chinotto', 'Lurisia lattina', 275, 'ml'),
    ('te freddo', 'Estathé', 'Limone brick', 200, 'ml'),
    ('te freddo', 'Estathé', 'Pesca bottiglia', 1.5, 'l'),
    ('te freddo', 'Lipton', 'Ice Tea limone', 1.5, 'l'),
    ('energy drink', 'Red Bull', 'Energy drink lattina', 250, 'ml'),
    ('energy drink', 'Monster', 'Energy lattina', 500, 'ml'),
    ('birra', 'Moretti', 'Birra lattina', 330, 'ml'),
    ('birra', 'Peroni', 'Nastro Azzurro bottiglia', 330, 'ml'),
    ('birra', 'Ichnusa', 'Non filtrata bottiglia', 330, 'ml'),
    ('birra', 'Heineken', 'Lattina', 330, 'ml'),
    ('vino rosso', 'Tavernello', 'Rosso brick', 1, 'l'),
    ('vino bianco', 'Tavernello', 'Bianco brick', 1, 'l'),
    ('latte di soia', 'Alpro', 'Soia senza zuccheri', 1, 'l'),
    ('latte di mandorla', 'Alpro', 'Mandorla senza zuccheri', 1, 'l'),
    ('latte di avena', 'Alpro', 'Avena', 1, 'l'),
    ('acqua', 'SantAnna', 'Naturale', 1.5, 'l'),
    ('acqua', 'Ferrarelle', 'Effervescente naturale', 1.5, 'l'),
    ('acqua frizzante', 'Levissima', 'Frizzante', 1.5, 'l'),
    ('succo di mela', 'Santal', 'Mela brick', 200, 'ml'),
    ('succo di ananas', 'Yoga', 'Ananas brick', 200, 'ml'),
    ('biscotti frollini', 'Mulino Bianco', 'Macine', 350, 'g'),
    ('biscotti frollini', 'Mulino Bianco', 'Abbracci', 350, 'g'),
    ('biscotti frollini', 'Gran Cereale', 'Croccante', 500, 'g'),
    ('biscotti frollini', 'Oro Saiwa', 'Biscotti', 500, 'g'),
    ('biscotti al cioccolato', 'Pan di Stelle', 'Biscotti', 350, 'g'),
    ('biscotti al cioccolato', 'Oreo', 'Biscotti', 154, 'g'),
    ('merendine', 'Kinder', 'Brioss', 280, 'g'),
    ('merendine', 'Mulino Bianco', 'Sofficini alla albicocca', 280, 'g'),
    ('merendine', 'Ferrero', 'Kinder Cereali', 200, 'g'),
    ('crema spalmabile alle nocciole', 'Nutella', 'Crema spalmabile', 400, 'g'),
    ('crema spalmabile alle nocciole', 'Nutella', 'Crema spalmabile grande', 750, 'g'),
    ('crema spalmabile alle nocciole', 'Novi', 'Nocciolata', 350, 'g'),
    ('marmellata', 'Rigoni di Asiago', 'Fiordifrutta albicocca', 250, 'g'),
    ('marmellata', 'Santa Rosa', 'Confettura albicocche', 350, 'g'),
    ('cereali da colazione', 'Kelloggs', 'Corn Flakes', 375, 'g'),
    ('cereali da colazione', 'Kelloggs', 'Special K', 375, 'g'),
    ('cereali da colazione', 'Nestlé', 'Cheerios', 375, 'g'),
    ('caramelle', 'Haribo', 'Orsetti doro', 200, 'g'),
    ('caramelle', 'Golia', 'Bianca', 180, 'g'),
    ('snack salati', 'Saiwa', 'Tuc originale', 100, 'g'),
    ('patatine in busta', 'San Carlo', 'Classica', 180, 'g'),
    ('patatine in busta', 'Pringles', 'Original', 175, 'g'),
    ('patatine in busta', 'Amica Chips', 'Patatine classiche', 150, 'g'),
    ('frutta secca', 'Noberasco', 'Mandorle', 150, 'g'),
    ('frutta secca', 'Noberasco', 'Noci sgusciate', 150, 'g'),
    ('cioccolato al latte', 'Kinder', 'Cioccolato', 100, 'g'),
    ('cioccolato al latte', 'Novi', 'Latte', 100, 'g'),
    ('cioccolato bianco', 'Milka', 'Bianco', 100, 'g'),
    ('pasta alluovo', 'Barilla', 'Tagliatelle alluovo', 250, 'g'),
    ('pasta alluovo', 'Rana', 'Tagliatelle fresche', 250, 'g'),
    ('gnocchi di patate', 'Rana', 'Gnocchi di patate', 500, 'g'),
    ('cous cous', 'Tipiak', 'Cous cous', 500, 'g'),
    ('pangrattato', 'Mulino Bianco', 'Pangrattato', 400, 'g'),
    ('lievito di birra', 'Mastro Fornaio', 'Lievito di birra secco', 21, 'g'),
    ('lievito per dolci', 'Paneangeli', 'Lievito vanigliato', 16, 'g'),
    ('pasta', 'Barilla', 'Rigatoni n.89', 500, 'g'),
    ('pasta', 'Barilla', 'Tortiglioni n.83', 500, 'g'),
    ('pasta', 'La Molisana', 'Spaghetti n.15', 500, 'g'),
    ('riso', 'Scotti', 'Riso per risotti', 1, 'kg'),
    ('farina 00', 'Caputo', 'Farina 00 pizzeria', 1, 'kg'),
    ('pizza surgelata', 'Buitoni', 'Margherita', 320, 'g'),
    ('pizza surgelata', 'Cameo', 'Ristorante margherita', 340, 'g'),
    ('verdure surgelate', 'Findus', 'Minestrone', 600, 'g'),
    ('verdure surgelate', 'Orogel', 'Spinaci foglia', 450, 'g'),
    ('patatine fritte surgelate', 'McCain', 'Patatine classiche', 750, 'g'),
    ('gelato', 'Algida', 'Cornetto classico', 4, 'pz'),
    ('gelato', 'Sammontana', 'Vaschetta crema e cioccolato', 500, 'ml'),
    ('merluzzo', 'Findus', 'Filetti di merluzzo', 400, 'g'),
    ('yogurt bianco', 'Granarolo', 'Yogurt bianco intero', 500, 'g'),
    ('yogurt greco', 'Muller', 'Greco naturale', 150, 'g'),
    ('mozzarella', 'Galbani', 'Mozzarella Santa Lucia', 125, 'g'),
    ('formaggio spalmabile', 'Galbani', 'Certosa', 200, 'g'),
    ('parmigiano reggiano', 'Parmareggio', 'Parmigiano 24 mesi', 200, 'g'),
    ('burro', 'Granarolo', 'Burro', 250, 'g'),
    ('panna fresca', 'Parmalat', 'Panna fresca', 200, 'ml'),
    ('uova', 'Le Naturelle', 'Uova fresche allevate a terra', 6, 'pz'),
    ('prosciutto cotto', 'Aia', 'Prosciutto cotto a fette', 120, 'g'),
    ('salsiccia', 'Beretta', 'Salsiccia fresca', 300, 'g'),
    ('pollo intero', 'Aia', 'Pollo intero', 1.2, 'kg'),
    ('passata di pomodoro', 'Mutti', 'Passata bottiglia grande', 1, 'kg'),
    ('pelati', 'Petti', 'Pomodori pelati', 400, 'g'),
    ('pesto', 'Barilla', 'Pesto rosso', 190, 'g'),
    ('maionese', 'Kraft', 'Maionese', 310, 'ml'),
    ('ketchup', 'Calve', 'Ketchup', 300, 'ml'),
    ('olio extravergine di oliva', 'Bertolli', 'Extravergine classico', 1, 'l'),
    ('aceto balsamico', 'Monari Federzoni', 'Aceto balsamico', 500, 'ml'),
    ('tonno in scatola', 'Rio Mare', 'Tonno al naturale', 160, 'g'),
    ('tonno in scatola', 'Callipo', 'Tonno allolio doliva', 160, 'g'),
    ('mais dolce', 'Valfrutta', 'Mais dolce', 300, 'g'),
    ('ceci', 'Bonduelle', 'Ceci', 400, 'g'),
    ('dado vegetale', 'Knorr', 'Dado vegetale', 10, 'pz'),
    ('miele', 'Rigoni di Asiago', 'Mielbio millefiori', 300, 'g'),
    ('caffè', 'Lavazza', 'Crema e Gusto macinato', 250, 'g'),
    ('caffè', 'Kimbo', 'Napoletano macinato', 250, 'g'),
    ('caffè solubile', 'Nescafé', 'Gold solubile', 200, 'g'),
    ('tè nero', 'Twinings', 'English Breakfast filtri', 25, 'pz'),
    ('tè verde', 'Star', 'Tè verde filtri', 20, 'pz'),
    ('detersivo per piatti', 'Svelto', 'Limone', 900, 'ml'),
    ('detersivo per piatti', 'Nelsen', 'Limone', 900, 'ml'),
    ('detersivo per piatti', 'Fairy', 'Ultra limone', 650, 'ml'),
    ('detersivo per lavastoviglie', 'Finish', 'Powerball All in 1', 40, 'pz'),
    ('detersivo per lavastoviglie', 'Fairy', 'Platinum caps', 30, 'pz'),
    ('brillantante', 'Finish', 'Brillantante', 800, 'ml'),
    ('sale per lavastoviglie', 'Finish', 'Sale rigenerante', 1, 'kg'),
    ('detersivo per lavatrice', 'Dash', 'Classico liquido', 2.1, 'l'),
    ('detersivo per lavatrice', 'Dixan', 'Classico liquido', 1.9, 'l'),
    ('detersivo per lavatrice', 'Bio Presto', 'Classico polvere', 2.6, 'kg'),
    ('ammorbidente', 'Coccolino', 'Blu', 1.4, 'l'),
    ('ammorbidente', 'Vernel', 'Classico', 1.4, 'l'),
    ('candeggina', 'Ace', 'Candeggina classica', 1, 'l'),
    ('sgrassatore', 'Chanteclair', 'Sgrassatore marsiglia', 600, 'ml'),
    ('sgrassatore', 'Cif', 'Sgrassatore con candeggina', 650, 'ml'),
    ('detergente per pavimenti', 'Lysoform', 'Pavimenti', 1.25, 'l'),
    ('detergente per pavimenti', 'Chanteclair', 'Pavimenti marsiglia', 1, 'l'),
    ('anticalcare', 'Viakal', 'Anticalcare spray', 700, 'ml'),
    ('spugne per piatti', 'Spontex', 'Spugne abrasive', 4, 'pz'),
    ('carta casa', 'Scottex', 'Rotoloni', 4, 'pz'),
    ('carta casa', 'Foxy', 'Asciugatutto', 2, 'pz'),
    ('carta igienica', 'Scottex', 'Carta igienica', 12, 'pz'),
    ('carta igienica', 'Foxy', 'Mille veli', 10, 'pz'),
    ('fazzoletti di carta', 'Tempo', 'Fazzoletti classici', 10, 'pz'),
    ('tovaglioli di carta', 'Foxy', 'Tovaglioli', 100, 'pz'),
    ('sacchi per la spazzatura', 'Domopak', 'Sacchi 50 litri', 20, 'pz'),
    ('sacchi per la spazzatura', 'Cuki', 'Sacchi con maniglie', 20, 'pz'),
    ('pellicola per alimenti', 'Cuki', 'Pellicola', 1, 'pz'),
    ('alluminio per alimenti', 'Cuki', 'Alluminio', 1, 'pz'),
    ('carta da forno', 'Cuki', 'Carta da forno', 1, 'pz'),
    ('profumatore per ambienti', 'Glade', 'Spray ambiente', 300, 'ml'),
    ('lampadine', 'Osram', 'LED E27 9W', 1, 'pz'),
    ('pile stilo', 'Duracell', 'Stilo AA', 4, 'pz'),
    ('dentifricio', 'Mentadent', 'Dentifricio', 75, 'ml'),
    ('dentifricio', 'Colgate', 'Total', 75, 'ml'),
    ('dentifricio', 'Sensodyne', 'Sensitivo', 75, 'ml'),
    ('spazzolino da denti', 'Oral-B', 'Spazzolino medio', 1, 'pz'),
    ('collutorio', 'Listerine', 'Collutorio', 500, 'ml'),
    ('filo interdentale', 'Oral-B', 'Filo interdentale', 1, 'pz'),
    ('bagnoschiuma', 'Nivea', 'Bagnoschiuma', 750, 'ml'),
    ('bagnoschiuma', 'Dove', 'Bagnoschiuma idratante', 700, 'ml'),
    ('shampoo', 'Pantene', 'Shampoo', 250, 'ml'),
    ('shampoo', 'Head & Shoulders', 'Antiforfora', 250, 'ml'),
    ('balsamo per capelli', 'Pantene', 'Balsamo', 200, 'ml'),
    ('sapone per le mani', 'Dove', 'Sapone liquido', 250, 'ml'),
    ('deodorante', 'Nivea', 'Deodorante spray', 150, 'ml'),
    ('deodorante', 'Borotalco', 'Deodorante spray', 150, 'ml'),
    ('schiuma da barba', 'Gillette', 'Schiuma da barba', 200, 'ml'),
    ('rasoi usa e getta', 'Gillette', 'Blue II', 5, 'pz'),
    ('salviette umidificate', 'Chicco', 'Salviette', 72, 'pz'),
    ('cotton fioc', 'Johnsons', 'Cotton fioc', 200, 'pz'),
    ('crema idratante', 'Nivea', 'Crema Soft', 200, 'ml')
)
INSERT INTO prodotti_alimentari (
    alimento_id, marca, marca_normalizzata, nome_commerciale,
    nome_commerciale_normalizzato, quantita_confezione, unita_confezione_id,
    codice_ean, creato_da_utente_id, verificato, attivo
)
SELECT a.id, seed.marca, lower(seed.marca), seed.nome_commerciale,
       lower(seed.nome_commerciale), seed.quantita, um.id, NULL, NULL, 0, 1
FROM seed
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = seed.alimento
JOIN unita_misura um ON um.simbolo = seed.unita
WHERE NOT EXISTS (
    SELECT 1 FROM prodotti_alimentari p
    WHERE p.alimento_id = a.id
      AND p.marca_normalizzata = lower(seed.marca)
      AND p.nome_commerciale_normalizzato = lower(seed.nome_commerciale)
      AND p.quantita_confezione = seed.quantita
      AND p.unita_confezione_id = um.id
);

-- Ogni voce del catalogo deve avere tutte le etichette alimentari: il resto
-- del bot conta su questo (un test lo verifica). Per le voci nuove lo stato
-- parte da 'verificare': nessuno le ha ancora guardate una per una, e per la
-- roba che non si mangia la domanda non ha nemmeno senso -- la nota lo dice.
INSERT INTO alimento_compatibilita (alimento_id, etichetta_id, stato, fonte, nota)
SELECT a.id, e.id, 'verificare', 'catalogo',
       CASE WHEN EXISTS (
           SELECT 1 FROM alimento_categorie ac
           JOIN categorie_alimento c ON c.id = ac.categoria_id
           WHERE ac.alimento_id = a.id AND c.codice IN ('casa', 'igiene')
       ) THEN 'Non e'' un alimento' ELSE NULL END
FROM alimenti a
CROSS JOIN etichette_alimentari e
WHERE a.catalogo_globale = 1
  AND NOT EXISTS (
      SELECT 1 FROM alimento_compatibilita ac
      WHERE ac.alimento_id = a.id AND ac.etichetta_id = e.id
  );
