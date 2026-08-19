# Navigazione dei luoghi

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

## Terminologia

Evitare `Livello principale`. Mostrare la destinazione reale, ad esempio:
- `📍 Sposta in Camera`;
- `📍 Sposta in Casa principale`.

## Stato Step 6C

- 6C.1 ✅ backend gerarchia contenitori.
- 6C.2 ✅ UI Telegram contenitori, verificata su S9.
- 6C.3A 🔧 navigazione unificata + creazione contestuale oggetti, in verifica.
- 6C.3 successivo ⏭️ completare assegnazione/spostamento oggetti tra contenitori.
- 6C.4 ⏭️ storico contenitori/percorso.
- 6C.5 ⏭️ verifica finale, docs, PR/CI/merge.

Nessun reset globale del database e nessuna cancellazione automatica degli oggetti dovuta alla rimozione di un luogo.
