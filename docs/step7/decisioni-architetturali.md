# Decisioni architetturali Step 7

Questo documento raccoglie le decisioni approvate prima dell'implementazione.
Quando una decisione cambia, va aggiornato questo file e il documento del modulo
interessato.

## D01 — Database centrale, non SQLite condiviso

**Decisione:** più utenti possono usare lo stesso gestionale, ma non devono
sincronizzare o condividere fisicamente il file SQLite.

Il backend resta l'unico proprietario del DB. La condivisione avviene a livello
applicativo tramite utenti, spazi e permessi.

## D02 — Utente interno separato dagli account esterni

Telegram e Google sono identità esterne collegate a un utente interno.

Questo consente:

- più provider per la stessa persona;
- profili senza account Telegram;
- cambio di account esterno senza riscrivere la proprietà dei dati.

## D03 — Spazio come confine principale dei dati condivisi

Gli spazi possono essere personali, familiari o condivisi. Un utente può
appartenere a più spazi.

Case, oggetti e nuovi dati condivisibili dovranno essere associati allo spazio
quando la migration 7.1 verrà progettata.

## D04 — Condivisione e copia sono concetti differenti

**Condividere:** più persone accedono alla stessa entità.

**Copiare:** viene creata una nuova entità indipendente. Quando utile, la copia
può conservare una provenienza informativa dall'originale.

Il principio si applica soprattutto a:

- ricette;
- modelli turno/routine;
- modelli di pianificazione;
- checklist viaggio;
- template/export;
- altre entità riutilizzabili che verranno valutate caso per caso.

Non si applica automaticamente a credenziali, account Google/Telegram o altri
dati sensibili/personali.

## D05 — Autore obbligatorio nello storico condiviso

Ogni azione umana deve essere attribuibile all'utente che l'ha eseguita.
Gli effetti automatici devono risultare come automatici e conservare il
collegamento all'azione che li ha originati.

Dettagli in [storico-e-audit.md](storico-e-audit.md).

## D06 — Snapshot dell'autore nello storico

Oltre all'ID dell'utente va conservato un nome/autore snapshot adeguato alla
visualizzazione storica. Una futura rinomina dell'utente non deve rendere
ambiguo il passato.

## D07 — Profili alimentari separati dagli account

Un profilo alimentare rappresenta una persona che partecipa ai pasti e alle
quantità, non necessariamente un account del gestionale.

Può quindi rappresentare:

- un utente registrato;
- un partner;
- un bambino;
- un ospite.

## D08 — Alimento, prodotto acquistabile, scorta e oggetto posseduto sono concetti diversi

Esempio:

```text
ALIMENTO: pollo
PRODOTTO: confezione di pollo acquistabile in un negozio
SCORTA: 800 g di pollo disponibili
RICETTA: richiede 300 g di pollo
OGGETTO: bene fisico catalogato nel modulo Oggetti
```

Non va usata la tabella degli oggetti come catalogo alimentare.

## D09 — Catalogo alimentare globale + personalizzazioni dello spazio

Gli alimenti comuni possono essere globali all'installazione. Alimenti
personalizzati possono appartenere a uno spazio.

## D10 — Quantità e unità strutturate

Le ricette e le future funzioni di acquisto devono salvare quantità e unità in
modo strutturato. Conversioni automatiche solo quando semanticamente valide,
per esempio g↔kg o ml↔l.

Unità non direttamente convertibili, come pezzi, cucchiai o `q.b.`, restano
separate.

## D11 — Ricetta base + personalizzazioni per profilo

La ricetta mantiene quantità base. Un profilo può avere un fattore di porzione
o override specifici per ingrediente.

Esempio: stessa pasta, 120 g per una persona e 80 g per un'altra senza
duplicare la ricetta.

## D12 — Turno/routine come modello, assegnazione giornaliera come istanza

I turni hanno un nome personalizzato e regole abituali. L'assegnazione a una
data eredita i default ma può essere modificata senza alterare il modello.

La routine può descrivere lavoro, riposo, università, trasferta o altre giornate
tipo. La UI può presentare inizialmente il concetto come `Turni`.

## D13 — Il pasto conosce situazione e preparazione

Per ogni pasto possono essere definiti:

- orario;
- a casa / al lavoro / fuori / saltato / altra situazione;
- necessità di preparazione anticipata;
- data/ora o anticipo della preparazione;
- reminder opzionale.

Alla creazione/configurazione di un turno il flusso dovrebbe chiedere se
configurare il reminder, usando poi i default dell'utente quando disponibili.

## D14 — Reminder Telegram/email, niente SMS

I canali previsti nello Step 7 sono Telegram ed email. Gli SMS non fanno parte
della specifica corrente e non vanno predisposti senza una nuova decisione.

## D15 — Pianificazione per intervalli reali e slot non obbligatori

Il planner deve supportare singoli giorni, settimane, mesi e intervalli
arbitrari. Colazione/pranzo/cena/spuntino sono tipi utili ma non devono imporre
la presenza di tutti gli slot.

## D16 — Prezzi base persistenti, volantini solo per confronto

Il futuro modulo Acquisti conserva prezzi normali/base modificabili. Le offerte
di un volantino servono a confrontare il risparmio e non devono sostituire il
prezzo base.

Ogni confronto mostra, quando possibile:

- prezzo della confezione;
- prezzo normalizzato, per esempio €/kg, €/l o €/pezzo.

## D17 — Monitoraggio prezzi non obbligatorio per ogni oggetto

Per beni non alimentari la funzione prezzi va attivata solo dove ha senso, per
esempio prodotti acquistati frequentemente. Non ogni oggetto posseduto deve
avere un monitoraggio prezzi.

## D18 — Viaggi: checklist generica prima, oggetti reali come collegamento opzionale

Una voce di checklist esiste anche senza un oggetto registrato. Può però essere
coperta da uno o più oggetti reali.

Esempio: `Calzini × 5` può collegare cinque calzini registrati, oppure una parte
registrata e una parte generica.

Le quantità supportano una **quantità extra opzionale**, senza etichette
speciali nella UI.

## D19 — Stato temporaneo `in viaggio` senza perdere la posizione abituale

Un oggetto collegato a un viaggio conserva il suo luogo abituale. Durante il
viaggio può avere uno stato temporaneo e il riferimento al bagaglio/viaggio.
Al rientro viene verificato esplicitamente prima di chiudere lo stato.

## D20 — Spese come funzione generale

Le spese non appartengono solo ai viaggi. Devono poter essere personali o
condivise, collegate a contesti diversi e divise in quote uguali o
personalizzate.

Partecipanti senza account devono essere supportabili come ospiti.

## D21 — Nessun reset generale nel bot

Il DB attuale è ancora di sviluppo e potrà essere azzerato manualmente prima
del go-live. Non va però introdotto un comando/pulsante applicativo di reset
globale.

Dopo il go-live le migration dovranno preservare i dati reali.

## D22 — Il vecchio prototipo Step 7 è superato

`gestionale_step7_prototipo_bundle` non va applicato. È stato creato prima delle
ultime decisioni e non rappresenta la specifica corrente.

## D23 — Multi-spazio non esposto prima dello scoping completo

Lo schema utenti/spazi può essere introdotto prima della UI multi-spazio, ma il bot non deve permettere di creare o cambiare spazio finché tutte le query CRUD dei moduli Step 6 non filtrano esplicitamente lo spazio attivo.

Durante questa transizione lo spazio `#1` è il contesto di compatibilità e i default DB mantengono il comportamento precedente. È preferibile una fase single-space chiaramente documentata a una UI multiutente che possa mostrare o modificare dati dello spazio sbagliato.


## D24 — Lo spazio attivo è un confine di sicurezza applicativo

Quando la UI multi-spazio viene attivata, ogni lettura e scrittura dei moduli
già operativi deve essere filtrata usando lo `spazio_id` dell'attore corrente.
Un callback o comando contenente l'ID di un'entità di un altro spazio deve
comportarsi come se quell'entità non esistesse.

Il cambio spazio cancella le sessioni temporanee, così una bozza non può
attraversare accidentalmente il confine fra spazi.

## D25 — I nuovi utenti non entrano automaticamente nel bootstrap

Lo spazio `#1` rimane la casa dei dati legacy e viene assegnato al primo utente.
Un nuovo account Telegram che non possiede ancora membership riceve invece uno
spazio personale proprio. La condivisione con altri spazi dovrà avvenire tramite
invito esplicito, non per effetto collaterale del bootstrap.

## D26 — Membership e spazio attivo devono restare atomicamente coerenti

`preferenze_utente.spazio_attivo_id` non è sufficiente da sola per autorizzare
un contesto: lo spazio attivo deve essere anche una membership corrente
dell'utente. La rimozione della membership attiva produce un fallback
deterministico verso un altro spazio disponibile; se non restano membership,
la preferenza viene eliminata e il bootstrap identità la ricrea al successivo
accesso.

La risoluzione Telegram verifica nuovamente la membership prima di costruire
l'`AuditActor`, così eventuali database legacy o stati incoerenti non possono
produrre accessi in lettura a uno spazio dal quale l'utente è già stato rimosso.
In produzione le operazioni space-aware richiedono inoltre un contesto
`AuditActor` installato: non esiste fallback silenzioso verso lo spazio `#1`.


## ADR — spazio predefinito distinto dalla vista

**Decisione:** `spazio_attivo_id` viene mantenuto per compatibilità ma rappresenta lo **spazio predefinito**. La preferenza `vista_spazi` determina se la consultazione è limitata a tale spazio oppure comprende tutte le membership dell'utente.

**Motivazione:** una persona può appartenere contemporaneamente a un contesto personale e a uno condiviso; obbligarla a cambiare spazio per ogni ricerca o confronto rende il modello inutilmente rigido.

**Sicurezza:** la vista globale non autorizza nuovi spazi. Le query usano l'insieme delle membership dell'utente; le mutazioni ricontrollano il ruolo sullo spazio effettivamente coinvolto.

## ADR — proprietà dell'item separata dalla posizione

**Decisione:** `items.spazio_id` non cambia quando un oggetto viene portato in una casa di un altro spazio. `item_luogo` può quindi riferirsi a una casa di un altro spazio, purché l'operazione sia autorizzata su entrambi i contesti.

**Conseguenza:** un portatile personale può trovarsi in una casa condivisa senza diventare proprietà dello spazio condiviso. Lo storico dell'item resta attribuito allo spazio proprietario, mentre conserva il luogo reale dell'evento.
