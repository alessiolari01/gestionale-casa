<!-- CHANGELOG_STEP7_3_20260901 -->
# 01/09/2026 — Step 7.2I, 7.3A e 7.3B: porzioni, planner operativo e riallineamento

**Branch di lavoro: `step-7-alimentazione-s9`.** Il branch `step-7-alimentazione`
contiene un 7.3B parallelo scartato e non va piu' usato: la spiegazione e' nella
sezione "Due implementazioni parallele" qui sotto.

## Porzioni e override — Step 7.2I

- 7.2I.0: fondazioni porzioni e override, con `profilo_ricetta_porzioni` e
  `profilo_ricetta_ingredienti_override`; l'esclusione di un ingrediente resta
  distinta da una quantita' pari a zero;
- 7.2I.1: porzione della ricetta per profilo;
- 7.2I.2: personalizzazione combinata percentuale + override del singolo
  ingrediente, con l'override assoluto che prevale sulla percentuale;
- 7.2I.3: calcolo multi-profilo, con i profilo esclusi mantenuti separati per
  non confonderli con un contributo nullo.

## Planner alimentare — Step 7.3A e 7.3B

- 7.3A: fondazioni persistenti (`planner_alimentari`, `planner_pasti`,
  `planner_pasto_profili`, `planner_pasto_ingredienti_snapshot`) piu' il dominio
  minimo in `planner_alimentare.rs`, senza UI;
- 7.3B: planner operativo su Telegram, sviluppato direttamente sul Galaxy S9.
  Apertura da Alimentazione o con `/planner`, vista settimanale lunedi'-domenica
  con settimana precedente e successiva, dettaglio giornaliero, aggiunta pasto
  con tipo, scelta ricetta paginata a 5 e selezione multipla dei Profili,
  snapshot delle quantita' calcolate con percentuali e override, quantita'
  aggregate nel dettaglio, modifica e rimozione dei pasti pianificati,
  completamento con congelamento e avviso quando la ricetta viva cambia dopo la
  pianificazione. La settimana viene creata implicitamente alla prima apertura:
  non esiste una creazione manuale del planner, per non aggiungere un concetto
  in piu' all'utente medio.
- migration `20260831191500_planner_pasto_saltato.sql`: esito "saltato" con
  `saltato_il`, incompatibile con il completamento, immutabile e non
  eliminabile una volta registrato.

## Rifiniture del planner — segnalazione e aggiornamento della ricetta

Collaudo su Telegram: l'avviso di ricetta cambiata compariva solo aprendo il
singolo pasto. Su una settimana con piu' pasti sarebbe rimasto invisibile, e un
avviso che si vede solo se gia' sai dove guardare non serve.

- **La segnalazione sale alla settimana e al giorno.** La riga del pasto porta
  ora con se' data, esito e versione della ricetta, cosi' le viste possono
  decidere da sole. Nella settimana il giorno interessato mostra `🔄` accanto al
  conteggio; nel giorno ogni pasto ha il proprio simbolo, con le stesse
  convenzioni gia' scelte nel dettaglio: `✅` consumata, `⏭` saltata, `🔄` da
  aggiornare, `○` pianificata. La spiegazione del `🔄` compare solo quando c'e'
  almeno un pasto segnato.
- **Il pasto saltato era indistinguibile.** Nella vista giorno compariva `○`
  come un pasto ancora da consumare, perche' il suo `stato` resta `pianificato`
  e la vista guardava solo quello.
- **Solo i pasti non passati vengono segnalati.** Un pasto con data precedente a
  oggi non mostra piu' il `🔄`: riscriverne le quantita' significherebbe
  riscrivere la storia. Le date sono ISO, quindi il confronto fra stringhe
  coincide con quello del calendario; coperto da test sui cambi di mese e anno.
- **`🔄 Aggiorna alla ricetta attuale`.** Dal dettaglio di un pasto segnalato si
  puo' riallinearlo, con una conferma esplicita che dichiara cosa cambia: le
  quantita' vengono ricalcolate con la ricetta di adesso, i partecipanti e le
  loro percentuali personali restano quelli scelti, gli altri pasti non vengono
  toccati. L'operazione riusa il percorso della modifica gia' esistente, quindi
  il ricalcolo degli snapshot resta scritto in un posto solo. Rifiuta pasti
  consumati, saltati, con data passata o con la ricetta ormai eliminata.
- **Sette query in meno per ogni apertura della settimana.** La vista faceva tre
  interrogazioni per giorno — conteggio, nomi e giorno della settimana; ora la
  lettura dei pasti e' una sola e ne ricava tutto. `planner_count_meals` e
  `planner_meal_names` sono state rimosse.
- Se il database non riesce a dire che giorno e', il codice considera tutti i
  pasti passati e non segnala nulla: meglio un avviso mancante che un avviso
  ovunque.

## Due implementazioni parallele — cosa e' successo

Il 31 agosto il 7.3B e' stato sviluppato due volte in parallelo: una volta sul
Galaxy S9 e una volta in una sessione che leggeva soltanto lo stato pubblicato.
La causa e' stata una somma di disallineamenti: `main` fermo al 6C, la
documentazione ferma al 7.2H e il lavoro del planner presente solo come
modifiche non committate sul telefono.

La versione sviluppata sull'S9 e' stata mantenuta perche' piu' completa e gia'
provata su dati reali. La versione parallela (`src/modules/planner_elenco.rs`,
con planner nominati e periodo scelto a mano) e' stata scartata insieme al suo
branch.

**Regola adottata:** ogni lavoro deve passare da un branch pushato prima che una
seconda sessione ci metta mano. Uno stato che esiste solo su un dispositivo non
e' uno stato condiviso.

## Correzioni di questo blocco

- **Test di navigazione del planner.** `navigazione_globale_ha_indietro_migliora_menu`
  falliva (atteso 3, ottenuto 2). Il codice era corretto e il test sbagliato:
  `💡 Migliora` non lo aggiunge `planner_global_nav`, lo inserisce il ContextBot
  prima di `🏠 Menù principale` quando la riga ha meno di tre pulsanti. Il test
  ora verifica cio' che la funzione deve davvero garantire: due pulsanti con
  `menu:main` in ultima posizione, altrimenti l'inserimento cadrebbe nel punto
  sbagliato.
- **`ricette.aggiornato_il` non era una versione del contenuto.** Veniva scritto
  solo da rinomina, cambio `porzioni_base` e archiviazione; modificare un
  ingrediente non lo toccava e nessun trigger lo faceva. Poiche' il planner
  confronta proprio quel campo per decidere se mostrare l'avviso di ricetta
  cambiata, l'avviso non sarebbe mai comparso nel caso piu' importante, quello
  in cui cambiano le quantita'. La migration
  `20260901013000_versione_contenuto_ricetta.sql` aggiunge i tre trigger su
  `ricetta_ingredienti`. Il procedimento (`ricetta_step`) resta escluso di
  proposito: non cambia le quantita'.
- **Partecipanti storici di un pasto.** `planner_pasto_profili` ha una primary
  key composita con la colonna profilo `ON DELETE SET NULL`, e SQLite ammette
  NULL nelle primary key composite: due profili eliminati avrebbero prodotto due
  righe `(pasto, NULL)` indistinguibili. Aggiunto un indice unico parziale su
  `(pasto_id, profilo_nome_snapshot)` per le sole righe orfane.
- **Lint Clippy in `ricette.rs`**: condizione booleana non minimale nel
  dispatcher delle callback, estratta in una variabile leggibile.

## Infrastruttura

- la CI si attiva ora anche sui branch `step-*`, non piu' solo su `main`: fino al
  31 agosto nessuno dei 22 commit dello Step 7 era mai passato da GitHub Actions;
- aggiunto `scripts/aggiorna-s9.sh`, che sostituisce il giro
  zip → scp → unzip → installer python con un aggiornamento via git. Rifiuta di
  partire se sull'S9 ci sono modifiche non committate, imposta le variabili che
  evitano l'esaurimento di memoria in fase di link, esegue l'intera pipeline,
  fa il backup del database e prova su una copia **le sole migration non ancora
  applicate**, lette da `_sqlx_migrations`, quindi non va aggiornato a ogni step.

## Stato verificato

- migration nel repository: **42**; applicate al database reale dell'S9: fino a
  `20260831191500`. La `20260901013000` e' presente ma **non ancora applicata**;
- `cargo fmt`, `cargo check --locked`, `cargo clippy --all-targets --locked
  -- -D warnings` e `cargo test --locked`: verdi, **226 test**, verificati sia in
  ambiente esterno sia sul Galaxy S9;
- tutte e 42 le migration si applicano in sequenza su un database vuoto, con
  `integrity_check` e `foreign_key_check` puliti.

## Aperti

1. **CI rossa su GitHub Actions**, con codice 101 dopo circa quattro minuti. Non
   si riproduce ne' sull'S9 ne' in ambiente esterno, dove tutti e quattro i passi
   passano. Ipotesi principale: memoria esaurita durante il link del binario di
   test sul runner, lo stesso problema gia' noto sull'S9 e risolto li' con
   `CARGO_BUILD_JOBS=1` e `debuginfo=0`. Da verificare aggiungendo
   `CARGO_PROFILE_TEST_DEBUG: 0` al blocco `env:` del workflow. `actions/checkout@v7`
   e' stato controllato ed esiste, quindi quella pista e' esclusa.
2. **Pasti liberi** ("cena fuori", "avanzi"): non rappresentabili, perche'
   `ricetta_nome_snapshot` e' NOT NULL. Decisione rimandata ora che esiste
   l'esito "saltato", che copre una parte dello stesso bisogno.
3. **Aritmetica delle date in Rust.** `planner_show_week` calcola il calendario
   con query a SQLite: una singola schermata settimanale ne esegue una ventina
   solo per spostare date e ricavare il giorno della settimana. Spostare quei
   conti in Rust le azzera, ed e' coerente con l'obiettivo di ottimizzare su
   hardware limitato.
4. **`main` fermo allo Step 6C** del 21 agosto, e branch `step-7-alimentazione`
   da abbandonare.

<!-- CHANGELOG_STEP7_2H_20260829 -->
# 29/08/2026 — Step 7.2H: Profili, membri/inviti Spazi e chiusura UX

- aggiunte fondazioni `profili_alimentari` + condivisione tramite Spazi;
- resa operativa la UI Profili con creazione, modifica, dettaglio, archiviazione e storico leggibile;
- aggiunta gestione membri degli Spazi e inviti privati Telegram con ruolo, scadenza, calendario, limite utilizzi, revoca e notifiche;
- aggiunte gallette al catalogo alimentare globale con compatibilità prudenziale `verificare`;
- rifinite navigazione Spazi/Miglioramenti, ritorni contestuali, calendario/orari inviti e limiti utilizzi;
- migliorato il workflow `Fatto · da verificare` con piani guidati e verifiche differite per i casi multi-account;
- corretto l'input inatteso fuori dai wizard: la schermata corrente non viene più sostituita e dopo tre tentativi consecutivi viene suggerito `/start`;
- aggiunto `📦 Esporta progetto`, handoff tecnico sanitizzato con `_project_handoff/CURRENT_STATE.md` generato da zero;
- esclusi dall'export `.env`, token, DB, `data/`, `.git/`, `target/`, backup, cache e file tecnici temporanei;
- migration repository portate a 36, ultima `20260829005000_h4e_input_export_progetto.sql`;
- collaudo manuale completato per input inattesi ed export progetto; i casi che richiedono un secondo account restano esplicitamente differiti.

# Changelog

## Step 7.2G.1 → 7.2G.6 — rifinitura Miglioramenti e UI Telegram — 2026-08-26/27

Blocco finale costruito sulla baseline `54dc4dd`, completato e collaudato sul Galaxy S9 prima del commit conclusivo del ramo.

### Miglioramenti e verifica guidata

- reintrodotto lo stato attivo `fatto` come “implementato, da collaudare” e separata la verifica manuale dall'archiviazione;
- stato verificato visualizzato come `🧪 Verificato · da archiviare`;
- modifica del testo/allegati dopo il completamento riporta il miglioramento a `da_fare`;
- piani di verifica e allegati di collaudo salvabili;
- liste Miglioramenti paginate a 5 elementi;
- ritorno al contesto/lista/pagina corretto da dettaglio, modifica e annullamento;
- eliminazione globale degli scartati con doppia conferma;
- descrizioni lunghe e multimessaggio, con lettura paginata;
- utente normale limitato ai propri suggerimenti; azioni di stato/verifica/archivio riservate agli admin.

### `💡 Migliora` contestuale

- pulsante disponibile trasversalmente e, quando possibile, sulla stessa riga di `🏠 Menù principale`;
- contesto con sezione reale, titolo schermata e buffer delle azioni recenti;
- azioni ordinate dalla più recente alla meno recente e descritte con il vero nome del pulsante e la destinazione;
- annullamento della creazione contestuale ritorna alla schermata di origine;
- corretta la grafia globale `Menù principale`.

### UI Telegram a schermata singola e runtime

- introdotto `src/context_bot.rs` come wrapper Telegram per gestione schermata attiva, contesto Migliora e media temporanei;
- persistenza del `message_id` attivo in `telegram_ui_state` per sopravvivere ai riavvii;
- vecchie schermate e callback obsolete non devono produrre azioni duplicate;
- input testuali/media temporanei vengono rimossi dopo acquisizione riuscita nei flussi supportati;
- startup e shutdown amministrativi mantengono una sola schermata coerente online/offline;
- aggiunto `⏻ Spegni gestionale` in Amministrazione con seconda conferma e shutdown Teloxide controllato;
- dipendenze dei dispatcher raggruppate in `Arc<HandlerDependencies>`, eliminando definitivamente il limite di arità `dptree::Injectable` emerso con l'aggiunta dello shutdown controller.

### Alimentazione e Ricette

- eliminazione dei formati di vendita oltre a modifica quantità/unità;
- scelta dell'unità dell'ingrediente prima dell'inserimento quantità, con possibilità di cambiarla rispetto al default dell'alimento;
- menu Alimentazione riorganizzato in `Alimenti` e `Ricette`;
- Ricette: eliminazione definitiva oltre all'archiviazione;
- conclusione della procedura guidata con messaggio esplicito di ricetta terminata;
- ricerca Ricette separata per categorie e per ingredienti;
- nella ricerca per ingredienti la categoria è un **filtro**, non un ingrediente alternativo;
- primo ingrediente digitabile direttamente senza passaggio ridondante “Aggiungi ingrediente”.

### Export amministrativo Miglioramenti

- aggiunto `scripts/export_miglioramenti.py`, parte del repository;
- `📦 Esporta miglioramenti` disponibile all'amministratore principale;
- ZIP con snapshot del working tree, stato Git, attivi/archivio, schema e allegati;
- esclusi `.env`, database reale, `.git`, `target`, backup e runtime non necessario;
- invio diretto del documento via Telegram;
- copia temporanea mantenuta finché l'admin non preme `✅ Ho scaricato il file`;
- cancellazione locale verificata realmente sull'S9;
- pulizia automatica degli export orfani più vecchi di 24 ore.

### Migration append-only del blocco

- `20260826123000_miglioramenti_verifica_guidata.sql`;
- `20260826223000_miglioramenti_contesto_rifiniture.sql`;
- `20260827003000_miglioramenti_ultimo_passaggio.sql`;
- `20260827014500_miglioramenti_finalissimi.sql`;
- `20260827104500_runtime_ui_persistente.sql`;
- `20260827123000_esporta_miglioramenti_bot.sql`.

Tutte le migration sopra risultano applicate al DB reale e sono **immutabili**.

### Verifica finale

- `cargo fmt --all -- --check`: OK;
- `cargo check --locked`: OK;
- `cargo clippy --all-targets --locked -- -D warnings`: OK;
- `cargo test --locked -- --test-threads=1`: **153/153**;
- export #8 collaudato end-to-end dal bot e archiviato;
- attivi rimasti: #7 gestione account (backlog separato) e #9 Zona test/aggiornamenti quasi zero-downtime (futuro infrastrutturale).

> Il warning di future incompatibility di `proc-macro-error2 v2.0.1` proviene da una dipendenza esterna e non ha impedito check, Clippy o test.

---

## Decisione Step 7.2G — workflow Miglioramenti semplificato — 2026-08-26

- consolidato `6449f70` come checkpoint funzionale verificato dello Step 7.2F.1;
- semplificato il workflow futuro di `💡 Miglioramenti`: `🟡 Da approvare`, `🟢 Da fare`, `❌ Scartato`;
- eliminata la distinzione futura fra `verificato` e `pianificato`: un miglioramento approvato entra direttamente in `da_fare`;
- i miglioramenti creati da admin nasceranno `da_fare`, quelli creati da utenti normali `da_approvare`;
- `🆕` viene definito come flag di lettura amministrativo separato dallo stato e verrà applicato anche alle richieste di accesso;
- aprire o decidere un elemento lo marca come letto;
- durante una revisione, i `da_fare` devono essere realizzati direttamente, i `da_approvare` non vanno implementati prima dell'approvazione e gli `scartato` vanno eliminati con i relativi allegati;
- dopo implementazione, test e documentazione, un miglioramento completato viene archiviato e rimosso dall'elenco attivo; gli allegati non più necessari possono essere eliminati all'archiviazione;
- il futuro backfill mapperà gli stati legacy admin `aperto/pianificato` a `da_fare`, `fatto` ad archivio e manterrà temporaneamente `scartato` fino alla prima revisione;
- il prossimo intervento applicativo è Step 7.2G e userà una nuova migration append-only.

## Step 7.2F.1 — Ricette operative con procedimento guidato — 2026-08-25

- attivato il menu `🍳 Ricette` dentro Alimentazione;
- aggiunti elenco paginato, dettaglio, creazione, modifica e archiviazione;
- ingredienti sempre collegati ad `alimenti.id` con prodotto commerciale opzionale;
- il formato di vendita non viene salvato nella ricetta e resta responsabilità della futura Lista spesa;
- aggiunta ricerca per nome e ricerca OR per più ingredienti con ranking per corrispondenze;
- visibilità multi-spazio e collaboratori riusano il modello generico di permessi con backend fail-closed;
- aggiunta migration `20260825231500_ricette_procedimento_guidato.sql`;
- procedimento modellato in step numerati ordinabili;
- ogni step supporta zero o più foto/video locali;
- aggiunte due modalità di consultazione: `📖 Procedimento completo` e `👨‍🍳 Procedura guidata`;
- la modalità guidata mostra un solo step alla volta con precedente/successivo e indicatore `X/Y` no-op;
- il procedimento completo viene spezzato in più messaggi se supera il limite Telegram, senza perdere step;
- i vecchi procedimenti testuali vengono migrati conservativamente nello Step 1;
- aggiunti test di regressione per salvataggio, ricerca OR, prodotto/formati, permessi, riordino/rinumerazione step, callback e testi lunghi.
- confermata la politica di sviluppo: macro-struttura prima, rifiniture UX nel backlog `💡 Miglioramenti`;
- approvato come step successivo il workflow semplificato `da_approvare`/`da_fare` con archivio dei completati e indicatore amministrativo `🆕` separato dallo stato.

**Stato:** verificato su S9; compilazione/avvio e smoke Telegram strutturale completati con esito positivo. Consolidato nel checkpoint `6449f70`.

## Step 7.2E — accesso controllato e Miglioramenti — 2026-08-25

- `ALLOWED_CHAT_IDS` diventa whitelist di bootstrap/emergenza e non più il modello ordinario di autorizzazione;
- gli account Telegram già approvati vengono autorizzati tramite `account_telegram` + `utenti.stato`;
- un account sconosciuto può inviare una richiesta di accesso dal bot;
- introdotto `amministratore_principale`, distinto dal normale ruolo di sistema `admin`;
- solo l'amministratore principale può approvare/rifiutare le richieste;
- l'approvazione crea un utente normale e uno spazio personale senza concedere accesso agli spazi altrui;
- aggiunta la sezione `💡 Miglioramenti` per tutti gli utenti approvati;
- miglioramenti con autore, stato e più screenshot/allegati locali;
- gli admin possono leggere tutti i miglioramenti e cambiarne lo stato;
- introdotta la regola di sviluppo “macro-struttura prima, rifiniture UX nel backlog Miglioramenti”;
- aggiornata la procedura di handoff: una nuova persona può partire semplicemente da `docs/HANDOFF_COMPLETO.md`.

**Stato:** patch da verificare su S9/Termux prima del commit.


## Step 7.2F.0 — prodotti commerciali con più formati

- separata l'identità del prodotto commerciale dalla confezione acquistabile;
- aggiunta `formati_prodotto_alimentare` con quantità, unità, EAN e stato;
- migrati automaticamente i formati già esistenti senza perdere i prodotti;
- un prodotto come `Philadelphia · Original` può ora avere più formati, ad
  esempio 175 g, 200 g e 350 g, senza creare prodotti duplicati;
- barcode/EAN spostato logicamente sul formato;
- aggiunta la vista `v_prodotti_formati_attivi` per future Lista spesa,
  disponibilità e prezzi per punto vendita;
- Ricette continuano a referenziare il prodotto commerciale opzionale e non il
  formato: la scelta della confezione resta responsabilità della futura Lista
  spesa;
- aggiunti elenco, dettaglio, creazione e modifica dei formati nella UI
  Telegram;
- esteso lo storico del prodotto agli eventi `formato_prodotto`;
- `/status` verifica anche la presenza della migration dei formati;
- confermata la navigazione del secondo account approvato anche nel modulo
  Alimentazione dopo il rinforzo dello stack Tokio introdotto durante 7.2E.

## Step 7.2D.0.2–0.3 — prodotti commerciali, paginazione e nutrizione — 2026-08-25

- aggiunta paginazione reale del catalogo alimenti con conteggio totale e pagina X/Y;
- introdotti prodotti commerciali associati agli alimenti generici;
- quantità e unità della confezione sono salvate sul prodotto;
- aggiunto cambio unità durante il wizard prodotto;
- aggiunti valori nutrizionali facoltativi per 100 g / 100 ml;
- predisposto `prodotto_alimentare_id` opzionale negli ingredienti Ricetta mantenendo sempre `alimento_id`;
- vincolo DB che impedisce prodotto e alimento incoerenti nella stessa riga ingrediente;
- database verificato: integrity_check OK, foreign_key_check pulito, 418 alimenti base;
- introdotto `docs/HANDOFF_COMPLETO.md` come documento strutturale permanente da mantenere dopo gli step importanti.

## Step 7.2C — alimenti operativi, fondazioni Ricette e amministrazione — 2026-08-25

### Stato

Checkpoint verificato sul Samsung Galaxy S9 e pronto per il commit.

### Alimentazione

- catalogo alimenti operativo con proprietà reale dell'alimento;
- visibilità su più spazi senza duplicazione del record;
- categorie alimentari e filtro multi-categoria con semantica OR;
- creazione alimento nel flusso nome → unità → categoria → visibilità → salva;
- modifica di nome, unità, categoria, visibilità e collaboratori;
- unità obbligatorie e mostrate in forma leggibile, ad esempio `grammi (g)`;
- accenti italiani corretti nelle stringhe UI interessate;
- liste sintetiche e dettagli separati;
- rimossi gli ID tecnici dalle schermate utente interessate.

### Permessi condivisi

- introdotte `inviti_risorsa` e `permessi_risorsa`;
- distinzione fra visibilità, modifica e gestione dei permessi;
- backend fail-closed: nascondere un pulsante non costituisce autorizzazione;
- fondazione riutilizzabile da alimenti, ricette e future risorse condivisibili.

### Ricette — fondazioni

- introdotta la migration delle fondazioni Ricette;
- ingredienti predisposti per referenziare direttamente `alimenti.id`;
- predisposta la ricerca per ingredienti con conteggio delle corrispondenze;
- nessuna duplicazione testuale degli alimenti;
- UI Telegram completa delle Ricette rimandata allo Step 7.2D.

### Pulizia UI trasversale

- rimossi dalla UI ID come `#12`, `Casa #3`, `Oggetto #4` ed `Evento #7`;
- gli ID restano normalmente usati internamente in database, callback e query;
- ripuliti Oggetti, Luoghi, Contenitori, Foto e Storico;
- la struttura Luoghi espone comandi leggibili come `/stanza_camera` e
  `/contenitore_scatola_attrezzi`;
- in caso di nomi duplicati viene aggiunto progressivamente contesto umano,
  senza esporre l'ID tecnico;
- rimossa dal menu principale la riga dei “Comandi rapidi”; i comandi testuali
  restano disponibili in parallelo ai pulsanti.

### Ruoli di sistema e amministrazione

- introdotto `ruolo_sistema` indipendente dai ruoli negli spazi e dai permessi
  sulle singole risorse;
- ruoli iniziali: `utente` e `admin`;
- il primo utente/bootstrap è amministratore;
- gli utenti normali non vedono funzioni tecniche;
- l'amministratore dispone di `🛠️ Amministrazione`;
- area amministrativa con panoramica, stato sistema ed elenco utenti;
- `/admin`, `/status` e callback amministrative sono protetti anche lato backend;
- notifiche online/offline riservate agli amministratori;
- il ruolo admin non concede automaticamente proprietà o permessi sulle risorse.

### Verifiche

- `cargo fmt --all -- --check`: OK;
- `cargo check --locked`: OK;
- `cargo clippy --all-targets --locked -- -D warnings`: OK;
- `cargo test --locked -- --test-threads=1`: **109/109** test superati;
- smoke test Telegram di Alimentazione, pulizia UI, comandi Luoghi e area
  amministrativa: completato con esito positivo;
- `PRAGMA integrity_check`: `ok`;
- `PRAGMA foreign_key_check`: nessun errore;
- migration `20260825003000_ruoli_sistema_amministrazione.sql`: applicata con successo;
- utente bootstrap verificato con `ruolo_sistema = admin`.

### Requisito futuro già approvato

L'attuale whitelist statica Telegram dovrà essere sostituita come meccanismo
ordinario da un flusso di ammissione applicativo:

1. qualsiasi account Telegram può contattare il bot;
2. un account non autorizzato può soltanto richiedere l'accesso;
3. la richiesta arriva all'amministratore principale;
4. l'amministratore può accettarla o rifiutarla dalla propria area;
5. dopo l'accettazione viene creato/attivato un normale utente del gestionale;
6. l'accesso al bot non concede automaticamente accesso a spazi o risorse.

La whitelist configurata potrà restare come meccanismo bootstrap/emergenza,
ma non dovrà rappresentare il modello applicativo definitivo.

### Prossimo step

**Step 7.2D — Ricette operative su Telegram.**

## Step 7.1B — vista multi-spazio e proprietà separata dalla posizione — 2026-08-23

**Stato: IN SVILUPPO — da verificare su Galaxy S9 prima del commit.**

- aggiunta la migration `20260823200000_vista_multispazio_condivisione.sql`;
- lo spazio attivo diventa lo **spazio predefinito** per la creazione, non l'unico contesto consultabile;
- aggiunte le modalità `🎯 Solo spazio predefinito` e `🌐 Tutti i miei spazi`;
- aggiunti comandi `/vista_spazio` e `/vista_tutti` e relativi pulsanti inline;
- ripristinati i flussi inline `➕ Nuovo spazio` e `✏️ Rinomina spazio`, con navigazione verso Profilo/Spazi/Menu;
- oggetti, luoghi, contenitori, foto e storico possono leggere tutti e soli gli spazi di cui l'utente è membro quando la vista globale è attiva;
- `items.spazio_id` resta lo spazio proprietario dell'item; la posizione fisica può appartenere a un altro spazio accessibile;
- uno spostamento cross-space di un oggetto richiede permessi di scrittura sia sullo spazio proprietario sia sulla destinazione;
- i contenitori restano legati allo spazio della casa e non possono essere trasferiti fra spazi diversi;
- aggiunta `item_condivisioni` come fondazione trasversale per condividere in futuro item/ricette con permesso `lettura` o `modifica` senza duplicarli;
- lo storico conserva lo spazio proprietario dell'entità anche quando il contesto fisico dell'evento è in un altro spazio;
- aggiunti test per persistenza della vista e per il caso oggetto personale → casa condivisa;
- disambiguazione UI dei luoghi omonimi: nella vista multi-spazio case/percorsi mostrano anche lo spazio (`Casa principale · Spazio`), e i messaggi di assegnazione/spostamento mostrano sempre lo spazio della posizione per evitare ambiguità.
- dettaglio storico multi-spazio: lo spazio proprietario dell'entità resta distinto dallo spazio della posizione e i cambi luogo mostrano `Da`/`A` con lo spazio; gli eventi esistenti vengono backfillati dalla relativa identità storica della casa.
# Diario di sviluppo

<!-- STEP_7_2G_CHIUSURA_DOCS -->
## Step 7.2G — workflow Miglioramenti e coda amministrativa — 2026-08-26

### Obiettivo

Lo Step 7.2G rende operativo il workflow semplificato dei miglioramenti definito
nel checkpoint documentale `ccb110a` (`Step 7.2G.0: definisce workflow
miglioramenti semplificato`), mantenendo separati stato operativo e stato di
lettura amministrativa.

### Implementazione

- nuova migration append-only
  `20260826024500_miglioramenti_workflow_admin.sql`;
- le migration precedenti restano immutabili;
- stati attivi dei miglioramenti:
  - `da_approvare`;
  - `da_fare`;
  - `scartato`;
- `letto_admin_il` gestisce il flag amministrativo `🆕` senza modificare lo
  stato operativo;
- miglioramento creato da admin → `da_fare`, già letto;
- miglioramento creato da utente normale → `da_approvare`, non letto;
- apertura del dettaglio admin → marca letto senza cambiare lo stato;
- approvazione → `da_approvare -> da_fare`;
- scarto → `scartato`;
- completamento → spostamento in `miglioramenti_archivio` e rimozione dal
  backlog attivo;
- gli allegati dei completati vengono conservati in
  `miglioramento_archivio_allegati`;
- gli elementi legacy `fatto` vengono archiviati durante la migration;
- gli elementi legacy admin aperti/pianificati diventano `da_fare`;
- gli elementi legacy non-admin aperti/pianificati diventano `da_approvare`;
- gli elementi legacy `scartato` restano scartati e risultano già letti;
- `richieste_accesso.letto_admin_il` applica lo stesso concetto `🆕` alle
  richieste di accesso;
- le richieste di accesso già approvate/rifiutate prima della migration vengono
  considerate già lette;
- eliminando uno `scartato` vengono eliminate anche le righe allegato e il
  backend tenta la rimozione dei file fisici.

File applicativi modificati:

- `src/modules/miglioramenti.rs`;
- `src/access_control.rs`;
- `src/main.rs`.

### Verifiche sul Galaxy S9 / Termux

Pipeline completata con esito positivo:

- `cargo fmt --all`;
- `cargo fmt --all -- --check`;
- `cargo check --locked`;
- `git diff --check`;
- `cargo clippy --all-targets --locked -- -D warnings`;
- `cargo test --locked -- --test-threads=1` → **142 passed, 0 failed**.

Database reale:

- backup pre-migration creato:
  `~/gestionale_pre_step7_2g_20260826_030715.db`;
- migration `20260826024500` → `success = 1`;
- `PRAGMA integrity_check` → `ok`;
- `PRAGMA foreign_key_check` → nessuna riga;
- dopo il backfill osservati `9` miglioramenti `da_fare` e `4` `scartato`.

Archivio verificato anche direttamente sul DB:

- il miglioramento di prova `prova` è stato completato;
- è stato creato `miglioramenti_archivio.id = 1`;
- `miglioramento_origine_id = 14`;
- l'elemento non è più presente nel backlog attivo;
- integrity e foreign key sono rimasti corretti.

Il warning di future incompatibility di `proc-macro-error2 v2.0.1` resta noto,
proviene da una dipendenza esterna e non blocca il checkpoint.

### Smoke Telegram

Le funzioni verificabili con il solo account amministratore sono state
dichiarate funzionanti dall'utente; il passaggio completamento → archivio è
stato inoltre verificato direttamente nel database.

Resta intenzionalmente **pendente** lo smoke manuale multi-account, da eseguire
quando sarà disponibile un secondo account:

1. utente normale crea un miglioramento → `da_approvare`;
2. comparsa `🆕` lato admin;
3. apertura dettaglio → rimozione `🆕` senza cambio stato;
4. approvazione → `da_fare`;
5. scarto/eliminazione;
6. nuova richiesta di accesso → `🆕`;
7. apertura/decisione richiesta → rimozione del flag.

Questa verifica pendente non invalida i test automatici né le verifiche DB già
completate, ma deve restare documentata finché non viene eseguita live.

### Stato del checkpoint

Lo Step **7.2G è pronto per commit/push** sul branch
`step-7-alimentazione`. Dopo il consolidamento non riaprire o riscrivere la
migration `20260826024500_miglioramenti_workflow_admin.sql`.

Il prossimo sviluppo deve riprendere dal prossimo elemento già previsto nella
roadmap corrente dello Step 7.2; non introdurre un nuovo sottostep numerato
senza prima rileggere `docs/step7/roadmap.md` e l'handoff aggiornato.

---


## Step 7.1 — spazi operativi e isolamento reale — 2026-08-23

### Implementazione

- nuova migration `20260823174500_spazi_operativi.sql`;
- migration resa compatibile con SQLx 0.8.6/SQLite senza transazioni annidate e senza disabilitare le foreign key;
- `abitazioni.nome` e `tag.nome` diventano unici per `spazio_id`, non globalmente;
- `/spazi`, `/spazio_nuovo <nome>` e `/spazio_rinomina <nome>`;
- cambio spazio tramite pulsanti inline;
- nuovi utenti successivi al bootstrap ricevono uno spazio personale proprio;
- oggetti, luoghi, contenitori, foto e storico filtrano lo spazio attivo;
- i flussi temporanei vengono cancellati al cambio spazio;
- scritture principali protette dai ruoli (`lettura` non può modificare);
- `/status` espone `Isolamento multi-spazio`;
- test aggiunti per oggetti, case, contenitori, foto e storico cross-space, incluse mutazioni dirette per ID;
- test CRUD reale del ruolo `lettura`;
- la rimozione della membership attiva riallinea automaticamente `preferenze_utente`;
- la risoluzione Telegram ricontrolla sempre che lo spazio attivo sia ancora una membership valida;
- in produzione un accesso space-aware senza `AuditActor` fallisce invece di ricadere silenziosamente nello spazio `#1`.

### Sicurezza e compatibilità

- i dati preesistenti restano nello spazio bootstrap `#1`;
- nessun dato viene copiato o spostato automaticamente fra spazi;
- conoscere un ID di un altro spazio non deve renderlo accessibile;
- inviti e gestione completa dei membri restano nel seguito della 7.1;
- nessuna funzione di reset globale viene aggiunta.

## Step 7.1 — fondazioni condivise, primo checkpoint tecnico — 2026-08-23

### Stato precedente

- Step 7.0 documentale chiuso e pushato come `135dd33`;
- branch `step-7-alimentazione` pulito e allineato al remoto;
- runtime e schema ancora Step 6C;
- DB di sviluppo Step 6C disponibile come banco di prova.

### Implementazione predisposta

- nuova migration `20260823153000_fondazioni_condivise.sql`;
- tabelle `utenti`, `spazi`, `membri_spazio`, `account_telegram`, `preferenze_utente`, `inviti_spazio`;
- spazio bootstrap `#1` per preservare tutti i dati esistenti;
- `spazio_id` su `items`, `abitazioni`, `tag`, `storico_entita`, `storico_eventi`;
- trigger di validazione spazio e blocco cross-space item/casa e item/tag;
- `src/identity.rs` per risolvere Telegram → utente interno e installare il contesto audit task-local;
- primo account autorizzato proprietario del bootstrap, successivi amministratori durante la fase di compatibilità;
- `/profilo` e pulsante `👤 Profilo e spazio`;
- storico esteso con autore, origine, spazio e flag automatico;
- eventi legacy senza autore inventato;
- `/status` esteso con verifica delle fondazioni condivise.

### Verifiche già effettuate fuori dal runtime Rust

- migration SQL applicata da zero su SQLite: `integrity_check = ok`, `foreign_key_check = 0`;
- migration applicata su copia di `gestionale_step7_base.db`: dati Step 6 conservati, 45 eventi storici conservati, tutti assegnati allo spazio #1, nessun autore retroattivo inventato;
- trigger cross-space verificati sul modello SQL.

### Verifiche ancora necessarie prima del commit

- `cargo fmt --all`;
- `cargo fmt --all -- --check`;
- `cargo check --locked`;
- `cargo test --locked` con profilo low-memory sull'S9;
- `cargo clippy --all-targets --locked -- -D warnings`;
- `git diff --check`;
- runtime Telegram: `/profilo`, creazione/modifica/spostamento oggetto e controllo autore nello storico.

### Limite transitorio intenzionale

La UI non consente ancora di creare/cambiare spazio. Le query Step 6 non sono ancora tutte space-aware e continuano a operare nello spazio #1. Questo evita di esporre multi-spazio prima dell'isolamento completo.

## Step 7.0 — specifica e organizzazione — 2026-08-23

### Stato precedente

- Step 6C chiuso e mergiato in `main` con baseline `219caba`;
- branch `step-7-alimentazione` pulito e allineato a `origin/step-7-alimentazione`;
- schema runtime ancora quello Step 6C;
- esiste un DB di sviluppo con dati di prova utile per verificare le future migration;
- un precedente `gestionale_step7_prototipo_bundle` viene dichiarato superato.

### Decisioni consolidate

- Step 7 ridefinito come **Fondazioni condivise + Alimentazione**;
- tre macro-fasi 7.1/7.2/7.3, precedute dal checkpoint docs-only 7.0;
- utenti interni separati da Telegram/Google;
- spazi personali/familiari/condivisi;
- condivisione distinta dalla copia indipendente;
- storico multiutente con autore e distinzione degli effetti automatici;
- Alimentazione strutturata: alimenti, unità, ricette, profili/porzioni,
  turni/routine, planner, spesa, reminder ed export;
- reminder Step 7 via Telegram/email, SMS esclusi;
- Acquisti/prezzi specificato come modulo futuro con prezzo base modificabile,
  prezzo confezione + normalizzato e volantini solo per confronto;
- Viaggi specificato con bagagli reali, checklist generiche modificabili,
  quantità extra opzionale, più oggetti reali per voce, stato temporaneo
  `in viaggio` e controllo rientro;
- Spese specificato come modulo generale personale/condiviso con ospiti,
  divisioni personalizzate, saldi e rimborsi;
- nessun reset globale nel bot; il DB di sviluppo può essere azzerato
  manualmente solo prima del go-live dopo backup.

### Modifiche documentali

- creato `docs/step7/` come indice e specifica architetturale dello step;
- creato `docs/moduli/alimentazione/` con documentazione dettagliata;
- create specifiche future `docs/moduli/acquisti/`, `viaggi/` e `spese/`;
- aggiornati README centrale, Architettura, Roadmap, Handoff e indice moduli;
- il README centrale rimanda esplicitamente al README Alimentazione invece di
  duplicarne tutti i dettagli.

### Verifiche previste per chiudere 7.0

- `git diff --check`;
- revisione del diff documentale;
- nessun file Rust/migration modificato;
- commit/push sul branch `step-7-alimentazione`.

### Prossimo passo

**Step 7.1 — Fondazioni condivise**: progettare e implementare la prima migration
utenti/spazi/audit, testandola da zero e su una copia del DB Step 6C.

## Step 6C.5 — chiusura documentale e preparazione PR — 2026-08-22

- checkpoint di partenza: `fd4cbea` (`Step 6C.4: integra contenitori nello storico`);
- 6C.4 verificato su Galaxy S9 con **69/69 test**, `cargo check --locked`, Clippy `-D warnings`, `git diff --check` e runtime Telegram;
- migration `20260820230000_storico_contenitori.sql` applicata al database reale dopo backup senza reset o perdita dati;
- verificati su Telegram: percorsi contenitore prima/dopo, riparentamento nella stessa stanza, eventi padre/figlio del sottoalbero, rinomina senza falso spostamento, contesto contenitore sugli oggetti e filtro per entità contenitore;
- il 6C.5 aggiorna soltanto documentazione/stato di progetto: **nessuna nuova migration e nessuna modifica applicativa**;
- stato finale locale: Step 6C funzionalmente completo; resta la chiusura di rilascio tramite PR, CI GitHub verde e merge `step-6c-test -> main`.


## Step 6C.4 — contenitori nello storico — 2026-08-20 — verificato

- aggiunta la migration `20260820230000_storico_contenitori.sql`;
- estesi `storico_eventi` e `storico_cambi_luogo` con identità e percorso snapshot del contenitore;
- backfill dei contenitori esistenti in `storico_entita` senza creare eventi retroattivi;
- aggiunti eventi per creazione, rinomina, modifica descrizione, spostamento ed eliminazione dei contenitori;
- gli spostamenti di sottoalberi e le promozioni dopo eliminazione generano eventi figli per contenitori e oggetti coinvolti;
- `evento_padre_id` collega gli effetti automatici all'azione principale;
- eliminazione stanza/casa conserva i percorsi prima dell'operazione e storicizza gli effetti su contenitori/oggetti;
- eventi oggetto/foto conservano ora anche il contesto contenitore;
- aggiunta icona storico `📦` e visualizzazione del percorso completo nel contesto e nel prima/dopo;
- nessun reset, nessuna cancellazione globale e nessun evento storico inventato per dati già esistenti;
- aggiunti 7 test: attesi **69 test totali** dopo l'applicazione.

Verifica completata sul Galaxy S9: **69/69 test**, Clippy `-D warnings` e runtime Telegram superati; commit `fd4cbea` pushato su `step-6c-test`.

## Step 6C.3C — spostamento oggetti nei contenitori — 2026-08-20

- completato il picker gerarchico di destinazione per gli oggetti: casa → stanza → contenitore → sottocontenitore;
- la schermata di spostamento mostra ora il percorso corrente completo, incluso il contenitore;
- aggiunte destinazioni dirette casa/stanza e navigazione nei contenitori;
- aggiunto lo spostamento esplicito stanza → contenitore, contenitore → contenitore/sottocontenitore, contenitore → stanza e contenitore → casa;
- `set_item_home` e `set_item_room` azzerano `contenitore_id`, evitando posizioni incoerenti;
- lo stesso contenitore viene riconosciuto come no-op;
- aggiunti test per spostamenti, azzeramento del contenitore, percorso completo e limite callback Telegram;
- nessuna migration e nessuna cancellazione dati;
- storico contenitore/percorso rimandato al 6C.4.

## Step 6C.3B — rifiniture UX e posizione completa — 2026-08-20

- rifinita la gerarchia visiva delle tastiere: figli immediati prima delle azioni, casa con `➕🚪 Stanza` · `➕📦 Contenitore` · `➕🏷️ Oggetto` sulla stessa riga e pulsanti elenco compatti `📋📦 ... qui` / `📋🏷️ ... qui`;

- `/annulla` ritorna al contesto di partenza per creazione/rinomina di case, stanze, contenitori e per la creazione/modifica oggetti.
- Elenchi, ricerca e dettaglio oggetto mostrano il percorso completo fino al contenitore e `/luogo_*` del luogo più specifico.
- La scheda contenitore espone `Oggetti in questo contenitore` con elenco degli oggetti diretti.
- Dopo la creazione contestuale, la scheda dell'oggetto offre `↩️ Torna a <luogo>` verso la casa/stanza/contenitore da cui è stato avviato `Nuovo oggetto qui`.
- La scheda oggetto usa `📋 Elenco oggetti`; oggetti e contenitori sono visivamente distinti con `🏷️` e `📦`.
- Le tastiere inline adottano una gerarchia compatta: azioni simili affiancate, `⚙️ Gestisci` per le operazioni amministrative e `🗑 Elimina` isolato nelle schermate di gestione.
- Rimosso dai nuovi flussi il passaggio `Dettaglio posizione`: la posizione operativa è strutturata.
- `oggetti.posizione` resta nel DB e nella ricerca come dato legacy, senza cancellazioni o migration distruttive.
- Aggiunto `docs/INFRASTRUTTURA.md` con topologia PC ↔ S9 ↔ GitHub ↔ Telegram, Tailscale, SSH/SCP senza password, GitHub SSH senza PAT, Termux:Boot e diagnostica.
- Nessuna nuova migration.


## Step 6C.1–6C.3A — Contenitori e navigazione dei luoghi — 2026-08-17 → 2026-08-19

- 6C.1 (`cc3ba4c`): backend contenitori gerarchici.
- 6C.2 (`4c64798`): UI Telegram contenitori; 47/47 test e runtime S9 verificati.
- 6C.3A: sezione unificata `Case, stanze e contenitori`, elenchi globali, albero, `/luogo_*`, azioni contestuali e `Nuovo oggetto qui` con posizione strutturata precompilata.
- Spostamento: destinazioni esplicite (`Sposta in Camera`, `Sposta in Casa principale`) al posto di `Livello principale`.
- Contratto UI: `Indietro` semantico + accesso diretto al menu principale.

6C.3A non introduce migration; è stato verificato su S9 e consolidato nel checkpoint `413605e`.


## Step 6B — Storico trasversale globale + individuale — 2026-08-15 → 2026-08-16

**Implementazione completata e verificata sul Galaxy S9; PR/CI/merge ancora necessari per la chiusura ufficiale.**

Introdotte le tabelle `storico_entita`, `storico_eventi`, `storico_cambiamenti` e `storico_cambi_luogo`, con identità storiche permanenti, prima/dopo strutturato e snapshot dei luoghi. Il backfill non inventa eventi precedenti.

Coperti gli eventi di oggetti, foto, case, stanze e luoghi; le modifiche no-op non generano eventi.

6B.3A ha aggiunto storico globale/individuale, paginazione e dettaglio Telegram (`d106678`). 6B.3B ha aggiunto filtri combinabili per periodo, modulo, operazione, casa, stanza ed elemento, mantenuti durante paginazione e dettaglio.

Verifiche finali: `cargo fmt`, `cargo check --locked`, **37/37 test**, Clippy `-D warnings` e runtime Telegram tutti verdi. Sul Galaxy S9, se il linker esaurisce memoria, usare `CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -- --test-threads=1`.

Prossimo passo dopo PR/CI/merge: **Step 6C — Contenitori e sotto-posizioni**.

---

## Step 6A — Case, stanze e posizione strutturata — 2026-08-15

- UX luoghi: distingue prima assegnazione, spostamento e rimozione; gli spostamenti mostrano origine e destinazione in preparazione allo storico dello Step 6B.
- UX spostamento: la stanza (o la sola casa) già occupata dall'oggetto è marcata come `Attualmente qui` direttamente nel selettore di destinazione.
- UX creazione oggetto: la posizione diventa un flusso guidato unico `Casa -> Stanza -> Dettaglio`; saltando la casa si salta automaticamente anche la stanza. Casa/stanza vengono salvate nella stessa transazione della nuova scheda.

### Stato precedente

Gli Step 5A, 5B e 5C sono chiusi e verificati. Lo Step 5C è stato mergiato su
`main` con CI verde dopo i test runtime sul Galaxy S9. Il modello precedente
aveva solo `oggetti.posizione` come stringa libera e non riconosceva case o
stanze come entità.

### Decisione architetturale approvata

Lo Step 6A usa:

```text
abitazioni
└── stanze

items ── item_luogo ──> abitazione + stanza opzionale
```

La relazione viene collegata a `items` e non direttamente a `oggetti`, così il
sistema di luoghi potrà essere riusato da Vestiti, Veicoli e altri moduli.

Il vecchio `oggetti.posizione` resta disponibile e assume il significato di
dettaglio libero, per esempio `scaffale 2`. Nessun dato esistente viene
interpretato automaticamente come casa o stanza.

### Implementazione predisposta per test

- nuova migration `20260815183000_luoghi.sql`;
- nuove tabelle `abitazioni`, `stanze` e `item_luogo`;
- vincolo DB che impedisce di associare una stanza a una casa diversa;
- eliminazione stanza: item conservato nella casa con `stanza_id = NULL`;
- eliminazione casa: relazione di luogo rimossa, item conservato;
- nuovo modulo `src/modules/luoghi.rs`;
- menu Telegram `🏠 Case e stanze`;
- creazione, elenco, dettaglio, rinomina ed eliminazione con conferma per case e
  stanze;
- comandi testuali equivalenti `/luoghi`, `/case`, `/casa_*`, `/stanza_*`;
- scheda oggetto con `🏠 Casa / stanza`;
- assegnazione alla sola casa, a una stanza, spostamento e rimozione luogo;
- comandi `/oggetto_luogo <id>` e `/oggetto_sposta <id>`;
- filtro degli oggetti dalla scheda casa/stanza;
- ricerca oggetti estesa a nome casa e nome stanza;
- elenco/scheda oggetto mostrano separatamente luogo strutturato e dettaglio
  libero;
- durante la creazione di un nuovo oggetto il pannello `🏠 Posizione` guida in sequenza casa, stanza e dettaglio; la modifica di un oggetto esistente continua invece a usare `🚚 Sposta oggetto` per casa/stanza, mantenendo espliciti gli spostamenti;
- documentazione del punto attuale e della roadmap futura aggiornata, incluse le
  decisioni su storico globale/individuale, contenitori, documenti, garanzie,
  promemoria, tag, ricerca globale, manutenzioni, costi, prestiti, QR, archivio,
  registro acquisti e dashboard.

### Verifiche da eseguire prima della chiusura

1. `cargo fmt --all -- --check`;
2. `cargo check --locked`;
3. `cargo test --locked`;
4. `cargo clippy --all-targets --locked -- -D warnings`;
5. test runtime su due case e più stanze;
6. rinomina casa/stanza;
7. assegnazione oggetto a casa, stanza e spostamento fra case;
8. filtro e ricerca per casa/stanza;
9. rimozione del luogo;
10. eliminazione stanza con oggetto collegato, verificando che l'oggetto resti
    nella casa;
11. eliminazione casa con oggetto collegato, verificando che l'oggetto resti
    senza luogo;
12. persistenza dopo riavvio;
13. Pull Request e CI GitHub Actions verdi.

### Prossimo passo previsto

Dopo la chiusura del 6A: **Step 6B — storico globale + storico individuale**.
Lo storico dovrà registrare eventi strutturati con data/ora, valori prima/dopo e
filtri per modulo, casa, stanza, periodo e operazione.

## Step 5C — Modifica ed eliminazione oggetti — 2026-08-15

### Stato precedente

Gli Step 5A e 5B sono chiusi. Lo Step 5B è stato verificato sul Galaxy S9 con
11 test automatici, Clippy, CI GitHub Actions verde, salvataggio reale delle
foto sul filesystem e persistenza dopo riavvio. `main` contiene anche la
notifica automatica di avvio e il ritorno al menu da `/status`.

### Implementato e verificato

- aggiunti `✏️ Modifica` e `🗑 Elimina` alla scheda di ogni oggetto;
- aggiunti i comandi equivalenti `/oggetto_modifica <id>` e
  `/oggetto_elimina <id>`;
- la modifica carica dal database una bozza completa con l'ID dell'oggetto,
  evitando di creare una nuova riga durante il salvataggio;
- il nome diventa modificabile dall'apposito pulsante ed è sempre obbligatorio;
- tutti i dettagli già presenti possono essere sostituiti;
- `/salta` mantiene il valore corrente e il nuovo `/rimuovi` cancella il campo
  aperto;
- reso contestuale `❌ Annulla`/`/annulla`: durante la modifica di un oggetto
  salvato si torna direttamente alla sua scheda, mentre durante una nuova
  creazione si torna al menu Oggetti;
- la condizione può essere rimossa con un pulsante dedicato;
- `💾 Salva modifiche` aggiorna `items` e `oggetti` nella stessa transazione;
- l'eliminazione richiede una seconda conferma esplicita prima del `DELETE`;
- la cancellazione parte da `items`, sfruttando `ON DELETE CASCADE` per
  `oggetti`, `foto` e le altre relazioni core;
- dopo il commit della cancellazione viene rimossa anche la directory
  `data/media/oggetti/<id>/`; un eventuale errore di pulizia filesystem viene
  segnalato senza nascondere l'avvenuta eliminazione dal database;
- nessuna nuova migration: lo schema corrente supporta già modifica e delete;
- aggiunti test per caricamento della bozza di modifica, update senza duplicati
  e cascade delle foto durante l'eliminazione;
- documentata separatamente la specifica in
  `docs/moduli/modifica-eliminazione.md`.

### Verifiche di chiusura

- controlli Rust e Clippy superati;
- modifica reale verificata sul Galaxy S9 senza duplicare l'oggetto;
- `/salta` e `/rimuovi` verificati;
- annullamento contestuale verificato: dalla modifica si torna alla scheda dello
  stesso oggetto, dalla nuova creazione al menu Oggetti;
- eliminazione con conferma, cascade SQLite e rimozione dei media locali
  verificate;
- Pull Request mergiata su `main`;
- CI GitHub Actions del merge verde.

### Stato finale

**Step 5C chiuso e verificato.** Il passo successivo approvato è Step 6A — case,
stanze e posizione strutturata.

## Step 5B — Foto oggetti e navigazione di avvio — 2026-08-15

### Stato precedente

Lo Step 5A è stato mergiato su `main`, la CI del merge è verde e la seconda
migration è stata applicata e verificata anche sul database reale del Galaxy S9.
Un backup consistente del database reale è stato creato prima dell'upgrade e il
secondo avvio ha confermato `Migrazioni applicate: 2`.

### Implementazione predisposta per test

- notifica automatica `🟢 Gestionale Casa è online` all'avvio del backend;
- la notifica di avvio contiene direttamente l'inline keyboard del menu principale;
- `/status` e il pulsante Stato sistema mostrano `🏠 Menu principale`;
- nuovo modulo `src/modules/foto.rs`;
- pulsante `📷 Foto` nella scheda degli oggetti;
- menu foto con aggiunta, visualizzazione e ritorno all'oggetto;
- comandi equivalenti `/foto <id>` e `/foto_aggiungi <id>`;
- ricezione delle immagini Telegram anche quando il messaggio non contiene testo;
- download della versione più grande della foto in `data/media/oggetti/<item_id>/`;
- registrazione del percorso, ruolo e descrizione nella tabella core `foto`;
- prima foto di un oggetto marcata `principale`, successive `galleria`;
- didascalia Telegram usata come descrizione della foto;
- visualizzazione delle foto dal file locale tramite Telegram;
- rimozione del file locale se la registrazione SQLite fallisce, per evitare
  file orfani;
- due test automatici dedicati a estensione file e ruoli principale/galleria;
- nessuna nuova migration: viene riusata la tabella `foto` dello schema core;
- `tokio` abilita la feature `fs` necessaria al salvataggio asincrono dei file.

### Verifiche completate

- `cargo fmt --all -- --check`, `cargo check --locked` e Clippy superati;
- `cargo test --locked`: 11 test superati, 0 falliti;
- notifica online e menu automatico verificati sul Galaxy S9;
- ritorno al menu da `/status` verificato;
- due foto caricate sullo stesso oggetto con ruoli principale/galleria corretti;
- file locali verificati sotto `data/media/oggetti/<id>/`;
- visualizzazione e persistenza dopo riavvio verificate;
- file di test rimossi prima dell'uso reale;
- CI della Pull Request e CI su `main` verdi.

### Prossimo passo previsto

Dopo la chiusura dello Step 5B: **Step 5C — modifica ed eliminazione sicura
degli oggetti già salvati**. Il requisito multi-casa/stanze resta registrato per
lo Step 6 e la relativa architettura deve essere confermata prima della migration.



### Step 5A — verifica UX e preparazione chiusura

- Verificato sul Galaxy S9 il comportamento dei campi già compilati: il pannello
  mostra `✅`, riaprendo il campo viene mostrato il valore corrente, `/salta` lo
  conserva e un nuovo valore lo sostituisce esplicitamente.
- Verificato manualmente il caso Marca/Modello e un campo singolo (Posizione),
  con salvataggio e successiva ricerca dell'oggetto.
- `cargo fmt --all -- --check`, `git diff --check` e Clippy con `-D warnings`
  risultano superati dopo la patch UX; la suite `check/test` va rieseguita come
  controllo finale immediatamente prima del commit di chiusura.
- Durante un `cargo run` il linker LLVM/Termux è terminato una volta con
  segmentation fault; il comando ripetuto ha avviato correttamente il backend.
  Non sono state necessarie modifiche al codice.
- Configurato e provato l'accesso **OpenSSH locale PC -> S9** e il trasferimento
  file via SCP. GitHub `main` resta la fonte ufficiale; SSH/SCP diventano il
  canale operativo per testare patch senza commit di trasporto.
- Registrati per il futuro due requisiti: modifica degli oggetti già salvati e
  gestione trasversale di più case/stanze con ricerca sia filtrata sia globale.
  La struttura dei luoghi dovrà essere proposta e confermata prima di creare la
  migration.

### Roadmap aggiornata proposta

1. completare CI della Pull Request e merge su `main`; a quel punto Step 5A è
   formalmente chiuso;
2. Step 5B — foto degli oggetti;
3. Step 5C — modifica/eliminazione degli oggetti già salvati;
4. Step 6 — luoghi e multi-abitazione, prima dei successivi grandi moduli, dopo
   conferma dell'architettura.

### Step 5A — affinamento UX bozza oggetti

- Le sezioni gia' compilate nel pannello dettagli sono marcate con `✅`.
- Riaprendo un campo gia' valorizzato il bot mostra il valore attuale prima della sostituzione.
- Durante la revisione di un campo, `/salta` conserva il valore esistente; su un campo vuoto continua a lasciarlo vuoto.
- Aggiunto un test automatico dedicato al prompt dei campi gia' compilati.

## Step 5A — Oggetti generici: prima implementazione — 2026-08-14

### Stato precedente

Lo Step 4 era chiuso e verificato: Telegram, SQLx, SQLite, migration automatiche
e `/status` erano operativi sul Galaxy S9. Il modulo `oggetti` era ancora uno
scheletro e non esisteva una tabella specifica.

### Decisioni concordate

- il modulo riguarda solo **oggetti generici**;
- il nome è l'unico campo obbligatorio;
- i dettagli opzionali vengono scelti da un **pannello dettagli**;
- il numero seriale resta disponibile ma non è in primo piano;
- l'interfaccia principale usa pulsanti inline, mantenendo `/comandi`
  equivalenti in parallelo;
- pulsanti e comandi convergono sulla stessa logica applicativa.

### Implementato

- nuova migration `20260814121600_oggetti.sql`, senza modificare quella core;
- tabella `oggetti` collegata 1:1 a `items` con `ON DELETE CASCADE`;
- prezzi e valore stimato salvati in centesimi interi con `CHECK >= 0`;
- condizione limitata a `ottimo`, `buono`, `usurato`, `da_riparare`;
- menu principale Telegram con inline keyboard;
- menu Oggetti con Nuovo / Elenco / Cerca;
- comandi `/oggetti`, `/oggetto_nuovo`, `/oggetti_lista`,
  `/oggetto_cerca`, `/oggetto`, `/annulla`, `/salta`;
- creazione rapida con solo nome oppure pannello dettagli;
- flussi guidati per marca/modello e dati di acquisto;
- selezione condizione tramite pulsanti;
- altri dettagli: descrizione, valore stimato e seriale;
- salvataggio atomico di `items` + `oggetti` in transazione SQL;
- elenco alfabetico paginato;
- ricerca su nome, marca, modello, seriale, posizione, venditore, descrizione e note;
- scheda singola richiamabile da pulsante o `/oggetto <id>`;
- sessione bozza in memoria per chat;
- callback Telegram sottoposte alla stessa whitelist delle chat autorizzate;
- documentazione `docs/moduli/oggetti.md`.

### Test predisposti

- parsing importi italiani/decimali;
- validazione e normalizzazione date;
- parser dei comandi con suffisso `@nome_bot`;
- salvataggio, lettura, elenco e ricerca su SQLite;
- verifica `ON DELETE CASCADE`;
- verifica del `CHECK` contro importi negativi.

La sintassi delle due migration è stata verificata anche applicandole in ordine
su SQLite in memoria, inserendo un oggetto reale di prova e confermando il
rifiuto di un prezzo negativo.

### Stato dello step

**Implementato, non ancora chiuso.**

Prima della chiusura servono:

1. `cargo fmt --all -- --check`;
2. `cargo check --locked`;
3. `cargo test --locked`;
4. `cargo clippy --all-targets --locked -- -D warnings`;
5. GitHub Actions verde;
6. test runtime sul Galaxy S9 di pulsanti, comandi, creazione, elenco, ricerca,
   scheda e persistenza dopo riavvio.

### Prossimo passo standard

Chiudere e verificare lo Step 5A. Solo dopo passare allo **Step 5B —
modifica ed eliminazione sicura**.

---

Questo file registra gli step del progetto in ordine cronologico. Ogni step
spiega da quale stato si partiva, cosa è stato modificato, cosa è stato
verificato e quale sarà il passo successivo.

## Step 4 — SQLite operativo e stato del sistema — 2026-08-13 → 2026-08-14

### Stato precedente

Lo Step 3.1 era chiuso con CI verde. Il bot Telegram e la whitelist erano gia'
verificati sul Galaxy S9 e lo schema core SQLite esisteva come migration, ma
`src/db.rs` era ancora uno scheletro: il backend non apriva alcun database e
non eseguiva migration all'avvio.

### Fatto in questo step

- scelta SQLx 0.8.6 con `default-features = false` e sole feature necessarie:
  Tokio, SQLite, migration e macro;
- usato il driver SQLite bundled per ridurre le dipendenze native dell'host;
- aggiunto `DATABASE_URL` alla configurazione con default
  `sqlite://data/db/gestionale.db`;
- implementato `src/db.rs` con creazione cartella/file, pool SQLite e foreign
  key esplicitamente abilitate;
- incorporate e applicate automaticamente le migration all'avvio;
- aggiunto `build.rs` per far ricompilare il progetto quando cambia la cartella
  `migrations/`;
- condiviso `SqlitePool` con il dispatcher Teloxide;
- aggiunto `/status` con verifica di database, foreign key, migration applicate
  e presenza dello schema core;
- aggiornato `/start` per mostrare anche `/status`;
- aggiornati `.env.example`, README, architettura, handoff e documentazione
  delle migration;
- reso `scripts/backup.sh` consistente tramite l'API `.backup` di SQLite;
- aggiornato `scripts/termux-boot.sh` a `cargo run --release --locked`.

### Decisione sulla versione SQLx

La serie SQLx 0.9 richiede un toolchain Rust molto recente. Per non introdurre
un requisito non ancora verificato sull'host Android, lo Step 4 usa la serie
0.8.6, che offre gia' tutte le funzionalita' necessarie. Gli aggiornamenti
futuri possono essere valutati tramite le PR di Dependabot e testati sul Galaxy
S9 prima del merge.

### Verifiche effettuate sul Galaxy S9

- toolchain verificato: `rustc 1.97.1` e `cargo 1.97.1`;
- aggiunto SQLx 0.8.6 e rigenerato/versionato `Cargo.lock` direttamente sul
  Galaxy S9;
- `cargo check` completato correttamente con SQLx/SQLite;
- `cargo tree -i openssl-sys -e features` conferma che `openssl-sys` non è
  presente nella dependency graph;
- `cargo test --locked` completato con 2 test superati e 0 falliti;
- `cargo run --locked` avvia correttamente il backend;
- creato realmente `data/db/gestionale.db`;
- `/start` e `/ping` continuano a funzionare;
- `/status` verifica correttamente database SQLite, foreign key, migration
  applicata e presenza delle cinque tabelle core (`items`, `foto`, `tag`,
  `item_tag`, `promemoria`);
- un secondo avvio sullo stesso database funziona senza errori e senza
  riapplicazione distruttiva della migration.

Durante `cargo check`/`cargo test` Rust segnala una future incompatibility in
`proc-macro-error2 v2.0.1`. Non è un errore attuale e non blocca lo Step 4; va
rivalutata durante futuri aggiornamenti delle dipendenze, senza forzare upgrade
non verificati sul Galaxy S9.

### Stato dello step

**Step 4 chiuso e verificato sul dispositivo di destinazione.**

La chiusura resta valida finché anche la CI GitHub Actions associata al commit
di chiusura rimane verde; un eventuale fallimento della CI riapre lo step e va
risolto prima di iniziare lo Step 5.

### Prossimo passo standard

Dopo la chiusura dello Step 4: **Step 5 — progettazione e prima
implementazione del modulo Oggetti generici**.

---

## Step 3.1 — Handoff, workflow Git e automazioni GitHub — 2026-08-13

### Stato precedente

Lo Step 3 era chiuso e verificato sul Galaxy S9, PC/GitHub/S9 erano stati
riallineati e `Cargo.lock` era già versionato. Il repository era utilizzabile,
ma mancavano un documento di handoff autonomo, controlli CI e una descrizione
formale del workflow PC ↔ GitHub ↔ S9. Inoltre README e changelog contenevano
ancora riferimenti a `Cargo.lock` come file “da aggiungere”, ormai obsoleti.

### Fatto in questo step

- creato `docs/HANDOFF.md` come guida autosufficiente per una terza persona o
  un'altra AI;
- definito **GitHub `main` come fonte ufficiale** del progetto;
- formalizzato il workflow corrente:
  - PC Windows = sviluppo principale e commit/push;
  - GitHub = fonte ufficiale e sincronizzazione;
  - Galaxy S9 = host reale e test runtime;
- formalizzata l'eccezione per modifiche semplici nate sull'S9, seguite da
  push e successivo `git pull --ff-only` sul PC;
- documentata la regola di non sviluppare contemporaneamente sugli stessi
  file da PC e S9;
- documentata come evoluzione futura, **non implementata**, l'amministrazione
  remota tramite Tailscale + OpenSSH in Termux senza esporre SSH a Internet;
- aggiunto `.github/workflows/ci.yml` per controllare automaticamente format,
  check, test e Clippy su push/pull request verso `main` usando Rust stable;
- aggiunto `.github/dependabot.yml` per controlli settimanali di Cargo e
  GitHub Actions, senza auto-merge;
- corretto il comando di clone usando l'URL reale del repository;
- corrette le note obsolete su `Cargo.lock`, che è già versionato;
- aggiornati README e architettura per riflettere workflow e roadmap.

### Verifiche effettuate durante la preparazione

- lo Step 3 di partenza corrisponde al commit `734b23d`;
- `Cargo.lock` è presente;
- nessun valore reale di `TELOXIDE_TOKEN`, PAT GitHub o altro segreto è stato
  aggiunto;
- i file GitHub Actions/Dependabot sono stati predisposti secondo la sintassi
  documentata per i rispettivi strumenti;
- la logica Rust del bot non è stata modificata.

La verifica automatica definitiva viene registrata nella sezione seguente, dopo
le correzioni emerse dalla prima run GitHub Actions.

### Problemi emersi nella prima run CI e correzione

La prima esecuzione GitHub Actions dello Step 3.1 ha svolto correttamente il
proprio compito di controllo e ha evidenziato due problemi:

- `cargo fmt --all -- --check` ha segnalato che `src/config.rs` e `src/main.rs`
  non erano ancora formattati secondo `rustfmt`; sul Galaxy S9 è stato quindi
  eseguito `cargo fmt`, senza modificare la logica del bot;
- il job separato “Minimum Rust 1.88” ha fallito. Per questo gestionale non è
  utile mantenere un MSRV formale derivato dalle dipendenze transitive: il
  controllo è stato rimosso insieme a `rust-version = "1.88"` dal manifest.

La CI definitiva usa Rust stable aggiornato e mantiene i quattro controlli che
portano valore al progetto: format, check, test e Clippy. Il Galaxy S9 resta
l'ambiente reale di verifica runtime.

Dopo le correzioni è stata eseguita una nuova GitHub Action con esito positivo:

- `cargo fmt --all -- --check` — superato;
- `cargo check --locked` — superato;
- `cargo test --locked` — superato;
- `cargo clippy --all-targets --locked -- -D warnings` — superato.

### Stato dello step

**Step 3.1 chiuso e verificato tramite GitHub Actions.**

La prima run fallita resta documentata perché dimostra il valore della CI e rende
riconoscibili in futuro le correzioni effettuate.

### Prossimo passo standard

**Step 4 — SQLite operativo e stato del sistema**, come già annunciato nello
Step 3.

---

## Step 3 — Base backend Telegram e whitelist — 2026-08-12 → 2026-08-13

### Stato precedente

Il repository conteneva lo scheletro Rust e lo schema dati core SQLite, ma
`main.rs`, `config.rs` e `auth.rs` erano ancora composti principalmente da
TODO. Il bot Telegram esisteva già e token/chat ID erano stati configurati
sul Galaxy S9 tramite Termux.

### Fatto in questo step

- aggiunte al `Cargo.toml` le dipendenze minime per il primo backend:
  `tokio`, `teloxide`, `dotenvy`, `anyhow`, `tracing` e
  `tracing-subscriber`;
- Teloxide configurato con `rustls` e senza TLS nativo, per ridurre le
  dipendenze di sistema e facilitare l'esecuzione su Termux;
- implementato `Config::load()` in `src/config.rs`;
- validati `TELOXIDE_TOKEN` e `ALLOWED_CHAT_IDS`;
- evitato `Debug` sulla struct `Config` per ridurre il rischio di stampare
  accidentalmente il token nei log;
- implementata la whitelist in `src/auth.rs`;
- aggiunti due unit test per autorizzazione positiva e negativa;
- implementato il primo `Dispatcher` Teloxide in `src/main.rs`;
- aggiunta verifica iniziale del token/API tramite `get_me()`;
- aggiunti i comandi `/start` e `/ping`;
- le chat non autorizzate vengono ignorate senza eseguire comandi;
- aggiornato `migrations/README.md` perché lo schema core esiste già;
- aggiornata la roadmap e introdotto questo diario di sviluppo.

### Verifiche effettuate sul Galaxy S9

- `cargo test` completato correttamente;
- entrambi gli unit test della whitelist superati;
- `cargo run` avvia correttamente il backend e raggiunge le API Telegram;
- `/ping` verificato con risposta `Pong! Gestionale Casa è online.`;
- `/start` verificato con il messaggio di avvio e l'elenco dei comandi;
- test end-to-end della whitelist eseguito da un secondo account Telegram non
  presente in `ALLOWED_CHAT_IDS`: il bot non risponde, come previsto;
- nessun token Telegram reale o altro segreto è presente nei file versionati.

### Problema incontrato e risoluzione

Al primo `cargo test` su Termux la compilazione si è fermata su
`openssl-sys`. `cargo tree` ha mostrato la catena
`teloxide default -> native-tls -> reqwest -> openssl-sys`.

La causa non era il codice dello Step 3: il Galaxy S9 era ancora un commit
indietro e il `Cargo.toml` locale apparteneva allo Step 2, con
`teloxide = "0.17.0"`. Questa forma abilita le feature predefinite di
Teloxide, tra cui `native-tls`.

Dopo aver ripristinato il `Cargo.toml` locale e riallineato il telefono con
`origin/main`, la dipendenza è diventata quella prevista:

```toml
teloxide = { version = "0.17", default-features = false, features = ["rustls", "ctrlc_handler"] }
```

La successiva compilazione e tutti i test sono andati a buon fine. Questa nota
resta nel changelog per rendere riconoscibile lo stesso problema in futuro.

### Stato finale dello step

**Step 3 chiuso e verificato sul dispositivo di destinazione.**

`Cargo.lock` è stato generato sul Galaxy S9 durante la compilazione verificata
e successivamente versionato nel repository, così le versioni effettivamente
testate delle dipendenze restano riproducibili.

### Prossimo passo standard

**Step 4 — SQLite operativo e stato del sistema.**

Obiettivi previsti:

1. aggiungere `sqlx` con supporto SQLite;
2. leggere e validare `DATABASE_URL`;
3. creare automaticamente `data/db/` se necessario;
4. aprire SQLite con foreign key abilitate;
5. eseguire automaticamente le migration presenti in `migrations/`;
6. condividere il pool/database con il dispatcher Telegram;
7. aggiungere `/status` per verificare bot, database e migration.

Lo Step 4 non introduce ancora il modulo oggetti: deve prima dimostrare che la
catena `Telegram -> Rust -> SQLite` funziona correttamente dall'inizio alla
fine.

---

## Step 2 — Schema dati core — 2026-08-12

### Stato precedente

Era presente solo lo scheletro iniziale del repository.

### Fatto

- progettata la tabella centrale `items`;
- aggiunte `foto`, `tag`, `item_tag` e `promemoria`;
- creata la prima migration SQL;
- documentato lo schema in `docs/schema-core.md`;
- normalizzati i fine riga a LF.

### Passo successivo previsto allora

Creare la prima base eseguibile del backend Telegram.

---

## Step 1 — Scheletro iniziale

### Fatto

- creata la struttura del progetto Rust;
- separati configurazione, autenticazione, database e moduli funzionali;
- predisposte cartelle per migration, documentazione, script e dati locali;
- documentate le principali decisioni architetturali.
