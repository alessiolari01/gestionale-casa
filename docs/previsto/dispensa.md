# Dispensa, frigo e freezer — previsto

> **Scritta il 16 settembre 2026 come specifica; lo stesso giorno Alessio ha
> dato il via libera e la prima fetta è stata costruita** — vedi
> `docs/moduli/dispensa.md` per come si comporta oggi
> (`src/modules/dispensa.rs`, `migrations/20260916120000_dispensa.sql`).
>
> Questo documento resta perché contiene **il perché** delle scelte e quello
> che ancora **non** esiste: la sottrazione delle scorte dal fabbisogno della
> lista della spesa, lo scarico al consumo di un pasto, la quantità
> realmente comprata e il formato della confezione.

## Perché esiste questo documento

Fino al 16 settembre 2026 frigo e dispensa erano due accenni sparsi: "la
futura dispensa potrà sottrarre le scorte" in
`docs/previsto/lista-della-spesa.md` e un esempio in
`docs/moduli/alimenti.md`. Non una specifica: nessuno poteva dire cosa
sarebbe successo spuntando "comprato".

I contenitori di case e stanze (`docs/moduli/contenitori.md`) non risolvono
il problema: contengono **oggetti**, cose singole che si possiedono, non
quantità di un alimento che cala ogni volta che si cucina. Un "Frigo"
creato come contenitore oggi non saprebbe dire che ci sono 800 g di pollo.

## Cosa manca, prima di poter costruire

Queste sono le cose che oggi la lista della spesa non sa, e senza le quali
una dispensa darebbe numeri sbagliati.

1. **La quantità davvero comprata.** "Comprato" è un sì/no su tutta la
   riga: se servono 280 g e si compra una confezione da 500 g, la lista non
   lo sa. In dispensa devono entrare 500 g, non 280.
2. **Il formato acquistato.** I formati esistono già a database
   (`docs/moduli/alimenti.md`), ma la lista non chiede mai quale si è
   preso.
3. **Il consumo.** Un pasto segnato "consumato" nel planner non toglie
   niente da nessuna scorta.
4. **Le voci libere.** "Detersivo piatti" non è collegato al catalogo: non
   può diventare una scorta di un alimento, e va trattato a parte (o
   lasciato fuori).

## Decisioni già prese (16 settembre 2026)

Prese con Alessio, che ha risposto "scegli tu" sulle due domande di
struttura e ha deciso le altre due.

### 1. Frigo, freezer e dispensa sono **luoghi di conservazione**, non case o stanze

**Scelta: un'entità dedicata**, non un riuso di case/stanze/contenitori.

Il motivo è che le due cose rispondono a domande diverse: un contenitore
dice *dove sta un oggetto in casa*, un luogo di conservazione dice *in che
condizione è conservata una scorta* — ed è quella condizione, non la
posizione fisica, a decidere quanto dura e cosa ci si può fare. Due frigo
in due stanze diverse si comportano allo stesso modo; un freezer e una
dispensa nella stessa stanza no.

Una scorta avrà quindi un tipo di conservazione (`dispensa`, `frigo`,
`freezer`), con l'eventuale collegamento a un luogo reale lasciato
**opzionale** per chi vuole precisare ("il freezer in garage"). Così il
caso normale resta a zero configurazione: chi non definisce niente ha
comunque i tre posti standard.

### 2. La merce entra **in automatico**, ma si può cambiare idea

**Scelta: automatico come impostazione predefinita, disattivabile.**

Chiudendo la spesa, le voci comprate collegate al catalogo entrano da sole
in dispensa. Una preferenza per utente (sullo stesso schema di
`preferenze_utente`, come `vista_spazi`) permette di passare a "chiedimi
prima", che apre un passaggio di conferma dove si correggono quantità e
luogo di conservazione.

Il motivo della predefinita automatica: la maggior parte delle volte la
quantità giusta è quella che si è comprata, e un passaggio obbligatorio in
più a fine spesa — con le borse ancora da svuotare — è il modo più sicuro
per far smettere di usare la funzione.

### 3. La scadenza è **opzionale**, mai obbligatoria

Decisa da Alessio: non serve da subito. Una scorta nasce senza scadenza e
chi vuole può aggiungerla dopo, sulla singola scorta. Nessun campo
obbligatorio, nessun avviso finché non c'è una data: un campo che si può
lasciare vuoto non costa niente a chi non lo usa.

Quando ci sarà una data, il passo successivo naturale è l'avviso "sta per
scadere" — che però richiede l'infrastruttura dei reminder, oggi
inesistente (`docs/previsto/reminder.md`).

### 4. La chiusura della spesa è il gancio, ed **esiste già**

Costruita il 16 settembre 2026: chiudere la spesa archivia le voci comprate
in `liste_spesa_voci_archiviate`, con alimento, prodotto, quantità, unità e
data di acquisto. È esattamente l'elenco di ciò che è entrato in casa.

La dispensa non dovrà quindi inventarsi un punto di ingresso: leggerà da
lì, o aggancerà la stessa transazione di `chiudi_spesa`.

## Come si lega alla lista della spesa

Una volta costruita, la dispensa chiude il cerchio già descritto in
`docs/moduli/alimenti.md`:

```text
ALIMENTO: Pollo
SCORTA:   800 g in frigo
RICETTA:  300 g richiesti
PIANO:    martedì Pollo
SPESA:    la quantità mancante, non tutta
```

L'aggregazione della lista (`calcola_fresche`) sottrarrebbe le scorte
disponibili **dopo** aver aggregato il fabbisogno e **prima** di sottrarre
quanto già comprato, con la stessa conversione di unità già in uso. Resta
da decidere, quando si costruirà: se una scorta insufficiente debba sparire
del tutto dalla lista o comparire ridotta (probabilmente ridotta, coerente
con il modo in cui si comporta oggi il comprato).

## Fuori da questa specifica

- **Prezzi e negozi**: sono del futuro modulo Acquisti
  (`docs/previsto/acquisti.md`).
- **Inventario non alimentare** (detersivi, carta igienica): stesso schema
  concettuale, ma passa da Acquisti, non da qui.
- **Scarico automatico al consumo di un pasto**: naturale, ma è un secondo
  blocco — prima deve esistere la scorta.

## Stato

**Costruita la prima fetta** (16 settembre 2026, `docs/moduli/dispensa.md`):
i tre luoghi di conservazione, l'aggiunta dal catalogo o a mano, quantità,
scadenza opzionale, spostamento, eliminazione con conferma, e l'ingresso
automatico dalla spesa chiusa con la sua preferenza per utente.

**Restano da fare**, nell'ordine suggerito:

1. la **quantità realmente comprata** e il formato della confezione sulla
   voce della lista della spesa (punti 1 e 2 di "Cosa manca"): finché non ci
   sono, in dispensa entra la quantità che serviva, non quella che si è
   presa davvero;
2. la **sottrazione delle scorte** dal fabbisogno della lista
   (`calcola_fresche`): cambia il cuore di un'aggregazione collaudata e in
   produzione, quindi è un blocco a sé con il suo collaudo dal vivo;
3. lo **scarico al consumo** di un pasto del planner;
4. gli **avvisi di scadenza**, quando esisterà l'infrastruttura dei reminder
   (`docs/previsto/reminder.md`).
