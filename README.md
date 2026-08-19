# Gestionale Casa


## Step 6C — Luoghi gerarchici e navigazione contestuale

Il gestionale tratta **case, stanze e contenitori** come un unico sistema di luoghi. `4c64798` è il checkpoint 6C.2 verificato su Galaxy S9; 6C.3A aggiunge navigazione contestuale, vista ad albero e creazione di un oggetto direttamente dal luogo corrente.

Regola UI: le azioni dipendono dal luogo visualizzato e ogni schermata interna rilevante offre ritorno al livello logico precedente e `🏠 Menu principale`.

Specifiche: `docs/moduli/navigazione-luoghi.md` e `docs/moduli/contenitori.md`.


## Stato Step 6B

Lo **Step 6B — Storico trasversale globale + individuale** è implementato e verificato sul Galaxy S9. Sono disponibili storico globale e individuale, dettaglio prima/dopo, paginazione e filtri combinabili per periodo, modulo, operazione, casa, stanza ed elemento.

Verifica finale locale: **37/37 test**, Clippy con `-D warnings` e test runtime Telegram superati. Prima della chiusura ufficiale restano PR, CI GitHub verde e merge su `main`. Dopo il merge il prossimo sviluppo approvato è **Step 6C — Contenitori e sotto-posizioni**.

Documentazione tecnica: `docs/moduli/storico.md`.

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
9. **[docs/moduli/luoghi.md](./docs/moduli/luoghi.md)** — specifica Step 6A;
10. **[docs/ROADMAP.md](./docs/ROADMAP.md)** — sequenza approvata e future implementazioni.

## Stato del progetto

- [x] Step 1 — Scheletro del progetto
- [x] Step 2 — Schema dati core
- [x] Step 3 — Backend Telegram + whitelist, verificato sul Galaxy S9
- [x] Step 3.1 — Handoff, workflow Git, CI e Dependabot
- [x] Step 4 — SQLite operativo + migration automatiche + `/status`
- [x] Step 5 — Modulo Oggetti
  - [x] Step 5A — anagrafica, creazione, elenco, ricerca e scheda
  - [x] Step 5B — foto locali + navigazione di avvio/status
  - [x] Step 5C — modifica ed eliminazione sicura
- [ ] Step 6 — Luoghi e funzioni trasversali
  - [ ] **Step 6A — case, stanze e posizione strutturata (corrente)**
  - [ ] Step 6B — storico globale + individuale
  - [ ] Step 6C — contenitori e sotto-posizioni
- [ ] Step 7 — documenti/garanzie, promemoria/scadenze, tag/ricerca globale
- [ ] Modulo vestiti
- [ ] Modulo veicoli
- [ ] Modulo ricette

### Ultimo step funzionale verificato

Lo **Step 5C — Modifica ed eliminazione oggetti è chiuso e verificato**. È
stato mergiato su `main` con CI GitHub Actions verde. Sul Galaxy S9 sono stati
verificati modifica senza duplicati, `/salta`, `/rimuovi`, annullamento
contestuale, eliminazione con conferma, cascade SQLite e rimozione dei media
locali.

Il warning di compatibilità futura relativo a `proc-macro-error2 v2.0.1` non ha
bloccato build o test e resta una nota da rivalutare durante futuri aggiornamenti.

### Passo corrente

È in sviluppo lo **Step 6A — Case, stanze e posizione strutturata**. La scelta
architetturale è stata approvata e usa tre elementi:

```text
abitazioni
   └── stanze

items ── item_luogo ──> abitazione + stanza opzionale
```

Obiettivi dello step:

- più case nello stesso gestionale;
- stanze riconosciute e legate alla propria casa;
- oggetti assegnabili direttamente a una casa o a una stanza;
- spostamento guidato;
- filtri per casa/stanza;
- ricerca anche per nome casa e stanza;
- mantenimento di `oggetti.posizione` come dettaglio libero, senza perdere dati
  già presenti.

La specifica è in `docs/moduli/luoghi.md`. Lo Step 6A non va considerato chiuso
finché migration, test automatici, test runtime sul Galaxy S9 e CI della Pull
Request non sono tutti verdi.

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

## Evoluzione funzionale già approvata

Dopo lo Step 6A la roadmap prevede funzioni trasversali che dovranno essere
riusate dai moduli futuri invece di essere duplicate:

- **Step 6B** — storico globale dell'account + storico individuale di ogni
  entità, con data/ora, prima/dopo e filtri per modulo, casa, stanza, periodo e
  tipo di operazione;
- **Step 6C** — contenitori e sotto-posizioni strutturate;
- **Step 7A** — documenti e garanzie;
- **Step 7B** — promemoria e scadenze;
- **Step 7C** — tag e ricerca globale.

Sono inoltre approvate come direzioni future: manutenzioni, costi e valore,
prestiti, QR code, archivio per elementi venduti/regalati/buttati/persi,
registro acquisti e dashboard/statistiche. La descrizione completa e l'ordine
di sviluppo sono mantenuti in `docs/ROADMAP.md`.

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
│       ├── luoghi.rs
│       ├── vestiti.rs
│       ├── veicoli.rs
│       └── ricette.rs
├── migrations/              # file .sql di modifica schema
├── scripts/                 # avvio e backup
├── docs/
│   ├── HANDOFF.md           # istruzioni complete per continuare
│   ├── schema-core.md
│   ├── ROADMAP.md
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
