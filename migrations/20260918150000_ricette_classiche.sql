-- Ricette classiche nel catalogo globale.
--
-- Alessio ha scelto l'opzione (a) del 18 settembre 2026: ricette con nome,
-- ingredienti e link, **senza** il procedimento di un sito altrui.
--
-- Quindi: ingredienti e passaggi qui dentro sono scritti da me, sono piatti
-- di tradizione spiegati con parole mie, e non vengono da GialloZafferano ne'
-- da altri siti. Il campo `fonte_url` resta vuoto: non ho modo di verificare
-- l'indirizzo di una pagina, e un link morto e' peggio di nessun link --
-- quello lo aggiunge l'utente con "🔗 Fonte della ricetta".
--
-- Sono ricette di base, da correggere a piacere: porzioni e quantita' sono
-- indicative.


INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Spaghetti al pomodoro', 'spaghetti al pomodoro',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 180, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pasta'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 300, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'passata di pomodoro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 1, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'aglio'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 1, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'parmigiano reggiano'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Metti a bollire l''acqua e salala quando bolle.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Scalda l''olio con l''aglio, togli l''aglio quando profuma.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Versa la passata, aggiusta di sale e cuoci 15 minuti a fuoco basso.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 4, 'Cuoci la pasta al dente, scolala e saltala nel sugo.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 4
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 5, 'Servi con il parmigiano, se ti va.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'spaghetti al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 5
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pasta al pesto', 'pasta al pesto',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 180, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pasta'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 90, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pesto'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 1, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'parmigiano reggiano'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Cuoci la pasta in acqua salata.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Tieni da parte mezzo bicchiere di acqua di cottura.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Scola la pasta e mescolala col pesto fuori dal fuoco, allungando con l''acqua tenuta da parte.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 4, 'Completa con il parmigiano.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al pesto'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 4
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Riso al burro e parmigiano', 'riso al burro e parmigiano',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 160, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'riso'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 30, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'burro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 40, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'parmigiano reggiano'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Cuoci il riso in acqua salata.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Scolalo tenendo un po'' di acqua di cottura.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Manteca con burro e parmigiano fuori dal fuoco fino a renderlo cremoso.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'riso al burro e parmigiano'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Frittata', 'frittata',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 4, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'uova'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 1, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'parmigiano reggiano'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 10, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 3, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Sbatti le uova con sale e, se vuoi, il parmigiano.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Scalda l''olio in padella e versa le uova.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Cuoci a fuoco medio finche'' si rapprende, poi girala e finisci l''altro lato.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'frittata'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Insalata di riso', 'insalata di riso',
       'Ricetta di base del catalogo globale', 4, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 320, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'riso'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 160, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'tonno in scatola'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 150, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'mais dolce'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pomodori'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 30, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Cuoci il riso, scolalo e raffreddalo sotto l''acqua.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Taglia i pomodori, scola tonno e mais.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Mescola tutto con olio e sale e lascia in frigo almeno un''ora.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata di riso'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pasta e fagioli', 'pasta e fagioli',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 140, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pasta'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 400, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'fagioli borlotti'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 150, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'passata di pomodoro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 60, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'cipolle dorate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Soffriggi la cipolla tritata nell''olio.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Aggiungi i fagioli scolati e la passata, copri d''acqua e cuoci 15 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Butta la pasta nella pentola e cuocila nel brodo dei fagioli, aggiungendo acqua se serve.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e fagioli'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Petto di pollo alla piastra', 'petto di pollo alla piastra',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'petto di pollo alla piastra'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 300, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'petto di pollo'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'petto di pollo alla piastra'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 15, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'petto di pollo alla piastra'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 4, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'petto di pollo alla piastra'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Scalda bene la piastra.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'petto di pollo alla piastra'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Ungi il pollo con poco olio e salalo.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'petto di pollo alla piastra'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Cuoci 4-5 minuti per lato, finche'' non e'' piu'' rosa all''interno.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'petto di pollo alla piastra'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Zucchine trifolate', 'zucchine trifolate',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 400, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'zucchine'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 1, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'aglio'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 4, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Taglia le zucchine a rondelle.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Scalda l''olio con l''aglio, aggiungi le zucchine e sala.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Cuoci a fuoco vivo 10 minuti, mescolando.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'zucchine trifolate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Purè di patate', 'purè di patate',
       'Ricetta di base del catalogo globale', 4, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 800, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'patate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'latte intero'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 50, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'burro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 40, um.id, 1, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'parmigiano reggiano'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Lessa le patate con la buccia finche'' la forchetta entra senza sforzo.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Sbucciale e schiacciale ancora calde.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Scalda il latte, uniscilo poco per volta con burro, sale e parmigiano.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'purè di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Minestrone', 'minestrone',
       'Ricetta di base del catalogo globale', 4, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 600, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'verdure surgelate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'patate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 6, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 120, um.id, 1, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pasta'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Metti le verdure e le patate a pezzi in pentola, copri d''acqua e sala.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Cuoci 30 minuti a fuoco medio.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Se vuoi, aggiungi la pasta negli ultimi minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 4, 'Completa con un filo d''olio a crudo.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'minestrone'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 4
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pasta al tonno', 'pasta al tonno',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 180, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pasta'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 160, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'tonno in scatola'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'passata di pomodoro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 1, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'aglio'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Scalda l''olio con l''aglio, poi togli l''aglio.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Aggiungi il tonno scolato e la passata, cuoci 10 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Cuoci la pasta al dente e saltala nel sugo.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta al tonno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Uova sode', 'uova sode',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'uova sode'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 4, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'uova'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'uova sode'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 3, um.id, 1, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'uova sode'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Metti le uova in acqua fredda e porta a bollore.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'uova sode'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Da quando bolle conta 9 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'uova sode'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Raffreddale in acqua fredda prima di sbucciarle.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'uova sode'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Insalata mista', 'insalata mista',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'lattuga'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pomodori'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 100, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'carote'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 10, um.id, 1, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'aceto di vino bianco'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 3, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Lava e asciuga la lattuga.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Taglia pomodori e carote.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Condisci con olio, sale e, se ti va, aceto.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'insalata mista'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pasta all''amatriciana', 'pasta all''amatriciana',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 180, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pasta'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 400, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pelati'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 100, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'guanciale'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 15, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 30, um.id, 1, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'parmigiano reggiano'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Rosola il guanciale a listarelle nell''olio finche'' e'' croccante e mettilo da parte.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Nella stessa padella cuoci i pelati schiacciati per 15 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Rimetti il guanciale, cuoci la pasta e saltala nel sugo.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 4, 'Completa con il pecorino, o il parmigiano se non ce l''hai.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta all''amatriciana'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 4
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Vellutata di zucca', 'vellutata di zucca',
       'Ricetta di base del catalogo globale', 4, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 800, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'zucca'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'patate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 80, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'cipolle dorate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 6, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 100, um.id, 1, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'panna da cucina'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Soffriggi la cipolla, aggiungi zucca e patate a pezzi.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Copri d''acqua, sala e cuoci 25 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Frulla tutto; se vuoi, aggiungi la panna.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'vellutata di zucca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pizza margherita in teglia', 'pizza margherita in teglia',
       'Ricetta di base del catalogo globale', 4, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 500, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'farina 00'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 7, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'lievito di birra'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 300, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'passata di pomodoro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 250, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'mozzarella'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 30, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 10, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Sciogli il lievito in 300 ml di acqua tiepida.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Impasta con farina, sale e olio finche'' l''impasto e'' liscio.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Lascia lievitare coperto per almeno 3 ore.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 4, 'Stendi in teglia, condisci con la passata e inforna a 220 gradi per 15 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 4
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 5, 'Aggiungi la mozzarella e finisci la cottura per altri 5 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pizza margherita in teglia'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 5
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Tortilla di patate', 'tortilla di patate',
       'Ricetta di base del catalogo globale', 4, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 600, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'patate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 6, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'uova'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 100, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'cipolle dorate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 40, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 6, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Taglia le patate a fette sottili e la cipolla a velo.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Cuocile in padella con l''olio a fuoco basso finche'' sono morbide.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Sbatti le uova col sale, unisci le patate e rimetti tutto in padella.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 4, 'Cuoci da un lato, gira con un piatto e finisci dall''altro.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'tortilla di patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 4
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pollo al forno con patate', 'pollo al forno con patate',
       'Ricetta di base del catalogo globale', 4, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 1.2, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pollo intero'
JOIN unita_misura um ON um.simbolo = 'kg'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 800, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'patate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 40, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 10, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Taglia le patate a spicchi e condiscile con olio e sale.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Metti il pollo in teglia con le patate intorno.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Inforna a 200 gradi per circa un''ora, girando a meta'' cottura.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pollo al forno con patate'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Caprese', 'caprese',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 300, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pomodori'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 250, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'mozzarella'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 3, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 1, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'basilico'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Taglia pomodori e mozzarella a fette.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Alternali sul piatto.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Condisci con olio, sale e basilico.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'caprese'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pasta e ceci', 'pasta e ceci',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 140, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pasta'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 400, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'ceci'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 100, um.id, 1, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'passata di pomodoro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 1, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'aglio'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Scalda l''olio con l''aglio, aggiungi i ceci scolati.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Frulla meta'' dei ceci con un po'' d''acqua e rimettili in pentola.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Aggiungi acqua calda, porta a bollore e cuoci la pasta dentro.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pasta e ceci'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Bruschette al pomodoro', 'bruschette al pomodoro',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pane'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 300, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'pomodori'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 1, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'aglio'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 3, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Tosta il pane.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Strofina l''aglio sulle fette ancora calde.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Condisci i pomodori a cubetti con olio e sale e mettili sul pane.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'bruschette al pomodoro'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Risotto ai funghi', 'risotto ai funghi',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 160, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'riso arborio'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 250, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'funghi champignon'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 50, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'cipolle dorate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 30, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'burro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 40, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'parmigiano reggiano'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 800, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'brodo vegetale'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Soffriggi la cipolla nel burro, aggiungi i funghi affettati.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Tosta il riso qualche minuto.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Aggiungi il brodo caldo poco per volta, mescolando, per circa 18 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 4, 'Manteca fuori dal fuoco con burro e parmigiano.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'risotto ai funghi'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 4
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Merluzzo al forno', 'merluzzo al forno',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 400, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'merluzzo'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 400, um.id, 1, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'patate'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'olio extravergine di oliva'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 5, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'sale fino'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Metti il merluzzo in teglia con le patate a fette, se le usi.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Condisci con olio e sale.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Inforna a 190 gradi per 25 minuti.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'merluzzo al forno'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Yogurt con frutta e frutta secca', 'yogurt con frutta e frutta secca',
       'Ricetta di base del catalogo globale', 1, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 150, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'yogurt bianco'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 100, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'mele'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'frutta secca'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 10, um.id, 1, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'miele'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Taglia la frutta a pezzi.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Mettila sullo yogurt con la frutta secca.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Aggiungi il miele se ti va.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'yogurt con frutta e frutta secca'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );

INSERT INTO ricette (
    proprietario_utente_id, nome, nome_normalizzato, descrizione,
    porzioni_base, catalogo_globale, archiviata, fonte_nome
)
SELECT NULL, 'Pancake', 'pancake',
       'Ricetta di base del catalogo globale', 2, 1, 0,
       'Catalogo del bot'
WHERE NOT EXISTS (
    SELECT 1 FROM ricette r
    WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
);

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 200, um.id, 0, 0
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'farina 00'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 250, um.id, 0, 1
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'latte intero'
JOIN unita_misura um ON um.simbolo = 'ml'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 2, um.id, 0, 2
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'uova'
JOIN unita_misura um ON um.simbolo = 'pz'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 8, um.id, 0, 3
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'lievito per dolci'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 30, um.id, 0, 4
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'zucchero semolato'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_ingredienti (ricetta_id, alimento_id, quantita, unita_misura_id, opzionale, ordinamento)
SELECT r.id, a.id, 20, um.id, 0, 5
FROM ricette r
JOIN alimenti a ON a.catalogo_globale = 1 AND a.nome_normalizzato = 'burro'
JOIN unita_misura um ON um.simbolo = 'g'
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_ingredienti ri
      WHERE ri.ricetta_id = r.id AND ri.alimento_id = a.id
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 1, 'Mescola farina, lievito e zucchero.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 1
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 2, 'Unisci uova e latte fino a ottenere una pastella liscia.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 2
  );

INSERT INTO ricetta_step (ricetta_id, numero, testo)
SELECT r.id, 3, 'Cuoci un mestolo per volta in padella imburrata, girando quando si formano le bolle.'
FROM ricette r
WHERE r.catalogo_globale = 1 AND r.nome_normalizzato = 'pancake'
  AND NOT EXISTS (
      SELECT 1 FROM ricetta_step s WHERE s.ricetta_id = r.id AND s.numero = 3
  );
