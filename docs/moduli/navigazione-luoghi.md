# Navigazione dei luoghi

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

Il 6C.3C non estende ancora il modello storico con l'identità del contenitore: questa integrazione è riservata al 6C.4.

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

- 6C.1 ✅ backend gerarchia contenitori.
- 6C.2 ✅ UI Telegram contenitori, verificata su S9.
- 6C.3A ✅ navigazione unificata + creazione contestuale oggetti — `413605e`.
- 6C.3 successivo ⏭️ completare assegnazione/spostamento oggetti tra contenitori.
- 6C.4 ⏭️ storico contenitori/percorso.
- 6C.5 ⏭️ verifica finale, docs, PR/CI/merge.

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
