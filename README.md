# Gestionale Casa

Gestionale personale per tenere traccia delle cose di casa (vestiti, veicoli,
ricette, oggetti generici) tramite un bot Telegram. Nessuna app dedicata da
installare per chi lo usa: basta scrivere al bot.

- Architettura e motivazioni: **[ARCHITETTURA.md](./ARCHITETTURA.md)**
- Cronologia dettagliata degli step: **[CHANGELOG.md](./CHANGELOG.md)**
- Schema dati core: **[docs/schema-core.md](./docs/schema-core.md)**

## Stato del progetto

- [x] Step 1 — Scheletro del progetto
- [x] Step 2 — Schema dati core
- [x] Step 3 — Backend Telegram + whitelist, verificato sul Galaxy S9
- [ ] Step 4 — Connessione SQLite + migration automatiche
- [ ] Modulo oggetti
- [ ] Modulo vestiti
- [ ] Modulo veicoli
- [ ] Modulo ricette

### Passo corrente

Lo **Step 3 è stato verificato realmente sul Galaxy S9**:

1. `cargo test` è completato correttamente;
2. il backend si collega alle API Telegram;
3. `/ping` risponde `Pong! Gestionale Casa è online.`;
4. `/start` risponde con i comandi disponibili;
5. un secondo account Telegram, non presente in `ALLOWED_CHAT_IDS`, non riceve
   alcuna risposta dal bot.

Durante il primo test è comparso un errore `openssl-sys`: il Galaxy S9 era
ancora sul `Cargo.toml` dello Step 2, che attivava le feature predefinite di
Teloxide e quindi `native-tls`. Riallineando il repository allo Step 3,
Teloxide usa `rustls` come previsto e la compilazione riesce senza OpenSSL
nativo. Il dettaglio è registrato in `CHANGELOG.md`.

Il prossimo sviluppo è lo **Step 4 — SQLite operativo**: connessione al
database, creazione delle cartelle, foreign key, migration automatiche e un
primo comando `/status`.

`Cargo.lock`, generato sul Galaxy S9 durante la compilazione verificata, va
versionato nel repository appena trasferito dal telefono al PC.

Il dettaglio di cosa è cambiato rispetto allo step precedente e del prossimo
step previsto è sempre registrato in `CHANGELOG.md`.

## Requisiti

- Rust 1.82+ (`rustup`)
- Un bot Telegram creato tramite [@BotFather](https://t.me/BotFather)
- SQLite

## Setup su Termux (Android)

```bash
# Pacchetti di base
pkg update && pkg upgrade
pkg install git rust sqlite

# Clona il repository
git clone <url-del-tuo-repo> gestionale-casa
cd gestionale-casa

# Se le variabili non sono già state configurate nell'ambiente:
cp .env.example .env
# Apri .env e inserisci token e chat_id. DATABASE_URL servirà dallo Step 4.
# Non committare mai il file .env.

# Primo controllo
cargo test

# Compila ed esegui
cargo run
```

Quando il test base è concluso, per l'uso continuativo si userà una build
release:

```bash
cargo build --release
```

Per far partire il bot automaticamente all'accensione del telefono:

1. Installa **Termux:Boot** (da F-Droid, non dal Play Store).
2. Copia `scripts/termux-boot.sh` in `~/.termux/boot/`.
3. Attiva `termux-wake-lock` all'avvio (già incluso nello script) così Android
   non sospende il processo.
4. Disattiva l'ottimizzazione batteria per Termux nelle impostazioni Android.

> L'avvio automatico verrà considerato operativo solo dopo aver verificato il
> backend manualmente sul Galaxy S9.

## Setup su Linux (Raspberry Pi / PC)

```bash
sudo apt update && sudo apt install git build-essential sqlite3
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone <url-del-tuo-repo> gestionale-casa
cd gestionale-casa
cp .env.example .env
# Inserisci token e chat_id in .env

cargo test
cargo build --release
```

Per l'avvio automatico si userà `systemd`; il relativo service verrà aggiunto
quando il backend sarà pronto per il primo deploy stabile.

## Struttura del repository

```text
gestionale-casa/
├── README.md               # quick start e stato sintetico
├── CHANGELOG.md            # diario degli step e prossimo passo
├── ARCHITETTURA.md         # architettura e motivazioni delle scelte
├── Cargo.toml
├── src/
│   ├── main.rs             # avvio bot e dispatcher Telegram
│   ├── config.rs           # lettura e validazione configurazione
│   ├── db.rs               # connessione database e migrazioni (step 4)
│   ├── auth.rs             # whitelist chat autorizzate
│   └── modules/            # un file per ciascun modulo funzionale
│       ├── oggetti.rs
│       ├── vestiti.rs
│       ├── veicoli.rs
│       └── ricette.rs
├── migrations/             # file .sql di migrazione schema database
├── scripts/                # script di avvio e backup
├── docs/moduli/            # documentazione dettagliata dei moduli
└── data/                   # database e foto (NON versionato su Git)
```

## Regola per gli step futuri

Ogni nuovo step deve:

1. partire dallo stato dell'ultimo ZIP/repository;
2. modificare solo ciò che serve allo scopo dello step;
3. aggiornare `CHANGELOG.md` con differenze, verifiche e prossimo passo;
4. aggiornare lo stato sintetico in questo README;
5. evitare di dichiarare “fatto” ciò che non è stato realmente verificato.
