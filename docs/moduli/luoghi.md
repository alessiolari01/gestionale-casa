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

---

## Navigazione dei luoghi

## Spostamento gerarchico degli oggetti (6C.3C)

Il comando `🚚 Sposta` non si ferma più a casa/stanza. Il selettore segue la gerarchia reale:

```text
Casa
└── Stanza
    └── Contenitore
        └── Sottocontenitore
```

Regole:
- la posizione attuale è sempre ricostruita fino al contenitore più specifico;
- a ogni livello esiste un'azione `Sposta qui` per fermarsi a quel livello;
- casa e stanza mostrano i contenitori radice compatibili con il proprio ambito;
- un contenitore mostra i propri figli e permette di scendere ricorsivamente;
- `↩️ Livello precedente` risale al contenitore padre, alla stanza o alla casa coerente;
- quando l'oggetto viene spostato direttamente in casa/stanza, `contenitore_id` viene impostato a `NULL`;
- selezionare la destinazione identica a quella corrente non produce modifiche.

Il 6C.3C aveva lasciato fuori l'identità storica del contenitore; il 6C.4 l'ha poi integrata con snapshot del percorso e prima/dopo container-aware.

## Principio

La UI dipende dal **luogo che l'utente sta visualizzando**, non dal modulo o dal menu da cui è arrivato. Questa è una regola architetturale anche per gli sviluppi futuri.

## Gerarchia canonica

```text
Casa
├── Stanza
│   └── Contenitore
│       └── Sottocontenitore
│           └── Oggetto
└── Contenitore direttamente nella casa
    └── Oggetto
```

Una stanza appartiene a una casa. Un contenitore appartiene sempre a una casa, può appartenere a una stanza e può avere un contenitore padre. Un oggetto può stare senza luogo strutturato, direttamente nella casa, nella stanza o in un contenitore a qualunque profondità.

Il percorso completo viene **calcolato dalle relazioni** e non deve essere persistito come unica stringa `Casa/Garage/Armadio/...`.

## Sezione Telegram

Dal menu principale: `🏠 Case, stanze e contenitori`.

La sezione espone:
- `🏠 Elenco case`;
- `🚪 Elenco stanze`;
- `📦 Elenco contenitori`;
- `🌳 Struttura completa`;
- `➕ Crea…`;
- `🏠 Menu principale`.

`➕ Crea…` consente di avviare casa, stanza o contenitore.

## Azioni contestuali

Casa:
- nuova stanza;
- nuovo contenitore direttamente nella casa;
- nuovo oggetto qui;
- stanze, contenitori e oggetti;
- ritorno alla sezione + menu principale.

Stanza:
- nuovo contenitore nella stanza;
- nuovo oggetto qui;
- contenitori e oggetti;
- ritorno alla casa + menu principale.

Contenitore:
- nuovo sottocontenitore;
- nuovo oggetto qui;
- rinomina, sposta, elimina;
- ritorno al padre/stanza/casa + menu principale.

## Contratto di navigazione

Ogni schermata interna rilevante deve avere:
1. un ritorno al livello logico precedente, con etichetta esplicita quando possibile;
2. un accesso diretto a `🏠 Menu principale`.

Esempi: `↩️ Torna al Garage`, `↩️ Torna a Casa principale`, `↩️ Contenitori`.

Non obbligare l'utente a usare "indietro" due o tre volte per raggiungere la home.

## Albero e riferimenti diretti

Esempio:

```text
🏠 Casa principale  /luogo_h1
├── 🚪 Camera       /luogo_r2
└── 🚪 Garage       /luogo_r3
    └── 📦 Armadio  /luogo_c7
        └── 📦 Ripiano  /luogo_c9
```

Riferimenti canonici:
- `/luogo_h<ID>` = casa;
- `/luogo_r<ID>` = stanza;
- `/luogo_c<ID>` = contenitore.

Si usano ID tipizzati, non i nomi, così nomi uguali in rami differenti non sono ambigui.

## Nuovo oggetto qui

Da casa, stanza o contenitore è disponibile `➕ Nuovo oggetto qui`.

Il bot mostra il percorso rilevato e chiede conferma:
- `✅ Sì, crea qui`;
- `🔄 Scegli un'altra posizione`.

Se confermato, casa/stanza/contenitore vengono mantenuti come relazioni strutturate e la sezione `Posizione` della bozza risulta già completata. Il percorso visualizzato viene ricostruito dalle relazioni.

Se si sceglie un'altra posizione, dopo il nome viene riaperto il normale selettore di posizione.

Dopo il salvataggio di un oggetto creato da casa, stanza o contenitore, la scheda appena creata conserva il contesto di partenza e aggiunge un pulsante inline `↩️ Torna a <luogo>`. Il pulsante è contestuale: compare per la creazione avviata da `Nuovo oggetto qui`, non per un oggetto aperto normalmente da ricerca/elenco.

## Terminologia

Evitare `Livello principale`. Mostrare la destinazione reale, ad esempio:
- `📍 Sposta in Camera`;
- `📍 Sposta in Casa principale`.

## Stato Step 6C

- 6C.1 ✅ backend gerarchia contenitori — `cc3ba4c`.
- 6C.2 ✅ UI Telegram contenitori, verificata su S9 — `4c64798`.
- 6C.3A ✅ navigazione unificata + creazione contestuale oggetti — `413605e`.
- 6C.3B ✅ UX gerarchica e posizione completa — `24944ac`.
- 6C.3C ✅ spostamento oggetti tra contenitori — `658e455`.
- 6C.5 🔧 chiusura documentale e PR/CI/merge.

Nessun reset globale del database e nessuna cancellazione automatica degli oggetti dovuta alla rimozione di un luogo.
## Annullamento contestuale (6C.3B)

`/annulla` non è una voce permanente. È disponibile quando il bot sta aspettando un dato o sta eseguendo un flusso temporaneo. L'annullamento deve riaprire il contesto di partenza: casa, stanza, contenitore, oggetto oppure menu di sezione.

Esempi:
- Casa principale → `Nuova stanza` → `/annulla` → Casa principale;
- Garage → `Nuovo contenitore` → `/annulla` → Garage;
- Armadio → `Nuovo oggetto qui` → `/annulla` → Armadio.

## Posizione completa degli oggetti (6C.3B)

Quando un oggetto ha un luogo strutturato, elenchi, ricerca e dettaglio mostrano tutto il percorso fino al luogo più specifico e, su una riga separata, il riferimento diretto:

```text
📍 Casa principale / Garage / Armadio / Ripiano 2
/luogo_c9
```

Se l'oggetto è direttamente in una stanza si usa `/luogo_r<ID>`; se è direttamente nella casa si usa `/luogo_h<ID>`.

Il vecchio campo libero `oggetti.posizione` non viene più richiesto nei nuovi flussi. Rimane nel database come dato legacy e può continuare a essere letto/ricercato finché non verrà eventualmente gestita una migrazione esplicita futura.

## Convenzione visiva oggetti/contenitori (6C.3B)

Per evitare ambiguità nella gerarchia:

- `🏷️` = oggetto/item catalogato;
- `📦` = contenitore/sottocontenitore.

La convenzione vale per menu, schede, elenchi, albero dei luoghi e storico individuale. `🏭` viene usato per marca/modello così da non confondere il simbolo dell'oggetto con i suoi attributi.

## Gerarchia delle azioni e tastiere compatte

Per evitare schermate troppo trafficate, le tastiere inline seguono una regola comune:

- normalmente massimo due azioni per riga; sono ammesse tre azioni quando le etichette sono corte e omogenee, soprattutto nelle righe di creazione;
- azioni simili e brevi vengono affiancate;
- i figli gerarchici già presenti nella schermata vengono mostrati prima delle azioni operative: stanza → contenitore → oggetto;
- gli elementi dinamici con nomi potenzialmente lunghi restano su una riga propria;
- i comandi di creazione usano la forma compatta `➕<simbolo> Nome`, per esempio `➕🚪 Stanza`, `➕📦 Contenitore`, `➕🏷️ Oggetto`;
- i pulsanti che aprono un elenco usano `📋` insieme al simbolo dell'entità, per esempio `📋📦 Contenitori qui` e `📋🏷️ Oggetti qui`;
- le azioni amministrative sono raccolte sotto `⚙️ Gestisci`;
- `🗑 Elimina` resta isolato su una riga propria nelle schermate di gestione;
- la navigazione (`↩️ ...` e `🏠 Menu principale`) resta in fondo.

Applicazione attuale:

- casa: `⚙️ Gestisci` → rinomina / elimina;
- stanza: `⚙️ Gestisci` → rinomina / elimina;
- contenitore: `⚙️ Gestisci` → rinomina + sposta, poi elimina isolato;
- oggetto: lo spostamento resta visibile perché è un'azione frequente, mentre `⚙️ Gestisci` raccoglie modifica dati ed eliminazione.

Nella casa, le stanze già presenti vengono mostrate prima dei comandi. Subito dopo viene usata una sola riga compatta `➕🚪 Stanza` · `➕📦 Contenitore` · `➕🏷️ Oggetto`, seguita dagli elenchi `📋📦 Contenitori qui` e `📋🏷️ Oggetti qui`. Nella stanza e nel contenitore la stessa convenzione si riduce alle sole entità che possono essere create in quel livello.

### Ritorno dagli elenchi contestuali

Gli elenchi aperti con `Contenitori qui` mantengono il contesto del luogo:
- da una casa, `↩️ Torna alla casa` riapre quella casa;
- da una stanza, `↩️ Torna alla stanza` riapre quella stanza.

Il pulsante non deve risalire automaticamente al livello superiore della gerarchia: deve tornare al luogo dal quale l'elenco contestuale è stato aperto.
