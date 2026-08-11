# Gestionale Casa

Gestionale personale per tenere traccia delle cose di casa (vestiti, veicoli,
ricette, oggetti generici) tramite un bot Telegram. Nessuna app da installare
per chi lo usa: basta scrivere al bot.

Per la descrizione completa dell'architettura, le decisioni di design e il
"perché" delle scelte fatte, vedi **[ARCHITETTURA.md](./ARCHITETTURA.md)**.

## Stato del progetto

- [x] Scheletro del progetto
- [ ] Schema dati core (foto, categorie, promemoria)
- [ ] Modulo oggetti
- [ ] Modulo vestiti
- [ ] Modulo veicoli
- [ ] Modulo ricette

## Requisiti

- Rust 1.82+ (`rustup`)
- Un bot Telegram creato tramite [@BotFather](https://t.me/BotFather)
- SQLite (incluso, nessuna installazione separata richiesta)

## Setup su Termux (Android)

```bash
# Pacchetti di base
pkg update && pkg upgrade
pkg install git rust sqlite

# Clona il repository
git clone <url-del-tuo-repo> gestionale-casa
cd gestionale-casa

# Configura le variabili d'ambiente
cp .env.example .env
# Apri .env e inserisci il token del bot (da @BotFather) e il tuo chat_id

# Compila ed esegui
cargo run --release
```

Per far partire il bot automaticamente all'accensione del telefono:

1. Installa **Termux:Boot** (da F-Droid, non dal Play Store).
2. Copia `scripts/termux-boot.sh` in `~/.termux/boot/`.
3. Attiva `termux-wake-lock` all'avvio (già incluso nello script) così Android
   non sospende il processo.
4. Disattiva l'ottimizzazione batteria per Termux nelle impostazioni Android
   (altrimenti il sistema può comunque terminare il processo in background).

## Setup su Linux (Raspberry Pi / PC)

```bash
sudo apt update && sudo apt install git build-essential sqlite3
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone <url-del-tuo-repo> gestionale-casa
cd gestionale-casa
cp .env.example .env
# Inserisci token e chat_id in .env

cargo build --release
```

Per l'avvio automatico, usa `systemd` (vedi `scripts/gestionale-casa.service`
una volta creato — verrà aggiunto quando il backend sarà pronto per il primo
deploy reale).

## Struttura del repository

```
gestionale-casa/
├── README.md              # questo file
├── ARCHITETTURA.md         # descrizione completa dell'architettura
├── Cargo.toml
├── src/
│   ├── main.rs              # avvio bot, caricamento configurazione
│   ├── config.rs            # lettura variabili d'ambiente
│   ├── db.rs                # connessione database e migrazioni
│   ├── auth.rs               # whitelist utenti autorizzati
│   └── modules/               # un file per ciascun modulo funzionale
│       ├── oggetti.rs
│       ├── vestiti.rs
│       ├── veicoli.rs
│       └── ricette.rs
├── migrations/              # file .sql di migrazione schema database
├── scripts/                  # script di avvio e backup
├── docs/moduli/              # documentazione dettagliata di ogni modulo
└── data/                      # database e foto (NON versionato su git)
```

## Documentazione dei moduli

Ogni modulo ha un file dedicato in `docs/moduli/`, aggiornato mano a mano
che viene progettato e implementato.
