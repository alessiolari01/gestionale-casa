# Handoff operativo corrente

## 1. Punto di ripartenza

Repository: `alessiolari01/gestionale-casa`
Branch: `step-7-alimentazione`
Baseline committata prima del blocco corrente: `54dc4dd` (`Step 7.2G: completa workflow miglioramenti e coda amministrativa`).

L'handoff esportato dal bot il **27/08/2026 12:26** mostra un working tree con il blocco **7.2G.1→7.2G.6** ancora da committare. Non ricostruire o riscrivere le migration già applicate.

## 2. Stato verificato

Sul Galaxy S9:

```text
cargo fmt --all -- --check                         OK
cargo check --locked                               OK
cargo clippy --all-targets --locked -- -D warnings OK
cargo test --locked -- --test-threads=1            153/153
```

Il warning `proc-macro-error2 v2.0.1` è una future-incompatibility di dipendenza esterna e non ha bloccato la pipeline.

Runtime verificato:

- migration fino a `20260827123000_esporta_miglioramenti_bot.sql` applicate;
- UI Telegram a schermata singola;
- stato UI persistente fra shutdown/restart;
- `⏻ Spegni gestionale` amministrativo;
- contesto `💡 Migliora` corretto anche dal Menù principale;
- export Miglioramenti creato e ricevuto direttamente via Telegram;
- `✅ Ho scaricato il file` elimina realmente lo ZIP temporaneo dall'S9.

## 3. File applicativi modificati nel blocco non committato

```text
src/identity.rs
src/main.rs
src/context_bot.rs                      [nuovo]
src/modules/alimentazione.rs
src/modules/contenitori.rs
src/modules/foto.rs
src/modules/luoghi.rs
src/modules/miglioramenti.rs
src/modules/oggetti.rs
src/modules/ricette.rs
src/modules/storico.rs
scripts/export_miglioramenti.py         [nuovo]
```

Le modifiche più grandi sono in `miglioramenti.rs` e `ricette.rs`; non ridurle a una patch cosmetica senza prima comprenderne stato/sessioni/callback.

## 4. Migration del blocco 7.2G.1→G.6

Tutte append-only, già applicate al database reale e quindi **immutabili**:

```text
20260826123000_miglioramenti_verifica_guidata.sql
20260826223000_miglioramenti_contesto_rifiniture.sql
20260827003000_miglioramenti_ultimo_passaggio.sql
20260827014500_miglioramenti_finalissimi.sql
20260827104500_runtime_ui_persistente.sql
20260827123000_esporta_miglioramenti_bot.sql
```

Regola assoluta: se serve correggere schema/dati dopo l'applicazione, aggiungere una nuova migration. Non modificare questi file.

## 5. Architettura runtime Telegram consolidata

### Schermata singola

`src/context_bot.rs` gestisce un solo messaggio UI principale per chat, ripulisce media temporanei e conserva il contesto per `💡 Migliora`.

`telegram_ui_state` persiste il `message_id` attivo. Lo shutdown lascia una singola schermata offline; il riavvio ripristina/pulisce lo stato precedente prima di presentare la nuova schermata.

### HandlerDependencies

Gli endpoint DPTree hanno firma compatta:

```text
handle_message(Bot, Message, Arc<HandlerDependencies>)
handle_callback(Bot, CallbackQuery, Arc<HandlerDependencies>)
```

Non tornare a iniettare singolarmente tutte le session store: il refactor è stato introdotto per non superare l'arità supportata da `dptree::Injectable`.

### Shutdown

`ShutdownController` usa il `ShutdownToken` Teloxide. `🛠️ Amministrazione → ⏻ Spegni gestionale` è admin-only lato backend e richiede conferma.

## 6. Workflow Miglioramenti corrente

Utente normale:

```text
da_approvare → [admin approva] → da_fare → fatto → verificato → archivio
```

Admin principale:

```text
da_fare → fatto → verificato → archivio
```

Regole:

- `fatto` = implementato ma ancora da collaudare;
- verificato = campi di verifica valorizzati, UI `🧪 Verificato · da archiviare`;
- archiviazione sempre esplicita;
- modificare testo/allegati dopo `fatto`/verifica invalida il risultato e torna `da_fare`;
- liste massimo 5 elementi/pagina;
- ritorni preservano lista e pagina;
- descrizioni possono essere multimessaggio e molto lunghe;
- `scartato` può essere eliminato singolarmente o in blocco con doppia conferma;
- backend admin-only per approvazione, stato, verifica, archivio, eliminazione globale scartati ed export.

Attivi al momento dell'handoff:

- **#7** — eliminazione/reset/revoca account: resta `da_fare`, non implementare incidentalmente;
- **#9** — Zona test/aggiornamenti quasi zero-downtime: requisito futuro da roadmap, non implementare ora.

Tutti gli altri miglioramenti del giro sono archiviati; l'archivio conta **29** elementi, incluso #8 Export Miglioramenti.

## 7. Export Miglioramenti dal bot

Admin:

```text
💡 Miglioramenti
→ 📦 Esporta miglioramenti
→ riceve gestionale-casa_handoff_miglioramenti_YYYYMMDD_HHMMSS.zip
→ scarica
→ ✅ Ho scaricato il file
→ ZIP locale cancellato
```

Implementazione:

- script: `scripts/export_miglioramenti.py`;
- directory temporanea: `data/tmp/miglioramenti_export/`;
- pulizia orfani: 24 ore;
- export in sola lettura;
- include working tree non committato, manifest Git, attivi/archivio, schema e allegati;
- esclude `.env`, token, DB completo, `.git`, `target`, backup e runtime non necessario;
- se l'invio Telegram fallisce, la copia temporanea viene eliminata;
- la cancellazione dopo conferma è limitata con controllo del percorso alla directory export dedicata.

Il vecchio exporter manuale esterno a `~/gestionale-casa` non è più necessario come workflow normale.

## 8. Alimentazione/Ricette dopo le rifiniture

Alimentazione contiene due ingressi principali: **Alimenti** e **Ricette**.

Ricette operative:

- ingredienti sempre legati a `alimento_id`;
- prodotto commerciale opzionale, mai formato di vendita;
- unità proposta dal default alimento ma modificabile prima della quantità;
- procedimento a step con foto/video;
- procedura guidata con messaggio finale esplicito;
- ricerca per nome/categoria/ingredienti;
- categoria nella ricerca ingredienti usata come filtro;
- eliminazione definitiva disponibile oltre all'archiviazione.

Prodotti commerciali:

- prodotto e formati acquistabili separati;
- formati modificabili ed eliminabili;
- ricerca Alimenti trova anche marca/nome commerciale ma restituisce l'alimento generico.

## 9. Regole UI da non regredire

- grafia `Menù principale` con accento;
- nessun ID tecnico visibile;
- massimo 5 elementi per pagina;
- pulsante pagina/totale centrale no-op dove previsto;
- `⬅️ Indietro | 🏠 Menù principale | 💡 Migliora` sulla stessa riga quando possibile;
- `💡 Migliora` deve conoscere la sezione reale e le ultime azioni;
- non mostrare comandi stringa come normale UI;
- autorizzazioni sempre replicate lato backend.

## 10. Database e sicurezza

- DB reale normalmente: `./data/db/gestionale.db` tramite `.env`;
- backup prima di ogni nuova migration reale;
- dopo migration: `PRAGMA integrity_check;` e `PRAGMA foreign_key_check;`;
- `.env`, database reale, token e chiavi non entrano in Git;
- `ALLOWED_CHAT_IDS` è bootstrap/emergenza; l'accesso ordinario è DB-driven;
- approvazione dell'account non concede automaticamente membership o permessi sulle risorse.

## 11. Workflow PC/GitHub/S9

S9 è runtime/test reale; GitHub conserva la storia del progetto. Per sequenze sul telefono usare i flag low-memory già consolidati:

```bash
export CARGO_BUILD_JOBS=1
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_DEV_CODEGEN_UNITS=16
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-C debuginfo=0 -C link-arg=-Wl,--threads=1"
```

Per i test:

```bash
touch src/main.rs
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_PROFILE_TEST_CODEGEN_UNITS=16
cargo test --locked -- --test-threads=1
```

Non usare `set -e` nei blocchi interattivi Termux.

## 12. Prossimo sviluppo dopo il commit documentale

Ordine corrente:

1. profili alimentari separati dagli account Telegram;
2. porzioni personali e override ingrediente;
3. turni/routine;
4. planner pasti;
5. lista della spesa;
6. reminder/export/integrations residue.

Non iniziare dalla Zona test #9: è stata volutamente posizionata come evoluzione infrastrutturale finale.

## 13. Cosa fare prima di proseguire

1. applicare la chiusura documentale;
2. eseguire `git diff --check`;
3. rieseguire la pipeline Rust finale sul working tree completo;
4. controllare `git status` e lo stat;
5. `git add`, commit e push del blocco 7.2G.1→7.2G.6 + documentazione;
6. solo dopo aprire il prossimo step funzionale.
