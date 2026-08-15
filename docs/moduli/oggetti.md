# Modulo Oggetti generici — Step 5A + Step 5C

## Scopo

Il modulo cataloga tutto cio' che non appartiene a un modulo dedicato: attrezzi,
elettronica, elettrodomestici, valigie, mobili, accessori e altri beni di casa.

Lo Step 5A copre il ciclo minimo completo:

`Telegram -> Rust -> SQLx -> items + oggetti -> Telegram`

La revisione dei **campi della bozza** prima di `✅ Salva` fa parte dello Step
5A. Lo **Step 5C** estende lo stesso pannello agli oggetti già persistiti e
aggiunge l'eliminazione con conferma. Le foto sono state aggiunte nello Step 5B;
documenti/tag, garanzie/promemoria e prestiti restano sottostep successivi.

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
| `posizione` | testo | no | es. Garage - scaffale 2 |
| `data_acquisto` | data ISO | no | salvata come `AAAA-MM-GG` |
| `prezzo_acquisto_centesimi` | intero | no | denaro in centesimi, mai `REAL` |
| `venditore` | testo | no | es. Amazon, MediaWorld, privato |
| `valore_stimato_centesimi` | intero | no | valore attuale stimato |
| `condizione` | enum testo | no | ottimo/buono/usurato/da_riparare |
| `note` | testo | no | note libere |

> `posizione` è testuale nello Step 5A. In futuro verrà migrata verso il sistema
> condiviso di luoghi/case/stanze, dopo approvazione della relativa architettura.

Foto, scontrini, garanzie, tag e promemoria non vengono duplicati in questa
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
| foto oggetto | `📷 Foto` | `/foto <id>` |
| annulla operazione | `❌ Annulla` | `/annulla` |
| mantieni valore corrente | — | `/salta` |
| rimuovi campo opzionale aperto | — | `/rimuovi` |

`/start` apre il menu principale con Oggetti e Stato sistema; Vestiti, Veicoli
e Ricette sono gia' rappresentati esteticamente ma marcati come prossimamente.

## Creazione con pannello dettagli

1. il bot chiede solo il nome;
2. dopo il nome mostra il pannello dettagli;
3. l'utente puo' salvare subito oppure aggiungere dati opzionali;
4. `Marca e modello` e `Acquisto` sono piccoli flussi guidati;
5. `Condizione` usa quattro pulsanti;
6. `Altri dettagli` contiene descrizione, valore stimato e numero seriale;
7. le sezioni gia' compilate vengono marcate con `✅` nel pannello;
8. riaprendo un campo gia' valorizzato, il bot mostra il valore attuale prima di chiedere quello nuovo; `/salta` mantiene il valore esistente invece di sovrascriverlo;
9. `✅ Salva` inserisce `items` e `oggetti` nella stessa transazione SQL.

Una bozza incompleta vive solo in memoria: se il backend viene riavviato prima
del salvataggio, la bozza viene persa ma il database resta invariato.

## Modifica di un oggetto salvato — Step 5C

Dalla scheda di un oggetto, `✏️ Modifica` oppure `/oggetto_modifica <id>` carica
dal database tutti i valori correnti in una bozza di modifica. La bozza conserva
l'ID originale: `💾 Salva modifiche` esegue `UPDATE` su `items` e `oggetti` nella
stessa transazione, senza creare un nuovo item.

Regole UX:

- il nome è modificabile ma non può essere rimosso;
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

## Elenco e ricerca

- elenco alfabetico, 8 oggetti per pagina;
- ogni riga ha un pulsante che apre la scheda;
- la ricerca controlla nome, marca, modello, numero seriale, posizione, venditore,
  descrizione e note;
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

- Step 5D: documenti e tag;
- Step 5E: garanzie e promemoria;
- Step 5F: prestiti e storico;
- Step 6: luoghi e multi-abitazione (più case, stanze, filtri e ricerca globale),
  con architettura da confermare prima dell'implementazione.
