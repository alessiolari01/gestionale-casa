# Ricette

**Stato: STEP 7.2C — fondazioni operative.**

## Decisione architetturale

Le ricette seguono lo stesso modello introdotto per gli alimenti:

- proprietà personale separata dalla visibilità;
- una sola ricetta centrale, condivisibile in zero, uno o più spazi;
- nessuna duplicazione automatica quando una ricetta viene condivisa;
- permessi espliciti di modifica/gestione separati dalla semplice visibilità;
- backend fail-closed.

La precedente ipotesi di riusare direttamente la radice `items` non viene
adottata per la ricetta centrale, perché lo scoping storico di `items` non
rappresenta bene il modello «un proprietario + più spazi visibili» consolidato
nello Step 7.1/7.2. Foto, storico e altre funzioni trasversali verranno
collegate con integrazioni dedicate, senza duplicare la ricetta.

## Tabelle Step 7.2C

La migration introduce:

- `ricette` — dati principali e proprietario;
- `ricetta_spazi` — visibilità della stessa ricetta nei diversi spazi;
- `ricetta_ingredienti` — ingredienti strutturati che referenziano gli
  `alimenti` esistenti.

Ogni ingrediente conserva:

- alimento;
- quantità;
- unità di misura;
- nota opzionale;
- flag opzionale;
- ordinamento.

L'alimento non viene copiato dentro la ricetta: `alimento_id` resta il
riferimento centrale.

## Ricerca per ingredienti

La struttura è predisposta per selezionare più alimenti e cercare le ricette
che ne contengono **almeno uno**.

Esempio, con ingredienti richiesti `Pollo + Riso + Zucchine`:

1. ricetta con Pollo + Riso + Zucchine → 3 corrispondenze;
2. ricetta con Pollo + Riso → 2 corrispondenze;
3. ricetta con Zucchine → 1 corrispondenza.

L'ordinamento principale è quindi il numero di ingredienti richiesti presenti,
dal maggiore al minore. A parità viene usato il nome della ricetta come
criterio stabile e leggibile.

L'indice `idx_ricetta_ingredienti_ricerca (alimento_id, ricetta_id)` è pensato
proprio per questa query con `COUNT(DISTINCT alimento_id)`.

## Dosi e porzioni

La ricetta salva `porzioni_base`. Gli ingredienti hanno quantità e unità
strutturate, così il passo successivo potrà scalare le dosi senza interpretare
testo libero.

## Condivisione e permessi

`inviti_risorsa` e `permessi_risorsa` vengono riusati con
`tipo_risorsa = 'ricetta'`.

La sola visibilità in uno spazio non concede automaticamente il diritto di
modifica. Proprietario e collaboratori autorizzati seguiranno gli stessi
livelli già definiti per gli alimenti:

- può modificare;
- può modificare e gestire i permessi.

## Passo successivo

Il backend Telegram dovrà aggiungere:

- creazione ricetta;
- aggiunta/rimozione ingredienti;
- procedimento;
- modifica;
- condivisione e collaboratori;
- ricerca per più ingredienti con ranking delle corrispondenze.

## Prodotto specifico e formato

Dal Step 7.2F.0 un prodotto commerciale può avere più formati. Un ingrediente
ricetta può continuare a scegliere opzionalmente il prodotto commerciale, ma
**non salva il formato della confezione**.

Esempio: una ricetta può richiedere `150 g` di `Philadelphia · Original`; non
deve sapere se al supermercato verrà acquistata una confezione da 175 g, 200 g
o 350 g. Questa decisione appartiene alla futura Lista spesa.
