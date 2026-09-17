# Ricette

**Operativo.** `src/modules/ricette.rs`, `🍽️ Alimentazione → 🍳 Ricette`.

## Modello

Le Ricette seguono gli stessi principi del resto del gestionale:

- proprietario separato dalla visibilità negli spazi;
- una sola ricetta centrale, condivisibile in zero, uno o più spazi;
- nessuna copia della ricetta quando viene condivisa;
- visibilità ≠ modifica ≠ gestione permessi;
- backend fail-closed;
- nessun ID tecnico mostrato nella UI utente.

Le tabelle di base restano:

- `ricette`;
- `ricetta_spazi`;
- `ricetta_ingredienti`.

I permessi riusano `inviti_risorsa` / `permessi_risorsa` con
`tipo_risorsa = 'ricetta'`.

## Ingredienti

Ogni ingrediente mantiene sempre il riferimento all'alimento generico:

```text
ricetta_ingredienti
├── ricetta_id
├── alimento_id                 obbligatorio
├── prodotto_alimentare_id      opzionale
├── quantita
├── unita_misura_id
├── note                        opzionali
└── opzionale
```

Se l'alimento possiede prodotti commerciali, il wizard permette di scegliere:

```text
🌐 Usa alimento generico
oppure
🛒 Scegli prodotto specifico
```

Il prodotto specifico non sostituisce mai `alimento_id`. Un trigger DB verifica
che il prodotto appartenga allo stesso alimento.

### Prodotto ≠ formato

Dal Step 7.2F.0 un prodotto commerciale può avere più formati di vendita.
La Ricetta salva eventualmente il prodotto, **mai il formato acquistabile**.

Esempio:

```text
Ricetta:
Philadelphia · Original
150 g necessari

Prodotto:
Philadelphia · Original

Formati disponibili:
175 g
200 g
350 g
```

La scelta del formato verrà effettuata dalla futura Lista spesa in base alla
quantità aggregata, disponibilità, prezzo e avanzo previsto.

## Porzioni

`ricette.porzioni_base` conserva il numero di porzioni per cui sono state
inserite le quantità. Quantità e unità degli ingredienti sono strutturate e
restano indipendenti dalle confezioni dei prodotti.

## Procedimento strutturato

La migration `20260825231500_ricette_procedimento_guidato.sql` aggiunge:

- `ricetta_step`;
- `ricetta_step_media`;
- `v_ricetta_step_con_media`.

Il procedimento non viene più modellato come un unico testo libero. Ogni
ricetta ha step ordinati e numerati:

```text
Ricetta
└── Procedimento
    ├── Step 1
    │   ├── testo
    │   ├── 0..N foto
    │   └── 0..N video
    ├── Step 2
    │   ├── testo
    │   └── media opzionali
    └── Step N
```

La colonna legacy `ricette.procedimento` resta nello schema per compatibilità
storica ma non è più la fonte autorevole. Se al momento della migration esiste
un vecchio procedimento testuale, viene convertito conservativamente nello
Step 1.

Gli allegati sono salvati localmente sotto:

```text
data/media/ricette/<ricetta_id>/<step_id>/
```

Durante la creazione vengono prima salvati sotto una cartella `_draft` e
spostati nella posizione definitiva solo dopo il salvataggio della ricetta.

## Due modalità di consultazione

Gli stessi step alimentano due viste differenti.

### 📖 Procedimento completo

Mostra tutti gli step in ordine, con indicazione degli allegati disponibili.
Se il testo supera il limite di un singolo messaggio Telegram, viene suddiviso
in più messaggi senza perdere step. Gli step con foto/video espongono un
pulsante per aprire i media associati.

### 👨‍🍳 Procedura guidata

Mostra un solo step alla volta:

```text
👨‍🍳 Procedura guidata
Step 2/7

[testo dello step]

📎 Vedi foto/video dello step

⬅️ Step precedente | 2/7 | Step successivo ➡️
```

Il pulsante centrale `2/7` è informativo/no-op. All'ultimo step compare
`✅ Termina`.

## Creazione Telegram

Flusso strutturale:

```text
➕ Nuova ricetta
→ Nome
→ Porzioni base
→ Ingredienti
   → alimento
   → generico / prodotto specifico
   → quantità
   → unità
→ Procedimento
   → testo Step 1
   → foto/video opzionali
   → aggiungi Step 2 / fine procedimento
→ Visibilità
→ Riepilogo
→ Salva
```

Sono richiesti almeno un ingrediente e uno step.

## Modifica

La UI operativa permette almeno:

- modifica nome;
- modifica porzioni;
- aggiunta/rimozione ingredienti;
- aggiunta/modifica/eliminazione step;
- spostamento step su/giù con numerazione coerente;
- aggiunta/rimozione foto e video per step;
- modifica visibilità;
- gestione collaboratori;
- archiviazione da parte del proprietario.

Una ricetta deve mantenere almeno uno step.

### Ingredienti (17 settembre 2026)

`🥕 Ingredienti` mostra una riga per ingrediente: a sinistra il nome con
quantità e unità (e, a capo, `🛒` col prodotto specifico se c'è), a destra
`🗑`. Il testo della schermata dice solo quanti sono (C1).

- **Toccare il nome** apre la modifica: si scrive la nuova quantità
  nell'unità attuale, oppure `📏 Cambia unità` e poi la quantità.
  Solo chi può modificare la ricetta (controllo anche a database).
- **`🗑`** chiede conferma (C16): "Eliminare X dalla ricetta
  definitivamente? Non si può recuperare."
- L'esito ("✅ Ingrediente aggiunto.", "✅ Farina: ora 250 g." ...) sta in
  testa alla stessa schermata (C3).

**Il difetto che ha portato qui**: fino al 17 settembre ogni ricetta
sembrava senza ingredienti. La lettura (`list_recipe_ingredients`) chiedeva
la colonna `notes`, che a database si chiama `note`: falliva sempre, e sia
il dettaglio sia la gestione degli ingredienti nascondevano l'errore
mostrando un elenco vuoto. C'era dal 26 agosto (Step 7.2F.1). Ora il
dettaglio e la gestione scrivono "⚠️ Non riesco a leggere gli
ingredienti." e lasciano una riga nel log, e un test passa da tutte le
letture di una ricetta vera.

### Da dove viene la ricetta (18 settembre 2026)

`🔗 Fonte della ricetta` nel menù di modifica: si incolla un link — il bot
ne ricava il nome del sito e mostra `🔗 Apri su giallozafferano.it`, un
pulsante che apre la pagina — oppure si scrive un nome qualunque ("Nonna"),
che resta solo come testo. `🗑 Nessuna fonte` la toglie.

Il dettaglio mostra `🔗 Ricetta di …` sotto la visibilità.

**Il procedimento resta quello scritto da chi usa il bot.** Copiare i
passaggi di un sito dentro le proprie ricette è copiare materiale di altri:
il meccanismo della fonte serve a dare il merito e a portare all'originale,
non a sostituirlo.

## Elenco, dettaglio e ricerca

Menu Ricette:

```text
🍳 Ricette
├── 📋 Elenco ricette
├── ➕ Nuova ricetta
├── 🔎 Cerca
├── 🥕 Cerca per ingredienti
└── 🥫 Con quello che ho in casa      (17 settembre 2026)
```

Gli elenchi usano 5 ricette per pagina e il pulsante centrale pagina/totale è
informativo. La riga di paginazione è quella comune di `modules::liste`:
`⬅️ Precedente | n/tot | Successiva ➡️`, assente quando c'è una pagina sola.

Il testo non elenca le ricette (C1): stanno sui pulsanti, con il marcatore
`👤`/`👥` sull'etichetta. Porzioni, numero di ingredienti e numero di step si
leggono aprendo la ricetta — prima erano nel messaggio e costavano due
sotto-query correlate per riga, dieci `COUNT` a ogni pagina, per un testo che
nessuno doveva più leggere. L'ordinamento è alfabetico, quindi non va
dichiarato.

La ricerca per nome usa `nome_normalizzato` e rispetta la visibilità corrente.

## Ricerca per ingredienti

La ricerca multi-ingrediente usa semantica **OR** e ordina per numero di
corrispondenze:

```text
Richiesti: pollo + riso + zucchine

Ricetta A → 3/3
Ricetta B → 2/3
Ricetta C → 1/3
```

A parità vengono usati nome ricetta e ID interno stabile; l'ID non viene
mostrato all'utente.

## Con quello che ho in casa (17 settembre 2026)

Stessa idea della ricerca per ingredienti, ma gli ingredienti non si
scrivono: sono **quelli che ci sono già in dispensa, frigo e freezer**.
Chiesto da Alessio: le ricette ordinate da quella per cui hai più
ingredienti a quella per cui ne hai meno, con il conteggio sul pulsante
(`Carbonara · 3/5`). La schermata vive in `src/modules/dispensa.rs`
(`ricette_con_quello_che_ho`), che conosce le scorte; il pulsante è qui e
anche in `🥫 Scorte`, e `⬅️ Indietro` torna da dove si è arrivati. Dettagli
in `docs/moduli/dispensa.md`.

## Compatibilità alimentare

Il dettaglio può usare `v_ricetta_compatibilita_alimentare` per derivare la
compatibilità dagli ingredienti:

- almeno un ingrediente `no` → ricetta `no`;
- nessun `no` ma almeno un `verificare`/dato mancante → `da verificare`;
- tutti `si` → compatibile.

Le etichette sono un supporto gestionale e non sostituiscono la verifica delle
etichette reali in caso di allergie/intolleranze.

## Permessi e condivisione

- proprietario: modifica e gestione;
- permesso `Edit`: modifica contenuti ma non gestione dei permessi;
- permesso `Manage`: modifica + gestione permessi;
- semplice visibilità nello stesso spazio: sola lettura;
- ruolo admin di sistema: non rende automaticamente proprietario della ricetta.

## Da non confondere con le rifiniture UX

Il macro-step punta alla struttura e alle funzioni principali. Piccole
rifiniture di testi, disposizione pulsanti e scorciatoie possono essere
registrate in `💡 Miglioramenti` senza bloccare lo sviluppo strutturale.

## Rifiniture operative consolidate nel 7.2G

Dopo gli smoke test sono state integrate nel flusso principale:

- durante l'aggiunta ingrediente viene prima proposta l'unità predefinita dell'alimento e resta disponibile `📏 Cambia unità`; solo dopo viene richiesta la quantità;
- nella ricerca per ingredienti il primo alimento può essere digitato direttamente, senza il passaggio ridondante `Aggiungi ingrediente`;
- il filtro categoria restringe i risultati degli alimenti nella ricerca per ingredienti e non viene trattato come ingrediente alternativo;
- la ricerca Ricette espone separatamente ricerca per categoria e ricerca per ingredienti;
- al termine dell'ultimo step la procedura guidata dichiara esplicitamente che la ricetta è terminata;
- oltre all'archiviazione è disponibile l'eliminazione definitiva della ricetta con le necessarie conferme/autorizzazioni;
- la navigazione segue la UI Telegram a schermata singola e include `💡 Migliora` contestuale dove previsto.
