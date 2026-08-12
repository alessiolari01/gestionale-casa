# Diario di sviluppo

Questo file registra gli step del progetto in ordine cronologico. Ogni step
spiega da quale stato si partiva, cosa è stato modificato, cosa è stato
verificato e quale sarà il passo successivo.

## Step 3 — Base backend Telegram e whitelist — 2026-08-12

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
- aggiornata la roadmap e la documentazione dello stato del progetto.

### Verifiche

- struttura e file controllati sullo ZIP ricevuto;
- nessun token Telegram reale o altro segreto evidente trovato nei file
  versionati;
- la migration core era già stata verificata separatamente: creazione delle
  cinque tabelle, vincolo `CHECK` sul tipo e cancellazione `CASCADE` delle
  foto funzionanti;
- **compilazione ed esecuzione del nuovo backend da verificare sul Galaxy S9**:
  nell'ambiente usato per preparare questo ZIP non è disponibile Cargo.

### Prossimo passo standard

**Step 4 — Test reale su Galaxy S9 + collegamento SQLite.**

Prima si eseguono `cargo test` e `cargo run` sul telefono e si verifica la
risposta a `/ping`. `DATABASE_URL` resta predisposta in `.env.example`, ma non
è ancora obbligatoria: verrà letta quando collegheremo SQLite. Il primo comando
Cargo che risolve le dipendenze genererà
anche `Cargo.lock`: trattandosi di un'applicazione, verrà mantenuto nel
repository per fissare le versioni effettivamente verificate. Solo dopo il
test positivo si implementa `src/db.rs`, si eseguono automaticamente le
migration all'avvio e si aggiunge un comando di stato che confermi che
Telegram e database siano entrambi operativi.

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
