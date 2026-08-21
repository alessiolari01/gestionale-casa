# Case, stanze e posizione strutturata — Step 6A

## Scopo

Lo Step 6A introduce una posizione riconosciuta dal gestionale invece di usare
solo testo libero. La struttura approvata è:

```text
Account
├── Casa principale
│   ├── Cucina
│   ├── Camera
│   └── Garage
└── Casa al mare
    ├── Cucina
    └── Camera
```

Un elemento può essere:

- senza luogo;
- assegnato direttamente a una casa;
- assegnato a una stanza di quella casa.

Il modello è volutamente condiviso tramite `items`, così in futuro potrà essere
riusato anche da Vestiti, Veicoli e altri moduli senza duplicare la logica.

## Scelta architetturale

Sono state considerate due alternative principali:

1. una singola tabella gerarchica generica `luoghi` con `parent_id`;
2. tabelle esplicite `abitazioni` e `stanze`, con un collegamento condiviso
   `item_luogo`.

Per lo Step 6A è stata scelta la seconda soluzione perché il dominio attuale ha
solo due livelli certi, casa e stanza. È più leggibile, rende semplici i vincoli
SQLite e non costringe a introdurre subito una gerarchia arbitraria.

Lo Step 6C ha aggiunto contenitori/sotto-posizioni senza cancellare questo
modello. In quel momento si valuterà se aggiungere un terzo livello dedicato o
una gerarchia generica sotto la stanza.

## Migrazione

Migrazione: `migrations/20260815183000_luoghi.sql`.

### `abitazioni`

| Campo | Note |
|---|---|
| `id` | chiave primaria |
| `nome` | obbligatorio, unico senza distinzione maiuscole/minuscole |
| `descrizione` | opzionale, predisposto per estensioni future |
| `creato_il` / `aggiornato_il` | timestamp ISO8601 |

### `stanze`

| Campo | Note |
|---|---|
| `id` | chiave primaria |
| `abitazione_id` | FK verso `abitazioni`, `ON DELETE CASCADE` |
| `nome` | obbligatorio; unico all'interno della stessa casa |
| `descrizione` | opzionale |
| `creato_il` / `aggiornato_il` | timestamp ISO8601 |

Lo stesso nome di stanza può quindi esistere in due case diverse, per esempio
`Camera` in `Casa principale` e `Camera` in `Casa al mare`.

### `item_luogo`

È la relazione condivisa tra un elemento del gestionale e il suo luogo:

| Campo | Note |
|---|---|
| `item_id` | PK + FK verso `items(id)`, cascade quando l'item viene eliminato |
| `abitazione_id` | casa assegnata |
| `stanza_id` | stanza opzionale |

Regole:

- se c'è una stanza deve esserci anche la casa;
- la stanza deve appartenere alla casa selezionata;
- eliminando una stanza, `stanza_id` diventa `NULL` e l'item resta nella casa;
- eliminando una casa, la relazione `item_luogo` viene eliminata ma l'item
  rimane nel gestionale;
- eliminando l'item, la relazione viene eliminata automaticamente.

Un trigger controlla che non sia possibile associare, per errore, una stanza di
`Casa B` a un item indicato come appartenente a `Casa A`.

## Compatibilità con il vecchio campo `oggetti.posizione`

La colonna `oggetti.posizione` **non viene eliminata e non viene convertita
automaticamente**. Dallo Step 6A assume il significato di dettaglio libero della
posizione.

Esempio:

```text
🏠 Casa principale / 🚪 Garage
📌 Scaffale 2, cassetto alto
```

Questo evita di perdere valori già inseriti e impedisce al codice di indovinare
in modo rischioso se una stringa vecchia rappresenti una casa, una stanza o un
dettaglio.

Gli oggetti esistenti restano quindi inizialmente senza casa/stanza strutturata;
possono essere assegnati manualmente dalla loro scheda.

## Interfaccia Telegram

Il menu principale aggiunge:

```text
🏠 Case e stanze
```

Menu luoghi:

```text
[ ➕ Nuova casa ]
[ 📋 Elenco case ]
[ 🏠 Menu principale ]
```

La scheda casa mostra le stanze registrate, il numero di elementi associati e
permette di:

- aggiungere una stanza;
- aprire una stanza;
- vedere gli oggetti della casa;
- rinominare la casa;
- eliminare la casa con conferma.

La scheda stanza permette di:

- vedere gli oggetti della stanza;
- rinominare la stanza;
- eliminare la stanza con conferma;
- tornare alla casa.

## Posizione di un oggetto

La scheda oggetto distingue esplicitamente il primo inserimento da uno
spostamento successivo:

```text
oggetto senza luogo  → 🏠 Assegna casa / stanza
oggetto già collocato → 🚚 Sposta oggetto
```

Flusso di prima assegnazione:

```text
Scheda oggetto
→ Assegna casa / stanza
→ scegli casa
→ scegli "solo casa" oppure una stanza
→ conferma: luogo assegnato
→ ritorno alla scheda oggetto
```

### Assegnazione durante la creazione di un oggetto

La creazione di un nuovo oggetto integra il luogo direttamente nella bozza con
un flusso guidato unico:

```text
Nuovo oggetto
→ 🏠 Posizione
→ 1/3 Casa
→ 2/3 Stanza
→ 3/3 Dettaglio posizione
→ ✅ Salva
```

Regole:

- se viene scelta una casa, il bot mostra **solo le stanze di quella casa**;
- è possibile scegliere `solo casa` e passare al dettaglio;
- se viene premuto `⏭ Salta casa -> dettaglio`, il passaggio stanza non viene
  mostrato e si passa direttamente al dettaglio libero;
- non esiste quindi nell'interfaccia il caso `stanza senza casa`, coerentemente
  con il modello dati;
- casa, stanza, dettaglio dell'oggetto e riga `items` vengono persistiti insieme
  nella stessa transazione quando si preme `✅ Salva`;
- durante la modifica di un oggetto già esistente, invece, casa/stanza non vengono
  nascoste dentro `✏️ Modifica`: si usa `🚚 Sposta oggetto`, mantenendo esplicita
  la semantica dello spostamento in preparazione allo storico dello Step 6B.

Flusso di spostamento:

```text
Scheda oggetto
→ 🚚 Sposta oggetto
→ viene mostrata la posizione attuale
→ scegli la nuova casa
→ scegli "solo casa" oppure una stanza
→ conferma esplicita: Da: ... / A: ...
→ ritorno alla scheda oggetto
```

Durante uno spostamento, la stanza in cui si trova già l'oggetto viene marcata
direttamente nel pulsante, per esempio `🚚 → Garage (Attualmente qui)`. Se
l'oggetto è associato direttamente alla casa senza stanza, anche l'opzione
`solo casa` viene marcata come posizione attuale. In questo modo la destinazione
corrente è riconoscibile prima del click.

Se viene selezionata comunque la stessa destinazione, il gestionale segnala che
nessuno spostamento è stato effettuato. Questa distinzione non è solo grafica:
è una scelta semantica predisposta per lo Step 6B, così lo storico potrà
registrare separatamente `assegnazione luogo`, `spostamento luogo` e
`rimozione luogo`.

È sempre possibile usare `🧹 Nessun luogo` per rimuovere l'associazione
strutturata senza eliminare l'oggetto.

Comandi equivalenti:

```text
/luoghi
/case
/casa_nuova [nome]
/case_lista
/casa <id>
/casa_rinomina <id> [nuovo nome]
/casa_elimina <id>
/stanza_nuova <casa_id> [nome]
/stanza <id>
/stanza_rinomina <id> [nuovo nome]
/stanza_elimina <id>
/oggetto_luogo <id>
/oggetto_sposta <id>
```

`/annulla` interrompe una creazione/rinomina di casa o stanza quando è attiva
una sessione del modulo Luoghi.

## Filtri e ricerca

Dalla sezione Case e stanze è possibile aprire:

- gli oggetti di una casa, comprese le sue stanze;
- gli oggetti di una singola stanza.

La ricerca generale degli oggetti include inoltre:

- nome casa;
- nome stanza;
- dettaglio libero `oggetti.posizione`;
- tutti i campi già supportati nello Step 5.

Quindi cercando `Garage` possono essere trovati sia oggetti con dettaglio
libero contenente quella parola sia oggetti assegnati alla stanza registrata
`Garage`.

## Eliminazione sicura dei luoghi

### Eliminazione stanza

La conferma esplicita informa quanti elementi sono collegati. Dopo il delete:

- la stanza scompare;
- gli oggetti non vengono eliminati;
- gli oggetti restano assegnati alla casa ma senza stanza.

### Eliminazione casa

La conferma esplicita informa quante stanze e quanti elementi sono collegati.
Dopo il delete:

- la casa scompare;
- le stanze della casa vengono eliminate tramite cascade;
- gli oggetti non vengono eliminati;
- gli oggetti restano nel gestionale senza luogo strutturato.

## Test automatici previsti

Lo Step 6A include test per:

- creazione e lettura di case e stanze;
- unicità del nome casa senza distinzione maiuscole/minuscole;
- unicità del nome stanza all'interno della stessa casa;
- stessa stanza ammessa in case diverse;
- assegnazione di un oggetto direttamente alla casa;
- assegnazione a una stanza;
- rifiuto DB di una stanza appartenente a un'altra casa;
- eliminazione stanza senza eliminare l'oggetto e mantenendo la casa;
- eliminazione casa senza eliminare l'oggetto;
- ricerca oggetti per nome di casa e stanza;
- creazione di un oggetto con casa, stanza e dettaglio salvati insieme;
- rifiuto della creazione di una bozza incoerente con stanza ma senza casa.

## Verifica runtime richiesta prima del merge

1. creare almeno due case;
2. creare almeno due stanze nella prima casa e una nella seconda;
3. creare un nuovo oggetto usando il flusso `Casa -> Stanza -> Dettaglio` e verificare il luogo già alla prima scheda salvata;
4. creare una seconda bozza, premere `Salta casa` e verificare che venga chiesto direttamente il dettaglio senza passare dalle stanze;
5. verificare che lo stesso nome stanza sia ammesso in case diverse;
6. rinominare una casa e una stanza;
7. assegnare un oggetto direttamente a una casa;
8. spostarlo in una stanza della stessa casa;
9. spostarlo in una stanza dell'altra casa;
10. verificare scheda, elenco e ricerca per casa/stanza;
11. rimuovere il luogo dall'oggetto;
12. testare annullamento della cancellazione di casa/stanza;
13. eliminare una stanza con un oggetto assegnato e verificare che l'oggetto
    resti nella casa;
14. eliminare una casa con un oggetto assegnato e verificare che l'oggetto
    resti nel gestionale senza luogo;
15. riavviare il backend e verificare la persistenza;
16. eseguire `fmt`, `check`, `test`, Clippy e CI GitHub Actions.

## Fuori perimetro Step 6A

- storico degli spostamenti: Step 6B;
- contenitori, armadi, scaffali e sotto-posizioni strutturate: Step 6C;
- autorizzazioni differenti per casa/account;
- mappe o coordinate GPS;
- migrazione automatica delle vecchie stringhe `oggetti.posizione`.
