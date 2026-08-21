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
- `20260820230000_storico_contenitori.sql` — Step 6C.4: snapshot storico del contenitore/percorso e backfill delle sole identità.

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
