# Migrazioni database

Questa cartella contiene i file `.sql` di migrazione dello schema, uno per ogni
modifica, con nome `<timestamp>_<descrizione>.sql` compatibile con SQLx.

## Migrazioni presenti

- `20260812120000_schema_core.sql` — schema condiviso iniziale: `items`, `foto`,
  `tag`, `item_tag`, `promemoria`;
- `20260814121600_oggetti.sql` — tabella specifica `oggetti` dello Step 5A;
- `20260815183000_luoghi.sql` — Step 6A: `abitazioni`, `stanze`, `item_luogo`,
  indici e trigger di coerenza casa/stanza;
- `20260815215400_storico.sql` — Step 6B: infrastruttura dello storico trasversale;
- `20260817171600_contenitori.sql` — Step 6C.1: contenitori gerarchici e `item_luogo.contenitore_id`;
- `20260820230000_storico_contenitori.sql` — Step 6C.4: snapshot storico del contenitore/percorso e backfill delle sole identità;
- `20260823153000_fondazioni_condivise.sql` — Step 7.1: utenti, spazi, membership, account Telegram, inviti, confine di spazio e audit autore/origine.
- `20260823174500_spazi_operativi.sql` — Step 7.1: unicità case/tag per spazio, rebuild SQLite e coerenza membership ↔ spazio attivo con fallback sicuro.
  Il rebuild è compatibile con SQLx 0.8.6: resta nella transazione del driver, mantiene le foreign key attive e ricostruisce in sicurezza anche le tabelle figlie necessarie.

Documentazione:

- `docs/schema-core.md`;
- `docs/moduli/oggetti.md`;
- `docs/moduli/luoghi.md`.

## Esecuzione runtime

Dallo Step 4 le migration sono incorporate nel binario tramite
`sqlx::migrate!("./migrations")` e applicate automaticamente durante l'avvio in
`src/db.rs`.

Il file `build.rs` segnala a Cargo di ricompilare il progetto quando cambia la
cartella `migrations/`, così una nuova migration viene inclusa anche usando Rust
stable.

Le foreign key vengono abilitate esplicitamente nelle opzioni di connessione
SQLx (`foreign_keys(true)`) per ogni connessione del pool.

## Regola fondamentale

**Non modificare una migration già applicata su un database reale.** Per
cambiare lo schema va creato un nuovo file di migration, mantenendo la
cronologia riproducibile.

Per questo lo Step 6A non modifica né la migration core né quella Oggetti:
aggiunge una terza migration separata.

## Step 6B — storico trasversale

Migration: `20260815215400_storico.sql`. Introduce `storico_entita`, `storico_eventi`, `storico_cambiamenti` e `storico_cambi_luogo`, oltre agli indici necessari.

La migration esegue il backfill delle identità per elementi, abitazioni e stanze già esistenti ma **non crea eventi retroattivi**. Le migration precedenti restano immutabili.


## Step 6C.4 — storico contenitori

`20260820230000_storico_contenitori.sql` aggiunge campi nullable alle tabelle dello storico esistenti e registra in `storico_entita` i contenitori già presenti. Non modifica le migration precedenti e **non crea eventi retroattivi**.

## Step 7.1 — fondazioni condivise

`20260823153000_fondazioni_condivise.sql` assegna i dati preesistenti allo spazio bootstrap `#1` senza creare utenti fittizi e senza inventare autori per gli eventi storici. I nuovi account interni vengono creati dal runtime alla prima interazione Telegram autorizzata.

Nel checkpoint iniziale 7.1 i default `spazio_id = 1` mantengono compatibilità.
Con `20260823174500_spazi_operativi.sql` la UI multi-spazio può essere attivata
insieme allo scoping delle query: `abitazioni` e `tag` vengono ricostruite con
unicità `(spazio_id, nome)` senza spostare i dati legacy.

- `20260823200000_vista_multispazio_condivisione.sql`: aggiunge la preferenza di vista multi-spazio, la fondazione `item_condivisioni` e separa la proprietà dell'item dalla sua posizione fisica.
- `20260823232000_storico_spazi_luogo.sql`: conserva nello storico lo spazio della posizione e lo spazio prima/dopo dei cambi luogo, con backfill degli eventi esistenti.

## Step 7.2A — alimenti e unità

`20260824143000_alimenti_unita.sql` introduce `unita_misura`, `alimenti` e
`alimento_alias`. Le conversioni automatiche sono previste soltanto nelle
famiglie massa (`g`/`kg`) e volume (`ml`/`l`). Il dettaglio architetturale è in
`docs/step7/step-7.2a-alimenti-unita.md`.

## Step 7.2B — proprietà e condivisione alimenti

`20260824160500_alimenti_proprieta_condivisione.sql` separa la proprietà
personale dell'alimento dalla sua visibilità negli spazi e introduce
`alimento_spazi`. La perdita di una membership non cancella gli alimenti
posseduti dall'utente.

## Step 7.2B — categorie alimenti

`20260824173500_categorie_alimenti.sql` introduce `categorie_alimento` e la
relazione molti-a-molti `alimento_categorie`; gli alimenti esistenti e nuovi
partono dalla categoria `Altro`.

## Step 7.2B — fondazione permessi risorse

`20260824201500_permessi_risorse_condivise.sql` introduce `inviti_risorsa` e
`permessi_risorsa`. La struttura e trasversale e verra riusata da Ricette e
dalle future entita condivisibili.

## Step 7.2C — fondazioni Ricette

`20260824222000_ricette_fondazioni.sql` introduce `ricette`, `ricetta_spazi`
e `ricetta_ingredienti`. Gli ingredienti referenziano gli alimenti esistenti.
La struttura è indicizzata per la ricerca OR su più ingredienti e per il
ranking in base al numero di ingredienti richiesti presenti nella ricetta.

## Step 7.2D.0 — catalogo alimenti base

`20260825014500_catalogo_alimenti_base.sql` sostituisce gli alimenti presenti
al momento del passaggio con il catalogo globale di base usato come vocabolario
condiviso per le Ricette. La migration è già applicata sul database reale e
non deve più essere modificata.

## Step 7.2D.0.1 — compatibilità alimentare

`20260825023000_compatibilita_alimentare.sql` aggiunge le etichette alimentari
trasversali (diete ed esclusioni), la matrice di compatibilità degli alimenti
del catalogo base e la vista `v_ricetta_compatibilita_alimentare`.

Ogni associazione usa uno stato tra `si`, `no` e `verificare`. Lo stato
`verificare` è usato quando formulazione, marca o processo produttivo possono
cambiare la compatibilità. La vista Ricette applica una regola fail-closed:
una compatibilità mancante viene trattata come `verificare`, non come
compatibile.

Le etichette sono un supporto gestionale e non sostituiscono la verifica
dell'etichetta reale per allergie o intolleranze.

## Step 7.2D.0.2 — prodotti commerciali e catalogo paginato

`20260825101500_prodotti_alimentari.sql` separa l'alimento generico usato
nelle Ricette dal prodotto commerciale acquistabile. Nello schema storico di
questa migration marca, nome commerciale e prima confezione erano ancora sulla
stessa riga. Lo Step 7.2F.0 separa successivamente i formati senza riscrivere
questa migration già applicata.

## Step 7.2D.0.3 — nutrizione prodotti e prodotto specifico nelle Ricette

`20260825113000_prodotti_nutrizione_ricette.sql` aggiunge i valori nutrizionali
facoltativi dei prodotti commerciali, normalizzati per 100 g oppure 100 ml.
I dati restano separati dall'alimento generico e possono essere modificati o
rimossi senza alterare la ricetta o il catalogo base.

La stessa migration aggiunge a `ricetta_ingredienti` il riferimento opzionale
`prodotto_alimentare_id`. L'ingrediente mantiene sempre `alimento_id`: se viene
scelto un prodotto reale, un trigger verifica che appartenga allo stesso
alimento. Questo permette alle future Ricette di scegliere tra ingrediente
generico e prodotto specifico senza legare il modello ricetta a una marca.

## Step 7.2E — accesso controllato e Miglioramenti

`20260825153000_accesso_miglioramenti.sql` introduce il modello applicativo di
accesso Telegram approvato e il backlog interno `💡 Miglioramenti`.

La migration:

- aggiunge `utenti.amministratore_principale` con unicità sul solo valore attivo;
- inizializza come amministratore principale il proprietario/bootstrap già admin;
- crea `richieste_accesso` con stati `pendente`, `approvata`, `rifiutata`;
- crea `miglioramenti` con autore e stato;
- crea `miglioramento_allegati` per screenshot/foto multiple.

`ALLOWED_CHAT_IDS` resta bootstrap/emergenza. L'approvazione di una richiesta
crea un utente normale e il suo spazio personale ma non concede membership ad
altri spazi né permessi su risorse esistenti.

## Step 7.2F.0 — formati dei prodotti commerciali

`20260825220000_formati_prodotti_alimentari.sql` separa l'identità stabile del
prodotto commerciale dai suoi formati acquistabili. Esempio: `Philadelphia ·
Original` resta un solo prodotto, mentre `175 g`, `200 g` e `350 g` diventano
righe distinte in `formati_prodotto_alimentare`.

La migration migra automaticamente ogni confezione già presente in
`prodotti_alimentari` come primo formato, sposta il significato autorevole del
barcode/EAN sul formato e aggiunge la vista `v_prodotti_formati_attivi`, pensata
come base per Lista spesa, prezzi e disponibilità per punto vendita. Le vecchie
colonne confezione presenti in `prodotti_alimentari` restano temporaneamente
solo per compatibilità con le migration già applicate e non vanno più usate
come fonte autorevole dal codice nuovo.
