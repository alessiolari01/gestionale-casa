# Roadmap Step 7

## Stato corrente

Il blocco **7.2H.0→7.2H.4F** è chiuso funzionalmente e in finalizzazione documentale/GitHub. Profili alimentari, membri/inviti Spazi, verifiche guidate, input inattesi ed export progetto sono operativi. Le sole prove residue sono quattro collaudi end-to-end che richiedono un secondo account Telegram e restano differiti.

La prossima funzione da sviluppare è **Porzioni e override per Profilo/ingrediente**.
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

## Dopo i domini funzionali

Rimangono già specificati Acquisti, Viaggi, Spese, documenti/garanzie, manutenzioni, prestiti, ricerca globale, QR/codici, Veicoli e Vestiti.

## Ultima evoluzione infrastrutturale futura

`🧪 Zona test` + aggiornamenti quasi zero-downtime (#9).

Non è prioritaria adesso. Quando verrà implementata dovrà consentire all'admin di testare una candidata separata mentre tutti gli altri restano sulla stabile, con database/snapshot di test, pipeline automatica, `✅ Conferma versione`, `🚀 Installa e riavvia`, backup e rollback. Un solo processo deve ricevere gli update Telegram per lo stesso token.

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
