# Come ci siamo arrivati

Questo documento si legge **una volta**, per capire perché il progetto è fatto
così e non in un altro modo. Non descrive lo stato attuale: quello sta in
`STATO.md`. Non elenca le modifiche: quelle stanno in `CHANGELOG.md`.

Qui ci sono tre cose che negli altri documenti non trovano posto: la sequenza
dei blocchi, le cose provate e abbandonate, e gli errori di processo con quello
che ne è stato imparato.

Sostituisce dodici file di appunti per singolo step e un handoff di 56 KB che
raccontavano gli stessi fatti in ordini diversi. Restano tutti recuperabili con
`git log --follow`.

---

## 1. La sequenza dei blocchi

| Blocco | Cosa ha introdotto |
|---|---|
| Step 1-2 | Scheletro Rust e schema dati core. |
| Step 3 | Bot Telegram, bootstrap dell'accesso, Git e CI. |
| Step 4 | Runtime SQLite con migration applicate all'avvio. |
| Step 5 | Oggetti generici, foto, modifica ed eliminazione sicura. |
| Step 6A | Case e stanze: posizione strutturata, multi-abitazione. |
| Step 6B | Storico trasversale: eventi, cambiamenti, snapshot. |
| Step 6C | Contenitori gerarchici e navigazione contestuale dei luoghi. |
| 7.0 | Specifica, confini di dominio, politica migration. Nessun codice. |
| 7.1 | Utenti interni, account Telegram, spazi, membership, ruoli, audit. |
| 7.2A-B | Alimenti, unità, alias, categorie, proprietà personale e condivisione. |
| 7.2C | Fondazioni Ricette e ricerca per ingredienti. |
| 7.2D | Ruoli di sistema, catalogo base di alimenti, compatibilità alimentare. |
| 7.2E | Accesso su richiesta approvata, e il backlog `💡 Miglioramenti`. |
| 7.2F | Prodotti commerciali e formati acquistabili; ricette operative su Telegram. |
| 7.2G | Workflow Miglioramenti semplificato e archivio. |
| 7.2H | Profili alimentari, membri e inviti degli Spazi, rifiniture UX. |
| 7.2I | Porzioni per profilo, override e esclusione del singolo ingrediente. |
| 7.3A-B | Planner alimentare: fondazioni con snapshot, poi UI Telegram completa. |

---

## 2. Cose provate e abbandonate

È la parte che spiega perché il codice non è fatto in un altro modo.

### Due implementazioni parallele dello stesso blocco

Il 31 agosto il blocco 7.3B è stato sviluppato **due volte in parallelo**. Il
ramo `step-7-alimentazione` contiene la versione scartata; quella buona è
`step-7-alimentazione-s9`, poi mergiata in `main` con la PR #9.

La causa non è tecnica: il lavoro esisteva solo come modifiche non committate
su un dispositivo, e una seconda sessione è ripartita da uno stato più vecchio
credendolo attuale. Da qui la regola che ogni lavoro passa da un branch pushato
prima che qualcun altro ci metta mano.

### Due calendari gregoriani

Sono esistite due implementazioni delle stesse regole sulle date: la congruenza
di Zeller scritta a mano per il calendario degli inviti, e una basata su
`chrono` nel planner. Unificate in `src/modules/calendario.rs`.

### L'aritmetica delle date delegata a SQLite

Aprire la settimana del planner costava diciannove query, di cui diciassette
calcolavano soltanto date senza leggere alcun dato. Sostituite da funzioni pure
in Rust. Resta una sola query, `SELECT date('now','localtime')`, perché il fuso
orario del telefono lo conosce solo SQLite.

### Il workflow Miglioramenti a quattro stati

Gli stati `aperto / pianificato / fatto / scartato` sono stati abbandonati:
`pianificato` e `verificato` non aggiungevano informazione — un elemento
approvato è semplicemente da fare — e `fatto` è diventato archiviazione invece
che stato. In 7.2I.3 è sparita anche la categoria `Verificati`: un collaudo
positivo porta direttamente in archivio.

### La whitelist statica come modello di autorizzazione

`ALLOWED_CHAT_IDS` era il modo di aggiungere utenti. Dallo 7.2E è declassata a
**bootstrap di emergenza** per il primo amministratore su un database nuovo: il
backend prova prima a risolvere un account approvato nel database.

### Colonne rese legacy invece che rimosse

Poiché una migration applicata è immutabile, alcune colonne restano nello
schema senza essere più la fonte della verità:

- `alimenti.spazio_id`, non più fonte di proprietà o visibilità;
- `prodotti_alimentari.quantita_confezione`, `unita_confezione_id`,
  `codice_ean`, superate da `formati_prodotto_alimentare`;
- `ricette.procedimento`, campo testuale unico sostituito da `ricetta_step`.

Esistono anche tabelle create e **mai usate da nessuna riga di codice**:
`item_condivisioni`, `tag`, `item_tag`, `promemoria`. Sono schema morto:
prima di riusarle va verificato che il modello sia ancora quello giusto.

### Interfaccia tolta

- la riga "Comandi rapidi" del menù principale;
- i pulsanti `👕 Vestiti · prossimamente` e `🚗 Veicoli · prossimamente`, tolti
  il 1 settembre perché un pulsante che non porta da nessuna parte è un invito
  a premerlo per niente. I moduli restano previsti;
- l'emoji carota davanti a ogni ingrediente: gli indicatori sono rimasti solo
  per le eccezioni (`⚙️` personalizzato, `🚫` escluso).

### La consegna via ZIP

Il giro `zip → scp → unzip → installer Python` è stato sostituito da
`scripts/aggiorna-s9.sh` su Git.

### Il prototipo dello Step 7

Esiste un bundle `gestionale_step7_prototipo_bundle` precedente alle decisioni
finali: **non va applicato**, non rappresenta la specifica.

### Rimandati per un vincolo di schema

I "pasti liberi" senza ricetta non sono rappresentabili perché
`ricetta_nome_snapshot` è `NOT NULL`. La decisione è stata rinviata quando è
arrivato l'esito "saltato", che copre il caso più frequente.

---

## 3. Errori di processo, e cosa ne è stato imparato

Sono gli episodi che hanno prodotto le regole operative di `STATO.md`.

### I comandi Git dati sulla macchina sbagliata

Il 1 settembre i comandi di commit sono stati eseguiti sull'S9 invece che sul
PC. Sull'S9 l'albero era pulito — le modifiche esistevano solo sul PC — quindi
`git add` non ha trovato nulla e `git commit` non ha creato nulla. Nessuno dei
due ha fallito in modo visibile: il push ha pubblicato **un ramo vuoto**, e il
collaudo è girato verde sul codice precedente.

L'unico segnale era il conteggio dei test: 226 invece di 235.

### Lo script lanciato senza `--ramo`

Poche ore dopo, la stessa forma di errore: lo script aggiornava soltanto il ramo
su cui l'S9 si trovava già, quindi il lavoro consegnato su un ramo nuovo non
arrivava e il collaudo girava, di nuovo verde, sul codice di prima.

Da qui i due controlli scritti in `STATO.md`: `git log --stat -1` prima del
push, e il numero dei test dopo il collaudo.

### La toolchain locale più vecchia della CI

Il lint `clippy::drain_collect` era invisibile sull'S9 ed è emerso solo quando
la CI ha iniziato a girare sui branch. Un controllo locale che passa non è una
prova se la toolchain non è la stessa: quando gli esiti divergono ha ragione la
CI.

### I limiti dell'hardware presi per errori di codice

Sull'S9 il linker segfaultava in modo apparentemente casuale. La causa era che
le impostazioni anti-memoria vivevano solo come variabili d'ambiente dello
script: chi lanciava `cargo run` a mano otteneva i default di cargo, 257 file
oggetto pieni di informazioni di debug da collegare su un telefono. Il
"riprovando passa" era la memoria libera che oscillava.

Lezione: se un'impostazione serve a far compilare il progetto, il posto giusto
è il progetto — `Cargo.toml` e `build.rs` — non chi lo lancia.

### Documentazione che afferma senza verificare

I documenti hanno dichiarato più volte cose non vere: che una migration fosse
da applicare quando era già applicata, che il branch di lavoro fosse uno
scartato, che i moduli Porzioni e Planner fossero da costruire quando erano già
in produzione. Bastava leggere `_sqlx_migrations` o il menù del bot.

È la ragione della regola in testa a `STATO.md`.

### Le verifiche che richiedono un secondo account Telegram

Ricorrono da 7.2G e sono **ancora aperte**: invito accettato con apertura dello
spazio, notifica al creatore, notifica di cambio ruolo, notifica di rimozione.
La regola adottata è registrarle come verifiche differite esplicite, senza
dichiararle eseguite e senza bloccare lo step successivo.

### Il rituale delle migration

Prima di applicare una migration al database reale: backup, controllo che non
sia già registrata, prova su una copia, `PRAGMA integrity_check` e
`PRAGMA foreign_key_check` sulla copia, e solo dopo l'avvio. Oggi lo fa
`scripts/aggiorna-s9.sh`.
