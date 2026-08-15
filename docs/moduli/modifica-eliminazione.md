# Modifica ed eliminazione oggetti — Step 5C

## Scopo

Lo Step 5C completa il ciclo CRUD di base degli oggetti generici aggiungendo la
modifica di un oggetto già persistito e la cancellazione definitiva con conferma.
Non introduce nuove tabelle o migration.

## Modifica

Dalla scheda oggetto sono disponibili:

```text
[ ✏️ Modifica ]   [ 🗑 Elimina ]
```

Comando equivalente:

```text
/oggetto_modifica <id>
```

La modifica legge `items` + `oggetti` e costruisce una bozza che conserva
l'`item_id` originale. Il pannello è lo stesso usato durante la creazione, con
alcune differenze:

- compare `✏️ Nome`;
- il pulsante finale è `💾 Salva modifiche`;
- i valori correnti sono mostrati prima della sostituzione;
- `/salta` mantiene il valore corrente;
- `/rimuovi` cancella il campo opzionale attualmente aperto;
- `🗑 Rimuovi condizione` azzera la condizione;
- il nome non può essere rimosso.

`❌ Annulla` e `/annulla` eliminano soltanto la bozza in memoria. Nessuna
modifica arriva a SQLite fino al salvataggio finale. Se l'operazione annullata
era la modifica di un oggetto già salvato, il bot torna direttamente alla
scheda dello stesso oggetto; durante la creazione di un nuovo oggetto torna
invece al menu Oggetti.

## Persistenza dell'update

`💾 Salva modifiche` apre una transazione e aggiorna:

1. `items.nome` per l'ID esistente;
2. tutti i campi della riga `oggetti` collegata.

Se uno dei due record attesi non esiste, la transazione fallisce. Non viene mai
creato un nuovo oggetto come effetto della modifica.

## Eliminazione

Dalla scheda oppure con:

```text
/oggetto_elimina <id>
```

il bot mostra prima una conferma:

```text
⚠️ Eliminare definitivamente?

📦 Nome oggetto
#ID

[ 🗑 Sì, elimina definitivamente ]
[ ↩️ Annulla ]
```

Solo la conferma positiva esegue il `DELETE`.

La riga cancellata è quella di `items`. Le foreign key già previste nello schema
core rimuovono tramite `ON DELETE CASCADE` i record collegati, comprese la riga
`oggetti` e le righe `foto`.

Dopo il commit SQLite il backend elimina anche:

```text
data/media/oggetti/<id>/
```

Se la directory non esiste, l'operazione è comunque considerata riuscita. Se il
database è stato eliminato ma il filesystem restituisce un altro errore, il bot
segnala la directory residua invece di nascondere il problema.

## Comandi

| Azione | Pulsante | Comando |
|---|---|---|
| modifica | `✏️ Modifica` | `/oggetto_modifica <id>` |
| elimina | `🗑 Elimina` | `/oggetto_elimina <id>` |
| mantieni campo | — | `/salta` |
| rimuovi campo opzionale | — | `/rimuovi` |
| annulla bozza | `❌ Annulla` | `/annulla` |

## Test richiesti prima della chiusura

- `cargo fmt --all -- --check`;
- `cargo check --locked`;
- `cargo test --locked`;
- `cargo clippy --all-targets --locked -- -D warnings`;
- modifica di nome e dettaglio su un oggetto di test;
- rimozione di un valore tramite `/rimuovi`;
- `/salta` su un valore esistente;
- annullamento della bozza senza modifiche persistite;
- annullamento della schermata di eliminazione;
- delete confermato su un oggetto di test con foto;
- verifica cascade SQLite e rimozione della directory media;
- verifica dopo riavvio;
- CI della Pull Request verde prima del merge.

## Fuori perimetro

- cestino o recupero di un oggetto eliminato;
- storico delle modifiche;
- versionamento delle foto;
- luoghi strutturati multi-casa/stanze: implementati successivamente nello Step
  6A e documentati in `docs/moduli/luoghi.md`.
