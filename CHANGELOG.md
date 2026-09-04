> I percorsi citati nelle voci piu' vecchie si riferiscono alla struttura dei
> documenti dell'epoca. La cartella e' stata riordinata il 2 settembre 2026:
> la mappa attuale e' nel `README.md`.

<!-- CHANGELOG_ORCHESTRAZIONE_DEPLOY_20260904 -->
# 04/09/2026 — L'orchestrazione dello swap (sotto-step 5d, scritta)

Ultimo pezzo del sotto-step 5/5 (lo swap vero): tre script legano insieme
tutto quello costruito e collaudato nei sotto-step 1-5c.

`scripts/countdown-manutenzione.sh` (nuovo) e' la versione "vera" del
countdown gia' collaudato in `prova-countdown.sh` -- stessa meccanica a
scadenza fissa -- ma legge il default (tipo/minuti/orario) dalla tabella
`impostazioni_distribuzione` invece di un valore fisso da riga di comando.
Il tipo "countdown" e' l'unico collaudato per davvero fin qui (e' l'unico
default operativo del progetto); "subito" e "programmato" sono scritti ma
meno esercitati.

`scripts/deploy.sh` (nuovo) e' la sequenza vera: countdown →
`controlla-sessioni-attive.sh` → `salva-binario.sh` (prima di aggiornare
il codice, non dopo) → `ferma-bot.sh` → `aggiorna-s9.sh --ramo X
--solo-controlli` (ricompila e riverifica sull'S9, non un `git pull`
nudo -- il binario resta pronto per il passo dopo senza ricompilare di
nuovo) → copia del riepilogo/checklist e `avvia-bot.sh --riservato` →
controllo di stabilita' (online + resta vivo per una finestra, default
20s) → rollback automatico se qualcosa va storto, con notifica Telegram
dell'errore preciso a ogni passo che fallisce. Da li' l'agente si ferma:
il collaudo lo gestisce il bot stesso (5c).

`scripts/completa-deploy.sh` (nuovo) e' il seguito: legge
`esito_collaudo.txt` sull'S9 e, se "confermato", fa il merge del ramo su
`main` -- **solo con `--confermo-merge` esplicito**, deciso per non
rendere irreversibile un'azione con conseguenze reali senza un secondo
controllo. Se "rifiutato", rollback -- ma qui serve un nuovo flag
`--riservato` su `rollback-binario.sh` (sotto-step 5b), trovato scrivendo
questo pezzo: il rollback per salute fallita torna in modalita' normale
(il binario vecchio era gia' in produzione, non ha bisogno di ripartire
riservato), ma un collaudo *rifiutato* dalla specifica deve restare in
manutenzione per gli utenti normali finche' non c'e' una versione
corretta -- lo stesso script di rollback, due comportamenti diversi a
seconda di perche' viene chiamato.

Nessun test Rust coinvolto (solo script bash). Collaudo end-to-end non
ancora fatto: la prima prova reale sara' una "ridistribuzione" dello
stesso commit gia' in produzione, per esercitare la sequenza vera senza
rischio funzionale. Totale invariato: 289.

<!-- CHANGELOG_SOTTOSTEP_5C_COMPLETO_20260904 -->
# 04/09/2026 — Sotto-step 5c completo: collaudo end-to-end confermato

Ultimo giro sull'S9, dopo tre correzioni trovate collaudando per davvero:
checklist di prova spuntata voce per voce, bottoni di conferma comparsi
solo a completamento, "✅ Confermo, funziona" premuto → messaggio diventato
"✅ Collaudo confermato", notifica separata "✅ Di nuovo online." con il
bottone `🏠 Menù principale`, premuto quel bottone → menu principale vero
e operativo. Confermato da Alessio sulla chat reale.

Bot fermato con `ferma-bot.sh` a fine collaudo. File di prova
(`riepilogo_deploy.txt`, `esito_collaudo.txt`) ripuliti da `data/run/`
sull'S9.

Con 5a, 5b e 5c fatti, resta solo il sotto-step 5d: l'orchestrazione che
lega insieme i pezzi già costruiti e collaudati singolarmente (controllo
sessioni, salvataggio del binario, stop/avvio, riepilogo/checklist, esito)
in un'unica sequenza automatica.

Nessun codice nuovo (solo verifica e pulizia). Totale invariato: 289.

<!-- CHANGELOG_MENU_DOPO_CONFERMA_20260904 -->
# 04/09/2026 — Dopo la conferma, un bottone per tornare subito operativi

Collaudata per davvero sull'S9 la correzione precedente
(`send_message_untracked`): la checklist e' rimasta "Collaudo confermato",
la notifica "Di nuovo online" separata, senza il bottone Migliora.

Alessio ha chiesto un miglioramento a caldo, dopo aver visto il
comportamento corretto: poter riprendere a usare il gestionale subito
dopo la conferma, senza dover scrivere un comando a mano. Aggiunto un
bottone `🏠 Menu principale` (callback `menu:main`, lo stesso di ogni
altra schermata) alla notifica "Di nuovo online".

Perche' quel bottone sia davvero cliccabile, pero', la notifica dev'essere
di nuovo *tracciata* da `ContextBot` (`send_message_without_improve`, non
`send_message_untracked`): `claim_callback` accetta un click solo su una
schermata registrata come "attiva" per quella chat, e un messaggio
untracked non lo e' mai. Per restare sicuri senza reintrodurre il bug
delle due voci precedenti, riordinato il gestore di `collaudo:conferma`:
l'edit del messaggio di collaudo in "confermato" avviene *prima* della
notifica broadcast, non dopo -- cosi' la notifica tracciata (che cancella
la schermata attiva precedente, come ogni altra schermata del progetto)
cancella il messaggio "confermato" gia' mostrato, non la checklist ancora
da completare.

Nessun test nuovo. Totale invariato: 289. Ricollaudo sull'S9 nel prossimo
commit.

<!-- CHANGELOG_SEND_MESSAGE_UNTRACKED_20260904 -->
# 04/09/2026 — Il primo tentativo di correzione non bypassava davvero ContextBot

Il commit precedente diceva di correggere il problema con
`(*bot).send_message(...)`, per bypassare il wrapper `ContextBot` e
raggiungere il bot grezzo di teloxide. Non era vero, e il ricollaudo
sull'S9 se n'e' accorto subito: in chat compariva di nuovo il bottone
"💡 Migliora" sul messaggio "Di nuovo online.", segno che il messaggio era
ancora tracciato da `ContextBot` esattamente come prima.

La causa dell'errore: `*bot` dove `bot: &ContextBot` e' il deref
*built-in* del riferimento, che restituisce `ContextBot` stesso -- non
passa mai dal suo `impl Deref<Target = TelegramBot>`, che si attiverebbe
solo derefando *quel* valore un'altra volta (`**bot`). Quindi
`(*bot).send_message(...)` era, di fatto, identico a
`bot.send_message(...)`: stesso identico bug di prima, mascherato da un
cambiamento che sembrava dover funzionare.

Corretto per davvero con `send_message_untracked`, un metodo che esiste
gia' in `context_bot.rs` (usato altrove nel progetto, es. per l'avviso di
input inatteso) proprio per le notifiche che non devono toccare lo stato
di "schermata attiva" di nessuno: usa il bot grezzo di teloxide senza
passare dal wrapper, nessun trucco di dereferenziazione necessario.

Nessun test nuovo. Totale invariato: 289. Ricollaudo sull'S9 nel prossimo
commit.

<!-- CHANGELOG_NOTIFICA_CANCELLAVA_COLLAUDO_20260904 -->
# 04/09/2026 — La notifica di "di nuovo online" cancellava la checklist appena confermata

Trovato per davvero collaudando la conferma sull'S9, subito dopo il fix
precedente: premuto "✅ Confermo, funziona", in chat e' rimasto solo
"✅ Di nuovo online." -- la checklist con scritto "Collaudo confermato"
non c'era piu'.

Causa: `esci_da_modalita_riservata` manda quella notifica con
`send_message_without_improve`, che passa dal wrapper `ContextBot` e dalla
sua regola "una sola schermata UI attiva per chat" -- mandare un messaggio
nuovo a una chat cancella quello attivo precedente. Per l'amministratore
principale, il messaggio attivo precedente era proprio la checklist di
collaudo appena modificata: la cancellazione arriva prima che l'edit
"confermato" abbia effetto (o lo cancella subito dopo), quindi l'edit
fallisce silenziosamente su un messaggio gia' sparito.

Corretto mandando quella notifica con il bot grezzo (`(*bot).send_message`,
bypassando il wrapper `ContextBot`): e' una notifica di cortesia a
chat qualsiasi (compresi utenti che non hanno nessuna schermata di
collaudo aperta), non una nuova schermata -- non deve toccare lo stato di
"schermata attiva" di nessuno.

Nessun test nuovo (comportamento verificato dal collaudo reale sull'S9).
Totale invariato: 289. Ricollaudo nel prossimo commit.

<!-- CHANGELOG_CLAIM_CALLBACK_RIPETIBILE_20260904 -->
# 04/09/2026 — La checklist di collaudo si bloccava al secondo click

Trovato per davvero collaudando il sotto-step 5c sull'S9, su Telegram
vero: Alessio ha spuntato la prima voce della checklist con successo, poi
la seconda ha fatto comparire "⚠️ Questa schermata non è più attiva. Ho
aperto un nuovo Menù principale." -- come se il messaggio fosse vecchio,
anche se era lo stesso appena arrivato.

Causa: `ContextBot::claim_callback` (`src/context_bot.rs`) permette una
sola rivendicazione per `message_id` e poi lo segna come "gia' usato" per
sempre. E' una protezione corretta per come funziona il resto del
progetto -- ogni altra schermata manda sempre un messaggio nuovo a ogni
cambio, mai una modifica in place -- ma la checklist del collaudo guidato
(sotto-step 5c) e' la prima a modificare piu' volte lo stesso messaggio
via edit: ogni click dopo il primo sullo stesso `message_id` falliva
sempre, a prescindere dalla velocita' con cui veniva premuto.

Corretto estendendo ai callback `collaudo:*` la stessa eccezione gia'
esistente per i bottoni `*:noop`, che verifica "e' ancora il messaggio
attivo?" invece di "non e' mai stato rivendicato prima?" -- la checklist
puo' essere premuta piu' volte finche' resta la schermata attiva, come
serve a un messaggio che si aggiorna via edit invece di rimandarne uno
nuovo.

Nessun test nuovo (la logica di `claim_callback` non ha una suite propria
in questo file; il comportamento e' verificato dal collaudo reale sull'S9,
non da un unit test). Totale invariato: 289. Ricollaudo sull'S9 nel
prossimo commit.

<!-- CHANGELOG_BIT_ESECUZIONE_SCRIPT_20260904 -->
# 04/09/2026 — Gli script nuovi non avevano il bit di esecuzione

Trovato per davvero aggiornando l'S9 al commit del sotto-step 5c:
`aggiorna-s9.sh` si e' fermato per modifiche locali non committate su
`scripts/rollback-binario.sh` e `scripts/salva-binario.sh` -- il `chmod +x`
dato a mano durante il collaudo del sotto-step 5b, mai tornato indietro.

La causa di fondo: `avvia-bot.sh` e `ferma-bot.sh` sono tracciati in git
come eseguibili (`100755`), ma i tre script piu' recenti
(`controlla-sessioni-attive.sh`, `rollback-binario.sh`,
`salva-binario.sh`), creati su questa macchina Windows, sono finiti nel
repository come file normali (`100644`) -- Windows non ha il concetto di
bit di esecuzione, quindi git li ha aggiunti senza. Sull'S9 (Termux, Unix
vero) `./scripts/x.sh` fallisce senza quel bit, servirebbe un `chmod +x`
manuale a ogni checkout pulito.

Corretto impostando il bit in git (`git update-index --chmod=+x`), non sul
filesystem: cosi' resta nel repository e ogni futuro `git clone`/checkout
sull'S9 lo trova gia' giusto.

Nessun test coinvolto (permessi, non codice). Totale invariato: 289.

<!-- CHANGELOG_COLLAUDO_GUIDATO_20260904 -->
# 04/09/2026 — Riepilogo, checklist e conferma pilotati dal bot (sotto-step 5c, scritto)

Terzo pezzo del sotto-step 5/5 (lo swap vero): i punti 7-8-9 della
specifica (riepilogo di cosa e' stato implementato, checklist di collaudo
guidato, conferma/rifiuto finale dell'amministratore principale).

Deciso insieme ad Alessio prima di scrivere codice: a differenza del
countdown (agente orchestratore, API diretta, perche' deve continuare ad
aggiornarsi anche col vecchio processo fermo), qui e' il bot nuovo stesso
a pilotare tutto — e' gia' acceso e in ascolto su Telegram appena lo swap
e' completo, non serve altro.

Il contenuto (riepilogo + passi da provare) arriva da un file scritto
dall'agente prima dello swap (`data/run/riepilogo_deploy.txt`: testo
libero, poi una riga `---CHECKLIST---`, poi una voce per riga) — stesso
canale a file gia' usato per `RISERVATO` e per lo stato delle sessioni,
non un parametro nuovo per ogni pezzo. All'avvio, con `RISERVATO=1` e il
file leggibile, il bot manda da solo il messaggio all'amministratore
principale (`identity::list_primary_admin_chat_ids`): checklist a bottoni
`☐`/`✅` che si spuntano via edit dello stesso messaggio, mai un messaggio
nuovo. I bottoni `✅ Confermo, funziona` / `❌ Non funziona` compaiono solo
a checklist completa — e il server li rifiuta comunque se qualcuno li
premesse prima, senza fidarsi del solo stato lato client.

Alla conferma: disattiva la modalita' riservata e notifica tutte le chat
attive, con `esci_da_modalita_riservata` — la stessa funzione gia' usata
dal bottone di sblocco del sotto-step 5a, condivisa invece di duplicata.
Al rifiuto: resta in modalita' riservata, come da specifica. In entrambi i
casi scrive `data/run/esito_collaudo.txt` ("confermato"/"rifiutato"), che
l'agente orchestratore (sotto-step 5d) leggera' per decidere se procedere
al merge su `main` o innescare il rollback del sotto-step 5b — non ancora
collegato a nessuno dei due.

289 test (280 prima), 9 nuovi in `src/modules/collaudo.rs`: interpretazione
del file (separatore mancante, checklist vuota, righe vuote ignorate),
`tutto_fatto`/`alterna` (incluso un indice inesistente), la tastiera che
mostra conferma/rifiuto solo a checklist completa, il troncamento delle
etichette lunghe. Collaudo reale sull'S9 nel prossimo commit.

<!-- CHANGELOG_ROLLBACK_BINARIO_COLLAUDO_20260904 -->
# 04/09/2026 — Sotto-step 5b collaudato per davvero: rollback in ~2 secondi

Il rollback del binario descritto nella voce precedente e' stato
collaudato sul bot vero, sull'S9 vero. `salva-binario.sh` ha copiato il
binario (50M). Avviato il bot normale con `avvia-bot.sh`, poi lanciato
`rollback-binario.sh`: ha fermato quel processo e fatto ripartire il
binario salvato in **~2 secondi** (misurato con `time`), log senza nessuna
riga "Compiling" — conferma diretta che non c'e' stata ricompilazione — e
senza `RISERVATO` nel log, cioe' tornato in modalita' normale come
deciso (era gia' la versione in produzione, non ha bisogno di ripartire
riservata). `ferma-bot.sh` ha poi fermato il processo ripristinato con lo
spegnimento pulito consueto, senza saperne nulla del rollback avvenuto
prima.

Nessun codice Rust coinvolto (verifica di due script bash). Totale
invariato: 280.

<!-- CHANGELOG_ROLLBACK_BINARIO_20260904 -->
# 04/09/2026 — Rollback del binario senza ricompilazione (sotto-step 5b, scritto)

Secondo pezzo del sotto-step 5/5 (lo swap vero): il rollback "automatico e
immediato al binario precedente" richiesto dalla specifica non puo'
aspettare una `cargo build`, che potrebbe anche essere quella appena
rivelatasi rotta.

Deciso insieme ad Alessio prima di scrivere codice: una copia del binario
compilato, non due cartelle di lavoro separate (blue/green) — piu'
semplice e meno spazio su un telefono. `scripts/salva-binario.sh` copia
`target/debug/gestionale-casa` in `data/run/binario_precedente`, e va
lanciato mentre quel binario corrisponde ancora al codice in esecuzione —
cioe' *prima* di aggiornare il codice e ricompilare per lo swap, non dopo
(lanciarlo dopo salverebbe il binario sbagliato).

`scripts/rollback-binario.sh` ferma quello che sta girando (necessario:
Telegram rifiuta un secondo long-polling con lo stesso token, 409
Conflict) ed esegue direttamente il binario salvato — nessuna build.
Stesso schema di processo di `avvia-bot.sh` (nohup+disown, `data/run/bot.pid`,
`data/run/bot.out`), cosi' `ferma-bot.sh` continua a funzionare dopo un
rollback senza saperne nulla. Il binario ripristinato torna in modalita'
normale (senza `RISERVATO`): era gia' la versione in produzione prima
dello swap fallito.

Timeout di avvio molto piu' corto di `avvia-bot.sh` (30s contro 180s): non
c'e' nessuna build da aspettare, se il binario gia' compilato non parte in
pochi secondi il problema e' un altro.

Nessun codice Rust coinvolto (due script bash). Collaudo reale sull'S9 nel
prossimo commit. Totale invariato: 280.

<!-- CHANGELOG_MODALITA_RISERVATA_COLLAUDO_20260904 -->
# 04/09/2026 — Sotto-step 5a collaudato per davvero: admin passa, bottone sblocca

La modalita' riservata descritta nella voce precedente e' stata collaudata
sul bot vero, sull'S9 vero. Aggiornamento sul ramo: 280 test verdi anche
sulla sua toolchain, backup del database, nessuna migration nuova da
provare. Bot avviato con `avvia-bot.sh --riservato`, log che conferma
"Avvio in modalita' riservata (RISERVATO=1)".

Alessio (amministratore principale) ha continuato a usare il bot
normalmente con la modalita' attiva: `/start` e navigazione, nessun avviso
di manutenzione. Nel menu' `🛠️ Amministrazione` e' comparso il bottone
`✅ Sblocca, torna online per tutti`; premuto, e' sparito dal menu' — segno
che il flag e' tornato a modalita' normale — senza errori nel log durante
la notifica "✅ Di nuovo online." alle chat attive. Bot fermato a fine
collaudo con `ferma-bot.sh`, spegnimento pulito.

Resta non collaudabile per davvero la meta' "blocca chi non e'
amministratore": serve un secondo account Telegram, che il progetto non ha
ancora (punto 6 aperto in `STATO.md`). Copertura solo via unit test per
quella parte, non un limite di questo collaudo.

Nessun test nuovo (nessuna modifica al codice, solo verifica). Totale
invariato: 280.

<!-- CHANGELOG_MODALITA_RISERVATA_20260904 -->
# 04/09/2026 — Modalità riservata (sotto-step 5a, meccanica scritta, collaudo sull'S9 in corso)

Primo pezzo del sotto-step 5/5 (lo swap vero), il più delicato del ciclo di
automazione: spezzato in quattro sotto-pezzi concordati con Alessio prima
di scrivere codice (5a modalità riservata, 5b rollback del binario, 5c
riepilogo/checklist/conferma pilotati dal bot stesso, 5d l'orchestrazione).

Decisioni prese insieme ad Alessio:

- **il flag vive in memoria**, non su file né come variabile letta una
  volta sola: `ModalitaRiservata` è un `AtomicBool` condiviso, stesso
  schema di `ShutdownController`. Deve poter tornare disattivo premendo un
  bottone in chat senza riavviare il processo — è proprio quel momento
  (la conferma dell'amministratore) a farlo tornare online per tutti;
- **chi pilota il riepilogo/checklist finale (punti 7-9 della specifica)
  è il bot nuovo stesso**, non l'agente orchestratore via API diretta come
  il countdown: a differenza del countdown (che deve aggiornarsi anche col
  vecchio processo fermo), qui il bot nuovo è già acceso e in ascolto su
  Telegram, quindi può gestire i suoi stessi bottoni come ogni altra
  schermata — arriva col sotto-step 5c.

`scripts/avvia-bot.sh --riservato` imposta `RISERVATO=1` solo per quel
lancio; un avvio normale (uso quotidiano su Termux, o senza il flag) resta
aperto a tutti come oggi. Il bot legge la variabile una volta all'avvio e
la tiene nel flag condiviso. Il gate (`deve_bloccare_per_manutenzione`,
pura e unit-testata) si applica sia ai messaggi sia ai callback, appena
risolta l'identità di chi scrive — prima di toccare qualunque handler dei
moduli. Il bottone `✅ Sblocca, torna online per tutti`, visibile solo
all'amministratore principale nel menù `🛠️ Amministrazione` quando la
modalità è attiva, disattiva il flag e manda "✅ Di nuovo online." a tutte
le chat di utenti attivi (`identity::list_active_chat_ids`, nuova — non
solo agli amministratori, a differenza della notifica di spegnimento).

**Limite noto, non un problema di questa modifica**: verificare per davvero
che un utente *non* amministratore veda l'avviso di manutenzione richiede
un secondo account Telegram, che il progetto non ha ancora — stessa lacuna
già segnata al punto 6 di `STATO.md`. La logica di blocco resta comunque
coperta da un unit test con i quattro casi possibili
(`manutenzione_blocca_solo_chi_non_e_amministratore_principale`). Il
collaudo reale sull'S9 verificherà quello che si può verificare oggi:
l'amministratore principale continua a usare il bot normalmente con la
modalità attiva, e il bottone la disattiva senza riavviare nulla.

280 test (279 prima), 1 nuovo.

<!-- CHANGELOG_SESSIONI_ATTIVE_COLLAUDO_20260904 -->
# 04/09/2026 — Sotto-step 4/5 collaudato per davvero: bot libero, sessione aperta, bot liberato

Il canale SIGUSR1 descritto nella voce precedente e' stato collaudato sul
bot vero, sull'S9 vero, non a tavolino. Prima di tutto l'S9 e' stato
aggiornato sul ramo: 279 test verdi anche sulla sua toolchain (diversa da
questa macchina e dalla CI), backup del database creato, la nuova migration
della schermata Distribuzione provata su una copia senza errori.

Bot avviato con `avvia-bot.sh`. A bot libero, `controlla-sessioni-attive.sh`
ha riportato subito "0 sessioni attive". Alessio ha poi aperto per davvero
`🚀 Distribuzione → ✏️ Cambia default → Countdown standard → ✏️ Altro
valore` sulla chat reale: lo script ha rilevato "1" a ogni ripetizione, e al
timeout di prova e' uscito con "procedo comunque" come previsto dalla
specifica. Liberata la sessione (valore scritto nella chat), il conteggio
e' tornato a "0". Bot fermato a fine collaudo con `ferma-bot.sh`.

Un bug reale trovato al primo giro, non a tavolino: lo script falliva con
"nessun file PID" anche a bot avviato. La causa era un `~` dentro una
variabile del PC (`CARTELLA_RUN_S9="~/gestionale-casa/data/run"`)
interpolata tra apici singoli nel comando remoto — l'espansione della tilde
vale solo a inizio parola durante il parsing della shell, non dentro un
valore gia' sostituito da un'altra variabile, quindi restava un carattere
`~` letterale e il file cercato non esisteva mai. Corretto usando un
percorso relativo (`data/run`), dato che il comando remoto fa gia' `cd
~/gestionale-casa` prima di usarlo.

Nessun test Rust coinvolto (il bug era in uno script bash). Totale
invariato: 279.

<!-- CHANGELOG_SESSIONI_ATTIVE_20260904 -->
# 04/09/2026 — Il bot sa dire se qualcuno sta scrivendo, su richiesta (meccanica scritta, collaudo sull'S9 in corso)

Sotto-step 4/5 del punto 6 del ciclo (deploy): il controllo pre-swap che
rimanda lo stop del bot finche' qualche chat ha una sessione "in attesa di
input testuale" attiva, deciso il 3 settembre. Le dieci mappe che tengono
quello stato (nove piu' `distribuzione_sessions` del sotto-step 3) vivono
solo nella memoria del processo Rust sull'S9: un controllo esterno via SSH
non puo' leggerle, serve un canale che il bot esponga.

Decisione presa insieme ad Alessio prima di scrivere codice: un segnale
Unix su richiesta invece di una scrittura periodica. Il bot ascolta
`SIGUSR1` (nuovo, non tocca il `SIGINT` gia' collaudato nel sotto-step 2 per
lo spegnimento pulito) e alla ricezione scrive `data/run/sessioni.txt` con
il numero di chat con una sessione attiva. Il nuovo
`scripts/controlla-sessioni-attive.sh` manda il segnale via SSH, legge il
file, e ripete con un intervallo fino a "0 sessioni attive" o a un timeout
massimo, oltre il quale procede comunque com'e' scritto nella specifica.

`SIGUSR1` non esiste su Windows: il canale (`avvia_ascolto_segnale_stato_sessioni`
e le funzioni che chiama) e' dietro `#[cfg(unix)]`, un'alternativa vuota lo
sostituisce sulle altre piattaforme, cosi' `fmt`/`check`/`test`/`clippy`
restano verdi in locale su questa macchina esattamente come prima — l'unico
posto dove il canale serve davvero e' l'S9.

Aggiunto `active_chat_ids()` a ciascuna delle dieci mappe di sessione (le
otto nei moduli piu' le due in `main.rs`): stesso schema di `has_active`
gia' presente in quattro di loro, ma senza un chat_id specifico — serve solo
al controllo pre-swap, non ai singoli handler.

Non ancora collegato a `ferma-bot.sh`: la sequenza reale (aspetta, poi
ferma, poi swap) arriva con il sotto-step 5.

<!-- CHANGELOG_VERIFICA_CI_SHA_20260904 -->
# 04/09/2026 — verifica-ci.sh poteva riportare "verde" leggendo il commit sbagliato

Trovato verificando la CI del push precedente (schermata Distribuzione,
sotto-step 3/5): subito dopo `git push`, `scripts/verifica-ci.sh` ha
stampato "OK CI verde" leggendo la run del **commit precedente**
(`7c730932`, gia' completata con successo), non di quello appena pushato
(`760089b`), che in quel momento era ancora `in_progress` e non ancora
comparso nell'API — `per_page=1` prende la run piu' recente per ramo senza
controllare a quale commit appartiene. Stesso errore del 2 settembre (un
riassunto letto verde dove non lo era), in una forma nuova: uno script
pensato apposta per non fidarsi di un riassunto locale si e' fidato di una
run del commit sbagliato.

Corretto confrontando lo `head_sha` dell'API con `git rev-parse <ramo>`: se
non coincidono la run viene trattata come "non ancora quella giusta" e lo
script continua ad aspettare. Ricollaudato per davvero sul push di questo
stesso commit: il controllo singolo ha riconosciuto correttamente la run
giusta (#77) ancora in corso, e `--attendi` ha aspettato ~80s prima di
riportare l'esito reale (success) della run del commit corretto.

Nessun test Rust coinvolto (lo script e' bash). Totale invariato: 279.

<!-- CHANGELOG_DISTRIBUZIONE_ADMIN_20260904 -->
# 04/09/2026 — Schermata admin 🚀 Distribuzione: il default della manutenzione

Sotto-step 3/5 del punto 6 del ciclo (deploy): la schermata
`🛠️ Amministrazione → 🚀 Distribuzione`, per configurare il default di
tipo/orario della manutenzione proposto a ogni deploy automatico (Subito /
Countdown standard / Programma orario), decisa il 3 settembre.

Schema dati concordato con Alessio prima di scrivere codice (due domande
puntuali, non deciso da solo): tabella a riga singola
`impostazioni_distribuzione` (`migrations/20260904150000_impostazioni_distribuzione.sql`),
con `CHECK` che tengono coerenti tipo e parametro (un `subito` non porta ne'
minuti ne' orario, un `countdown` richiede i minuti, un `programmato`
richiede l'orario). Le colonne `scelta_puntuale_*` esistono gia' nello
schema per la scelta puntuale del singolo deploy, ma non hanno ancora una
UI: arriva con il sotto-step 5, quando esiste un deploy vero che la offre —
costruirla ora sarebbe stata un'interazione senza niente da innescare.

L'inserimento dei valori (minuti del countdown, orario della manutenzione
programmata) e' ibrido su richiesta esplicita di Alessio: bottoni con
valori preimpostati (3/5/10/15 minuti; 02:00/03:00/04:00/05:00) piu' testo
libero validato (`valida_minuti`, `valida_orario` in
`src/modules/distribuzione.rs`, pure e testate). Per il testo libero serve
sapere quando un chat sta aspettando un valore: una decima mappa di
sessione indipendente in `main.rs` (`distribuzione_sessions`), sullo stesso
schema di `identity_sessions` — coerente con la decisione del 3 settembre
di non unificare le nove mappe esistenti prima del sotto-step 4. Ogni punto
del codice che azzera le altre nove per cambiare contesto azzera anche
questa.

Per il default "Programma orario" la schermata mostra anche il tempo
rimanente fino a quell'orario (es. "alle 03:00, tra 2h 30m"), letto
dall'ora locale di SQLite — stessa scelta gia' presa per il calendario,
perche' e' l'unico posto che conosce davvero il fuso orario del telefono.

Nel rispondere a una domanda di Alessio sul comportamento del countdown
gia' collaudato (sotto-step 1) durante un intoppo di rete, si e' trovato un
bug reale in `scripts/prova-countdown.sh`: il tempo mostrato veniva
decrementato di 1 a ogni giro del loop, non calcolato da una scadenza
fissa, quindi un tick piu' lento di un secondo (un retry di rete) faceva
restare il numero indietro rispetto al tempo vero senza mai recuperare.
Corretto calcolando il rimanente da `scadenza - ora_attuale` a ogni giro.
Ricollaudato per davvero sulla chat reale (10s): due `Recv failure:
Connection was reset` transitori assorbiti dai retry, countdown arrivato a
0 con i salti nel numero dovuti proprio ai due intoppi — comportamento
atteso, confermato da Alessio dopo la spiegazione. Messaggio di prova
ripulito a fine collaudo.

279 test (270 prima), 9 nuovi su `src/modules/distribuzione.rs`: validazione
minuti/orario e calcolo del tempo rimanente, incluso l'attraversamento della
mezzanotte.

Trovato usando `pipeline-locale.sh --commit` per la prima volta con un file
nuovo citato da un documento nello stesso commit: `controlla-documenti.sh`
verifica i rimandi sull'albero **tracciato da git** (`git ls-files`), ma la
pipeline lo eseguiva **prima** del `git add` — un commit legittimo che
aggiunge insieme un file e il documento che lo cita avrebbe sempre fallito.
Corretto spostando quel controllo dopo `git add` (solo in modalita'
`--commit`; fuori da quella modalita' resta dove'era, non c'e' nessun add
da aspettare), con `git reset` dei file se fallisce, cosi' niente resta
staged da un tentativo fallito.

<!-- CHANGELOG_NIENTE_PIN_20260904 -->
# 04/09/2026 — Tolto il pin: lascia una notifica fantasma che non si ripulisce

Trovato da Alessio guardando la chat vera dopo un collaudo del countdown: il
messaggio di prova era stato fissato (pin) e poi eliminato a fine collaudo,
ma restava comunque una riga "Gestionale_Bot pinned Deleted message" — una
notifica di sistema che Telegram genera per l'azione di pin, indipendente
dal messaggio pinnato, e che **non ha un `message_id` restituito dall'API
utilizzabile per eliminarla**.

Tolto il pin del tutto da `scripts/telegram-api.sh`: `tg_invia_e_fissa` e
`tg_sblocca` diventano `tg_invia` (senza `pinChatMessage`/
`unpinChatMessage`). Non serve fissare il messaggio per tenerlo "fermo": è
comunque l'unico messaggio che l'agente continua a modificare, e la chat
dell'amministratore principale — l'unico destinatario, verificato che
`tg_leggi_credenziali` cerca proprio quel chat_id — non ha altro traffico
nel mezzo durante un collaudo.

Riprovato su chat svuotata a mano da Alessio: countdown al secondo, nessuna
notifica fantasma. `docs/previsto/automazione-ciclo-sviluppo.md` aggiornato
nei punti 6 e 8 e nella sezione delle decisioni.

Nessun test Rust coinvolto. Totale invariato: 270.

<!-- CHANGELOG_GESTIONE_PROCESSO_20260904 -->
# 04/09/2026 — Avviare e fermare il bot da remoto, senza lasciarlo appeso alla sessione SSH

Sotto-step 2/5 del punto 6 del ciclo (deploy): `scripts/avvia-bot.sh` e
`scripts/ferma-bot.sh`. Oggi il bot vive solo dentro una sessione Termux
tenuta aperta a mano (`cargo run` in foreground) — non c'è nessun
supervisore di processo sull'S9, verificato prima di scrivere qualunque
cosa (né `tmux`, né `screen`, né un servizio). Deciso il 4 settembre:
`nohup ... & disown` con il PID salvato in un file, niente pacchetto nuovo
da installare — coerente con la scelta già fatta nel progetto contro
Docker/i container (`docs/architettura.md`, 2.4).

**`ferma-bot.sh` manda `SIGINT`, non `SIGTERM`** — letto nel codice prima di
agire, non per tentativi sul processo vero: in `main.rs` il dispatcher è
collegato solo a `.enable_ctrlc_handler()`, che ascolta SIGINT (lo stesso
del Ctrl+C interattivo). Un SIGTERM avrebbe ucciso il processo senza
passare dal percorso che manda "🔴 Gestionale Casa è offline." agli
amministratori.

Provato per davvero contro l'S9 vero, partendo da bot spento (confermato
con `ps`, non dato per scontato — un tentativo precedente di verificarlo
con `pgrep -f` aveva dato un falso positivo, la ricerca vedeva se stessa
nell'elenco processi):

- `avvia-bot.sh`: avvio in background, PID scritto su file, il processo
  sopravvive alla chiusura della sessione SSH (verificato con `ps`: nessun
  terminale di controllo). Riconosce l'avvenuto avvio cercando "Gestionale
  Casa online" nel log — la riga vera che il codice scrive dopo essersi
  collegato a Telegram, non un segnale indovinato;
- `ferma-bot.sh`: SIGINT, poi attesa fino a un timeout. Log confermato:
  `^C received`, `Dispatching has been shut down`, `Gestionale Casa
  offline`. File PID ripulito da solo.

Aggiunto anche `tg_elimina` a `telegram-api.sh` (API `deleteMessage`), per
ripulire dalla chat reale i messaggi di prova lasciati dai collaudi — non
fa parte del ciclo di deploy, è igiene dopo i test.

## Trovato collaudando, fuori da questo blocco

Il messaggio "ℹ️ Non sto aspettando un input in questo momento"
(`unexpected_input_notice` in `main.rs`) appare sotto la schermata
principale invece che vicino ad essa, spostandola fuori dalla vista —
limite strutturale di Telegram, non un bug banale (non si può inserire un
messaggio "sopra" uno esistente). Il meccanismo che lo fa sparire alla
prossima interazione (`cleanup_transient_media`) esiste già, ma non è stato
messo alla prova per davvero: nel collaudo del 4 settembre il bot è stato
spento subito dopo la sua comparsa. Segnato in `STATO.md` come miglioramento
a sé, non affrontato in questo commit.

Nessun test Rust coinvolto in questo commit. Totale invariato: 270.

<!-- CHANGELOG_COUNTDOWN_PINNATO_20260904 -->
# 04/09/2026 — Il countdown pinnato, e due bug di rete trovati provandolo per davvero

Sotto-step 1/5 del punto 6 del ciclo (deploy): `scripts/telegram-api.sh`
(`tg_leggi_credenziali`, `tg_invia_e_fissa`, `tg_modifica`, `tg_sblocca`) e
`scripts/prova-countdown.sh` per collaudarlo isolato, senza nessun deploy
vero attaccato. Deciso il 3 settembre: pilotato dall'agente via API diretta,
non dal bot sull'S9, perché deve continuare ad aggiornarsi anche quando il
processo S9 è fermo per lo swap.

Provato sulla chat reale dell'amministratore principale, non su un caso
finto. Prima versione: tick ogni 15s, confermato che il meccanismo di
pin/edit funziona ma il countdown "non si vedeva scendere". Corretto a un
tick al secondo — modificare non spamma la chat come inviare farebbe.

## Due problemi di rete, trovati provando

`curl --data-urlencode` su questa macchina (curl 8.21/mingw-w64) **corrompe
i caratteri non-ASCII**: una vocale accentata diventa `U+FFFD` prima ancora
di essere codificata, e Telegram rifiuta con "strings must be encoded in
UTF-8". Isolato con `https://httpbin.org/get` prima di incolpare Telegram.
Aggirato codificando il testo con Python (`urllib.parse.quote`, stesso
rilevamento `python3`/`python` già usato in `verifica-ci.sh` e
`controlla-documenti.sh`) e passandolo già pronto a curl con `--data`
semplice, che non lo ritocca più.

Un test da 30 tick ha incontrato **tre fallimenti di connessione
transitori** (`Recv failure: Connection was reset`) — non un limite di
frequenza di Telegram, la rete. `curl --retry 4 --retry-all-errors
--retry-delay 2` li assorbe da solo, e dal 7.66 rispetta anche l'header
`Retry-After` di un eventuale 429 senza doverlo leggere a mano: più semplice
di un retry scritto a mano, e il countdown è arrivato in fondo lo stesso
(un piccolo scatto visibile una sola volta, confermato irrilevante).

Nessun test Rust coinvolto. Totale invariato: 270.

<!-- CHANGELOG_COLLAUDO_REMOTO_20260904 -->
# 04/09/2026 — Il collaudo sull'S9 lanciato dall'agente, non più a mano da Termux

`scripts/collauda-remoto.sh`: punto 4 del ciclo. Si collega via SSH e lancia
`aggiorna-s9.sh --ramo <nome> --solo-controlli` — sempre `--solo-controlli`,
perché questo passo verifica che il codice sia sano (compilazione, Clippy,
test, migration di prova) e non deve mai avviare il bot: quello è il passo 6
(swap), non questo. Controlla anche che il ramo richiesto sia stato pushato
su GitHub prima di collaudarlo — l'S9 vede solo quello, non l'albero locale
del PC (STATO.md, sezione 4).

**Provato per davvero contro l'S9 vero**, non un caso finto: l'S9 era su
`ux-spazi-profilo`, lo script lo ha portato su `automazione-ciclo-sviluppo`,
compilato, passati 270 test (confermato uguale a quanto dichiarato in
`STATO.md` — lo confronto lo fa già `aggiorna-s9.sh` da solo), creato un
backup del database reale, verificata l'integrità, controllato che non ci
fossero migration pendenti, fermato prima dell'avvio. Il bot non è mai
partito.

L'S9 resta sul ramo appena collaudato invece di tornare a quello precedente:
comportamento voluto, lo stesso di quando lo lancia una persona — il passo 6
del ciclo (deploy vero) parte da questo stesso stato.

Nessun test Rust coinvolto in questo commit (non è codice Rust). Totale
invariato: 270.

<!-- CHANGELOG_PIPELINE_LOCALE_20260904 -->
# 04/09/2026 — La pipeline locale, e un bug di `controlla-documenti.sh` che c'era da settimane

`scripts/pipeline-locale.sh`: punto 2 del ciclo in
`docs/previsto/automazione-ciclo-sviluppo.md`. Stessa sequenza di
`.github/workflows/ci.yml`, stesso ordine (documenti → fmt → check → test →
clippy), in locale. `--commit FILE_MESSAGGIO FILE... [--push]` aggiunge,
committa e pusha **solo se tutti i controlli passano** — la regola "niente
commit o push se la pipeline fallisce" (STATO.md, sezione 7) diventa
meccanica invece che da ricordarsi ogni volta.

Provato in entrambi i sensi: pipeline vera, verde, 270 test; e pipeline fatta
fallire apposta (un file con formattazione sbagliata) — fermata su "Formato",
nessun commit creato, verificato con `git status` subito dopo.

## Un bug reale, non di oggi, trovato scrivendo lo script sopra

`scripts/controlla-documenti.sh` usava `os.path.normpath`. Su Windows quello
e' `ntpath`, che normalizza i separatori con `\`: un percorso valido come
`docs/architettura.md` restituito da `git ls-files` (sempre con `/`) smette
di essere uguale a se stesso dopo la normalizzazione, e **ogni** rimando del
progetto risultava "rotto" — compresi quelli mai toccati. Non si era mai
notato perche' lo script gira in CI su Linux, dove `os.path` **e'**
`posixpath` e il bug non esiste; in locale, su Windows, veniva semplicemente
scavalcato leggendo il diff a mano. Corretto usando sempre `posixpath`
esplicitamente, indipendentemente dal sistema operativo (Termux, CI, o
Windows in locale) — cosi' come lo script rileva anche da solo se `python3`
sul `PATH` e' lo stub Microsoft Store che non esegue nulla, e ripiega su
`python` (stessa soluzione gia' scritta in `verifica-ci.sh`).

Nessun test Rust coinvolto: nessun cambiamento al totale, **270**.

<!-- CHANGELOG_VERIFICA_CI_20260904 -->
# 04/09/2026 — Il primo pezzo dell'automazione: leggere la CI, non riassumerla

`scripts/verifica-ci.sh`, primo passo verificabile del ciclo descritto in
`docs/previsto/automazione-ciclo-sviluppo.md` (punto 5): legge l'ultima run
CI di un ramo direttamente dall'API di GitHub Actions. Nessuna scrittura,
nessun token — il repository è pubblico.

Provato su una run vera e non su un caso finto: `#70`, la run generata dal
commit precedente su questo stesso ramo. Letta correttamente sia mentre era
`in_progress` (osservato prima che finisse) sia dopo, `completed`/`success`.
Provati anche un ramo inesistente (nessuna run trovata, uscita 1) e
`--attendi` su una run già conclusa (esce subito, non aspetta il timeout).

**Un bug trovato scrivendolo, non dopo**: la prima versione passava la
risposta JSON dell'API sullo stdin dello stesso comando invocato come
`python - <<EOF` — cioè lo stdin già usato per lo script Python stesso.
Arrivava sempre vuota (`Expecting value: line 1 column 1`). Corretto
scrivendo lo script Python in un file temporaneo e passando la risposta
dell'API come secondo file, non su uno stdin conteso.

**Una trappola di Windows, la stessa già scritta in `claude/stato-reale-progetto.md`**:
`python3` esiste sul `PATH` di questa macchina ma è lo stub del Microsoft
Store, che esce con un errore invece di eseguire — `command -v python3` lo
trova comunque, perché esiste come file. Lo script ora verifica anche che
l'interprete risponda a `--version` prima di fidarsi, e ripiega su `python`.

Nessun test Rust coinvolto (non è codice Rust): nessun cambiamento al
totale, resta **270**.

<!-- CHANGELOG_AUTOMAZIONE_SPEC_20260903 -->
# 03/09/2026 — Specifica dell'automazione del ciclo dev → deploy, e una topologia da correggere

Nessun codice: solo documenti, apertura del ramo `automazione-ciclo-sviluppo`
e le prime verifiche prima di scrivere qualunque cosa.

Aggiunti `docs/previsto/automazione-ciclo-sviluppo.md` (il ciclo completo:
richiesta → scrittura → collaudo locale → push → collaudo S9 via SSH →
verifica reale della CI → deploy a downtime minimo in modalità riservata →
conferma funzionale → merge) e `docs/previsto/invio-miglioramenti-a-claude.md`
(il canale `📤 Invia a Claude` sui miglioramenti `da_fare`, complementare alla
richiesta in chat). Esplicitamente fuori scope, per ora: nodi di standby,
failover automatico fra macchine.

## Una cosa da correggere prima di iniziare

`docs/infrastruttura.md` descriveva un **PC fisso** distinto dal portatile,
dove dovrebbe girare l'agente — non esiste ancora. Verificato: questa sessione
gira **sul portatile stesso** (`galaxybookalessio`), che ha già Tailscale e
SSH verso l'S9 funzionanti (`ssh s9` risponde). Per ora il portatile fa da
host; la migrazione a un PC fisso futuro resta a basso costo (repository via
git, Rust/GCC da reinstallare, una chiave SSH **nuova e dedicata** per quel
PC, non quella del portatile).

Anche `ContextState` in `src/context_bot.rs`, citato nella specifica come il
posto dove vive lo stato "in attesa di input testuale", non lo è: tiene lo
storico di `💡 Migliora`. Lo stato vero vive in nove mappe indipendenti in
`main.rs` (`identity_sessions`, `food_sessions`, `profile_sessions`,
`recipe_sessions`, `improvement_sessions`, `container_sessions`,
`location_sessions`, `photo_sessions`, `sessions`).

## Decisioni prese, prima di scrivere codice

Quattro punti "Aperto" nei due documenti, con opzioni proposte e non decise
da soli:

- il messaggio pinnato (countdown, poi checklist) è pilotato dall'**agente**
  via API Telegram diretta, non dal bot sull'S9 — deve continuare ad
  aggiornarsi anche mentre il processo S9 è fermo per lo swap;
- tipo/orario della manutenzione: schermata
  `🛠️ Amministrazione → 🚀 Distribuzione`, default + scelta puntuale a ogni
  deploy;
- coda "in attesa di input testuale": si interrogano le nove mappe cosi'
  come sono, nessun refactoring preliminare, timeout massimo prima di
  procedere comunque;
- invii duplicati di `📤 Invia a Claude`: un flag `in_coda_claude` +
  timestamp sulla riga `miglioramenti`, non una tabella coda separata.

**Confermato, non è più un punto aperto**: `cargo test` non tocca mai il
database reale — ricerca su tutto `src/`, ogni `.connect(...)` nei test usa
`sqlite::memory:`, zero eccezioni. Il DB reale si raggiunge solo a runtime
via `DATABASE_URL`.

Nessun test nuovo, nessun cambiamento al totale: **270**.

<!-- CHANGELOG_LISTE_CI_ROSSA_20260902 -->
# 02/09/2026 — Un `format!` inutile, e quattro run rosse per accorgersene

Il ramo `ux-liste` aveva la CI **rossa da subito**, su tutte e quattro le run.
Compilava, i 270 test passavano, il formato era a posto e il controllo dei
documenti era verde: falliva soltanto **Clippy**, con un errore solo.

```text
error: useless use of `format!`
  --> src/modules/miglioramenti.rs:2176:26
   |   let mut lines = vec![format!(
   |       "{}",
   |       liste::intestazione("📦 Archivio miglioramenti", total, page)
   |   )];
```

Un `format!("{}", x)` attorno a una funzione che restituisce gia' una `String`,
scritto per non cambiare la forma `vec![format!(...)]` che c'era prima. Il
rimedio non e' il `.to_string()` suggerito da Clippy ma togliere il `format!`:

```rust
let mut lines = vec![liste::intestazione("📦 Archivio miglioramenti", total, page)];
```

## Perche' non se n'era accorto nessuno prima del runner

`useless_format` non e' un lint nuovo, ma **e' stato esteso** fra le versioni.
Il runner ha la **1.98**, l'ambiente in cui il codice viene scritto e provato ha
la **1.95** e non puo' aggiornarsi. Tre versioni di distanza bastano.

E' la seconda volta che questo scarto morde, dopo `drain_collect`. Il punto
aperto 1 di `STATO.md` lo diceva gia' — *un controllo locale che passa non e'
una prova se la toolchain non e' la stessa* — ma restava una nota generica: ora
porta i numeri veri delle tre toolchain e la conseguenza operativa.

## L'errore piu' grave non e' stato il lint

Il lint e' un refuso. L'errore vero e' che lo stato della CI e' stato
**riassunto invece che guardato**: uno strumento ha letto «verde» dove la pagina
diceva rosso, e su quella base era gia' stato consigliato di mergiare in `main`.
Un merge fatto su quel consiglio avrebbe portato la CI rossa sul ramo
principale.

Da qui in avanti, prima di ogni merge si apre la pagina della run e si legge
l'icona. E' scritto nel punto aperto 1.

## Nota su `rust-toolchain.toml`

Sembra la soluzione ovvia e **non lo e'**: sull'S9 `cargo` arriva da `pkg` di
Termux e non da rustup, quindi il file verrebbe semplicemente ignorato proprio
sulla macchina piu' fragile, mentre potrebbe costringere altri ambienti a
scaricare una versione che non riescono a prendere. La strada resta
`rustup update` sull'S9, con la CI come unico giudice.

Nessun test nuovo: **270**.

<!-- CHANGELOG_LISTE_CHIUSURA_20260902 -->
# 02/09/2026 — Cosa resta da fare, scritto dove si trova

Chiusura del blocco liste. Nessun cambiamento al codice: due buchi nei
documenti, tutti e due della stessa specie.

**`STATO.md` dichiarava «nessun ramo aperto» mentre `ux-liste` era aperto.**
E' la seconda volta che questo file sbaglia a dire quali rami esistono: la
prima dava `ux-convenzioni-telegram` per aperto quando era gia' mergiato.
Un fatto che cambia a ogni ramo non puo' stare scritto in un documento che si
aggiorna a mano, quindi adesso non c'e' piu' la risposta ma i due comandi che
la danno — `git branch -a` e `git log --oneline main..origin/<ramo>` — con
scritto cosa vuol dire ciascun esito. Un documento che dice «chiedilo a git»
non puo' invecchiare.

**I difetti visti durante i collaudi vivevano solo nella conversazione.** Tre
cose osservate sul bot e non ancora sistemate — il prompt della ricerca che
descrive meta' di quello che fa, la riga di navigazione della ricerca spezzata
su due righe, e C9 mai applicata allo Storico che e' la schermata con piu' date
di tutte — piu' una scelta rimasta aperta sulla riga di spiegazione quando il
risultato e' uno solo. Ora sono la **parte 4** di
`docs/convenzioni-telegram.md`, ognuna con il blocco della parte 3 a cui
appartiene. La regola del progetto e' che chi apre la cartella deve capire dove
siamo: vale anche per cio' che manca, non solo per cio' che c'e'.

Aggiunto anche, nella parte 3, che il collaudo si fa **due volte**: prima di
scrivere e dopo aver consegnato. Nel blocco liste il primo giro ha corretto C1
e il secondo ha trovato un difetto che i test non vedevano.

CI verde su tutti e tre i commit del ramo. Nessun test nuovo: **270**.

<!-- CHANGELOG_LISTE_PAGINAZIONE_COMPLETATA_20260902 -->
# 02/09/2026 — La paginazione unica non era unica

Aprendo lo `📜 Storico` sul bot, la riga di paginazione era ancora quella
vecchia: `1 / 21` e due frecce nude, invece di
`⬅️ Precedente | 1/21 | Successiva ➡️`. Le etichette degli eventi erano quelle
nuove, l'intestazione pure — solo la riga sotto era rimasta indietro.

**Nello storico ci sono due tastiere, e ne avevo convertita una sola.**
`history_list_keyboard` serve lo storico di un singolo oggetto;
`global_history_keyboard` serve lo `📜 Storico` del menu' principale, cioe'
proprio quello che si apre per primo. Il conteggio «sei posti» scritto nella
voce di ieri era sbagliato: nei quattro moduli delle liste le righe scritte a
mano erano **nove**, e ne erano rimaste indietro quattro.

## Perche' erano rimaste indietro

Non per distrazione: **la primitiva non ci entrava.** `riga_paginazione`
prendeva *il totale delle voci* e dava per scontate cinque voci per pagina, ma
tre di quei quattro punti non contano in voci da cinque:

- il selettore dei filtri dello storico ne mostra **sette** per pagina;
- la descrizione lunga di un miglioramento e' spezzata **a caratteri**, non a
  voci;
- la lista delle categorie in «cerca per ingredienti» conosce solo il numero di
  pagine.

Chi non poteva usarla si e' tenuto la propria riga, ed e' esattamente il modo in
cui una convenzione torna a divergere — cioe' il problema che il modulo doveva
chiudere. Una primitiva che non entra dove serve non unifica niente.

Ora `riga_paginazione` prende **il numero di pagine**, che e' l'unita' che tutti
i chiamanti conoscono, e `riga_paginazione_da_totale` resta per chi ha il
totale. Tutte e nove le righe dei quattro moduli passano di li'.

## Cosa resta fuori, dichiarato

Sei righe scritte a mano vivono ancora in moduli che **non** sono in questo
blocco: `spazi_membri`, `porzioni_profili`, `porzioni_ingredienti`,
`profili_alimentari` (due) e `planner_alimentare`. Passeranno con i blocchi
Spazi/Profilo e con il planner. Le frecce di `calendario.rs` non c'entrano: li'
una freccia che non porta da nessuna parte si spegne (C13), non e' una riga di
paginazione.

## Verificato sul bot, non dedotto

Questa volta l'esempio della ricerca e' stato controllato sul bot vero:
cercando `philadelphia` compare `Formaggio spalmabile` con la riga
`→ prodotto Philadelphia · Original`. E cercando `zucchine`, che i due risultati
li trova per nome, la riga non compare — che e' il comportamento giusto.
Controllato anche il contrario dell'esempio inventato di ieri: `Amido di mais`
non ha nessun prodotto associato.

**270 test** (erano 269).

<!-- CHANGELOG_LISTE_RICERCA_CORREZIONE_20260902 -->
# 02/09/2026 — Un esempio inventato, e il buco che nascondeva

Nella voce precedente e nel commento della funzione stava scritto che «cercando
*barilla* compare `Amido di mais`». **Non era vero: l'esempio era inventato.**
Non e' stato osservato su nessuna schermata ne' su nessun dato, e sul database
reale quella ricerca non restituisce quell'alimento. E' il divieto n.1 della
sezione 0 di questo progetto — non dichiarare fatto cio' che non e' stato
verificato — e va segnato perche' un esempio falso in un documento e' peggio di
un esempio mancante: chi lo legge lo prende per una verifica gia' fatta.

Al suo posto c'e' **Philadelphia su `Formaggio spalmabile`**, che non e'
inventato: e' l'esempio gia' presente in `docs/moduli/alimenti.md` ed e'
esercitato dal test `ricerca_alimenti_trova_anche_marca_e_nome_commerciale`.

## Il difetto che l'esempio finto copriva

Cercando l'esempio vero e' saltato fuori che la funzione era **incompleta**.
`list_foods_with_offset` fa comparire un alimento per tre strade — il nome, gli
**alias**, e marca piu' nome dei prodotti commerciali collegati — mentre la
riga di spiegazione ne copriva una sola, quella dei prodotti. Un alimento
trovato per alias restava inspiegato esattamente come prima del lavoro sulle
liste: il difetto che C1 doveva chiudere era chiuso a meta'.

Ora la riga copre tutte e due le strade e dice quale delle due e' stata:

```text
Perché sono nei risultati:
Formaggio spalmabile → prodotto Philadelphia · Original
Formaggio spalmabile → anche «Crema spalmabile»
```

Il nuovo test `la_ricerca_spiega_perche_un_alimento_e_nei_risultati` verifica
tutti e tre i casi su un database vero: trovato per prodotto, trovato per
alias, e trovato per nome — quest'ultimo non deve produrre nessuna riga, perche'
non c'e' niente da spiegare.

Corretto anche `Perche'` in `Perché` nel testo mostrato su Telegram: la
sezione 7 di `STATO.md` chiede accenti italiani corretti, e l'apostrofo va bene
nei commenti del codice, non in quello che legge una persona.

**269 test** (erano 268).

<!-- CHANGELOG_LISTE_CONVENZIONI_20260902 -->
# 02/09/2026 — Le liste smettono di dire due volte la stessa cosa

Secondo blocco della parte 3 di `docs/convenzioni-telegram.md`: **le liste**,
tutte e quattro insieme — alimenti, ricette, storico, miglioramenti — perche'
C11 chiede che le sezioni gemelle si somiglino, e sistemarne due su quattro
avrebbe lasciato il bot piu' incoerente di prima.

## Cosa si e' visto guardando il bot, e non si vedeva dal codice

Il giro sul bot reale ha cambiato due decisioni.

**C1 sbagliava a dire chi fosse «Giorgia».** La convenzione descriveva i tre
pulsanti identici `🍽 Porzione modificata · Giorgia` e proponeva come rimedio
`[ 🍽 Porzione · Giorgia · 31/08 19:49 ]`, trattando *Giorgia* come l'autore.
Sul bot l'autore era `Alessio Lari` per tutti gli eventi della pagina, e
*Giorgia* e' il **profilo su cui la porzione e' stata modificata**, cioe'
`nome_entita_snapshot`, che sull'etichetta c'era gia'. La ricetta scritta in C1
aggiungeva al pulsante una cosa che c'era e continuava a non aggiungere quella
che mancava. La convenzione e' stata corretta.

**Data e ora da sole non bastavano.** Due eventi che sembravano doppioni erano
due modifiche opposte della stessa porzione — `Pasta test 120% → 100%` e
`100% → 120%` — fatte nello stesso minuto, sullo stesso profilo, dalla stessa
persona. Applicando C1 alla lettera restavano due pulsanti identici. La scelta
presa: sull'etichetta vanno **quando** e **su cosa**, il resto sta nel
dettaglio; due eventi nello stesso minuto restano adiacenti e in ordine, e si
distinguono aprendoli. E' scritto in C1 come limite noto, invece di essere
scoperto di nuovo fra sei mesi.

## Un modulo solo per le liste

Nasce `src/modules/liste.rs`, per la stessa ragione per cui esiste un solo
`calendario.rs`: la riga di paginazione era riscritta a mano in **sei posti**,
(il conteggio e' sbagliato: erano nove nei soli quattro moduli, e quattro sono
rimaste indietro fino alla voce successiva)
con **quattro etichette diverse** per lo stesso pulsante — `⬅️ Pagina
precedente`, `⬅️`, `Pagina successiva ➡️`, `➡️` — e due formati di
intestazione (`· 422 risultati` + `Pagina 1/85` contro `Pagina 1 di 21 · 102
eventi`). Una convenzione ricopiata a mano in sei posti non e' una convenzione.

Ora la riga e' una sola, `⬅️ Precedente | n/tot | Successiva ➡️`, con `n/tot`
non premibile, e sparisce del tutto quando c'e' una pagina sola.

## C1 applicata: il testo non ripete piu' i pulsanti

In tutte e quattro le liste il messaggio si riduce a cio' che i pulsanti non
possono dire — quante voci ci sono, dove siamo, quale filtro e' attivo — e
tiene solo tre eccezioni, tutte informazioni che sui pulsanti non stanno:

- la **legenda** `👤 tuo · 👥 condiviso`, mostrata solo quando in pagina c'e'
  almeno un marcatore da spiegare;
- l'**ordinamento** degli alimenti, che non e' alfabetico (prima i tuoi, poi i
  condivisi, poi il catalogo base) e che C6 impone di dichiarare. Le ricette
  sono ordinate alfabeticamente e quindi non dichiarano niente;
- nella ricerca alimenti, **perche'** un alimento e' fra i risultati quando il
  suo nome non contiene la parola cercata — e' finito li' per un alias o per un
  prodotto commerciale collegato, e senza quella riga sembra un errore del bot.
  (La prima stesura di questa voce portava qui un esempio inventato e una
  funzione incompleta: vedi la voce successiva, del 2 settembre.)

E' sparita «Tocca un evento sotto per vedere il dettaglio» (C2).

## C6 e C7 sui menu'

`🥕 Alimenti` con 422 voci offre ora `🔎 Cerca` come prima azione e
`📋 Elenco alimenti · 422` come seconda: sfogliare 85 pagine non e' una strada.
Sotto le 20 voci l'ordine resta quello di prima, perche' con poche voci
scorrere e' piu' veloce che scrivere.

`💡 Miglioramenti` era l'esempio da cui nasce C7: i conteggi c'erano gia', ma
in un blocco di testo sopra i pulsanti — «🟡 Da approvare: 0» sopra un pulsante
`🟡 Da approvare` — insieme a «Usa i pulsanti qui sotto», che C2 vieta. Ora il
numero e' sull'etichetta e quel blocco non esiste piu'.

## Effetto collaterale: due liste chiedono meno al database

Togliere dal testo cio' che nessuno mostrava piu' ha tolto anche le letture che
lo producevano, e su un S9 non e' un dettaglio:

- lo **storico** leggeva otto colonne per riga — luogo, stanza, contenitore,
  spazio, autore, origine, tipo entita' — cinque righe per pagina, a ogni
  apertura, per non mostrarle;
- le **ricette** calcolavano due sotto-query correlate per riga (`COUNT` degli
  ingredienti e degli step): **dieci `COUNT`** a ogni apertura di una pagina da
  cinque, per una riga di testo che ora sta nel dettaglio.

## Verifica

`fmt`, `clippy --all-targets -- -D warnings` e `test` verdi: **268 test**
(erano 248). I nuovi coprono la riga di paginazione ai bordi, il contatore non
premibile, il conteggio sull'etichetta, l'accorciamento della data, e il caso
che ha generato il lavoro: due eventi dello stesso tipo in momenti diversi non
producono piu' la stessa etichetta.

Nota sulla toolchain: `div_ceil` sugli interi con segno e' ancora instabile
sulla versione dell'S9, e la divisione scritta a mano fa scattare
`manual_div_ceil` sulla Clippy della CI, che e' piu' recente. Il conto si fa
senza segno, dove `div_ceil` e' stabile e nessuna delle due protesta.

<!-- CHANGELOG_SQLX_RINVIO_20260902 -->
# 02/09/2026 — sqlx 0.9.0 rinviata, con il perche'

La PR #6 di Dependabot (sqlx 0.8.6 → 0.9.0) era aperta da giorni come «da
valutare». Valutata e **rinviata**: la decisione e' scritta nei punti aperti di
`STATO.md`, perche' un rinvio senza motivo scritto e' solo una cosa dimenticata.

**Non e' un aggiornamento di sicurezza**: RUSTSEC-2024-0363 e' chiusa dalla
0.8.1, e la 0.9.0 e' una release con rotture di compatibilita' (6 maggio 2026).

Costo misurato sul nostro codice:

- **Rust 1.94 come minimo**, mentre la toolchain dell'S9 e' piu' vecchia: si
  finirebbe per cambiare libreria e toolchain insieme sulla macchina piu'
  fragile;
- **27 query costruite dinamicamente** da riscrivere con `AssertSqlSafe`, in
  `luoghi.rs`, `contenitori.rs` e `ricette.rs`, perche' le funzioni ora
  accettano solo `&'static str`;
- **cambiamenti significativi al trait `Migrate`**, cioe' proprio alla parte su
  cui poggiano 42 migration immutabili gia' applicate a un database vivo.

Il progetto non usa le macro `query!` a compile-time (zero occorrenze), quindi
quella meta' delle rotture non ci tocca.

Rientra in agenda se esce una vulnerabilita' sulla 0.8.x o se serve `sqlx.toml`.
Da rifare comunque dopo l'aggiornamento della toolchain dell'S9, per tenere un
cambiamento alla volta.

<!-- CHANGELOG_CI_MAIUSCOLE_20260902 -->
# 02/09/2026 — La CI su tutti i rami, e due nomi di file sbagliati

## Sei push senza nessun controllo

La CI partiva su `push: branches: [main, 'step-*']`, un filtro nato quando i
rami si chiamavano `step-7-...`. Il ramo `ux-convenzioni-telegram` non
combaciava: **sei push di seguito non hanno fatto partire niente.** Il verde
arrivava solo dalla pull request, cioe' quando gli errori da bisezionare erano
gia' sei.

Un filtro sui nomi dei rami e' una rete che si buca appena si cambia
convenzione di nome. Ora la CI gira su `branches: ['**']`, e un blocco
`concurrency` con `cancel-in-progress` tiene solo l'ultimo controllo di una
raffica di push allo stesso ramo: e' meta' del costo di girare su tutti.

## Due nomi di file sbagliati, e come sono nati

Il riordino dei documenti e' stato committato dal PC, dove il filesystem **non
distingue le maiuscole**. Estratto l'archivio e dato `git add -A`, Git ha visto
`docs/infrastruttura.md` come lo *stesso percorso* di `docs/INFRASTRUTTURA.md`
e ha tenuto il nome vecchio con il contenuto nuovo. Idem per `ROADMAP.md`.

Su Linux — cioe' su GitHub e nella CI — sono due file diversi, e quattro
documenti rimandavano al nome minuscolo, che non esisteva.

La verifica «zero rimandi rotti» era stata fatta sulla **copia di lavoro** in un
ambiente Linux, non sull'albero committato da Windows. Era vera dove e' stata
fatta e falsa dove conta.

## `scripts/controlla-documenti.sh`

Gira in CI prima ancora di `cargo fmt`, non serve rete ne' cargo, e verifica due
cose sull'albero **tracciato da Git**, non sui file su disco:

1. **nessun rimando rotto**: ogni `percorso/file.md` citato in un documento
   deve esistere. Il `CHANGELOG.md` e' escluso: e' un verbale, e le voci
   vecchie citano percorsi dell'epoca;
2. **nessuna coppia di percorsi che differisce solo per maiuscole**, che su
   Windows e macOS e' un file solo e su Linux sono due.

Provato su entrambi i casi: sul commit rotto trova i sei rimandi, e su una
collisione di maiuscole introdotta apposta la segnala.

<!-- CHANGELOG_DOCUMENTI_20260902 -->
# 02/09/2026 — I documenti riordinati, e la regola che li tiene allineati

**55 file markdown, 413 KB, e quattro documenti che descrivevano il presente
dicendo cose diverse.** Tre su quattro mandavano il lettore su
`step-7-alimentazione`, cioe' sul ramo con il 7.3B **scartato** il 31 agosto.

Il `README.md` — il file che chiunque apra il repository vede per primo — era
sbagliato quasi in ogni riga: quel ramo, la baseline `34d076c`, 36 migration
(sono 42) e «prossimo lavoro: Porzioni e override», chiuso da giorni.

## La regola

In testa a `STATO.md`, sezione 0:

> **Nessuna modifica e' finita finche' i documenti non la raccontano.**
> Il metro: una persona che apre questa cartella senza aver visto nessuna
> conversazione deve poter capire dove siamo, cosa fa il programma e perche' e'
> fatto cosi'.

Con la tabella di cosa aggiornare a ogni tipo di modifica e tre divieti, ognuno
nato da un errore realmente commesso: non dichiarare fatto cio' che non e'
stato verificato; quello che viene scartato esce dai documenti del presente; un
fatto sta in un posto solo.

**E un controllo automatico.** `aggiorna-s9.sh` confronta il numero di test
appena eseguiti con quello dichiarato in `STATO.md` e avvisa se non
coincidono: e' il fatto piu' facile da lasciare indietro, ed e' anche quello
che segnala se sul telefono e' arrivato il codice giusto.

## La struttura

Da 55 file a **29**. Un documento per domanda, e la mappa nel `README.md`.

```text
STATO.md                        l'unico documento del presente
CHANGELOG.md                    cosa e' cambiato e quando
docs/architettura.md            perche' e' fatto cosi'
docs/storico-del-progetto.md    perche' non e' fatto in un altro modo
docs/convenzioni-telegram.md    come deve comportarsi ogni schermata
docs/database.md                schema + regola delle migration
docs/condivisione.md            spazi, ruoli, permessi
docs/infrastruttura.md          server, script, CI
docs/moduli/                    cio' che ESISTE
docs/previsto/                  cio' che e' solo specificato
```

La distinzione che mancava di piu' non era presente/passato ma **fatto /
previsto**: acquisti, viaggi, spese, reminder e turni avevano lo stesso aspetto
dei moduli operativi, e un lettore nuovo credeva che il gestionale facesse
molto piu' di quello che fa.

Sono spariti: `HANDOFF_COMPLETO.md` (56 KB), `docs/ROADMAP.md`, i dodici file
`step-7.2x.md`, i due `ricette.md` di cui uno era solo un rimando, e le
duplicazioni letterali fra `contenitori.md`, `navigazione-luoghi.md` e
`luoghi.md`. Restano tutti recuperabili con `git log --follow`.

## Cosa e' emerso rileggendo tutto contro il codice

Il riordino ha fatto trovare cose che nessuno cercava, verificate nel sorgente
e non nei documenti:

- **`item_condivisioni` e' schema morto**: la tabella esiste, ma **zero
  riferimenti** in `src/`. Lo stesso per `tag`, `item_tag` e `promemoria`.
  Ora e' scritto in `docs/database.md`: prima di riusarle va verificato che il
  modello sia ancora quello giusto, invece di darlo per buono perche' la
  tabella c'e'.
- **Ricette, planner e spazi non scrivono nello storico**, benche' i documenti
  dichiarassero come principio che ogni modifica condivisa debba essere
  attribuibile. Lacuna nota, ora documentata in `docs/moduli/storico.md`.
- **Porzioni e Planner erano dichiarati da costruire** in tre documenti diversi
  mentre erano in produzione da giorni.
- **`docs/condivisione.md` si contraddiceva da solo**: l'intestazione diceva
  «restano da implementare» e due sezioni piu' sotto dello stesso file
  descrivevano le stesse funzioni come realizzate.
- **`schema-core.md` diceva che le Ricette usano `items`**: hanno invece una
  tabella propria, e `items.tipo` ammette un valore `'ricetta'` mai usato.
- **Lo Storico dichiarava 6 eventi per pagina**, il codice ne mostra 5.
- Conteggi dei test fossilizzati a 37, 69 e 153 sparsi in sei documenti.

## Il planner aveva un buco

Non esisteva un documento del modulo piu' usato: era descritto dentro
`pianificazione-e-spesa.md`, marcato PREVISTO, insieme alla lista della spesa
che invece non esiste davvero. Ora c'e' `docs/moduli/planner.md`, e la lista
della spesa resta in `docs/previsto/`.

## Verifica

- **zero rimandi rotti** fra i documenti del presente, controllati uno per uno;
  i dieci rimasti sono nelle voci storiche del CHANGELOG, che e' un verbale e
  come tale resta scritto com'era;
- pipeline verde, 248 test: nessuna modifica al codice tranne il controllo
  aggiunto allo script.

<!-- CHANGELOG_CALENDARIO_20260902 -->
# 02/09/2026 — Un calendario solo, e il planner impara a saltare

Il calendario delle scadenze degli inviti era la cosa meglio riuscita
dell'interfaccia: spaziale invece che testuale, un tocco per una data, i limiti
visibili invece che spiegati. Il modo giusto di riusarlo non era copiarlo.

## Due calendari gregoriani nello stesso progetto

`spazi_membri.rs` conteneva una implementazione a mano delle regole del
calendario — congruenza di Zeller per il giorno della settimana, anni
bisestili, giorni del mese — accanto a quella basata su `chrono` introdotta il
1 settembre nel planner. Due copie delle stesse regole in due moduli sono due
occasioni di divergere.

Ora c'e' **`src/modules/calendario.rs`**: le primitive sulle date e la griglia
del mese, in un posto solo. Gli inviti e il planner lo usano entrambi; il
codice scritto a mano e' stato tolto.

La griglia si configura con: la data di oggi, una funzione che per ogni giorno
dice se e' selezionabile e se porta un **marcatore**, e un mese minimo
opzionale. Il marcatore e' la parte che la rende piu' di un selettore di date:
il calendario puo' mostrare *cosa c'e'* nei giorni.

## `📅 Vai a una data` nel planner

Per raggiungere una settimana di tre mesi avanti servivano dodici pressioni
della freccia. Ora c'e' la griglia del mese, e i giorni che hanno gia' dei
pasti portano un `•`:

```text
|  ⬅️   |Settembre 2026|  ➡️   |
| Lun | Mar | Mer | Gio | Ven | Sab | Dom |
|  ·  |[1] •|  2  |  3  | 4 • |  5  |  6  |
|  7  |  8  |  9  | 10  | 11  |12 • | 13  |
```

I conteggi del mese arrivano da **una sola query**, non una per giorno.

## Due difetti del calendario originale, corretti

- **Oggi non era segnalato.** In un calendario e' il riferimento principale.
  Ora e' il numero fra parentesi quadre. Niente emoji: su sette colonne
  allargherebbe la cella, e `·` era gia' il riempitivo dei giorni fuori dal
  mese — un simbolo, un significato.
- **`⬅️ ❌`** per la freccia disattivata faceva sembrare che «indietro» fosse
  rotto. Una freccia che non porta da nessuna parte si spegne e basta.

## Una trappola, chiusa dentro il modulo

I campi della configurazione sono `&dyn Fn`, che senza `+ Sync` non e' `Send`:
una schermata che tiene la configurazione a cavallo di un `.await` produce un
future non-`Send`, e teloxide lo rifiuta con un errore che parla di
`Injectable` e non nomina mai `Send`. Il `+ Sync` e' nella firma del modulo, con
il commento che spiega perche', cosi' chi lo usera' dopo non ci ricasca.

## Tre correzioni dopo averlo visto sul telefono

La griglia sta bene anche su schermo stretto: sette colonne leggibili, numeri
chiari. Ma guardandola sono uscite tre cose che dal codice non si vedevano.

- **Si apriva sul mese sbagliato.** Il pulsante portava il mese dell'inizio
  settimana, e la settimana 31/08 → 06/09 apriva agosto: il 2 settembre, cioe'
  oggi, non era nemmeno nella schermata, e il marcatore di oggi si perdeva
  proprio all'apertura. Ora vale la regola dei numeri di settimana ISO: la
  settimana appartiene al mese in cui cade il **giovedi'**.
- **Il mese era scritto due volte**, nel testo del messaggio e
  nell'intestazione della griglia. E' la convenzione C1, e l'avevo violata io
  scrivendola. Nel testo resta solo la legenda del `•`, e solo quando c'e'
  almeno un giorno da spiegare.
- **`gia'` invece di `già`** nella legenda, contro la regola del progetto sugli
  accenti italiani corretti.

## Verifica

- pipeline verde: fmt, clippy `-D warnings`, **248 test** (da 236);
- 11 test nuovi sulla griglia e sul mese della settimana: sette colonne per riga, oggi marcato una volta
  sola, marcatore accanto al numero, giorno bloccato non premibile, freccia che
  si spegne al limite, bisestili con le eccezioni secolari;
- nessuna migration.

<!-- CHANGELOG_UX_TELEGRAM_20260901 -->
# 01/09/2026 — Convenzioni dell'interfaccia, e primo giro di correzioni

Primo giro completo del bot fatto guardandolo davvero, schermata per schermata,
sull'istanza in esecuzione sull'S9. Il risultato e' `docs/convenzioni-telegram.md`:
cosa e' stato trovato, e dodici convenzioni per non ritrovarlo.

## La scoperta

I problemi visti nel planner **non erano del planner**. Sette degli otto sono
sistemici e si ripetono in ogni sezione. Il piu' diffuso e' che **il testo del
messaggio ripete i pulsanti sottostanti**: in `Elenco alimenti` cinque nomi nel
testo e gli stessi cinque nei pulsanti, identici.

Il caso che fa piu' danno e' lo Storico: nel testo ogni evento ha data, ora e
autore, nel pulsante resta solo il titolo. Risultato: tre pulsanti identici
`🍽 Porzione modificata · Giorgia`, distinguibili solo contandoli e
confrontandoli con il testo sopra. **L'informazione che distingue le righe era
finita nel posto dove non si puo' premere.**

## Planner

- **Vista settimana.** Sparisce l'elenco dei giorni nel testo, che ricompariva
  identico nei pulsanti. I pulsanti portano conteggio e stato; il testo dice
  l'unica cosa che i pulsanti non possono dire, cioe' **cosa si mangia oggi**.
- **Oggi e' segnalato** con `👉`, nella settimana e nel giorno. Era il
  riferimento piu' utile della schermata e non c'era.
- **Un giorno vuoto tace** invece di scrivere `0 pasti`: su una settimana quasi
  libera si vedevano sette righe di niente. Se la settimana e' vuota lo dice una
  riga sola, con cosa fare.
- **Date compatte** nei pulsanti (`Lun 31/08`): dentro una settimana l'anno e'
  lo stesso su tutte e sette le righe e non distingue nulla, mentre lo spazio
  serviva al marcatore di oggi.
- **Vista giorno**: i pasti stanno solo sui pulsanti, non piu' anche nel testo.
- **Concordanze**: il soggetto e' *il pasto*, quindi `✅ Segna come consumato` e
  `⏭ Segna come saltato`. Erano al femminile.
- **Un simbolo solo per stato**: il dettaglio diceva `📅 pianificata` mentre le
  liste dicono `○`. Ora dicono entrambi `○`.
- `➕ Aggiungi pasto` diventa `➕ Nuovo pasto`: creare si dice in un modo solo.

## Il bug dietro le righe di navigazione

`💡 Migliora` non e' scritto nelle tastiere: lo inserisce `context_bot.rs`,
cercando nell'ultima riga il pulsante che porta al menu' principale e mettendosi
davanti a quello. In Alimentazione e Storico **sia `⬅️ Indietro` sia `🏠 Menu'
principale` puntavano a `menu:main`**, e la ricerca partiva dall'inizio: trovava
l'Indietro, e Migliora finiva davanti a tutto. Da li' venivano le righe
`Migliora | Indietro | Menu'`, diverse da ogni altra schermata.

Corretto cercando dal fondo (`rposition`). E poiche' in una sezione di primo
livello «indietro» e «menu' principale» sono lo stesso posto, l'`⬅️ Indietro`
ridondante di Alimentazione e Storico e' stato tolto: la regola ora e' scritta,
ed e' che quel pulsante esiste solo se porta da qualche altra parte.

## Menù principale

- **Ordinato per uso e non per architettura**: prima Alimentazione, Oggetti e
  Case, poi lo Storico, poi Profilo e Spazi, in fondo gli strumenti. Lo Storico
  era il primo pulsante del gestionale.
- **I moduli non disponibili non compaiono.** `👕 Vestiti · prossimamente` e
  `🚗 Veicoli · prossimamente` occupavano una riga senza fare niente e
  costringevano il messaggio a spiegare cosa volesse dire «prossimamente».
  Restano previsti e sono scritti in `docs/roadmap.md`, con i moduli
  segnaposto che gia' esistono.
- `💡 Miglioramenti` diventa **`📋 Miglioramenti`**: c'erano due lampadine nello
  stesso menu', con nomi quasi uguali e funzioni diverse. `💡 Migliora` resta
  quello che segnala un problema sulla schermata corrente.
- Via la frase che spiegava «prossimamente», ormai senza oggetto.

## Frasi che leggevano i pulsanti ad alta voce

- `🏷️ Oggetti generici` diceva: «Scegli cosa vuoi fare. Usa i pulsanti per
  scegliere cosa fare.» La stessa frase due volte nella stessa riga. Ora la
  schermata si chiama `🏷️ Oggetti`, come il pulsante che ci porta, e non dice
  altro.
- `🥕 Alimenti` elencava a parole i quattro pulsanti sottostanti e aggiungeva
  «i dati vengono riletti automaticamente ogni volta che apri questa sezione»:
  un dettaglio di implementazione che crea un dubbio che l'utente non aveva.

## `aggiorna-s9.sh --ramo`

Consegnando questo lavoro su un ramo nuovo, sull'S9 non e' arrivato niente: lo
script aggiorna soltanto il ramo su cui si trova gia'. Il collaudo e' girato
sul codice di prima, verde, senza che nulla lo segnalasse — la stessa forma
dell'errore del ramo vuoto di poche ore prima.

```bash
./scripts/aggiorna-s9.sh --ramo ux-convenzioni-telegram
```

Dopo il `git checkout` lo script **si riavvia da solo**: bash legge il file man
mano che lo esegue, e il cambio di ramo puo' averlo appena riscritto;
proseguire significherebbe eseguire meta' della versione vecchia e meta' di
quella nuova. Il riavvio riparte dalla versione appena scaricata e tiene lo
stesso file di log.

## Verifica

- pipeline verde: fmt, clippy `-D warnings`, **236 test**;
- `--ramo` provato su un repository finto: cambio di ramo, riavvio, file del
  ramo nuovo presenti, un solo log; piu' gli argomenti sbagliati che si
  fermano con un messaggio invece di essere ignorati;
- nessuna migration: nessuna modifica ai dati;
- il resto — liste lunghe, Spazi e Profilo, conteggi sui pulsanti — e' nella
  parte 3 del documento delle convenzioni, in ordine di applicazione.

<!-- CHANGELOG_BUILD_S9_20260901 -->
# 01/09/2026 — Il linker che segfaultava, e perche'

Un `cargo run` dato a mano sull'S9 falliva spesso cosi':

```text
error: linking with `cc` failed: exit status: 1
  = note: "cc" ".../symbols.o" "<257 object files omitted>" ...
          cc: error: unable to execute command: Segmentation fault
error: could not compile `gestionale-casa` (bin "gestionale-casa")
```

Sembrava casuale — riprovando spesso passava — e la riga finale non diceva
niente di utile.

## La causa

**`257 object files`.** E' il numero che risolve il caso: 256 unita' di codegen
piu' `symbols.o`, cioe' esattamente il default di cargo per il profilo `dev`.
Ma il progetto girava sull'S9 con `codegen-units = 16`. Quindi in quella
compilazione le impostazioni anti-memoria **non erano attive**.

Non lo erano perche' vivevano soltanto come variabili d'ambiente esportate al
passo 2 di `scripts/aggiorna-s9.sh`. Chi lancia `cargo run` a mano non passa da
li' e ottiene i default: 256 unita' di codegen e le informazioni di debug
complete. Il linker si trovava 257 file oggetto pieni di debuginfo da unire, su
un telefono, e finiva la memoria a meta' collegamento. Su Android il sintomo non
e' un messaggio chiaro: e' un segmentation fault di `cc`.

Il "riprovando passa" era la memoria libera del telefono che cambiava fra un
tentativo e l'altro, non una compilazione difettosa.

## La correzione

Le impostazioni che proteggono il collegamento sono state spostate **nel
progetto**, dove valgono per chiunque compili, comunque lo faccia:

- `Cargo.toml`, profili `dev` e `test`: `debug = 0` e `codegen-units = 16`;
- `build.rs`: emette `-Wl,--threads=1` **solo quando il target e' Android**,
  cosi' non tocca ne' il PC ne' la CI. Senza, LLD lancia un thread per core e
  ognuno tiene la propria copia delle strutture di link.

  Sta in `build.rs` e non in `.cargo/config.toml` per due motivi: e'
  condizionato al target senza doverlo nominare, e non puo' essere annullato da
  una `RUSTFLAGS` impostata nell'ambiente, che sostituirebbe il file di
  configurazione invece di aggiungersi.

Effetto misurato sullo stesso codice:

```text
file oggetto da linkare     257  →  ~17
binario di debug           183 MB →  40 MB
```

Nello script restano solo le impostazioni che dipendono dalla macchina
(`CARGO_BUILD_JOBS`, `CARGO_INCREMENTAL`), piu' un avviso se `RUSTFLAGS` e'
impostata nell'ambiente: sostituisce la configurazione di cargo invece di
aggiungersi, ed e' il primo posto dove guardare quando due compilazioni si
comportano in modo diverso senza motivo apparente.

## La lezione

Una protezione che funziona solo se ti ricordi di usare lo script giusto non e'
una protezione. Se una impostazione serve a far compilare il progetto, il posto
giusto e' il progetto.

## Manutenzione: backup e log

Nello stesso passaggio, `aggiorna-s9.sh`:

- tiene **solo gli ultimi 5 backup** del database e rimuove le copie
  `gestionale_prova_*` orfane lasciate da esecuzioni interrotte;
- scrive il **log completo dei controlli** in `data/log/`, ruotato a 5.
  L'avvio del bot resta fuori dal log di proposito: un bot acceso per ore lo
  farebbe crescere senza limite, cioe' il problema che stiamo risolvendo;
- **controlla lo spazio libero** prima di partire e avvisa sotto 1,5 GB;
- quando cargo fallisce, ripesca dal log le prime righe `error`, ricontrolla lo
  spazio e riconosce i due casi che non sono errori di codice: disco pieno o
  processo ucciso, e linker fermato per memoria.

Questo log e' il motivo per cui l'errore del linker e' stato finalmente
diagnosticabile: la riga che conta era scorsa via dallo schermo di Termux.

<!-- CHANGELOG_STEP7_3_CALENDARIO_20260901 -->
# 01/09/2026 — Aritmetica delle date in Rust

Punto aperto n. 3 dell'handoff, chiuso. Non aggiunge funzioni: toglie lavoro
inutile al telefono.

## Il problema

`planner_show_week` chiedeva a SQLite ogni singolo conto di calendario:
l'inizio settimana, la fine settimana, la settimana precedente, quella
successiva, e poi, per ognuno dei sette giorni, la data del giorno e il nome
del giorno della settimana. **Diciannove query per aprire una schermata**, di
cui diciassette non leggevano alcun dato: calcolavano soltanto date. Su un
Galaxy S9 che fa da server, ogni query e' un round-trip attraverso il pool di
connessioni per ottenere una cosa che il processo sapeva gia' fare da solo.

Lo stesso schema era nella vista giorno (tre query di calendario), nel
dettaglio del pasto (due) e nel salvataggio di un pasto (due).

## La soluzione

Le funzioni di calendario diventano funzioni pure Rust:

```text
planner_parse_date          "YYYY-MM-DD" → NaiveDate, con validazione vera
planner_format_iso          NaiveDate → "YYYY-MM-DD", solo anni a 4 cifre
planner_shift_date          sposta di N giorni
planner_week_start_for_date lunedi' della settimana che contiene la data
planner_weekday             nome italiano del giorno
```

`chrono` viene dichiarato come dipendenza diretta con
`default-features = false`. **Non aggiunge nulla al binario**: era gia' nel
grafo, con le stesse feature, perche' lo usa `teloxide-core`. La scelta e' fra
riusare un'aritmetica gregoriana gia' collaudata e riscriverla a mano; la
prima costa zero byte in piu'.

Resta **una sola** query di calendario, ed e' necessaria:
`SELECT date('now','localtime')`. La data di oggi dipende dal fuso orario del
telefono, che solo SQLite conosce.

## Query risparmiate per schermata

```text
schermata settimana (dal menu')   19 → 2      -17
navigazione fra settimane         18 → 1      -17
schermata giorno                   3 → 1       -2
dettaglio pasto                    2 → 1       -1
salvataggio di un pasto            2 → 0       -2
```

## Effetto collaterale utile: la validazione diventa vera

`planner_valid_date` controllava solo la forma. `2026-02-30`, `2026-13-01` e
`2026-04-31` hanno la forma giusta e non esistono: passavano il controllo e
arrivavano fino a SQLite, che le trasformava in `NULL`. Ora la validazione
costruisce davvero la data e le rifiuta all'ingresso, dove arrivano — dai
callback Telegram.

Simmetricamente, `planner_format_iso` rifiuta gli anni fuori dalle quattro
cifre: una data che il nostro stesso parser non saprebbe rileggere non deve
uscire dal modulo, altrimenti finisce in un callback e torna indietro rotta.

## Comportamento di ripiego

Se la data di oggi non e' leggibile, `planner_today` restituisce la sentinella
`9999-12-31`, che tiene ogni pasto "non passato" e non fa partire segnalazioni
a vuoto. Quella sentinella pero' non deve diventare una settimana: la
schermata aprirebbe l'anno 9999. Il ripiego resta `1970-01-05`, che e' un
lunedi', com'era prima. Le due costanti ora hanno un nome e un test.

## Verifica

- 9 test nuovi: cambi di mese e di anno, anni bisestili comprese le eccezioni
  secolari (2000 bisestile, 1900 no), reversibilita' dello spostamento su 400
  settimane consecutive, date con forma giusta ma inesistenti, inizio
  settimana su tutti e sette i giorni, nomi italiani dei giorni, coerenza
  della settimana di ripiego;
- pipeline: `fmt`, `check`, `clippy --all-targets -- -D warnings`,
  `test` — **235 test**, da 226;
- nessuna migration: il database non viene toccato.

## Nota di processo: un ramo vuoto

Il primo tentativo di consegna e' finito su GitHub come **ramo vuoto**. I
comandi `git checkout -b`, `git add`, `git commit`, `git push` sono stati dati
sull'S9 invece che sul PC. Sull'S9 l'albero era pulito — le modifiche esistevano
solo sul PC — quindi `git add` non ha trovato nulla e `git commit` non ha creato
nulla. Nessuno dei due ha fallito in modo visibile: il push ha semplicemente
pubblicato un ramo che puntava al commit di partenza.

Il collaudo e' poi girato regolarmente, verde, **sul codice di prima**. L'unico
segnale era il conteggio: 226 test invece di 235.

Due controlli che lo intercettano, entrambi da un secondo:

- sul PC, prima del push: `git log --stat -1` deve elencare i file attesi;
- sull'S9, dopo il collaudo: il numero dei test deve essere quello nuovo.

La distinzione fra le due macchine e' ora scritta nella sezione 4 di
`docs/HANDOFF.md`, con percorsi e shell.

## Resta da fare, nella stessa direzione

`planner_load_meals` chiama `planner_find_for_date` una volta per giorno: sono
sette letture identiche per aprire una settimana. Cercare il planner una volta
sola e passarlo al ciclo toglie altre sei query, ed e' il passo successivo
naturale.

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

## La CI rossa: causa trovata

Attivare la CI sui branch `step-*` ha immediatamente prodotto una serie di run
rosse, con codice 101 dopo circa quattro minuti. Il codice era sano: la stessa
pipeline con `--locked` passava sia sul Galaxy S9 sia in un ambiente esterno.

**Causa reale:** il lint `clippy::drain_collect`, introdotto in una versione di
Clippy piu' recente di quelle usate in locale, su `take_transient_media` in
`src/context_bot.rs`. La funzione faceva `.drain(..).collect()` su un
`Vec<MessageId>` per ottenere un altro `Vec<MessageId>`, allocando un vettore
nuovo senza motivo. Sostituito con `std::mem::take`, che restituisce il
contenuto lasciando il vettore vuoto: stesso comportamento, un'allocazione in
meno. Nel codice resta un commento che spiega perche' e' scritto cosi', per non
farlo "semplificare" di nuovo in futuro.

**Perche' nessuno l'aveva visto:**

1. la CI girava solo su `main`, quindi i 22 commit dello Step 7 non erano mai
   passati da GitHub Actions;
2. il controllo Clippy sull'S9 usa una versione diversa da quella del runner e
   non emette quel lint. Un controllo locale che passa **non e' una prova** se la
   toolchain non e' la stessa: quando i due esiti divergono, ha ragione la CI.

Piste escluse durante la diagnosi, tutte verificate e nessuna colpevole:
`actions/checkout@v7` (esiste), memoria esaurita in fase di link (la riduzione
delle informazioni di debug non ha cambiato l'esito), parallelismo dei test (226
test passano anche a otto thread).

Le due variabili `CARGO_PROFILE_TEST_DEBUG` e `CARGO_PROFILE_DEV_DEBUG` sono
state mantenute: non erano la causa, ma fanno risparmiare tempo e memoria e non
tolgono nulla ai quattro controlli.

**Prima run verde dello Step 7:** Rust CI #42.

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

1. **Toolchain dell'S9 disallineata da quella della CI.** E' il motivo per cui
   il lint `drain_collect` non compariva in locale. Conviene aggiornarla, cosi'
   il controllo sul telefono torna equivalente a quello del runner.
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
