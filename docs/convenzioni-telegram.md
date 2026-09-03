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

> **Correzione del 2 settembre.** La prima stesura di questa riga diceva che
> *Giorgia* fosse l'autore dell'evento. Guardando di nuovo il bot: l'autore era
> `Alessio Lari` per tutti e cinque gli eventi della pagina, e *Giorgia* è il
> **profilo su cui la porzione è stata modificata**. Sull'etichetta c'era già;
> quello che mancava era **quando**. La differenza conta, perché la prima
> versione di C1 proponeva di aggiungere al pulsante una cosa che c'era.

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
📋 Elenco alimenti · 422                ← titolo con il totale
Pagina 1/85                             ← posizione

Prima i tuoi, poi i condivisi, poi il catalogo base.
👤 tuo · 👥 condiviso                    ← cose che i pulsanti non dicono

[ prova alimento 👤            ]
[ 🌾 Amido di mais             ]        ← solo i pulsanti, con tutto quello
[ 🌾 Avena                     ]           che serve a distinguerli
```

Il testo non elenca più le voci. Se una voce ha bisogno di data, autore o
categoria per essere distinta dalle altre, **quella roba va sul pulsante**, non
in un elenco parallelo:

```text
[ 🍽️ 31/08/26 19:49 · Giorgia ]
```

Sull'etichetta di un evento vanno **quando** e **su cosa**. Non ci va l'azione
a parole: in una lista filtrata per azione è identica su ogni riga, quindi non
distingue niente e occupa il posto di ciò che distingue. Resta l'icona, e il
nome per esteso è nel dettaglio insieme all'autore e al luogo. L'anno c'è in
forma breve: costa tre caratteri e toglie l'ambiguità fra un evento di
quest'anno e lo stesso giorno di un anno passato.

**Un limite che questa regola non risolve.** Due eventi dello stesso tipo, sulla
stessa entità, nello stesso minuto restano indistinguibili sull'etichetta: è il
caso reale di `Pasta test 120% → 100%` e `100% → 120%`, due modifiche opposte
fatte di seguito. Restano adiacenti e in ordine cronologico, e si distinguono
aprendole. Mettere il cambiamento sull'etichetta funzionerebbe solo per le
variazioni numeriche brevi e romperebbe tutti gli altri tipi di evento; i
secondi sarebbero una precisione che a chi legge non serve. Se all'uso questo
caso dovesse pesare davvero, si riapre la scelta — non prima.

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
`⬅️ Precedente | n/tot | Successiva ➡️` e `n/tot` non è premibile. Quando c'è
una pagina sola la riga non compare: una navigazione fra una pagina e se stessa
è rumore.

La riga non si riscrive a mano: sta in `modules::liste`, insieme
all'intestazione e alla soglia delle venti voci. Prima era ricopiata in nove
posti nei soli moduli delle liste, con quattro etichette diverse per lo stesso
pulsante, ed è per questo che la convenzione non veniva rispettata da nessuno.

La primitiva prende **il numero di pagine**, non il totale delle voci. È una
lezione pagata: la prima versione prendeva il totale e dava per scontate cinque
voci per pagina, così i punti che contano diversamente — il selettore dei filtri
dello storico ne mostra sette, una descrizione lunga è spezzata a caratteri —
non potevano usarla e si sono tenuti la loro riga. **Una primitiva che non entra
dove serve non unifica niente**, e il difetto si è visto solo aprendo il bot:
lo Storico mostrava ancora `1 / 21` con le frecce nude.

### C7. Il conteggio sta sul pulsante

`🟡 Da approvare · 0` invece di una riga di testo «Da approvare: 0» sopra un
pulsante `🟡 Da approvare`. Un pulsante che porta a una lista vuota lo dichiara
prima di essere premuto.

### C8. Una schermata vuota dice cosa fare

Mai sette righe che dicono «0 pasti». Una settimana senza pasti mostra una riga
di testo — «Nessun pasto pianificato in questa settimana» — e il pulsante che
serve a rimediare.

### C9. Oggi è sempre segnalato

In ogni vista che contiene date, il giorno corrente è marcato in modo visibile:

- in una **lista di righe**: `👉` all'inizio dell'etichetta, o `· oggi` nel
  titolo della schermata;
- in una **griglia** (il calendario): il numero fra parentesi quadre, `[1]`.
  Un'emoji allargherebbe la cella su sette colonne, e `·` è già il riempitivo
  dei giorni fuori dal mese.

È il riferimento più usato di qualunque schermata con date, e costa una riga di
codice.

### C13. Le date si scelgono da un calendario

Dove serve una data, si mostra la griglia del mese: è spaziale invece che
testuale, un tocco vale una data senza digitare, e i limiti si vedono invece di
essere spiegati.

La griglia è una sola, in `modules::calendario`, e si configura:

- `oggi`, che viene marcato;
- una funzione che, per ogni giorno, dice se è selezionabile e se porta un
  **marcatore** — così il calendario mostra anche *cosa c'è* in quei giorni,
  invece di essere solo un selettore;
- un `mese_minimo` opzionale, che spegne la freccia indietro quando non ha più
  senso.

Una freccia che non porta da nessuna parte **si spegne**, non si etichetta con
una croce: `⬅️ ❌` faceva sembrare che «indietro» fosse rotto.

Esempio, il calendario del planner: `[1]` è oggi, `•` segna i giorni che hanno
già dei pasti, `·` sono i giorni di altri mesi.

```text
|  ⬅️   |Settembre 2026|  ➡️   |
| Lun | Mar | Mer | Gio | Ven | Sab | Dom |
|  ·  |[1] •|  2  |  3  | 4 • |  5  |  6  |
|  7  |  8  |  9  | 10  | 11  |12 • | 13  |
```

Il campo di testo resta accettato dove già c'è (orari, quantità): il calendario
aggiunge una strada, non ne toglie una.

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
2. ~~**liste** (alimenti, ricette, storico, miglioramenti) — C1, C6, C7~~
   **fatto il 2 settembre**, tutte e quattro insieme perché C11 non permette di
   farne due su quattro. Il giro sul bot prima di scrivere codice ha corretto
   C1 e fatto nascere `modules::liste`;
3. **menù di sezione** — C2, C3, C10, C11;
4. **Spazi e Profilo** — C5, la parte concettualmente più difficile;
5. **menù principale** — C12;
6. **date** — C13, ovunque se ne inserisca una a mano.

Ogni blocco chiude con il collaudo su Telegram sull'S9, e questo documento si
aggiorna quando una convenzione si rivela sbagliata all'uso — non quando è
scomoda da rispettare.

**Il collaudo si fa due volte: prima di scrivere e dopo aver consegnato.** Nel
blocco liste il giro fatto prima ha corretto C1 (vedi il riquadro nella parte 1)
e quello fatto dopo ha trovato un difetto che i test non vedevano: lo Storico
mostrava ancora la vecchia riga di paginazione, perché delle sue due tastiere
ne era stata convertita una sola.

---

## Parte 4 — Visto sul bot, non ancora sistemato

Difetti osservati durante i collaudi, ognuno con il blocco della parte 3 a cui
appartiene. Stanno qui e non in un messaggio perché *chi apre questa cartella
deve poter sapere cosa manca* senza aver visto nessuna conversazione.

### Blocco 3 — menù di sezione

- **`🔎 Cerca alimento` descrive solo metà di quello che fa.** Il testo dice
  «Scrivi il nome o un alias da cercare», ma la ricerca guarda anche marca e
  nome dei prodotti commerciali collegati: è la strada per cui cercando
  `philadelphia` compare `Formaggio spalmabile`. Chi legge non può indovinarlo.
- **La riga di navigazione della ricerca è spezzata su due righe:**
  `⬅️ Indietro | ❌ Annulla | 🏠 Menù principale` e sotto `💡 Migliora`. C3 per
  un passo di procedura vuole `❌ Annulla | 💡 Migliora | 🏠 Menù principale`,
  e qui `⬅️ Indietro` e `❌ Annulla` portano allo stesso posto.

### Blocco 6 — date

- **C9 non è applicata allo Storico**, che è la schermata con più date di tutto
  il gestionale: il giorno corrente non è segnalato da nessuna parte. Serve il
  `👉` all'inizio dell'etichetta degli eventi di oggi, o `· oggi` nel titolo.

### Da decidere, non un difetto

- Nei risultati di ricerca **con un solo risultato**, la riga «Perché sono nei
  risultati» nomina l'alimento che è già sul pulsante sotto. Con cinque
  risultati il nome serve a capire di quale si parla; con uno è ridondante.
  Va deciso se toglierlo nel caso singolo o accettare la ripetizione in cambio
  di una regola sola.
