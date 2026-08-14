# Gestionale Casa

Gestionale personale per tenere traccia delle cose di casa (vestiti, veicoli,
ricette, oggetti generici) tramite un bot Telegram. Nessuna app dedicata da
installare per chi lo usa: basta scrivere al bot.

## Documentazione principale

Prima di modificare il progetto, leggere nell'ordine:

1. **[README.md](./README.md)** — stato sintetico, setup e workflow quotidiano;
2. **[ARCHITETTURA.md](./ARCHITETTURA.md)** — scelte tecniche e motivazioni;
3. **[CHANGELOG.md](./CHANGELOG.md)** — cronologia degli step e verifiche;
4. **[docs/HANDOFF.md](./docs/HANDOFF.md)** — consegna completa per chi deve
   continuare il progetto;
5. **[docs/schema-core.md](./docs/schema-core.md)** — schema dati condiviso;
6. **[docs/moduli/oggetti.md](./docs/moduli/oggetti.md)** — specifica Step 5A.

## Stato del progetto

- [x] Step 1 — Scheletro del progetto
- [x] Step 2 — Schema dati core
- [x] Step 3 — Backend Telegram + whitelist, verificato sul Galaxy S9
- [x] Step 3.1 — Handoff, workflow Git, CI e Dependabot, verificato con
  GitHub Actions
- [x] Step 4 — SQLite operativo + migration automatiche + `/status`, verificato sul Galaxy S9
- [ ] Step 5 — Modulo oggetti
  - [ ] Step 5A — anagrafica, pulsanti + comandi, creazione, elenco, ricerca e scheda
- [ ] Modulo vestiti
- [ ] Modulo veicoli
- [ ] Modulo ricette

### Ultimo step funzionale verificato

Lo **Step 4 è stato verificato realmente sul Galaxy S9**. La catena
`Telegram -> Rust -> SQLx -> SQLite` è ora operativa.

Verifiche effettuate:

1. `cargo check` con SQLx 0.8.6 completato correttamente;
2. `cargo test --locked` completato con 2 test superati e 0 falliti;
3. `openssl-sys` assente dalla dependency graph;
4. `cargo run --locked` avvia il backend e crea `data/db/gestionale.db`;
5. `/start`, `/ping` e `/status` funzionano;
6. `/status` conferma foreign key attive, migration applicata e tutte le tabelle
   dello schema core presenti;
7. un secondo avvio sullo stesso database avviene senza errori e senza
   riapplicare in modo distruttivo la migration.

Il warning di compatibilità futura relativo a `proc-macro-error2 v2.0.1` non ha
bloccato build o test ed è mantenuto come nota da rivalutare durante futuri
aggiornamenti delle dipendenze.

Per completezza, lo **Step 3** aveva già verificato realmente sul Galaxy S9:

1. `cargo test` è completato correttamente;
2. il backend si collega alle API Telegram;
3. `/ping` risponde `Pong! Gestionale Casa è online.`;
4. `/start` risponde con i comandi disponibili;
5. un secondo account Telegram, non presente in `ALLOWED_CHAT_IDS`, non riceve
   alcuna risposta dal bot.

Durante il primo test era comparso un errore `openssl-sys`: il Galaxy S9 era
ancora sul `Cargo.toml` dello Step 2, che attivava le feature predefinite di
Teloxide e quindi `native-tls`. Riallineando il repository allo Step 3,
Teloxide usa `rustls` come previsto e la compilazione riesce senza OpenSSL
nativo. Il dettaglio resta documentato in `CHANGELOG.md`.

`Cargo.lock` è già versionato ed è quello generato durante la build verificata
sul Galaxy S9.

### Step 3.1 verificato

Lo **Step 3.1** non aggiunge funzioni al gestionale. Rende il repository più
facile da mantenere e consegnare ad altri attraverso:

- `docs/HANDOFF.md`;
- workflow Git esplicito PC ↔ GitHub ↔ Galaxy S9;
- CI GitHub Actions su Rust stable per formattazione, check, test e Clippy;
- Dependabot settimanale per Cargo e GitHub Actions, senza auto-merge.

La prima esecuzione della CI ha evidenziato due problemi di qualità/configurazione:
formattazione Rust non ancora applicata e un controllo MSRV 1.88 troppo rigido per
questo progetto. Dopo `cargo fmt` sul Galaxy S9 e la semplificazione della CI su
Rust stable, una nuova esecuzione GitHub Actions ha completato con esito positivo
`fmt`, `check`, `test` e `clippy`. Lo **Step 3.1 è quindi chiuso e verificato**.

### Passo corrente

Lo **Step 4 — SQLite operativo** è chiuso e verificato sul Galaxy S9.

È ora **implementato lo Step 5A — Oggetti generici**, ancora da verificare su
GitHub Actions e sul Galaxy S9 prima di dichiararlo chiuso. Lo Step 5A aggiunge:

- nuova migration `oggetti`, senza modificare la migration core già applicata;
- menu Telegram con inline keyboard;
- comandi testuali equivalenti ai pulsanti;
- creazione rapida con solo nome oppure pannello dettagli opzionale;
- salvataggio atomico `items + oggetti`;
- elenco paginato, ricerca e scheda singola;
- test automatici del parsing e della persistenza.

La specifica del modulo è in `docs/moduli/oggetti.md`.

## Fonte ufficiale e workflow corrente

La **fonte ufficiale del progetto è il branch `main` su GitHub**:

`https://github.com/alessiolari01/gestionale-casa`

Gli ZIP sono solo snapshot/backup e non devono prevalere su un `main` più
recente.

Per il momento i ruoli sono:

- **PC Windows**: sviluppo principale, modifica dei file, commit e push;
- **GitHub `main`**: fonte ufficiale e punto di sincronizzazione;
- **Galaxy S9 + Termux**: host reale del backend e dispositivo di test.

### Flusso normale: modifica dal PC

Prima di iniziare:

```bash
git status
git pull --ff-only
```

Dopo le modifiche e i controlli:

```bash
git add .
git commit -m "Descrizione dello step"
git push
```

Poi sul Galaxy S9:

```bash
cd ~/gestionale-casa
git status
git pull --ff-only
```

Se lo step modifica il comportamento runtime, va poi verificato realmente
sull'S9 prima di dichiararlo completato.

### Eccezione: piccola modifica nata sull'S9

Quando una modifica ha senso direttamente sul telefono (per esempio un file
generato durante una build verificata), il flusso può essere invertito:

```text
S9 -> commit/push -> GitHub -> git pull --ff-only sul PC
```

Dopo il riallineamento si torna a usare normalmente il PC come punto di
sviluppo. Evitare di modificare contemporaneamente gli stessi file su PC e
S9.

Il workflow completo, incluse gestione dei conflitti, segreti e consegna a
terzi, è descritto in `docs/HANDOFF.md`.

## Amministrazione remota futura

L'accesso remoto diretto all'S9 **non fa parte dello Step 3.1**. In futuro è
prevista una soluzione basata su **Tailscale + server OpenSSH in Termux**, in
modo da amministrare il telefono dal PC anche fuori dalla stessa rete senza
esporre una porta SSH direttamente a Internet.

Questa estensione verrà progettata e testata in uno step dedicato; per ora il
workflow GitHub resta il metodo ufficiale di gestione tra PC e S9.

## Requisiti

- un toolchain Rust **stable aggiornato**;
- un bot Telegram creato tramite `@BotFather`;
- SQLite CLI (`sqlite3`) per diagnostica e backup; SQLx usa SQLite bundled nel backend.

Non viene dichiarata per ora una versione minima Rust (MSRV) formale: il progetto
usa il `Cargo.lock` versionato, la CI verifica Rust stable e il Galaxy S9 resta
l'ambiente reale di test runtime.

## Setup su Termux (Android)

```bash
# Pacchetti di base
pkg update && pkg upgrade
pkg install git rust sqlite

# Clona il repository ufficiale
git clone https://github.com/alessiolari01/gestionale-casa.git
cd gestionale-casa

# Se le variabili non sono già state configurate nell'ambiente:
cp .env.example .env
# Apri .env e inserisci token e chat_id. DATABASE_URL e' opzionale: se manca,
# viene usato sqlite://data/db/gestionale.db. Non committare mai il file .env.

# Primo controllo
cargo test --locked

# Compila ed esegui
cargo run --locked
```

Quando il test base è concluso, per l'uso continuativo si userà una build
release:

```bash
cargo build --release --locked
```

Per far partire il bot automaticamente all'accensione del telefono:

1. installa **Termux:Boot** da una fonte compatibile con Termux;
2. copia `scripts/termux-boot.sh` in `~/.termux/boot/`;
3. attiva `termux-wake-lock` all'avvio, come previsto dallo script;
4. disattiva l'ottimizzazione batteria per Termux nelle impostazioni Android.

> L'avvio automatico va verificato separatamente prima di considerarlo parte
> dell'esercizio stabile del servizio.

## Setup su Linux / futuro Raspberry Pi

```bash
sudo apt update && sudo apt install git build-essential sqlite3
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone https://github.com/alessiolari01/gestionale-casa.git
cd gestionale-casa
cp .env.example .env
# Inserisci token e chat_id in .env

cargo test --locked
cargo build --release --locked
```

Per l'avvio automatico si userà `systemd`; il relativo service verrà aggiunto
quando il backend sarà pronto per il primo deploy stabile su Linux.

## Struttura del repository

```text
gestionale-casa/
├── .github/
│   ├── workflows/
│   │   └── ci.yml           # controlli automatici Rust
│   └── dependabot.yml       # aggiornamenti dipendenze via PR
├── README.md                # quick start e stato sintetico
├── CHANGELOG.md             # diario degli step e verifiche
├── ARCHITETTURA.md          # architettura e motivazioni delle scelte
├── Cargo.toml
├── Cargo.lock               # versioni effettivamente bloccate/testate
├── build.rs                  # ricompila se cambiano le migration
├── src/
│   ├── main.rs              # avvio bot e dispatcher Telegram
│   ├── config.rs            # lettura e validazione configurazione
│   ├── db.rs                # pool SQLite, migration e stato runtime
│   ├── auth.rs              # whitelist chat autorizzate
│   └── modules/
│       ├── oggetti.rs
│       ├── vestiti.rs
│       ├── veicoli.rs
│       └── ricette.rs
├── migrations/              # file .sql di modifica schema
├── scripts/                 # avvio e backup
├── docs/
│   ├── HANDOFF.md           # istruzioni complete per continuare
│   ├── schema-core.md
│   └── moduli/
└── data/                    # database e file locali, NON versionati
```

## Regola per gli step futuri

Ogni nuovo step deve:

1. partire dal `main` aggiornato (`git pull --ff-only`);
2. modificare solo ciò che serve allo scopo dello step;
3. aggiornare `CHANGELOG.md` con **stato precedente → modifiche → verifiche →
   prossimo passo**;
4. aggiornare lo stato sintetico nel README e, se cambia una decisione di
   design, `ARCHITETTURA.md`;
5. eseguire i controlli disponibili e verificare GitHub Actions dopo il push;
6. effettuare sul Galaxy S9 i test runtime pertinenti;
7. non dichiarare “fatto” ciò che non è stato realmente verificato;
8. non committare mai `.env`, token Telegram, PAT GitHub, database reale o
   altri segreti.
