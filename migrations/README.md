# Migrazioni database

Questa cartella contiene i file `.sql` di migrazione dello schema, uno per
ogni modifica, con nome nel formato `<timestamp>_<descrizione>.sql`
(convenzione compatibile con `sqlx migrate`).

## Migrazioni presenti

- `20260812120000_schema_core.sql`: schema dati condiviso da tutti i moduli
  (`items`, `foto`, `tag`, `item_tag`, `promemoria`). La descrizione è in
  `docs/schema-core.md`.

## Prossimo passo

Nel prossimo step del backend verrà implementato `src/db.rs` con:

1. apertura del database SQLite;
2. creazione delle cartelle necessarie se assenti;
3. foreign key abilitate esplicitamente;
4. esecuzione automatica delle migrazioni all'avvio.
