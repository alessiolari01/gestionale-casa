# Storico trasversale — Step 6B

## Estensione Step 6C.4 — contenitori e percorsi (verificata)

La migration `20260820230000_storico_contenitori.sql` estende il modello 6B senza cambiare le migration già applicate:
- `storico_eventi`: `contenitore_storico_id` + `contenitore_percorso_snapshot`;
- `storico_cambi_luogo`: contenitore/percorso prima e dopo;
- backfill in `storico_entita` dei contenitori esistenti, senza eventi retroattivi.

Il percorso viene memorizzato come snapshot (`Armadio / Ripiano 2 / Scatola`), mentre casa e stanza continuano ad avere i propri snapshot. In Telegram il risultato è, per esempio, `Casa principale / Garage / Armadio / Ripiano 2 / Scatola`.

Eventi container-aware:
- contenitore: creazione, rinomina, modifica descrizione, spostamento, eliminazione;
- oggetto: assegnazione/spostamento/rimozione con percorso contenitore prima/dopo;
- oggetti e foto: contesto del luogo completo anche quando l'evento non è uno spostamento;
- eliminazione stanza: promozione dei contenitori/oggetti alla casa con eventi figli;
- eliminazione casa: eliminazione storica dei contenitori e rimozione del luogo degli oggetti, mantenendo gli snapshot precedenti.

`evento_padre_id` distingue l'azione richiesta dagli effetti automatici sul sottoalbero. La rinomina, invece, non viene trattata come movimento fisico dei discendenti.


## Stato

Lo Step 6B introduce lo storico tecnico trasversale del gestionale.

Implementazione verificata sul Galaxy S9:
- `cargo fmt --all -- --check`;
- `cargo check --locked`;
- `cargo clippy --all-targets --locked -- -D warnings`;
- test runtime Telegram completato con storico globale, storico individuale, dettaglio eventi, paginazione e filtri combinati.


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

Gli eventi oggetto conservano anche il contesto completo casa/stanza/contenitore corrente quando l'evento non è direttamente uno spostamento.

## UI Telegram

Storico globale:
- pulsante `📜 Storico` nel menu principale;
- comando `/storico`;
- 5 eventi per pagina;
- dettaglio evento;
- prima/dopo e cambio luogo;
- orario locale in visualizzazione.

**Etichette della lista (C1).** Il messaggio non elenca più gli eventi: porta
solo il titolo con il totale, la posizione e il filtro attivo. Ogni evento è un
pulsante `{icona} {gg/mm/aa hh:mm} · {entità}`, per esempio
`🍽️ 31/08/26 19:49 · Giorgia`.

Sull'etichetta non c'è più l'azione a parole. In una lista filtrata per azione
è identica su ogni riga, quindi non distingue niente e occupa il posto di ciò
che distingue: prima il risultato erano tre pulsanti identici
`🍽 Porzione modificata · Giorgia`. L'azione resta come icona; il nome per
esteso, l'autore, il modulo e il luogo sono nel dettaglio.

**Limite noto.** Due eventi dello stesso tipo, sulla stessa entità, nello
stesso minuto restano indistinguibili sull'etichetta — è successo davvero con
due modifiche opposte della stessa porzione. Restano adiacenti e in ordine
cronologico, e si separano aprendoli.

**Cosa legge la lista.** Da quando il testo non ripete gli eventi, la query
della lista chiede cinque colonne invece di tredici: luogo, stanza,
contenitore, spazio, autore, origine, automatico e tipo entità venivano letti a
ogni apertura per non essere mostrati. Il dettaglio li legge come prima.

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
# Storico e audit multiutente

**Operativo.** Audit di autore e origine attivi dal 7.1.

Lo storico attuale registra già eventi, cambiamenti prima/dopo, cambi di luogo,
percorsi container-aware ed eventi padre/figlio. Lo Step 7 deve aggiungere il
contesto multiutente senza riscrivere gli eventi esistenti.

## Obiettivo

Ogni evento condiviso deve rispondere almeno a:

- **cosa** è cambiato;
- **chi** ha richiesto/eseguito l'azione;
- **quando**;
- **dove/nel quale spazio**;
- **come** è nata l'azione (utente, sistema, integrazione);
- quali effetti sono stati automatici.

## Autore umano

Per un'azione avviata da un utente vanno conservati:

- riferimento all'utente interno;
- snapshot del nome visualizzato utile allo storico;
- eventuale origine, per esempio Telegram.

Esempio UI:

```text
✏️ Oggetto modificato
👤 Alessio
🕒 23/08/2026 18:42

Nome:
Vecchio -> Nuovo
```

## Eventi automatici

Gli effetti derivati non devono sembrare azioni manuali separate.

Esempio:

```text
#101 Spostato Armadio
👤 Richiesto da Alessio

#102 Scatola spostata
⚙️ Effetto automatico di #101

#103 Trapano spostato
⚙️ Effetto automatico di #101
```

L'attuale `evento_padre_id` è il meccanismo naturale da preservare.

## Sistema e integrazioni

Sono previste origini come:

- utente/Telegram;
- sistema;
- reminder/automazione;
- integrazione Google/email.

Un evento automatico può conservare anche l'utente che ha originato il flusso,
quando applicabile.

## Snapshot

Come per i percorsi dei contenitori, gli elementi utili a interpretare il
passato non devono dipendere esclusivamente dai dati correnti.

Candidati a snapshot:

- nome autore;
- nome entità;
- contesto del luogo;
- spazio proprietario dell'entità;
- spazio della posizione fisica, separato dal proprietario;
- spazio prima/dopo nei cambi di luogo;
- valori prima/dopo già previsti dallo storico.

## Filtri

Lo storico globale dovrà poter aggiungere almeno:

- spazio;
- autore;
- origine automatica/manuale;

mantenendo i filtri esistenti per modulo, operazione, periodo, luogo ed entità.

## Moduli futuri

La stessa regola vale per Alimentazione, Acquisti, Viaggi e Spese.

Esempi:

```text
💰 Prezzo base aggiornato
👤 Laura
1,29 € -> 1,39 €
```

```text
✅ GoPro verificata al rientro
👤 Alessio
Viaggio: Corfù
```

```text
💸 Spesa modificata
👤 Marco
48,00 € -> 52,00 €
```

## Migration 7.1

La migration `20260823153000_fondazioni_condivise.sql` applica la strategia definitiva:

- eventi pre-Step 7: `attore_utente_id = NULL`;
- `origine_azione = legacy`;
- spazio snapshot = `Spazio principale`;
- `automatico = 1` per gli eventi che avevano già `evento_padre_id`;
- nessun evento nuovo viene creato per simulare il passato.

I nuovi eventi Telegram ricevono automaticamente l'utente interno e lo snapshot del nome tramite il contesto task-local installato nel dispatcher. Questo evita di dover propagare manualmente l'autore attraverso tutte le funzioni Step 6.


---

## Cosa scrive nello storico, e cosa no

Verificato sul codice il 2 settembre 2026. **Non tutti i moduli emettono
eventi**, e il principio "ogni modifica condivisa deve poter essere attribuita
all'autore" oggi non vale ovunque.

Scrivono nello storico: alimenti e prodotti, profili alimentari, porzioni e
override, oggetti, luoghi, contenitori, foto.

**Non scrivono nello storico**: ricette, pasti del planner, membership e inviti
degli spazi. È una lacuna nota, non una scelta: chi tocca quei moduli valuti se
colmarla.
