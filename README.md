# Gestionale Casa

## Step 6C.5 — chiusura finale dello Step 6C

Base di chiusura: `fd4cbea` sul branch `step-6c-test`.

Lo **Step 6C — Contenitori e sotto-posizioni** è funzionalmente completo e verificato sul Galaxy S9:

- gerarchia arbitraria di contenitori con spostamento sicuro del sottoalbero;
- navigazione unificata casa → stanza → contenitore → sottocontenitore;
- creazione contestuale e spostamento oggetti fino a qualunque contenitore;
- storico container-aware con snapshot immutabili del percorso ed eventi padre/figlio;
- migration `20260820230000_storico_contenitori.sql` applicata sul database reale dopo backup;
- **69/69 test**, `cargo check --locked`, Clippy con `-D warnings`, `git diff --check` e prove Telegram runtime superati.

Il 6C.5 è una **chiusura documentale e di rilascio**: non introduce codice applicativo, migration o modifiche ai dati. Il branch è pronto per l'ultima verifica, Pull Request, CI GitHub e merge in `main`.

## Archivio Step 6C.4 — contenitori nello storico (verificato)

Base di partenza del 6C.4: `658e455` (`step-6c-test`), con 6C.3C già verificato e pushato.

Il 6C.4 estende lo storico trasversale fino al livello contenitore:
- `contenitore` diventa un tipo di entità storica con icona `📦`;
- gli snapshot di luogo conservano casa, stanza, contenitore finale e **percorso completo dei contenitori**;
- il percorso viene salvato come snapshot storico, quindi rinomine o eliminazioni future non riscrivono il passato;
- creazione, rinomina, modifica descrizione, spostamento ed eliminazione di un contenitore generano eventi;
- spostamento/eliminazione di un contenitore registra come eventi figli gli effetti sul sottoalbero e sugli oggetti contenuti tramite `evento_padre_id`;
- eliminare una stanza storicizza la promozione dei contenitori e degli oggetti alla casa;
- eliminare una casa conserva nello storico contenitori, percorsi e rimozione del luogo degli oggetti;
- anche eventi ordinari di oggetti e foto conservano il percorso del contenitore corrente.

Nuova migration: `migrations/20260820230000_storico_contenitori.sql`. La migration aggiunge solo campi/snapshot e fa il backfill delle **identità** dei contenitori già presenti: non crea eventi retroattivi.

Stato finale 6C.4: verificato sul Galaxy S9 e pushato come `fd4cbea`; **69/69 test**, Clippy `-D warnings` e runtime Telegram superati.

## Step 6C.3C — spostamento oggetti nei contenitori

Il selettore `🚚 Sposta` percorre ora tutta la gerarchia `casa -> stanza -> contenitore -> sottocontenitore`.

Comportamento previsto:
- la posizione attuale mostra il percorso completo fino al contenitore;
- scelta una casa, si può fermare l'oggetto direttamente nella casa, entrare in una stanza oppure scegliere un contenitore direttamente nella casa;
- scelta una stanza, si può fermare l'oggetto direttamente nella stanza oppure entrare nei suoi contenitori;
- dentro un contenitore si può scegliere `Sposta qui` oppure scendere nei sottocontenitori;
- il ritorno segue il livello gerarchico precedente;
- spostare un oggetto da un contenitore alla stanza/casa azzera correttamente `contenitore_id`;
- scegliere lo stesso contenitore è un no-op;
- nessuna migration: viene usata la struttura `item_luogo.contenitore_id` già introdotta nel 6C.1.

Il 6C.3C manteneva ancora il contesto storico casa/stanza; il successivo 6C.4 ha completato lo storico di contenitori e percorsi.

## Step 6C.3B — rifiniture UX e posizione completa

Il checkpoint di partenza è `413605e` (6C.3A verificato). Il 6C.3B rende coerente l'uso quotidiano dei luoghi:
- `/annulla` esiste durante un'operazione/input e torna al contesto da cui l'azione è partita;
- gli oggetti mostrano il percorso strutturato completo fino al contenitore e il riferimento `/luogo_*` del luogo più specifico;
- ogni contenitore permette di aprire l'elenco degli oggetti direttamente contenuti;
- dopo `Nuovo oggetto qui`, la scheda appena salvata offre `↩️ Torna a <luogo>` verso la casa, stanza o contenitore di partenza;
- l'etichetta della scheda usa `📋 Elenco oggetti` invece del generico `📋 Elenco`;
- oggetti e contenitori hanno simboli distinti: `🏷️` per gli oggetti e `📦` per i contenitori;
- il vecchio campo libero `oggetti.posizione` è legacy: viene preservato per compatibilità ma non è più richiesto nei nuovi flussi.

Le regole sono descritte in `docs/moduli/navigazione-luoghi.md` e `docs/moduli/contenitori.md`. L'infrastruttura PC ↔ S9 ↔ GitHub ↔ Telegram è descritta separatamente in `docs/INFRASTRUTTURA.md`.


## Step 6C — Luoghi gerarchici e navigazione contestuale

Il gestionale tratta **case, stanze e contenitori** come un unico sistema di luoghi. I checkpoint verificati sono `4c64798` (6C.2), `413605e` (6C.3A), `24944ac` (6C.3B), `658e455` (6C.3C) e `fd4cbea` (6C.4).

Regola UI: le azioni dipendono dal luogo visualizzato e ogni schermata interna rilevante offre ritorno al livello logico precedente e `🏠 Menu principale`.

Specifiche: `docs/moduli/navigazione-luoghi.md` e `docs/moduli/contenitori.md`.


## Stato Step 6B

Lo **Step 6B — Storico trasversale globale + individuale** è implementato e verificato sul Galaxy S9. Sono disponibili storico globale e individuale, dettaglio prima/dopo, paginazione e filtri combinabili per periodo, modulo, operazione, casa, stanza ed elemento.

Lo Step 6B è già entrato nella baseline `main`; il suo storico trasversale è stato successivamente esteso dal 6C.4 con contenitori e snapshot dei percorsi.

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
10. **[docs/INFRASTRUTTURA.md](./docs/INFRASTRUTTURA.md)** — collegamenti PC/S9, Tailscale, SSH e GitHub;
11. **[docs/ROADMAP.md](./docs/ROADMAP.md)** — sequenza approvata e future implementazioni.

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
- [x] Step 6 — Luoghi e funzioni trasversali, implementazione verificata
  - [x] Step 6A — case, stanze e posizione strutturata
  - [x] Step 6B — storico globale + individuale
  - [x] Step 6C — contenitori e sotto-posizioni; branch pronto per PR/CI/merge
- [ ] Step 7 — documenti/garanzie, promemoria/scadenze, tag/ricerca globale
- [ ] Modulo vestiti
- [ ] Modulo veicoli
- [ ] Modulo ricette

### Ultimo step funzionale verificato

Lo **Step 6C.4 — Contenitori nello storico** è verificato sul Galaxy S9 e
pushato come `fd4cbea`. La suite corrente è **69/69 test**; `cargo check`,
Clippy `-D warnings`, migration sul database reale e prove Telegram dello
storico container-aware sono verdi.

Il warning di compatibilità futura relativo a `proc-macro-error2 v2.0.1` non ha
bloccato build o test e resta una nota da rivalutare durante futuri aggiornamenti.

### Passo corrente

Lo **Step 6C.5** chiude documentazione e rilascio del 6C. Non aggiunge
funzionalità: dopo l'ultima verifica del branch `step-6c-test` restano Pull
Request, CI GitHub verde e merge in `main`. Solo dopo il merge si apre lo
sviluppo dello **Step 7A — Documenti e garanzie**.

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

## UX Telegram compatta

Le tastiere inline raggruppano le azioni simili, usano `⚙️ Gestisci` per rinomina/modifica/eliminazione e mantengono `🗑 Elimina` isolato. Le azioni frequenti, come lo spostamento degli oggetti, restano direttamente accessibili. I figli gerarchici vengono mostrati prima dei comandi; le righe di creazione usano etichette compatte come `➕🚪 Stanza`, `➕📦 Contenitore`, `➕🏷️ Oggetto`, mentre gli elenchi usano `📋` + simbolo dell'entità.
