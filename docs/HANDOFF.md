# Handoff — Gestionale Casa

Questo documento serve a consegnare il progetto a una nuova persona o a
un'altra AI senza richiedere l'accesso alle conversazioni usate durante lo
sviluppo.

## 1. Fonte ufficiale

Repository:

`https://github.com/alessiolari01/gestionale-casa`

Branch ufficiale: `main`.

**GitHub `main` è la fonte di verità del progetto.** Gli ZIP sono snapshot o
backup: se uno ZIP e `main` differiscono, verificare prima quale sia più
recente e, salvo recuperi intenzionali, usare `main`.

## 2. Cosa leggere prima di modificare il progetto

Ordine consigliato:

1. `README.md` — stato e quick start;
2. `ARCHITETTURA.md` — decisioni di design e motivazioni;
3. `CHANGELOG.md` — cosa è stato fatto e realmente verificato;
4. questo `docs/HANDOFF.md` — workflow operativo;
5. `docs/schema-core.md` e i documenti del modulo su cui si deve lavorare.

Non cambiare una decisione architetturale importante senza motivarla e
aggiornare la documentazione pertinente.

## 3. Obiettivo del sistema

Gestionale personale accessibile tramite bot Telegram per catalogare e gestire
nel tempo:

- oggetti generici;
- vestiti e outfit;
- veicoli e manutenzioni;
- ricette e futura aggregazione della lista della spesa.

L'architettura punta a essere semplice da usare, portabile e adatta a un host
a basso consumo.

## 4. Architettura attuale

```text
Utente Telegram
      |
      v
API Telegram
      |
      | long polling HTTPS in uscita
      v
Backend Rust / Teloxide
      |
      +-- whitelist chat_id
      |
      +-- SQLite / SQLx (Step 4 verificato sul Galaxy S9)
      |
      +-- file locali in data/ (non versionati)
```

Tecnologie/scelte principali:

- Rust;
- Teloxide con `rustls`, non `native-tls`;
- Tokio;
- SQLite;
- configurazione tramite variabili d'ambiente / `.env` locale;
- nessuna porta pubblica necessaria per il bot Telegram.

## 5. Hardware e ruoli operativi

### Galaxy S9

È l'host reale attuale del backend.

Ambiente:

- Android;
- Termux;
- Termux:Boot;
- Termux:API disponibile;
- telefono pensato per rimanere alimentato durante l'esercizio del servizio.

Il Galaxy S9 è anche l'ambiente su cui vanno eseguiti i test runtime quando
uno step modifica comportamento, dipendenze native, startup o integrazioni.

### PC Windows

È il punto di lavoro principale per:

- modificare comodamente i file;
- controllare `git diff` e `git status`;
- creare commit;
- fare push su GitHub.

### GitHub

È il punto centrale tra i dispositivi e la fonte ufficiale della cronologia.

## 6. Stato del progetto

Completati e verificati:

- Step 1 — scheletro;
- Step 2 — schema dati core;
- Step 3 — backend Telegram + whitelist;
- Step 3.1 — handoff, workflow Git, CI e Dependabot;
- Step 4 — SQLite operativo, migration automatiche e `/status`.

Prossimo sviluppo funzionale:

- Step 5 — modulo Oggetti generici.

Verifiche reali dello Step 3 sul Galaxy S9:

- `cargo test` superato;
- `/ping` funzionante;
- `/start` funzionante;
- secondo account Telegram non autorizzato ignorato correttamente;
- `Cargo.lock` generato durante la build verificata e versionato.

Problema già incontrato: un checkout S9 rimasto sul vecchio `Cargo.toml`
attivava `native-tls -> OpenSSL`. Riallineando il telefono al `main` corretto,
Teloxide usa `rustls` e il problema è scomparso. Non reintrodurre
`native-tls` senza una ragione esplicita e documentata.

## 7. Step 3.1 — repository e qualità

Lo Step 3.1 introduce solo infrastruttura di sviluppo/documentazione:

- questo file di handoff;
- workflow operativo Git esplicito;
- `.github/workflows/ci.yml`;
- `.github/dependabot.yml`;
- correzione della documentazione ormai obsoleta;
- controlli automatici su Rust stable, senza fissare per ora un MSRV formale.

Non introduce SQLite operativo e non aggiunge comandi Telegram.

**Chiusura verificata:** la prima run CI ha rilevato formattazione Rust non
applicata e un controllo MSRV 1.88 non utile al progetto. Dopo `cargo fmt` sul
Galaxy S9 e la semplificazione della CI su Rust stable, la run successiva ha
superato `fmt`, `check`, `test` e `clippy`. Lo Step 3.1 è quindi chiuso.


## 8. Step 4 — SQLite runtime

Lo Step 4 collega il backend Telegram allo schema SQLite gia' progettato.

Decisioni attuali:

- SQLx `0.8.6`, senza feature predefinite;
- feature: `runtime-tokio`, `sqlite`, `migrate`, `macros`;
- SQLite bundled: non dipende dalla libreria SQLite di sistema per il backend;
- `DATABASE_URL` opzionale, default `sqlite://data/db/gestionale.db`;
- foreign key abilitate esplicitamente nelle `SqliteConnectOptions`;
- migration incorporate con `sqlx::migrate!()` e applicate ad ogni avvio;
- `build.rs` forza la ricompilazione quando cambia `migrations/`;
- `SqlitePool` condiviso tramite le dipendenze del dispatcher Teloxide;
- `/status` verifica foreign key, numero di migration applicate e presenza
  delle cinque tabelle dello schema core (`items`, `foto`, `tag`, `item_tag`,
  `promemoria`);
- `scripts/backup.sh` usa il comando `.backup` di SQLite, evitando una semplice
  copia del file mentre il backend puo' essere attivo.

Lo Step 4 è **chiuso e verificato sul Galaxy S9**. Sono stati eseguiti:

1. `cargo check` con SQLx 0.8.6;
2. verifica dell'assenza di `openssl-sys`;
3. `cargo test --locked` con 2 test superati;
4. `cargo run --locked`;
5. verifica della creazione reale di `data/db/gestionale.db`;
6. `/start`, `/ping` e `/status`;
7. verifica di foreign key, migration applicata e cinque tabelle core;
8. secondo avvio sullo stesso database senza errori o riapplicazione
   distruttiva della migration.

Il warning Rust su `proc-macro-error2 v2.0.1` è una future incompatibility, non
un errore attuale, e va rivalutato durante futuri upgrade delle dipendenze.

Dopo la chiusura dello Step 4, il prossimo sviluppo previsto e' **Step 5 —
modulo Oggetti generici**.

## 8.1 Step 5A — Oggetti generici

Il primo modulo applicativo usa la tabella core `items` e la nuova tabella
specifica `oggetti`. La documentazione completa è in
`docs/moduli/oggetti.md`.

Il menu principale Telegram mostra pulsanti cliccabili; i comandi testuali
restano disponibili in parallelo. Il dispatcher gestisce sia `Message` sia
`CallbackQuery`. La whitelist viene applicata a entrambi gli ingressi.

Comandi Step 5A:

- `/oggetti`;
- `/oggetto_nuovo [nome]`;
- `/oggetti_lista`;
- `/oggetto_cerca [testo]`;
- `/oggetto <id>`;
- `/annulla`;
- `/salta`.

Lo stato delle bozze è conservato in memoria per chat tramite `SessionStore`.
Non contiene token o altri segreti.

## 9. Workflow Git ufficiale attuale

### 9.1 Regole generali

- lavorare normalmente da un dispositivo alla volta;
- prima di iniziare, verificare sempre `git status`;
- aggiornarsi usando `git pull --ff-only`;
- evitare merge automatici non intenzionali;
- GitHub `main` prevale sugli snapshot ZIP;
- dopo un push, riallineare l'altro dispositivo prima di modificarlo.

### 9.2 Flusso normale: sviluppo dal PC

Sul PC:

```bash
git status
git pull --ff-only
# modifica dei file
# eventuali test locali
git diff
git add .
git status
git commit -m "Step X: descrizione"
git push
```

Su GitHub:

1. controllare che il push sia arrivato;
2. controllare GitHub Actions;
3. non considerare superato un controllo che risulta rosso o non eseguito.

Sul Galaxy S9:

```bash
cd ~/gestionale-casa
git status
git pull --ff-only
```

Poi eseguire i test runtime richiesti dallo step.

### 9.3 Eccezione: modifica nata sull'S9

Una modifica piccola e strettamente legata all'ambiente S9 può essere
committata dal telefono. Esempio già avvenuto: `Cargo.lock` generato nella
build verificata.

Sul telefono:

```bash
git status
git pull --ff-only
# modifica / file generato / test
git add <file>
git diff --cached
git commit -m "Descrizione"
git push
```

Poi sul PC:

```bash
git status
git pull --ff-only
```

Dopo questo riallineamento, il PC torna a essere il punto principale di
sviluppo.

### 9.4 Se `git pull --ff-only` fallisce

Non forzare e non usare subito `reset --hard`, `push --force` o merge casuali.
Prima controllare:

```bash
git status
git log --oneline --decorate -5
git fetch origin
git log --oneline --left-right HEAD...origin/main
```

Capire quale dispositivo contiene modifiche non pubblicate prima di decidere
come riallineare.

## 10. Segreti e dati locali

Variabili previste:

- `TELOXIDE_TOKEN` — segreto;
- `ALLOWED_CHAT_IDS` — configurazione privata;
- `DATABASE_URL` — configurazione non segreta; opzionale, con default `sqlite://data/db/gestionale.db`.

Non committare:

- `.env` reale;
- token Telegram;
- PAT GitHub;
- password o chiavi private;
- database SQLite reale;
- foto/PDF personali presenti in `data/`.

`.env.example` deve contenere solo nomi delle variabili ed esempi non reali.

## 11. Controlli automatici

Il workflow `.github/workflows/ci.yml` viene eseguito su push e pull request
verso `main` e controlla:

1. `cargo fmt --all -- --check`;
2. `cargo check --locked`;
3. `cargo test --locked`;
4. `cargo clippy --all-targets --locked -- -D warnings`.

La CI usa un toolchain Rust stable aggiornato e non definisce, per ora, un MSRV
(versione minima Rust) formale. La run di chiusura dello Step 3.1 ha superato
tutti e quattro i controlli. La CI non sostituisce i test sul Galaxy S9: un
runner Linux GitHub e Android Termux sono ambienti diversi.

Dependabot controlla settimanalmente:

- dipendenze Cargo;
- versioni delle GitHub Actions.

Gli aggiornamenti devono arrivare come pull request da valutare; non è previsto
auto-merge.

## 12. Workflow futuro di amministrazione remota

**Non implementato al momento.**

Obiettivo futuro: poter aprire una shell Termux dell'S9 dal PC anche quando i
due dispositivi non sono sulla stessa rete.

Soluzione prevista da valutare e testare in uno step dedicato:

```text
PC Windows
    |
    | rete privata Tailscale
    v
Galaxy S9
    |
    v
OpenSSH server in Termux
```

Principi:

- niente port forwarding SSH pubblico sul router;
- Tailscale serve solo a creare connettività privata tra i dispositivi;
- sull'S9 si userebbe un normale server OpenSSH in Termux;
- preferire autenticazione SSH a chiave;
- definire regole di accesso Tailscale restrittive;
- verificare comportamento in background, riavvio Android e cambio rete prima
  di dichiarare la soluzione stabile.

Nota tecnica: il componente **server** della funzione “Tailscale SSH” è
supportato ufficialmente su Linux e macOS open-source, non su Android. Per
l'S9 il progetto prevede quindi Tailscale come rete privata + OpenSSH di
Termux come servizio SSH, non il server Tailscale SSH integrato.

## 13. Step corrente — Step 5A Oggetti generici

**Step 4 è chiuso e verificato sul Galaxy S9.**

Lo **Step 5A è implementato ma non ancora chiuso**. La verifica deve ancora
passare GitHub Actions e il test runtime sul Galaxy S9.

File principali dello Step 5A:

- `migrations/20260814121600_oggetti.sql`;
- `src/modules/oggetti.rs`;
- `src/main.rs`;
- `docs/moduli/oggetti.md`.

Decisioni da preservare:

- solo il nome è obbligatorio;
- i dettagli opzionali si aggiungono da un pannello Telegram;
- inline keyboard e `/comandi` convivono e richiamano la stessa logica;
- il numero seriale resta disponibile ma secondario;
- prezzi e valori sono memorizzati come centesimi interi;
- `items` e `oggetti` vengono creati nella stessa transazione;
- le bozze incomplete sono in memoria e non sopravvivono al riavvio;
- la migration core dello Step 2 non va modificata.

Sottostep successivi previsti solo dopo la chiusura del 5A:

- 5B modifica/eliminazione;
- 5C foto/documenti/tag;
- 5D garanzie/promemoria;
- 5E prestiti/storico.

## 14. Regola di chiusura di ogni step

Ogni step deve lasciare documentati:

1. **stato precedente**;
2. **modifiche effettuate**;
3. **verifiche realmente effettuate**;
4. **problemi incontrati e relative soluzioni**;
5. **stato finale**;
6. **prossimo passo previsto**.

Aggiornare almeno `CHANGELOG.md`; aggiornare anche `README.md`,
`ARCHITETTURA.md` o i documenti dei moduli quando il loro contenuto cambia.
