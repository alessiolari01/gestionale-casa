# Ricette

**Stato: PREVISTO.**

## Radice condivisa `items`

La scelta architetturale esistente viene preservata: una ricetta è un'entità
gestita dal gestionale e deve riusare la radice comune `items` se la migration
7.2 conferma la compatibilità con il nuovo scoping per spazio. Questo consente
di riusare foto, tag, storico e altre funzioni trasversali senza duplicarle.

La tabella specifica ricetta conterrà solo i dati di dominio che non appartengono
al core comune.

## Struttura

Una ricetta deve avere almeno:

- nome;
- descrizione/nota opzionale;
- categorie;
- tag;
- ingredienti strutturati;
- quantità e unità;
- procedimento ordinato in passaggi;
- foto riusando l'infrastruttura condivisa quando possibile;
- numero/scala di riferimento delle porzioni.

## Ingredienti

Ogni ingrediente deve riferirsi a un alimento strutturato. Il testo libero può
rimanere come nota, non come unica sorgente della quantità.

Questo abilita:

- filtro per ingrediente;
- aggregazione lista della spesa;
- porzioni personalizzate;
- futura dispensa;
- future sostituzioni.

## Categorie e tag

Le categorie descrivono il tipo di piatto; i tag caratteristiche trasversali.

Esempi categorie:

- colazione;
- primo;
- secondo;
- contorno;
- piatto unico;
- dolce;
- snack;
- altro.

Esempi tag:

- veloce;
- economica;
- meal prep;
- preferita.

La tassonomia definitiva resta modificabile e non va irrigidita inutilmente.

## Condivisione e copia

Una ricetta può essere:

- personale nello spazio personale;
- condivisa nello spazio famiglia;
- copiata in modo indipendente;
- inviata come copia ad un altro utente quando la funzione sarà implementata.

Una copia può conservare la provenienza informativa dall'originale ma non viene
sincronizzata automaticamente.

## Storico

Creazione, modifica ingredienti/procedimento, condivisione e altre operazioni
significative devono essere attribuite all'autore.
