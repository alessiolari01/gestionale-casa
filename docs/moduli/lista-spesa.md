# Lista della spesa

**Scritta l'8 settembre 2026, collaudo dal vivo su Telegram fatto per la
prima versione (aggregazione dal planner, voci manuali libere, comprato
congelato).** Il collaudo dal vivo della ricerca nel catalogo (9 settembre
2026) ha trovato e fatto correggere un'icona duplicata (§ Aggiunta dal
catalogo). Un secondo giro di collaudo lo stesso giorno, dopo che le prime
tre correzioni (refresh automatico alla deselezione, bottone "🔄 Aggiorna
lista" condizionale, assenza di paginazione) erano già state confermate dal
vivo, ha portato tre ulteriori correzioni: la fusione anche quando si
*spunta* una voce residua (non solo quando la si deseleziona), la
segnalazione di un eccesso quando il fabbisogno reale scende sotto quanto
già comprato, e un ordine della lista indipendente da `comprato` con un modo
per riordinarla a piacere — confermate dal vivo lo stesso giorno, con due
difetti estetici trovati e corretti (etichetta dell'eccesso troncata,
disallineamento nella schermata di riordino — quest'ultimo ha portato anche
alla nuova convenzione **C15**, `docs/convenzioni-telegram.md`). Un quarto
giro di collaudo (10 settembre 2026) ha trovato che un refresh qualunque —
non solo quello scatenato dal deseleziona — rimescolava un ordine già
sistemato a mano, e ha aggiunto due mancanze: l'unità di misura andava
sempre scritta anche per un'aggiunta dal catalogo (ora usa quella
predefinita dell'alimento/prodotto, sovrascrivibile), e non si poteva
rimuovere né una voce manuale né un'aggiunta dal catalogo. Queste ultime
correzioni sono scritte e testate in locale, **non ancora ricollaudate dal
vivo**. Resta non ancora collaudato anche il profilo alimentare automatico
(`docs/moduli/profili-e-porzioni.md`, stesso giro di feedback, non riguarda
questo modulo direttamente).
`src/modules/lista_spesa.rs`, raggiungibile da
`🍽️ Alimentazione → 🛒 Lista della spesa`.

Aggrega automaticamente gli ingredienti dei pasti pianificati in un
intervallo di date scelto dall'utente, indipendente dalle settimane del
planner, e permette di aggiungere voci a mano e spuntarle mentre si fa la
spesa.

## Una sola lista, con un intervallo proprio

Stesso principio del planner (`spazio_id` nullable, una lista attiva per
spazio o personale): non esiste una creazione manuale, la lista nasce alla
prima apertura con un intervallo di default (oggi + 6 giorni), modificabile
in qualunque momento con `🗓️ Cambia intervallo`.

L'intervallo della lista **non è legato alle settimane del planner**: il
planner crea una riga `planner_alimentari` per settimana, ma la lista
aggrega su *tutti* i `planner_alimentari` non archiviati dello stesso
spazio/proprietario la cui `data_pasto` ricade nell'intervallo scelto,
qualunque settimana attraversi.

## Cosa aggrega

Da `planner_pasto_ingredienti_snapshot` (via `planner_pasti` →
`planner_alimentari`), solo le righe che soddisfano tutte queste condizioni:

- il pasto è `stato = 'pianificato'`, non saltato (`saltato_il IS NULL`), non
  completato (`completato_il IS NULL`) — un pasto saltato non contribuisce,
  decisione esplicita: non si compra qualcosa per un pasto che non si farà;
- `quantita_finale_snapshot IS NOT NULL` — le righe con quantità finale nulla
  sono ingredienti esclusi da quel profilo per quel pasto, non un valore da
  sommare come zero;
- `data_pasto` dentro l'intervallo della lista.

## Conversione di unità

Per ogni riga aggregata, `unita_misura.famiglia_conversione` decide se
convertire: `massa` e `volume` si sommano nell'unità-base della famiglia
(`g` per la massa, `ml` per il volume — fattore 1/1 per definizione); un
simbolo senza famiglia (`pz`, `cucchiaio`, `cucchiaino`, `qb`) o non presente
in `unita_misura` non si converte e si aggrega per simbolo esatto, separato
da qualunque altro simbolo.

La chiave di aggregazione è l'alimento (per id se presente, altrimenti nome
normalizzato) più l'unità risultante dopo l'eventuale conversione: due
ingredienti diversi nella stessa unità restano voci separate, lo stesso
ingrediente in unità di famiglie diverse resta separato.

## Voci generate e manuali

Ogni voce (`liste_spesa_voci`) ha `origine` `generato` o `manuale`. Le
manuali sono testo libero puro (nessun collegamento al catalogo, mai
sommate a nient'altro): descrizione obbligatoria, quantità e unità
opzionali. Da non confondere con le aggiunte dal catalogo (sezione
seguente), che pur diventando anch'esse righe `generato` seguono un
percorso e una tabella diversi.

## Aggiunta dal catalogo (9 settembre 2026)

Secondo miglioramento deciso con Alessio dopo il primo collaudo dal vivo:
"➕ Aggiungi voce manuale" offre ora, prima del testo libero, la scelta
"🔎 Cerca nel catalogo" — cerca sia alimenti generici (es. "Pasta") sia
prodotti commerciali specifici (es. "Pasta De Cecco", tabella
`prodotti_alimentari`), mostrando i risultati come pulsanti distinti: un
alimento usa il nome così com'è (porta già la propria icona di categoria
incorporata, es. "🌾 Pasta" — un `🥕` fisso aggiunto qui sopra duplicava
l'icona, bug trovato da Alessio collaudando dal vivo e corretto lo stesso
9 settembre), un prodotto usa `🏷️ {marca} {nome commerciale}`. Scelto un
risultato, chiede la quantità — sempre richiesta, niente "➖ Senza
quantità", perché serve a sommare. Se la ricerca non trova nulla, resta
disponibile "📝 Voce libera" (il flusso testo-libero di sempre, invariato).

**Unità predefinita (10 settembre 2026)**: chiesto da Alessio dopo un
collaudo dal vivo, per non dover riscrivere l'unità di un alimento già noto
al catalogo. Se l'alimento ha un'unità predefinita (`unita_predefinita_id`)
o si tratta di un prodotto (`unita_confezione_id`, sempre presente), basta
scrivere il numero (es. "500") e si usa quella; scrivere comunque un'unità
(es. "500 ml") la sovrascrive per quella singola aggiunta. Un alimento
senza unità predefinita si comporta come la voce libera: l'unità va
scritta. Dominio puro in `valida_quantita_con_default`, accanto a
`valida_quantita_manuale` (invariata, resta quella della voce libera).

**Due esiti diversi a seconda di cosa si sceglie** (dominio puro in
`Identita`, terza variante `Prodotto(i64)` accanto ad `Alimento(i64)` e
`Nome(String)`):

- un **alimento generico** (es. "Pasta") si **somma** al fabbisogno già
  calcolato dal planner per lo stesso alimento — l'esempio di Alessio: 200 g
  di pasta per le ricette pianificate + 50 g aggiunti a mano danno 250 g in
  un'unica riga, non due;
- un **prodotto commerciale specifico** (es. "Pasta De Cecco") resta sempre
  una **riga separata e distinta**, anche se collegato allo stesso alimento
  generico richiesto altrove — mai un merge, per non confondere "mi serve
  della pasta" con "voglio comprare proprio quella marca" (decisione
  esplicita presa con Alessio).

**Non sono uno snapshot**: a differenza delle righe `generato` pure-planner
(cancellate e rigenerate da zero a ogni refresh), le aggiunte dal catalogo
vivono nella propria tabella (`liste_spesa_aggiunte_catalogo`) e
partecipano di nuovo ogni volta al calcolo di `🔄 Aggiorna lista`
(`aggiorna_lista` unisce le righe del planner con quelle di questa tabella
prima di aggregare) — restano "vive" attraverso ogni refresh, finché non
vengono coperte da una voce comprata (stesso congelamento di sempre: una
riga segnata comprata resta congelata, e se serve dell'altro compare una
voce nuova per la differenza).

Salvata l'aggiunta, la lista si aggiorna subito (chiamata automatica ad
`aggiorna_lista`) così la somma o la nuova riga compaiono senza dover
premere "🔄 Aggiorna lista" a mano.

Ricerca in `cerca_nel_catalogo` (dedicata a questo modulo, stesso schema di
visibilità di `ricette::search_food_choices` ma non riusata da lì: quella
funzione non restituisce i prodotti come risultati distinti, solo come
motivo per cui un alimento compare in ricerca). Nessuna paginazione vera,
un `LIMIT` come "top N" per ciascuna delle due ricerche (alimenti e
prodotti), stesso approccio già in uso in `ricette.rs`.

La rimozione di un'aggiunta è descritta più sotto (§ Rimozione di voci
manuali e aggiunte dal catalogo).

## Segna comprato

Flag booleano per riga intera, non quantità parziale: non c'è modo di dire
"ho comprato solo metà" di una voce. Una volta `comprato = 1`, un trigger a
database impedisce di modificare quantità/unità/descrizione/origine — solo
il toggle (tornare a `comprato = 0`) resta permesso, stesso principio del
congelamento di `planner_pasti`.

**Due casi eccezionali in cui il ricalcolo scatta da solo**, entrambi
decisi con Alessio il 9 settembre 2026 dopo averli visti dal vivo, entrambi
**solo** per le voci `generato` (una voce manuale non ha nulla con cui
fondersi e non viene mai toccata da un refresh, nemmeno indiretto):

- **deselezionare**: togliere la spunta a una voce `generato` la rimette
  disponibile al refresh, ma se nel frattempo un altro pasto aveva già
  prodotto una riga nuova per la differenza (vedi sotto), restavano due
  righe frammentate dello stesso alimento finché non si premeva "🔄
  Aggiorna lista" a mano — visto con "Pasta · 50 g" due volte invece di
  "Pasta · 100 g". `toggle_comprato` richiama `aggiorna_lista` subito dopo
  aver tolto la spunta;
- **selezionare**: spuntare proprio quella riga residua (invece di
  deselezionare l'altra) lasciava lo stesso problema al contrario — due
  righe comprate separate per sempre, dato che `aggiorna_lista` non tocca
  mai le voci comprate. `fondi_comprate_se_serve` le fonde in una sola:
  cerca un'altra voce `generato` già comprata con la stessa identità e
  unità, somma le quantità nella voce appena spuntata ed elimina l'altra.
  Il trigger di congelamento impedisce di cambiare `quantita` mentre
  `comprato = 1`, quindi la funzione passa da `comprato = 0` a `1` due
  volte con l'aggiornamento della quantità in mezzo (l'unico modo per
  cambiarla restando dentro la regola, dato che il toggle stesso resta
  sempre permesso), dentro un'unica transazione.

## Eccesso quando il fabbisogno scende sotto il comprato

Una voce comprata resta **sempre** congelata — nessuna correzione
automatica della quantità, per lo stesso principio del congelamento. Ma se
il fabbisogno reale scende dopo l'acquisto (un pasto tolto dal planner, una
ricetta ridotta), l'utente deve poterlo sapere: `eccessi_comprati` confronta
il fabbisogno grezzo (`fresche_grezze`, *prima* di sottrarre il comprato,
condivisa con `calcola_fresche`) con quanto è già segnato comprato per
quell'identità; se il comprato supera il fabbisogno, la differenza è
l'eccesso. `calcola_eccessi` è dominio puro, testato senza database.

La lista lo mostra su ogni voce coinvolta, su una riga a parte dentro lo
stesso pulsante (`⚠️ 150 g in eccesso` **a capo**, non accodato con " · ":
su una riga sola Telegram tronca il testo con "…" invece di andare a capo
da solo, visto da Alessio dal vivo — un "\n" fa occupare al pulsante una
riga in più invece di tagliare) più un avviso generale in testa alla
schermata quando c'è almeno un eccesso — non un'azione da compiere, solo
un'informazione: sta all'utente decidere cosa farne (usarlo comunque,
tenerlo per un pasto futuro...).

## Aggiornamento esplicito, mai automatico

`🔄 Aggiorna lista` ricalcola **solo** le voci `origine = 'generato' AND
comprato = 0`: le cancella e re-inserisce da zero il risultato fresco
dell'aggregazione. Le voci comprate (generate o manuali) e le voci manuali
non comprate non vengono mai toccate.

Se dopo un refresh serve più di un alimento già segnato comprato (es. un
nuovo pasto pianificato che usa lo stesso ingrediente), compare una voce
**nuova** non comprata per la sola differenza — non c'è merge con la vecchia
riga comprata, per non alterare mai una quantità già segnata come acquistata.

**Il bottone compare solo se serve davvero** (deciso con Alessio il 9
settembre 2026, stesso principio già in uso per "🔄 Aggiorna planner" sulle
ricette cambiate): `serve_aggiornamento` calcola il fresco dell'aggregazione
con la stessa logica di `aggiorna_lista` (estratta in `calcola_fresche`,
condivisa dalle due) e lo confronta con le voci generate non ancora comprate
già in lista. Se coincidono, "🔄 Aggiorna lista" non compare — e il testo
"Nessuna voce nella lista" cambia di conseguenza quando non c'è nulla da
generare (nessun pasto pianificato, nessuna aggiunta dal catalogo).

## Ordine indipendente da comprato, e riordino manuale

Prima l'ordine dipendeva da `comprato ASC`: spuntare una voce la faceva
saltare in fondo alla lista, sotto le voci ancora da comprare — visto da
Alessio dal vivo e non voluto. Dal 9 settembre 2026 `carica_voci` ordina
per `ordinamento ASC, id ASC`, una colonna indipendente dallo stato
comprato: spuntare o deselezionare una voce non le cambia più posizione.

Una nuova voce (generata da un refresh, manuale, o dal catalogo) prende
sempre il prossimo `ordinamento` disponibile — in coda, mai in mezzo a un
ordine che l'utente ha già sistemato a mano. Le righe rigenerate a ogni
refresh finiscono anch'esse in coda: solo le voci comprate o manuali, che
il refresh non tocca, mantengono una posizione stabile nel tempo.

**"↕️ Riordina lista"** (visibile solo con più di una voce) apre una
modalità dedicata dove ogni voce mostra `⬆️`/`⬇️` invece del checkbox —
`sposta_voce` scambia l'`ordinamento` con la voce vicina in quella
direzione, indipendentemente dal fatto che sia comprata o meno. "✅ Fine
riordino" torna alla schermata normale.

**Un refresh qualunque non deve rimescolare l'ordine** (trovato da Alessio
il 10 settembre 2026, non solo nel caso specifico del deseleziona: capitava
con *ogni* `aggiorna_lista`, perché le voci generate non comprate vengono
sempre cancellate e re-inserite da zero). Corretto rileggendo
l'`ordinamento` delle voci generate non comprate **prima** di cancellarle,
e riassegnandolo alla stessa identità nel fresco appena calcolato: solo
un'identità davvero nuova (mai vista tra le voci generate non comprate)
prende un ordinamento nuovo, in coda. Un refresh che non cambia nulla di
reale, quindi, non cambia nemmeno l'ordine — coerente con `serve_aggiornamento`.

## Rimozione di voci manuali e aggiunte dal catalogo (10 settembre 2026)

Chiesto da Alessio dopo un collaudo dal vivo: prima non si poteva
rimuovere né una voce manuale né un'aggiunta dal catalogo, una volta
inserite. Le righe `generato` pure-planner **non sono rimovibili**: le
gestisce il planner (pianificare/rimuovere un pasto), non una rimozione
manuale dalla lista.

"🗑️ Rimuovi voci" (visibile solo se c'è qualcosa da rimuovere) apre una
schermata dedicata con un pulsante per ogni voce manuale e ogni aggiunta
dal catalogo di questa lista (`voci_rimovibili`), comprate o no: il
congelamento protegge la *quantità* di una voce comprata, non la sua
esistenza, quindi anche una voce manuale già comprata resta rimovibile.

- **Voce manuale**: `rimuovi_voce_manuale` la cancella per sempre — non ha
  nulla che la rigeneri.
- **Aggiunta dal catalogo**: `rimuovi_aggiunta_catalogo` cancella la riga
  in `liste_spesa_aggiunte_catalogo`, poi la lista si aggiorna subito
  (stesso trattamento di quando si aggiunge). La riga `generato`
  eventualmente già in lista non sparisce da sola: il refresh la ricalcola
  secondo il fabbisogno rimasto — torna al solo fabbisogno del planner se
  ce n'è ancora, sparisce del tutto se non ne resta nessuno.

## Schermate

**Principale** — intervallo, conteggio comprate/totale, avviso generale se
c'è un eccesso, **tutte** le voci come pulsanti `✅`/`☐` (toggle al tocco,
con l'eventuale eccesso sul pulsante), **senza paginazione**: eccezione
esplicita a C6 (`docs/convenzioni-telegram.md`), l'unica lista del bot che
mostra tutto insieme, perché l'utente deve vedere l'intera lista per
decidere cosa prendere prima e cosa dopo. `🔄 Aggiorna lista` (solo se
serve davvero, vedi sopra), `➕ Aggiungi voce manuale`, `↕️ Riordina lista`
(con più di una voce), `🗑️ Rimuovi voci` (solo se c'è qualcosa da
rimuovere), `🗓️ Cambia intervallo`.

**Riordina lista** — ogni voce come `⬆️ | etichetta | ⬇️`, **sempre tre
pulsanti nello stesso ordine**: prima le frecce assenti alle estremità
spostavano l'etichetta di colonna riga per riga, un disallineamento visto
da Alessio dal vivo. Alle estremità la freccia resta al suo posto ma non fa
nulla (`lista_spesa:noop`), stesso trattamento del contatore di pagina non
premibile altrove nel bot. `✅ Fine riordino` per tornare alla schermata
principale.

**Rimuovi voci** — un pulsante `🗑️` per ogni voce manuale e aggiunta dal
catalogo. **Dall'11 settembre 2026** (nuova convenzione C16 di
`docs/convenzioni-telegram.md`, punto 9 del collaudo dal vivo di Turni e
routine): il tocco non elimina più subito, apre una schermata "⚠️
Eliminare questa voce definitivamente? Non si può recuperare." con
`✅ Sì, elimina` / `❌ Annulla` — prima l'eliminazione era immediata, senza
nessuna conferma. `⬅️ Indietro` per tornare alla schermata principale.
Dopo l'ultima rimozione, se non resta più nulla da rimuovere,
`mostra_dopo_rimozione` porta direttamente alla lista principale invece di
ripresentare "🗑️ Rimuovi voci" ormai vuota (trovato da Alessio dal vivo il
10 settembre 2026: quella schermata vuota era un vicolo cieco che
richiedeva comunque "⬅️ Indietro" per uscirne).

**Aggiungi voce manuale** — input ibrido in due passi: descrizione libera
(testo), poi quantità+unità scritte a mano (es. "500 g") oppure `➖ Senza
quantità`. Sessione dedicata (`ListaSpesaSessionStore`, dentro
`lista_spesa.rs`), stesso schema di `DistribuzioneSessionStore`.

**Cambia intervallo** — due passaggi sul calendario di `modules::calendario`
(data di inizio, poi data di fine — i giorni prima dell'inizio sono
bloccati).

## Tabelle

```text
liste_spesa                       intervallo, proprietario, spazio
liste_spesa_voci                  voci generate o manuali, comprato/comprato_il, ordinamento
liste_spesa_aggiunte_catalogo     aggiunte dal catalogo, vive attraverso ogni refresh
```

Vedi `docs/database.md` per i campi.

## Fuori scope

Non modella la dispensa, i prezzi, o la scelta del formato/confezione da
acquistare: quello è il futuro modulo Acquisti, che dirà *quale prodotto o
confezione comprare*, mentre questa lista dice solo *cosa serve* — vedi
`docs/previsto/lista-della-spesa.md`, sezione "Relazione con Acquisti".
