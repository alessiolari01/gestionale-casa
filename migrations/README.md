# Migrazioni database

Questa cartella contiene i file `.sql` di migrazione dello schema, uno per ogni
modifica, con nome `<timestamp>_<descrizione>.sql` compatibile con SQLx.

## Migrazioni presenti

- `20260812120000_schema_core.sql` — schema condiviso iniziale: `items`, `foto`,
  `tag`, `item_tag`, `promemoria`;
- `20260814121600_oggetti.sql` — tabella specifica `oggetti` dello Step 5A;
- `20260815183000_luoghi.sql` — Step 6A: `abitazioni`, `stanze`, `item_luogo`,
  indici e trigger di coerenza casa/stanza.

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
