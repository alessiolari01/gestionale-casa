<!-- MIGLIORAMENTI_CHIUSURA_7_2H -->
# Aggiornamento 7.2H — verifica guidata ed export tecnici

Regola operativa: un miglioramento implementato ma non ancora provato è `Fatto · da verificare`; quando l'utente conferma che funziona, va direttamente in `📦 Archiviato`. Le verifiche che richiedono un secondo account possono restare differite con piano guidato senza bloccare lo sviluppo.

L'area Miglioramenti espone `📦 Esporta miglioramenti` e `📦 Esporta progetto`. Il secondo genera un handoff tecnico sanitizzato e auto-documentante, includendo `_project_handoff/CURRENT_STATE.md` e senza dati sensibili/runtime.

# Modulo Miglioramenti

**Operativo.** `src/modules/miglioramenti.rs`, `📋 Miglioramenti` dal menù principale.

`💡 Miglioramenti` è il backlog interno del gestionale. Consente di registrare una richiesta direttamente dalla schermata in cui nasce, allegare prove, implementarla, collaudarla e archiviarla senza perdere il contesto.

Il dominio è globale al gestionale, non legato a una singola casa/spazio.

## Workflow

Stati DB attivi:

| Stato | Significato |
|---|---|
| `da_approvare` | suggerimento di utente normale in attesa di decisione admin |
| `da_fare` | approvato / creato da admin e ancora da implementare |
| `fatto` | implementato, in attesa di collaudo/archiviazione |
| `scartato` | rifiutato, eliminabile singolarmente o in blocco |

La verifica non è un valore aggiuntivo del `CHECK stato`: usa `verifica_esito`, `verifica_note`, `verificato_il` e `verificato_da_utente_id`.

Flussi:

```text
utente normale
  da_approvare → approvazione admin → da_fare → fatto → 🧪 verificato → archivio

admin
  da_fare → fatto → 🧪 verificato → archivio
```

`✅ Fatto` **non archivia automaticamente**. Il record resta attivo finché l'amministratore principale non verifica e preme `📦 Archivia miglioramento`.

Se testo o allegati vengono modificati dopo `fatto`/verifica, il miglioramento torna `da_fare`: una prova precedente non può certificare contenuto cambiato.

## Permessi

Utente normale approvato:

- crea suggerimenti;
- legge/modifica/elimina i propri suggerimenti ancora gestibili;
- aggiunge screenshot/allegati ai propri.

Admin:

- vede il backlog globale;
- approva/scarta;
- segna Fatto;
- collauda e archivia;
- elimina gli scartati anche in blocco con doppia conferma.

Admin principale:

- tutte le funzioni sopra;
- `📦 Esporta miglioramenti`.

Ogni controllo sensibile è ripetuto lato backend.

## Lettura amministrativa

`letto_admin_il` è separato dallo stato. `🆕` significa soltanto “non ancora letto dall'admin”; aprire o decidere l'elemento lo marca come letto.

La stessa semantica è disponibile per le richieste di accesso.

## Paginazione e navigazione

- massimo 5 elementi per pagina;
- riga di paginazione comune (`modules::liste`):
  `⬅️ Precedente | n/tot | Successiva ➡️`, assente con una pagina sola;
- il testo della lista non elenca i miglioramenti (C1): l'etichetta porta
  stato, testo del suggerimento, il segno `🆕` di non letto e l'icona
  dell'esito del collaudo. Autore e allegati sono nel dettaglio;
- il menu porta il conteggio sull'etichetta (C7): `🟡 Da approvare · 0` invece
  di una riga di testo sopra il pulsante. Era l'esempio da cui la convenzione è
  nata, e con quel blocco è sparita anche la frase «Usa i pulsanti qui sotto»,
  che C2 vieta;
- l'archivio resta un elenco testuale: lì non ci sono pulsanti per riga, quindi
  non c'è duplicazione da togliere. Ne condivide intestazione e paginazione;
- dettaglio → Indietro conserva lista e pagina;
- annullare modifica testo conserva il contesto;
- cambio stato torna alla lista appropriata;
- `💡 Migliora` annullato torna alla schermata da cui è stato aperto;
- descrizioni molto lunghe vengono lette a pagine senza il vecchio limite applicativo di 2000 caratteri.

## Descrizione multimessaggio

La creazione può raccogliere più messaggi consecutivi e unirli in ordine fino a `Fine descrizione`. Il DB usa `TEXT`; resta soltanto un limite interno di sicurezza molto più alto del limite di un singolo messaggio Telegram.

## Piani di verifica

`miglioramento_piani_verifica` conserva, quando disponibile:

- titolo del collaudo;
- istruzioni;
- etichetta azione;
- callback per aprire direttamente la schermata da testare.

Foto/video/documenti di verifica sono separati dagli allegati descrittivi originali.

## `💡 Migliora` contestuale

Il pulsante è trasversale e, quando possibile, condivide la riga di navigazione con `🏠 Menù principale`.

Il contesto include:

- sezione reale corrente;
- titolo/schermata;
- azioni recenti, dalla più recente;
- nome effettivo del pulsante premuto;
- destinazione dell'azione quando nota.

I callback tecnici non vengono esposti all'utente.

## UI a schermata singola

Il modulo usa l'infrastruttura `ContextBot`:

- una sola schermata UI principale attiva per chat;
- vecchi callback non devono duplicare azioni;
- messaggi/media temporanei del bot sono ripuliti alla navigazione;
- input dell'utente acquisiti con successo vengono eliminati nei wizard supportati;
- lo stato della schermata attiva è persistito in `telegram_ui_state` e sopravvive ai riavvii.

## Export amministrativo

Pulsante:

```text
📦 Esporta miglioramenti
```

Flusso:

```text
admin principale
→ genera ZIP sanitizzato
→ Telegram invia il documento
→ download manuale
→ ✅ Ho scaricato il file
→ cancellazione copia locale dall'S9
```

Exporter: `scripts/export_miglioramenti.py`.

Contenuto utile:

- snapshot del repository, comprese modifiche non committate;
- branch, HEAD, stato Git e ultimi commit;
- miglioramenti attivi e archivio;
- piani/allegati di verifica;
- schema SQL del modulo;
- utenti di riferimento sanitizzati;
- riepilogo Markdown;
- allegati locali collegati ai miglioramenti.

Esclusioni:

- `.env` e token;
- DB SQLite completo;
- `.git`;
- `target`;
- backup;
- runtime non necessario;
- identificativi/percorso non necessari quando possono essere sanitizzati.

Directory temporanea:

```text
data/tmp/miglioramenti_export/
```

Gli export orfani più vecchi di 24 ore vengono ripuliti all'avvio/nuova esportazione. La cancellazione manuale dopo conferma accetta soltanto ZIP con naming previsto nella directory dedicata.

## Tabelle principali

```text
miglioramenti
miglioramento_allegati
miglioramento_piani_verifica
miglioramento_verifica_allegati
miglioramenti_archivio
miglioramento_archivio_allegati
miglioramento_archivio_verifica_allegati
```

## Migration

```text
20260825153000_accesso_miglioramenti.sql
20260826024500_miglioramenti_workflow_admin.sql
20260826123000_miglioramenti_verifica_guidata.sql
20260826223000_miglioramenti_contesto_rifiniture.sql
20260827003000_miglioramenti_ultimo_passaggio.sql
20260827014500_miglioramenti_finalissimi.sql
20260827104500_runtime_ui_persistente.sql
20260827123000_esporta_miglioramenti_bot.sql
```

Tutte quelle già applicate al DB reale sono immutabili.

## Verifica finale

Sul Galaxy S9:

```text
153 test passati
0 falliti
cargo check OK
Clippy -D warnings OK
```

L'export è stato collaudato realmente: ZIP ricevuto da Telegram, scaricato e poi cancellato dalla directory temporanea tramite `✅ Ho scaricato il file`.

## Backlog residuo al checkpoint

- **#7**: eliminazione/reset/revoca account — rimane `da_fare`, richiede uno step amministrativo dedicato;
- **#9**: `🧪 Zona test` / aggiornamenti quasi zero-downtime — requisito futuro e ultimo dell'infrastruttura, non da implementare durante l'attuale uso personale.

## Export tecnico del progetto

L'amministratore principale può usare `📦 Esporta progetto` per generare uno ZIP adatto a un handoff tecnico completo. L'archivio contiene lo stato corrente dei sorgenti, anche se non ancora committato, insieme a migration, documentazione, script e metadati Git essenziali.

L'export non contiene dati runtime o sensibili: `.env`, token, database, `data/`, allegati utente, backup, `target/`, `.git/`, cache e ZIP temporanei sono esclusi. Prima della creazione viene effettuato anche un controllo conservativo dei file testuali per evitare la fuoriuscita accidentale di credenziali evidenti.
