# Gestionale Casa

Gestionale personale/condiviso basato su **Rust + SQLite + Telegram**, pensato
per organizzare beni, luoghi e funzioni della vita quotidiana senza richiedere
un'app dedicata.

## Stato corrente — Step 7

Gli Step 1→6C sono chiusi e presenti in `main` tramite il merge `219caba`.

Lo sviluppo corrente avviene sul branch:

```text
step-7-alimentazione
```

Lo **Step 7 — Fondazioni condivise e Alimentazione** ha chiuso il checkpoint
documentale 7.0 con `135dd33` ed è entrato nella macro-fase tecnica 7.1.
Il primo checkpoint tecnico 7.1 (`a650bc8`) ha introdotto utenti, spazi,
membership, collegamento Telegram e audit con autore. Il checkpoint successivo
rende gli **spazi operativi**: lo spazio attivo isola oggetti, luoghi,
contenitori, foto e storico; il bot può creare, rinominare e cambiare spazio.

Il vecchio `gestionale_step7_prototipo_bundle` è superato e non va usato come
base di sviluppo.

### Macro-fasi Step 7

- [x] **7.0 — Specifica e organizzazione** — documentazione e decisioni;
- [ ] **7.1 — Fondazioni condivise** — **IN SVILUPPO**: utenti, spazi, ruoli, inviti, audit e reminder;
- [ ] **7.2 — Alimentazione completa** — alimenti, ricette, profili, turni, planner e spesa;
- [ ] **7.3 — Integrazioni** — condivisione operativa, Google Calendar ed email.

Dettagli: **[docs/step7/README.md](docs/step7/README.md)**.

## Alimentazione

Il modulo Alimentazione comprenderà alimenti strutturati, ricette, profili e
porzioni personalizzate, turni/routine, pianificazione dei pasti, lista della
spesa, reminder ed export.

Il `README.md` centrale mantiene solo questa panoramica. La specifica completa è
qui:

**[docs/moduli/alimentazione/README.md](docs/moduli/alimentazione/README.md)**

## Moduli futuri già specificati

Alcune funzioni sono state documentate ora perché influenzano le fondamenta
condivise, ma non fanno parte dell'implementazione immediata della macro-fase Alimentazione:

- [Acquisti e prezzi](docs/moduli/acquisti/README.md) — prezzi base,
  confezioni e confronto volantini;
- [Viaggi](docs/moduli/viaggi/README.md) — bagagli, checklist, oggetti in
  viaggio e controllo rientro;
- [Spese](docs/moduli/spese/README.md) — spese personali/condivise, quote e saldi.

## Documentazione principale

Prima di modificare il progetto, leggere nell'ordine:

1. **[README.md](README.md)** — stato sintetico e workflow;
2. **[docs/step7/README.md](docs/step7/README.md)** — step corrente;
3. **[ARCHITETTURA.md](ARCHITETTURA.md)** — scelte tecniche e motivazioni;
4. **[docs/moduli/alimentazione/README.md](docs/moduli/alimentazione/README.md)** — specifica del modulo corrente;
5. **[docs/ROADMAP.md](docs/ROADMAP.md)** — roadmap generale;
6. **[docs/HANDOFF.md](docs/HANDOFF.md)** — consegna operativa completa;
7. **[CHANGELOG.md](CHANGELOG.md)** — cronologia degli step;
8. **[docs/schema-core.md](docs/schema-core.md)** — schema core già implementato;
9. **[docs/INFRASTRUTTURA.md](docs/INFRASTRUTTURA.md)** — PC/S9/Tailscale/SSH/GitHub;
10. **[docs/moduli/README.md](docs/moduli/README.md)** — indice dei moduli.

## Stato del progetto

- [x] Step 1 — Scheletro
- [x] Step 2 — Schema dati core
- [x] Step 3/3.1 — Telegram, whitelist, Git/CI
- [x] Step 4 — SQLite runtime + migration
- [x] Step 5 — Oggetti, foto, modifica/eliminazione
- [x] Step 6A — Case e stanze
- [x] Step 6B — Storico trasversale
- [x] Step 6C — Contenitori, navigazione gerarchica e storico container-aware
- [ ] Step 7 — Fondazioni condivise + Alimentazione

Il checkpoint `a650bc8` della 7.1 è stato verificato sull'S9 con **74/74 test**,
Clippy `-D warnings`, migration reale, `/profilo`, `/status` e audit autore.
Il blocco multi-spazio operativo va nuovamente verificato sull'S9 prima del
commit.

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

## Accesso operativo al Galaxy S9

L'accesso remoto è ora operativo tramite **Tailscale + OpenSSH Termux** e non dipende più dall'IP LAN del telefono. Sul PC l'alias SSH è:

```text
Host s9
    HostName galaxy-s9-di-alessio
    User u0_a266
    Port 8022
    IdentityFile ~/.ssh/id_ed25519_s9
    IdentitiesOnly yes
```

Uso quotidiano dal PC:

```powershell
ssh s9
scp "C:\percorso\patch.zip" s9:~/
```

La chiave consente l'accesso senza password. Tailscale fornisce il collegamento privato tra PC e S9 anche se cambia l'IP locale; non viene esposta pubblicamente la porta SSH.

Anche il Galaxy S9 usa GitHub via SSH con una chiave dedicata e remote `git@github.com:alessiolari01/gestionale-casa.git`, quindi `git push` non richiede più PAT. GitHub resta comunque la fonte ufficiale del codice.

Termux:Boot avvia wake lock e `sshd` dopo il reboot. Sul Galaxy S9 è stato osservato che Android può ritardare di alcuni minuti l'esecuzione di Termux:Boot; una volta avviato, SSH diventa disponibile praticamente subito.

La topologia completa, i comandi di diagnostica e le regole sui segreti sono in **`docs/INFRASTRUTTURA.md`**.

## Principi dello Step 7

- il database resta centrale e SQLite non viene condiviso fra account;
- utenti interni separati da Telegram/Google;
- dati condivisi organizzati in spazi;
- condividere ≠ copiare;
- storico con autore e distinzione degli effetti automatici;
- reminder Step 7 via Telegram/email, senza SMS;
- nessun reset generale dentro il bot;
- il DB corrente è ancora di sviluppo e può essere azzerato manualmente solo
  prima del go-live;
- Acquisti, Viaggi e Spese devono riusare le stesse fondamenta invece di creare
  sistemi paralleli.

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

## Struttura documentale principale

```text
docs/
├── step7/
│   ├── README.md
│   ├── roadmap.md
│   ├── decisioni-architetturali.md
│   ├── modello-condivisione.md
│   ├── storico-e-audit.md
│   └── database-e-migrazioni.md
└── moduli/
    ├── alimentazione/
    ├── acquisti/
    ├── viaggi/
    └── spese/
```

Il codice applicativo rimane sotto `src/modules/`. La 7.1 aggiunge
`src/identity.rs` come infrastruttura trasversale di identità/audit; i moduli
dominio vengono creati solo quando l'implementazione reale li richiede.

## Regola per gli step futuri

Ogni step deve documentare:

1. stato precedente;
2. decisioni;
3. modifiche effettive;
4. verifiche realmente eseguite;
5. problemi/soluzioni;
6. stato finale;
7. prossimo passo.

Le feature non ancora implementate devono essere marcate esplicitamente come
PREVISTO o RIMANDATO.

## UX Telegram compatta

Le tastiere inline raggruppano le azioni simili, usano `⚙️ Gestisci` per rinomina/modifica/eliminazione e mantengono `🗑 Elimina` isolato. Le azioni frequenti, come lo spostamento degli oggetti, restano direttamente accessibili. I figli gerarchici vengono mostrati prima dei comandi; le righe di creazione usano etichette compatte come `➕🚪 Stanza`, `➕📦 Contenitore`, `➕🏷️ Oggetto`, mentre gli elenchi usano `📋` + simbolo dell'entità.

### Step 7.1B — vista multi-spazio (in sviluppo)

Lo spazio selezionato è lo **spazio predefinito**; tramite Profilo/Spazi è possibile passare fra `🎯 Solo spazio predefinito` e `🌐 Tutti i miei spazi`. Gli oggetti mantengono il proprio spazio proprietario anche quando vengono collocati in una casa di un altro spazio accessibile. Dettagli in `docs/step7/`.
