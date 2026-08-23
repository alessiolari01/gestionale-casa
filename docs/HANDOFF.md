# Handoff — Gestionale Casa

## Handoff corrente — Step 7.0 / specifica architetturale

Baseline ufficiale: `main @ 219caba` (merge dello Step 6C).

Branch di sviluppo: `step-7-alimentazione`, creato da `219caba` e tracciato su
`origin/step-7-alimentazione`.

Il working tree era pulito all'avvio dello Step 7. Il primo checkpoint è
**docs-only**: consolida fondazioni multiutente, Alimentazione e specifiche
future Acquisti/Viaggi/Spese. Non deve introdurre migration o comportamento
Telegram.

Il vecchio `gestionale_step7_prototipo_bundle` è superato e non va usato.

Documenti da leggere prima di implementare 7.1:

1. `docs/step7/README.md`;
2. `docs/step7/decisioni-architetturali.md`;
3. `docs/step7/modello-condivisione.md`;
4. `docs/step7/storico-e-audit.md`;
5. `docs/step7/database-e-migrazioni.md`;
6. `docs/moduli/alimentazione/README.md`.

Prossimo passo dopo approvazione/commit della documentazione: progettare la
prima migration reale **7.1 — Fondazioni condivise** e provarla sia su DB vuoto
sia su una copia del DB di test Step 6C.

## Archivio handoff Step 6C.4 — storico contenitori/percorso

Base obbligatoria: branch `step-6c-test`, commit `658e455`, working tree pulito.

Il 6C.3 è completo: il 6C.3C è stato verificato con **62/62 test** e runtime Telegram, quindi il lavoro corrente è esclusivamente il 6C.4.

Il 6C.4 introduce `migrations/20260820230000_storico_contenitori.sql` e amplia lo storico senza riscrivere le migration precedenti. I contenitori già presenti ricevono soltanto una riga identitaria in `storico_entita`; non vengono creati eventi retroattivi.

Decisioni da preservare:
- percorso contenitore salvato come snapshot storico testuale, non ricalcolato dal DB vivo quando si legge un vecchio evento;
- ID storico del contenitore finale salvato separatamente;
- modulo storico `luoghi`, componente `contenitori`;
- rinomina contenitore = evento di rinomina, **non** finto spostamento dei discendenti;
- vero spostamento del contenitore = evento principale + eventi figli per discendenti/oggetti il cui percorso cambia;
- eliminazione contenitore = promozione sicura come già previsto dal 6C.1 + storico degli effetti;
- eliminazione stanza/casa non elimina gli oggetti; lo storico conserva i percorsi precedenti;
- nessuna utility di reset/cancellazione globale DB.

Suite verificata dopo l'applicazione: **69/69 test**; `fmt`, `check`, Clippy `-D warnings`, `git diff --check` e prove Telegram dello storico sono stati completati prima del commit `fd4cbea`.

## Handoff Step 6C.3C — spostamento completo oggetti ↔ contenitori

Base di lavoro: snapshot successivo al commit `24944ac`, comprendente le rifiniture UI gerarchiche e il ritorno contestuale dagli elenchi.

Il 6C.3C completa il pezzo rimasto del 6C.3:
- picker gerarchico casa → stanza → contenitore → sottocontenitore;
- posizione corrente completa nel flusso di spostamento;
- destinazione diretta casa/stanza o contenitore;
- ritorni coerenti al livello precedente;
- pulizia di `contenitore_id` quando si risale a stanza/casa;
- no-op sul medesimo contenitore.

Non introduce migration. Lo storico specifico dei contenitori resta nel 6C.4.

La base precedente aveva 58 test; il 6C.3C aggiunge 4 test, quindi dopo l'applicazione sono attesi **62 test** complessivi.

## Handoff Step 6C.3B — rifiniture sopra 413605e

- UX gerarchica compatta: figli prima delle azioni; `➕🚪 Stanza` / `➕📦 Contenitore` / `➕🏷️ Oggetto`; elenchi `📋📦` / `📋🏷️`.

Base stabile: `413605e` — Step 6C.3A verificato e pushato su `step-6c-test`.

Il 6C.3B aggiunge: annullamento contestuale; percorso completo e `/luogo_*` negli oggetti; elenco oggetti diretti nei contenitori; deprecazione UI del vecchio `oggetti.posizione` senza perdita dati; `📋 Elenco oggetti`; ritorno inline al luogo dopo `Nuovo oggetto qui`; simboli distinti `🏷️` oggetto / `📦` contenitore. Non introduce migration.

La prima parte del 6C.3B è stata verificata su S9 con test automatici e prove Telegram. La finalizzazione aggiunge un test unitario per il ritorno post-salvataggio: dopo l'applicazione aspettarsi **55 test** complessivi, poi ripetere runtime Telegram prima del commit.

Prima del commit: `cargo fmt`, `cargo check --locked`, test low-memory S9, Clippy `-D warnings`, `git diff --check` e runtime Telegram.

Resta successivamente da completare il movimento/assegnazione esplicita di oggetti verso contenitori dal selettore di posizione (6C.3 restante), poi storico dei contenitori/percorso (6C.4).

## Infrastruttura operativa corrente

La struttura di comunicazione è ora parte della documentazione del progetto in `docs/INFRASTRUTTURA.md`. In sintesi:

```text
PC Windows -- Tailscale + SSH/SCP --> Galaxy S9 / Termux
     \                                  |
      \------ Git/GitHub --------------+
                                         |
                                         +-- HTTPS long polling --> Telegram
```

Punti da non perdere nel passaggio a una terza persona:

- dal PC l'S9 si raggiunge con l'alias `ssh s9`, senza password, usando una chiave dedicata;
- l'host Tailscale è `galaxy-s9-di-alessio`, quindi il workflow non dipende dall'IP LAN;
- `scp ... s9:~/` è il canale normale per trasferire patch e snapshot tra PC e telefono;
- sull'S9 il remote GitHub è SSH (`git@github.com:alessiolari01/gestionale-casa.git`) e il push non richiede PAT;
- GitHub resta la fonte ufficiale e `main` la baseline stabile;
- Termux:Boot avvia wake lock + `sshd`; dopo un reboot Android può ritardarne l'esecuzione di alcuni minuti, ma una volta partito SSH è disponibile quasi immediatamente;
- nessuna chiave privata, token, password, `.env` reale o database personale deve essere versionato.

Per configurazioni, comandi e diagnostica leggere `docs/INFRASTRUTTURA.md` prima di modificare il workflow.


## Handoff Step 6C — checkpoint 4c64798 e 6C.3A

Branch: `step-6c-test`.

Checkpoint già pushati:
- `cc3ba4c` — 6C.1 backend contenitori;
- `4c64798` — 6C.2 UI contenitori, 47/47 test e runtime verificati.

Lavoro corrente: 6C.3A sopra `4c64798`, senza nuove migration.

Requisiti: navigazione unificata case/stanze/contenitori, elenco globale e albero, `/luogo_h/r/c<ID>`, azioni contestuali, `Nuovo oggetto qui`, destinazioni di spostamento esplicite, `Indietro + Menu principale`.

Prima di commit/push: fmt, check, test low-memory S9, Clippy `-D warnings`, diff check e runtime Telegram.

Passi successivi: completare assegnazione/spostamento oggetti tra contenitori (6C.3), poi storico contenitori/percorso (6C.4). Non introdurre reset/cancellazione globale DB.


## Stato corrente — Step 6B pronto per PR

Lo Step 6B è implementato e verificato sul Galaxy S9 sul branch `step-6b-test`. Checkpoint noti: `a28bdf8` (storico trasversale) e `d106678` (UI Telegram), seguiti dal commit 6B.3B dei filtri globali.

Verifiche finali locali: **37/37 test**, `cargo check --locked` verde, Clippy `-D warnings` verde e runtime Telegram verde.

Non considerare ancora lo step nella baseline ufficiale finché PR, GitHub Actions verde e merge su `main` non sono completati. Dopo il merge riallineare S9 e PC a `main`.

Problema operativo noto S9: il linker LLVM può esaurire memoria; usare se necessario `CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -- --test-threads=1`.

Prossimo step approvato dopo il merge: **6C — Contenitori e sotto-posizioni**.

---

## Aggiornamento corrente — Step 6A in sviluppo

- `main` contiene gli Step 5A, 5B e 5C chiusi e verificati;
- il merge dello Step 5C è su `main` con CI verde;
- lo sviluppo corrente usa il branch `step-6a-test`;
- Step 6A introduce case, stanze e posizione strutturata condivisa;
- nuova migration: `migrations/20260815183000_luoghi.sql`;
- nuovo modulo: `src/modules/luoghi.rs`;
- il modello approvato è `abitazioni` + `stanze` + `item_luogo`;
- `item_luogo` punta a `items`, così la posizione è riutilizzabile da altri
  moduli futuri;
- `oggetti.posizione` non viene eliminata: diventa dettaglio libero della
  posizione e i valori esistenti non vengono reinterpretati automaticamente;
- eliminare una stanza/casa non deve eliminare gli oggetti;
- dopo 6A sono pianificati 6B storico globale/individuale e 6C
  contenitori/sotto-posizioni;
- tutte le future funzioni approvate sono raccolte in `docs/ROADMAP.md`.


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
5. `docs/INFRASTRUTTURA.md` — Tailscale, SSH, SCP, GitHub e diagnostica;
6. `docs/schema-core.md` e i documenti del modulo su cui si deve lavorare.

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

- Step 1→6C, confluiti in `main`;
- baseline `219caba` dopo merge PR Step 6C;
- storico container-aware con 69 test nella baseline conclusiva del 6C.

Step corrente:

- **Step 7.0 — specifica e organizzazione**, branch `step-7-alimentazione`.

Macro-sequenza:

- 7.1 — fondazioni utenti/spazi/condivisione/audit;
- 7.2 — Alimentazione completa;
- 7.3 — integrazioni e condivisione operativa.

Acquisti/prezzi, Viaggi e Spese sono già documentati come RIMANDATI per evitare
incompatibilità architetturali. Documenti/garanzie, ricerca globale, Veicoli,
Vestiti e le altre funzioni storiche restano in roadmap.

Non reintrodurre `native-tls` senza una ragione esplicita e documentata.

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

## 8.2 Accesso locale PC -> Galaxy S9 via SSH

È stato configurato e provato OpenSSH in Termux sulla rete locale. Questo
permette di usare il terminale S9 direttamente dal PC e di trasferire patch con
SCP, evitando commit provvisori usati solo come trasporto.

Forma generale dei comandi:

```text
ssh -p 8022 <utente-termux>@<ip-lan-s9>
scp -P 8022 <file-locale> <utente-termux>@<ip-lan-s9>:~/
```

Regole operative:

- non versionare IP LAN, password o chiavi private;
- SSH/SCP non sostituiscono Git: una modifica verificata deve comunque finire
  in un commit e poi su GitHub;
- non aprire la porta 8022 sul router;
- per accesso fuori dalla LAN si valuterà Tailscale + lo stesso OpenSSH.

## 8.3 Requisiti trasversali già approvati

La multi-abitazione non è più una proposta: è stata introdotta nello Step 6A. Le decisioni
sono:

- più case nello stesso gestionale;
- stanze appartenenti a una casa;
- item assegnabile alla casa o a una stanza;
- spostamento guidato;
- filtri per casa/stanza;
- ricerca anche per nome casa/stanza;
- `oggetti.posizione` mantenuta come dettaglio libero;
- niente cancellazione degli oggetti quando si elimina un luogo.

Le funzioni future devono seguire il principio di riuso: foto, documenti, tag,
reminder, luogo, storico, condivisione e audit non vanno duplicati per ogni
modulo quando possono essere servizi trasversali.

Lo storico Step 6B/6C è già operativo; lo Step 7 deve estenderlo con spazio,
autore e origine dell'azione, mantenendo gli effetti automatici distinguibili e
collegati agli eventi padre.

Per la roadmap completa usare `docs/ROADMAP.md` come fonte aggiornata.

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

## 13. Step corrente — Step 7.0

Il branch `step-7-alimentazione` parte da `219caba` e il primo checkpoint deve
restare documentale.

Obiettivi del 7.0:

1. rendere il README centrale sintetico e collegato ai README dei moduli;
2. documentare utenti/spazi/membership/ruoli;
3. fissare condivisione vs copia e provenienza;
4. documentare storico con autore;
5. consolidare Alimentazione;
6. specificare Acquisti, Viaggi e Spese come moduli futuri;
7. fissare la politica del DB di sviluppo e delle migration.

Decisioni da preservare:

- database centrale, non file SQLite condiviso;
- account Telegram/Google separati dall'utente interno;
- profili alimentari separati dagli account;
- reminder Step 7 solo Telegram/email;
- prezzi futuri: base persistente modificabile + confronto temporaneo volantini;
- Viaggi: checklist generiche modificabili, quantità extra opzionale, più oggetti
  reali collegabili, stato `in viaggio` temporaneo;
- Spese: personali/condivise con ospiti e saldi;
- nessun reset globale nel bot;
- vecchio prototipo Step 7 da ignorare.

Dopo il commit del 7.0 si passa alla progettazione della migration 7.1, senza
modificare le migration Step 2→6C già applicate.

### Regola UX tastiere inline (6C.3B)

La convenzione generale resta: massimo due pulsanti per riga di norma, azioni affini affiancate, `⚙️ Gestisci` per le operazioni amministrative e `🗑 Elimina` isolato. Casa, stanza, contenitore e oggetto seguono la stessa gerarchia; sugli oggetti lo spostamento resta visibile perché considerato frequente.

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
