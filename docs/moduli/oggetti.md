# Modulo Oggetti generici — Step 5A → Step 6A

## Scopo

Il modulo cataloga tutto cio' che non appartiene a un modulo dedicato: attrezzi,
elettronica, elettrodomestici, valigie, mobili, accessori e altri beni di casa.

Lo Step 5A copre il ciclo minimo completo:

`Telegram -> Rust -> SQLx -> items + oggetti -> Telegram`

La revisione dei **campi della bozza** prima di `✅ Salva` fa parte dello Step
5A. Lo **Step 5C** estende lo stesso pannello agli oggetti già persistiti e
aggiunge l'eliminazione con conferma. Le foto sono state aggiunte nello Step 5B.
Lo **Step 6A** aggiunge casa/stanza strutturate senza perdere il vecchio campo
libero di posizione. Documenti, garanzie, promemoria, tag, storico e prestiti
restano sviluppi successivi.

## Modello dati

Ogni oggetto ha sempre una riga in `items`:

- `items.tipo = 'oggetto'`;
- `items.nome` e' l'unico dato obbligatorio per l'utente;
- `items.id` e' anche la chiave primaria di `oggetti.item_id`.

La migration `20260814121600_oggetti.sql` aggiunge:

| Campo | Tipo logico | Obbligatorio | Note |
|---|---|---:|---|
| `item_id` | ID | si | PK + FK verso `items(id)`, cascade |
| `descrizione` | testo | no | dettaglio libero |
| `marca` | testo | no | es. Bosch |
| `modello` | testo | no | es. UniversalImpact 800 |
| `numero_serie` | testo | no | tenuto per gli oggetti dove ha senso |
| `posizione` | testo | no | dallo Step 6A: dettaglio libero, es. scaffale 2 |
| `data_acquisto` | data ISO | no | salvata come `AAAA-MM-GG` |
| `prezzo_acquisto_centesimi` | intero | no | denaro in centesimi, mai `REAL` |
| `venditore` | testo | no | es. Amazon, MediaWorld, privato |
| `valore_stimato_centesimi` | intero | no | valore attuale stimato |
| `condizione` | enum testo | no | ottimo/buono/usurato/da_riparare |
| `note` | testo | no | note libere |

> Dallo Step 6A `posizione` **non rappresenta più da sola il luogo completo**.
> Casa e stanza sono salvate tramite `item_luogo`; `oggetti.posizione` resta un
> dettaglio libero (es. `scaffale 2`). I valori storici non vengono convertiti
> automaticamente.

Foto, luoghi, futuri documenti/garanzie, tag e promemoria non vengono duplicati in questa
tabella: usano le tabelle core predisposte. Dallo Step 5B la voce `📷 Foto` della
scheda oggetto usa concretamente la tabella core `foto`.

## Interfaccia Telegram

L'interfaccia principale usa **inline keyboard**. I comandi testuali restano in
parallelo come scorciatoie e per debug; entrambe le strade richiamano la stessa
logica applicativa.

| Azione | Pulsante | Comando equivalente |
|---|---|---|
| menu oggetti | `📦 Oggetti` | `/oggetti` |
| nuova bozza | `➕ Nuovo oggetto` | `/oggetto_nuovo [nome]` |
| elenco | `📋 Elenco oggetti` | `/oggetti_lista` |
| ricerca | `🔎 Cerca` | `/oggetto_cerca [testo]` |
| scheda per ID | pulsante risultato | `/oggetto <id>` |
| modifica oggetto | `✏️ Modifica` | `/oggetto_modifica <id>` |
| elimina oggetto | `🗑 Elimina` | `/oggetto_elimina <id>` |
| casa/stanza | `🏠 Casa / stanza` | `/oggetto_luogo <id>` o `/oggetto_sposta <id>` |
| foto oggetto | `📷 Foto` | `/foto <id>` |
| annulla operazione | `❌ Annulla` | `/annulla` |
| mantieni valore corrente | — | `/salta` |
| rimuovi campo opzionale aperto | — | `/rimuovi` |

`/start` apre il menu principale con Oggetti, Case e stanze e Stato sistema; Vestiti, Veicoli
e Ricette sono gia' rappresentati esteticamente ma marcati come prossimamente.

## Creazione con pannello dettagli

1. il bot chiede solo il nome;
2. dopo il nome mostra il pannello dettagli;
3. l'utente puo' salvare subito oppure aggiungere dati opzionali;
4. durante una **nuova creazione**, `🏠 Posizione` apre un unico flusso guidato `Casa -> Stanza -> Dettaglio posizione`;
5. se l'utente salta la casa, il passaggio stanza viene saltato automaticamente e il bot chiede direttamente il dettaglio libero;
6. dopo aver scelto una casa si puo' scegliere una sua stanza oppure lasciare l'oggetto associato alla sola casa; non viene mai proposta una stanza senza aver prima scelto la casa;
7. il dettaglio posizione resta opzionale e puo' essere saltato con `/salta`;
8. `Marca e modello` e `Acquisto` sono piccoli flussi guidati;
9. `Condizione` usa quattro pulsanti;
10. `Altri dettagli` contiene descrizione, valore stimato e numero seriale;
11. le sezioni gia' compilate vengono marcate con `✅` nel pannello;
12. riaprendo un campo gia' valorizzato, il bot mostra il valore attuale prima di chiedere quello nuovo; `/salta` mantiene il valore esistente invece di sovrascriverlo;
13. `✅ Salva` inserisce `items`, `oggetti` e, quando scelto, `item_luogo` nella **stessa transazione SQL**.

Esempio di creazione completa:

```text
Nome: Trapano Bosch
→ 🏠 Posizione
→ Casa principale
→ Garage
→ Scaffale 2
→ ✅ Salva

Risultato:
🏠 Casa principale / 🚪 Garage
📌 Scaffale 2
```

Se invece viene premuto `⏭ Salta casa -> dettaglio`, non viene chiesta alcuna
stanza e si passa direttamente a `📌 Dettaglio posizione`.

Una bozza incompleta vive solo in memoria: se il backend viene riavviato prima
del salvataggio, la bozza viene persa ma il database resta invariato.

## Modifica di un oggetto salvato — Step 5C

Dalla scheda di un oggetto, `✏️ Modifica` oppure `/oggetto_modifica <id>` carica
dal database tutti i valori correnti in una bozza di modifica. La bozza conserva
l'ID originale: `💾 Salva modifiche` esegue `UPDATE` su `items` e `oggetti` nella
stessa transazione, senza creare un nuovo item.

Regole UX:

- il nome è modificabile ma non può essere rimosso;
- nella modifica il campo `📌 Dettaglio posizione` resta separato dal luogo strutturato: casa/stanza si cambiano dalla scheda con `🚚 Sposta oggetto`, così uno spostamento resta un'azione esplicita e potrà essere registrato correttamente nello storico;
- riaprendo un campo viene mostrato il valore corrente;
- `/salta` mantiene il valore;
- `/rimuovi` imposta a `NULL` il campo opzionale attualmente aperto;
- la condizione ha un pulsante dedicato `🗑 Rimuovi condizione`;
- `❌ Annulla` e `/annulla` scartano l'intera bozza: il database non viene
  toccato fino al salvataggio finale; se si stava modificando un oggetto già
  salvato, il bot torna alla scheda di quell'oggetto, mentre durante una nuova
  creazione torna al menu Oggetti.

## Eliminazione sicura — Step 5C

`🗑 Elimina` oppure `/oggetto_elimina <id>` non cancella immediatamente. Il bot
mostra nome e ID e richiede una seconda conferma esplicita. Solo il pulsante
`🗑 Sì, elimina definitivamente` esegue la cancellazione.

La cancellazione parte dalla riga `items`. Le foreign key con
`ON DELETE CASCADE` rimuovono i dati collegati, comprese le righe `oggetti` e
`foto`. Dopo il commit SQLite il backend prova anche a eliminare
`data/media/oggetti/<id>/`. Se la pulizia del filesystem fallisce, l'oggetto
resta eliminato dal database ma il bot segnala chiaramente la directory da
controllare manualmente.

## Luogo strutturato — Step 6A

La scheda dell'oggetto espone `🏠 Casa / stanza`. L'utente può:

- scegliere una casa;
- scegliere una stanza di quella casa;
- lasciare l'oggetto associato alla sola casa;
- spostare l'oggetto in un'altra casa/stanza;
- rimuovere del tutto il luogo strutturato.

La relazione è salvata in `item_luogo`, quindi non è specifica del modulo
Oggetti. La scheda mostra, per esempio:

```text
🏠 Casa principale / 🚪 Garage
📌 Scaffale 2
```

Eliminare una stanza non elimina l'oggetto: rimane nella casa senza stanza.
Eliminare una casa non elimina l'oggetto: perde solo la relazione di luogo.
Dettagli completi in `docs/moduli/luoghi.md`.

## Elenco e ricerca

- elenco alfabetico, 8 oggetti per pagina;
- ogni riga ha un pulsante che apre la scheda;
- la ricerca controlla nome, marca, modello, numero seriale, dettaglio posizione,
  venditore, descrizione, note, nome casa e nome stanza;
- massimo 12 risultati per ricerca nello Step 5A.

## Validazione

- nome: 1-120 caratteri;
- date accettate: `GG/MM/AAAA` e `AAAA-MM-GG`, salvate in ISO;
- prezzi/valori: accettano `89`, `89,90`, `89.90` e formati italiani come
  `1.234,56`; vengono salvati come centesimi interi;
- importi negativi sono rifiutati sia dal parser sia dai `CHECK` SQLite;
- `condizione` e' vincolata a quattro valori ammessi.

## Test automatici Step 5A

Sono inclusi test per:

- parsing prezzi;
- normalizzazione/validazione date;
- parsing dei comandi con suffisso `@nome_bot`;
- salvataggio + lettura + elenco + ricerca;
- `ON DELETE CASCADE` tra `items` e `oggetti`;
- rifiuto di valori monetari negativi a livello SQLite.

Lo Step 5A è stato verificato sul Galaxy S9, integrato in `main` con CI verde e
applicato anche al database reale. La gestione delle foto è sviluppata
separatamente nello Step 5B.

## Fuori perimetro

- Step 6B: storico globale + individuale;
- Step 6C: contenitori e sotto-posizioni;
- Step 7A: documenti e garanzie;
- Step 7B: promemoria e scadenze;
- Step 7C: tag e ricerca globale;
- moduli Veicoli, Vestiti e Ricette.

---

## Modifica ed eliminazione

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
