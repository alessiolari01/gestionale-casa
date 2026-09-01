# Convenzioni dell'interfaccia Telegram

Documento di riferimento per ogni schermata del bot. Nasce dal giro completo
dell'interfaccia fatto il 1 settembre 2026, schermata per schermata, sul bot
reale in esecuzione sull'S9.

Serve a una cosa sola: **rendere il gestionale usabile da una persona che non
sa come è fatto dentro.** Ogni regola qui sotto viene da un problema osservato,
non da un gusto personale. Dove una regola non è ovvia, sotto c'è il caso reale
che l'ha generata.

---

## Parte 1 — Cosa è stato trovato

Otto problemi, e sette sono **sistemici**: non appartengono a un modulo, si
ripetono ovunque. È la ragione per cui vale la pena fissare delle convenzioni
invece di correggere una schermata per volta.

### 1. Il testo ripete i pulsanti

Il problema più diffuso e quello che pesa di più. Il messaggio elenca delle voci
e subito sotto gli stessi elementi ricompaiono come pulsanti.

Osservato in: `📋 Elenco alimenti`, `🕘 Storico`, `👥 Spazi`, `💡 Miglioramenti`,
planner settimana, planner giorno.

Il caso peggiore è `Elenco alimenti`: cinque nomi nel testo, gli stessi cinque
nomi nei pulsanti, identici parola per parola. Il messaggio è alto il doppio del
necessario e l'occhio legge tutto due volte per capire che non c'era niente di
nuovo da leggere.

Il caso più dannoso è `🕘 Storico`: nel testo ogni evento ha data, ora e autore;
nel pulsante resta solo il titolo. Risultato: **tre pulsanti identici**
`🍽 Porzione modificata · Giorgia`, e per sapere quale sia quale bisogna
contarli e confrontarli con il testo sopra.

### 2. Ogni menù ha una frase che legge i pulsanti ad alta voce

- `📦 Oggetti`: «Scegli cosa vuoi fare. Usa i pulsanti per scegliere cosa fare.»
  — la stessa frase due volte, nella stessa riga.
- `🥕 Alimenti`: «Crea, consulta, cerca e filtra gli alimenti disponibili» sopra
  i pulsanti Nuovo, Elenco, Cerca, Filtra.
- `💡 Miglioramenti`: «Usa i pulsanti qui sotto per aprire la sezione
  desiderata.»
- `👥 Spazi`: «Usa i pulsanti per creare, rinominare o cambiare spazio.»

Se un pulsante ha bisogno di essere spiegato, il problema è il nome del
pulsante.

### 3. La riga di navigazione non è la stessa in due schermate

Il documento fissa `⬅️ Indietro | 💡 Migliora | 🏠 Menù principale`. Nella
realtà:

| schermata | riga trovata |
|---|---|
| Planner, Profilo, Alimenti, Ricette | `⬅️ Indietro \| 💡 Migliora \| 🏠 Menù principale` |
| Alimentazione, Storico | `💡 Migliora \| ⬅️ Indietro \| 🏠 Menù principale` |
| Spazi, Oggetti, Case, Miglioramenti | **`⬅️ Indietro` non c'è** |

Guardando il codice, però, le due metà del problema si sono rivelate cose
diverse.

**L'ordine sbagliato era un bug, non una svista.** `💡 Migliora` non è scritto
nelle tastiere: lo inserisce `context_bot.rs`, cercando nell'ultima riga il
pulsante che porta al menù principale e mettendocisi davanti. Ma in
Alimentazione e Storico *sia* `⬅️ Indietro` *sia* `🏠 Menù principale`
puntavano a `menu:main`, e la ricerca partiva dall'inizio della riga: trovava
l'Indietro e Migliora finiva davanti a tutto. Bastava cercare dal fondo.

**L'Indietro mancante, invece, era ragionevole.** In una sezione di primo
livello «indietro» e «menù principale» sono lo stesso posto, e due pulsanti per
la stessa destinazione non aiutano nessuno. Il difetto era la mancanza di una
regola, non la mancanza del pulsante.

### 4. Simboli diversi per la stessa cosa

- selezionato/non selezionato: `●` e `○` nel testo di Spazi, `⭐` sul pulsante
  dello spazio predefinito;
- pasto pianificato: `○` nelle liste del planner, `📅` nel dettaglio;
- `🌐 Tutti i miei spazi` e `🎯 Solo predefinito` per un'unica impostazione a due
  valori;
- due pulsanti `💡` diversi nello stesso menù principale: `💡 Miglioramenti` (la
  lista) e `💡 Migliora` (segnala un problema su questa schermata). Stessa icona,
  nomi quasi uguali, funzioni diverse.

### 5. Il vocabolario è quello del modello dati

`👤 Profilo` mostra in fila: «Spazio predefinito», «Vista», «Ruolo nello
spazio», «Ruolo sistema», «Spazi disponibili». Cinque concetti, quattro dei
quali sono nomi interni.

Il caso più difficile è la coppia **spazio predefinito** / **vista**: due
impostazioni indipendenti con nomi che si somigliano, una decide *dove finiscono
le cose nuove* e l'altra *cosa vedi*. Non c'è nessuna schermata che le spieghi
insieme, e sono la cosa più facile da fraintendere di tutto il gestionale.

`🥕 Alimenti` mostra: «ℹ️ I dati vengono riletti automaticamente ogni volta che
apri o modifichi questa sezione.» È un dettaglio di implementazione. L'utente
non aveva il dubbio finché non gliel'abbiamo dato noi.

### 6. Le liste lunghe non sono navigabili

`Elenco alimenti`: **422 risultati, pagina 1 di 85.** Per arrivare alle zucchine
servono ottanta pressioni. La lista è offerta come azione principale, la ricerca
come secondaria: è l'ordine sbagliato. Non c'è modo di saltare a una lettera né
di sapere con che criterio è ordinata (i propri alimenti prima, poi
alfabetico — ma non è scritto da nessuna parte).

### 7. `👤 Profilo` dice di premere un pulsante che non c'è

«Usa il pulsante 👥 Spazi per cambiare spazio predefinito o modalità di
visualizzazione.» In quella schermata `👥 Spazi` non esiste: bisogna tornare
indietro e cercarlo. Una schermata che descrive un'azione invece di offrirla.

### 8. Nomi e concordanze incoerenti

- creare qualcosa si dice in tre modi: `➕ Nuovo alimento`, `➕ Aggiungi pasto`,
  `➕ Crea…` (l'unico con i puntini, che fanno pensare a un sottomenù);
- il menù principale dice `📦 Oggetti`, la schermata si intitola
  `📦 Oggetti generici`;
- nel dettaglio pasto: `✅ Segna come consumata`, `⏭ Segna come saltata`, al
  femminile, mentre il soggetto è **il pasto** e gli altri messaggi dicono
  correttamente «un pasto consumato o saltato»;
- `Elenco ricette` prima di `Nuova ricetta`, ma `Nuovo alimento` prima di
  `Elenco alimenti`: due sezioni gemelle con l'ordine invertito.

### Un problema che non è un difetto

Il menù principale mette `🕘 Storico` in cima, sopra i moduli che si usano tutti
i giorni, e dedica due pulsanti a `👕 Vestiti · prossimamente` e
`🚗 Veicoli · prossimamente`, che non fanno niente. La riga di spiegazione
«I moduli non ancora disponibili sono indicati come prossimamente» esiste solo
perché quei due pulsanti ci sono.

---

## Parte 2 — Le convenzioni

### C1. Testo e pulsanti non si ripetono mai

Il testo dice **quello che i pulsanti non possono dire**; i pulsanti dicono il
resto. Se un'informazione sta bene sul pulsante, va sul pulsante e sparisce dal
testo.

In pratica, per una lista:

```text
🥕 Alimenti · 422                      ← titolo con il totale
Pagina 1 di 85                          ← posizione

[ 👤 prova alimento            ]
[ 🌐 Amido di mais             ]        ← solo i pulsanti, con tutto quello
[ 🌐 Avena                     ]           che serve a distinguerli
```

Il testo non elenca più le voci. Se una voce ha bisogno di data, autore o
categoria per essere distinta dalle altre, **quella roba va sul pulsante**, non
in un elenco parallelo:

```text
[ 🍽 Porzione · Giorgia · 31/08 19:49 ]
```

### C2. Nessuna frase che descrive i pulsanti

Il testo di un menù o è vuoto o dice qualcosa che i pulsanti non dicono: uno
stato, un avviso, una conseguenza. Mai «usa i pulsanti per…».

Se un pulsante non si capisce da solo, si cambia il nome del pulsante.

### C3. Una sola riga di navigazione, e sempre nello stesso ordine

Sempre ultima riga, sempre in quest'ordine:

```text
sezione di primo livello   💡 Migliora | 🏠 Menù principale
schermata più interna      ⬅️ Indietro | 💡 Migliora | 🏠 Menù principale
passo di una procedura     ❌ Annulla  | 💡 Migliora | 🏠 Menù principale
```

**`⬅️ Indietro` esiste solo se porta da qualche altra parte.** In una sezione
di primo livello coinciderebbe con `🏠 Menù principale`: in quel caso non si
mette.

`⬅️ Indietro` torna sempre alla schermata da cui si è arrivati, mai a una
schermata "logicamente superiore" scelta dal codice.

Nessuna tastiera scrive `💡 Migliora` da sé: lo inserisce `context_bot.rs`
prima dell'**ultimo** pulsante `menu:main` della riga. Chi scrive una tastiera
deve solo mettere il pulsante del menù per ultimo.

### C4. Un simbolo, un significato

| simbolo | significato | dove |
|---|---|---|
| `○` | pianificato / da fare | ovunque, anche nei dettagli |
| `✅` | fatto, confermato, consumato | ovunque |
| `⏭` | saltato | ovunque |
| `🔄` | da aggiornare | ovunque |
| `⭐` | predefinito | spazi, profili |
| `👤` | mio / personale | proprietà dei contenuti |
| `👥` | condiviso | proprietà dei contenuti |
| `🌐` | globale, di tutti | proprietà dei contenuti |

Un simbolo non compare mai con due significati, e uno stato non si scrive mai
con due simboli diversi in due schermate.

`💡 Migliora` (segnala un problema su questa schermata) e la lista dei
miglioramenti non possono avere la stessa icona: la lista diventa
`📋 Miglioramenti`.

### C5. Le parole dell'utente, non quelle del modello

- niente «ruolo sistema», «vista», «spazio predefinito» senza una frase che le
  renda concrete;
- niente dettagli di funzionamento interno: cache, riletture, id, versioni;
- **spazio predefinito** e **vista** vanno spiegati insieme, con le
  conseguenze pratiche, in una riga sola:
  «Le cose nuove finiscono in *Spazio principale*. Adesso stai vedendo *tutti i
  tuoi spazi*.»

### C6. Le liste lunghe si cercano, non si sfogliano

Sopra le 20 voci, `🔎 Cerca` è la prima azione della sezione e l'elenco è la
seconda. Il titolo dice sempre quante sono. L'ordinamento va dichiarato quando
non è alfabetico.

Cinque voci per pagina restano la regola; la paginazione mostra
`⬅️ Precedente | n/tot | Successiva ➡️` e `n/tot` non è premibile.

### C7. Il conteggio sta sul pulsante

`🟡 Da approvare · 0` invece di una riga di testo «Da approvare: 0» sopra un
pulsante `🟡 Da approvare`. Un pulsante che porta a una lista vuota lo dichiara
prima di essere premuto.

### C8. Una schermata vuota dice cosa fare

Mai sette righe che dicono «0 pasti». Una settimana senza pasti mostra una riga
di testo — «Nessun pasto pianificato in questa settimana» — e il pulsante che
serve a rimediare.

### C9. Oggi è sempre segnalato

In ogni vista che contiene date, il giorno corrente è marcato in modo visibile
(`👉` sulla riga, o `· oggi` nell'etichetta). È il riferimento più usato e
costa una riga di codice.

### C10. Un verbo solo per ogni azione

| azione | forma |
|---|---|
| creare | `➕ Nuovo <cosa>` (mai «Aggiungi», mai «Crea…») |
| elencare | `📋 Elenco <cose>` |
| cercare | `🔎 Cerca` |
| filtrare | `🏷 Filtra` |
| modificare | `✏️ Modifica` |
| eliminare | `🗑 Rimuovi <cosa>` |

Il nome della schermata coincide con il nome del pulsante che ci porta.
Le concordanze seguono il soggetto reale: **il pasto** è consumat**o** e
salt**ato**.

### C11. Le sezioni gemelle si somigliano

Alimenti, Ricette, Profili, Oggetti sono la stessa cosa con contenuti diversi:
stesso ordine dei pulsanti, stessi nomi, stessa paginazione. Chi ha imparato una
sezione le ha imparate tutte.

Ordine canonico di una sezione:

```text
[ 🔎 Cerca ]                 ← per primo sopra le 20 voci, altrimenti terzo
[ 📋 Elenco <cose> · n ]
[ ➕ Nuovo <cosa> ]
[ 🏷 Filtra ]                ← solo se esistono filtri veri
⬅️ Indietro | 💡 Migliora | 🏠 Menù principale
```

### C12. Il menù principale è ordinato per uso, non per architettura

Prima quello che si usa tutti i giorni, poi il resto, in fondo gli strumenti.
I moduli non ancora disponibili **non compaiono**: quando non ci sono, sparisce
anche la riga che spiega cosa vuol dire «prossimamente».

---

## Parte 3 — Come si applica

Una convenzione che vale solo per il codice nuovo non serve a niente. L'ordine
di applicazione è questo, dal più visibile al meno:

1. **Planner** — è la sezione che si usa ogni giorno ed è quella dove la
   duplicazione pesa di più;
2. **liste** (alimenti, ricette, storico, miglioramenti) — C1, C6, C7;
3. **menù di sezione** — C2, C3, C10, C11;
4. **Spazi e Profilo** — C5, la parte concettualmente più difficile;
5. **menù principale** — C12.

Ogni blocco chiude con il collaudo su Telegram sull'S9, e questo documento si
aggiorna quando una convenzione si rivela sbagliata all'uso — non quando è
scomoda da rispettare.
