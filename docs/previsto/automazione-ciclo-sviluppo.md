# Automazione del ciclo di sviluppo e deploy

**Stato: PREVISTO.**

Riprende e sostituisce, come specifica più ampia, la voce di backlog #9 già
segnata in `docs/moduli/miglioramenti.md` ("🧪 Zona test / aggiornamenti
quasi zero-downtime").

## Obiettivo

Automatizzare il ciclo di sviluppo in modo che il ruolo dell'amministratore
principale si riduca a: chiedere una nuova funzionalità all'inizio, e
confermare il collaudo funzionale alla fine. Il resto — scrittura, controlli
locali, commit, push, collaudo, verifica CI — avviene senza intervento
manuale.

## Topologia

- **PC fisso**: dove gira la sessione Claude Code che scrive il codice, fa i
  controlli locali, committa e pusha.
- **Galaxy S9**: nodo di produzione, esegue il bot via Termux. Raggiungibile
  dal PC fisso via Tailscale + SSH — stessa configurazione già documentata in
  `docs/infrastruttura.md` per il portatile, da replicare sul PC fisso quando
  sarà pronto.
- **Portatile**: solo per controllare il PC fisso da remoto (tunnel), non
  parla direttamente con l'S9.

**Dentro lo scope, ma indipendente dal resto**: una replica continua (sola
lettura) del database dell'S9 verso il PC fisso, come misura di sicurezza sui
dati — nessuna logica su "chi è attivo", nessuna scrittura dal lato PC fisso,
solo una copia sempre aggiornata a scopo di backup.

**Esplicitamente fuori scope per ora**: nessun nodo di standby che esegue il
bot al posto dell'S9, nessun failover automatico tra macchine. Rimandato a un
momento futuro, molto più avanti; se ripreso, userà un arbitraggio a
maggioranza tra 3 nodi (2 su 3 devono concordare), non un arbitro fisso
singolo.

## Il ciclo

1. L'amministratore principale chiede una funzionalità (in chat, o tramite il
   canale descritto in `docs/previsto/invio-miglioramenti-a-claude.md`).
2. L'agente scrive il codice, fa girare `fmt`, `check`, `clippy`, `test` in
   locale.
3. Commit su un branch dedicato, poi push.
4. L'agente si collega via SSH all'S9 ed esegue `scripts/aggiorna-s9.sh` per
   il collaudo automatico.
5. Verifica lo **stato reale della CI su GitHub Actions** (via API, non un
   riassunto locale — un esito locale verde non è un lasciapassare, regola
   già in vigore per lo sviluppo umano in `STATO.md`). Se non è verde, si
   ferma o corregge da solo, fino a un massimo di due tentativi di
   autocorrezione prima di fermarsi comunque e avvisare.
6. Se la CI è verde, avvia un deploy a downtime minimo:
   - salva/tiene disponibile l'ultimo binario/commit funzionante, così un
     rollback non richiede mai una ricompilazione;
   - manda un messaggio con un countdown alla manutenzione, aggiornato per
     modifica del messaggio stesso, mai un nuovo messaggio. **Deciso il
     3 settembre 2026**: non lo pilota il bot sull'S9, lo pilota l'agente
     orchestratore stesso, con chiamate dirette all'API Telegram
     (`sendMessage` una volta, poi solo `editMessageText`). Il motivo è che
     deve continuare ad aggiornarsi anche nel momento in cui il processo S9 è
     fermo per lo swap — l'unico in cui serve di più. Il token viene letto
     via SSH dall'S9 al momento del bisogno, mai scritto su disco sul
     dispositivo che esegue l'agente. **Niente pin (deciso il
     4 settembre 2026, dopo un secondo collaudo)**: fissare il messaggio e
     poi eliminarlo a fine collaudo lasciava in chat una notifica di sistema
     fantasma («Gestionale_Bot pinned Deleted message»), non ripulibile via
     API. Un messaggio normale aggiornato sempre sullo stesso id basta —
     resta comunque l'unico messaggio che cambia nella chat;
   - prima di fermare il processo, controlla se qualche chat ha uno stato
     "in attesa di input testuale" attivo e, se sì, rimanda lo stop fino a
     quando si libera, con un tempo massimo di attesa oltre il quale procede
     comunque. **Verificato il 3 settembre 2026**: quello stato non vive in
     un posto solo. `ContextState` in `src/context_bot.rs` tiene lo storico
     delle azioni recenti e il contesto di `💡 Migliora`, non l'attesa di un
     testo. L'attesa vera vive in nove mappe indipendenti in `src/main.rs`
     (`identity_sessions`, `food_sessions`, `profile_sessions`,
     `recipe_sessions`, `improvement_sessions`, `container_sessions`,
     `location_sessions`, `photo_sessions`, `sessions`), ognuna con il proprio
     `has_active(chat_id)`. Il controllo pre-swap le interroga tutte, senza
     unificarle prima — vedi la decisione in `STATO.md`. **Costruito e
     collaudato per davvero sull'S9 (sotto-step 4/5, 4 settembre 2026)**: le
     mappe (dieci, con
     `distribuzione_sessions` del sotto-step 3) vivono solo nella memoria
     del processo Rust sull'S9, quindi il controllo esterno via SSH non
     puo' leggerle direttamente. Deciso insieme ad Alessio: il bot ascolta
     `SIGUSR1` (non tocca il `SIGINT` gia' usato per lo spegnimento) e alla
     ricezione scrive `data/run/sessioni.txt` con il numero di chat con una
     sessione attiva — un segnale su richiesta, non una scrittura
     periodica. `scripts/controlla-sessioni-attive.sh` manda il segnale via
     SSH e ripete la lettura fino a "0 sessioni attive" o al timeout
     massimo. Non ancora collegato a `ferma-bot.sh`: la sequenza reale
     arriva con il sotto-step 5;
   - ferma il vecchio processo, avvia il nuovo binario;
   - se il nuovo processo non si avvia o va in errore subito dopo lo swap:
     rollback automatico e immediato al binario precedente, con notifica
     Telegram dell'errore preciso — l'S9 non deve mai restare giù in
     silenzio;
   - il codice nuovo riparte in **modalità riservata**: solo l'amministratore
     principale può usarlo davvero (ruoli/permessi già esistenti in
     `access_control.rs`). Gli utenti normali vedono lo stato di
     manutenzione e non possono interagire con la nuova versione finché non
     arriva la conferma. **Meccanica scritta (sotto-step 5a, 4 settembre
     2026), collaudo sull'S9 in corso**: un flag in memoria condiviso
     (`ModalitaRiservata`, un `AtomicBool` come `ShutdownController`), non
     un file o una variabile letta una volta sola all'avvio — deve poter
     tornare disattivo premendo un bottone in chat, senza riavviare il
     processo (deciso insieme ad Alessio). `scripts/avvia-bot.sh --riservato`
     imposta `RISERVATO=1` solo per quel lancio. Il bottone
     `✅ Sblocca, torna online per tutti`, visibile solo all'amministratore
     principale quando la modalità è attiva, disattiva il flag e notifica
     tutte le chat di utenti attivi. Non collaudabile fino in fondo senza un
     secondo account Telegram (stessa lacuna del punto 6 aperto in
     `STATO.md`): la logica di blocco è coperta da un unit test, non da un
     collaudo reale su un utente non amministratore;
   - tipo di messaggio di manutenzione e momento dell'aggiornamento
     (immediato o programmato) sono configurabili dall'amministratore
     principale. **Deciso il 3 settembre 2026**: una schermata
     `🛠️ Amministrazione → 🚀 Distribuzione`, con un default configurabile
     (es. sempre countdown di 5 minuti) e una scelta puntuale a ogni deploy —
     Subito / Countdown standard / Programma orario — offerta quando il
     countdown parte. **Il default e' costruito (sotto-step 3/5, 4 settembre
     2026)**: tabella a riga singola `impostazioni_distribuzione`
     (`migrations/20260904150000_impostazioni_distribuzione.sql`), con CHECK
     di coerenza tipo/parametro e le colonne `scelta_puntuale_*` gia'
     presenti ma senza UI — arriveranno con il sotto-step 5, quando esiste
     un deploy reale che le offre. **Deciso il 4 settembre 2026**: l'input
     dei valori (minuti del countdown, orario della manutenzione
     programmata) e' ibrido — bottoni con valori preimpostati piu' testo
     libero validato — gestito da una nuova mappa di sessione indipendente
     in `main.rs` (`distribuzione_sessions`), sullo stesso schema delle
     altre nove.
7. Notifica Telegram con il riepilogo di cosa è stato implementato e i passi
   concreti da provare — l'amministratore principale ha già accesso alla
   versione nuova in modalità riservata, gli altri utenti no.
8. Durante il collaudo guidato, un messaggio (senza pin — stessa decisione
   del punto 6) mostra una checklist dinamica degli step (☐ → ✅ via edit,
   mai nuovi messaggi), che si sblocca da sola a fine collaudo.
9. Solo dopo la conferma funzionale esplicita dell'amministratore principale,
   data su Telegram:
   - il bot esce dalla modalità riservata, sblocca l'uso normale per tutti e
     manda "✅ Di nuovo online" a tutte le chat autorizzate;
   - si procede al merge su `main`.
   Questo gate non diventa mai automatico. Se il collaudo viene rifiutato: lo
   stesso rollback immediato descritto sopra, si resta in modalità
   manutenzione per gli utenti normali finché non c'è una nuova versione
   pronta, e si riparte dal punto 2 con le correzioni.

Qualunque fallimento in un passaggio (connessione SSH persa, build o
collaudo in errore) genera una notifica Telegram immediata con l'errore
specifico e lo stato in cui è stato lasciato l'S9 — mai un fallimento
silenzioso.

## Tracciamento

Ogni ciclo automatico (richiesta, esito, eventuale rollback) va tracciato in
un log o nel `CHANGELOG.md`, coerente con la disciplina documentale già in
vigore nel progetto (vedi la sezione 0 di `STATO.md`).

## Deciso il 3 settembre 2026

- **chi pilota il messaggio di countdown/checklist**: l'agente orchestratore,
  via API Telegram diretta, non il bot sull'S9 — vedi il punto 6 sopra;
- **niente pin**, deciso il 4 settembre dopo un secondo collaudo: lascia una
  notifica di sistema fantasma quando il messaggio pinnato viene eliminato,
  non ripulibile via API — vedi il punto 6 sopra;
- **interfaccia per tipo/orario del messaggio di manutenzione**: schermata
  `🛠️ Amministrazione → 🚀 Distribuzione` con default + scelta puntuale —
  vedi il punto 6 sopra;
- **strategia per la coda "in attesa di input testuale"**: interrogare le
  nove mappe di sessione esistenti così come sono (nessun refactoring
  preliminare), con un timeout massimo prima di procedere comunque — vedi il
  punto 6 sopra;
- **`cargo test` non tocca mai il database reale**: verificato il
  3 settembre 2026 con una ricerca su tutto `src/` — ogni `.connect(...)` nei
  test usa `sqlite::memory:`, zero eccezioni. Non è più un punto aperto.

## Aperto

- dove e come l'agente tiene la chiave SSH e il token Telegram durante
  l'esecuzione automatica (oggi lette al bisogno via SSH, mai salvate su
  disco: da confermare che basti, quando il ciclo sarà end-to-end);
- cosa succede se la CI resta rossa dopo i due tentativi di autocorrezione:
  solo notifica, o anche uno stato visibile da qualche parte nel bot?
