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
6. **[docs/moduli/oggetti.md](./docs/moduli/oggetti.md)** — specifica del modulo Oggetti;
7. **[docs/moduli/foto.md](./docs/moduli/foto.md)** — specifica Step 5B;
8. **[docs/moduli/modifica-eliminazione.md](./docs/moduli/modifica-eliminazione.md)** — specifica Step 5C;
9. **[docs/ROADMAP.md](./docs/ROADMAP.md)** — requisiti futuri e decisioni ancora da confermare.

## Stato del progetto

- [x] Step 1 — Scheletro del progetto
- [x] Step 2 — Schema dati core
- [x] Step 3 — Backend Telegram + whitelist, verificato sul Galaxy S9
- [x] Step 3.1 — Handoff, workflow Git, CI e Dependabot, verificato con
  GitHub Actions
- [x] Step 4 — SQLite operativo + migration automatiche + `/status`, verificato sul Galaxy S9
- [ ] Step 5 — Modulo oggetti
  - [x] Step 5A — anagrafica, pulsanti + comandi, creazione, elenco, ricerca e scheda
  - [x] Step 5B — foto locali degli oggetti + navigazione di avvio/status
  - [ ] Step 5C — modifica ed eliminazione degli oggetti gia' salvati (implementazione in test)
- [ ] Modulo vestiti
- [ ] Modulo veicoli
- [ ] Modulo ricette

### Ultimo step funzionale verificato

Lo **Step 5B — Foto degli oggetti è chiuso e verificato**. È stato integrato in
`main` con CI GitHub Actions verde. Sul Galaxy S9 sono stati verificati:

1. `cargo fmt --all -- --check`, `cargo check --locked`, 11 test automatici e
   Clippy con `-D warnings`;
2. messaggio automatico `🟢 Gestionale Casa è online` con menu principale;
3. ritorno al menu principale da `/status` e da Stato sistema;
4. caricamento di due foto sullo stesso oggetto, con ruoli `principale` e
   `galleria`;
5. presenza reale dei file in `data/media/oggetti/<id>/`;
6. visualizzazione delle foto dal file locale e persistenza dopo riavvio;
7. pulizia dei file di test prima dell'uso sul database reale;
8. `scripts/backup.sh` eseguibile e predisposto a includere `data/media`.

Il warning di compatibilità futura relativo a `proc-macro-error2 v2.0.1` non ha
bloccato build o test e resta una nota da rivalutare durante futuri aggiornamenti.

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

Gli **Step 5A e 5B sono chiusi**. È in sviluppo lo **Step 5C — Modifica ed
eliminazione degli oggetti già salvati**. L'implementazione di test aggiunge:

- pulsanti `✏️ Modifica` e `🗑 Elimina` nella scheda oggetto;
- comandi equivalenti `/oggetto_modifica <id>` e `/oggetto_elimina <id>`;
- modifica del nome e di tutti i dettagli già supportati;
- `/salta` per mantenere il valore corrente e `/rimuovi` per cancellare il campo
  aperto;
- salvataggio atomico delle modifiche senza creare duplicati;
- eliminazione solo dopo una conferma esplicita e irreversibile;
- cascade SQLite dei dati collegati e rimozione della directory locale
  `data/media/oggetti/<id>/`;
- nessuna nuova migration: lo schema esistente è sufficiente.

Lo Step 5C non va considerato stabile finché non supera test automatici, test
runtime sul Galaxy S9 e CI della Pull Request.

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

Sulla rete locale è ora disponibile un **server OpenSSH in Termux**. Dal PC si
può quindi aprire direttamente il terminale dell'S9 e trasferire patch senza
usare GitHub come semplice mezzo di trasporto:

```text
ssh -p 8022 <utente-termux>@<ip-lan-s9>
scp -P 8022 <file> <utente-termux>@<ip-lan-s9>:~/
```

GitHub `main` resta comunque la **fonte ufficiale**: SSH/SCP servono per sviluppo
e test, poi le modifiche verificate devono essere committate e pubblicate. Non
vanno salvati nel repository IP locali, password o altri segreti.

L'accesso da reti esterne resta futuro: la soluzione prevista è
**Tailscale + OpenSSH Termux**, senza port forwarding pubblico della porta SSH.

## Requisito futuro: più case e stanze

Il gestionale dovrà supportare più abitazioni separate e permettere sia viste
filtrate sia ricerche combinate. In particolare sono requisiti già registrati:

- più case/abitazioni nello stesso gestionale;
- stanze appartenenti a una casa e riconosciute dal sistema;
- assegnazione e spostamento di un oggetto scegliendo una stanza registrata;
- elenco per casa e per stanza;
- ricerca limitata a una casa/stanza oppure estesa a tutte le case.

La struttura dati definitiva non è ancora approvata. L'ipotesi preferita è un
sistema gerarchico di **luoghi** (casa -> stanza -> eventuale sotto-posizione),
riutilizzabile anche dagli altri moduli. Verrà proposta e confermata prima
dell'implementazione.

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
│       ├── foto.rs             # foto locali collegate agli items
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
