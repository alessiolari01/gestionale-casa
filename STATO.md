# Stato del progetto — 02/09/2026

**Questo e' l'unico documento che descrive il presente.** Se un fatto sta qui,
non sta anche altrove: gli altri documenti raccontano com'e' fatto il codice
(`docs/architettura.md`), come ci siamo arrivati (`docs/storico-del-progetto.md`)
o cosa non esiste ancora (`docs/previsto/`).

---

## 0. La regola dei documenti

**Nessuna modifica e' finita finche' i documenti non la raccontano.**

Il metro e' questo: *una persona che apre questa cartella senza aver visto
nessuna conversazione deve poter capire dove siamo, cosa fa il programma e
perche' e' fatto cosi'.* Se non ci riesce, la modifica non e' completa.

Nello **stesso commit** che cambia il comportamento si aggiornano:

| se cambia | aggiorna |
|---|---|
| qualunque cosa | `CHANGELOG.md`: cosa e' cambiato e **perche'** |
| lo stato presente | questo file: conteggio test, migration, cosa e' fatto, cosa e' aperto |
| una schermata o il modello di un modulo | il file in `docs/moduli/` |
| una regola d'interfaccia | `docs/convenzioni-telegram.md` |
| lo schema dati | `docs/database.md` |
| cosa viene dopo | `docs/roadmap.md` |

Tre divieti, ognuno nato da un errore realmente commesso:

1. **Non dichiarare fatto cio' che non e' stato verificato.** I documenti hanno
   affermato che una migration fosse da applicare quando era gia' applicata, e
   che due moduli fossero da costruire quando erano gia' in produzione. Bastava
   leggere `_sqlx_migrations` o il menu' del bot.
2. **Quello che viene scartato esce dai documenti del presente.** Va in
   `docs/storico-del-progetto.md`, che si legge per capire perche' il codice
   non e' fatto in un altro modo. Un documento del presente che descrive una
   funzione abbandonata fa perdere tempo a chi lo legge.
3. **Un fatto in un posto solo.** Due documenti che affermano lo stesso numero
   finiscono per affermarne due diversi: e' gia' successo con il conteggio
   delle migration e dei test.

Due controlli automatici, perche' una regola che nessuno verifica dura poco:

- `scripts/aggiorna-s9.sh` confronta il conteggio dei test dichiarato qui con
  quello reale e avvisa se non coincidono;
- `scripts/controlla-documenti.sh`, che gira in CI su ogni push, verifica che
  nessun documento rimandi a un file inesistente e che non esistano due
  percorsi che differiscono solo per maiuscole.

---

## 1. Punto di ripartenza

Repository: `alessiolari01/gestionale-casa`

**La PR #9 e' stata mergiata il 1 settembre: `main` contiene tutto lo Step 7**,
fino al planner 7.3B.

`ux-convenzioni-telegram` e' stato mergiato con la PR #10 e non esiste piu':
il lavoro sull'interfaccia del 1 settembre e' su `main`.

**Il blocco liste** (2 settembre, tre commit) e' arrivato con il ramo
`ux-liste`. Questo file ha gia' sbagliato due volte a dichiarare quali rami
fossero aperti, quindi la risposta non si legge qui ma si chiede a git:

```powershell
git branch -a
git log --oneline main..origin/ux-liste
```

Se il secondo comando non stampa niente, il merge e' avvenuto e `main`
contiene tutto. Se stampa dei commit, quel ramo e' ancora aperto e il lavoro
nuovo parte da li', non da `main` — e finche' resta aperto la
sincronizzazione GitHub del progetto, che segue `main`, non lo vede.

I due branch dello Step 7 sono storia:
`step-7-alimentazione-s9` e' quello mergiato, mentre `step-7-alimentazione`
contiene un 7.3B parallelo scartato il 31 agosto e **non va usato**. Il motivo
e' spiegato nel `CHANGELOG.md`, alla voce "Due implementazioni parallele".

## 2. Cosa e' completo

Step 1-6C e tutto lo Step 7 sono su `main`:

- **7.0-7.1** fondazioni condivise: utenti, spazi, membership, ruoli, vista
  multi-spazio, permessi e audit;
- **7.2A-7.2H** Alimentazione: alimenti, unita', categorie, catalogo e
  compatibilita', prodotti commerciali e nutrizione, ricette con procedimento
  guidato, workflow Miglioramenti con verifica guidata ed export, profili
  alimentari, membri e inviti degli spazi, UI a schermata singola;
- **7.2I** porzioni personali per profilo, override e esclusione del singolo
  ingrediente, calcolo multi-profilo;
- **7.3A** fondazioni del planner: quattro tabelle piu' il dominio degli
  snapshot;
- **7.3B** planner operativo su Telegram: vista settimanale, giorno, aggiunta e
  modifica pasti, scelta ricetta, profili partecipanti, quantita' aggregate,
  completamento con congelamento, esito "saltato", segnalazione della ricetta
  cambiata su settimana, giorno e dettaglio, e riallineamento del pasto alla
  ricetta attuale con conferma esplicita.

La segnalazione riguarda solo i pasti di oggi o futuri: su un pasto passato
riscrivere le quantita' significherebbe riscrivere la storia. L'aggiornamento
non e' mai automatico ed e' sempre limitato al pasto scelto.

La settimana del planner viene creata implicitamente alla prima apertura: non
c'e' una creazione manuale, per non aggiungere concetti all'utente medio.

**Convenzioni dell'interfaccia Telegram.** `docs/convenzioni-telegram.md` e'
il documento di riferimento per ogni schermata: nasce dal giro completo del bot
fatto il 1 settembre e contiene otto problemi trovati e dodici convenzioni. La
piu' importante e' C1: **il testo di un messaggio non ripete mai i pulsanti**.
Applicate al planner, al menu' principale, alle righe di navigazione e — dal
2 settembre — alle **quattro liste**: alimenti, ricette, storico,
miglioramenti. Restano da fare Spazi e Profilo, poi il menu' principale e le
date, nell'ordine della parte 3 di quel documento.

**Un modulo solo per le liste.** `src/modules/liste.rs` tiene la riga di
paginazione, l'intestazione con il totale, la soglia delle venti voci oltre la
quale la ricerca diventa l'azione principale, e il taglio delle etichette. E'
la stessa storia del calendario, e la stessa cura.

La riga prende **il numero di pagine**, non il totale delle voci: la prima
versione prendeva il totale e dava per scontate cinque voci per pagina, e i
chiamanti che contano diversamente — il selettore dei filtri dello storico ne
mostra sette, la descrizione lunga di un miglioramento e' spezzata a caratteri —
non potevano usarla e si erano tenuti la loro riga scritta a mano. Se ne e'
accorto il collaudo sul bot, non il codice: lo `📜 Storico` mostrava ancora
`1 / 21` con le frecce nude. `riga_paginazione_da_totale` resta per chi ha il
totale.

**Restano sei righe di paginazione scritte a mano** fuori da questo blocco, in
`spazi_membri`, `porzioni_profili`, `porzioni_ingredienti`,
`profili_alimentari` (due) e `planner_alimentare`: passeranno con i blocchi
Spazi/Profilo e con il planner.

Guardare le schermate sul bot prima di scrivere ha corretto la convenzione
stessa: C1 diceva che nello Storico «Giorgia» fosse l'autore dell'evento, e
invece e' il profilo su cui la porzione e' stata modificata — cioe' una cosa
che sul pulsante c'era gia'. Quello che mancava era **quando**. La correzione
e' scritta in C1 insieme al suo limite noto: due eventi dello stesso tipo,
sulla stessa entita', nello stesso minuto restano indistinguibili
sull'etichetta e si separano solo aprendoli.

**Un calendario solo.** `src/modules/calendario.rs` tiene le primitive sulle
date e la griglia del mese. Prima ce n'erano due implementazioni: la congruenza
di Zeller scritta a mano negli inviti e quella basata su `chrono` del planner.
Il planner ha guadagnato `📅 Vai a una data`, con i giorni che hanno pasti
marcati da un `•`.

**Aritmetica delle date in Rust.** I conti di calendario non passano piu' da
SQLite: aprire la schermata settimana costava diciannove query, di cui
diciassette calcolavano soltanto date. Ora ne resta una,
`SELECT date('now','localtime')`, che serve davvero perche' il fuso orario del
telefono lo conosce solo SQLite. `chrono` e' dichiarato come dipendenza diretta
con `default-features = false` e non aggiunge nulla al binario: era gia' nel
grafo tramite `teloxide-core`. Effetto collaterale utile: la validazione delle
date e' diventata semantica, quindi `2026-02-30` viene rifiutata all'ingresso
invece di arrivare a SQLite e diventare `NULL`.

## 2bis. Automazione del ciclo di sviluppo — in corso

Dal 3 settembre 2026, per gradi: specifica in
`docs/previsto/automazione-ciclo-sviluppo.md` e
`docs/previsto/invio-miglioramenti-a-claude.md`. Per sapere se c'è lavoro
aperto su questo si chiede a git, come in sezione 1 — non lo si dichiara qui.

Corretta durante la lettura dei due documenti: `docs/infrastruttura.md`
parlava di un "PC fisso" distinto dal portatile, non ancora esistente. La
sessione che scrive questo codice gira sul portatile stesso
(`galaxybookalessio`), che ha già Tailscale e SSH verso l'S9 funzionanti — fa
da host dell'automazione per ora (dettagli in `docs/infrastruttura.md`,
sezione 0).

Decisioni prese il 3 settembre, dettagliate negli stessi due documenti: il
messaggio pinnato di countdown/checklist è pilotato dall'agente via API
Telegram diretta (non dal bot sull'S9); tipo/orario della manutenzione si
configurano da una schermata admin con default + scelta puntuale; la coda
"in attesa di input testuale" si controlla interrogando le nove mappe di
sessione esistenti in `main.rs`, senza unificarle prima; gli invii duplicati
di `📤 Invia a Claude` si evitano con un flag sulla riga `miglioramenti`, non
una tabella coda separata. Confermato che `cargo test` non tocca mai il
database reale (ogni test usa `sqlite::memory:`).

**Primo pezzo pronto**: `scripts/verifica-ci.sh`, legge lo stato reale
dell'ultima run CI di un ramo dall'API di GitHub (nessun token necessario, il
repository è pubblico). Provato su una run vera, `#70` su questo stesso ramo:
letto correttamente sia mentre era `in_progress` sia dopo, `completed` /
`success`. `--attendi` ripete il controllo fino a un timeout invece di uscire
subito con "ancora in corso".

**Secondo pezzo pronto**: `scripts/pipeline-locale.sh`, la stessa sequenza di
`ci.yml` (documenti, fmt, check, test, clippy) in locale, punto 2 del ciclo.
Con `--commit FILE_MESSAGGIO FILE...` aggiunge, committa e — con `--push` —
pusha solo se tutti i controlli sono verdi: "niente commit o push se la
pipeline fallisce" era già una regola operativa (sezione 7), ora è meccanica
invece che da ricordarsi. Provato sia a far passare una pipeline vera (270
test) sia a farla fallire di proposito (un file non formattato): nel secondo
caso nessun commit è stato creato, verificato con `git status`.

Nel provarlo si è trovato e sistemato un bug reale in
`scripts/controlla-documenti.sh`, non nuovo di oggi: usava `os.path`, che su
Windows normalizza con `\` invece di `/`, e faceva risultare "rotto" ogni
singolo rimando del progetto — anche quelli mai toccati, come si era già
visto collaudando a mano il blocco Spazi/Profilo. Corretto usando sempre
`posixpath`, indipendentemente dal sistema operativo. Lo script ora rileva
anche da solo se `python3` è lo stub Windows che non esegue nulla, e ripiega
su `python` — la stessa soluzione già in `verifica-ci.sh`.

**Terzo pezzo pronto**: `scripts/collauda-remoto.sh`, punto 4 del ciclo —
lancia `aggiorna-s9.sh --ramo <nome> --solo-controlli` sull'S9 via SSH invece
di chiederlo a chi lo faceva a mano da Termux. Usa sempre `--solo-controlli`:
verifica compilazione/Clippy/test/migration, non avvia mai il bot. Provato
per davvero su questo ramo: S9 passato da `ux-spazi-profilo` ad
`automazione-ciclo-sviluppo`, 270 test (uguale a quanto dichiarato qui),
backup creato, nessuna migration pendente, fermato prima dell'avvio come
richiesto. L'S9 resta sul ramo appena collaudato — comportamento voluto,
lo stesso di quando lo lancia una persona.

**Punto 6 del ciclo (deploy), sotto-step 1/5 fatto**: la meccanica del
messaggio di countdown via API diretta, isolata da qualunque deploy vero.
`scripts/telegram-api.sh` (`tg_leggi_credenziali`, `tg_invia`, `tg_modifica`,
`tg_elimina`) e `scripts/prova-countdown.sh` per collaudarla. Confermato da
Alessio sulla chat reale, su chat vuota: tick al secondo, stesso messaggio
modificato senza mai duplicarlo, contatore visibile che scende a zero. Il
messaggio arriva **solo alla chat dell'amministratore principale** — la
funzione cerca proprio quel chat_id nel database, nessun altro utente lo
vede mai.

Tre problemi trovati collaudando per davvero, non a tavolino:

- `curl --data-urlencode` su questa macchina (curl 8.21/mingw-w64) corrompe
  i caratteri non-ASCII (una vocale accentata diventa `U+FFFD` prima di
  essere codificata) e Telegram rifiuta la richiesta. Aggirato codificando
  il testo con Python (`urllib.parse.quote`) e passandolo già pronto con
  `--data` semplice;
- un tick al secondo per 30 modifiche ha incontrato tre `Recv failure:
  Connection was reset` transitori. `curl --retry 4 --retry-all-errors
  --retry-delay 2` li assorbe da solo (e rispetta anche l'header
  `Retry-After` di un eventuale 429 di Telegram, senza doverlo leggere a
  mano): il countdown è arrivato in fondo lo stesso, con un piccolo scatto
  visibile una sola volta;
- **il pin è stato tolto**: la prima versione fissava (pin) il messaggio.
  Dopo averlo eliminato a fine collaudo restava in chat una notifica di
  sistema fantasma («Gestionale_Bot pinned Deleted message»), non
  ripulibile via API (i service message di pin/unpin non restituiscono un
  `message_id` utilizzabile). Trovato da Alessio guardando la chat vera, non
  a tavolino. Tolto il pin del tutto: un messaggio normale, sempre
  aggiornato sullo stesso id, basta — non serve fissarlo per tenerlo
  "fermo", ci pensa già il fatto che è l'unico messaggio a cambiare.
  Riprovato su chat vuota: nessuna notifica fantasma.

**Sotto-step 2/5 fatto**: `scripts/avvia-bot.sh` e `scripts/ferma-bot.sh`,
gestione del processo sull'S9 con `nohup`+`disown`+file PID (deciso il
4 settembre — niente tmux/screen/supervisore nuovo, coerente con la scelta
già fatta nel progetto contro Docker/container). Provato per davvero,
partendo da bot spento (verificato con `ps`, non dato per scontato): avvio,
online confermato dal log (`Gestionale Casa online`), spegnimento con
`SIGINT` — non `SIGTERM`, perché il dispatcher in `main.rs` è collegato solo
a `.enable_ctrlc_handler()`, che ascolta SIGINT — e spegnimento pulito
confermato dal log (`^C received`, `Gestionale Casa offline`) e dal
messaggio che il bot manda da solo agli amministratori.

**Sotto-step 3/5 fatto**: la schermata admin `🛠️ Amministrazione → 🚀
Distribuzione`, per configurare il default di tipo/orario della
manutenzione proposto a ogni deploy (Subito / Countdown standard / Programma
orario). Schema deciso insieme ad Alessio prima di scrivere codice: tabella
dedicata a riga singola `impostazioni_distribuzione`
(`migrations/20260904150000_impostazioni_distribuzione.sql`), con CHECK che
tengono coerenti tipo e parametro, più le colonne `scelta_puntuale_*` — già
nello schema, senza UI ancora — che il sotto-step 5 userà per la scelta
puntuale del singolo deploy. L'input dei valori (minuti del countdown,
orario della manutenzione programmata) è ibrido su richiesta di Alessio:
bottoni con valori preimpostati (3/5/10/15 minuti; 02:00/03:00/04:00/05:00)
più testo libero validato, gestito da una decima mappa di sessione
indipendente in `main.rs` (`distribuzione_sessions`), sullo stesso schema di
`identity_sessions` — coerente con la decisione del 3 settembre di non
unificare le mappe esistenti. Il modulo `src/modules/distribuzione.rs`
contiene la logica pura (validazione minuti/orario, calcolo del tempo
rimanente fino a un orario programmato) coperta da unit test, sullo stesso
schema di `porzioni.rs` e `calendario.rs`.

Nel farlo, rispondendo a una domanda di Alessio sul comportamento del
countdown già collaudato (sotto-step 1) sotto un intoppo di rete, si è
trovato un bug reale in `scripts/prova-countdown.sh`: il tempo rimanente
veniva decrementato di 1 a ogni giro del loop invece di essere calcolato da
una scadenza fissa, quindi un tick più lento di un secondo (un retry di
rete) faceva restare il numero mostrato indietro rispetto al tempo vero,
senza mai recuperare. Corretto calcolando il rimanente da
`scadenza - ora_attuale` a ogni giro. Ricollaudato per davvero sulla chat
reale (10s): due `Recv failure: Connection was reset` transitori assorbiti
dai retry di `_tg_curl`, countdown arrivato a 0 con i salti nel numero
mostrato dovuti proprio ai due intoppi — comportamento atteso e confermato
da Alessio dopo la spiegazione. Messaggio di prova ripulito a fine collaudo.

**Sotto-step 4/5 fatto**: il controllo pre-swap "qualche chat sta scrivendo?"
(le dieci mappe di sessione — nove più `distribuzione_sessions` del
sotto-step 3 — interrogate così come sono, senza unificarle, deciso il
3 settembre). Decisione di schema presa insieme ad Alessio prima di
scrivere codice: le mappe vivono solo nella memoria del processo Rust
sull'S9, quindi un controllo esterno via SSH non può leggerle
direttamente — serve un canale che il bot esponga su richiesta. Scelto un
segnale on-demand invece di una scrittura periodica: il bot ascolta
`SIGUSR1` (nuovo, non tocca il `SIGINT` già collaudato nel sotto-step 2) e,
alla ricezione, scrive `data/run/sessioni.txt` con il numero di chat che
hanno una sessione attiva in una qualunque delle dieci mappe. Il nuovo
script `scripts/controlla-sessioni-attive.sh` (lanciato dal PC, come
`collauda-remoto.sh`) manda il segnale via SSH, legge il file, e ripete con
un intervallo fino a "0 sessioni attive" o a un tempo massimo — oltre il
quale procede comunque, come deciso nella specifica. Non tocca ancora
`ferma-bot.sh`: il collegamento dei due nella sequenza reale di stop arriva
con il sotto-step 5.

Collaudato per davvero sull'S9 (bot aggiornato sul ramo, 279 test verdi
anche sulla sua toolchain, avviato con `avvia-bot.sh`): a bot libero
`controlla-sessioni-attive.sh` ha riportato subito "0 sessioni attive";
messa una sessione vera in attesa sulla chat dell'amministratore principale
(`🚀 Distribuzione → ✏️ Cambia default → Countdown standard → ✏️ Altro
valore`, che apre `AwaitingMinuti`), lo script ha rilevato "1" a ogni
ripetizione e, al timeout di prova (20s), è uscito con "procedo comunque"
come da specifica; liberata la sessione (valore scritto), il conteggio è
tornato a "0". Bot fermato a fine collaudo con `ferma-bot.sh`, spegnimento
pulito confermato come nel sotto-step 2.

Un bug reale trovato collaudando, non a tavolino: il primo giro dello
script falliva con "nessun file PID" anche a bot avviato. La causa era un
`~` dentro una variabile del PC (`CARTELLA_RUN_S9="~/gestionale-casa/data/run"`)
interpolata dentro apici singoli nel comando remoto: l'espansione della
tilde vale solo a inizio parola durante il parsing, non dentro un valore
già sostituito, quindi restava letterale. Corretto usando un percorso
relativo (`data/run`), visto che il comando remoto fa già `cd
~/gestionale-casa` prima di usarlo.

Resta 1 sotto-step, il più delicato, spezzato a sua volta in quattro pezzi
concordati con Alessio prima di scrivere codice: 5a modalità riservata,
5b rollback del binario senza ricompilazione, 5c riepilogo/checklist/
conferma pilotati dal bot stesso (non dall'agente via API diretta, a
differenza del countdown: qui il bot nuovo è già acceso e in ascolto,
non serve altro), 5d l'orchestrazione che li lega.

**Sotto-step 5a fatto**: la modalità riservata (solo l'amministratore principale può
usare il bot, gli altri vedono "🚧 Manutenzione in corso"). Deciso insieme
ad Alessio: un flag in memoria (`ModalitaRiservata`, un `AtomicBool`
condiviso come `ShutdownController`), non un file o una variabile letta
una volta sola — deve poter tornare disattivo premendo un bottone in chat,
senza riavviare il processo. `scripts/avvia-bot.sh --riservato` imposta
`RISERVATO=1` solo per quel lancio; un avvio normale (Termux quotidiano,
o senza il flag) resta aperto a tutti come oggi. Il bottone
`✅ Sblocca, torna online per tutti`, visibile solo all'amministratore
principale quando la modalità è attiva, disattiva il flag e manda
"✅ Di nuovo online." a tutte le chat di utenti attivi
(`identity::list_active_chat_ids`, nuova).

Collaudato per davvero sull'S9 (bot aggiornato sul ramo, 280 test verdi
anche sulla sua toolchain, avviato con `avvia-bot.sh --riservato`, log che
conferma "Avvio in modalità riservata (RISERVATO=1)"): Alessio (amministratore
principale) ha continuato a usare il bot normalmente con la modalità
attiva (`/start` e navigazione, nessun avviso di manutenzione); nel menù
`🛠️ Amministrazione` è comparso il bottone `✅ Sblocca, torna online per
tutti`; premuto, il bottone è sparito dal menù, segno che il flag è
tornato a modalità normale — nessun errore nel log durante la notifica
"✅ Di nuovo online." alle chat attive. Bot fermato a fine collaudo con
`ferma-bot.sh`.

**Verificabile solo in parte per ora**: il gate vero e proprio (bloccare
chi *non* è amministratore principale) richiederebbe un secondo account
Telegram per il collaudo reale — la stessa lacuna già segnata al punto 6
di questa sezione. Resta coperto solo dall'unit test
(`deve_bloccare_per_manutenzione`, quattro casi), non da un collaudo su un
utente vero non amministratore.

**Sotto-step 5b fatto**: rollback del
binario senza ricompilazione. Deciso insieme ad Alessio: una copia del
binario compilato, non due cartelle di lavoro separate (blue/green) —
più semplice e meno spazio, su un telefono conta. `scripts/salva-binario.sh`
copia `target/debug/gestionale-casa` in `data/run/binario_precedente`
**prima** di aggiornare il codice e ricompilare (l'ordine conta: lanciato
dopo la build nuova salverebbe il binario sbagliato).
`scripts/rollback-binario.sh` ferma quello che sta girando (necessario:
Telegram rifiuta un secondo long-polling con lo stesso token, 409
Conflict) ed esegue direttamente il binario salvato — stesso schema di
processo di `avvia-bot.sh` (nohup+disown, `data/run/bot.pid`,
`data/run/bot.out`), così `ferma-bot.sh` continua a funzionare dopo un
rollback senza saperne nulla. Timeout di avvio molto più corto
(30s contro i 180s di `avvia-bot.sh`): non c'è nessuna build da aspettare,
se il binario già compilato non parte in pochi secondi il problema è
un altro.

Collaudato per davvero sull'S9: `salva-binario.sh` ha copiato il binario
(50M); avviato il bot normale (`avvia-bot.sh`); `rollback-binario.sh` ha
fermato quel processo e fatto ripartire il binario salvato in **~2
secondi** (`time` alla mano), log senza nessuna riga "Compiling" —
conferma che non c'è stata ricompilazione — e senza `RISERVATO` (torna in
modalità normale, come deciso). `ferma-bot.sh` ha fermato il processo
ripristinato con lo spegnimento pulito consueto, senza saperne nulla del
rollback: stesso schema di PID di sempre.

**Sotto-step 5c fatto**: riepilogo,
checklist e conferma/rifiuto pilotati dal bot stesso, deciso il 4
settembre. Il contenuto (cosa è stato implementato + passi da provare)
arriva da un file scritto dall'agente prima dello swap
(`data/run/riepilogo_deploy.txt`, formato: testo libero, poi una riga
`---CHECKLIST---`, poi una voce per riga) — stesso canale a file già usato
per `RISERVATO` e per lo stato delle sessioni, non un parametro nuovo per
ogni pezzo. All'avvio, se `RISERVATO=1` e il file è leggibile, il bot manda
da solo il messaggio all'amministratore principale
(`identity::list_primary_admin_chat_ids`): riepilogo, checklist con
bottoni `☐`/`✅` che si spuntano via edit (mai un messaggio nuovo), e i
bottoni `✅ Confermo, funziona` / `❌ Non funziona` che compaiono solo a
checklist completa — verificato anche lato server, non solo nascondendo i
bottoni, perché uno stato lato client non è mai una garanzia.

Alla conferma: disattiva la modalità riservata e notifica tutte le chat
attive — stessa funzione (`esci_da_modalita_riservata`) già usata dal
bottone di sblocco del sotto-step 5a, condivisa invece di duplicata. Al
rifiuto: resta in modalità riservata, come da specifica. In entrambi i
casi scrive `data/run/esito_collaudo.txt` ("confermato" o "rifiutato"),
che l'agente orchestratore (sotto-step 5d) leggerà per decidere se
procedere al merge o innescare il rollback del sotto-step 5b — non ancora
collegato a nessuno dei due.

Bug reale trovato collaudando per davvero sull'S9, non a tavolino: il
secondo click su una qualunque voce della checklist veniva rifiutato con
"⚠️ Questa schermata non è più attiva", come se il messaggio fosse vecchio.
Causa: `ContextBot::claim_callback` (`src/context_bot.rs`) permette **una
sola rivendicazione per `message_id`**, perché finora ogni altra schermata
del progetto manda sempre un messaggio nuovo a ogni cambio — mai una
modifica in place. La checklist del collaudo è la prima a modificare più
volte lo stesso messaggio via edit, quindi ogni click dopo il primo sullo
stesso messaggio falliva sempre, non per un doppio click veloce. Corretto
estendendo la stessa eccezione già esistente per `*:noop` (che usa "è
ancora il messaggio corrente?" invece di "rivendicato una volta sola") ai
callback `collaudo:*`. Ricollaudato sull'S9: le tre voci della checklist di
prova si sono spuntate correttamente una dopo l'altra, e i bottoni
`✅ Confermo` / `❌ Non funziona` sono comparsi solo a checklist completa.

**Secondo bug reale, trovato subito dopo collaudando la conferma**:
premendo `✅ Confermo, funziona`, in chat è rimasto solo "✅ Di nuovo
online." — la checklist con "Collaudo confermato" non si vedeva più.
Causa: `esci_da_modalita_riservata` manda quel messaggio con
`send_message_without_improve`, che passa dal wrapper `ContextBot` e dalla
sua regola "una sola schermata UI attiva per chat" — mandare un nuovo
messaggio a una chat **cancella quello attivo precedente**, che per
l'amministratore principale era proprio il messaggio di collaudo appena
modificato in "confermato" (o stava per esserlo: la cancellazione avviene
prima che l'edit abbia effetto, quindi l'edit poi falliva su un messaggio
già sparito).

Primo tentativo di correzione sbagliato, trovato ricollaudando subito
dopo: `(*bot).send_message(...)` doveva bypassare `ContextBot` per
raggiungere il bot grezzo di teloxide, ma il deref di un `&ContextBot`
produce `ContextBot` stesso (non passa dal suo `impl Deref`, che serve
solo quando si deref-a un valore *non* di riferimento) — quindi quella
riga richiamava comunque il metodo di `ContextBot`, di fatto identico a
`bot.send_message(...)`: in chat compariva anche il bottone "💡 Migliora",
segno che il messaggio era ancora tracciato. Corretto usando
`send_message_untracked`, un metodo che esiste già in `context_bot.rs`
apposta per le notifiche che non devono toccare nessuno stato di
"schermata attiva": usa il bot grezzo per davvero, senza passare dal
wrapper.

Con questo secondo fix il collaudo della conferma è passato per davvero
(checklist rimasta come "Collaudo confermato", notifica separata "Di
nuovo online" senza bottone Migliora). Chiesto da Alessio a quel punto:
poter riprendere a usare il gestionale subito dopo la conferma, senza
scrivere un comando a mano. Aggiunto un bottone `🏠 Menù principale`
(callback `menu:main`, lo stesso di sempre — risolve l'attore vero al
momento del click, non ricostruito qui) alla notifica di broadcast. Perché
quel bottone sia cliccabile, la notifica dev'essere di nuovo *tracciata*
(`send_message_without_improve`, non più `send_message_untracked`):
`claim_callback` accetta un click solo su una schermata registrata come
attiva. Per restare sicuri, riordinato: l'edit del messaggio di collaudo
in "confermato" avviene *prima* della notifica broadcast, non dopo — così
la notifica tracciata cancella il messaggio "confermato" già mostrato
(la stessa transizione "vecchia schermata sparisce, nuova arriva" di
sempre), non la checklist ancora da completare.

**Collaudo finale, end-to-end, confermato da Alessio sulla chat reale**:
checklist spuntata voce per voce, bottoni di conferma comparsi solo a
completamento, "✅ Confermo, funziona" premuto → messaggio diventato
"Collaudo confermato", notifica separata "✅ Di nuovo online." con il
bottone `🏠 Menù principale`, premuto quel bottone → menù principale vero
e operativo, non un vicolo cieco. Bot fermato con `ferma-bot.sh` a fine
collaudo; file di prova (`riepilogo_deploy.txt`, `esito_collaudo.txt`)
ripuliti da `data/run/` sull'S9.

289 test (280 prima), 9 nuovi in `src/modules/collaudo.rs`: interpretazione
del file (separatore mancante, checklist vuota, righe vuote ignorate),
`tutto_fatto`/`alterna` (incluso un indice inesistente), la tastiera che
mostra conferma/rifiuto solo a checklist completa, il troncamento delle
etichette lunghe.

**Trovato per davvero aggiornando l'S9 su questo commit**: gli script più
recenti creati su questa macchina Windows (`controlla-sessioni-attive.sh`,
`rollback-binario.sh`, `salva-binario.sh`) erano finiti in git come file
normali (`100644`), non eseguibili (`100755`) come `avvia-bot.sh` e
`ferma-bot.sh` — Windows non ha il bit di esecuzione, quindi git li aveva
aggiunti senza. Corretto in git (`git update-index --chmod=+x`), non sul
filesystem, così ogni futuro checkout sull'S9 li trova già giusti.

Con 5a, 5b e 5c fatti, resta il sotto-step 5d: l'orchestrazione che lega
insieme i controlli e i pezzi già costruiti (aspetta le sessioni, salva il
binario, ferma, aggiorna, avvia riservato con il riepilogo pronto,
controlla la salute del nuovo processo, chiama il rollback se serve) in
un'unica sequenza — oggi ogni pezzo è stato collaudato lanciandolo a mano,
uno alla volta.

**Sotto-step 5d, scritto — collaudo end-to-end sull'S9 da fare**: tre
script legano insieme i pezzi 1-5c. `scripts/countdown-manutenzione.sh`
(nuovo) è la versione "vera" del countdown collaudato in
`prova-countdown.sh`: stessa meccanica a scadenza fissa, ma legge il
default (tipo/minuti/orario) dalla tabella `impostazioni_distribuzione`
invece di un valore fisso — il tipo "countdown" è quello collaudato per
davvero fin qui, "subito" e "programmato" sono scritti ma meno esercitati
(l'unico default operativo del progetto oggi è "countdown").

`scripts/deploy.sh` è la sequenza vera: countdown →
`controlla-sessioni-attive.sh` → `salva-binario.sh` (**prima** di
aggiornare il codice) → `ferma-bot.sh` → `aggiorna-s9.sh --ramo X
--solo-controlli` (ricompila e riverifica sull'S9, non un `git pull`
nudo: build/clippy/test/migration di nuovo, appena prima dello swap) →
copia del riepilogo/checklist e `avvia-bot.sh --riservato` → controllo di
stabilità (online + resta vivo per una finestra, default 20s) → rollback
automatico (`rollback-binario.sh`) se qualcosa va storto, con notifica
Telegram dell'errore preciso a ogni passo che fallisce — mai un
fallimento silenzioso. Da lì l'agente si ferma: il collaudo lo gestisce
il bot stesso (5c).

`scripts/completa-deploy.sh` è il seguito, lanciato quando arriva
l'esito: legge `esito_collaudo.txt` sull'S9 — "confermato" → merge del
ramo su `main` (**solo con `--confermo-merge` esplicito**, deciso per non
rendere irreversibile un'azione con conseguenze reali senza un secondo
controllo); "rifiutato" → `rollback-binario.sh --riservato` (nuovo
flag, aggiunto qui: **al contrario** del rollback per salute fallita, che
torna in modalità normale, un collaudo rifiutato deve restare in
manutenzione per gli utenti normali finché non arriva una versione
corretta, come dice esplicitamente la specifica — il vecchio binario
comunque funzionante non basta a giustificare di riaprire l'accesso).

**Collaudo end-to-end fatto per davvero, sotto-step 5d completo**: tre
ridistribuzioni reali dello stesso commit già in produzione (nessun
codice nuovo, solo per esercitare la sequenza vera sull'S9 vero), tutte
concordate con Alessio prima di lanciarle. La sequenza intera (countdown
→ sessioni → salvataggio binario → arresto → ricompilazione e riverifica
→ riavvio riservato → riepilogo/checklist mandati dal bot stesso →
conferma del collaudo → ritorno alla modalità normale) ha funzionato
end-to-end, ma solo dopo tre correzioni trovate dal vivo, non a tavolino:

1. **Click ripetuti sulla checklist bloccati**: `claim_callback` in
   `context_bot.rs` reclama ogni `message_id` una sola volta, pensato per
   evitare doppie azioni sullo stesso pulsante — ma la checklist del
   collaudo va spuntata più volte sullo stesso messaggio. Corretto
   estendendo il bypass già esistente per i callback `*:noop` (che usa
   `is_current_message` invece del claim one-shot) anche ai callback
   `collaudo:*`.
2. **Il messaggio di conferma spariva subito dopo**: `esci_da_modalita_riservata`
   manda un messaggio "di nuovo online" a tutte le chat attive usando il
   percorso *tracciato* di `ContextBot` (una sola schermata attiva per
   chat), che quindi cancellava come effetto collaterale il messaggio
   "Collaudo confermato" appena mostrato. Un primo tentativo di correzione
   con `(*bot).send_message(...)` non ha funzionato — `*bot` su un
   `&ContextBot` è la deref built-in del riferimento esterno, non invoca
   la `impl Deref` di `ContextBot` verso il bot grezzo (ci vorrebbe
   `**bot`) — riconosciuto dal ricomparire del pulsante "Migliora".
   Corretto usando il metodo già esistente `send_message_untracked`.
   Aggiunto anche un pulsante "torna al menù" al messaggio di conferma
   (richiesto da Alessio, per non dover riscrivere un messaggio a mano
   dopo il collaudo) — che per essere cliccabile ha richiesto di tornare
   al percorso *tracciato* solo per quel messaggio specifico, e di
   spostare la modifica della checklist a "confermato" *prima* della
   notifica broadcast (altrimenti il broadcast tracciato cancella la
   checklist ancora aperta invece del messaggio finale).
3. **Il messaggio di manutenzione restava bloccato**: `deploy.sh`
   scartava l'id del messaggio di countdown (`>/dev/null`), quindi non
   poteva più aggiornarlo a swap concluso — restava per sempre su
   "🚧 Manutenzione in corso". Corretto catturando l'id in
   `ID_MANUTENZIONE`. La prima correzione lo aggiornava a "✅ Aggiornamento
   completato", ma Alessio ha notato dal vivo (screenshot) che restava
   comunque visibile come banner sopra il menù principale già in uso
   normale — un residuo, non un'informazione utile, dato che a quel punto
   il racconto lo prende il bot stesso (riepilogo, checklist, conferma,
   "di nuovo online"). Corretto sostituendo l'aggiornamento con
   un'eliminazione (`tg_elimina`) a swap riuscito, mantenendo
   l'aggiornamento solo sui percorsi di errore (dove resta un'informazione
   utile).

Con questi tre fix ricollaudati dal vivo (l'ultimo con una quarta
ridistribuzione mirata), il sotto-step 5d — e quindi l'intero punto 6
"deploy a downtime minimo" — è completo. Non ancora esercitato dal vivo:
il percorso di merge reale su `main` di `completa-deploy.sh`
(`--confermo-merge`), rimandato a quando Alessio deciderà di promuovere
per davvero questo ramo.

**Trovato per davvero verificando la CI di questo stesso push**: subito
dopo `git push`, `scripts/verifica-ci.sh` ha riportato "OK CI verde"
leggendo pero' la run del **commit precedente** (`7c730932`, gia'
`completed`/`success` da prima), non di quello appena pushato
(`760089b`) — che in quel momento era ancora `in_progress` e non ancora
registrato su GitHub nella finestra tra push e comparsa della run nuova.
`per_page=1` prende la run piu' recente per ramo senza controllare a quale
commit appartiene: se la piu' recente e' ancora quella vecchia ma gia'
completata, lo script si ferma li' e riporta un esito che non e' quello del
push appena fatto — lo stesso errore del 2 settembre (un riassunto letto
verde dove non lo era), in una forma nuova. Corretto confrontando lo
`head_sha` restituito dall'API con `git rev-parse <ramo>`: se non
coincidono, lo script tratta la run come "non ancora quella giusta" e
continua ad aspettare invece di fidarsi. Ricollaudato per davvero sul push
di questo stesso commit: la prima chiamata senza `--attendi` ha
correttamente riconosciuto la run giusta (#77, commit `760089b`) come
ancora in corso; con `--attendi` ha aspettato ~80s e riportato l'esito reale
(`success`) solo a run davvero completata.

**Trovato collaudando, fuori dall'ambito di questo blocco**: il messaggio
"ℹ️ Non sto aspettando un input in questo momento" (`main.rs`,
`unexpected_input_notice`) appare *sotto* la schermata principale invece
che vicino ad essa, spostandola fuori dalla vista — limite strutturale di
Telegram (non si può inserire un messaggio "sopra" uno esistente, solo in
fondo alla cronologia). Il meccanismo che lo fa sparire alla prossima
interazione (`cleanup_transient_media`, chiamato sia su un nuovo testo sia
su un pulsante premuto) esiste già nel codice, ma non è stato collaudato
per davvero: nello screenshot del 4 settembre il bot è stato spento subito
dopo la comparsa del messaggio, senza che nessuna interazione successiva
lo mettesse alla prova. Da riprendere come miglioramento a sé, non dentro
questo blocco di automazione.

## 2ter. Badge "🆕" per le novità — infrastruttura pronta, in attesa del primo uso reale

Deciso con Alessio il 5 settembre 2026, indipendente dall'automazione del
ciclo di sviluppo (sezione 2bis): quando una funzionalità è nuova o cambia in
modo significativo, il suo pulsante e ogni pulsante di menù che porta fino a
lì (su su fino al menù principale) mostrano `🆕 `. Sparisce solo per chi
arriva davvero fino alla schermata cambiata (non aprendo un menù intermedio),
e solo per quella persona — non un flag condiviso, perché più persone usano
lo stesso bot. Alla prima visita di ciascuno, la schermata mostra anche un
breve tutorial, chiudibile con `✅ Ho capito`. Dettagli e motivazione in
`docs/convenzioni-telegram.md` (C14).

**Deciso**: si applica solo da questa data in avanti — **nessun retrofit**
delle schermate esistenti, che quindi oggi non mostrano nessun badge.

**Costruita l'infrastruttura di base**: `src/modules/novita.rs` (nuovo),
`migrations/20260905090000_novita_lette.sql` (nuova tabella, additiva). "Cosa
è nuovo" vive in un registro statico nel codice (`REGISTRO`: chiave,
genitore per la risalita nel menù, tutorial opzionale), non nel database — nel
database (`novita_lette`) si tiene traccia solo di chi ha già visto cosa, per
utente. `REGISTRO` **parte vuoto**: nessuna funzionalità è ancora dichiarata,
in attesa del primo pezzo di codice reale che la userà.

Un bug reale trovato scrivendo il primo test (non a tavolino): i nodi
intermedi del registro (es. una categoria usata solo per risalire al menù
superiore, mai una schermata che qualcuno visita davvero) non vengono mai
segnati come "visti" da nessuno — contarli come "ancora da vedere" avrebbe
tenuto il badge acceso per sempre anche a foglie tutte viste. Corretto
distinguendo le foglie del registro (funzionalità vere, visitabili) dai nodi
intermedi (usati solo per la catena verso il menù): solo le foglie contano
per decidere se un pulsante deve mostrare il badge.

8 nuovi test (5 sulla logica pura di propagazione/foglie, 1 sull'etichetta, 2
sulle funzioni DB con `sqlite::memory:`). Totale: 297 (289 prima).

**Primo uso reale, scritto — collaudo dal vivo su Telegram da fare**: gli
allegati di un miglioramento accettano ora anche un video, non solo una
foto (`miglioramento_allegati` aveva `CHECK (tipo = 'foto')`, corretto in
`CHECK (tipo IN ('foto','video'))` con la stessa migration che sistema
anche `miglioramento_archivio_allegati` — altrimenti un miglioramento con
un video non si sarebbe mai potuto archiviare). `save_original_media`
(prima `save_original_photo`) riconosce da sola quale dei due è arrivato e
lo dice nella conferma ("✅ Video aggiunto..." / "✅ Screenshot
aggiunto..."), ricalcando `save_verification_media` che già lo faceva per
le prove di collaudo.

Registrata la prima voce reale in `novita::REGISTRO`
(`miglioramenti_allegato_video`, genitore `improve_menu`): il pulsante
`📋 Miglioramenti` del menù principale mostra "🆕" finché l'utente non
arriva davvero a inviare un allegato. La prima volta che ci riesce, invece
della conferma normale, il bot gli propone un piccolo tutorial guidato —
idea di Alessio: non solo spiegare a parole, ma far *provare* la
funzione — con due bottoni `✅ Tienilo` / `🗑️ Era una prova, elimina` per
decidere se l'allegato appena inviato durante la prova resta vero o va
tolto. Solo un tentativo riuscito (allegato salvato davvero) segna la
novità come vista: chi annulla o non riesce a inviare nulla la ritrova al
tentativo successivo.

3 nuovi test (300 totali, 297 prima): un allegato `tipo = 'video'`
accettato dal CHECK reale (non solo dal codice), le etichette
foto/video, e il badge sul menù principale mostrato/nascosto secondo il
parametro.

**Collaudato per davvero su Telegram (6 settembre 2026), confermato da
Alessio**: badge "🆕" visibile su "📋 Miglioramenti" nel menù principale,
tutorial comparso alla prima prova di allegato con i bottoni
`✅ Tienilo` / `🗑️ Era una prova, elimina` funzionanti, foto e video
riconosciuti correttamente nella conferma, badge sparito dal menù
principale dopo. Corretta un'etichetta notata durante il collaudo:
"✅ Salva senza foto" → "✅ Salva senza allegato" (tre punti), coerente
con l'aver aggiunto il video.

**Deciso e costruito il 7 settembre 2026**: `🏠 Menù principale`, premuto
mentre una bozza/un input atteso è attivo (la stessa situazione in cui
compare il pulsante `❌ Annulla` locale), ora avvisa "❌ Operazione
annullata." esattamente come annullare dalla schermata — prima andava al
menù in silenzio. Centralizzato in un unico punto in `main.rs`
(`handle_authorized_callback`): calcolato subito, prima che qualunque
modulo pulisca la propria sessione, se una qualunque delle dieci mappe di
sessione ha uno stato attivo per quella chat (le stesse già interrogate
dal pre-swap dell'automazione). Aggiunto anche `has_active` a sei mappe
che non lo avevano ancora (`SessionStore`, `LocationSessionStore`,
`ContainerSessionStore`, `PhotoSessionStore`, `IdentitySessionStore`,
`DistribuzioneSessionStore`) — tutte già lo avevano per le altre quattro.

Trovato mentre si sistemava questo: il blocco generico "menu:main" non
puliva mai `profile_sessions`, `identity_sessions` e
`distribuzione_sessions` (a differenza delle altre sette mappe) — un
input testuale in attesa lì poteva restare appeso dopo aver premuto
"Menù principale". Corretto aggiungendo la pulizia delle tre mappe
mancanti nello stesso blocco.

**Secondo bug reale, trovato collaudando per davvero (non a tavolino)**:
la prima versione mandava l'avviso "❌ Operazione annullata." come
messaggio **separato**, mandato subito prima del menù principale.
Alessio ha visto il vero comportamento sul bot: l'avviso compariva per
una frazione di secondo e spariva subito, sostituito dal menù principale
— la regola "una sola schermata attiva per chat" (`ContextBot`) cancella
un messaggio non appena arriva il successivo. Corretto unendo avviso e
menù principale in un **unico** messaggio (`send_main_menu_con_avviso`,
avviso anteposto al testo normale), esattamente come fa già da sempre
`❌ Annulla` in `cancel_improvement_flow`. **Diventata regola globale**
(C3 in `docs/convenzioni-telegram.md`): un avviso di annullamento va
sempre nello stesso messaggio della schermata di destinazione, mai
separato. Audit in corso sugli altri moduli per verificare che ogni
`❌ Annulla` esistente la rispetti già.

**Audit completo e correzione di tutti gli altri punti "❌ Annulla" del
bot (7 settembre 2026)**: chiesto da Alessio dopo il collaudo del punto
precedente. Trovate 8 violazioni reali (stesso bug del messaggio
separato che sparisce) in `alimentazione.rs` (quattro punti, incluso un
"menu:main" locale che ne produceva perfino due) e un punto ciascuno in
`contenitori.rs`, `luoghi.rs`, `oggetti.rs`, `foto.rs` — quest'ultimo
condiviso da comando testuale e pulsante. Più quattro punti dove
l'avviso mancava del tutto (non lo stesso bug, ma comunque fuori dalla
regola): `/annulla` per le sessioni identity/distribuzione in `main.rs`,
il pulsante `foto:cancel`, il pulsante `foodprof:cancel`.

**Cambiata strategia di correzione durante il lavoro**: la prima
correzione (menu:main) aveva aggiunto un parametro `avviso: Option<&str>`
a `send_main_menu`. Estendere lo stesso schema a `contenitori.rs` e
`luoghi.rs` avrebbe richiesto aggiungerlo anche alle **cinque** funzioni
di destinazione diverse a cui un `/annulla` può portare in quei moduli
(alcune in un modulo diverso, es. `luoghi::show_home_detail`) — poco
sostenibile. Costruito invece un meccanismo centrale in `context_bot.rs`:
`ContextBot::annulla_e_avvisa(chat_id, testo)` mette l'avviso "in coda"
per quella chat; il punto unico in cui ogni messaggio tracciato viene
davvero mandato (`ContextRequest::send`) lo preleva e lo antepone al
testo, una sola volta, prima di mandarlo — qualunque sia la schermata di
destinazione, senza dover cambiare la sua funzione. Le correzioni già
fatte (menu:main, Alimentazione) sono state riscritte con lo stesso
meccanismo invece di tenerne due diversi. 3 nuovi test sul meccanismo
(coda consumata una sola volta, isolamento tra chat, che l'avviso vada
davvero nel testo/nella didascalia). 303 test totali (300 prima).

**Decisione di design cambiata dopo il collaudo (7 settembre 2026)**:
Alessio si è confuso a non trovare `⬅️ Indietro` nelle sezioni di primo
livello (dove prima veniva tolto perché avrebbe portato dove porta già
`🏠 Menù principale`) — si aspetta "indietro" sempre nella stessa
posizione, come i tre tasti fissi di un telefono. C3 aggiornata: da ora
`⬅️ Indietro` compare **sempre**, anche quando punta alla stessa
`menu:main`. Applicato a tutte le sette sezioni di primo livello che ne
erano prive (trovate con una ricognizione completa, un'ottava —
`👤 Profilo` — ce l'aveva già): `alimentazione::alimentation_menu_keyboard`,
`oggetti::objects_menu_keyboard`, `luoghi::locations_menu_keyboard`,
`storico::global_history_keyboard`, `main::send_spaces`,
`miglioramenti::menu_keyboard_con_conteggi`, `main::admin_menu_keyboard`.
Le tastiere di fallback/errore che riusano queste stesse funzioni
(decine di punti in `oggetti.rs` e `luoghi.rs`) ne beneficiano
automaticamente, senza toccarle una per una.

**Audit di layout su C3, trovato collaudando dal vivo**: Alessio ha
notato su "➕ Nuovo oggetto" che `❌ Annulla` stava da solo su una riga e
`💡 Migliora | 🏠 Menù principale` sulla riga sotto, invece dell'unica
riga di navigazione prevista da C3 — `context_bot.rs` inserisce
"💡 Migliora" solo accanto a "Menù principale", quindi due righe separate
lasciano l'altro pulsante isolato. Prima ricerca nel bot: sei punti
(`oggetti.rs` due volte, `foto.rs`, `ricette.rs` due volte,
`miglioramenti.rs`), due senza alcun pulsante "Menù principale". Tutti
uniti in un'unica riga finale.

**Stesso audit, secondo giro più a fondo**: continuando a cercare lo
stesso schema sono emersi altri **15 punti**: `oggetti.rs` (5 in più),
`contenitori.rs` (3), `storico.rs` (7, tre delle quali senza "Menù
principale" del tutto). Dove il pulsante era "↩️ Cambia casa/stanza" o
"↩️ Torna a X" (funzionalmente un Indietro contestuale anche se scritto
diverso), trattato come "⬅️ Indietro"/"❌ Annulla" — stessa regola.
21 punti corretti in totale tra i due giri: la stessa disattenzione
(due righe invece di una) si era ripetuta più e più volte scrivendo
tastiere diverse nel tempo, non un errore isolato.

**Uniformate le emoticon, trovato collaudando dal vivo**: Alessio ha
notato su Telegram che `oggetti.rs` mostrava `↩️ Operazione annullata.`
invece di `❌`. Cercato in tutto il bot: altri due punti con la stessa
incoerenza (`contenitori.rs`, `luoghi.rs`), entrambi corretti. Aggiunta
la riga in C4 (`docs/convenzioni-telegram.md`): il simbolo
dell'annullamento è sempre `❌`, mai `↩️`.

**Falso allarme, corretto dopo un ricollaudo dal vivo**: analizzando una
registrazione del collaudo era sembrato che `🏠 Menù principale`, premuto
mentre una bozza di miglioramento è attiva, annullasse il flusso invece
di aprire il vero menù principale. Il campionamento a 1 fotogramma al
secondo aveva probabilmente attribuito al tocco un messaggio
"❌ Operazione annullata." già presente in chat da un'azione precedente,
non la risposta vera a quel tocco. Alessio ha riprovato dal vivo:
`🏠 Menù principale` porta correttamente al menù principale, senza nessun
messaggio di annullamento. Nessun codice da correggere qui — lezione
per la prossima volta: un'analisi video a campionamento sparso non
sostituisce un collaudo dal vivo, soprattutto quando il campione è troppo
rado per essere sicuri di cosa causa cosa.

## 2quater. Lista della spesa — scritta l'8 settembre 2026, primo collaudo dal vivo fatto, due feedback aggiunti il 9

Prossimo macro-step deciso lo stesso giorno (punto 9 della sezione 6),
appoggiato sul planner alimentare già operativo: aggrega automaticamente gli
ingredienti dei pasti pianificati in un intervallo di date scelto
dall'utente, indipendente dalle settimane del planner. Design completo e
motivazione in `docs/previsto/lista-della-spesa.md` e
`docs/moduli/lista-spesa.md`.

Nuovo modulo `src/modules/lista_spesa.rs` (dominio puro, funzioni database,
UI Telegram, stesso schema di `planner_alimentare.rs`) e due tabelle
additive (`migrations/20260908150000_lista_spesa.sql`): `liste_spesa` (una
lista attiva per spazio o personale, `spazio_id` nullable, stesso pattern di
`planner_alimentari`) e `liste_spesa_voci` (voci generate dall'aggregazione
o manuali).

**Cosa aggrega**: da `planner_pasto_ingredienti_snapshot`, solo i pasti
`pianificato` non saltati e non completati, solo le righe con
`quantita_finale_snapshot` non nullo (le righe nulle sono ingredienti
esclusi da quel pasto e non contribuiscono come zero), con `data_pasto`
dentro l'intervallo della lista.

**Conversione di unità**: usa `unita_misura.famiglia_conversione` — stessa
famiglia (massa/volume) sommata nell'unità-base (`g`/`ml`), famiglie diverse
o unità senza famiglia (pz, cucchiaio, qb, o un simbolo sconosciuto) restano
separate, aggregate per simbolo esatto senza conversione.

**Congelamento delle voci comprate, stesso principio di `planner_pasti`**:
un trigger a database (`trg_lista_spesa_voce_comprata_immutabile`) impedisce
di modificare quantità/unità/descrizione/origine una volta `comprato = 1` —
solo il toggle di `comprato` stesso resta permesso. Il refresh esplicito
(mai automatico, bottone `🔄 Aggiorna lista`) tocca *solo* le voci
`origine = 'generato' AND comprato = 0`: le cancella e re-inserisce da zero,
senza mai toccare una voce già comprata o una voce manuale non comprata — se
dopo il refresh serve più di un alimento già comprato, compare una voce
nuova per la sola differenza, non un merge con quella vecchia.

**UI**: nuovo bottone `🛒 Lista della spesa` in `🍽️ Alimentazione`, accanto a
`📅 Planner alimentare`. Voci come bottoni `✅`/`☐` che fanno toggle via
callback; `➕ Aggiungi voce manuale` con input ibrido a due passi (testo
libero per la descrizione, poi quantità+unità scritte a mano oppure
`➖ Senza quantità`) in una nuova mappa di sessione
(`ListaSpesaSessionStore`, dentro `lista_spesa.rs` stesso — non in
`main.rs`, perché a differenza di `DistribuzioneSessionStore` questo modulo
ha un proprio `handle_message`/`handle_callback`, stesso schema delle mappe
di sessione degli altri moduli come `FoodSessionStore`); `🗓️ Cambia
intervallo` riusa `modules::calendario` per le due date (inizio, poi fine
con i giorni precedenti bloccati).

**Badge "🆕" (C14)**: prima voce registrata in `novita::REGISTRO` dopo
l'allegato video dei miglioramenti (`lista_spesa`, genitore `food_menu`). Il
badge risale da `🍽️ Alimentazione` nel menù principale
(`main_menu_keyboard` ha guadagnato un parametro `badge_alimentazione`) fino
al pulsante `🛒 Lista della spesa` nel sotto-menù, con un breve tutorial
alla prima vera visita.

25 nuovi test (16 sul dominio puro dell'aggregazione/conversione/validazione
manuale, 8 sulle funzioni database con `sqlite::memory:`, 1 sul nuovo badge
in `main_menu_keyboard`), per un totale di **328 (303 prima)**. Pipeline
`fmt`, `check --locked`, `clippy --all-targets --locked -- -D warnings`,
`test --locked` verde in locale, tutti i 328 test passano.

Un errore reale trovato dai test stessi, non a tavolino: la prima versione
di `aggiorna_lista` ricalcolava sempre la somma piena dell'aggregazione,
ignorando quanto già coperto da una voce comprata — il test
`refresh_non_tocca_voci_comprate_ne_manuali_non_comprate` si aspettava la
sola differenza (150 g) e otteneva la somma intera (350 g). Corretto
aggiungendo `sottrai_gia_comprato` (dominio puro, tre test dedicati): il
refresh sottrae dal fresco appena aggregato quanto già coperto da voci
`generato` comprate con la stessa identità/unità, e scarta del tutto una
voce se il residuo non è positivo.

**Primo giro di collaudo dal vivo fatto da Alessio**: aggregazione dal
planner, voci manuali libere, comprato congelato — quella parte del
funzionamento è confermata dall'uso reale su Telegram. Dallo stesso giro
sono arrivati due feedback, implementati il 9 settembre come continuazione
dello stesso lavoro (stessi commit, non un ramo nuovo) — **questi due non
sono ancora stati ricollaudati dal vivo**.

### Feedback 1 — profilo alimentare automatico per ogni account

Un utente nuovo non deve più andare manualmente su "👥 Profili alimentari"
e collegare sé stesso prima di poter partecipare a pasti/planner: ogni
account ha già il proprio profilo "sé stesso" fin dal bootstrap.
`identity::provision_approved_telegram_account` chiama, nella stessa
transazione con cui crea lo spazio iniziale (`ensure_initial_space`), una
nuova funzione `profili_alimentari::ensure_self_profile_in_tx(tx, user_id,
display_name)`. Non riusa `create_profile(..., link_to_self: true)`: quella
funzione legge l'utente e registra lo storico tramite
`identity::current_actor()`, che durante il bootstrap è l'amministratore
che sta approvando la richiesta, non il nuovo utente — riusarla avrebbe
attribuito il profilo (`gestore_utente_id`) al gestore sbagliato. La nuova
funzione prende `user_id`/`display_name` espliciti e non tocca lo storico,
stesso trattamento già riservato a `ensure_initial_space` durante il
bootstrap. Idempotente: se il profilo collegato esiste già non fallisce né
duplica (verificato con un test dedicato che la richiama a mano su un
utente già provvisto). La UI di "👥 Profili alimentari" non è cambiata:
gestiva già il caso "profilo esistente" per gli account creati prima di
questa modifica. Dettagli in `docs/moduli/profili-e-porzioni.md`.

### Feedback 2 — aggiunta manuale con ricerca nel catalogo

"➕ Aggiungi voce manuale" offre ora la scelta fra "🔎 Cerca nel catalogo"
(alimenti generici e prodotti commerciali specifici da
`prodotti_alimentari`) e "📝 Voce libera" (flusso testo-libero invariato).
Un **alimento generico** scelto dal catalogo si **somma** al fabbisogno già
calcolato dal planner sullo stesso alimento (l'esempio di Alessio: 200 g
dal planner + 50 g a mano = 250 g in un'unica riga). Un **prodotto
commerciale specifico** resta sempre una riga **separata e distinta**,
anche se collegato allo stesso alimento generico richiesto altrove —
decisione esplicita presa con Alessio per non confondere "mi serve della
pasta" con "voglio comprare proprio quella marca".

Dominio puro: `Identita` guadagna la variante `Prodotto(i64)` (mai fusa con
`Alimento` a parità di alimento sottostante); `RigaIngrediente` e
`VoceGenerata` guadagnano un campo `prodotto_id: Option<i64>`.

A differenza delle voci manuali libere e delle righe `generato`
pure-planner (cancellate e rigenerate da zero a ogni refresh), le aggiunte
dal catalogo non sono uno snapshot: vivono in una tabella propria
(`liste_spesa_aggiunte_catalogo`, migration
`migrations/20260908160000_lista_spesa_aggiunte_catalogo.sql`, che aggiunge
anche la colonna `prodotto_alimentare_id` a `liste_spesa_voci` e ricrea il
trigger di congelamento per coprirla) e partecipano di nuovo ogni volta al
ricalcolo di `aggiorna_lista`, finché non vengono coperte da una voce
comprata (stesso congelamento di sempre). Salvata l'aggiunta, la lista si
aggiorna subito in automatico (chiamata a `aggiorna_lista`), così l'utente
vede la somma o la nuova riga senza dover premere "🔄 Aggiorna lista" a
mano.

Ricerca in `cerca_nel_catalogo` (dedicata al modulo, stesso schema di
visibilità di `ricette::search_food_choices` ma non riusata da lì: quella
funzione non restituisce i prodotti come risultati distinti). Nessuna
paginazione vera, `LIMIT` come "top N", stesso approccio già in uso in
`ricette.rs`. `novita::REGISTRO` non è stato esteso: è la stessa
funzionalità già registrata l'8 settembre (`lista_spesa`), non una
schermata nuova.

**Non implementato (non richiesto)**: rimuovere una voce o un'aggiunta già
inserita, per nessuna delle vie.

13 nuovi test in `lista_spesa.rs` (7 sul dominio puro: identità `Prodotto`
non fusa con `Alimento`, somma su alimento generico, somma fra prodotti
uguali; 6 sulle funzioni database con `sqlite::memory:`: somma col planner,
riga separata per prodotto, persistenza attraverso un secondo refresh,
congelamento esteso alla nuova colonna, ricerca nel catalogo) e 2 nuovi
test in `identity.rs` (bootstrap crea il profilo, bootstrap non duplica se
già presente), per un totale di **338 (328 prima)**, migration **47 (46
prima)**. Pipeline `fmt`, `check --locked`,
`clippy --all-targets --locked -- -D warnings`, `test --locked` verde in
locale, tutti i 338 test passano.

Un bug reale trovato dai test, non a tavolino: la prima stesura di
`aggrega_ingredienti` propagava sempre `prodotto_id: None` sulla riga
generata (residuo di un inserimento automatico del campo durante il
refactor), così due prodotti identici non si sarebbero mai sommati tra loro
e un prodotto si sarebbe confuso con l'alimento generico in certi casi.
Corretto propagando `riga.prodotto_id`; i due test sull'identità `Prodotto`
sono quelli che l'hanno trovato.

**Non collaudato dal vivo (i due feedback di questa sezione)**: scritti in
un worktree isolato, senza accesso a Telegram né al database reale
dell'S9. Nessuna migration applicata a un database reale.

**Collaudo dal vivo iniziato il 9 settembre 2026, bug reale trovato**:
Alessio ha aperto "🔎 Cerca nel catalogo" e visto ogni risultato con
un'icona 🥕 duplicata (es. "🥕🌾 Pasta", "🥕🏷️ Pasta sfoglia").
Causa: `RisultatoCatalogo::etichetta()` anteponeva un `🥕` fisso a ogni
alimento, ma `alimenti.nome` porta già la propria icona di categoria
incorporata nel testo (`migrations/20260825014500_catalogo_alimenti_base.sql`,
es. `'🌾 Pasta'`, `'🏷️ Pasta sfoglia'`) — l'icona fissa la duplicava
sempre. Corretto usando il nome così com'è, senza prefisso aggiuntivo;
l'icona `🏷️` di un prodotto commerciale specifico (marca +
nome_commerciale, senza icona di categoria propria) non è toccata.

**Tre correzioni in più, dallo stesso giro di collaudo dal vivo (9
settembre 2026)**:

1. **Refresh automatico nell'unico caso in cui deve scattare da solo**:
   togliere la spunta a una voce `generato` la rimette disponibile al
   prossimo refresh, ma se nel frattempo un altro pasto aveva già
   prodotto una riga nuova per la differenza (`sottrai_gia_comprato`),
   restavano due righe frammentate dello stesso alimento finché non si
   premeva "🔄 Aggiorna lista" a mano — visto da Alessio con due volte
   "Pasta · 50 g" invece di una "Pasta · 100 g". `toggle_comprato` ora
   richiama `aggiorna_lista` subito dopo aver tolto la spunta, ma solo
   quando la voce è `generato`: una voce manuale non ha nulla con cui
   fondersi e non viene mai toccata da un refresh, nemmeno indiretto.
2. **`🔄 Aggiorna lista` compare solo se cambierebbe davvero qualcosa**,
   stesso principio già in uso per `🔄 Aggiorna planner` sulle ricette
   cambiate. Nuova funzione pura di confronto `serve_aggiornamento`:
   calcola il fresco dell'aggregazione (stessa logica di `aggiorna_lista`,
   estratta in `calcola_fresche` e condivisa dalle due) e lo confronta con
   le voci generate non ancora comprate già in lista — se coincidono, il
   bottone non compare, e il testo "Nessuna voce nella lista" cambia di
   conseguenza quando non c'è nulla da generare.
3. **Nessun limite di cinque voci per pagina**, eccezione esplicita a C6:
   la lista della spesa è l'unica lista del bot che mostra tutte le voci
   su una sola schermata, perché l'utente deve vederle tutte insieme per
   decidere cosa prendere prima e cosa dopo — spezzarla in pagine
   negherebbe lo scopo della schermata. Il callback `lista_spesa:page:` e
   la relativa riga di paginazione sono stati rimossi insieme alla
   funzione `show_lista_pagina` (unita a `show_lista`).

3 nuovi test in `lista_spesa.rs` (rifusione dopo la deselezione, nessun
refresh su una voce manuale, `serve_aggiornamento` vero/falso secondo lo
stato reale), per un totale di **341 (338 prima)**. Nessuna migration
nuova. Pipeline `fmt`, `check --locked`,
`clippy --all-targets --locked -- -D warnings`, `test --locked` verde in
locale.

**Non ancora ricollaudate dal vivo**: queste tre correzioni sono scritte
e testate in locale, non ancora provate su Telegram.

**Secondo giro di collaudo dello stesso giorno, dopo che le tre correzioni
sopra erano già confermate dal vivo**: Alessio ha trovato tre punti in più.

1. **La fusione mancava nel verso opposto**: spuntare la riga residua
   (invece di deselezionare quella già comprata) lasciava due righe
   comprate separate per sempre, perché `aggiorna_lista` non tocca mai le
   voci comprate. Nuova `fondi_comprate_se_serve`, richiamata da
   `toggle_comprato` quando si spunta (non solo quando si deseleziona):
   cerca un'altra voce `generato` già comprata con la stessa identità e
   unità e le fonde, sommando la quantità nella voce appena spuntata ed
   eliminando l'altra. Il trigger di congelamento non permette di
   cambiare `quantita` mentre `comprato = 1`: la funzione passa da
   `comprato = 0` a `1` due volte con l'aggiornamento della quantità in
   mezzo, dentro un'unica transazione — l'unico modo per farlo restando
   dentro la regola (il toggle stesso resta sempre permesso).
2. **Segnalare l'eccesso**: se il fabbisogno reale scende sotto quanto già
   comprato (un pasto tolto dal planner, una ricetta ridotta), prima non
   c'era modo di saperlo — la voce comprata resta sempre congelata, giusto
   così, ma l'utente deve poterlo sapere. Nuova `calcola_eccessi` (dominio
   puro) confronta il fabbisogno grezzo — *prima* di sottrarre il
   comprato, funzione condivisa `fresche_grezze` estratta da
   `calcola_fresche` — con quanto è già segnato comprato; la differenza,
   se positiva, è l'eccesso. Mostrato su ogni voce coinvolta (`· ⚠️ 150 g
   in eccesso`) più un avviso generale in testa alla schermata — pura
   informazione, nessuna correzione automatica della voce comprata.
3. **Ordine indipendente da comprato, e riordino manuale**: prima l'ordine
   dipendeva da `comprato ASC`, quindi spuntare una voce la faceva saltare
   in fondo alla lista — non voluto. Nuova colonna `ordinamento` su
   `liste_spesa_voci` (migration
   `migrations/20260909180000_lista_spesa_ordinamento.sql`, additiva, le
   righe esistenti mantengono l'ordine che avevano per `id`): `carica_voci`
   ordina per `ordinamento ASC, id ASC`, indipendente da `comprato`. Una
   voce nuova prende sempre il prossimo `ordinamento` disponibile, in coda
   — mai in mezzo a un ordine sistemato a mano. Nuova modalità dedicata
   "↕️ Riordina lista" (visibile solo con più di una voce): ogni voce
   mostra `⬆️`/`⬇️` invece del checkbox, `sposta_voce` scambia
   `ordinamento` con la voce vicina in quella direzione.

7 nuovi test in `lista_spesa.rs` (fusione al check, isolamento delle voci
manuali dalla fusione, spostamento su/giù, tre sul dominio puro degli
eccessi, un eccesso rilevato per davvero dopo aver tolto un pasto dal
planner), per un totale di **348 (341 prima)**, migration **48 (47
prima)**. Pipeline `fmt`, `check --locked`,
`clippy --all-targets --locked -- -D warnings`, `test --locked` verde in
locale.

**Collaudate dal vivo lo stesso giorno**: eccesso segnalato e riordino
manuale funzionano (Alessio ha visto l'avviso di eccesso su più voci dopo
aver ridotto l'intervallo della lista, e ha spostato voci con "↕️ Riordina
lista"). Due difetti estetici trovati dal vivo, non a tavolino:

1. **L'etichetta dell'eccesso veniva troncata**: su una voce con nome
   lungo, Telegram tagliava il testo con "…" invece di andare a capo da
   solo (visto con "125 in ecc…"). Corretto anteponendo un `\n` invece di
   " · " prima di "⚠️ ... in eccesso": il pulsante ora occupa una riga in
   più invece di tagliare il testo.
2. **La schermata di riordino disallineava le etichette**: `riordina_row`
   ometteva la freccia assente alle estremità (nessun `⬆️` sulla prima
   voce, nessun `⬇️` sull'ultima), quindi l'etichetta cambiava colonna riga
   per riga a seconda di quali frecce c'erano. Corretto rendendo la riga
   **sempre** di tre pulsanti nello stesso ordine: alle estremità la
   freccia resta al suo posto ma diventa un no-op (`lista_spesa:noop`),
   stesso trattamento già riservato al contatore di pagina non premibile
   altrove nel bot.

Nessun nuovo test (modifiche solo di formattazione/struttura della
tastiera, non di logica): 348 test invariati (vedi il conteggio corrente
in sezione 3, non ripetuto qui in grassetto per non confondere il
controllo automatico di `aggiorna-s9.sh`, che cerca il primo "**N
test**" del documento). Pipeline `fmt`,
`check --locked`, `clippy --all-targets --locked -- -D warnings`,
`test --locked` verde in locale.

**Diventata regola globale, non solo della lista della spesa** (chiesto da
Alessio subito dopo): nuova **C15** in `docs/convenzioni-telegram.md` — una
parte opzionale o di lunghezza variabile aggiunta a un'etichetta (un
avviso, un sottotitolo) va sempre a capo, mai concatenata con " · ", perché
Telegram non va a capo da solo su un pulsante troppo largo: taglia con "…".
Cercato ogni altro punto del bot che concatena etichette di pulsanti con
" · ": un solo caso analogo reale, `dynamic_filter_keyboard` in
`src/modules/storico.rs` (etichetta di un filtro + sottotitolo opzionale),
corretto allo stesso modo. Gli altri quattro punti trovati con lo stesso
pattern (`planner_alimentare.rs`, due in `profili_alimentari.rs`,
`ricette.rs`) compongono testo di messaggio, non etichette di pulsante —
Telegram lo va a capo da solo, nessun rischio, lasciati invariati. La
quantità di una voce della lista della spesa resta sulla stessa riga del
nome: è sempre presente e breve, non la parte che causava il taglio.

**Quarto giro di collaudo dal vivo (10 settembre 2026)**: un bug reale e
due mancanze.

1. **Un refresh qualunque rimescolava un ordine già sistemato a mano**,
   non solo quello scatenato dal deseleziona come sembrava dal collaudo
   precedente: `aggiorna_lista` cancella e re-inserisce sempre da zero le
   voci generate non comprate, e prima assegnava un `ordinamento` nuovo a
   ognuna, sempre in coda. Corretto rileggendo l'`ordinamento` di ogni
   voce generata non comprata *prima* di cancellarle, e riassegnandolo
   alla stessa identità nel fresco ricalcolato: solo un'identità
   **davvero nuova** prende un ordinamento nuovo. Un refresh senza
   cambiamenti reali (`serve_aggiornamento` direbbe falso) ora non
   sposta più nulla.
2. **L'unità di misura andava sempre scritta anche per un'aggiunta dal
   catalogo**, mentre l'alimento/prodotto scelto spesso ha già un'unità
   naturale. Nuova `valida_quantita_con_default`: se scrivi solo il
   numero, usa l'unità predefinita dell'alimento (`unita_predefinita_id`,
   opzionale) o del prodotto (`unita_confezione_id`, sempre presente);
   scrivere comunque un'unità la sovrascrive. Un alimento senza unità
   predefinita richiede comunque di scriverla, come la voce libera
   (`valida_quantita_manuale`, invariata).
3. **Non si poteva rimuovere né una voce manuale né un'aggiunta dal
   catalogo.** Nuova schermata "🗑️ Rimuovi voci" (visibile solo se c'è
   qualcosa da rimuovere): un pulsante per ogni voce manuale
   (`rimuovi_voce_manuale`, cancellazione diretta, anche se comprata — il
   congelamento protegge la quantità di una voce comprata, non la sua
   esistenza) e ogni aggiunta dal catalogo (`rimuovi_aggiunta_catalogo`,
   cancella la riga in `liste_spesa_aggiunte_catalogo` e rilancia subito
   `aggiorna_lista`, che ricalcola la riga `generato` collegata secondo il
   fabbisogno rimasto). Le righe `generato` pure-planner restano escluse:
   le gestisce il planner.

7 nuovi test in `lista_spesa.rs` (ordine che sopravvive a un refresh senza
cambiamenti reali, tre sulla quantità con unità predefinita, rimozione
voce manuale anche comprata, rimozione aggiunta catalogo che fa tornare la
riga al solo fabbisogno del planner, `voci_rimovibili` che esclude le
righe del planner), per un totale di **355 (348 prima)**. Nessuna
migration nuova. Pipeline `fmt`, `check --locked`,
`clippy --all-targets --locked -- -D warnings`, `test --locked` verde in
locale.

**Non ancora ricollaudate dal vivo**: anche queste tre correzioni sono
scritte e testate in locale, non ancora provate su Telegram.

**Un'altra piccola correzione trovata dal vivo (10 settembre 2026)**: dopo
l'ultima rimozione da "🗑️ Rimuovi voci", la schermata restava lì, ormai
vuota — un vicolo cieco che richiedeva comunque "⬅️ Indietro" per uscirne.
Nuova `mostra_dopo_rimozione`: se non resta più nulla da rimuovere dopo una
rimozione riuscita, porta direttamente alla lista principale.

## 2quinquies. Turni e routine — prima fetta scritta il 10 settembre 2026, collaudo dal vivo su Telegram da fare

Prossimo blocco della sequenza Alimentazione dopo la lista della spesa
(deciso con Alessio l'8 settembre 2026), con il design di dettaglio del 10
settembre 2026. Specifica completa in `docs/previsto/turni-e-routine.md`
(stato aggiornato: prima fetta scritta, il resto resta previsto);
dettaglio del modulo in `docs/moduli/turni-e-routine.md`.

**Dentro questa fetta**: un **modello** turno (`turno_modelli` +
`turno_modello_pasti`, nuovo modulo `src/modules/turni.rs`) con nome
scelto dall'utente e una lista di pasti-modello (tipo pasto — stesso
vocabolario del planner, colazione/spuntino_mattina/pranzo/
spuntino_pomeriggio/cena/altro —, orario opzionale HH:MM, situazione
casa/lavoro/fuori/saltato/altro, preparazione anticipata con nota
opzionale, nota libera). **Assegnare** un modello a una data per un
profilo alimentare (`turno_assegnazioni` + `turno_assegnazione_pasti`)
**copia** i pasti del modello in quel momento — stesso principio di
snapshot già usato da planner e lista della spesa: modificare o rimuovere
un pasto assegnato non tocca mai il modello, e rinominare/archiviare il
modello dopo non cambia le assegnazioni già fatte (il nome resta uno
snapshot, `modello_nome_snapshot`).

Nuova sezione "📋 Turni e routine" raggiungibile da "🍽️ Alimentazione",
con le schermate minime richieste: elenco modelli (paginato da
`modules::liste` sopra la soglia) con crea/rinomina/archivia; dentro un
modello l'elenco dei suoi pasti con l'aggiunta guidata (tipo da bottoni,
poi orario libero HH:MM o salta, poi situazione da bottoni, poi
preparazione sì/no con nota opzionale, poi nota libera opzionale) e la
rimozione diretta di un pasto; assegnazione a una data (calendario da
`modules::calendario`, scelta del profilo con la stessa visibilità già
usata dal planner) con conferma esplicita se quel profilo ha già un turno
in quel giorno (sostituzione, non fusione); una schermata "vedi/modifica
assegnazione" che parte dal profilo, passa dal calendario (marcato con
`•` sui giorni già assegnati) e arriva al singolo pasto assegnato, dove
situazione/orario/preparazione/nota si modificano uno per volta senza
mai toccare il modello.

**Relazione col planner, solo lettura**: `planner_show_day` (schermata
"Giorno" di `planner_alimentare.rs`) ora chiama
`turni::info_giorno_per_planner`, che — se esiste un'assegnazione turno
per quella data nello stesso spazio — antepone un blocco testuale con i
pasti del turno il cui tipo **non ha ancora** un pasto vero pianificato
per quel giorno (calcolato dal dominio puro `pasti_da_segnalare`, testato
senza database). Nessun bottone, nessuna scrittura su `planner_pasti`:
l'unica modifica a `planner_alimentare.rs` è queste sei righe nella
funzione di visualizzazione, per non rischiare la logica già in
produzione (segnalazione ricetta cambiata, congelamento a completamento,
ecc.) che quel modulo gestisce da solo.

**Scelte prese in autonomia**:

- il modello/l'assegnazione appartengono allo spazio predefinito
  dell'attore corrente (`identity::current_actor().spazio_id`, mai
  nullo), esattamente come `planner_alimentari` e `liste_spesa` — nessuna
  schermata per scegliere lo spazio, coerente con quei due moduli;
- l'orario è validato con una piccola reimplementazione di
  `spazi_membri::valid_time_strict` (quella funzione è privata al suo
  modulo): duplicazione minima, stessa scelta già fatta per le icone dei
  tipi pasto (`meal_emoji`, copiate da `planner_alimentare::MealType`,
  anch'esse private lì);
- rimuovere un pasto (dal modello o da un'assegnazione) è immediato,
  senza un passo di conferma — stesso stile di `rimuovi_voce_manuale`
  nella lista della spesa;
- il blocco informativo nel planner è scoperto per **spazio**, non per
  singolo profilo/partecipante del pasto pianificato: la schermata
  "Giorno" del planner non ha oggi un concetto di "profilo corrente" (un
  pasto può avere più partecipanti), quindi il confronto è fra i tipi
  pasto già pianificati in quello spazio quel giorno e i pasti di
  *qualunque* assegnazione turno dello stesso spazio in quella data — una
  approssimazione dichiarata, non un'integrazione più profonda;
- nessun badge "🆕" (`novita::REGISTRO`): lasciato fuori per restare nei
  tempi di questa fetta, come esplicitamente concesso dalla richiesta.

**Fuori scope, dichiarato in `docs/roadmap.md`**: condivisione di un
modello nello spazio/copia indipendente/invio a un altro utente; reminder
alla creazione (l'infrastruttura reminder — email, scheduler — non esiste
ancora nel progetto); riordino dei pasti di un modello (solo aggiunta e
rimozione in questa fetta).

Nuova migration additiva `migrations/20260910120000_turni_e_routine.sql`
(quattro tabelle: `turno_modelli`, `turno_modello_pasti`,
`turno_assegnazioni`, `turno_assegnazione_pasti`), documentata in
`docs/database.md` e `docs/moduli/turni-e-routine.md`. Nuova convenzione
**C15** in `docs/convenzioni-telegram.md`: le parti aggiunte a
un'etichetta con più di due elementi vanno a capo con `\n`, non accodate
con "·", perché Telegram tronca senza avviso il testo di un pulsante
troppo lungo su una riga sola — applicata alle etichette dei pasti (tipo,
orario, situazione, preparazione) sia nel modello sia nell'assegnazione.

17 nuovi test in `src/modules/turni.rs` (7 di dominio puro — validazione
nome/nota/orario, andata e ritorno dei token di situazione, calcolo dei
pasti ancora da segnalare — e 10 su `sqlite::memory:` — creazione/
rinomina/archiviazione del modello, aggiunta/rimozione di un pasto
modello, assegnazione che copia senza toccare il modello, snapshot del
nome del modello che sopravvive a una rinomina successiva, disattivare la
preparazione anticipata che azzera la nota, rimozione di un pasto
assegnato che non tocca gli altri né il modello, il blocco per il planner
che nasconde un pasto già pianificato per davvero), per un totale di
**372 (355 prima)**. Una migration nuova (49 nel repository, 48 prima).
Pipeline `fmt`, `check --locked`, `clippy --all-targets --locked -- -D
warnings`, `test --locked` verde in locale.

**Scritto, collaudo dal vivo su Telegram da fare**: nessun accesso a
Telegram/S9 in questo worktree isolato — non è stato verificato con un
avvio reale del bot, solo con la pipeline automatica.

## 2sexies. Turni e routine — tredici correzioni dell'11 settembre 2026, primo collaudo dal vivo di Alessio

Alessio ha collaudato dal vivo su Telegram la prima fetta di Turni e
routine (sezione precedente) e ha segnalato tredici correzioni, tutte
discusse e decise con lui prima di scriverle. Dettaglio completo in
`docs/moduli/turni-e-routine.md` (che elenca i tredici punti uno per uno)
e `docs/database.md` (sezione "Step 7.4ter"). Nuova migration additiva
`migrations/20260911090000_turni_correzioni_collaudo.sql` — la migration
originale del 10 settembre, già applicata all'S9, non è mai stata
toccata.

**Fatti tutti e tredici i punti**:

1. Ordine cronologico dei pasti per orario, senza orario in fondo.
2. Un solo pasto per tipo, indice UNIQUE a database + messaggio leggibile.
3. Assegnare un turno dalla schermata Giorno del planner.
4. Orario del pasto vero del planner, con default proposto (mai imposto).
5. Indicatore visivo (`🗓️`/`◆`) per i giorni con turno assegnato.
6. Orario senza zero iniziale ("7:30"), validazione condivisa fra
   `turni.rs` e `planner_alimentare.rs` invece di duplicata.
7. Modifica di un pasto del modello uniformata a quella dell'assegnazione.
8. Modelli archiviati con una schermata per ripristinarli.
9. Conferma esplicita prima di ogni eliminazione definitiva — nuova
   convenzione **C16**, vedi sotto per l'audit sul resto del bot.
10. "🔄 Aggiorna assegnazione" quando il modello cambia dopo, stessa
    logica di "🔄 Aggiorna planner".
11. Default orari fissi per tipo di pasto (parte del punto 4).
12. Guida obbligata alla creazione del modello (i 5 tipi fissi in ordine,
    situazione prima di orario, "saltato" senza orario né preparazione) e
    le due incoerenze turno↔planner (un avviso mai un blocco quando si
    pianifica comunque un pasto "saltato"; una scelta esplicita quando si
    assegna un modello che segna "saltato" un pasto già pianificato per
    davvero).
13. Il modello appartiene a un profilo, non l'assegnazione: nuove colonne
    `turno_modelli.profilo_alimentare_id`/`profilo_nome_snapshot`, con
    backfill dei modelli di test già esistenti sul database reale (es.
    "Gelateria") al profilo "sé stesso" del proprietario. Nuovo
    "📤 Copia per un altro profilo": copia indipendente, mai collegata
    all'originale dopo la copia.

**Scelte prese in autonomia**, oltre a quelle già scritte nel documento
del modulo:

- Il nome di un modello copiato per un altro profilo include il nome del
  profilo di destinazione tra parentesi, per non violare l'indice UNIQUE
  su `(spazio_id, nome_normalizzato)` già in produzione (un errore reale
  trovato dai test: la prima versione teneva lo stesso nome esatto e il
  test di copia falliva con l'errore grezzo del database).
- Il giro guidato dei 5 tipi fissi (punto 12) riprende da solo anche
  quando si preme `➕ Aggiungi pasto` su un modello esistente a cui
  mancano ancora dei tipi fissi, non solo appena dopo la creazione: la
  scelta del tipo sparisce dalla UI finché resta un tipo fisso mancante,
  invece di offrire due percorsi diversi (creazione vs aggiunta
  successiva) per la stessa cosa.
- `turno_modelli.aggiornato_il` viene ora toccato da ogni
  aggiunta/modifica/rimozione di un pasto del modello (`tocca_modello`),
  non solo da rinomina/archiviazione come prima: è il segnale che il
  punto 10 confronta con lo snapshot preso a ogni assegnazione.

**Audit sistematico del punto 9 sul resto del bot**, con un agente di
sola lettura dedicato (non ha modificato nulla): cercato in
`src/modules/*.rs` ogni callback che esegue una `DELETE` (o un'azione
permanente equivalente, es. eliminare un allegato dal disco) senza una
schermata di conferma intermedia.

*Moduli già a posto* (hanno già uno schema conferma/annulla a due passi):
`ricette.rs` (eliminazione ricetta), `oggetti.rs`, `luoghi.rs` (casa e
stanza), `contenitori.rs`, `spazi_membri.rs` (rimozione di un membro),
`miglioramenti.rs` (eliminazione di un miglioramento ed eliminazione di
tutti gli scartati), `planner_alimentare.rs` (eliminazione di un pasto).
Zero punti di eliminazione trovati in `porzioni.rs`, `storico.rs`,
`distribuzione.rs`, `collaudo.rs`, `novita.rs`, `foto.rs` (le sue funzioni
di cancellazione file sono interne al flusso già confermato di
`oggetti.rs`), `vestiti.rs`/`veicoli.rs` (moduli non ancora costruiti) e
`profili_alimentari.rs` (usa archiviazione/soft-delete, non una `DELETE`
vera su un profilo).

*Cinque punti trovati e corretti allo stesso modo* (schermata "sei
sicuro?" con `✅ Sì, elimina` / `❌ Annulla`, callback `...:ask:`/`...:yes:`):

- `ricette.rs`: eliminazione di uno step del procedimento
  (`recipe:edit:step:del:ask:`/`:yes:` — accorciato da "delete" a "del"
  per restare sotto il limite di 64 byte di un callback Telegram con id
  al valore massimo, verificato dal test già esistente
  `callback_ricette_restano_sotto_il_limite_telegram`, esteso con i nuovi
  callback) e del suo allegato (`recipe:edit:md:ask:`/`:yes:`, elimina
  anche il file dal disco).
- `spazi_membri.rs`: revoca di un invito
  (`space-members:invite:revoke:ask:`/`:yes:`).
- `miglioramenti.rs`: eliminazione di uno screenshot/video
  (`improve:photo:delete:ask:`/`:yes:`, elimina anche il file dal disco) e
  di un singolo miglioramento scartato
  (`improve:delete_discarded:ask:`/`:yes:` — il "elimina tutti" aveva già
  la conferma, il singolo no).

*Lasciato non toccato perché ambiguo, da rivedere con Alessio* (non si è
indovinato, come richiesto): `alimentazione.rs`
(`food:product:nutrition:remove:`, azzera i valori nutrizionali di un
prodotto in un solo tocco — non chiaro se vada trattato come
un'eliminazione ai fini di C16 o come una normale modifica di un campo);
`porzioni_profili.rs` e `porzioni_ingredienti.rs` (i "reset" del fattore
di porzione o degli override di un ingrediente a un profilo cancellano
righe a database, ma ripristinano un valore calcolato di default,
recuperabile rifacendo la modifica — sembra più vicino a un normale
`↩️` che a una perdita di dati, ma non è stato deciso con certezza);
`miglioramenti.rs` (`improve:tutorial:elimina:`, la stessa funzione di
eliminazione allegato usata dal tutorial guidato del punto 9 sopra, ma lì
è già dentro una schermata "tieni/elimina" a scelta binaria subito dopo
l'invio di prova — sembra già una conferma di fatto, solo con un nome di
callback diverso dallo schema `ask`/`yes`, non modificato per non
rischiare di rompere il tutorial guidato collaudato).

**13 nuovi test, tutti in `src/modules/turni.rs`** (2 di dominio puro —
orario senza zero iniziale, pasto saltato mai segnalato come mancante — e
11 su `sqlite::memory:` — ordine cronologico, vincolo di unicità sul tipo
pasto, modifica di un pasto del modello, archiviazione/ripristino, tipi
fissi mancanti, creazione con profilo obbligatorio, copia per un altro
profilo indipendente, assegnazione che eredita il profilo, "aggiorna
assegnazione" e il suo confronto, conflitto turno "saltato"/pasto già
pianificato), per un totale di **385 (372 prima)**. I cinque punti
dell'audit corretti in `ricette.rs`/`spazi_membri.rs`/`miglioramenti.rs`
non hanno test nuovi: riusano funzioni di eliminazione già testate,
aggiungendo solo una schermata di conferma nella UI. Una migration nuova
(50 nel repository, 49 prima). Pipeline `fmt`, `check --locked`,
`clippy --all-targets --locked -- -D warnings`, `test --locked` verde in
locale, tutti i 385 test passano.

**Punti lasciati intenzionalmente più leggeri, da rivedere prima del
collaudo**:

- Il passo dell'orario del planner (punto 4) e l'indicatore dei giorni
  con turno (punto 5) toccano `planner_alimentare.rs`, che non ha un
  proprio modulo di test su `sqlite::memory:` (solo test di dominio puro,
  già esistenti prima di questo lavoro): la nuova logica riusa funzioni di
  `turni.rs` già testate lì (`valida_orario`, `orario_suggerito_da_turno`,
  `turno_segna_saltato`, `giorni_con_assegnazione_spazio_tra`), ma il
  nuovo passo UI in sé (schermata orario, avviso di incoerenza 1) non ha
  un test automatico dedicato — stessa lacuna di collaudo di tutto il
  resto della UI Telegram del progetto, che si verifica sempre a mano
  sull'S9, mai scritta qui come se fosse verificata quando non lo è.
- Le quattro ambiguità dell'audit del punto 9, elencate sopra, restano
  aperte: nessuna delle due letture (serve conferma / non serve) è stata
  scelta a caso.

**Ambiguità del punto 9 decise con Alessio l'11 settembre 2026**: azzerare
i valori nutrizionali di un prodotto (`alimentazione.rs`,
`food:product:nutrition:remove:`) **rientra** in C16 — corretto con lo
stesso schema `ask`/`yes` degli altri cinque punti dell'audit, nessun test
nuovo (riusa `remove_product_nutrition` già esistente). Il "reset" di una
porzione/override a un profilo (`porzioni_profili.rs`/
`porzioni_ingredienti.rs`) **non rientra**: torna a un valore calcolato di
default, non è una perdita di dati — lasciato invariato. Il tutorial
guidato dell'allegato in Miglioramenti (`improve:tutorial:elimina:`) **è
già conforme nella sostanza** — non toccato, per non rischiare di rompere
un flusso già collaudato dal vivo.

**Scritto, collaudo dal vivo su Telegram da fare**: nessun accesso a
Telegram/S9 in questo worktree isolato — non è stato verificato con un
avvio reale del bot, solo con la pipeline automatica.

## 2septies. Turni e routine — dieci correzioni del secondo collaudo dal vivo (12 settembre 2026)

Alessio ha fatto un secondo giro di collaudo dal vivo su Telegram delle
tredici correzioni della sezione precedente e ha segnalato dieci ulteriori
correzioni, tutte discusse e decise con lui prima di scriverle. Dettaglio
completo in `docs/moduli/turni-e-routine.md` (che elenca i dieci punti uno
per uno). **Nessuna migration nuova**: tutti i punti lavorano su dati e
schema già esistenti (verificato, non assunto: la cascata `ON DELETE
CASCADE` su `turno_modello_pasti` e `turno_assegnazione_pasti`, e `ON
DELETE SET NULL` su `turno_assegnazioni.modello_id`, erano già nella
migration del 10 settembre, mai toccata).

**Fatti tutti e dieci i punti**:

1. Tre bug identici di ordine dei callback (`turni:model:copy:`,
   `turni:assignhere:`, `turni:model:setprofile:`): un controllo generico
   scritto prima dei suoi corrispondenti più specifici nello stesso `if
   let` a catena lo faceva sempre intercettare per primo, perché le
   stringhe specifiche iniziano comunque per il suo stesso prefisso —
   sintomo osservato da Alessio: un pulsante sembrava "non fare nulla" (in
   realtà ricaricava la stessa schermata agganciata a un id di ripiego,
   `0`), e tornando indietro compariva "⚠️ Pulsante non valido o non più
   disponibile.". Corretti spostando i tre controlli generici dopo le
   varianti specifiche. Ricontrollato l'intero file per lo stesso schema:
   nessun altro punto trovato (i punti `apasto`/`mpasto` già segnalati
   come verificati la volta scorsa restano corretti).
2. "❌ Annulla" dalla scelta tipo pasto con solo "🍴 Altro" tornava
   all'elenco generale dei modelli invece che al modello di partenza,
   perché quella schermata non salva un draft e il gestore generico
   dell'annullamento dipende dal draft per sapere dove tornare. Corretto
   passando l'id del modello direttamente nel callback di quella
   schermata specifica.
3. Icona "Archivia" uniformata a 📦 in tutto il bot (`ricette.rs` e
   `turni.rs` allineati a `profili_alimentari.rs`/`miglioramenti.rs`).
4. Eliminazione definitiva (C16) nei "🗄 Modelli archiviati": un modello
   alla volta o tutti insieme.
5. "🗑 Elimina assegnazione" (C16) nel dettaglio di una singola
   assegnazione.
6. In `profili_alimentari.rs`, "➕ Nuovo profilo" salta la schermata di
   scelta quando l'unico bottone reale rimasto sarebbe "➕ Persona senza
   account" (l'utente ha già un profilo "sé stesso").
7. Correzione testo: "Esempio: Giulia" → "Esempio: Giorgia".
8. Dopo un'assegnazione completata senza tornare dal planner,
   `esegui_assegnazione` mostra ora il dettaglio del modello invece di un
   dettaglio dell'assegnazione mai richiesto.
9. "🔄 Aggiorna assegnazione" anche nella schermata Giorno del planner,
   un bottone per ciascuna assegnazione del giorno con un aggiornamento
   disponibile.
10. Documentata (non costruita) l'idea di sincronizzazione opzionale con
    diritto di veto del proprietario per le entità copiabili — vedi
    `docs/previsto/turni-e-routine.md` per il perché del rinvio.

**Un punto del collaudo non testabile da Alessio da solo**: il punto 32
della guida di collaudo (conferma C16 prima di revocare un invito in
`spazi_membri.rs`) richiede un secondo account Telegram per essere
verificato dal vivo (bisogna vedere l'invito sparire dal punto di vista
dell'invitato). Non essendo testabile in queste condizioni, inserito
direttamente sul database reale dell'S9 via SSH un miglioramento (id 11,
stato "fatto", esito di verifica non ancora impostato) con lo stesso
schema "Fatto · da verificare" già usato in questo file per gli altri
punti implementati ma non ancora ricollaudati dal vivo — non un bug, solo
un promemoria che resta in coda finché Alessio non avrà modo di
verificarlo con un secondo account.

**Scelte prese in autonomia**:

- Per "📤 Copia per un altro profilo" (punto 1), lo smistamento dei tre
  callback è stato estratto in un'unica funzione pura
  (`parse_model_copy_callback`, con l'enum `ModelCopyCallback`) invece di
  limitarsi a riordinare i tre `if let` come per gli altri due bug dello
  stesso punto: essendo questo il caso esplicitamente richiesto per il
  test di regressione end-to-end, centralizzare lo smistamento in una
  funzione testabile da sola (senza un `Bot` vero) garantisce l'ordine
  giusto a prescindere da come viene poi richiamata, invece di affidarsi
  di nuovo solo alla disciplina di scrittura.
- Il test di regressione del punto 1 (`sequenza_copia_dal_bottone_
  produce_una_copia_vera`) simula la sequenza di tre callback che
  l'utente produce premendo i bottoni (avvio → cambio pagina → conferma),
  passandole alla vera funzione di smistamento e poi eseguendo la copia
  per davvero su `sqlite::memory:` — non solo la funzione di dominio
  `copia_modello_per_profilo`, già testata a parte. Gli altri due bug
  dello stesso punto (`turni:assignhere:`, `turni:model:setprofile:`)
  sono stati corretti con lo stesso riordino ma non hanno un test di
  regressione dedicato: nessuna funzione di dominio nuova da testare (il
  bug era solo nell'ordine dei rami), e testare `handle_callback` per
  intero richiederebbe un `Bot` teloxide reale (nessuna libreria di mock
  HTTP nel progetto) — stessa lacuna di collaudo automatico della UI
  Telegram già segnalata più volte in questo file.
- Per il punto 9 (bottone "🔄 Aggiorna assegnazione" nella schermata
  Giorno del planner), il ritorno alla schermata giusta dopo la conferma
  usa lo stesso principio già in uso per `torna_al_planner`
  (`esegui_assegnazione`, punto 3 delle tredici correzioni) e per
  `turni:assign:conflict:keep:` (che porta già `modello_id:data:planner`
  nello stesso callback): qui il callback porta `id:planner:data`,
  interpretato da una nuova funzione pura `parse_refresh_target`.

**4 nuovi test, tutti in `src/modules/turni.rs`** (3 di dominio puro sulla
funzione di smistamento `parse_model_copy_callback` — riconosce `pick`,
`do` e la forma generica, verificando che le prime due non vengano mai
intercettate dalla terza — e 1 su `sqlite::memory:` che simula la
sequenza completa di callback di "📤 Copia per un altro profilo" e
verifica che la copia venga creata per davvero), per un totale di **389
(385 prima)**. Nessuna migration nuova (50 nel repository, invariate). I
restanti nove punti non hanno test nuovi: riusano funzioni di dominio già
testate (punti 4, 5, 8, 9) o sono correzioni di solo testo/ordine UI senza
nuova logica di dominio (punti 2, 3, 6, 7); il punto 10 è solo
documentazione. Pipeline `fmt`, `check --locked`,
`clippy --all-targets --locked -- -D warnings`, `test --locked` verde in
locale su questo worktree.

**Distribuito sull'S9 il 13 settembre 2026** (commit `6ac436d`): CI verde,
`applied_migrations=50` (invariato) confermato nel log di avvio, bot
online. **Collaudato dal vivo da Alessio il 13 settembre 2026**: confermato
tutto corretto tranne i tre punti raccolti nella sezione 2octies (12, 13,
14), che nascono proprio da questo collaudo.

## 2octies. Turni e planner — tre rifiniture dal collaudo del terzo giro (13 settembre 2026)

Collaudando dal vivo le dieci correzioni della sezione 2septies, Alessio
ha confermato che vanno tutte bene e ha segnalato tre rifiniture, tutte
sulla stessa area (assegnazioni turno viste dal planner o dagli
archiviati):

12. **Nella lista "🗄 Modelli archiviati"**: prima il ripristino e
    l'eliminazione definitiva di un modello stavano su due righe separate
    ("♻️ nome" sopra, "🗑 Elimina definitivamente" sotto). Ora stanno sulla
    stessa riga: "♻️ nome" a sinistra, solo l'icona "🗑" (senza scritta) a
    destra — il testo sotto la lista spiega una volta sola che il cestino
    elimina definitivamente, passando comunque dalla stessa schermata di
    conferma di sempre (C16), invariata.
13. **Con due assegnazioni turno lo stesso giorno su profili diversi**, non
    si capiva bene quale turno (modello) fosse associato a quale profilo.
    Il nome del modello ora accompagna sempre il nome del profilo: nel
    blocco testuale "📋 Turno assegnato:" della schermata Giorno del
    planner (`🔸 Alessio · turno «Ufficio»: ...`) e nei bottoni "🔄 Aggiorna"
    generati lì (il nome del turno va a capo nel pulsante, C15).
14. **Eliminare un'intera assegnazione anche dal planner**: prima era
    possibile solo aprendo "📅 Vedi/modifica assegnazione" dal menù Turni.
    Ora la schermata Giorno del planner mostra anche un pulsante "🗑
    Elimina" per ciascuna assegnazione di quel giorno (etichettato con
    profilo e turno, punto 13), con la stessa conferma esplicita (C16) già
    in uso; dopo l'eliminazione si torna alla schermata Giorno, non al
    menù Turni.

**Scelte tecniche**: `assegnazioni_aggiornabili_del_giorno` (punto I della
sezione 2septies) ora restituisce anche il nome del modello, non solo
profilo e id; nuova funzione `assegnazioni_del_giorno` (senza filtro
sull'aggiornamento disponibile) per il punto 14, che riusa la stessa
query di base tramite un piccolo helper condiviso
(`righe_assegnazioni_del_giorno`) invece di duplicarla. Il callback di
eliminazione (`turni:assegnazione:delete:ask:`/`:delete:yes:`) accetta ora
lo stesso suffisso opzionale `:planner:{data}` già usato per "🔄 Aggiorna
assegnazione" (rinominata la funzione di parsing condivisa,
`parse_id_con_ritorno_planner`, da `parse_refresh_target`, visto che ora
serve a due flussi e non solo al refresh) — stesso principio già
consolidato nella sezione 2septies per non introdurre un secondo bug di
ordine dei callback.

**2 nuovi test** in `src/modules/turni.rs`: un test di dominio
(`blocco_distingue_turno_e_profilo_quando_ce_ne_sono_due`, punto 13) e un
test su `sqlite::memory:` (`assegnazioni_del_giorno_elenca_tutte_con_
profilo_e_modello`, punto 14, che verifica anche che eliminando
un'assegnazione l'altra resti intatta) — per un totale di **391 (389
prima)**. Nessuna migration nuova: tutti e tre i punti lavorano su dati e
schema già esistenti. Pipeline `fmt`, `check --locked`,
`clippy --all-targets --locked -- -D warnings`, `test --locked` verde in
locale.

**Distribuito sull'S9 il 13 settembre 2026** (commit `253db44`): CI verde,
`applied_migrations=50` (invariato) confermato nel log di avvio, bot
online. Miglioramenti 12, 13, 14 segnati "fatto" sul database reale via
SSH, in coda per il prossimo collaudo dal vivo di Alessio (non ancora
dichiarati "collaudati").

**Collaudo dal vivo, 14 settembre 2026**: 12 e 14 confermati corretti. Il
punto 13 no — con più pasti di uno stesso turno, ripetere "profilo · turno
«nome»" su ogni riga era, parole di Alessio, "troppo confusionario".
Richiesto: separare visivamente dove inizia il turno di un profilo, e
descrivere i pasti sotto senza ripetere profilo+turno a ogni riga.
Corretto raggruppando: un'intestazione `👤 {profilo} — «{turno}»` una
volta sola per turno, seguita dai soli pasti (tipo, orario, situazione),
sia nel blocco testuale della schermata Giorno del planner sia nel
dominio (`formatta_blocco_routine`, nuova `formatta_riga_pasto` senza
profilo/turno) — i bottoni "🔄 Aggiorna"/"🗑 Elimina" restano invariati
(non erano nella segnalazione). Un test rifatto per riflettere il nuovo
raggruppamento
(`blocco_raggruppa_i_pasti_per_profilo_e_turno_senza_ripetere_l_intestazione`,
sostituisce `blocco_distingue_turno_e_profilo_quando_ce_ne_sono_due`) —
stesso totale di prima, 391 test. Nessuna migration nuova.

**Distribuito sull'S9 il 14 settembre 2026** (commit `3b72ff5`): CI verde,
`applied_migrations=50` (invariato) confermato nel log di avvio, bot
online. Il miglioramento 13 resta "fatto" sul database reale (la nuova
versione sostituisce quella già distribuita e non ancora collaudata) —
da riverificare da Alessio.

## 2nonies. Chiusura della spesa, intervallo da oggi, accesso dal planner e scorte (16 settembre 2026)

Chiesto da Alessio il 16 settembre 2026, **prima** del collaudo delle
correzioni del 10 settembre alla lista della spesa: ragionando su come si
collegherebbe una futura dispensa sono emerse tre mancanze vere, e nello
stesso giro è arrivato il via libera esplicito per costruire anche frigo,
freezer e dispensa.

**1. Chiusura della spesa.** Mancava il momento "spesa fatta": la lista è
una sola e le voci comprate ci restavano per sempre, quindi il giro dopo
ripartiva sporco e `comprato` finiva per dire due cose diverse (l'ho preso
adesso / l'avevo preso la settimana scorsa). `🧾 Chiudi la spesa` archivia
le voci comprate in `liste_spesa_chiusure` + `liste_spesa_voci_archiviate`
e le toglie dalla lista; le non comprate restano. Non è un'eliminazione
(restano in `🗄 Ultima spesa chiusa` e sono la base della dispensa), quindi
la conferma non è quella di C16: dice cosa succede, non che non si può
recuperare. Un trigger blocca ogni `UPDATE` su una voce archiviata. Le
aggiunte dal catalogo già coperte da ciò che si è comprato vengono tolte
insieme (`aggiunte_coperte_dalla_spesa`, dominio puro), altrimenti
tornerebbero il giorno dopo come se non fossero mai state comprate.

**2. La lista parte sempre almeno da oggi.** `intervallo_da_oggi` (dominio
puro) sposta in avanti l'inizio rimasto indietro a ogni apertura; se anche
la fine è passata riparte come il default, oggi + 6 giorni. Un inizio nel
passato resta possibile — serve a recuperare i pasti di ieri — ma è una
scelta esplicita: viene segnalata al momento della scelta e sulla schermata,
e da lì lo spostamento automatico si ferma (`liste_spesa.inizio_manuale`).

**3. Lista della spesa raggiungibile dal planner.** La schermata Settimana
ha ora `🛒 Lista della spesa`; il callback dedicato registra la provenienza
così `⬅️ Indietro` torna al planner e non al menù Alimentazione (C3). Le
schermate interne della lista usano `lista_spesa:back`, che non tocca la
provenienza.

**4. Scorte: dispensa, frigo e freezer** (`src/modules/dispensa.rs`, nuovo,
`🍽️ Alimentazione → 🥫 Scorte`). Decisioni prese da me dove Alessio ha
detto "scegli tu", motivate in `docs/previsto/dispensa.md`: la conservazione
è un'entità propria e non un riuso di case/stanze/contenitori; l'ingresso
automatico della spesa chiusa è **acceso di default** e si spegne con una
preferenza per utente; la **scadenza è opzionale** (decisa da Alessio), si
aggiunge dopo se serve. Elenco paginato per luogo con le scadenze prima,
aggiunta dal catalogo (ricerca condivisa con la lista della spesa) o a mano,
quantità, spostamento fra i tre luoghi, eliminazione con conferma C16.

**Non costruito di proposito, documentato in `docs/previsto/dispensa.md`**:
la sottrazione delle scorte dal fabbisogno della lista della spesa (cambia
il cuore di `calcola_fresche`, collaudato e in produzione: merita un blocco
a sé), lo scarico al consumo di un pasto, la quantità realmente comprata con
il formato della confezione, gli avvisi di scadenza (servono i reminder).

**Badge "🆕" (C14)**: nuova voce `lista_spesa_chiusura` in
`novita::REGISTRO`, con `lista_spesa` che da nodo foglia diventa
intermedio — il badge risale dal pulsante della lista (in Alimentazione e
nel planner) fino al menù principale.

**Un test instabile trovato e corretto, non causato da questo lavoro**:
`assegnazione_da_aggiornare_solo_se_il_modello_e_cambiato_dopo`
(`turni.rs`) falliva circa una volta su tre — creare il modello e
modificarlo dentro lo stesso millisecondo lascia `aggiornato_il` identico,
e il confronto non vede nessun cambiamento. Difetto del test, non del
confronto: risolto con una pausa di 5 ms prima della modifica.

15 nuovi test (5 di dominio puro sulla lista della spesa — spostamento
dell'intervallo e aggiunte coperte —, 4 su `sqlite::memory:` per
archiviazione, pulizia delle aggiunte comprate, trigger di immutabilità e
spostamento automatico; 3 di dominio puro sulle scorte — scadenza nei due
formati, etichetta a capo, token dei luoghi — e 3 su `sqlite::memory:` —
ciclo di vita di una scorta, ordine per scadenza, preferenza dell'ingresso
automatico). Due migration nuove.

**Distribuito sull'S9 il 16 settembre 2026** (commit `c6c71d9`): CI verde
(run #141 sul commit giusto), 406 test e clippy verdi anche sulla toolchain
del telefono, backup del database creato e le due migration nuove provate
prima su una copia; `applied_migrations=52` e `Gestionale Casa online`
confermati nel log di avvio. **Collaudo dal vivo su Telegram ancora da
fare**, sia per queste novità sia per le correzioni del 10 settembre alla
lista della spesa e per il punto 13 dei turni.

## 2decies. Dal collaudo della spesa e delle scorte: netto, resoconto, scarico coi pasti (17 settembre 2026)

Alessio ha collaudato dal vivo la sezione 2nonies e, durante il collaudo, ha
chiesto altro. Tutto discusso con lui prima di scrivere codice; dove ha detto
"scegli tu" la scelta è motivata nei documenti dei moduli. Dettaglio in
`docs/moduli/lista-spesa.md`, `docs/moduli/dispensa.md`,
`docs/moduli/planner.md`, `docs/moduli/ricette.md` e nella nuova convenzione
**C17** (date leggibili). Una migration nuova,
`migrations/20260917090000_scorte_netto_e_aggiornamento.sql`.

**Esito del collaudo della sezione 2nonies**:

- **Blocco A (chiusura)**: la roba comprata e chiusa ricompariva subito come
  non comprata — difetto di progettazione mio, che un test del 16 settembre
  verificava perfino come comportamento giusto. Risolto con la lista al
  netto delle scorte.
- **Blocco B (intervallo)**: l'avviso dell'inizio nel passato arrivava
  troppo tardi; ora al tocco del giorno, con "tienila" o "cambiala".
- **Blocco D (scorte)**: confezioni uguali in righe separate; ora sommate,
  con il dettaglio delle confezioni e delle loro scadenze.
- **Riordino di una voce spuntata "morto"**: diagnosticato leggendo una copia
  del database reale (cancellata subito dopo) — due voci con la stessa
  posizione. Corretto nel ricalcolo e ripulito dalla migration.
- Blocchi C (planner → lista) ed E (correzioni del 10 settembre, turni punto
  13): nessun difetto riportato.

**Costruito**:

1. **Lista al netto delle scorte** (`sottrai_scorte`), esclusi i pasti già
   scaricati. Un prodotto in casa copre l'alimento generico, non il
   contrario.
2. **Aggiornamento automatico con resoconto**, preferenza per persona attiva
   di default: all'apertura (solo se serve) e nei casi di prima; aggiunge,
   toglie, alza, abbassa, e lo dice sempre — riepilogo in testa, dettaglio
   con il motivo in `📋 Ultimi cambiamenti`, salvato a database. Confronta i
   totali per alimento, così spuntare e despuntare non producono rumore.
3. **Destinazione per alimento**: scelta a mano per spazio, poi nome, poi
   categoria. Frutta decisa dopo una ricerca su come si conserva.
4. **Scarico coi pasti**: `🍲 Segna come preparato` (colonna, non un nuovo
   stato), consumato, o da solo a orario passato; una volta sola per pasto;
   restituzione esatta se il pasto viene saltato dopo uno scarico
   automatico, o se viene modificato. Non c'è uno scheduler: il controllo
   gira all'apertura di lista, scorte e ricette con quello che ho.
5. **`🔁 Sostituisci` un pasto consumato** — l'unico punto che elimina un
   pasto consumato, solo su richiesta esplicita.
6. **Ricette con quello che ho**, da Scorte e da Ricette.
7. **`📅 Planner` nella lista**, con la memoria di provenienza nel planner
   (`planner:menu:lista` / `planner:menu:alimentazione`) e senza giri
   infiniti fra le due schermate.
8. **Date leggibili** in tutto il bot (C17), in un punto solo
   (`calendario::display_date`).
9. Messaggio della chiusura riscritto; l'eccesso mostra di nuovo l'unità.

**Scelte prese in autonomia** (oltre a quelle delegate da Alessio):

- lo stato del pasto **non** cambia da solo a orario passato — si scalano
  solo le scorte — per non congelare un pasto che magari è stato saltato;
- il pasto preparato e poi saltato **non** restituisce le scorte: il cibo è
  stato usato comunque;
- la migration considera già scaricati i pasti dei giorni passati, per non
  svuotare le scorte di oggi al primo avvio;
- per un pasto pianificato "sostituire" è il `✏️ Modifica` di sempre (ora
  con il ricalcolo delle scorte), per non avere due pulsanti per la stessa
  cosa;
- la preferenza "aggiornamento automatico" spenta ferma anche il ricalcolo
  alla despunta introdotto il 9 settembre: con la preferenza spenta la lista
  cambia solo quando lo chiede l'utente.

**Un difetto trovato dai test, non dal bot**: le regole per nome mandavano i
"pomodori ciliegini" in frigo come ciliegie; ora la verdura da tenere fuori
viene controllata prima della frutta da frigo.

13 nuovi test (5 di dominio e 4 su `sqlite::memory:` nella lista della
spesa; nelle scorte 2 di dominio e 2 su database in più, con due test
esistenti riscritti), per un totale di 419. Il test di chiusura del 16
settembre che verificava la voce che ricompare ora verifica il contrario.

**Distribuito sull'S9 il 17 settembre 2026** (commit `73654e4`): CI verde
(run #143 sul commit giusto), 419 test e clippy verdi anche sul telefono,
backup del database creato, migration provata prima su una copia;
`applied_migrations=53` e `Gestionale Casa online` confermati nel log di
avvio. I controlli sul telefono ora girano staccati dalla sessione SSH, con
il PID salvato: il 16 settembre una caduta della connessione durante i test
aveva interrotto il primo tentativo. **Collaudo dal vivo su Telegram da
fare.**

**Rimandato al prossimo giro, su richiesta di Alessio**: Documenti,
Promemoria, Palestra e Soldi nel menù principale — domande aperte fatte,
risposte da raccogliere (`docs/roadmap.md`).

## 2undecies. Consegna A: ricette, esiti dei pasti, controllo delle scorte, confezione presa (17 settembre 2026)

Dal collaudo dal vivo di Alessio della sezione 2decies, più le richieste
arrivate durante il collaudo; via libera esplicito ("parti con la consegna
A"). Una migration nuova,
`migrations/20260917180000_consegna_a_pasti_e_spesa.sql`. Dettaglio in
`docs/moduli/ricette.md`, `docs/moduli/planner.md`,
`docs/moduli/dispensa.md`, `docs/moduli/lista-spesa.md`,
`docs/moduli/turni-e-routine.md`. La **consegna B** (negozi, prezzi,
prodotti preferiti, codice a barre) è concordata ma non ancora scritta:
`docs/roadmap.md`.

**Difetti trovati dal collaudo**:

- **Ricette senza ingredienti**: ogni ricetta sembrava vuota. La lettura
  cercava la colonna `notes` ma quella vera è `note`; falliva sempre, e chi
  la chiamava nascondeva l'errore. Il difetto c'era dal 26 agosto. Corretto
  l'alias, e gli errori di lettura ora si vedono ("⚠️ Non riesco a leggere
  gli ingredienti.") invece di sembrare una ricetta vuota.
- **Un pasto saltato non si poteva più cambiare**: il trigger del 31 agosto
  bloccava tutto. Ora il pasto saltato o consumato torna a pianificato con
  `↩️ Riporta a pianificato`; il resto del pasto resta bloccato.
- **L'icona del pasto preparato non cambiava** nell'elenco del giorno: ora
  `🍲`.
- **Il giorno della settimana compariva due volte** in alcune intestazioni
  dopo C17: tolto.
- **Nessun controllo delle scorte** segnando preparato o consumato: ora c'è.

**Costruito**:

1. **Ingredienti delle ricette**: una riga per ingrediente, il nome apre la
   modifica di quantità e unità, `🗑` accanto con conferma (C16).
2. **`↩️ Riporta a pianificato`** sui pasti saltati e consumati. Consumato
   senza preparazione: le scorte tornano. Preparato: restano tolte, il cibo
   era già cucinato.
3. **Saltare un pasto preparato chiede** "Gli ingredienti preparati li hai
   ancora?": sì li rimette nelle scorte, no li lascia tolti. Questo
   sostituisce la scelta autonoma della sezione 2decies ("il preparato
   saltato non restituisce").
4. **Controllo delle scorte** prima di preparato e consumato: se manca
   qualcosa si vede cosa e si conferma. Nello scarico automatico a orario
   passato nessuno può confermare: si prende quello che c'è, la mancanza
   resta annotata sul pasto (`scorte_mancanti`) e compare come avviso
   all'apertura di lista e scorte e nel dettaglio del pasto.
5. **Il proprio profilo già spuntato** scegliendo i partecipanti di un
   pasto, e **`⭐` in cima** all'elenco dei profili nei turni.
6. **C17 anche in Oggetti.**
7. **"📦 Ho preso…"** nella lista della spesa: accanto a ogni voce con una
   quantità, per segnare la confezione presa (tra i prodotti registrati per
   quell'alimento) o scrivere la quantità. Segnarla spunta la voce;
   togliere la spunta la dimentica. Chiudendo la spesa entra in casa la
   quantità presa, e l'archivio ricorda quanto serviva.
8. **Conservazione**: per zucchine, cetrioli, fagiolini, melanzane,
   peperoni e mele Alessio ha scelto il frigo, contro le indicazioni del
   Ministero della Salute. Annotato in `docs/moduli/dispensa.md`.

**Scelte prese in autonomia**:

- una voce con la presa segnata non si fonde più con altre righe comprate
  dello stesso alimento: la somma di due confezioni diverse non avrebbe un
  significato chiaro;
- per coprire le aggiunte dal catalogo alla chiusura conta la quantità che
  serviva, non quella presa: l'eccedenza entra in casa, ed è da lì che si
  sottrae;
- `📦` compare solo sulle voci con una quantità;
- le confezioni proposte sono al massimo 8.

**Revisione di procedimento e allegati delle ricette**: fatta, riportata ad
Alessio, **nessuna modifica** finché non dà il via libera.

9 nuovi test: 2 sulla presa nella lista, 2 sulle letture e sulla modifica
degli ingredienti delle ricette, 3 nelle scorte (mancanze, scarico con
annotazione, trigger dei pasti), 1 nei turni, 1 nel planner. Più due test
esistenti estesi con i nuovi callback. Totale 428.

**Distribuito sull'S9 il 17 settembre 2026** (commit `5448d6e`): CI verde
(run #145 sul commit giusto), 428 test e clippy verdi anche sul telefono,
backup del database creato (`gestionale_pre_20260917_193955.db`), migration
provata prima su una copia; `applied_migrations=54` e `Gestionale Casa
online` confermati nel log di avvio. **Collaudo dal vivo su Telegram da
fare.**

## 2duodecies. Consegna B: negozi, prezzi, preferiti, codice a barre (17 settembre 2026)

Concordata con Alessio subito dopo la consegna A, e scritta nello stesso
giorno su sua richiesta ("fai anche la consegna B"). Una migration nuova,
`migrations/20260917210000_consegna_b_negozi_e_prezzi.sql`, e un modulo
nuovo, `src/modules/mercato.rs`. Comportamento in `docs/moduli/mercato.md`.

**Il principio, deciso con lui**: tutto facoltativo. Chi non usa i prezzi
vede la lista della spesa di prima.

**Costruito**:

1. **Negozi**: le 25 catene diffuse in Italia arrivano con la migration (le
   sette nominate da Alessio più le altre comuni); **è l'utente a scegliere
   su quali fare il confronto**, come ha chiesto. Si può creare un negozio
   proprio ("Il fruttivendolo di via Roma").
2. **Prezzi** (opzione "c", scelta da lui): prezzo registrabile voce per
   voce durante la spesa **e** totale dello scontrino alla chiusura,
   indipendenti e saltabili. L'avviso riporta il prezzo al chilo o al litro.
3. **Dove conviene**: la lista valutata con gli ultimi prezzi visti in ogni
   negozio scelto, ordinata prima per quante voci copre e poi per totale.
4. **Prodotti preferiti**: `⭐` sulla confezione, uno per alimento, proposto
   per primo.
5. **Codice a barre**: si scrive l'EAN, il bot lo cerca su **Open Food
   Facts**, crea il prodotto sull'alimento della voce e segna la confezione
   come quantità presa.
6. **Open Prices**: l'ultimo prezzo segnato da altri, mostrato come
   suggerimento da confermare, mai registrato da solo (`prezzi_osservati.fonte`).
7. Il **totale della spesa chiusa** resta a database e diventerà una
   transazione del futuro modulo Soldi, come approvato.

**Scelte prese in autonomia**:

- il confronto premia chi copre più voci prima del totale più basso: un
  negozio con due prezzi su venti non deve "vincere";
- un prezzo senza negozio scelto resta sulla voce ma non entra nelle stime,
  e il bot lo dice invece di tacere;
- un prodotto letto dal codice a barre con una quantità che non si capisce
  ("una confezione") non viene creato;
- il codice si valida prima di chiamare la rete; entrambe le fonti hanno 8
  secondi di tempo e un messaggio proprio se non rispondono.

**Le quattro correzioni alle ricette**, chieste da Alessio dopo la revisione
che gli avevo riportato:

1. gli esiti (`✅ Procedimento aggiornato.`, `✅ Foto aggiunta.`, `✅ Step
   eliminato…`, `✅ Allegato eliminato.`, spostamenti) stanno **dentro** la
   schermata, non più in un messaggio a parte (C3);
2. gli errori di lettura di step e allegati si vedono ("⚠️ Non riesco a
   leggere il procedimento."), invece di sembrare una ricetta vuota — è il
   modo in cui il difetto degli ingredienti è rimasto nascosto tre settimane;
3. la conferma prima di eliminare un allegato **c'era già** (C16): gliel'ho
   detto invece di rifarla;
4. **`📝 Riscrivi tutto`**: il procedimento si riscrive in un messaggio
   solo, un passaggio per riga o separati da righe vuote, con la numerazione
   scritta a mano tolta da sola. Cancella gli step di prima e i loro
   allegati, quindi chiede conferma (C16).

**Icona del pasto preparato**: era `🍳`, che però vuol dire "ricetta" in
mezzo bot (menù Ricette, dettaglio del pasto, porzioni). Alessio ha lasciato
scegliere: il preparato ora è **`🍲`**, e `🍳` resta la ricetta.

16 nuovi test: 5 di dominio in `mercato` (prezzi, prezzo al chilo, ordine
del confronto, quantità di Open Food Facts, codice a barre), 2 su database
(negozi scelti, prezzi per negozio e preferiti), 2 nelle ricette
(divisione del procedimento, riscrittura con permessi), più quelli della
consegna A. Totale 437. Le fonti esterne non sono coperte da test: nessun
test tocca la rete.

**Distribuito sull'S9 il 17 settembre 2026** (commit `04b4e14`): CI verde
(run #147 sul commit giusto), 437 test e clippy verdi anche sul telefono,
backup del database creato (`gestionale_pre_20260917_204809.db`), migration
provata prima su una copia; `applied_migrations=55` e `Gestionale Casa
online` confermati nel log di avvio. **Collaudo dal vivo su Telegram da
fare** (consegne A e B insieme).

## 2terdecies. Dal collaudo di A e B: correzioni, legenda, foto del codice a barre (18 settembre 2026)

Collaudo dal vivo di Alessio delle consegne A e B, con via libera esplicito
a correggere tutto insieme. Tre migration nuove:
`20260918100000_legenda_simboli.sql`,
`20260918120000_prodotti_marche_note.sql`,
`20260918130000_ricette_fonte.sql`.

**Difetti trovati dal collaudo**:

- **`💶 Prezzo` non funzionava mai**: la query che rilegge la voce non
  leggeva `prezzo_centesimi`, che la struct pretendeva — stessa famiglia del
  difetto degli ingredienti delle ricette. Nessun test copriva quella
  strada: ora ce n'è uno che passa da `registra_prezzo_voce`, cioè da dove
  passa il bot.
- **Due frasi con quattordici spazi in mezzo**: avevo spezzato il testo con
  la continuazione di riga `\`, e `cargo fmt` l'ha riunita male. Corretto, e
  imparata la regola: una frase mostrata all'utente non si spezza mai così.
- **I campi di un oggetto non avevano nessuna tastiera**: `⏭ Salta` e
  `🗑 Rimuovi` erano solo i comandi scritti `/salta` e `/rimuovi`, che il
  testo nominava come se fossero pulsanti, e non c'era modo di tornare
  indietro (C3). Ora la tastiera c'è, i comandi restano, e il testo non
  ripete i pulsanti (C1).
- **Un negozio creato a mano non si poteva né rinominare né togliere**: ora
  sì, e togliendolo i prezzi già visti restano nello storico.
- **L'allegato si eliminava senza vederlo**: ora la foto (o il video)
  arriva prima della conferma.

**Richieste nuove**:

1. **Legenda dei simboli** su planner (giorno e settimana), lista della
   spesa e scorte, con `❓ Mostra/Nascondi legenda` e una preferenza per
   persona, accesa all'inizio.
2. **Foto del codice a barre**: si fotografa invece di copiare tredici
   cifre. La lettura è tutta nel bot (`rxing`, il porto Rust di ZXing, su
   una immagine in scala di grigi): **la foto non esce dal telefono**.
   Tredici crate in più, un minuto e 48 di compilazione sull'S9 — provato
   in una cartella usa e getta prima di aggiungerlo al progetto.
3. **Dove va a finire un alimento dopo la spesa**, visibile e modificabile
   dalla sezione Alimenti (`📦 Dove va dopo la spesa`): la stessa scelta di
   `📌 Mettilo sempre qui`, raggiungibile prima di avere la roba in casa.
4. **Prodotti delle marche più diffuse in Italia**: 90 prodotti attaccati
   agli alimenti del catalogo. Marca, nome e formato vengono dalla mia
   memoria: **nessun codice a barre e nessun prezzo inventati**. Il codice
   vero si aggiunge fotografando la confezione.
5. **Fonte di una ricetta**: `🔗 Fonte della ricetta` nel menù di modifica.
   Un link diventa un pulsante che apre il sito e il nome del sito compare
   nel dettaglio ("🔗 Ricetta di giallozafferano.it"); un testo qualunque
   vale come nome ("Nonna"). **Il procedimento resta quello scritto da chi
   usa il bot**: copiare i passaggi di GialloZafferano dentro il bot è
   copiare materiale di altri, e non l'ho fatto — è la prima delle domande
   aperte qui sotto.

**Icona del pasto preparato**: era `🍳`, che però vuol dire "ricetta" in
mezzo bot. Ora il preparato è `🍲` e `🍳` resta la ricetta (C4).

**Domande aperte, in attesa di risposta** (`docs/roadmap.md`):

- **GialloZafferano**: importare i loro procedimenti sarebbe copiare testi
  protetti. Si può fare la ricetta con nome, link e ingredienti scritti da
  noi, oppure lasciare solo il link. Serve una decisione.
- **Prezzi automatici per molti prodotti**: Open Prices copre poco l'Italia
  e serve il codice a barre, che i prodotti seminati non hanno. Leggere i
  listini dei supermercati significa raschiare i loro siti, contro le loro
  condizioni e fragile. Serve una decisione.
- **Codici a barre dei prodotti seminati**: si riempiono man mano
  fotografando le confezioni, oppure si lasciano vuoti.

8 nuovi test: prezzo di una voce con lo storico del negozio, legenda,
negozio rinominato e tolto, lettura del codice da immagine, navigazione dei
campi oggetto, divisione del procedimento, fonte di una ricetta, prodotti
seminati. Totale 445.

**Distribuito sull'S9 il 18 settembre 2026** (commit `8286d72`): CI verde
(run #149 sul codice, più il commit del linker), 445 test e clippy verdi
anche sul telefono, backup del database creato
(`gestionale_pre_20260918_021633.db`), le tre migration provate prima su una
copia; `applied_migrations=58` e `Gestionale Casa online` confermati nel log
di avvio.

**Il telefono si è fermato una volta durante questo deploy**, ed è un fatto
da ricordare: `LLVM ERROR: out of memory` collegando
`rxing-one-d-proc-derive`. Un `cargo:rustc-link-arg` emesso da `build.rs`
vale solo per il crate che lo dichiara, quindi i **proc-macro** non
ricevevano `-Wl,--threads=1`. Risolto con `.cargo/config.toml` (rustflags per
il target Android) e `[profile.dev.build-override]`, che toglie le
informazioni di debug anche a proc-macro e build script.

**Collaudo dal vivo su Telegram da fare.**

## 2quaterdecies. Prodotti in elenco, prezzi con data e storico, catalogo esteso (18 settembre 2026)

Dalle risposte di Alessio alle domande aperte della sezione 2terdecies, più
la richiesta di vedere i soli prodotti commerciali. Una migration nuova,
`20260918140000_catalogo_esteso.sql`.

**Costruito**:

1. **`🛒 Prodotti commerciali`** nel menù Alimenti: l'elenco dei soli
   prodotti, paginato (C6) e con ricerca per marca, nome o alimento. Prima
   si potevano vedere solo entrando in un alimento alla volta.
2. **Prezzi con la data e lo storico** (risposta 2): ogni prezzo mostra
   quando e dove è stato visto (`1,29 € · Lidl · Gio 10 Set · 2,58 € al kg`)
   e, dopo 30 giorni, `⚠️ vecchio, conviene ricontrollarlo`. Nel prodotto
   c'è `💶 Prezzi visti` con gli ultimi dieci; nella schermata `📦 Ho
   preso…` compare l'ultimo prezzo, che è dove serve davvero. Lo storico non
   si cancella: `prezzi_osservati` tiene tutto.
3. **Catalogo esteso** (risposta 4): 67 voci nuove e 157 prodotti in più
   (247 in tutto), comprese due categorie nuove — **🧼 Casa e pulizia** e
   **🧴 Igiene personale** — con detersivi, carta igienica, dentifricio,
   lattine, succhi, surgelati e snack. Le voci non alimentari stanno nello
   stesso catalogo di proposito: lista della spesa e scorte le trattano
   esattamente come il cibo, e prima un detersivo si poteva solo riscrivere
   a mano ogni volta.

**Le risposte di Alessio, e cosa ne è seguito**:

- **(1) Ricette da GialloZafferano: opzione (a)** — nome, link e ingredienti
  scritti da noi, senza il loro procedimento. Il meccanismo della fonte c'è
  già; il seme delle ricette classiche è **ancora da fare**.
- **(2) Prezzi**: fatto quello che si può senza raschiare i siti dei
  supermercati — inserimento a mano, data, storico, avviso quando il prezzo
  invecchia, e Open Prices come suggerimento sui codici a barre letti.
- **(3) Codici a barre dei prodotti seminati**: **non li ho**, e inventarli
  sarebbe un dato falso. Si riempiono fotografando la confezione.
- **(4) Più prodotti**: fatto, e continuerà a crescere.
- **(5) Merge su `main`**: si aspetta la fine del collaudo.

**Una lezione dai test**: quattro test fissavano il numero esatto di
alimenti (421) e di categorie (12). Ora guardano che il catalogo base ci sia
tutto (`>=`), perché il catalogo è fatto per crescere. E le voci nuove
ricevono tutte le etichette di compatibilità alimentare a `verificare`,
perché il resto del bot conta su quell'invariante.

4 nuovi test: elenco e ricerca dei prodotti, riga del prezzo con data e
avviso, prodotti non alimentari nel catalogo, categorie non doppie. Totale
447.

**25 ricette classiche nel catalogo globale** (risposta 1, opzione a):
spaghetti al pomodoro, pasta al pesto, risotto ai funghi, frittata, pizza in
teglia, pollo al forno… Ingredienti **e** passaggi sono scritti da me, con
parole mie, su piatti di tradizione: non sono copiati da GialloZafferano né
da altri siti, che è esattamente quello che l'opzione (a) chiedeva di
evitare. `fonte_url` resta vuoto — non ho modo di verificare l'indirizzo di
una pagina di quel sito, e un link morto è peggio di nessun link: il link lo
aggiunge Alessio con `🔗 Fonte della ricetta`, che mostra il pulsante.

**Distribuito sull'S9 il 18 settembre 2026** (commit `df0666e`): CI verde
(run #153), 448 test e clippy verdi anche sul telefono, backup del database
creato, le due migration provate prima su una copia; `applied_migrations=60`
e `Gestionale Casa online` confermati nel log di avvio.

**Collaudo dal vivo su Telegram da fare.**

## 2quindecies. Riga di navigazione ovunque, "rimuovi tutte" nella lista (18 settembre 2026)

Due segnalazioni di Alessio da uno screenshot di `🗑️ Rimuovi voci`.

- **La riga `⬅️ Indietro | 💡 Migliora | 🏠 Menù principale` non era
  rispettata**: la schermata aveva solo `⬅️ Indietro`, e il bot, che mette
  `💡 Migliora` accanto al Menù principale, non trovandolo lo metteva in una
  riga a parte — e il Menù mancava del tutto. **Il difetto non era di una
  schermata sola** ma di ognuna costruita così. Corretto nel punto che
  inserisce `💡 Migliora` (`context_bot::inserisci_migliora`): se una
  schermata ha un `⬅️` da solo nella sua riga e nessun Menù principale, la
  riga viene completata con `💡 Migliora` e `🏠 Menù principale`. Vale per
  tutto il bot, con un test sui tre casi.
- **`🗑️ Rimuovi tutte (N)`** nella schermata `🗑️ Rimuovi voci`: toglie in
  un colpo le voci scritte a mano **e** gli alimenti e prodotti aggiunti a
  mano dal catalogo (precisazione di Alessio), con conferma C16 che dice
  quante voci spariscono. Le righe dei pasti pianificati restano, e la
  lista si ricalcola subito. Compare solo con almeno due voci (C8).

2 nuovi test. Totale 450.

**Distribuito sull'S9 il 18 settembre 2026** (commit `3ba439a`): CI verde,
450 test e clippy verdi anche sul telefono, nessuna migration nuova
(`applied_migrations=60` invariato), `Gestionale Casa online` nel log di
avvio.

**Collaudo dal vivo su Telegram da fare.**

## 2sexdecies. Voci del catalogo senza quantità (18 settembre 2026)

Chiesto da Alessio: "dai la possibilità di mettere senza quantità anche
quando si cerca un prodotto vero; senza quantità non venga contata nelle
scorte". D'accordo, con una rifinitura.

- Scegliendo un alimento o un prodotto dal catalogo c'è `➖ Senza quantità`,
  come già per le voci libere. La voce resta **legata al catalogo** ma è una
  riga a sé (`origine = 'manuale'`, con `alimento_id` e
  `prodotto_alimentare_id`): fuori dal calcolo del fabbisogno, che senza un
  numero non saprebbe cosa sommare.
- **Alla chiusura non entra nelle scorte** — era già la regola per ogni voce
  senza quantità (`dispensa::ingresso_da_chiusura` le salta).
- **La rifinitura**: su queste voci resta `📦`, così se al supermercato la
  confezione la prendi davvero puoi dire quanto (o fotografarne il codice a
  barre), e allora entra in casa quella quantità.
- **Il limite, detto**: se il planner chiede già 200 g di pasta e aggiungi
  "Pasta" senza quantità, in lista ci sono due righe. Non si possono
  sommare, perché una delle due non ha un numero.

1 nuovo test (senza presa fuori dalle scorte, con presa dentro). Totale 451.

**Distribuito sull'S9 il 18 settembre 2026** insieme alla sezione
2septdecies (commit `0859c28`).

## 2septdecies. Dal collaudo delle ricette (18 settembre 2026)

Alessio ha collaudato la parte ricette e ha dato il via a correggere tutto
insieme. Nessuna migration.

1. **`⬅️ Indietro` dal dettaglio di una ricetta tornava sempre alla prima
   pagina dell'elenco**, anche aprendo la ricetta da pagina 2, da una
   ricerca o dalle ricette "con quello che ho". Ora il bot ricorda per chat
   da dove si è arrivati (`ProvenienzaRicetta`: pagina dell'elenco, ricerca
   per nome, ricerca per ingredienti, scorte) e ci torna; le ricerche si
   ricostruiscono con `recipe:back`, perché il testo cercato non sta in un
   callback.
2. **Nessuno poteva modificare le ricette del catalogo globale, nemmeno
   l'amministratore**: non hanno proprietario, e il permesso guardava solo
   quello. Ora l'amministratore di sistema le modifica (nome, porzioni,
   ingredienti, procedimento, allegati, fonte) e le **archivia**; non le
   elimina, perché possono stare nei pasti già pianificati di altri. Gli
   utenti normali le leggono soltanto, come prima.
3. **Vedere un allegato di uno step**: prima arrivavano due messaggi — la
   foto e "📎 Allegato step" solo per i pulsanti — con `💡 Migliora` due
   volte, e `⬅️ Indietro` portava alla procedura guidata anche aprendo la
   foto dalla modifica. Ora è un messaggio solo (la foto con la riga di
   navigazione sotto) e Indietro torna allo step da cui la si è aperta. Lo
   stesso per la conferma di eliminazione: la domanda sta sotto la foto.
4. **Tastiere dei campi in modifica**: usavano quella della creazione, con
   tre pulsanti in riga (`💡 Migliora` finiva sotto) e un `❌ Annulla` che
   rispondeva "Creazione ricetta annullata" anche mentre si modificava il
   procedimento. Ora in modifica c'è solo `❌ Annulla` (che torna dove si
   era) e la riga è `❌ Annulla | 💡 Migliora | 🏠 Menù principale`; nella
   creazione `❌ Annulla la ricetta` sta sopra e la riga di navigazione resta
   intera.
5. **"📝 Riscrivi tutto" metteva tutto in un passaggio solo**, anche
   scrivendo un passaggio per riga: la pulizia del testo (`clean_text`)
   riduceva gli a-capo a spazi prima della divisione. I test non se ne
   accorgevano perché passavano il testo direttamente alla divisione. Ora
   c'è `clean_text_righe`, che tiene le righe, e un test che passa dalla
   stessa strada del bot. In più, un procedimento scritto **su una riga
   sola ma numerato** (`1. Scalda 2) Stendi 3) Inforna`) si divide lì — dal
   computer Invio manda il messaggio, quindi è il modo naturale di
   scriverlo. Solo numeri in fila da 1, seguiti da `.` o `)`: "200°" o
   "cuoci 2 minuti" non spezzano niente.

4 nuovi test (provenienza, amministratore sulle ricette globali, divisione
con pulizia e numerazione in riga, callback nuovi). Totale 455.

**Distribuito sull'S9 il 18 settembre 2026** (commit `0859c28`), insieme
alla sezione 2sexdecies che aspettava la fine del collaudo delle ricette:
CI verde, 455 test e clippy verdi anche sul telefono, nessuna migration
nuova (`applied_migrations=60` invariato), `Gestionale Casa online` nel log
di avvio.

**Collaudo dal vivo su Telegram da fare.**

## 2octodecies. Allegati nella procedura guidata, sostituire un pasto, archiviare da amministratore (19 settembre 2026)

Dal collaudo di Alessio (ricette e primi punti del planner), via libera a
fare tutto insieme. Nessuna migration.

1. **Procedura guidata con gli allegati nello step**: se uno step ha foto o
   video, la schermata *è* l'allegato, con il testo dello step come
   didascalia. I video partono da soli e **ricominciano in loop**: Telegram
   lo fa solo con le animazioni, che sono mute — per una ricetta conta
   vedere il gesto. Con più allegati `◀️ 📎 1/3 ▶️` li scorre (in tondo);
   gli step si cambiano con `⏪ Step 2/5 ⏩`, frecce diverse perché sono due
   cose diverse. Testo troppo lungo per una didascalia (oltre 900
   caratteri) o file sparito: la schermata resta di testo, come prima.
2. **`📦 Archivia` bloccato per l'amministratore** su una ricetta del
   catalogo: il menù lo mostrava, ma il controllo che scatta premendolo
   (`ensure_recipe_owner_ui`) accettava solo il proprietario. Ora
   l'archiviazione ha il suo controllo (`can_archive_recipe`); l'eliminazione
   resta al proprietario, con il messaggio giusto ("eliminare", non più
   "archiviare").
3. **Una foto con i pulsanti è una schermata**: prima il bot la trattava
   come allegato di passaggio, e la schermata di prima restava sopra.
   `ContextRequest::come_schermata` la fa sostituire alla precedente come
   ogni altra, e sotto la foto la navigazione è a sole icone `⬅️ | 💡 | 🏠`
   — i pulsanti sono larghi quanto la foto, e con le parole venivano
   tagliati.
4. **Righe dei pasti con un'icona sola**, quella dello stato: `🍲 ☕
   Colazione` metteva due icone attaccate e l'occhio non sapeva quale
   guardare. Il tipo di pasto resta scritto per esteso; `🍲` resta.
5. **Sostituire un pasto**: `🔁 Sostituisci` ora c'è **prima** di mangiare
   (da preparare e preparato) e porta dritti alla scelta della ricetta; se
   il pasto era preparato chiede prima se gli ingredienti ci sono ancora (sì
   → tornano in casa; no → restano tolti ma non sono più del pasto,
   `dispensa::dimentica_scarico_pasto`). Dopo aver mangiato il pulsante si
   chiama `✏️ Ho mangiato altro`: non si sostituisce, si corregge. Alessio
   proponeva di toglierlo sul consumato; gliel'ho sconsigliato perché
   correggere cosa si è mangiato è proprio il caso per cui era nato.
6. **La stellina del preferito** non si capiva: ora è `☆` se la confezione
   non è la preferita e `⭐` se lo è, con una riga che spiega a cosa serve.

4 nuovi test. Totale 458.

**Distribuito sull'S9 il 19 settembre 2026** (commit `1f7396f`): CI verde,
458 test e clippy verdi anche sul telefono, nessuna migration nuova
(`applied_migrations=60` invariato), `Gestionale Casa online` nel log di
avvio.

**Collaudo dal vivo su Telegram da fare.**

## 2novodecies. Dal collaudo E–26: sedici punti (24 settembre 2026)

Report di Alessio (`Collaudo_Gestionale_Casa_problemi_e_migliorie.docx`,
build S9 commit `6c59bf3`, prove da E a 26), via libera esplicito a
implementarli tutti. Una migration nuova, di soli dati (la 61ª).

### Bug

1. **Codice a barre scritto a mano che non si salvava**: il prodotto da
   creare esisteva già nel catalogo seminato, e l'indice unico su
   (alimento, marca, nome, quantità, unità) faceva fallire l'inserimento in
   silenzio. Ora `crea_prodotto_da_esterno` cerca prima un prodotto uguale:
   se c'è, gli attacca il codice a barre (quando non ne ha uno) e lo riusa.
   Un prodotto in meno da creare, e il codice finisce dove serve.
2. **"Dove conviene" diceva "non ho ancora prezzi"** appena si spuntava
   qualcosa: `stime_dei_negozi` guardava solo le voci non comprate, e a
   spesa quasi finita non restava niente da stimare. Ora guarda tutte le
   voci della lista — il confronto fra negozi riguarda la spesa intera, non
   quel che manca.
3. **"Formati disponibili: 0"** sui prodotti del catalogo: le due migration
   di seeding scrivevano `prodotti_alimentari` ma non
   `formati_prodotto_alimentare`, mentre il bot, quando crea un prodotto,
   scrive entrambe. Migration `20260924090000_formati_base_mancanti.sql`
   per i dati già presenti, e `assicura_formato_base` chiamata sia alla
   creazione sia al riuso perché non succeda più.
4. **Campi prezzo e venditore con il solo `💡 Migliora`**: i tre prompt
   incatenati degli oggetti (marca → modello, data → prezzo, prezzo →
   venditore) e i messaggi d'errore mandavano la schermata senza la
   tastiera del campo. Ora ce l'hanno tutti, e gli errori non nominano più
   i pulsanti (C1).
5. **Prezzo registrato con la data di ieri**: `rilevato_il` usava l'ora
   UTC, che dopo mezzanotte in Italia è ancora il giorno prima. Ora
   `localtime`.
6. **`0.99 €` invece di `0,99 €`**: `formatta_quantita` scriveva i decimali
   col punto.
7. **Voce fantasma che pesava sul fabbisogno**: "Rimuovi voci" mostrava
   aggiunte già comprate, perché si chiudevano solo se il comprato copriva
   l'intera quantità chiesta. Ma la lista mostra il **netto** delle scorte,
   quindi il comprato è quasi sempre inferiore: nessuna aggiunta si
   chiudeva mai. Ora basta aver comprato qualcosa di quell'identità.

### Migliorie

8. **Pulsanti tagliati** nella lista: le voci ora stanno su due righe —
   nome (tagliato a 40 caratteri) sopra, `quantità · 📦 confezione · 💶
   prezzo` sotto. Stessa cosa per le confezioni: nome sopra, formato sotto.
9. **"Voce aggiunta" con la lista che restava vuota**: ora il messaggio
   dice perché (in casa ce n'è già abbastanza, oppure ne restano X da
   comprare), invece di far dubitare che il salvataggio sia riuscito.
10. **`✏️ Ho mangiato altro` rimetteva le scorte da solo**: ora chiede se
    gli ingredienti preparati ci sono ancora, come già faceva
    `🔁 Sostituisci`.
11. **Sostituire richiedeva di nuovo profili e orario**: una sostituzione
    cambia la ricetta, non il resto. Nuovo flag `sostituzione` nel draft
    del planner: scelta la ricetta, si salva.
12. **Lo stesso alimento in tre posti**: se in casa ce n'è già,
    `destinazione_per` manda la roba nuova dove sta quella di prima (la
    scelta a mano resta più forte), e la scheda di una scorta dice
    `🏠 Ne hai anche in …`.
13. **Spiegazione della stellina con una confezione sola**: `☆`/`⭐` e la
    riga che li spiega compaiono solo quando le confezioni sono più di una.

### Testi

14. "1 voce comprata **vanno** nell'archivio" → singolare e plurale giusti.
15. "Pasto segnato come consumato **e congelato**" → il congelamento non
    c'entrava.
16. `➕ Nuovo oggetto` ripetuto nel testo e sul pulsante (C1).

### Le risposte di Alessio, lo stesso giorno

Quattro domande lasciate aperte, tre hanno prodotto altro lavoro.

1. **La regola del punto 7 era troppo grossolana**, e Alessio ha scelto la
   domanda. Ora un'aggiunta si chiude da sola solo se si è comprato **almeno
   quanto chiedeva**; se se n'è comprato meno, resta con quel che manca e
   dopo la chiusura il bot chiede "Le lascio in lista?" (`✅ Sì, lasciale` /
   `🗑 No, toglile`). La colonna `ridotta_chiusura_id` (migration 63) dice
   quali aggiunte ha ridotto quella chiusura, così "no" toglie esattamente
   quelle.
2. **`🏠 Ne hai anche in …` resta com'è**: compare sempre.
3. **Controllo del database reale dell'S9** (in sola lettura, via SSH):
   nessun prodotto doppio e nessun alimento doppio — la correzione del punto
   1 vale da qui in avanti e non c'è niente da fondere. Sono venute fuori
   due cose invece: **243 prodotti su 244 senza formato base** (li sistema la
   migration 61) e **tre prezzi con la data indietro di un giorno**, scritti
   in UTC prima della correzione del punto 5 — li riporta all'ora locale la
   migration 62.
4. **`🗄 Ricette archiviate` con `♻️ Ripristina`**: fatto. Il pulsante compare
   nel menù delle ricette solo quando ce n'è almeno una, con il numero
   accanto; si vedono solo le proprie (l'amministratore vede anche quelle del
   catalogo globale), e ripristinare chiede lo stesso permesso che serviva
   per archiviare. Prima l'archiviazione era una porta a senso unico.

5 nuovi test in tutto. Totale 463.

**Distribuito sull'S9 il 24 settembre 2026** (commit `d806846`): CI verde
(run #162), 463 test e clippy verdi anche sul telefono, le tre migration
provate prima su una copia del database e poi applicate
(`applied_migrations=63` nel log di avvio, era 60), `Gestionale Casa online`.
Le due migration di dati hanno fatto quello che dovevano, verificato
leggendo il database reale: **244 formati** di prodotto, nessun prodotto
attivo senza il suo formato base (erano 243 su 244 a mancare), e i tre
prezzi ora portano la data del **23 settembre**, il giorno in cui Alessio li
ha davvero segnati.

**Collaudo dal vivo su Telegram da fare.**

## 3. Stato tecnico verificato

- **63 migration** nel repository, tutte **applicate** al database reale
  dell'S9: le ultime tre (formati base mancanti, prezzi all'ora locale,
  `ridotta_chiusura_id` — sezione 2novodecies) il 24 settembre 2026,
  verificato leggendo `applied_migrations=63` nel log di avvio del bot dopo
  il deploy — non dedotto.
  Né il secondo giro
  di correzioni del 12 settembre 2026 (sezione 2septies) né le tre
  rifiniture del 13 settembre 2026 (sezione 2octies) avevano migration
  proprie: lavoravano su schema già esistente, confermato di nuovo
  `applied_migrations=50` (invariato) nel log di avvio dopo ciascun deploy;
- pipeline verde sia in locale sul PC sia sull'S9 (toolchain diversa,
  punto 1 della sezione 6): `fmt`, `check --locked`,
  `clippy --all-targets --locked -- -D warnings`, `test --locked` —
  **463 test** (458 prima della sezione 2novodecies, 455 prima della
  sezione 2octodecies, 451 prima della
  sezione 2septdecies, 450 prima della
  sezione 2sexdecies, 448 prima del giro
  della sezione 2quindecies, 445 prima
  del giro della sezione 2quaterdecies, 437 prima
  del giro della sezione 2terdecies, 428 dopo la
  consegna A e prima della consegna B della sezione 2duodecies, 419 prima
  della consegna A,
  406 prima del giro della sezione 2decies, 391 prima della
  chiusura della spesa e delle scorte del 16 settembre 2026 — sezione
  2nonies —, 389 prima delle tre rifiniture
  della sezione 2octies, 385
  prima del secondo giro di
  correzioni ai turni del 12 settembre 2026, 372 prima delle tredici
  correzioni ai turni e della correzione C16 sulla nutrizione, 355 prima
  di Turni e routine, 348
  prima del quarto giro di correzioni alla lista della
  spesa — ordine stabile sui refresh, unità predefinita, rimozione voci —,
  341 prima delle tre correzioni sulla fusione al check/
  eccesso/riordino, 338 prima delle prime tre correzioni del 9 settembre
  sulla lista della spesa — refresh automatico alla deselezione, "Aggiorna
  lista" condizionale, nessun limite di pagina —, 328 prima dei due
  feedback sulla lista della spesa, 303 prima della lista della spesa
  dell'8 settembre, 300 prima dell'audit annulla, 297 prima dell'allegato
  video, 289 prima del badge "🆕", 280 prima del sotto-step 5c, 279 prima
  del sotto-step 5a, 270 prima del sotto-step 3/5 della distribuzione, 248
  prima del 2 settembre: e' il numero da confrontare dopo ogni
  aggiornamento dell'S9);
- CI su GitHub Actions **verde** dalla run #42, la prima dello Step 7.

Regola invariata: una migration applicata al database reale e' immutabile. Ogni
correzione richiede una nuova migration append-only.

## 4. Le due macchine

Sono due cloni distinti dello stesso repository, con due shell diverse. E'
l'unico punto in cui e' facile sbagliare, e il 1 settembre e' successo: i
comandi di commit sono stati dati sull'S9, dove l'albero era pulito, quindi
`git add` non ha trovato nulla, `git commit` non ha creato nulla e il push ha
pubblicato un ramo vuoto. Il collaudo e' poi girato sul commit sbagliato e
nessuno se ne sarebbe accorto senza guardare il numero dei test.

| | PC | S9 |
|---|---|---|
| shell | PowerShell | bash di Termux |
| percorso | `~\Desktop\Gestionale_Casa_X_AI` | `~/gestionale-casa` |
| cosa ci si fa | l'assistente scrive i file; si committa e si pusha | si aggiorna, si collauda, gira il bot |

**Il PC e' l'unico posto dove esistono le modifiche prima del push.** L'S9 non
le vede finche' non sono su GitHub.

### Sul PC — commit e push

```powershell
cd ~\Desktop\Gestionale_Casa_X_AI
git status --short
```

Attenzione alla sintassi: in PowerShell il `\` a fine riga **non** e' una
continuazione (li' e' il backtick `` ` ``), quindi i comandi lunghi vanno su una
riga sola.

```powershell
git checkout -b <nome-ramo>
git add <elenco dei file su una riga sola>
git commit -m "<messaggio>"
git log --stat -1
git push -u origin <nome-ramo>
```

`git log --stat -1` deve elencare esattamente i file attesi: e' il controllo che
distingue un commit vero da un commit vuoto, e va fatto **prima** del push.

### Sull'S9 — configurazione della macchina, una volta sola

Le impostazioni che proteggono il collegamento stanno nel progetto
(`Cargo.toml` per i profili, `build.rs` per il flag del linker su Android),
quindi valgono anche per un `cargo run` dato a mano. Restano da mettere sull'S9 quelle che dipendono dalla macchina, in
`~/.cargo/config.toml` — **fuori dal repository**, perche' non riguardano il
progetto ma questo telefono:

```toml
[build]
jobs = 1
incremental = false
```

Non impostare `RUSTFLAGS` nell'ambiente: sostituisce la configurazione di cargo
invece di aggiungersi. Lo script avvisa se la trova.

### Sull'S9 — aggiornamento e collaudo

Il vecchio giro zip → scp → unzip → installer python non serve piu'.

```bash
cd ~/gestionale-casa
./scripts/aggiorna-s9.sh                          # aggiorna, verifica e avvia
./scripts/aggiorna-s9.sh --solo-controlli         # si ferma prima dell'avvio
./scripts/aggiorna-s9.sh --ramo <nome>            # passa a quel ramo e aggiorna
```

**`--ramo` non e' un accessorio.** Senza, lo script aggiorna soltanto il ramo su
cui l'S9 si trova gia': consegnando il lavoro su un ramo nuovo non arriva
niente, il collaudo gira verde sul codice di prima e nessun messaggio lo
segnala. E' successo il 1 settembre.

Lo script rifiuta di partire se sull'S9 ci sono modifiche non committate, imposta
le variabili che evitano l'esaurimento di memoria in fase di link, esegue
l'intera pipeline, fa il backup del database e prova su una copia le sole
migration non ancora applicate, lette da `_sqlx_migrations`. Non va aggiornato a
ogni step.

### Come si capisce in un secondo se e' arrivato il codice giusto

Lo script stampa il commit su cui sta girando e il numero dei test. **Il numero
dei test e' la verifica piu' rapida**: se il conteggio e' quello di prima, il
telefono sta collaudando il codice di prima. Il conteggio atteso e' scritto
nella sezione 3 di questo file e va aggiornato a ogni consegna.

## 5. File chiave

```text
docs/convenzioni-telegram.md        regole di ogni schermata Telegram
src/modules/calendario.rs           date e griglia del mese, per tutti
src/modules/liste.rs                paginazione e intestazioni, per tutte le liste
src/modules/planner_alimentare.rs   dominio + UI Telegram del planner
src/modules/porzioni.rs             calcolo base/percentuale/override
src/modules/porzioni_profili.rs     porzione per profilo
src/modules/porzioni_ingredienti.rs override del singolo ingrediente
src/modules/profili_alimentari.rs   profili e condivisione
src/modules/spazi_membri.rs         membri, inviti, ruoli
src/context_bot.rs                  schermata singola e `💡 Migliora`
src/main.rs                         routing, sessioni, input inattesi
scripts/aggiorna-s9.sh              aggiornamento e avvio sul telefono
scripts/verifica-ci.sh              stato reale della run CI via API, non un riassunto
scripts/pipeline-locale.sh          fmt/check/test/clippy in locale, commit/push solo se verde
scripts/collauda-remoto.sh          lancia aggiorna-s9.sh --solo-controlli sull'S9 via SSH
scripts/telegram-api.sh             pin/edit su Telegram via API diretta, per il countdown/checklist
scripts/prova-countdown.sh          collaudo isolato del countdown pinnato, nessun deploy vero
scripts/avvia-bot.sh                avvia il bot in background (nohup+disown+PID), per l'agente via SSH
scripts/ferma-bot.sh                spegnimento pulito via SIGINT, legge il PID da avvia-bot.sh
scripts/salva-binario.sh            copia il binario compilato per un rollback senza ricompilazione
scripts/rollback-binario.sh         ferma e riavvia il binario salvato, istantaneo, --riservato opzionale
scripts/countdown-manutenzione.sh   countdown reale, legge il default da impostazioni_distribuzione
scripts/deploy.sh                   l'orchestrazione: countdown → sessioni → binario → swap → salute
scripts/completa-deploy.sh          seguito: merge su main o rollback, in base all'esito del collaudo
src/modules/distribuzione.rs        default di manutenzione (tipo/minuti/orario), schermata admin
src/modules/collaudo.rs             riepilogo/checklist/conferma del collaudo guidato dopo lo swap
src/modules/novita.rs               registro delle novità e badge "🆕" propagato nei menù
```

## 6. Punti aperti

1. **Tre toolchain diverse, e solo una conta.** Il runner della CI usa la
   **1.98**; l'ambiente dell'assistente e' fermo alla **1.95** e non puo'
   aggiornarsi (`static.rust-lang.org` e' chiuso in uscita); l'S9 e' piu'
   vecchio di entrambi. La Clippy delle versioni piu' basse non emette lint che
   il runner invece applica: e' cosi' che `drain_collect` e' rimasto invisibile,
   ed e' successo di nuovo il 2 settembre con `useless_format`, che ha fatto
   fallire **quattro run di seguito** mentre la Clippy 1.95 diceva verde.

   **Un `clippy` locale che passa non e' un lasciapassare, e' solo l'assenza di
   una brutta notizia.** L'unico esito che conta e' quello della CI, e va
   **guardato sulla pagina della run**: il 2 settembre e' stato riassunto da uno
   strumento che ha letto verde dove era rosso, e su quella base era gia' stato
   consigliato il merge. Prima di mergiare si apre Actions e si legge l'icona.

   Da aggiornare con `rustup update` su Termux. Attenzione: **un
   `rust-toolchain.toml` non e' la soluzione ovvia** — su Termux `cargo` arriva
   da `pkg` e non da rustup, quindi il file verrebbe ignorato li' e potrebbe
   invece costringere altri ambienti a scaricare una versione che non hanno.
2. **Pasti liberi** non rappresentabili: `ricetta_nome_snapshot` e' NOT NULL.
   Decisione rimandata ora che esiste l'esito "saltato".
3. **`cargo clean` ogni tanto sull'S9.** `target/` cresce fino a qualche GB e
   lo spazio e' il vincolo vero del telefono. Lo script ora avvisa sotto
   1,5 GB liberi.
4. **Un planner cercato sette volte.** `planner_load_meals` chiama
   `planner_find_for_date` una volta per giorno: aprire una settimana costa
   sette letture identiche. Cercare il planner una volta sola e passarlo al
   ciclo toglie altre sei query. E' il seguito naturale del lavoro sulle date.
5. **PR #6 di Dependabot, sqlx 0.8.6 → 0.9.0: rinviata**, decisione presa il
   2 settembre 2026 dopo averla valutata. **Non e' un aggiornamento di
   sicurezza** — la falla RUSTSEC-2024-0363 era gia' chiusa in 0.8.1, e la
   0.9.0 e' una release con rotture di compatibilita'. Costa piu' di quanto
   renda, adesso, per tre motivi concreti:

   - **richiede Rust 1.94 come minimo**, e la toolchain dell'S9 e' piu' vecchia
     (punto 1 di questa lista). L'aggiornamento della libreria obbligherebbe a
     fare prima quello del telefono, cioe' due cambiamenti insieme sulla
     macchina piu' fragile;
   - **27 query costruite dinamicamente** vanno riscritte: in 0.9.0 le funzioni
     accettano solo `&'static str`, e una stringa composta a runtime va
     avvolta in `AssertSqlSafe`. Sono in `luoghi.rs`, `contenitori.rs` e
     `ricette.rs`;
   - **il trait `Migrate` cambia in modo significativo.** E' la parte piu'
     delicata del progetto: 42 migration immutabili gia' applicate a un
     database vivo, con un rituale di backup e prova su copia costruito
     attorno a `_sqlx_migrations`.

   Cosa la farebbe rientrare in agenda: una vulnerabilita' annunciata sulla
   0.8.x, oppure il bisogno di `sqlx.toml` o delle altre funzioni nuove. Fino
   ad allora la 0.8.6 fa quello che serve. Da rifare quando la toolchain
   dell'S9 sara' allineata, cosi' resta un cambiamento alla volta.
6. **Verifiche differite** che richiedono un secondo account Telegram: invito
   accettato con apertura dello spazio, notifica al creatore, notifica di cambio
   ruolo, notifica di rimozione con perdita dell'accesso.
7. **"Vista come utente normale" per l'amministratore principale — progettata
   il 7 settembre 2026, non ancora costruita.** Alessio vuole poter vedere il
   bot sia da admin sia da utente normale senza un secondo account Telegram.
   Verificato prima di proporre una soluzione: `identity::is_system_admin` e
   `identity::is_primary_admin` rileggono il ruolo dal database a ogni
   chiamata (`WHERE id = ? AND ruolo_sistema = 'admin'`), non si fidano di un
   valore in memoria — quindi non basta "mascherare" il campo su
   `AuditActor`, serve un controllo esplicito dentro quelle due funzioni.

   **Disegno proposto, ricalca un precedente diretto già nel codice**: esiste
   già un interruttore identico per un'altra preferenza, `view_all`
   (`identity::set_view_all`, bottoni "🌐 Tutti i tuoi spazi" / "🎯 Solo
   spazio predefinito" in `send_spaces`), salvato per utente in
   `preferenze_utente` e riletto a ogni richiesta quando si costruisce
   l'`AuditActor` (in `identity::lookup_telegram_actor` /
   `resolve_telegram_actor`). La stessa forma per questo caso:
   - una colonna in più in `preferenze_utente` (es. `vista_normale`,
     migration additiva);
   - un nuovo campo su `AuditActor` (es. `vista_forzata_non_admin: bool`),
     valorizzato leggendo quella colonna nello stesso punto in cui si legge
     `view_all` oggi;
   - `is_system_admin`/`is_primary_admin` controllano quel campo **prima**
     di interrogare il database, e rispondono `false` se attivo — senza mai
     toccare `ruolo_sistema`/`amministratore_principale` veri;
   - un bottone "👁️ Vista come utente normale" / "👁️ Torna a vista admin",
     riservato all'amministratore principale.

   **Cosa copre**: tutto ciò che oggi dipende da
   `is_system_admin`/`is_primary_admin` — il menù "🛠️ Amministrazione",
   "✅ Segna Fatto"/archiviazione dei miglioramenti, la schermata
   "🚀 Distribuzione", la modalità riservata, ecc.

   **Cosa non copre, resta nel punto 6 sopra**: i permessi *dentro* uno
   spazio condiviso (ruolo di membro: lettura/modifica/gestione — un asse
   di permessi diverso, indipendente da admin/non-admin) e i flussi che
   richiedono davvero una seconda persona (accettare un invito, ricevere
   una notifica). Per quelli serve comunque un secondo account Telegram.

8. **Il progetto si chiamerà "Villica"**, deciso l'8 settembre 2026 — non
   ancora applicato da nessuna parte (nessun file rinominato, il bot
   Telegram resta `Gestionale_personalizzato_Bot`). Da fare solo quando il
   progetto avrà raggiunto la forma "finita" immaginata, non ora. Dettagli,
   motivazione e nomi alternativi scartati in
   `docs/previsto/rinominazione-villica.md`.
9. **Prossimo macro-step deciso (8 settembre 2026): la lista della spesa**
   (`docs/previsto/lista-della-spesa.md`) — coerente con la sequenza già
   scritta in `docs/roadmap.md` e appoggiata sul planner pasti già
   operativo. **Scritta lo stesso giorno** (`src/modules/lista_spesa.rs`,
   `migrations/20260908150000_lista_spesa.sql`, dettagli in sezione 2quater
   qui sotto) — **primo collaudo dal vivo fatto da Alessio** (aggregazione
   dal planner, voci manuali libere, comprato congelato: confermati). Dal
   collaudo sono arrivati due feedback, implementati il 9 settembre come
   continuazione dello stesso lavoro (profilo alimentare automatico,
   aggiunta manuale con ricerca nel catalogo — migration
   `migrations/20260908160000_lista_spesa_aggiunte_catalogo.sql`, dettagli
   in sezione 2quater) — **questi due non sono ancora stati ricollaudati
   dal vivo**, nessun collaudo reale è stato possibile in questo worktree
   isolato.
10. **Riconfigurazione dell'S9 con un account dedicato — fatta l'8
    settembre 2026.** Chiarito con Alessio prima di toccare nulla: resta
    lo stesso account Telegram amministratore, Termux/il progetto/le
    chiavi SSH restano intatti; cambia solo l'account Google **di
    sistema** del telefono (fuori da Termux, non eseguibile via SSH —
    lo fa Alessio dalle Impostazioni Android) e il database riparte
    vuoto. Eseguito il reset: backup esplicito del database precedente
    salvato su `data/db/gestionale_PRE_RESET_20260908_021825.db`
    sull'S9 (non nella rotazione automatica dei 5 backup, va ripulito a
    mano quando non serve più), poi database svuotato e bot riavviato —
    45 migration riapplicate da zero, bootstrap del nuovo account admin
    confermato (stesso account Telegram di prima). D'ora in poi anche
    altri membri della famiglia potranno essere approvati come utenti
    normali.

    **Conseguenza notata da Alessio, non un difetto**: il badge "🆕"
    è ricomparso su "📋 Miglioramenti" dopo il reset — `novita_lette`
    tiene traccia di chi ha visto cosa *per utente*, e il reset ha
    cancellato quella tabella insieme al resto. Sparirà di nuovo
    arrivando davvero alla schermata dell'allegato, come la prima volta.

    **Trovato collaudando questa stessa conversazione, corretto lo
    stesso giorno**: il badge su "📋 Miglioramenti" arrivava fino al
    menù principale ma spariva subito dentro la sezione, senza
    continuare a indicare *quale* pulsante portasse alla novità.
    Deciso: da qui in avanti il badge deve guidare fino al pulsante
    specifico per **ogni** nuova funzionalità registrata in
    `novita::REGISTRO`, non solo fino al primo livello — vedi C14 in
    `docs/convenzioni-telegram.md`. Corretto per il caso esistente
    (`miglioramenti_allegato_video`): badge aggiunto anche su
    "➕ Nuovo miglioramento" e su "📷🎥 Aggiungi foto/video" nel dettaglio
    di un miglioramento esistente, non solo sul pulsante del menù
    principale.

    **Recuperati dal backup pre-reset i miglioramenti `da_fare`** che il
    reset aveva cancellato (il database ripartiva vuoto, come deciso, ma
    erano lavoro reale non ancora documentato altrove): controllati uno
    per uno contro tutta la documentazione esistente. Uno
    (`🧪 Zona test`/zero-downtime) era già ampiamente documentato **e
    già implementato** (è il punto 6 del ciclo di automazione,
    completo) — lasciato fuori di proposito. Gli altri cinque non
    erano documentati da nessuna parte e sono stati reinseriti come
    miglioramenti `da_fare` veri (id nuovi, contenuto e data di
    creazione originali preservati): gestione/reset degli account dalla
    sezione utenti; la panoramica amministrativa dovrebbe mostrare solo
    dati globali significativi (utenti attivi/di recente, non conteggi
    di case/oggetti); l'icona di un pasto segnato "saltato" nel planner
    non si aggiorna come fa quella di "completato"; possibilità di
    pianificare una ricetta nel planner senza associare un profilo,
    usando la ricetta base; la sessione Claude Code dedicata per la coda
    dei miglioramenti (già discussa sopra e in
    `docs/previsto/invio-miglioramenti-a-claude.md` — **attenzione**,
    quel documento la cita con l'id vecchio "#41", ora cambiato dal
    reset, cercarla per testo se serve).
11. **Multipiattaforma: il bot viene finito prima della web app, deciso
    l'8 settembre 2026.** Discusso con Alessio se e come estendere il
    progetto oltre Telegram (web app, poi Play Store/App Store/Windows/
    Mac). Decisione: nessuno sviluppo in parallelo — si completa prima il
    bot così come già pensato, poi la web app si appoggia alla stessa
    struttura dati/logica di dominio invece di duplicarla, per evitare il
    rischio di due implementazioni che divergono (già capitato una volta
    in questo progetto, vedi Step 7.3B in `docs/storico-del-progetto.md`).
    Dettagli e motivazione in `docs/previsto/multipiattaforma.md`. Nessun
    codice toccato da questa decisione.

## 7. Regole operative

- ogni lavoro passa da un branch pushato **prima** che una seconda sessione ci
  metta mano: uno stato che esiste solo su un dispositivo non e' uno stato
  condiviso, ed e' l'errore che il 31 agosto ha prodotto due 7.3B paralleli;
- **commit e push si danno sul PC, in PowerShell** (`~\Desktop\Gestionale_Casa_X_AI`);
  sull'S9 (`~/gestionale-casa`, bash di Termux) si aggiorna e si collauda e
  basta. Vedi la sezione 4: dare i comandi sulla macchina sbagliata non produce
  un errore, produce un ramo vuoto;
- prima di ogni push, `git log --stat -1` deve elencare i file attesi, e dopo
  ogni collaudo il numero dei test deve essere quello nuovo: sono i due
  controlli che intercettano un commit vuoto;
- nei blocchi shell dell'S9 non usare `set -e`: usare `|| return 1` o `|| exit 1`
  in modo che un errore fermi lo step senza chiudere la sessione SSH;
- niente commit o push se la pipeline fallisce;
- prima di una migration reale: backup, `integrity_check`, `foreign_key_check` e
  prova su copia — lo script lo fa gia';
- Telegram: massimo 5 elementi per pagina, nessun ID tecnico, accenti italiani
  corretti, riga di navigazione `⬅️ Indietro | 💡 Migliora | 🏠 Menù principale`
  dove applicabile, e ogni checklist di collaudo indica il percorso completo dal
  `🏠 Menù principale`;
- non dichiarare fatto cio' che non e' stato realmente verificato;
- i documenti si aggiornano nello stesso commit del codice: vedi la sezione 0.
