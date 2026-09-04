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

Restano 3 sotto-step: la schermata admin `🚀 Distribuzione`, la coda "in
attesa di input testuale", e lo swap vero con rollback.

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

## 3. Stato tecnico verificato

- **42 migration** nel repository, **tutte applicate** al database reale
  dell'S9. Confermato dall'avvio del 1 settembre sera:
  `applied_migrations=42`. Le versioni precedenti di questo file dicevano che
  `20260901013000_versione_contenuto_ricetta.sql` fosse ancora da applicare:
  non era vero, ed e' bastato leggere `_sqlx_migrations` per accorgersene;
- pipeline verde: `fmt`, `check --locked`, `clippy --all-targets --locked
  -- -D warnings`, `test --locked` — **270 test** (248 prima del 2 settembre:
  e' il numero da confrontare dopo ogni aggiornamento dell'S9);
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
