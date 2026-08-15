# Storico trasversale — Step 6B

## Stato

Lo Step 6B introduce lo storico tecnico trasversale del gestionale.

Implementazione verificata sul Galaxy S9:
- `cargo fmt --all -- --check`;
- `cargo check --locked`;
- `cargo test --locked`: 37/37 test superati;
- `cargo clippy --all-targets --locked -- -D warnings`;
- test runtime Telegram completato con storico globale, storico individuale, dettaglio eventi, paginazione e filtri combinati.

Lo step entra nella baseline ufficiale solo dopo Pull Request, CI GitHub verde e merge su `main`.

## Architettura

Lo storico è trasversale e riutilizzabile da oggetti, abitazioni, stanze, foto e moduli futuri.

La migration `migrations/20260815215400_storico.sql` introduce:
- `storico_entita`: identità storica permanente, separata dalla riga applicativa viva;
- `storico_eventi`: intestazione dell'evento e snapshot di nome/casa/stanza;
- `storico_cambiamenti`: solo i campi realmente modificati, con prima/dopo;
- `storico_cambi_luogo`: casa/stanza prima e dopo, con ID storici e snapshot dei nomi.

Il backfill iniziale crea identità storiche per gli elementi già esistenti ma non inventa eventi retroattivi.

## Regole

- `NULL -> valore`: aggiunta;
- `valore A -> valore B`: modifica;
- `valore -> NULL`: rimozione;
- valore invariato: nessun cambiamento salvato;
- modifica no-op: nessun evento;
- stessa posizione scelta di nuovo: nessun evento di spostamento.

Gli eventi sopravvivono alla cancellazione dell'entità applicativa. Il riuso futuro di un ID applicativo crea una nuova identità storica e non fonde le cronologie.

## Eventi coperti

- creazione/modifica/eliminazione oggetto;
- assegnazione/spostamento/rimozione luogo;
- foto aggiunta;
- creazione/rinomina/eliminazione casa;
- creazione/rinomina/eliminazione stanza;
- effetti secondari sui luoghi quando casa o stanza vengono eliminate.

Gli eventi oggetto conservano anche il contesto casa/stanza corrente quando l'evento non è direttamente uno spostamento.

## UI Telegram

Storico globale:
- pulsante `📜 Storico` nel menu principale;
- comando `/storico`;
- 6 eventi per pagina;
- dettaglio evento;
- prima/dopo e cambio luogo;
- orario locale in visualizzazione.

Storico individuale:
- pulsante `📜 Storico` nella scheda oggetto;
- mostra soltanto gli eventi della corrente identità storica.

Filtri globali combinabili:
- periodo: tutto, oggi, ultimi 7 giorni, ultimi 30 giorni;
- modulo;
- operazione;
- casa;
- stanza;
- elemento specifico.

I filtri restano attivi durante paginazione e dettaglio. Casa/stanza usano identità storiche e, per gli spostamenti, considerano anche il prima/dopo di `storico_cambi_luogo`.

I callback dei filtri usano prefisso compatto `h:` e base62 per restare nel limite Telegram di 64 byte senza dipendere da sessioni volatili.

## Test

Suite finale verificata: **37/37 test**.

Sono coperti anche:
- migration senza eventi inventati;
- prima/dopo strutturato;
- contesto luogo su oggetti e foto;
- no-op senza evento;
- parsing callback;
- formattazione valori;
- lettura globale e individuale;
- codec filtri compatto;
- filtri combinati;
- filtro casa sul prima/dopo di uno spostamento.

## Sviluppi futuri

Le nuove funzioni comuni (documenti, promemoria, tag, archivio, prestiti, ecc.) dovranno riusare questa infrastruttura invece di introdurre tabelle `*_storico` duplicate.
