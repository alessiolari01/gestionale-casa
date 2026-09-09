# Lista della spesa

**Scritta l'8 settembre 2026, collaudo dal vivo su Telegram fatto per la
prima versione (aggregazione dal planner, voci manuali libere, comprato
congelato).** Due miglioramenti aggiunti il 9 settembre 2026 dopo quel primo
collaudo, **non ancora collaudati dal vivo**: l'aggiunta dal catalogo (questo
documento, sezione "Aggiunta dal catalogo") e il profilo alimentare
automatico (`docs/moduli/profili-e-porzioni.md`, che non riguarda questo
modulo direttamente ma lo stesso giro di feedback).
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
`prodotti_alimentari`), mostrando i risultati come pulsanti distinti
(`🥕` per l'alimento, `🏷️` per il prodotto). Scelto un risultato, chiede
quantità e unità (stesso input di `valida_quantita_manuale`, ma qui sempre
richiesta: niente "➖ Senza quantità", perché la quantità serve a sommare).
Se la ricerca non trova nulla, resta disponibile "📝 Voce libera" (il
flusso testo-libero di sempre, invariato).

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

**Non implementato**: rimuovere un'aggiunta dal catalogo già inserita (né
una voce manuale libera) — oggi non esiste per nessuna voce, non richiesto.

## Segna comprato

Flag booleano per riga intera, non quantità parziale: non c'è modo di dire
"ho comprato solo metà" di una voce. Una volta `comprato = 1`, un trigger a
database impedisce di modificare quantità/unità/descrizione/origine — solo
il toggle (tornare a `comprato = 0`) resta permesso, stesso principio del
congelamento di `planner_pasti`.

## Aggiornamento esplicito, mai automatico

`🔄 Aggiorna lista` ricalcola **solo** le voci `origine = 'generato' AND
comprato = 0`: le cancella e re-inserisce da zero il risultato fresco
dell'aggregazione. Le voci comprate (generate o manuali) e le voci manuali
non comprate non vengono mai toccate.

Se dopo un refresh serve più di un alimento già segnato comprato (es. un
nuovo pasto pianificato che usa lo stesso ingrediente), compare una voce
**nuova** non comprata per la sola differenza — non c'è merge con la vecchia
riga comprata, per non alterare mai una quantità già segnata come acquistata.

## Schermate

**Principale** — intervallo, conteggio comprate/totale, voci come pulsanti
`✅`/`☐` (toggle al tocco), paginazione da `modules::liste`; `🔄 Aggiorna
lista`, `➕ Aggiungi voce manuale`, `🗓️ Cambia intervallo`.

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
liste_spesa_voci                  voci generate o manuali, comprato/comprato_il
liste_spesa_aggiunte_catalogo     aggiunte dal catalogo, vive attraverso ogni refresh
```

Vedi `docs/database.md` per i campi.

## Fuori scope

Non modella la dispensa, i prezzi, o la scelta del formato/confezione da
acquistare: quello è il futuro modulo Acquisti, che dirà *quale prodotto o
confezione comprare*, mentre questa lista dice solo *cosa serve* — vedi
`docs/previsto/lista-della-spesa.md`, sezione "Relazione con Acquisti".
