# Handoff operativo corrente — 01/09/2026

Questo e' il documento breve da leggere per primo. Dice dove sei davvero, non
dove eri l'ultima volta che qualcuno ha aggiornato la roadmap.

## 1. Punto di ripartenza

Repository: `alessiolari01/gestionale-casa`
**Branch di lavoro: `step-7-alimentazione-s9`.**

Attenzione: il branch `step-7-alimentazione` contiene un 7.3B parallelo scartato
il 31 agosto e **non va usato**. Il motivo e' spiegato nel `CHANGELOG.md`, alla
voce "Due implementazioni parallele".

`main` e' fermo allo **Step 6C del 21 agosto**: tutto lo Step 7 non e' ancora
stato mergiato. La knowledge base che segue `main` mostra quindi uno stato molto
piu' arretrato di quello reale.

## 2. Cosa e' completo

Step 1-6C chiusi e mergiati su `main`. Step 7 sul branch di lavoro:

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

## 3. Stato tecnico verificato

- **42 migration** nel repository;
- applicate al database reale dell'S9 fino a `20260831191500`;
- **`20260901013000_versione_contenuto_ricetta.sql` non e' ancora applicata**:
  lo sara' al primo avvio, dopo il backup che lo script fa da solo;
- pipeline verde: `fmt`, `check --locked`, `clippy --all-targets --locked
  -- -D warnings`, `test --locked` — **226 test**;
- la CI su GitHub Actions e' rossa per un motivo che non si riproduce in locale:
  vedi il punto 6.

Regola invariata: una migration applicata al database reale e' immutabile. Ogni
correzione richiede una nuova migration append-only.

## 4. Come si aggiorna l'S9

Il vecchio giro zip → scp → unzip → installer python non serve piu'. Dal PC si
committa e si pusha; sul telefono:

```bash
cd ~/gestionale-casa
./scripts/aggiorna-s9.sh                 # aggiorna, verifica e avvia
./scripts/aggiorna-s9.sh --solo-controlli   # si ferma prima dell'avvio
```

Lo script rifiuta di partire se sull'S9 ci sono modifiche non committate, imposta
le variabili che evitano l'esaurimento di memoria in fase di link, esegue
l'intera pipeline, fa il backup del database e prova su una copia le sole
migration non ancora applicate, lette da `_sqlx_migrations`. Non va aggiornato a
ogni step.

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

1. **CI rossa.** Codice 101 dopo circa quattro minuti; sull'S9 e in ambiente
   esterno tutti e quattro i passi passano. Ipotesi principale: memoria esaurita
   durante il link del binario di test sul runner, lo stesso problema gia' noto
   sull'S9. Prova da fare: aggiungere `CARGO_PROFILE_TEST_DEBUG: 0` al blocco
   `env:` di `.github/workflows/ci.yml`. `actions/checkout@v7` esiste, quella
   pista e' esclusa.
2. **Pasti liberi** non rappresentabili: `ricetta_nome_snapshot` e' NOT NULL.
   Decisione rimandata ora che esiste l'esito "saltato".
3. **Aritmetica delle date in Rust.** `planner_show_week` esegue una ventina di
   query a SQLite per soli conti di calendario a ogni schermata. Spostarle in
   Rust le azzera.
4. **`main` da riallineare**: e' indietro di tutto lo Step 7.
5. **Verifiche differite** che richiedono un secondo account Telegram: invito
   accettato con apertura dello spazio, notifica al creatore, notifica di cambio
   ruolo, notifica di rimozione con perdita dell'accesso.

## 7. Regole operative

- ogni lavoro passa da un branch pushato **prima** che una seconda sessione ci
  metta mano: uno stato che esiste solo su un dispositivo non e' uno stato
  condiviso, ed e' l'errore che il 31 agosto ha prodotto due 7.3B paralleli;
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
