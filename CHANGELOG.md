# Diario di sviluppo

Questo file registra gli step del progetto in ordine cronologico. Ogni step
spiega da quale stato si partiva, cosa è stato modificato, cosa è stato
verificato e quale sarà il passo successivo.

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
  check, test e Clippy su push/pull request verso `main`, più un controllo
  separato con Rust 1.88;
- aggiunto `.github/dependabot.yml` per controlli settimanali di Cargo e
  GitHub Actions, senza auto-merge;
- corretto il comando di clone usando l'URL reale del repository;
- corrette le note obsolete su `Cargo.lock`, che è già versionato;
- aggiornato il requisito Rust dichiarato da 1.82 a 1.88: la dependency graph
  attualmente bloccata include versioni della crate `time` che richiedono
  almeno Rust 1.88;
- aggiornati README e architettura per riflettere workflow e roadmap.

### Verifiche effettuate durante la preparazione

- lo Step 3 di partenza corrisponde al commit `734b23d`;
- `Cargo.lock` è presente;
- nessun valore reale di `TELOXIDE_TOKEN`, PAT GitHub o altro segreto è stato
  aggiunto;
- i file GitHub Actions/Dependabot sono stati predisposti secondo la sintassi
  documentata per i rispettivi strumenti;
- la logica Rust del bot non è stata modificata.

I comandi Rust della CI non possono essere dichiarati superati finché il
workflow non è stato eseguito realmente da GitHub sul commit dello Step 3.1.

### Stato dello step

**Configurato, in attesa di verifica CI su GitHub.**

Lo Step 3.1 sarà chiuso soltanto quando il primo run di `.github/workflows/ci.yml`
sarà verde. Se `fmt` o Clippy segnalano problemi, vanno corrette solo le
violazioni necessarie, documentando le modifiche prima di procedere allo Step
4.

### Prossimo passo standard

Dopo la chiusura della CI:

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
