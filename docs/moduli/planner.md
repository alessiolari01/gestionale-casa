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

**Dal 16 settembre 2026** la schermata Settimana porta anche
`🛒 Lista della spesa`, chiesto da Alessio: è pianificando i pasti che viene
in mente cosa manca, e prima bisognava risalire fino a `🍽️ Alimentazione`.
Il pulsante usa un callback dedicato (`lista_spesa:menu:planner`), così
`⬅️ Indietro` dalla lista riporta qui e non al menù Alimentazione (C3) —
vedi `docs/moduli/lista-spesa.md`.

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

## Pasti e scorte (17 settembre 2026)

Da quando la lista della spesa sottrae quello che c'è in casa, qualcuno deve
anche toglierlo quando lo si mangia: altrimenti dopo una settimana il bot
direbbe "hai la pasta" di una pasta già cucinata. Dettagli in
`docs/moduli/dispensa.md`; qui quello che cambia nel planner.

- **`🍲 Segna come preparato`** nel dettaglio di un pasto pianificato: gli
  ingredienti escono dalle scorte in quel momento. **Non è un nuovo stato**:
  il pasto resta pianificato finché non lo si consuma (colonna
  `planner_pasti.preparato_il`), così i trigger di congelamento non
  cambiano. Il dettaglio mostra `🍲 preparato`.
- **`✅ Segna come consumato`**: se il pasto non era stato preparato, le
  scorte si scalano adesso.
- **Orario passato**: le scorte si scalano da sole, ma lo stato del pasto
  non cambia — segnarlo consumato lo congelerebbe e non si potrebbe più dire
  che è stato saltato. Il dettaglio lo dice: `🥫 Ingredienti tolti dalle
  scorte da soli, a orario passato. Se lo segni saltato tornano indietro.`
- **`⏭ Segna come saltato`** dopo uno scarico automatico: gli ingredienti
  tornano esattamente com'erano (richiesta esplicita di Alessio). Dopo un
  `🍲 Preparato` il bot chiede "Gli ingredienti preparati li hai ancora?"
  (dal 17 settembre, consegna A): `🥫 Sì, rimettili nelle scorte` li
  restituisce e toglie il "preparato"; `🗑 No, usati o buttati` li lascia
  tolti.
- **`✏️ Modifica`** di un pasto che aveva già preso le sue scorte: tornano
  tutte, e se era preparato si riscalano con gli ingredienti nuovi. Per un
  pasto pianificato "sostituirlo con un altro" è proprio questo.

Il lato "lista della spesa": un pasto le cui scorte sono già state scalate
non chiede più niente alla lista, perché quegli ingredienti sono già stati
usati.

## Controllo delle scorte (consegna A, 17 settembre 2026)

Prima di `🍲 Segna come preparato` e `✅ Segna come consumato` il bot
controlla se in casa c'è tutto (`dispensa::mancanti_per_pasto`, senza
toccare niente). Se manca qualcosa è un'eccezione, e va confermata:

```
⚠️ In casa non c'è tutto per questo pasto:
• Pasta brisée: servono 200 g, ne hai 0 g
```

con `✅ Sì, li avevo comunque` (`planner:prepare:ok:` /
`planner:complete:ok:`) e `❌ Annulla`. Confermando si scala quello che
c'è, e la mancanza resta annotata sul pasto.

Lo scarico automatico a orario passato non può chiedere niente a nessuno:
prende quello che c'è, annota la mancanza (`planner_pasti.scorte_mancanti`)
e lo dice all'apertura della lista e delle scorte ("⚠️ Pasti passati,
ingredienti tolti dalle scorte da soli. In casa non c'era tutto: …"). Il
dettaglio del pasto mostra "⚠️ In casa mancavano: …".

## Riportare a pianificato (consegna A, 17 settembre 2026)

Alessio ha segnato saltata una colazione per sbaglio e non poteva più
tornare indietro: il trigger del 31 agosto bloccava il pasto saltato in
tutto. Ora c'è **`↩️ Riporta a pianificato`**:

- **su un pasto saltato** toglie il salto (le scorte eventualmente
  restituite restano in casa: il pasto, di nuovo pianificato, le chiederà
  alla lista o allo scarico come qualunque altro);
- **su un pasto consumato**: se non era stato preparato, le scorte scalate
  al consumo tornano in casa; se era preparato restano tolte (il cibo era
  già cucinato) e il pasto torna "pianificato, preparato".

La migration della consegna A ricrea i due trigger di congelamento: tutto il
resto del pasto (ricetta, giorno, tipo, ordine) resta bloccato.

## Icone e dettaglio (consegna A, 17 settembre 2026)

- Nell'elenco del giorno un pasto preparato ha `🍲` (prima restava `○`).
- Il dettaglio non ripete più il giorno della settimana (`📅 Mer 16 Set`),
  e sui consumati dice "🍲 Era stato preparato prima" se lo era.
- Scegliendo la ricetta di un pasto, i partecipanti ripartono con **il
  proprio profilo già spuntato** (quello con `utente_collegato_id`
  dell'utente), e se ne possono aggiungere altri.

## Sostituire un pasto (19 settembre 2026)

| stato | pulsante | cosa fa |
|---|---|---|
| da preparare | `🔁 Sostituisci` | dritti alla scelta della ricetta nuova (giorno, tipo, orario restano) |
| preparato | `🔁 Sostituisci` | prima chiede se gli ingredienti preparati ci sono ancora, poi come sopra |
| consumato | `✏️ Ho mangiato altro` | corregge quello che si è segnato (sotto) |
| saltato | — | c'è `↩️ Riporta a pianificato` |

Su un pasto preparato: "sì" rimette gli ingredienti nelle scorte e il pasto
torna da preparare; "no, usati o buttati" li lascia tolti ma stacca i
movimenti dal pasto (`dispensa::dimentica_scarico_pasto`), così il piatto
nuovo prenderà i suoi. `✏️ Modifica` resta per orario, partecipanti e tipo.

Nell'elenco del giorno ogni pasto ha **un'icona sola**, quella dello stato:
`🍲 Colazione · Frittata`. Con anche l'icona del tipo (`🍲 ☕`) le due
stavano attaccate e si confondevano.

## Correggere un pasto consumato (17 settembre 2026)

Chiesto da Alessio: "avevo pianificato la pasta, ho mangiato la pizza". Un
pasto consumato resta congelato, ma nel suo dettaglio c'è **`🔁 Sostituisci`**:
si sceglie il pasto mangiato davvero (stesso giorno, tipo, orario e
partecipanti già compilati, la ricetta da scegliere) e, salvando:

1. il nuovo pasto viene creato;
2. il vecchio restituisce tutte le sue scorte e **viene eliminato** — l'unico
   punto del bot che elimina un pasto consumato, e solo su richiesta
   esplicita: il congelamento serve a non riscrivere la storia per sbaglio,
   non a impedire di correggerla;
3. il nuovo diventa consumato e scala le sue scorte.

Se il passo 2 fallisse dopo il salvataggio del nuovo pasto, il messaggio lo
dice ("controlla il giorno") invece di tacere.

## Da dove si arriva (17 settembre 2026)

La settimana ricorda se ci si è arrivati dalla lista della spesa
(`planner:menu:lista`, pulsante `📅 Planner` della lista): in quel caso
`⬅️ Indietro` e `🛒 Lista della spesa` riportano alla lista (C3), invece
di aprire un nuovo giro. Il pulsante del menù Alimentazione usa
`planner:menu:alimentazione`, che azzera questa memoria. Il semplice
`planner:menu`, usato da tutte le schermate interne del planner, non la
tocca.

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
