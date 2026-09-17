-- Fonte di una ricetta, chiesta da Alessio il 18 settembre 2026 parlando di
-- GialloZafferano: "esplicita che la ricetta viene da quel sito e fornisci un
-- link sottoforma di pulsante".
--
-- Qui c'e' solo il *meccanismo*: una ricetta puo' dire da dove viene e
-- portare il suo link. Il testo del procedimento resta quello che scrive
-- l'utente: copiare i passaggi di un sito dentro il bot sarebbe copiare
-- materiale di altri, e non lo faccio senza una risposta esplicita (vedi le
-- domande aperte in STATO.md).
--
-- `fonte_nome` e' come si chiama la fonte ("GialloZafferano", "Nonna"),
-- `fonte_url` il link, che il bot mostra come pulsante solo se e' http(s).
ALTER TABLE ricette ADD COLUMN fonte_nome TEXT;
ALTER TABLE ricette ADD COLUMN fonte_url TEXT;
