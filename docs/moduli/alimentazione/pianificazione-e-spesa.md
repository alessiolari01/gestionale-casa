<!-- PLANNER_VERSIONATO_DECISIONE_7_2H -->
# Decisione consolidata 7.2H — planner versionato

Il futuro Planner deve salvare la versione/snapshot della ricetta effettivamente applicata al pasto. Se la ricetta cambia dopo la pianificazione, `🔄 Aggiorna planner` va mostrato solo quando esiste una differenza e solo per pasti non completati. I pasti completati restano congelati. L'aggiornamento della lista della spesa è un'azione separata e non deve cambiare automaticamente quantità già acquistate.

# Pianificazione pasti e lista della spesa

**Stato: PREVISTO.**

## Intervalli

Il planner usa date reali e supporta:

- singolo giorno;
- alcuni giorni;
- settimana;
- mese;
- intervallo personalizzato.

## Slot pasto

Colazione, pranzo, cena e spuntino sono tipi iniziali utili ma non obbligatori.
Devono poter esistere slot personalizzati e giornate senza determinati pasti.

## Contenuto del pasto

Un pasto può essere almeno:

- ricetta;
- pasto libero/non ancora definito;
- fuori casa;
- saltato.

La situazione derivata dal turno può essere sovrascritta.

## Partecipanti

Ogni pasto può avere uno o più profili alimentari. Le quantità vengono calcolate
con:

1. ricetta base;
2. partecipanti reali;
3. personalizzazione del profilo;
4. eventuali override di ingrediente.

## Preparazione anticipata

Il planner deve poter mostrare attività che avvengono prima del pasto.

Esempio:

```text
Martedì cena 19:00 al lavoro
Preparazione: lunedì 21:30
Reminder: lunedì 20:30
```

## Lista della spesa

La lista può essere generata da una pianificazione/intervallo e deve:

- aggregare lo stesso alimento;
- convertire unità compatibili quando possibile;
- mantenere separate unità incompatibili;
- restare modificabile;
- supportare voci manuali;
- distinguere generato vs aggiunto manualmente dove utile.

La futura dispensa potrà sottrarre le scorte ma non fa parte dello Step 7.

## Relazione con Acquisti

La lista dice **cosa serve**. Il futuro modulo Acquisti dirà **quale prodotto o
confezione comprare e a quale prezzo base**.

## Scelta del formato acquistabile

Dal Step 7.2F.0 i formati di vendita sono dati strutturati separati dal
prodotto commerciale. La Lista spesa dovrà quindi:

1. aggregare la quantità necessaria dello stesso alimento/prodotto;
2. sottrarre in futuro eventuali quantità già disponibili in dispensa;
3. leggere i formati attivi del prodotto;
4. scegliere una combinazione di confezioni che copra la quantità necessaria;
5. minimizzare inizialmente lo spreco e il numero di confezioni;
6. quando saranno disponibili i prezzi, poter privilegiare il costo totale più
   conveniente e indicare punto vendita e avanzo previsto.

Esempio: per 300 g necessari e formati 175 g, 200 g e 350 g, la Lista spesa
non modifica la ricetta ma valuta quale confezione/combinazione acquistare.

<!-- STEP7_2H0_PROFILI_FONDAZIONI -->
## Versioni delle ricette applicate al planner

Il planner futuro non deve cambiare silenziosamente quando una ricetta condivisa o globale viene modificata.

Ogni utilizzo della ricetta nel planner dovrà ricordare la **versione applicata**. Se la ricetta corrente cambia e almeno un pasto interessato è ancora pianificato/non completato, la UI mostrerà `🔄 Aggiorna planner` solo in presenza di differenze reali.

L'utente potrà:

- vedere quali ricette e quali pasti sono interessati;
- applicare tutti gli aggiornamenti compatibili;
- aggiornare singole occorrenze;
- mantenere deliberatamente una versione precedente.

I pasti completati/passati restano congelati come snapshot storico e non vengono riscritti da modifiche successive alla ricetta.

Un aggiornamento del planner può rendere obsoleta una lista della spesa già generata. In quel caso la lista non viene modificata automaticamente: viene segnalato un aggiornamento disponibile separato, così gli acquisti già effettuati non vengono persi o reinterpretati.
