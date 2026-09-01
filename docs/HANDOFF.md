# Handoff operativo corrente — 01/09/2026 (sera)

Questo e' il documento breve da leggere per primo. Dice dove sei davvero, non
dove eri l'ultima volta che qualcuno ha aggiornato la roadmap.

## 1. Punto di ripartenza

Repository: `alessiolari01/gestionale-casa`

**La PR #9 e' stata mergiata il 1 settembre: `main` contiene tutto lo Step 7**,
fino al planner 7.3B. Il riallineamento di `main`, che era il punto aperto n. 4,
e' chiuso, e la knowledge base che segue `main` mostra di nuovo lo stato reale.

Il nuovo lavoro riparte da `main`. I due branch dello Step 7 sono storia:
`step-7-alimentazione-s9` e' quello mergiato, mentre `step-7-alimentazione`
contiene un 7.3B parallelo scartato il 31 agosto e **non va usato**. Il motivo
e' spiegato nel `CHANGELOG.md`, alla voce "Due implementazioni parallele".

## 2. Cosa e' completo

Step 1-6C e tutto lo Step 7 sono su `main`:

- **7.0-7.1** fondazioni condivise: utenti, spazi, membership, ruoli, vista
  multi-spazio, permessi e audit;
- **7.2A-7.2H** Alimentazione: alimenti, unita', categorie, catalogo e
  compatibilita', prodotti commerciali e nutrizione, ricette con procedimento
  guidato, workflow Miglioramenti con verifica guidata ed export, profili
  alimentari, membri e inviti degli spazi, UI a schermata singola;
- **7.2I** porzioni personali per profilo, override e esclusione del singolo
  ingrediente, calcolo multi-profilo;
- **7.3A** fondazioni del planner: quattro tabelle piu' il dominio degli
  snapshot;
- **7.3B** planner operativo su Telegram: vista settimanale, giorno, aggiunta e
  modifica pasti, scelta ricetta, profili partecipanti, quantita' aggregate,
  completamento con congelamento, esito "saltato", segnalazione della ricetta
  cambiata su settimana, giorno e dettaglio, e riallineamento del pasto alla
  ricetta attuale con conferma esplicita.

La segnalazione riguarda solo i pasti di oggi o futuri: su un pasto passato
riscrivere le quantita' significherebbe riscrivere la storia. L'aggiornamento
non e' mai automatico ed e' sempre limitato al pasto scelto.

La settimana del planner viene creata implicitamente alla prima apertura: non
c'e' una creazione manuale, per non aggiungere concetti all'utente medio.

**Aritmetica delle date in Rust.** I conti di calendario non passano piu' da
SQLite: aprire la schermata settimana costava diciannove query, di cui
diciassette calcolavano soltanto date. Ora ne resta una,
`SELECT date('now','localtime')`, che serve davvero perche' il fuso orario del
telefono lo conosce solo SQLite. `chrono` e' dichiarato come dipendenza diretta
con `default-features = false` e non aggiunge nulla al binario: era gia' nel
grafo tramite `teloxide-core`. Effetto collaterale utile: la validazione delle
date e' diventata semantica, quindi `2026-02-30` viene rifiutata all'ingresso
invece di arrivare a SQLite e diventare `NULL`.

## 3. Stato tecnico verificato

- **42 migration** nel repository, **tutte applicate** al database reale
  dell'S9. Confermato dall'avvio del 1 settembre sera:
  `applied_migrations=42`. Le versioni precedenti di questo file dicevano che
  `20260901013000_versione_contenuto_ricetta.sql` fosse ancora da applicare:
  non era vero, ed e' bastato leggere `_sqlx_migrations` per accorgersene;
- pipeline verde: `fmt`, `check --locked`, `clippy --all-targets --locked
  -- -D warnings`, `test --locked` — **235 test** (erano 226 prima del lavoro
  sulle date: e' il numero da confrontare dopo ogni aggiornamento dell'S9);
- CI su GitHub Actions **verde** dalla run #42, la prima dello Step 7.

Regola invariata: una migration applicata al database reale e' immutabile. Ogni
correzione richiede una nuova migration append-only.

## 4. Le due macchine

Sono due cloni distinti dello stesso repository, con due shell diverse. E'
l'unico punto in cui e' facile sbagliare, e il 1 settembre e' successo: i
comandi di commit sono stati dati sull'S9, dove l'albero era pulito, quindi
`git add` non ha trovato nulla, `git commit` non ha creato nulla e il push ha
pubblicato un ramo vuoto. Il collaudo e' poi girato sul commit sbagliato e
nessuno se ne sarebbe accorto senza guardare il numero dei test.

| | PC | S9 |
|---|---|---|
| shell | PowerShell | bash di Termux |
| percorso | `~\Desktop\Gestionale_Casa_X_AI` | `~/gestionale-casa` |
| cosa ci si fa | l'assistente scrive i file; si committa e si pusha | si aggiorna, si collauda, gira il bot |

**Il PC e' l'unico posto dove esistono le modifiche prima del push.** L'S9 non
le vede finche' non sono su GitHub.

### Sul PC — commit e push

```powershell
cd ~\Desktop\Gestionale_Casa_X_AI
git status --short
```

Attenzione alla sintassi: in PowerShell il `\` a fine riga **non** e' una
continuazione (li' e' il backtick `` ` ``), quindi i comandi lunghi vanno su una
riga sola.

```powershell
git checkout -b <nome-ramo>
git add <elenco dei file su una riga sola>
git commit -m "<messaggio>"
git log --stat -1
git push -u origin <nome-ramo>
```

`git log --stat -1` deve elencare esattamente i file attesi: e' il controllo che
distingue un commit vero da un commit vuoto, e va fatto **prima** del push.

### Sull'S9 — configurazione della macchina, una volta sola

Le impostazioni che proteggono il collegamento stanno nel progetto
(`Cargo.toml` per i profili, `build.rs` per il flag del linker su Android),
quindi valgono anche per un `cargo run` dato a mano. Restano da mettere sull'S9 quelle che dipendono dalla macchina, in
`~/.cargo/config.toml` — **fuori dal repository**, perche' non riguardano il
progetto ma questo telefono:

```toml
[build]
jobs = 1
incremental = false
```

Non impostare `RUSTFLAGS` nell'ambiente: sostituisce la configurazione di cargo
invece di aggiungersi. Lo script avvisa se la trova.

### Sull'S9 — aggiornamento e collaudo

Il vecchio giro zip → scp → unzip → installer python non serve piu'.

```bash
cd ~/gestionale-casa
git pull
./scripts/aggiorna-s9.sh                    # aggiorna, verifica e avvia
./scripts/aggiorna-s9.sh --solo-controlli   # si ferma prima dell'avvio
```

Lo script rifiuta di partire se sull'S9 ci sono modifiche non committate, imposta
le variabili che evitano l'esaurimento di memoria in fase di link, esegue
l'intera pipeline, fa il backup del database e prova su una copia le sole
migration non ancora applicate, lette da `_sqlx_migrations`. Non va aggiornato a
ogni step.

### Come si capisce in un secondo se e' arrivato il codice giusto

Lo script stampa il commit su cui sta girando e il numero dei test. **Il numero
dei test e' la verifica piu' rapida**: se il conteggio e' quello di prima, il
telefono sta collaudando il codice di prima. Il conteggio atteso e' scritto
nella sezione 3 di questo file e va aggiornato a ogni consegna.

## 5. File chiave

```text
src/modules/planner_alimentare.rs   dominio + UI Telegram del planner
src/modules/porzioni.rs             calcolo base/percentuale/override
src/modules/porzioni_profili.rs     porzione per profilo
src/modules/porzioni_ingredienti.rs override del singolo ingrediente
src/modules/profili_alimentari.rs   profili e condivisione
src/modules/spazi_membri.rs         membri, inviti, ruoli
src/context_bot.rs                  schermata singola e `💡 Migliora`
src/main.rs                         routing, sessioni, input inattesi
scripts/aggiorna-s9.sh              aggiornamento e avvio sul telefono
```

## 6. Punti aperti

1. **Toolchain dell'S9 piu' vecchia di quella della CI.** La Clippy del
   telefono non emette lint che il runner invece applica: e' cosi' che
   `drain_collect` e' rimasto invisibile finche' la CI non ha iniziato a girare
   sui branch. Un controllo locale che passa non e' una prova se la toolchain
   non e' la stessa; quando i due esiti divergono, ha ragione la CI. Da
   aggiornare con `rustup update` su Termux.
2. **Pasti liberi** non rappresentabili: `ricetta_nome_snapshot` e' NOT NULL.
   Decisione rimandata ora che esiste l'esito "saltato".
3. **`cargo clean` ogni tanto sull'S9.** `target/` cresce fino a qualche GB e
   lo spazio e' il vincolo vero del telefono. Lo script ora avvisa sotto
   1,5 GB liberi.
4. **Un planner cercato sette volte.** `planner_load_meals` chiama
   `planner_find_for_date` una volta per giorno: aprire una settimana costa
   sette letture identiche. Cercare il planner una volta sola e passarlo al
   ciclo toglie altre sei query. E' il seguito naturale del lavoro sulle date.
5. **PR #6 di Dependabot** (sqlx 0.8.6 → 0.9.0) ancora da valutare.
6. **Verifiche differite** che richiedono un secondo account Telegram: invito
   accettato con apertura dello spazio, notifica al creatore, notifica di cambio
   ruolo, notifica di rimozione con perdita dell'accesso.

## 7. Regole operative

- ogni lavoro passa da un branch pushato **prima** che una seconda sessione ci
  metta mano: uno stato che esiste solo su un dispositivo non e' uno stato
  condiviso, ed e' l'errore che il 31 agosto ha prodotto due 7.3B paralleli;
- **commit e push si danno sul PC, in PowerShell** (`~\Desktop\Gestionale_Casa_X_AI`);
  sull'S9 (`~/gestionale-casa`, bash di Termux) si aggiorna e si collauda e
  basta. Vedi la sezione 4: dare i comandi sulla macchina sbagliata non produce
  un errore, produce un ramo vuoto;
- prima di ogni push, `git log --stat -1` deve elencare i file attesi, e dopo
  ogni collaudo il numero dei test deve essere quello nuovo: sono i due
  controlli che intercettano un commit vuoto;
- nei blocchi shell dell'S9 non usare `set -e`: usare `|| return 1` o `|| exit 1`
  in modo che un errore fermi lo step senza chiudere la sessione SSH;
- niente commit o push se la pipeline fallisce;
- prima di una migration reale: backup, `integrity_check`, `foreign_key_check` e
  prova su copia — lo script lo fa gia';
- Telegram: massimo 5 elementi per pagina, nessun ID tecnico, accenti italiani
  corretti, riga di navigazione `⬅️ Indietro | 💡 Migliora | 🏠 Menù principale`
  dove applicabile, e ogni checklist di collaudo indica il percorso completo dal
  `🏠 Menù principale`;
- non dichiarare fatto cio' che non e' stato realmente verificato.

## 8. Ordine di lettura

1. questo file;
2. `CHANGELOG.md`, la voce piu' recente in testa;
3. `docs/HANDOFF_COMPLETO.md` per le decisioni storiche;
4. `docs/step7/README.md` e `docs/step7/roadmap.md`;
5. `ARCHITETTURA.md`;
6. `docs/step7/modello-condivisione.md` e `docs/moduli/alimentazione/README.md`.

Le sezioni di `HANDOFF_COMPLETO.md` restano intenzionalmente storiche: dove
contraddicono questo file, prevale questo file.
