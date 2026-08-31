# Step 7.2I.2A — Ingredienti personalizzati, ripartenza pulita

Questo sottostep riparte dal commit I.1 `ca987da`.

## Scope

Implementa solo il core Telegram degli override ingrediente:

- elenco ingredienti della ricetta, massimo 5 per pagina;
- quantità calcolata in base a porzioni base e fattore personale;
- quantità personale inserita direttamente in chat;
- esclusione ingrediente;
- ritorno alla quantità calcolata;
- persistenza nella tabella I.0 `profilo_ricetta_ingredienti_override`.

## Routing

Non modifica `src/main.rs`.

Tutti i callback usano il namespace già esistente `foodprof:*`, quindi vengono
inoltrati dal dispatcher principale già presente in I.1.

Gli ID nei callback vengono codificati in base36 per rispettare il limite
Telegram di 64 byte.

## Fuori scope

- Storico degli override;
- planner;
- lista della spesa;
- nuove migration.

Queste parti verranno aggiunte solo dopo il collaudo del core.


## Rifiniture successive

- nell'elenco ingredienti non viene più aggiunta la carota generica davanti agli elementi normali;
  rimangono `⚙️` per quantità personalizzata e `🚫` per esclusione;
- dopo l'inserimento di una quantità personalizzata il bot conferma l'aggiornamento e torna
  automaticamente alla pagina corretta dell'elenco ingredienti.

## Export Miglioramenti

L'export ora richiede una scelta prima della creazione dello ZIP:

- Da approvare;
- Da fare;
- Fatte;
- Archiviate;
- Tutti.

Il filtro si applica anche alle tabelle figlie e agli allegati, così l'esclusione
dell'archivio può ridurre realmente la dimensione del pacchetto.

Il client HTTP Telegram usa un timeout di richiesta di 180 secondi per ridurre i falsi
timeout durante l'upload di file grandi. In caso di errore/timing incerto lo ZIP temporaneo
non viene eliminato immediatamente: resta disponibile fino alla normale pulizia degli export
orfani.
