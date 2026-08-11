# Migrazioni database

Qui andranno i file `.sql` di migrazione dello schema, uno per ogni
modifica, con nome nel formato `<timestamp>_<descrizione>.sql`
(convenzione di `sqlx migrate add`).

La prima migrazione da creare sarà lo schema dati "core" condiviso da tutti
i moduli (foto, categorie/tag, promemoria) — vedi `ARCHITETTURA.md`,
sezione 6.
