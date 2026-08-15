# Migrazioni database

Questa cartella contiene i file `.sql` di migrazione dello schema, uno per
ogni modifica, con nome nel formato `<timestamp>_<descrizione>.sql`
(convenzione compatibile con SQLx).

## Migrazioni presenti

- `20260812120000_schema_core.sql`: schema dati condiviso da tutti i moduli
  (`items`, `foto`, `tag`, `item_tag`, `promemoria`). La descrizione e' in
  `docs/schema-core.md`.
- `20260814121600_oggetti.sql`: tabella specifica `oggetti` per lo Step 5A;
  dettagli in `docs/moduli/oggetti.md`.
- `20260814121600_oggetti.sql`: tabella specifica `oggetti` per lo Step 5A;
  dettagli in `docs/moduli/oggetti.md`.

## Esecuzione runtime

Dallo Step 4 le migration sono incorporate nel binario tramite
`sqlx::migrate!("./migrations")` e applicate automaticamente durante l'avvio
in `src/db.rs`.

Il file `build.rs` segnala a Cargo di ricompilare il progetto quando cambia la
cartella `migrations/`, cosi' una nuova migration viene inclusa anche usando
Rust stable.

Le foreign key vengono abilitate esplicitamente nelle opzioni di connessione
SQLx (`foreign_keys(true)`) per ogni connessione del pool.

## Regola per le modifiche future

Non modificare una migration gia' applicata su un database reale. Per cambiare
lo schema va creato un nuovo file di migration, mantenendo la cronologia delle
modifiche riproducibile.
