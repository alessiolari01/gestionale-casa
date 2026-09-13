# Turni e routine

**Prima fetta scritta il 10 settembre 2026** (design di dettaglio deciso lo
stesso giorno), `src/modules/turni.rs`, raggiungibile da `🍽️ Alimentazione →
📋 Turni e routine`. **Tredici correzioni scritte l'11 settembre 2026** dopo
il primo collaudo dal vivo su Telegram di Alessio, e **dieci ulteriori
correzioni scritte il 12 settembre 2026** dopo un secondo giro di collaudo
dal vivo — entrambe tutte discusse con lui prima di scriverle, dettagliate
sezione per sezione più sotto. **Le correzioni del secondo giro non sono
ancora state collaudate dal vivo**: scritte in un worktree isolato senza
accesso a Telegram/S9, solo pipeline automatica in locale.

Specifica completa (comprese le parti non ancora costruite) in
`docs/previsto/turni-e-routine.md`.

## Il concetto che regge tutto: modello vs assegnazione

Un **modello** (es. "Chiusura", "Università") descrive una giornata tipo:
un nome scelto dall'utente, **un profilo alimentare a cui appartiene fin
dalla creazione** (cambiato l'11 settembre 2026, vedi sotto), e una lista
di pasti di default, ognuno con tipo pasto, orario suggerito, situazione,
preparazione anticipata (con nota) e nota libera.

**Assegnare** un modello a una data **copia** i pasti del modello in quel
momento, esattamente come il planner copia una ricetta in uno snapshot: il
modello contiene i default, l'assegnazione è una copia modificabile che non
cambia mai il modello da sola. Modificare o rimuovere un pasto assegnato
("il 27 agosto cena fuori invece che a casa") non tocca il modello;
rinominare, archiviare o persino cancellare il modello dopo non cambia le
assegnazioni già fatte, perché portano il nome del modello congelato in
`modello_nome_snapshot` — a meno che l'utente non scelga esplicitamente
"🔄 Aggiorna assegnazione" (punto 10, vedi sotto).

Un'assegnazione resta riferita a un **profilo alimentare** (la persona),
non all'account — ma da questa correzione **il profilo non si sceglie più
al momento dell'assegnazione**: si eredita dal modello, che ne ha già uno.
Un'assegnazione è unica per profilo e data: assegnare di nuovo sullo stesso
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
sopra la soglia, ogni pulsante mostra anche il profilo del modello),
`➕ Nuovo modello`, `🗄 Modelli archiviati` (punto 8), `📅 Vedi/modifica
assegnazione`.

**Dettaglio modello** — i pasti del modello come pulsanti (tipo, orario,
situazione, eventuale preparazione — su più righe per lo stesso pulsante,
convenzione C15), ordinati **per orario crescente, i senza orario in fondo**
(punto 1). Toccare un pasto apre un dettaglio con modifica di situazione,
orario, preparazione (con nota) e nota libera, più un'esplicita
eliminazione con conferma (punto 7 e 9 — prima toccare un pasto lo
eliminava subito). `➕ Aggiungi pasto` sparisce quando il modello ha già
tutti e sei i tipi (i 5 fissi più "altro", punto 12). `📅 Assegna a una
data` e `📤 Copia per un altro profilo` compaiono solo se il modello ha un
profilo impostato; altrimenti un pulsante `⚠️ Imposta profilo` lo chiede.

**Creazione guidata di un modello (punto 12)** — `➕ Nuovo modello` chiede
prima il **profilo** (punto 13, tra quelli visibili nello spazio corrente),
poi il nome. Creato il modello, parte subito il giro guidato dei 5 tipi
fissi **in ordine** (colazione → spuntino mattina → pranzo → spuntino
pomeriggio → cena): per ciascuno si sceglie prima la **situazione** (le
stesse cinque di sempre, incluso "saltato") e solo se non è "saltato" si
prosegue con orario, preparazione e nota — un pasto "saltato" non ha
bisogno di nessuna di queste informazioni e si salva subito. Finiti i 5
tipi fissi, la schermata segnala che si può aggiungere anche un pasto
"altro" separatamente, mai obbligatorio. Lo stesso meccanismo guidato
riprende da solo ogni volta che si preme `➕ Aggiungi pasto` su un modello
che ha ancora dei tipi fissi mancanti (non serve aver creato il modello lì
per lì): la scelta del tipo sparisce dalla UI finché ne manca uno fisso,
e ricompare (limitata ad "altro") solo quando i 5 sono già tutti presenti.

**🗄 Modelli archiviati (punto 8, esteso il 12 settembre 2026 — punto D)** —
elenco dei modelli con `archiviato = 1`, ognuno con `♻️ Ripristina` e
`🗑 Elimina definitivamente`, più `🗑️ Elimina tutti` per l'intera lista
quando non è vuota. Prima del punto 8 un modello archiviato spariva per
sempre nei fatti: non c'era nessuna schermata per rivederlo. Le due nuove
azioni sono permanenti e applicano C16 (schermata "sei sicuro?"): la
cascata su `turno_modello_pasti` è `ON DELETE CASCADE`, mentre le
assegnazioni già fatte con quel modello restano (`turno_assegnazioni.
modello_id` è `ON DELETE SET NULL`) con il nome congelato in
`modello_nome_snapshot` — lo stesso principio già usato per rinomina e
archiviazione.

**📤 Copia per un altro profilo (punto 13)** — crea un nuovo modello
indipendente con gli stessi pasti, intestato a un profilo diverso scelto
tra quelli visibili nello spazio (compresi profili di altri account, stessa
visibilità di sempre). Da quel momento modificare l'uno non tocca più
l'altro: nessun collegamento resta fra originale e copia, a differenza
dell'assegnazione (che invece ricopia dal modello ogni volta che lo si
richiede esplicitamente).

**📅 Assegna a una data** — dall'11 settembre 2026 il profilo non si
sceglie più qui (lo eredita il modello, punto 13): si va dritti al
calendario (`modules::calendario`, marcato con `•` sui giorni che hanno già
un'assegnazione per quel profilo); se il giorno scelto ha già
un'assegnazione, una conferma esplicita chiede se sostituirla. Se il
modello segna "saltato" un tipo di pasto che in quel giorno ha già un
pasto vero pianificato nel planner, l'assegnazione si ferma su una scelta
esplicita — tenere il pasto pianificato o eliminarlo (con la stessa
conferma del punto 9) — invece di procedere in silenzio (incoerenza 2 del
punto 12).

**📅 Vedi/modifica assegnazione** — invariato nella scelta (profilo →
calendario marcato, perché qui si può voler rivedere qualunque
assegnazione, non solo quelle di un modello specifico), arriva al
dettaglio dell'assegnazione: i suoi pasti (stesso ordine cronologico del
punto 1), ognuno apribile per cambiare situazione, orario, preparazione
anticipata (con nota) o nota libera, oppure eliminarlo con conferma
esplicita (punto 9) — senza mai toccare il modello di origine. Se il
modello di origine è cambiato dopo questa assegnazione (ed è di oggi o di
una data futura), compare `🔄 Aggiorna assegnazione` (punto 10): dichiara
cosa cambierebbe (pasti aggiunti/rimossi/modificati,
`turni::confronta_pasti_modello_assegnazione`) prima di ricopiare i pasti
attuali del modello, sostituendo qualunque modifica fatta finora
sull'assegnazione stessa. Dal 12 settembre 2026 (punto E) c'è anche
`🗑 Elimina assegnazione`, che elimina tutti i suoi pasti insieme (cascata
`ON DELETE CASCADE` su `turno_assegnazione_pasti`) con conferma esplicita
(C16); dopo l'eliminazione si torna alla scelta profilo di questa stessa
sezione, non al menù Turni generale.

**Assegnare un turno dalla schermata Giorno del planner (punto 3)** — un
pulsante `📅 Assegna un turno a questo giorno` in `planner_show_day` apre
lo stesso flusso di assegnazione saltando la scelta della data (già nota):
si sceglie il profilo, poi uno dei suoi modelli, e l'assegnazione avviene
subito su quella data. Dopo l'assegnazione si torna alla schermata Giorno
del planner con il promemoria aggiornato, non al dettaglio dell'assegnazione
di `turni.rs`.

**Dopo un'assegnazione completata, senza tornare dal planner (punto H, 12
settembre 2026)** — `esegui_assegnazione` (percorso diretto e percorso
dopo un conflitto risolto, `turni:assign:conflict:keep:`/
`:conflict:delete:yes:`) mostrava il dettaglio dell'assegnazione appena
creata anche quando l'utente non l'aveva chiesto. Ora torna al dettaglio
del **modello** di partenza quando non si arriva dal planner (che invece
continua a tornare alla schermata Giorno, invariato): il dettaglio
dell'assegnazione resta raggiungibile solo esplicitamente, da "📅
Vedi/modifica assegnazione".

**"🔄 Aggiorna assegnazione" anche nella schermata Giorno del planner
(punto I, 12 settembre 2026)** — prima il blocco informativo del turno lì
era solo testo, senza bottoni. Ora, per ciascuna assegnazione di quel
giorno (di uno o più profili dello spazio) con un aggiornamento
disponibile dal modello (stesso controllo di
`assegnazione_ha_aggiornamento_disponibile`), compare un bottone `🔄
Aggiorna {profilo}` — con il nome per distinguerli quando ce n'è più di
uno. Porta alla stessa schermata di conferma di "📅 Vedi/modifica
assegnazione"; dopo la conferma torna alla schermata Giorno del planner
(non al menù Turni), passando `data` nel callback
(`turni:assign:refresh:ask:{id}:planner:{data}`) esattamente come
`turni:assign:conflict:keep:` fa già per lo stesso scopo.

## Relazione col planner: solo un suggerimento, mai una scrittura automatica

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
solo. **Corretto l'11 settembre 2026 (punto 12)**: un pasto segnato
"saltato" non conta mai come "da segnalare", anche se non ha ancora un
pasto vero pianificato — è già una scelta esplicita di non mangiarlo, non
un buco da colmare.

**Incoerenza 1 (punto 12)**: se il turno assegnato quel giorno segna
"saltato" un tipo di pasto e l'utente prova comunque ad aggiungerne uno
vero nel planner, un avviso ("ℹ️ Il turno assegnato oggi non prevede
[tipo] per questo profilo. Vuoi pianificarlo comunque?") chiede conferma
prima di salvare — mai un blocco: la pianificazione può sempre fare
override della routine.

**Perché per spazio e non per singolo pasto/partecipante**: la schermata
"Giorno" del planner non ha oggi un concetto di "profilo che sto
guardando" — un pasto pianificato può avere più partecipanti insieme, e
il modulo planner non modella "fuori casa" o "saltato" per un pasto non
ancora creato. Costruire quella distinzione avrebbe richiesto toccare la
logica già in produzione del planner per un beneficio che questa fetta non
richiede.

## Le tredici correzioni dell'11 settembre 2026, in sintesi

1. **Ordine cronologico**: `lista_pasti_modello`/`lista_pasti_assegnati`
   ordinano per `(orario IS NULL), orario, id` invece che per
   `ordinamento, id` — i pasti senza orario vanno in fondo.
2. **Un solo pasto per tipo**: indici UNIQUE su
   `(modello_id, tipo_pasto)`/`(assegnazione_id, tipo_pasto)`, con un
   messaggio comprensibile (`TipoPastoGiaPresente`) al posto dell'errore
   grezzo del database.
3. **Assegnare un turno dalla schermata Giorno del planner** — vedi sopra.
4. **Orario del pasto vero del planner** — nuova colonna
   `planner_pasti.orario`, con un default proposto (mai imposto) preso dal
   turno assegnato o dai default fissi per tipo, vedi `docs/moduli/planner.md`.
5. **Indicatore visivo per i giorni con turno assegnato** nella settimana
   (`🗓️`) e nel calendario mensile del planner (`◆`, diverso da `•` dei
   pasti pianificati, C4).
6. **Orario senza zero iniziale**: `turni::valida_orario` accetta anche
   "7:30" e lo normalizza a "07:30"; riusata anche dal planner per il nuovo
   campo orario, invece di duplicare la validazione.
7. **Modifica di un pasto del modello uniformata a quella
   dell'assegnazione** — vedi "Dettaglio modello" sopra.
8. **Modelli archiviati con ripristino** — vedi "🗄 Modelli archiviati"
   sopra.
9. **Conferma esplicita prima di ogni eliminazione definitiva** — nuova
   convenzione C16 (`docs/convenzioni-telegram.md`), applicata qui alla
   rimozione di un pasto (modello o assegnazione) e in `lista_spesa.rs`
   alla rimozione di una voce; l'archiviazione di un modello non la
   richiede perché è reversibile (punto 8).
10. **"🔄 Aggiorna assegnazione"** — vedi "📅 Vedi/modifica assegnazione"
    sopra; nuova colonna `turno_assegnazioni.modello_aggiornato_il_snapshot`.
11. Default orari fissi per tipo di pasto — parte del punto 4.
12. **Guida obbligata alla creazione del modello, e coerenza turno↔planner**
    — vedi "Creazione guidata" sopra e le due incoerenze descritte.
13. **Il modello appartiene a un profilo, non l'assegnazione** — vedi
    "Il concetto che regge tutto" sopra; nuove colonne
    `turno_modelli.profilo_alimentare_id`/`profilo_nome_snapshot`, con
    backfill dei modelli storici al profilo "sé stesso" del proprietario.

## Le dieci correzioni del secondo collaudo dal vivo (12 settembre 2026), in sintesi

1. **Tre bug identici di ordine dei callback**: in `turni:model:copy:`,
   `turni:assignhere:` e `turni:model:setprofile:`, un controllo generico
   era scritto e verificato **prima** dei suoi corrispondenti più
   specifici (`...:pick:`/`...:do:`/`...:profile:`) nello stesso `if let`
   a catena — siccome le stringhe specifiche iniziano comunque per il
   prefisso generico, quest'ultimo le intercettava sempre per primo.
   Corretto spostando i tre controlli generici in fondo (stesso schema già
   corretto altrove nel file, es. `turni:mpasto:sit:set:` prima di
   `turni:mpasto:sit:`). Per "📤 Copia per un altro profilo" lo
   smistamento è stato estratto in un'unica funzione pura testabile
   (`parse_model_copy_callback`, con l'enum `ModelCopyCallback`), così
   l'ordine giusto non dipende più da come sono disposti gli `if let`.
2. **"❌ Annulla" dalla scelta tipo pasto con solo "Altro"** tornava
   all'elenco generale dei modelli invece che al modello di partenza:
   `avvia_prossimo_pasto`, nel ramo che offre solo "🍴 Altro", non salva
   un draft (nessun tipo ancora scelto), e il gestore generico di
   `turni:pasto:add:cancel` dipende dal draft per sapere a quale modello
   tornare. Corretto passando `modello_id` direttamente nel callback di
   *questa* schermata (`turni:pasto:add:cancel:{modello_id}`, nuovo
   parametro `cancel_callback` di `tipo_pasto_keyboard`), con un gestore
   nuovo che non dipende dal draft — gli altri punti in cui
   `turni:pasto:add:cancel` (senza id) è già usato correttamente restano
   invariati.
3. **Icona "Archivia" uniformata a 📦** in tutto il bot: `ricette.rs`
   (prima `🗄`) e `turni.rs` (prima `🗑`, la più fuorviante) allineati a
   `profili_alimentari.rs`/`miglioramenti.rs`, che la usavano già.
4. **Eliminazione definitiva nei "🗄 Modelli archiviati"** — vedi
   "🗄 Modelli archiviati" sopra.
5. **"🗑 Elimina assegnazione"** — vedi "📅 Vedi/modifica assegnazione"
   sopra.
6. **Salta la scelta profilo quando è ridondante**, in
   `profili_alimentari.rs`: se l'utente ha già un profilo "sé stesso"
   collegato, `foodprof:new` salta direttamente alla richiesta del nome
   di "➕ Persona senza account" (`send_new_person_prompt`, estratta da
   `send_new_profile_menu`) invece di mostrare una schermata con un solo
   bottone reale. Quando non ha ancora un profilo "sé stesso", la
   schermata di scelta resta con le due opzioni vere.
7. **Correzione testo**: "Esempio: Giulia" → "Esempio: Giorgia" nella
   richiesta del nome di un nuovo profilo (`profili_alimentari.rs`).
8. **Navigazione dopo un'assegnazione completata** — vedi "Dopo
   un'assegnazione completata, senza tornare dal planner" sopra.
9. **"🔄 Aggiorna assegnazione" nella schermata Giorno del planner** —
   vedi sopra, stessa sezione del punto precedente.
10. **Documentata, non costruita**: l'idea di sincronizzazione opzionale
    con diritto di veto del proprietario per le entità copiabili — vedi
    `docs/previsto/turni-e-routine.md`.

## Tabelle

```text
turno_modelli              nome, proprietario, spazio, archiviato, profilo (punto 13)
turno_modello_pasti        tipo, orario, situazione, preparazione, nota
turno_assegnazioni         modello+nome congelato, profilo+nome congelato, data,
                            snapshot dell'aggiornamento del modello (punto 10)
turno_assegnazione_pasti   copia dei pasti al momento dell'assegnazione/dell'ultimo aggiornamento
```

Schema completo con vincoli in `docs/database.md`.

## Cosa resta fuori da questa fetta

- **Condivisione di un modello nello spazio senza copiarlo, invio a un
  altro utente** — la copia indipendente (punto 13) è costruita, la
  condivisione live no.
- **Reminder alla creazione** — l'infrastruttura reminder (email,
  scheduler) non esiste ancora nel progetto: arriverà in un blocco
  successivo, deciso con Alessio.
- **Riordino manuale dei pasti di un modello** — dal punto 1 l'ordine è
  comunque cronologico per orario, quindi il bisogno di riordinare a mano è
  minore, ma non esiste un modo per due pasti allo stesso orario o per far
  passare "altro" prima di un tipo fisso.
- **Badge "🆕"** (`novita::REGISTRO`) — lasciato fuori per restare nei
  tempi di questa fetta e delle sue correzioni.

## Scelte di design

- Modello e assegnazione appartengono allo spazio predefinito dell'attore
  corrente (`identity::current_actor().spazio_id`, mai nullo), esattamente
  come `planner_alimentari` e `liste_spesa`: nessuna schermata per
  scegliere lo spazio.
- L'orario è validato da `turni::valida_orario`, riusata anche da
  `planner_alimentare` per il nuovo campo orario del punto 4 (cambiato
  l'11 settembre 2026: prima ogni modulo aveva la propria piccola copia
  della validazione, ora ce n'è una sola condivisa fra i due moduli che
  già si richiamano a vicenda).
- Il nome di un modello copiato per un altro profilo (`📤 Copia per un
  altro profilo`) include il nome del profilo di destinazione tra
  parentesi: lo stesso nome esatto nello stesso spazio violerebbe
  l'indice UNIQUE su `(spazio_id, nome_normalizzato)` già in produzione
  (migration originale, mai modificata).
