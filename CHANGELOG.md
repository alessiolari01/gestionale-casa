# Diario di sviluppo

## Step 5B — Foto oggetti e navigazione di avvio — 2026-08-15

### Stato precedente

Lo Step 5A è stato mergiato su `main`, la CI del merge è verde e la seconda
migration è stata applicata e verificata anche sul database reale del Galaxy S9.
Un backup consistente del database reale è stato creato prima dell'upgrade e il
secondo avvio ha confermato `Migrazioni applicate: 2`.

### Implementazione predisposta per test

- notifica automatica `🟢 Gestionale Casa è online` all'avvio del backend;
- la notifica di avvio contiene direttamente l'inline keyboard del menu principale;
- `/status` e il pulsante Stato sistema mostrano `🏠 Menu principale`;
- nuovo modulo `src/modules/foto.rs`;
- pulsante `📷 Foto` nella scheda degli oggetti;
- menu foto con aggiunta, visualizzazione e ritorno all'oggetto;
- comandi equivalenti `/foto <id>` e `/foto_aggiungi <id>`;
- ricezione delle immagini Telegram anche quando il messaggio non contiene testo;
- download della versione più grande della foto in `data/media/oggetti/<item_id>/`;
- registrazione del percorso, ruolo e descrizione nella tabella core `foto`;
- prima foto di un oggetto marcata `principale`, successive `galleria`;
- didascalia Telegram usata come descrizione della foto;
- visualizzazione delle foto dal file locale tramite Telegram;
- rimozione del file locale se la registrazione SQLite fallisce, per evitare
  file orfani;
- due test automatici dedicati a estensione file e ruoli principale/galleria;
- nessuna nuova migration: viene riusata la tabella `foto` dello schema core;
- `tokio` abilita la feature `fs` necessaria al salvataggio asincrono dei file.

### Da verificare prima della chiusura

1. `cargo fmt --all -- --check`;
2. `cargo check --locked`;
3. `cargo test --locked`;
4. `cargo clippy --all-targets --locked -- -D warnings`;
5. notifica automatica di avvio e menu principale sul Galaxy S9;
6. pulsante di ritorno al menu da `/status`;
7. aggiunta di almeno due foto allo stesso oggetto, verificando principale e
   galleria;
8. presenza reale dei file sotto `data/media/oggetti/<id>/`;
9. visualizzazione dopo riavvio;
10. backup contenente anche `data/media`;
11. CI GitHub Actions verde prima del merge.

### Prossimo passo previsto

Dopo la chiusura dello Step 5B: **Step 5C — modifica ed eliminazione sicura
degli oggetti già salvati**. Il requisito multi-casa/stanze resta registrato per
lo Step 6 e la relativa architettura deve essere confermata prima della migration.



### Step 5A — verifica UX e preparazione chiusura

- Verificato sul Galaxy S9 il comportamento dei campi già compilati: il pannello
  mostra `✅`, riaprendo il campo viene mostrato il valore corrente, `/salta` lo
  conserva e un nuovo valore lo sostituisce esplicitamente.
- Verificato manualmente il caso Marca/Modello e un campo singolo (Posizione),
  con salvataggio e successiva ricerca dell'oggetto.
- `cargo fmt --all -- --check`, `git diff --check` e Clippy con `-D warnings`
  risultano superati dopo la patch UX; la suite `check/test` va rieseguita come
  controllo finale immediatamente prima del commit di chiusura.
- Durante un `cargo run` il linker LLVM/Termux è terminato una volta con
  segmentation fault; il comando ripetuto ha avviato correttamente il backend.
  Non sono state necessarie modifiche al codice.
- Configurato e provato l'accesso **OpenSSH locale PC -> S9** e il trasferimento
  file via SCP. GitHub `main` resta la fonte ufficiale; SSH/SCP diventano il
  canale operativo per testare patch senza commit di trasporto.
- Registrati per il futuro due requisiti: modifica degli oggetti già salvati e
  gestione trasversale di più case/stanze con ricerca sia filtrata sia globale.
  La struttura dei luoghi dovrà essere proposta e confermata prima di creare la
  migration.

### Roadmap aggiornata proposta

1. completare CI della Pull Request e merge su `main`; a quel punto Step 5A è
   formalmente chiuso;
2. Step 5B — foto degli oggetti;
3. Step 5C — modifica/eliminazione degli oggetti già salvati;
4. Step 6 — luoghi e multi-abitazione, prima dei successivi grandi moduli, dopo
   conferma dell'architettura.

### Step 5A — affinamento UX bozza oggetti

- Le sezioni gia' compilate nel pannello dettagli sono marcate con `✅`.
- Riaprendo un campo gia' valorizzato il bot mostra il valore attuale prima della sostituzione.
- Durante la revisione di un campo, `/salta` conserva il valore esistente; su un campo vuoto continua a lasciarlo vuoto.
- Aggiunto un test automatico dedicato al prompt dei campi gia' compilati.

## Step 5A — Oggetti generici: prima implementazione — 2026-08-14

### Stato precedente

Lo Step 4 era chiuso e verificato: Telegram, SQLx, SQLite, migration automatiche
e `/status` erano operativi sul Galaxy S9. Il modulo `oggetti` era ancora uno
scheletro e non esisteva una tabella specifica.

### Decisioni concordate

- il modulo riguarda solo **oggetti generici**;
- il nome è l'unico campo obbligatorio;
- i dettagli opzionali vengono scelti da un **pannello dettagli**;
- il numero seriale resta disponibile ma non è in primo piano;
- l'interfaccia principale usa pulsanti inline, mantenendo `/comandi`
  equivalenti in parallelo;
- pulsanti e comandi convergono sulla stessa logica applicativa.

### Implementato

- nuova migration `20260814121600_oggetti.sql`, senza modificare quella core;
- tabella `oggetti` collegata 1:1 a `items` con `ON DELETE CASCADE`;
- prezzi e valore stimato salvati in centesimi interi con `CHECK >= 0`;
- condizione limitata a `ottimo`, `buono`, `usurato`, `da_riparare`;
- menu principale Telegram con inline keyboard;
- menu Oggetti con Nuovo / Elenco / Cerca;
- comandi `/oggetti`, `/oggetto_nuovo`, `/oggetti_lista`,
  `/oggetto_cerca`, `/oggetto`, `/annulla`, `/salta`;
- creazione rapida con solo nome oppure pannello dettagli;
- flussi guidati per marca/modello e dati di acquisto;
- selezione condizione tramite pulsanti;
- altri dettagli: descrizione, valore stimato e seriale;
- salvataggio atomico di `items` + `oggetti` in transazione SQL;
- elenco alfabetico paginato;
- ricerca su nome, marca, modello, seriale, posizione, venditore, descrizione e note;
- scheda singola richiamabile da pulsante o `/oggetto <id>`;
- sessione bozza in memoria per chat;
- callback Telegram sottoposte alla stessa whitelist delle chat autorizzate;
- documentazione `docs/moduli/oggetti.md`.

### Test predisposti

- parsing importi italiani/decimali;
- validazione e normalizzazione date;
- parser dei comandi con suffisso `@nome_bot`;
- salvataggio, lettura, elenco e ricerca su SQLite;
- verifica `ON DELETE CASCADE`;
- verifica del `CHECK` contro importi negativi.

La sintassi delle due migration è stata verificata anche applicandole in ordine
su SQLite in memoria, inserendo un oggetto reale di prova e confermando il
rifiuto di un prezzo negativo.

### Stato dello step

**Implementato, non ancora chiuso.**

Prima della chiusura servono:

1. `cargo fmt --all -- --check`;
2. `cargo check --locked`;
3. `cargo test --locked`;
4. `cargo clippy --all-targets --locked -- -D warnings`;
5. GitHub Actions verde;
6. test runtime sul Galaxy S9 di pulsanti, comandi, creazione, elenco, ricerca,
   scheda e persistenza dopo riavvio.

### Prossimo passo standard

Chiudere e verificare lo Step 5A. Solo dopo passare allo **Step 5B —
modifica ed eliminazione sicura**.

---

Questo file registra gli step del progetto in ordine cronologico. Ogni step
spiega da quale stato si partiva, cosa è stato modificato, cosa è stato
verificato e quale sarà il passo successivo.

## Step 4 — SQLite operativo e stato del sistema — 2026-08-13 → 2026-08-14

### Stato precedente

Lo Step 3.1 era chiuso con CI verde. Il bot Telegram e la whitelist erano gia'
verificati sul Galaxy S9 e lo schema core SQLite esisteva come migration, ma
`src/db.rs` era ancora uno scheletro: il backend non apriva alcun database e
non eseguiva migration all'avvio.

### Fatto in questo step

- scelta SQLx 0.8.6 con `default-features = false` e sole feature necessarie:
  Tokio, SQLite, migration e macro;
- usato il driver SQLite bundled per ridurre le dipendenze native dell'host;
- aggiunto `DATABASE_URL` alla configurazione con default
  `sqlite://data/db/gestionale.db`;
- implementato `src/db.rs` con creazione cartella/file, pool SQLite e foreign
  key esplicitamente abilitate;
- incorporate e applicate automaticamente le migration all'avvio;
- aggiunto `build.rs` per far ricompilare il progetto quando cambia la cartella
  `migrations/`;
- condiviso `SqlitePool` con il dispatcher Teloxide;
- aggiunto `/status` con verifica di database, foreign key, migration applicate
  e presenza dello schema core;
- aggiornato `/start` per mostrare anche `/status`;
- aggiornati `.env.example`, README, architettura, handoff e documentazione
  delle migration;
- reso `scripts/backup.sh` consistente tramite l'API `.backup` di SQLite;
- aggiornato `scripts/termux-boot.sh` a `cargo run --release --locked`.

### Decisione sulla versione SQLx

La serie SQLx 0.9 richiede un toolchain Rust molto recente. Per non introdurre
un requisito non ancora verificato sull'host Android, lo Step 4 usa la serie
0.8.6, che offre gia' tutte le funzionalita' necessarie. Gli aggiornamenti
futuri possono essere valutati tramite le PR di Dependabot e testati sul Galaxy
S9 prima del merge.

### Verifiche effettuate sul Galaxy S9

- toolchain verificato: `rustc 1.97.1` e `cargo 1.97.1`;
- aggiunto SQLx 0.8.6 e rigenerato/versionato `Cargo.lock` direttamente sul
  Galaxy S9;
- `cargo check` completato correttamente con SQLx/SQLite;
- `cargo tree -i openssl-sys -e features` conferma che `openssl-sys` non è
  presente nella dependency graph;
- `cargo test --locked` completato con 2 test superati e 0 falliti;
- `cargo run --locked` avvia correttamente il backend;
- creato realmente `data/db/gestionale.db`;
- `/start` e `/ping` continuano a funzionare;
- `/status` verifica correttamente database SQLite, foreign key, migration
  applicata e presenza delle cinque tabelle core (`items`, `foto`, `tag`,
  `item_tag`, `promemoria`);
- un secondo avvio sullo stesso database funziona senza errori e senza
  riapplicazione distruttiva della migration.

Durante `cargo check`/`cargo test` Rust segnala una future incompatibility in
`proc-macro-error2 v2.0.1`. Non è un errore attuale e non blocca lo Step 4; va
rivalutata durante futuri aggiornamenti delle dipendenze, senza forzare upgrade
non verificati sul Galaxy S9.

### Stato dello step

**Step 4 chiuso e verificato sul dispositivo di destinazione.**

La chiusura resta valida finché anche la CI GitHub Actions associata al commit
di chiusura rimane verde; un eventuale fallimento della CI riapre lo step e va
risolto prima di iniziare lo Step 5.

### Prossimo passo standard

Dopo la chiusura dello Step 4: **Step 5 — progettazione e prima
implementazione del modulo Oggetti generici**.

---

## Step 3.1 — Handoff, workflow Git e automazioni GitHub — 2026-08-13

### Stato precedente

Lo Step 3 era chiuso e verificato sul Galaxy S9, PC/GitHub/S9 erano stati
riallineati e `Cargo.lock` era già versionato. Il repository era utilizzabile,
ma mancavano un documento di handoff autonomo, controlli CI e una descrizione
formale del workflow PC ↔ GitHub ↔ S9. Inoltre README e changelog contenevano
ancora riferimenti a `Cargo.lock` come file “da aggiungere”, ormai obsoleti.

### Fatto in questo step

- creato `docs/HANDOFF.md` come guida autosufficiente per una terza persona o
  un'altra AI;
- definito **GitHub `main` come fonte ufficiale** del progetto;
- formalizzato il workflow corrente:
  - PC Windows = sviluppo principale e commit/push;
  - GitHub = fonte ufficiale e sincronizzazione;
  - Galaxy S9 = host reale e test runtime;
- formalizzata l'eccezione per modifiche semplici nate sull'S9, seguite da
  push e successivo `git pull --ff-only` sul PC;
- documentata la regola di non sviluppare contemporaneamente sugli stessi
  file da PC e S9;
- documentata come evoluzione futura, **non implementata**, l'amministrazione
  remota tramite Tailscale + OpenSSH in Termux senza esporre SSH a Internet;
- aggiunto `.github/workflows/ci.yml` per controllare automaticamente format,
  check, test e Clippy su push/pull request verso `main` usando Rust stable;
- aggiunto `.github/dependabot.yml` per controlli settimanali di Cargo e
  GitHub Actions, senza auto-merge;
- corretto il comando di clone usando l'URL reale del repository;
- corrette le note obsolete su `Cargo.lock`, che è già versionato;
- aggiornati README e architettura per riflettere workflow e roadmap.

### Verifiche effettuate durante la preparazione

- lo Step 3 di partenza corrisponde al commit `734b23d`;
- `Cargo.lock` è presente;
- nessun valore reale di `TELOXIDE_TOKEN`, PAT GitHub o altro segreto è stato
  aggiunto;
- i file GitHub Actions/Dependabot sono stati predisposti secondo la sintassi
  documentata per i rispettivi strumenti;
- la logica Rust del bot non è stata modificata.

La verifica automatica definitiva viene registrata nella sezione seguente, dopo
le correzioni emerse dalla prima run GitHub Actions.

### Problemi emersi nella prima run CI e correzione

La prima esecuzione GitHub Actions dello Step 3.1 ha svolto correttamente il
proprio compito di controllo e ha evidenziato due problemi:

- `cargo fmt --all -- --check` ha segnalato che `src/config.rs` e `src/main.rs`
  non erano ancora formattati secondo `rustfmt`; sul Galaxy S9 è stato quindi
  eseguito `cargo fmt`, senza modificare la logica del bot;
- il job separato “Minimum Rust 1.88” ha fallito. Per questo gestionale non è
  utile mantenere un MSRV formale derivato dalle dipendenze transitive: il
  controllo è stato rimosso insieme a `rust-version = "1.88"` dal manifest.

La CI definitiva usa Rust stable aggiornato e mantiene i quattro controlli che
portano valore al progetto: format, check, test e Clippy. Il Galaxy S9 resta
l'ambiente reale di verifica runtime.

Dopo le correzioni è stata eseguita una nuova GitHub Action con esito positivo:

- `cargo fmt --all -- --check` — superato;
- `cargo check --locked` — superato;
- `cargo test --locked` — superato;
- `cargo clippy --all-targets --locked -- -D warnings` — superato.

### Stato dello step

**Step 3.1 chiuso e verificato tramite GitHub Actions.**

La prima run fallita resta documentata perché dimostra il valore della CI e rende
riconoscibili in futuro le correzioni effettuate.

### Prossimo passo standard

**Step 4 — SQLite operativo e stato del sistema**, come già annunciato nello
Step 3.

---

## Step 3 — Base backend Telegram e whitelist — 2026-08-12 → 2026-08-13

### Stato precedente

Il repository conteneva lo scheletro Rust e lo schema dati core SQLite, ma
`main.rs`, `config.rs` e `auth.rs` erano ancora composti principalmente da
TODO. Il bot Telegram esisteva già e token/chat ID erano stati configurati
sul Galaxy S9 tramite Termux.

### Fatto in questo step

- aggiunte al `Cargo.toml` le dipendenze minime per il primo backend:
  `tokio`, `teloxide`, `dotenvy`, `anyhow`, `tracing` e
  `tracing-subscriber`;
- Teloxide configurato con `rustls` e senza TLS nativo, per ridurre le
  dipendenze di sistema e facilitare l'esecuzione su Termux;
- implementato `Config::load()` in `src/config.rs`;
- validati `TELOXIDE_TOKEN` e `ALLOWED_CHAT_IDS`;
- evitato `Debug` sulla struct `Config` per ridurre il rischio di stampare
  accidentalmente il token nei log;
- implementata la whitelist in `src/auth.rs`;
- aggiunti due unit test per autorizzazione positiva e negativa;
- implementato il primo `Dispatcher` Teloxide in `src/main.rs`;
- aggiunta verifica iniziale del token/API tramite `get_me()`;
- aggiunti i comandi `/start` e `/ping`;
- le chat non autorizzate vengono ignorate senza eseguire comandi;
- aggiornato `migrations/README.md` perché lo schema core esiste già;
- aggiornata la roadmap e introdotto questo diario di sviluppo.

### Verifiche effettuate sul Galaxy S9

- `cargo test` completato correttamente;
- entrambi gli unit test della whitelist superati;
- `cargo run` avvia correttamente il backend e raggiunge le API Telegram;
- `/ping` verificato con risposta `Pong! Gestionale Casa è online.`;
- `/start` verificato con il messaggio di avvio e l'elenco dei comandi;
- test end-to-end della whitelist eseguito da un secondo account Telegram non
  presente in `ALLOWED_CHAT_IDS`: il bot non risponde, come previsto;
- nessun token Telegram reale o altro segreto è presente nei file versionati.

### Problema incontrato e risoluzione

Al primo `cargo test` su Termux la compilazione si è fermata su
`openssl-sys`. `cargo tree` ha mostrato la catena
`teloxide default -> native-tls -> reqwest -> openssl-sys`.

La causa non era il codice dello Step 3: il Galaxy S9 era ancora un commit
indietro e il `Cargo.toml` locale apparteneva allo Step 2, con
`teloxide = "0.17.0"`. Questa forma abilita le feature predefinite di
Teloxide, tra cui `native-tls`.

Dopo aver ripristinato il `Cargo.toml` locale e riallineato il telefono con
`origin/main`, la dipendenza è diventata quella prevista:

```toml
teloxide = { version = "0.17", default-features = false, features = ["rustls", "ctrlc_handler"] }
```

La successiva compilazione e tutti i test sono andati a buon fine. Questa nota
resta nel changelog per rendere riconoscibile lo stesso problema in futuro.

### Stato finale dello step

**Step 3 chiuso e verificato sul dispositivo di destinazione.**

`Cargo.lock` è stato generato sul Galaxy S9 durante la compilazione verificata
e successivamente versionato nel repository, così le versioni effettivamente
testate delle dipendenze restano riproducibili.

### Prossimo passo standard

**Step 4 — SQLite operativo e stato del sistema.**

Obiettivi previsti:

1. aggiungere `sqlx` con supporto SQLite;
2. leggere e validare `DATABASE_URL`;
3. creare automaticamente `data/db/` se necessario;
4. aprire SQLite con foreign key abilitate;
5. eseguire automaticamente le migration presenti in `migrations/`;
6. condividere il pool/database con il dispatcher Telegram;
7. aggiungere `/status` per verificare bot, database e migration.

Lo Step 4 non introduce ancora il modulo oggetti: deve prima dimostrare che la
catena `Telegram -> Rust -> SQLite` funziona correttamente dall'inizio alla
fine.

---

## Step 2 — Schema dati core — 2026-08-12

### Stato precedente

Era presente solo lo scheletro iniziale del repository.

### Fatto

- progettata la tabella centrale `items`;
- aggiunte `foto`, `tag`, `item_tag` e `promemoria`;
- creata la prima migration SQL;
- documentato lo schema in `docs/schema-core.md`;
- normalizzati i fine riga a LF.

### Passo successivo previsto allora

Creare la prima base eseguibile del backend Telegram.

---

## Step 1 — Scheletro iniziale

### Fatto

- creata la struttura del progetto Rust;
- separati configurazione, autenticazione, database e moduli funzionali;
- predisposte cartelle per migration, documentazione, script e dati locali;
- documentate le principali decisioni architetturali.
