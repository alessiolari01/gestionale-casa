# Step 7.2C — Fondazioni Ricette

## Obiettivo

Introdurre il modello dati delle ricette senza duplicare alimenti e senza
rompere il modello di proprietà/condivisione consolidato nello Step 7.2B.

## Modello

Una ricetta:

- appartiene a un proprietario;
- può essere personale o visibile in più spazi;
- resta un solo record centrale;
- usa gli alimenti esistenti come ingredienti;
- riusa i permessi trasversali della migration 14.

## Ricerca predisposta

La funzione backend `ricette::search_by_ingredients` riceve più `alimento_id`
e applica semantica OR. Restituisce solo ricette visibili e ordina:

1. numero di ingredienti richiesti presenti, decrescente;
2. nome ricetta;
3. ID interno come tie-break tecnico, mai mostrato in UI.

## UI polish incluso nello stesso checkpoint

Questo checkpoint chiude anche alcune correzioni visuali già concordate:

- gli ID tecnici non vengono più mostrati nelle UI di alimenti, oggetti,
  contenitori, case, stanze e storico;
- callback e database continuano a usare gli ID internamente;
- nell'Alimentazione vengono usati gli accenti italiani corretti;
- l'unità viene mostrata in forma descrittiva, ad esempio
  `grammi (g)`, `chilogrammi (kg)`, `millilitri (ml)`;
- la stessa forma descrittiva viene usata nei pulsanti di scelta unità;
- negli elenchi sintetici degli alimenti l'unità resta nascosta.

## Fuori scope di questo checkpoint

Non sono ancora introdotti i flussi Telegram completi delle ricette. La
migration e la query backend preparano il terreno per il prossimo checkpoint
operativo.
