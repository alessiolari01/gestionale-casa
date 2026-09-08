# Invio diretto dei miglioramenti a Claude

**Stato: PREVISTO.**

Riprende la voce di backlog #9 già segnata in `docs/moduli/miglioramenti.md`
("🧪 Zona test / aggiornamenti quasi zero-downtime"), nel contesto più ampio
del ciclo di sviluppo automatizzato descritto altrove in `docs/previsto/`.

## Problema

Oggi un miglioramento approvato (stato `da_fare`) diventa una richiesta di
lavoro solo se l'amministratore principale lo riporta a mano in una sessione
Claude Code. Manca un canale diretto dal bot verso l'agente, sia per i
miglioramenti creati dall'admin sia per quelli proposti da utenti normali e
poi approvati.

## Comportamento previsto

Sui miglioramenti in stato `da_fare` compare l'azione:

```text
📤 Invia a Claude
```

Visibile e utilizzabile **solo dall'amministratore principale**, chiunque
ne sia l'autore originale (admin o utente normale approvato) — non un
permesso condiviso con altri ruoli.

L'azione mette il miglioramento in una coda (nuova colonna/tabella dedicata,
da progettare in fase di implementazione, coerente con lo schema esistente
di `miglioramenti`). Non implica esecuzione immediata: è l'agente, quando
attivo su una sessione Claude Code sul PC fisso, a leggere la coda e a
trattare ogni voce come una richiesta di funzionalità, seguendo lo stesso
ciclo descritto per l'automazione del deploy (scrittura, collaudo, conferma
funzionale, merge).

## Cosa passa nella richiesta

Lo stesso contenuto già presente nel record del miglioramento:

- testo/descrizione (multimessaggio, come oggi);
- allegati collegati;
- autore originale (admin o utente normale approvato), per contesto;
- eventuale piano di verifica già presente in `miglioramento_piani_verifica`,
  utile come base per la checklist di collaudo guidato.

## Relazione con il resto

Dipende dal ciclo di automazione già descritto in `docs/previsto/` (deploy a
downtime minimo, modalità riservata, conferma via Telegram): questo
documento aggiunge solo un canale di **ingresso** alternativo alla richiesta
scritta a mano in chat, non cambia il ciclo stesso una volta che la
richiesta è stata presa in carico.

## Transizioni di stato guidate da Claude

Per i miglioramenti presi in carico tramite questo canale, è l'agente a far
avanzare lo stato secondo il workflow già esistente nel modulo, non
l'amministratore a mano:

- `da_fare` → `fatto` quando l'implementazione è completa e la CI è verde
  (corrisponde al passaggio 6 del ciclo di automazione);
- `fatto` → verificato/archiviato quando arriva la tua conferma funzionale
  su Telegram (passaggio 9) — la conferma stessa sostituisce la pressione
  manuale di `📦 Archivia miglioramento`, non introduce un passaggio in più.

Se rifiuti il collaudo, il miglioramento resta `da_fare` con le note di
cosa non ha funzionato, coerente con il rollback già previsto nel ciclo di
automazione.

**Regola sui backup, legata a questo momento**: quando la conferma archivia
il miglioramento, sul database restano **sempre e solo i 5 backup più
recenti** — se il ciclo ne ha creato uno nuovo, il più vecchio oltre i 5
viene rimosso. Non è un meccanismo nuovo da costruire: `aggiorna-s9.sh` lo fa
già da solo a ogni esecuzione (`BACKUP_DA_TENERE=5`, verificato il
4 settembre 2026 — sull'S9 ce n'erano già esattamente 5), quindi la garanzia
vale già prima ancora che arrivi la conferma. Resta scritta qui perché e' il
punto in cui l'amministratore principale se lo aspetta: dopo aver confermato
e archiviato, il numero di backup non deve mai essere diverso da 5.

## Deciso il 3 settembre 2026

- **invii duplicati**: una colonna `in_coda_claude` (booleano) + timestamp
  sulla riga `miglioramenti`, non una tabella coda separata. Premere
  `📤 Invia a Claude` quando è già in coda aggiorna solo il timestamp, non
  crea una seconda voce; il pulsante mostra lo stato "già in coda" finché
  l'agente non lo prende in carico e lo azzera.

## Deciso il 7 settembre 2026 — nessun codice ancora scritto, solo architettura

Sessione di sole decisioni: si è discusso a fondo come costruire questo
pezzo, ma poi si è deviato su un altro lavoro (badge "🆕", vedi
`STATO.md` sezione 2ter/2quater) senza scrivere nemmeno una riga di
codice di questo documento. Le decisioni restano valide e vanno
implementate quando si riprende — sono scritte qui apposta perché non
andassero perse.

- **Selezione**: sui `da_fare` un pulsante `📤 Invia a Claude` apre una
  selezione multipla (checklist ☐/✅ che si spuntano via edit dello
  stesso messaggio, come in `src/modules/collaudo.rs`), non un invio
  singolo per miglioramento. Alla conferma, applica la selezione:
  `in_coda_claude = 1` + timestamp per i selezionati, `= 0` per i
  `da_fare` non selezionati ma attualmente in coda (semantica "modifica
  l'appartenenza alla coda", non "accumula").
- **Lettura/scrittura della coda dal PC fisso**: SSH on-demand +
  `sqlite3`, stesso pattern di `scripts/collauda-remoto.sh` /
  `scripts/controlla-sessioni-attive.sh`. Niente replica del database
  per questo pezzo.
- **Allegati**: fuori scope in questa prima versione — l'agente lavora
  su testo/descrizione/piano di verifica, non sugli allegati. Si
  aggiungono in un pezzo successivo se servirà davvero.
- **Momento della scelta subito/timer/orario** (passaggio 6 del ciclo):
  si chiede alla notifica "pronto per il collaudo" (passaggio 7), non al
  momento dell'invio a Claude — riusa il flusso già costruito nel punto
  6 di `automazione-ciclo-sviluppo.md` (schermata `🚀 Distribuzione`,
  colonne `scelta_puntuale_*` già pronte nello schema apposta per
  questo). Chiederlo prima sarebbe una decisione presa troppo presto,
  quando non si sa ancora quanto durerà l'implementazione.
- **Ordine di lavorazione della coda**: un miglioramento alla volta, in
  sequenza — il più vecchio in coda (per `in_coda_claude_il`) prima.
- **Archiviazione automatica al passaggio 9 (confermato)**:
  `verify_and_archive_improvement` in `miglioramenti.rs` è una
  transazione a 4 passaggi (verifica, copia in `miglioramenti_archivio`,
  copia allegati/prove, cancellazione) — troppo delicata da riscrivere in
  SQL puro via SSH senza rischiare che diverga nel tempo. Si riusa quella
  stessa funzione Rust tramite un piccolo comando CLI nascosto sul
  binario esistente (es. `gestionale-casa --archivia-miglioramento <id>`),
  invocato via SSH, che apre il pool DB e chiama la funzione sotto il
  contesto dell'amministratore principale senza avviare il long-polling
  Telegram (eviterebbe comunque un 409 Conflict con il bot già in
  ascolto).
- **Rifiuto al passaggio 9**: è un semplice `UPDATE` (non una
  transazione) — via SQL diretto su SSH va bene:
  `SET stato='da_fare', verifica_note=<nota>, in_coda_claude=0 WHERE id=?
  AND stato='fatto'`. La nota testuale va raccolta estendendo il
  pulsante `❌ Non funziona` del collaudo guidato (`collaudo.rs`) con una
  scelta ibrida "✏️ Scrivi nota" / "⏭️ Salta", coerente con lo schema
  già usato per `distribuzione_sessions`.
- **Collegamento tra il deploy in corso e il miglioramento che lo ha
  originato**: un file su `data/run/` (stesso canale già usato per
  `RISERVATO`/`riepilogo_deploy.txt`/`esito_collaudo.txt`), scritto da
  `deploy.sh` con un nuovo flag `--miglioramento-id N`, letto da
  `completa-deploy.sh` per sapere quale riga archiviare o riportare a
  `da_fare`.
- **Merge automatico dopo la conferma**: la pressione di "✅ Confermo,
  funziona" su Telegram **è** la conferma esplicita per il merge — una
  volta letto "confermato", il ciclo passa da solo `--confermo-merge` a
  `completa-deploy.sh`, chiudendo davvero il ciclo (chiedi all'inizio,
  confermi alla fine, nient'altro in mezzo). Lo script mantiene comunque
  `--confermo-merge` come flag richiesto esplicito (non diventa il
  default): resta una rete di sicurezza per chi lo lancia a mano senza
  quel segnale.

### Il punto rimasto irrisolto: l'innesco automatico

Discusso a lungo, **non deciso in modo definitivo, rimandato come pezzo
a sé**: come si accorge un agente che c'è qualcosa in coda? L'opzione
concreta emersa (poll, non push: uno spegnimento/riavvio del PC fisso o
la mancanza di un servizio dedicato rendono il "push" via listener HTTP
troppo costoso da costruire e mantenere rispetto al beneficio) è una
sessione Claude Code **dedicata** — non quella con cui si lavora di
solito — tenuta aperta apposta sul PC fisso, che usa il proprio `/loop`
a autoripianificazione per risvegliarsi ogni 5 minuti, controllare la
coda via SSH e, se trova qualcosa, eseguire da sola l'intero ciclo
(scrittura, pipeline locale, collaudo remoto, CI, deploy) fino al primo
gate umano.

Alessio ha preferito **non costruirlo ora**: l'idea è stata messa in
coda come un miglioramento vero e proprio nel bot stesso (creato il
5 settembre 2026, stato `da_fare`: "Creare una sessione Claude Code
dedicata... che controlli periodicamente la coda dei miglioramenti
inviati a Claude..."). **Attenzione all'id**: il database è stato
azzerato l'8 settembre 2026 (account dedicati, vedi `STATO.md` punto
10 dei "Punti aperti") e il miglioramento è stato ripristinato da un
backup con un id nuovo — cercarlo per testo, non per numero, se questo
riferimento risultasse disallineato in futuro. Per ora le richieste
continuano ad arrivare scritte a mano in chat, come sempre. Il canale di
ingresso (bottone + coda, sopra) si può costruire e collaudare
indipendentemente da questo — l'agente prenderebbe in carico la coda
quando gli viene chiesto esplicitamente, finché quel miglioramento non
sarà a sua volta implementato.
