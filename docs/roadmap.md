# Roadmap Step 7

## Stato corrente — 02/09/2026

**Tutto lo Step 7 e' su `main`** (PR #9 mergiata). **Nessun ramo aperto:**
`ux-convenzioni-telegram` e' stato mergiato con la PR #10.

Chiusi e verificati: 7.0, 7.1, tutta la 7.2 fino a 7.2I.3, e il planner 7.3A e
7.3B. Pipeline verde con **268 test**; **42 migration, tutte applicate** al
database reale (`applied_migrations=42` nel log di avvio dell'S9).

Chiuse anche: l'aritmetica delle date, che non passa piu' da SQLite (schermata
settimana da 19 query a 2); l'affidabilita' della build sull'S9; il primo giro
di correzioni UX, con `docs/convenzioni-telegram.md` come documento di
riferimento; e l'unificazione dei due calendari in `modules::calendario`.

Chiuso il 2 settembre anche il **blocco liste** delle convenzioni: alimenti,
ricette, storico e miglioramenti, con `modules::liste` a tenere paginazione e
intestazioni per tutte.

La prossima funzione e' la **lista della spesa aggregata**, costruita sugli
snapshot dei pasti non completati, con aggiornamento esplicito e separato da
quello del planner. Prima pero' restano da applicare le convenzioni a **Spazi e
Profilo** — la coppia «spazio predefinito» / «vista», la parte
concettualmente piu' difficile — poi al menu' principale e alle date: vedi la
parte 3 di quel documento.

CI verde dalla run #51. Aperti non bloccanti: toolchain dell'S9 da aggiornare
per allinearla a quella del runner, decisione sui pasti liberi, il planner
cercato sette volte per aprire una settimana, PR #6 di Dependabot.

## Sequenza

### 7.0 — Specifica e organizzazione — VERIFICATO

Decisioni, confini dei moduli, modello utenti/spazi e politica migration.

### 7.1 — Fondazioni condivise — OPERATIVE

Utenti, account Telegram, spazi, membership, ruoli, vista multi-spazio, ownership, condivisione, permessi e audit.

Da non confondere con il backlog #7: eliminazione/reset account richiede ancora una progettazione amministrativa dedicata.

### 7.2 — Alimentazione — IN SVILUPPO

Completato:

- Alimenti/unità/categorie;
- catalogo base e compatibilità;
- prodotti commerciali/formati/nutrizione;
- Ricette operative e procedimento guidato;
- accesso approvato, amministrazione e Miglioramenti;
- rifiniture UI Telegram e export Miglioramenti.

Prossimi blocchi funzionali, nell'ordine:

1. **Porzioni e override** — quantità personali e override ingrediente;
2. **Turni/routine**;
3. **Planner pasti** versionato;
4. **Lista della spesa** aggregata;
5. reminder/export Alimentazione.

### 7.3 — Integrazioni — PREVISTO

Google Calendar, email, inviti e completamento delle funzioni multiutente esterne.

## Moduli previsti e non ancora disponibili

Fino al 1 settembre 2026 il menù principale mostrava `👕 Vestiti · prossimamente`
e `🚗 Veicoli · prossimamente`: due pulsanti che non facevano niente e che
costringevano il messaggio del menù a spiegare cosa volesse dire
«prossimamente». Sono stati tolti dall'interfaccia — un pulsante che non porta
da nessuna parte e' un invito a premerlo per niente — ma **restano previsti**,
e sono elencati qui perche' non vadano persi:

- **👕 Vestiti** — `src/modules/vestiti.rs` esiste come segnaposto;
- **🚗 Veicoli** — `src/modules/veicoli.rs` esiste come segnaposto.

Torneranno nel menù quando avranno delle schermate vere dietro, insieme agli
altri domini gia' specificati elencati qui sotto.

## Dopo i domini funzionali

Rimangono già specificati Acquisti, Viaggi, Spese, documenti/garanzie, manutenzioni, prestiti, ricerca globale, QR/codici, Veicoli e Vestiti.

## Ultima evoluzione infrastrutturale futura

`🧪 Zona test` + aggiornamenti quasi zero-downtime (#9). **In corso dal
3 settembre 2026** sul ramo `automazione-ciclo-sviluppo`, per gradi. Specifica
completa in `docs/previsto/automazione-ciclo-sviluppo.md` (ciclo dev → deploy)
e `docs/previsto/invio-miglioramenti-a-claude.md` (canale di invio dal bot).

Consentirà all'admin di testare una candidata separata mentre tutti gli altri
restano sulla stabile, con database/snapshot di test, pipeline automatica,
conferma funzionale esplicita, deploy a downtime minimo e rollback. Un solo
processo riceve gli update Telegram per lo stesso token — nessun nodo di
standby o failover automatico, esplicitamente rimandato a un momento futuro.

<!-- STEP7_2H0_PROFILI_FONDAZIONI -->
## Decisioni aggiunte prima di 7.2H

Il blocco **7.2H.0 — Fondazioni Profili alimentari** introduce soltanto schema e vincoli di base, senza UI Telegram. I Profili restano separati dagli account e possono essere privati o condivisi tramite spazi; non possono essere globali.

Sono inoltre fissate come evoluzioni future:

- `🌐` contenuti globali soltanto per tipi compatibili e pubblicazione/modifica diretta riservata all'admin;
- coda di proposte utente per correggere o migliorare contenuti globali;
- `🛡️ Modalità utente` per consentire all'admin di usare il bot senza privilegi globali accidentali;
- planner con versione della ricetta applicata e `🔄 Aggiorna planner` mostrato soltanto quando una ricetta usata da pasti non completati è realmente cambiata;
- aggiornamento della lista della spesa separato dal planner, per non alterare automaticamente quantità già acquistate.

La sequenza funzionale resta: Profili → Porzioni/override → Turni → Planner → Lista della spesa.

<!-- STEP7_2H3_MEMBRI_RIFINITURE -->
## Step 7.2H.3 — membri degli spazi e rifiniture

Il blocco completa il passaggio necessario prima delle porzioni/planner:

- gestione esplicita dei membri degli spazi condivisi, senza associazione
  automatica dopo l'approvazione dell'account;
- rifinitura dello Storico dei Profili alimentari con etichette umane;
- pulizia degli export Miglioramenti abbandonati prima di perdere il riferimento
  al documento Telegram;
- supporto copia del testo originale durante `✏️ Modifica testo` quando rientra
  nel limite Telegram del pulsante copia;
- vero `⬅️ Indietro` nella schermata Profilo;
- aggiunta delle gallette al catalogo alimentare globale.

Restano futuri: condivisione diretta risorsa→account indipendente dallo spazio,
`🛡️ Modalità utente` per l'admin, proposte sui contenuti globali, porzioni e
override, turni, planner versionato e lista della spesa.

### Step 7.2H.4A — Inviti spazi

- inviti privati via deep-link Telegram;
- `📋 Copia link d'invito`;
- monouso, riutilizzabile, limite utilizzi e scadenza;
- calendario e orario modificabile anche dopo la creazione;
- `🔗 Inviti attivi`, revoca e modifica ruolo futuro;
- accettazione esplicita e notifica al creatore;
- modifica ruolo membro e notifiche su ruolo/rimozione;
- eliminazione automatica degli inviti non più validi.

Le rifiniture UX Miglioramenti/Spazi/export restano nel successivo Step 7.2H.4B, da applicare dopo il collaudo di H.4A.


## Step 7.2H.4B — Rifiniture UX inviti, spazi e miglioramenti

- selezione di uno spazio: conferma breve e ritorno immediato all'elenco `👥 Spazi`;
- apertura accidentale del proprio link / link di uno spazio di cui si è già membri: nessun consumo dell'invito e navigazione verso `👥 I miei spazi` o `🏠 Menù principale`;
- notifiche di accettazione, cambio ruolo e rimozione rese temporanee e navigabili;
- calendario inviti: mese prima della riga dei giorni, intestazioni `Lun`…`Dom` in pulsanti separati no-op, date passate non selezionabili, mese precedente bloccato quando non valido;
- navigazione delle schermate inviti uniformata sulla riga `⬅️ Indietro | 🏠 Menù principale | 💡 Migliora`;
- conferma download export: eliminazione immediata del documento Telegram e dello ZIP temporaneo;
- messaggio più corretto nei timeout d'invio export Telegram;
- salvataggio di un miglioramento contestuale: ritorno alla schermata da cui è stato premuto `💡 Migliora`;
- testi lunghi dei miglioramenti copiabili in parti da massimo 240 caratteri per rispettare il limite Telegram del pulsante copia.

Percorso canonico Spazi per i collaudi:
`🏠 Menù principale → 👥 Spazi`.

### Step 7.2H.4C — rifiniture inviti/orari e verifiche guidate

- calendario inviti: date passate marcate esplicitamente con `❌` e non selezionabili;
- cambio vista Spazi mantiene la schermata elenco aggiornata;
- dettaglio invito mostra data/ora di creazione;
- dall'orario `⬅️ Giorno` torna al calendario del mese selezionato;
- limite utilizzi modificabile anche dopo la creazione (`1` → monouso, valori finiti → limite, illimitato → riutilizzabile);
- orari rapidi applicati immediatamente;
- input manuale `HH:MM` in formato 24h con validazione e recupero dagli errori;
- predisposizione documentale per futura preferenza utente 12h/24h, senza cambiare ancora il formato globale del bot;
- navigazione finale Miglioramenti uniformata per permettere a ContextBot di affiancare `💡 Migliora` a `⬅️ Indietro | 🏠 Menù principale`;
- miglioramenti implementati H.3/H.4 portati a `Fatto · da verificare` con piano guidato e pulsante di apertura sezione;
- quattro collaudi che richiedono un secondo account registrati come verifiche `Fatto · da verificare`, da eseguire quando sarà disponibile.

### Step 7.2H.4D — rifiniture finali inviti e Miglioramenti
- input errati per orari/limiti restano recuperabili con navigazione;
- nella schermata orario è possibile digitare subito `HH:MM`, senza passaggio `Inserisci orario`;
- il limite utilizzi è digitabile direttamente (1–9999) oltre ai preset;
- dopo l’archiviazione di un miglioramento si torna alla lista `Fatti da verificare`;
- le quattro correzioni appena implementate entrano in `Fatto · da verificare` con piano guidato;
- i collaudi H.4 che richiedono un secondo account restano pendenti separatamente e non bloccano il passaggio allo step funzionale successivo.

### Step 7.2H.4E — input inatteso + export progetto

- Il testo inviato quando nessun wizard è in attesa non sostituisce più la schermata Telegram corrente: viene mostrato solo un avviso temporaneo e la UI persistente resta invariata.
- In `💡 Miglioramenti` l'amministratore dispone anche di `📦 Esporta progetto`.
- L'export progetto è un handoff tecnico sanitizzato: include codice, migration, documentazione, script e metadati Git ma esclude `.env`, token, database, `data/`, backup, `target/`, `.git/` e file temporanei.
- Lo ZIP contiene `_project_handoff/PROJECT_OVERVIEW.md`, manifest Git/file, albero del progetto e regole di esclusione.
- L'export fallisce in modo conservativo se nei file testuali inclusi viene rilevato un pattern compatibile con token Telegram, API key o chiave privata.


### Step 7.2H.4F — chiusura input/export

- dopo tre input inattesi consecutivi il bot suggerisce `/start`;
- navigazione/comando valido azzera il contatore;
- `📦 Esporta progetto` genera `_project_handoff/CURRENT_STATE.md`;
- `_project_handoff` viene rigenerato da zero, senza riciclare manifest precedenti;
- `GIT_MANIFEST.json` filtra file tecnici temporanei come `.pre_*`;
- il collaudo manuale di input escalation ed export progetto è stato confermato.
