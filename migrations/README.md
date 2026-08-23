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
