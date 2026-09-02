# Planner alimentare

**Operativo.** `src/modules/planner_alimentare.rs`, raggiungibile da
`🍽️ Alimentazione → 📅 Planner alimentare` o con `/planner`.

Pianifica i pasti di una settimana scegliendo una ricetta, chi partecipa e in
che quantità, e conserva quello che è stato deciso anche se le ricette cambiano
dopo.

## Il concetto che regge tutto: lo snapshot

Quando un pasto viene pianificato, il planner **non salva un riferimento alla
ricetta**: salva una copia congelata di ciò che serviva in quel momento — nome
della ricetta, porzioni base, profili partecipanti con le loro percentuali, e
la quantità calcolata di ogni ingrediente.

Il motivo è che modificare una ricetta non deve cambiare in silenzio il
passato. Se domani aggiungi 50 g di burro alla carbonara, la cena di martedì
scorso resta quella che hai davvero cucinato.

Quando la ricetta viva cambia dopo la pianificazione, il pasto viene
**segnalato** con `🔄` nella settimana, nel giorno e nel dettaglio, e
l'aggiornamento è offerto con una conferma esplicita che dichiara cosa cambia.
Non è mai automatico, e riguarda solo i pasti di oggi o futuri: riscrivere le
quantità di un pasto passato significherebbe riscrivere la storia.

## Stati di un pasto

| simbolo | stato | cosa comporta |
|---|---|---|
| `○` | pianificato | modificabile, rimovibile |
| `✅` | consumato | congelato: non si modifica e non si rimuove |
| `⏭` | saltato | congelato, e non eliminabile |
| `🔄` | da aggiornare | pianificato, ma la ricetta è cambiata dopo |

Il congelamento è imposto dal database con dei trigger, non solo dalla UI.

## Schermate

**Settimana** — lunedì-domenica, un pulsante per giorno con il numero di pasti;
oggi è marcato con `👉`; un giorno senza pasti non scrive «0 pasti», tace. Il
testo del messaggio non ripete i giorni: dice cosa si mangia **oggi**, che i
pulsanti non possono dire.

**`📅 Vai a una data`** — griglia del mese; i giorni che hanno già dei pasti
portano un `•`. La settimana appartiene al mese in cui cade il giovedì, così
aprendo il calendario oggi è sempre visibile.

**Giorno** — i pasti come pulsanti, e `➕ Nuovo pasto`.

**Dettaglio** — ricetta, partecipanti, stato e quantità totali aggregate per
alimento; le azioni disponibili dipendono dallo stato.

## La settimana si crea da sola

Non esiste una creazione manuale del planner: la settimana nasce alla prima
apertura. È una scelta deliberata per non aggiungere un concetto in più a chi
usa il bot — «creare un planner» sarebbe un passaggio che non decide niente.

## Tabelle

```text
planner_alimentari                  periodo, proprietario, spazio
planner_pasti                       data, tipo, ricetta + snapshot, stato
planner_pasto_profili               partecipanti + fattore porzione congelato
planner_pasto_ingredienti_snapshot  quantità congelate per profilo/ingrediente
```

`quantita_finale_snapshot` a `NULL` significa **ingrediente escluso**, che è
diverso da una quantità zero: zero è un numero da sommare, l'esclusione è
un'assenza intenzionale.

## Limite noto

I **pasti liberi** — «stasera si mangia fuori» — non sono rappresentabili
perché `ricetta_nome_snapshot` è `NOT NULL`. La decisione è stata rinviata
quando è arrivato l'esito «saltato», che copre il caso più frequente.
