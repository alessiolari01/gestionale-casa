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
**Il 16 settembre 2026** sono arrivate tre aggiunte chieste da Alessio —
chiusura della spesa, intervallo che parte sempre almeno da oggi, accesso
dalla schermata Settimana del planner — distribuite lo stesso giorno e
collaudate dal vivo in parte. Da quel collaudo è nato **il giro del 17
settembre 2026**: lista al netto di quello che c'è in casa, aggiornamento
automatico con resoconto dei cambiamenti, avviso della data passata al
momento della scelta, pulsante per il planner, e la correzione del riordino
(posizioni doppie). **Il giro del 17 settembre è scritto e testato in locale,
collaudo dal vivo da fare.**

`src/modules/lista_spesa.rs`, raggiungibile da
`🍽️ Alimentazione → 🛒 Lista della spesa` oppure dal planner.

Aggrega automaticamente gli ingredienti dei pasti pianificati in un
intervallo di date scelto dall'utente, indipendente dalle settimane del
planner, e permette di aggiungere voci a mano e spuntarle mentre si fa la
spesa.

## Una sola lista, con un intervallo proprio

Stesso principio del planner (`spazio_id` nullable, una lista attiva per
spazio o personale): non esiste una creazione manuale, la lista nasce alla
prima apertura con un intervallo di default (oggi + 6 giorni), modificabile
in qualunque momento con `🗓️ Cambia intervallo`.

**La lista parte sempre almeno da oggi (16 settembre 2026)**, chiesto da
Alessio: prima un intervallo scelto la settimana precedente restava fermo
lì e continuava ad aggregare pasti ormai passati, che non si comprano più.
A ogni apertura `applica_inizio_da_oggi` porta l'inizio a oggi se è rimasto
indietro (dominio puro in `intervallo_da_oggi`, nessuna scrittura se non
serve); se anche la fine è passata, l'intervallo riparte come quello di
default, oggi + 6 giorni.

Un inizio **precedente a oggi resta possibile** — serve a recuperare i
pasti di ieri — ma è una scelta esplicita e viene trattata come tale.
**Dal 17 settembre 2026** (collaudo di Alessio, blocco B) l'avviso arriva
**al tocco del giorno**, non più alla fine dopo aver scelto anche la data di
fine: `⚠️ Mer 10 Set è prima di oggi.` con `✅ Tienila` (si passa alla data
di fine) e `📅 Scegli un'altra data` (si torna al calendario). L'avviso che
compariva alla fine è stato tolto, perché ripeteva una scelta già
confermata; resta la riga fissa in testa alla lista, che spiega perché la
lista non si sposta più da sola. Da quel momento lo spostamento automatico
si ferma (colonna `liste_spesa.inizio_manuale`), perché una scelta
esplicita non si corregge alle spalle di chi l'ha fatta. Scegliere di nuovo
un inizio da oggi in avanti lo riattiva.

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
l'eccesso. `calcola_eccessi` è dominio puro, testato senza database. **Dal
17 settembre 2026** il fabbisogno con cui si confronta è quello **al netto
delle scorte** (sezione seguente): quello che serve davvero.

La lista lo mostra su ogni voce coinvolta, su una riga a parte dentro lo
stesso pulsante (`⚠️ 150 g in eccesso` **a capo**, non accodato con " · ".
Fino al 17 settembre 2026 il codice mostrava il numero senza l'unità, al
contrario di quanto scritto qui: allineato alla documentazione.
su una riga sola Telegram tronca il testo con "…" invece di andare a capo
da solo, visto da Alessio dal vivo — un "\n" fa occupare al pulsante una
riga in più invece di tagliare) più un avviso generale in testa alla
schermata quando c'è almeno un eccesso — non un'azione da compiere, solo
un'informazione: sta all'utente decidere cosa farne (usarlo comunque,
tenerlo per un pasto futuro...).

## Al netto di quello che c'è in casa (17 settembre 2026)

Chiesto da Alessio: la lista deve dire **quanto serve comprare davvero**,
tenendo conto di dispensa, frigo e freezer (`docs/moduli/dispensa.md`). Il
fabbisogno si calcola così:

1. ingredienti dei pasti pianificati nell'intervallo + aggiunte dal
   catalogo (`fresche_grezze`) — **esclusi i pasti le cui scorte sono già
   state scalate** (preparati, o con l'orario passato): quegli ingredienti
   sono già stati usati, non si ricomprano;
2. **meno le scorte** dello spazio (`sottrai_scorte`, dominio puro), con la
   solita conversione di unità. Una richiesta di un prodotto specifico si
   copre solo con quel prodotto; una di un alimento generico con le scorte
   generiche e poi con quelle dei suoi prodotti (la pasta De Cecco va bene
   quando la ricetta chiede pasta), mai il contrario;
3. **meno quanto è già spuntato comprato** in lista (`sottrai_gia_comprato`).

L'ordine dei punti 2 e 3 è quello che ha risolto il difetto trovato da
Alessio collaudando la chiusura (blocco A): una voce spuntata e non ancora
chiusa non è ancora in casa, quindi le due sottrazioni non si sovrappongono;
chiudendo la spesa la voce esce dalla lista ed entra nelle scorte, e il
totale sottratto resta lo stesso. Prima la voce appena chiusa ricompariva
subito, identica, perché il pasto che la chiedeva c'era ancora e niente la
copriva più.

## Aggiornamento automatico, con resoconto (17 settembre 2026)

Nato dal collaudo: una despunta aveva fatto ripartire il ricalcolo e
svuotato di colpo la lista, senza dire niente. Alessio ha scelto: **la lista
si aggiorna da sola, anche togliendo e riducendo, ma dice sempre cosa ha
cambiato.**

- **Preferenza per persona**, attiva di default: `⚙️ Aggiornamento
  automatico` (`preferenze_utente.lista_spesa_aggiornamento_automatico`).
- **Quando**: all'apertura della lista — solo se c'è davvero qualcosa da
  cambiare (`serve_aggiornamento`), per non riscrivere le voci a ogni tocco
  — più i casi di prima (despunta di una voce generata, aggiunta o
  rimozione dal catalogo, chiusura della spesa). Con la preferenza spenta la
  despunta non ricalcola più niente: la lista cambia solo con
  `🔄 Aggiorna lista`, che compare quando serve.
- **Cosa dice**: in testa alla lista `🔄 Aggiornata da sola: +1 nuova, 2
  tolte, 1 ridotta.` (`riepilogo_modifiche`), e `📋 Ultimi cambiamenti` apre
  il dettaglio riga per riga (`riga_modifica`):
  ```
  ➕ 🌾 Pasta · 250 g
  ➖ 🥩 Sovracosce · 125 g — non serve più per i pasti pianificati
  🔽 🥛 Latte · 500 → 300 ml — ce l'hai già in casa
  🔼 🌾 Farina · 200 → 350 g
  ```
- **Come si confronta** (`confronta_totali`, dominio puro): i **totali** di
  ogni alimento prima e dopo, comprate e non comprate insieme — non le
  singole righe. Così spuntare o togliere la spunta, che fonde o separa
  righe dello stesso alimento senza cambiarne il totale, non produce un
  resoconto inutile.
- **Salvato a database** (`liste_spesa_aggiornamenti`,
  `liste_spesa_modifiche`, deciso con Alessio): "avvisare sempre" regge male
  se l'avviso può svanire con un riavvio del bot. Si registra solo un
  aggiornamento che ha cambiato qualcosa. Anche `🔄 Aggiorna lista` a mano
  registra e dice cosa ha fatto (`🔄 Lista aggiornata: …` oppure
  `🔄 Lista già aggiornata.`).

## Cosa fa un aggiornamento

Un aggiornamento (automatico o con `🔄 Aggiorna lista`) ricalcola **solo** le
voci `origine = 'generato' AND comprato = 0`: le cancella e re-inserisce da
zero il risultato fresco dell'aggregazione. Le voci comprate (generate o
manuali) e le voci manuali non comprate non vengono mai toccate.

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

**Posizioni doppie (17 settembre 2026)**: collaudando, Alessio non riusciva
a spostare una voce spuntata. Guardando il database reale, due voci avevano
lo **stesso** `ordinamento`: scambiare due posizioni uguali non sposta
niente, e le frecce sembravano morte. Le produceva il refresh, che riusava
il numero di una voce rigenerata mentre contava quelle nuove solo da quelle
rimaste. Ora ogni `aggiorna_lista` **rinumera** tutte le voci da 1,
mantenendo l'ordine visibile; la migration del 17 settembre ha ripulito i
dati già esistenti. Non c'entrava lo stato comprato: il blocco a database
sulle voci comprate non protegge la posizione.

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

## Chiusura della spesa (16 settembre 2026)

Chiesto da Alessio: mancava il momento **"spesa fatta"**. La lista è una
sola e le voci comprate ci restavano per sempre, quindi il giro di spesa
successivo ripartiva sporco e `comprato` finiva per voler dire due cose
diverse — *l'ho appena preso* e *l'avevo preso la settimana scorsa*.

`🧾 Chiudi la spesa` (visibile solo con almeno una voce comprata) apre una
conferma e poi, con `✅ Sì, chiudi la spesa`:

- sposta **tutte** le voci comprate (generate e manuali) in
  `liste_spesa_voci_archiviate`, sotto una riga di
  `liste_spesa_chiusure` che ricorda quando, da chi e su quale intervallo;
- le toglie dalla lista attiva; **le voci non comprate restano** dov'erano,
  con il loro ordine;
- toglie le **aggiunte dal catalogo già coperte** da ciò che si è comprato
  (`aggiunte_coperte_dalla_spesa`, dominio puro): un'aggiunta resta viva
  attraverso ogni refresh — è il suo scopo — quindi senza questo passaggio
  ricomparirebbe il giorno dopo come se non fosse mai stata comprata. Si
  scala il comprato sulle aggiunte della stessa identità e unità, dalla più
  vecchia alla più recente, e rientra solo l'aggiunta coperta per intero;
- fa entrare in casa, ognuna nel suo posto, le voci collegate al catalogo
  (`dispensa::ingresso_da_chiusura`, se l'ingresso automatico è acceso);
- ricalcola subito la lista, che — al netto delle scorte appena entrate —
  non fa ricomparire la roba comprata.

Il messaggio dopo la chiusura: `✅ Spesa chiusa: 2 voci nell'archivio.` e,
a capo, `Entrate in casa: 1 in 🧺 Dispensa, 1 in 🧊 Frigo.` (la prima
versione diceva "voci archiviate nell'archivio", ripetizione corretta il 17
settembre 2026).

**Non è un'eliminazione**, e per questo non segue C16: le voci restano
consultabili in `🗄 Ultima spesa` e sono la memoria di cosa è entrato in
casa. La conferma dice quindi cosa succede, non che "non si può
recuperare". Una voce archiviata però non si modifica più: lo impedisce il
trigger `trg_lista_spesa_voce_archiviata_immutabile`, stesso principio del
congelamento di una voce comprata.

## "📦 Ho preso…" (consegna A, 17 settembre 2026)

Al supermercato la confezione giusta al grammo non c'è quasi mai: servono
250 g e la confezione è da 300 g. Chiesto da Alessio: segnarlo sul momento,
così in casa entra quello che si è preso davvero.

- Accanto a ogni voce **con una quantità** c'è `📦`
  (`lista_spesa:presa:{voce}`). Spuntare resta un tocco solo sul nome.
- La schermata dice quanto serve (e quanto è già segnato), propone le
  **confezioni registrate** per l'alimento — il formato base di ogni
  prodotto attivo e i suoi formati aggiuntivi, senza doppioni, al massimo 8;
  solo quelle del prodotto se la voce è già di un prodotto preciso —
  (`lista_spesa:presa:f:{voce}:p{prodotto}` / `:f{formato}`) e accetta una
  quantità scritta (senza unità vale quella della voce).
- Segnare la presa **spunta la voce**; il pulsante mostra, a capo,
  `📦 presi 300 g`. `↩️ Conta la quantità in lista`
  (`lista_spesa:presa:reset:{voce}`) la toglie lasciando la spunta.
  **Togliere la spunta** dimentica anche la presa.
- **Alla chiusura** entra in archivio — e quindi in casa — la quantità
  presa, con il prodotto preso; l'archivio conserva accanto quanto serviva
  (`quantita_richiesta`). Per coprire le aggiunte dal catalogo conta invece
  quanto serviva: l'eccedenza entra in casa, ed è da lì che la lista la
  sottrae.
- Una voce con la presa segnata **non si fonde** con un'altra riga comprata
  dello stesso alimento (vedi "fusione" sopra): la somma di due confezioni
  diverse non avrebbe un significato chiaro. Restano due righe.

Le colonne della presa non sono nel trigger di congelamento: si segnano
proprio dopo aver spuntato.

## Accesso dal planner (16 settembre 2026)

La schermata della settimana del planner ha un pulsante `🛒 Lista della
spesa`: è pianificando i pasti che viene in mente cosa manca, e prima
bisognava risalire fino a `🍽️ Alimentazione`.

Il pulsante usa un callback dedicato (`lista_spesa:menu:planner`) che
registra la provenienza, così `⬅️ Indietro` dalla lista riporta al planner
invece che al menù Alimentazione, come vuole C3. Le schermate interne della
lista (riordino, rimozione, archivio, calendario) tornano con
`lista_spesa:back`, che non tocca la provenienza: un giro dentro la lista
non deve far dimenticare da dove si è entrati.

**E il contrario (17 settembre 2026)**: la lista ha `📅 Planner`, chiesto da
Alessio. Porta al planner con `planner:menu:lista`, così la settimana
ricorda di arrivare dalla lista e il suo `⬅️ Indietro` (e il suo
`🛒 Lista della spesa`) tornano qui. Se invece alla lista si era arrivati
proprio dal planner, `📅 Planner` ci torna e basta: senza questa regola
lista → planner → lista → planner diventava un giro senza fine, perché
ognuno dei due avrebbe ricordato di arrivare dall'altro.

## Schermate

**Principale** — intervallo (con le date leggibili, `Mer 17 Set → Mar 23
Set`), eventuale riga fissa sull'inizio nel passato, eventuale resoconto
dell'aggiornamento automatico, conteggio comprate/totale, avviso generale se
c'è un eccesso, **tutte** le voci come pulsanti `✅`/`☐` (toggle al tocco,
con l'eventuale eccesso sul pulsante), **senza paginazione**: eccezione
esplicita a C6 (`docs/convenzioni-telegram.md`), l'unica lista del bot che
mostra tutto insieme, perché l'utente deve vedere l'intera lista per
decidere cosa prendere prima e cosa dopo. Poi, dal 17 settembre 2026, le
azioni a coppie per non allungare troppo la schermata:

```text
[ 🔄 Aggiorna lista ]                        solo con l'aggiornamento automatico spento, e se serve
[ ➕ Aggiungi voce manuale ]
[ ↕️ Riordina        | 🗑️ Rimuovi voci  ]      ciascuno solo se ha senso
[ 🧾 Chiudi la spesa | 🗄 Ultima spesa   ]      ciascuno solo se ha senso
[ 🗓️ Cambia intervallo | 📅 Planner      ]
[ 📋 Ultimi cambiamenti ]                    solo se c'è un resoconto
[ ⚙️ Aggiornamento automatico: attivo ]
[ ⬅️ Indietro | 💡 Migliora | 🏠 Menù principale ]
```

**Ultimi cambiamenti** — il resoconto dell'ultimo aggiornamento che ha
cambiato qualcosa: automatico o a mano, quando, e le righe una per una.
Solo lettura, quindi l'elenco sta nel testo (C1 vieta di ripetere i
pulsanti, non di scrivere un elenco quando i pulsanti non ci sono).

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
bloccati). Un inizio precedente a oggi chiede conferma subito (vedi sopra).

## Tabelle

```text
liste_spesa                       intervallo, proprietario, spazio, inizio_manuale
liste_spesa_voci                  voci generate o manuali, comprato/comprato_il, ordinamento, presa
liste_spesa_aggiunte_catalogo     aggiunte dal catalogo, vive attraverso ogni refresh
liste_spesa_chiusure              una spesa chiusa
liste_spesa_voci_archiviate       le voci di una spesa chiusa, immutabili
liste_spesa_aggiornamenti         un aggiornamento che ha cambiato qualcosa
liste_spesa_modifiche             le righe del resoconto, immutabili
```

Vedi `docs/database.md` per i campi.

## Fuori scope

Non modella i prezzi, né suggerisce quale formato/confezione acquistare
(registra solo quella presa, con `📦`):
quello è il futuro modulo Acquisti, che dirà *quale prodotto o confezione
comprare*, mentre questa lista dice solo *cosa serve* — vedi
`docs/previsto/lista-della-spesa.md`, sezione "Relazione con Acquisti". Le
scorte in casa invece le conosce, dal 17 settembre 2026
(`docs/moduli/dispensa.md`).
