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
pulsanti non possono dire. **Dall'11 settembre 2026** un giorno con almeno
un'assegnazione turno (per qualunque profilo dello spazio, vedi
`docs/moduli/turni-e-routine.md`) porta anche `🗓️` accanto al giorno —
simbolo diverso da `•` del calendario mensile, per non confondere i due
significati (C4).

**`📅 Vai a una data`** — griglia del mese; i giorni che hanno già dei pasti
portano un `•`, quelli con un turno assegnato portano `◆` (entrambi insieme:
`•◆`). La settimana appartiene al mese in cui cade il giovedì, così aprendo
il calendario oggi è sempre visibile.

**Giorno** — i pasti come pulsanti, `➕ Nuovo pasto`, e — dall'11 settembre
2026 — `📅 Assegna un turno a questo giorno`, che apre il flusso di
assegnazione di `turni.rs` saltando la scelta della data (già nota): si
sceglie solo il profilo e poi il modello, e si torna qui con il promemoria
aggiornato.

**Dettaglio** — ricetta, partecipanti, stato, orario (se impostato) e
quantità totali aggregate per alimento; le azioni disponibili dipendono
dallo stato.

## Orario del pasto (dall'11 settembre 2026)

Ogni pasto vero del planner può avere un orario opzionale
(`planner_pasti.orario`, stesso formato `HH:MM` e stesso CHECK già usato da
`turno_modello_pasti.orario`). Si imposta in un passo dedicato fra la
scelta dei profili partecipanti e il salvataggio (`▶️ Continua` sulla
schermata dei profili, poi `🕐 Orario di <tipo>`), con un **default
proposto ma mai imposto**:

1. se quel giorno ha un'assegnazione turno con un orario per lo stesso tipo
   di pasto e uno dei profili scelti, quello vince (`turni::orario_suggerito_da_turno`);
2. altrimenti i default fissi per tipo (`turni::orario_default_per_tipo`):
   colazione 07:00, spuntino mattina 10:30, pranzo 13:00, spuntino
   pomeriggio 16:30, cena 20:00 — "altro" non ne ha nessuno, si scrive o si
   salta come sempre.

L'orario scritto a mano accetta anche una cifra sola per l'ora ("7:30"),
normalizzata a due cifre — stessa validazione di `turni::valida_orario`,
condivisa fra i due moduli invece di duplicata.

**Incoerenza con la routine**: se il turno assegnato quel giorno segna
"saltato" il tipo di pasto che si sta per pianificare, un avviso (non un
blocco) chiede conferma prima di salvare — la pianificazione può sempre
fare override della routine, vedi `docs/moduli/turni-e-routine.md`.

## La settimana si crea da sola

Non esiste una creazione manuale del planner: la settimana nasce alla prima
apertura. È una scelta deliberata per non aggiungere un concetto in più a chi
usa il bot — «creare un planner» sarebbe un passaggio che non decide niente.

## Tabelle

```text
planner_alimentari                  periodo, proprietario, spazio
planner_pasti                       data, tipo, ricetta + snapshot, stato, orario (dall'11/9/2026)
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
