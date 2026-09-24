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

## Ricette classiche già nel catalogo (18 settembre 2026)

Il catalogo globale contiene venticinque ricette di base — pomodoro, pesto,
risotto ai funghi, frittata, pizza in teglia, pollo al forno, minestrone… —
con ingredienti collegati al catalogo alimenti e procedimento in passaggi
brevi.

**Da dove viene quel testo**: l'ha scritto il modello, con parole sue, su
piatti di tradizione. Non è copiato da GialloZafferano né da altri siti:
Alessio ha scelto proprio questa strada (opzione "a" del 18 settembre 2026),
perché i procedimenti di un sito sono di quel sito. Le ricette portano
`🔗 Ricetta di Catalogo del bot` e **nessun link**, che si aggiunge a mano
quando serve.

Sono ricette di base: porzioni e quantità sono indicative e si correggono
come qualunque altra ricetta.

### Tornare indietro da una ricetta (18 settembre 2026)

Una ricetta si apre da quattro strade: l'elenco (a una certa pagina), la
ricerca per nome, la ricerca per ingredienti e le ricette "con quello che
ho" delle scorte. Il bot ricorda per chat quale è stata l'ultima
(`ProvenienzaRicetta`) e `⬅️ Indietro` nel dettaglio torna lì. Le ricerche
si ricostruiscono con `recipe:back`, perché né il testo cercato né gli
ingredienti scelti stanno nei 64 byte di un callback.

### Ricette del catalogo e amministratore (18 settembre 2026)

Le ricette del catalogo globale non hanno proprietario. L'amministratore di
sistema le modifica come se fossero sue — nome, porzioni, ingredienti,
procedimento, allegati, fonte — e le archivia, ma non le elimina: possono
stare nei pasti già pianificati di qualcun altro. Visibilità e
collaboratori non compaiono, perché le vede già chiunque. Gli altri utenti
le leggono soltanto.

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

### Tastiere dei campi (18 settembre 2026)

- **Creazione**: `❌ Annulla la ricetta` in una riga sua (butta via la
  bozza), sotto `⬅️ Indietro | 💡 Migliora | 🏠 Menù principale` (il passo
  prima).
- **Modifica di una ricetta che esiste**, e ricerca: solo `❌ Annulla`, che
  torna dove si era senza salvare, nella riga `❌ Annulla | 💡 Migliora |
  🏠 Menù principale`. Prima si usava la tastiera della creazione, e
  `❌ Annulla` rispondeva "Creazione ricetta annullata" anche modificando il
  procedimento.

### Allegati degli step (18 settembre 2026)

Aprire un allegato manda **un messaggio solo**: la foto (o il video) con
la didascalia e la riga di navigazione sotto. `⬅️ Indietro` torna allo step
se l'allegato è stato aperto dalla modifica (`recipe:media:item:{id}:modifica`),
alla procedura guidata altrimenti. Anche la conferma di eliminazione è la
foto stessa, con la domanda come didascalia.

### Procedura guidata con gli allegati (19 settembre 2026)

Uno step con foto o video si mostra **con l'allegato**: la schermata è la
foto (o il video), con il testo dello step come didascalia e i pulsanti
sotto. I video si mandano come animazione, quindi partono da soli e
**girano in loop**, senza audio. Con più allegati `◀️ 📎 1/3 ▶️` li scorre
in tondo senza cambiare step; gli step si cambiano con `⏪ Step 2/5 ⏩`
(`✅` sull'ultimo). Sotto la foto la navigazione è `⬅️ | 💡 | 🏠`.

Oltre 900 caratteri il testo non sta in una didascalia: lo step resta una
schermata di testo con `📎 Vedi foto/video dello step`. Lo stesso se il file
non è più sul telefono.

### "📝 Riscrivi tutto": come si divide il testo

Un passaggio per riga, oppure separati da una riga vuota, oppure tutto su
una riga numerata in fila (`1. … 2) … 3) …`). La numerazione scritta a mano
viene tolta e rifatta. Il testo passa da `clean_text_righe`, che toglie gli
spazi doppi ma **tiene gli a-capo**: fino al 18 settembre 2026 passava da
`clean_text`, che li riduceva a spazi, e ogni procedimento diventava un
passaggio solo.

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

## Ricette archiviate e ripristino (24 settembre 2026)

`📦 Archivia` toglie una ricetta dagli elenchi senza cancellarla: serve
perché una ricetta può stare nei pasti già consumati di altri, e quella
storia non si riscrive. Fino al 24 settembre 2026 era però una porta a senso
unico — archiviata, la ricetta spariva e non c'era più modo di rivederla.

`🗄 Ricette archiviate` nel menù delle ricette apre l'elenco (5 per pagina,
C6) e compare **solo se ce n'è almeno una**, con il numero accanto: un
pulsante che porta a una schermata vuota è rumore. Si vedono soltanto le
proprie; l'amministratore vede anche quelle del catalogo globale, come per
`📦 Archivia`. Aprendone una si legge il nome e si conferma con
`♻️ Ripristina`; dopo, `🍳 Aprila` porta dritti alla ricetta tornata negli
elenchi, con i suoi ingredienti e il suo procedimento — non è una copia
nuova, è la stessa riga con `archiviata = 0`.

Ripristinare chiede lo stesso permesso che serviva per archiviare
(`restore_recipe` ripete la condizione di `archive_recipe`): chi non poteva
archiviare non può nemmeno disfare.

**Il giorno dopo, due correzioni** (collaudo del 24 settembre 2026). I
callback `recipe:restore:` non arrivavano a `handle_edit_callback`, che li
gestisce ma viene chiamata solo per i dati che iniziano con `recipe:edit:`:
toccando una ricetta archiviata il bot rispondeva "Azione Ricette non
disponibile". Ed è il tipo di errore che i test non prendono — la funzione
era giusta, non la raggiungeva nessuno. La seconda: il menù mostrato subito
dopo `✅ Ricetta archiviata` era la tastiera fissa, senza il pulsante delle
archiviate; adesso è `menu_keyboard_aggiornata`, che le conta.

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
