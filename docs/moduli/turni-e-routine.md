# Turni e routine

**Prima fetta scritta il 10 settembre 2026** (design di dettaglio deciso lo
stesso giorno), `src/modules/turni.rs`, raggiungibile da `🍽️ Alimentazione →
📋 Turni e routine`. **Non ancora collaudata dal vivo su Telegram**: nessun
accesso a Telegram/S9 nel worktree in cui è stata scritta, solo pipeline
automatica in locale (`STATO.md`, sezione 2quinquies).

Specifica completa (comprese le parti non ancora costruite) in
`docs/previsto/turni-e-routine.md`.

## Il concetto che regge tutto: modello vs assegnazione

Un **modello** (es. "Chiusura", "Università") descrive una giornata tipo:
un nome scelto dall'utente e una lista di pasti di default, ognuno con
tipo pasto, orario suggerito, situazione, preparazione anticipata (con
nota) e nota libera.

**Assegnare** un modello a una data per un profilo alimentare **copia** i
pasti del modello in quel momento, esattamente come il planner copia una
ricetta in uno snapshot: il modello contiene i default, l'assegnazione è
una copia modificabile che non cambia mai il modello. Modificare o
rimuovere un pasto assegnato ("il 27 agosto cena fuori invece che a casa")
non tocca il modello; rinominare, archiviare o persino cancellare il
modello dopo non cambia le assegnazioni già fatte, perché portano il nome
del modello congelato in `modello_nome_snapshot`.

Un'assegnazione è sempre riferita a un **profilo alimentare** (la
persona), non all'account — stesso concetto di partecipante già usato dal
planner — ed è unica per profilo e data: assegnare di nuovo sullo stesso
giorno richiede una conferma esplicita di sostituzione (l'assegnazione
precedente viene eliminata e ricreata, mai fusa).

## Vocabolario riusato dal planner

Il tipo di pasto (colazione / spuntino mattina / pranzo / spuntino
pomeriggio / cena / altro) è lo stesso enum del planner
(`planner_alimentare::MealType`), non uno nuovo — le icone
(`meal_emoji`) sono una piccola copia locale delle stesse di
`MealType::emoji`, perché quel metodo è privato al suo modulo e non vale
la pena accoppiare i due moduli per riusarlo.

La **situazione** (casa / lavoro / fuori / saltato / altro) è invece un
concetto nuovo di questo modulo (`Situazione`), che il planner oggi non
modella per un pasto non ancora pianificato — è uno dei motivi per cui il
collegamento col planner resta un suggerimento testuale, non una
scrittura automatica (vedi sotto).

## Schermate

**`📋 Turni e routine`** — elenco dei modelli (paginato da `modules::liste`
sopra la soglia), `➕ Nuovo modello`, `📅 Vedi/modifica assegnazione`.

**Dettaglio modello** — i pasti del modello come pulsanti (tipo, orario,
situazione, eventuale preparazione — su più righe per lo stesso pulsante,
convenzione C15, perché sono più di due informazioni), `➕ Aggiungi pasto`,
`📅 Assegna a una data`, `✏️ Rinomina`, `🗑 Archivia`. Toccare un pasto lo
rimuove dal modello (nessuna conferma, stesso stile di
`lista_spesa::rimuovi_voce_manuale`).

**➕ Aggiungi pasto** — flusso guidato: tipo pasto da bottoni, orario
libero `HH:MM` o salta, situazione da bottoni, preparazione anticipata
sì/no (se sì, nota di preparazione libera o salta), nota libera del pasto
o salta. Ogni passo si può annullare (`❌ Annulla`) o interrompere con
`/annulla`, che riporta al dettaglio del modello.

**📅 Assegna a una data** — scelta del profilo (stessa visibilità già
usata dal planner per i partecipanti), poi calendario (`modules::calendario`,
marcato con `•` sui giorni che hanno già un'assegnazione per quel
profilo); se il giorno scelto ha già un'assegnazione, una conferma
esplicita chiede se sostituirla.

**📅 Vedi/modifica assegnazione** — stesso percorso (profilo → calendario
marcato), ma arriva al dettaglio dell'assegnazione: l'elenco dei suoi
pasti, ognuno apribile per cambiare situazione, orario, preparazione
anticipata (con nota) o nota libera, oppure rimuoverlo — senza mai toccare
il modello di origine.

## Relazione col planner: solo un suggerimento, mai una scrittura

Deciso esplicitamente con Alessio: il turno **non crea mai righe vere** in
`planner_pasti`. Aprire la schermata "Giorno" del planner
(`planner_alimentare::planner_show_day`) chiama
`turni::info_giorno_per_planner`, che — se esiste un'assegnazione turno
per quella data nello stesso spazio — antepone un blocco di sola lettura:

```text
📋 Turno assegnato:
🔸 Alessio: 🍝 Pranzo 12:00 (casa)
🔸 Alessio: 🍽️ Cena 19:00 (lavoro, da preparare prima)
```

Solo i pasti il cui **tipo** non ha già un pasto vero pianificato per
quella data compaiono nel blocco (calcolato da `pasti_da_segnalare`,
dominio puro testato senza database): appena l'utente pianifica per
davvero "pranzo" quel giorno, il suggerimento per "pranzo" sparisce da
solo, senza bisogno di nasconderlo a mano.

**Perché per spazio e non per singolo pasto/partecipante**: la schermata
"Giorno" del planner non ha oggi un concetto di "profilo che sto
guardando" — un pasto pianificato può avere più partecipanti insieme, e
il modulo planner non modella "fuori casa" o "saltato" per un pasto non
ancora creato. Costruire quella distinzione avrebbe richiesto toccare la
logica già in produzione del planner (segnalazione ricetta cambiata,
congelamento a completamento, esito "saltato") per un beneficio che questa
fetta non richiede. La modifica a `planner_alimentare.rs` è quindi
volutamente minima: sei righe in `planner_show_day` che leggono e
mostrano il blocco testuale, nessun bottone che crei pasti da solo.

## Tabelle

```text
turno_modelli              nome, proprietario, spazio, archiviato
turno_modello_pasti        tipo, orario, situazione, preparazione, nota
turno_assegnazioni         modello+nome congelato, profilo+nome congelato, data
turno_assegnazione_pasti   copia dei pasti al momento dell'assegnazione
```

Schema completo con vincoli in `docs/database.md`.

## Cosa resta fuori da questa fetta

- **Condivisione/copia di un modello** nello spazio, copia indipendente,
  invio a un altro utente — la spec (`docs/previsto/turni-e-routine.md`)
  li elenca come candidati naturali, ma non sono ancora costruiti.
- **Reminder alla creazione** — l'infrastruttura reminder (email,
  scheduler) non esiste ancora nel progetto: arriverà in un blocco
  successivo, deciso con Alessio.
- **Riordino dei pasti di un modello** — in questa fetta si aggiunge in
  coda e si rimuove; riordinare l'ordine di visualizzazione non è ancora
  costruito.
- **Badge "🆕"** (`novita::REGISTRO`) — lasciato fuori per restare nei
  tempi di questa fetta, come esplicitamente concesso dalla richiesta.

## Scelte di design

- Modello e assegnazione appartengono allo spazio predefinito dell'attore
  corrente (`identity::current_actor().spazio_id`, mai nullo), esattamente
  come `planner_alimentari` e `liste_spesa`: nessuna schermata per
  scegliere lo spazio.
- L'orario è validato con una piccola reimplementazione di
  `spazi_membri::valid_time_strict` (privata al suo modulo): stessa scelta
  di piccola duplicazione già accettata altrove nel progetto (es.
  `lista_spesa::clausola_visibilita_alimento`) piuttosto che accoppiare
  moduli per una manciata di righe.
- Rimuovere un pasto (dal modello o da un'assegnazione) è immediato, senza
  un passo di conferma — stesso stile di `lista_spesa::rimuovi_voce_manuale`.
