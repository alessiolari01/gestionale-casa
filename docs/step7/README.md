# Step 7 — Fondazioni condivise e Alimentazione

**Stato:** IN SVILUPPO — checkpoint 7.0 chiuso, 7.1 tecnico in corso.

**Branch di lavoro:** `step-7-alimentazione`.

**Base Step 7:** `135dd33` sul branch `step-7-alimentazione` (7.0 documentale), derivata dal merge Step 6C `219caba`.

Lo Step 7 nasce dopo la chiusura dello Step 6C e sostituisce la precedente
idea di usare lo Step 7 per documenti/garanzie. La priorità attuale è costruire
le fondamenta multiutente/condivise necessarie ai moduli futuri e, sopra di
esse, il primo grande dominio applicativo: **Alimentazione**.

Il vecchio prototipo `gestionale_step7_prototipo_bundle` non è una sorgente di
verità e non va applicato al repository. Questa documentazione è la specifica
corrente approvata.

## Macro-fasi

Lo Step 7 viene mantenuto intenzionalmente in pochi blocchi grandi.

| Macro-fase | Stato | Contenuto |
|---|---|---|
| 7.0 — Specifica e organizzazione | VERIFICATO | documentazione, confini, decisioni e piano migration |
| 7.1 — Fondazioni condivise | IN SVILUPPO | utenti, spazi, membri, ruoli, inviti, proprietà dati, condivisione/copia, audit con autore, reminder trasversali |
| 7.2 — Alimentazione completa | PREVISTO | alimenti, unità, ricette, profili, porzioni, turni/routine, planner, lista spesa, export |
| 7.3 — Integrazioni e condivisione operativa | PREVISTO | inviti Telegram, Google Calendar, email, account Google e rifiniture multiutente |

I checkpoint possono diventare commit intermedi sullo stesso branch; non sono
obbligatoriamente branch o Pull Request separati.


## Checkpoint tecnico corrente — 7.1

La prima implementazione della macro-fase 7.1 introduce:

- migration `20260823153000_fondazioni_condivise.sql`;
- `utenti`, `account_telegram`, `spazi`, `membri_spazio`, `preferenze_utente`, `inviti_spazio`;
- spazio bootstrap `#1` per i dati Step 6 esistenti;
- `spazio_id` sulle entità radice già operative (`items`, `abitazioni`, `tag`, storico);
- trigger contro collegamenti item/casa e item/tag cross-space;
- risoluzione runtime Telegram → utente interno;
- primo account autorizzato come proprietario dello spazio bootstrap e successivi come amministratori durante la fase di compatibilità;
- `/profilo` e pulsante `👤 Profilo e spazio` per verificare identità, spazio e ruolo;
- storico con autore, origine, snapshot dello spazio e distinzione degli effetti automatici.

Gli eventi Step 6 già presenti restano con autore sconosciuto e origine `legacy`: non viene attribuito retroattivamente un autore inventato.

### Limite transitorio intenzionale

La 7.1 **non abilita ancora la creazione/cambio di spazio nella UI** e non rende ancora tutte le query Step 6 space-aware. Per compatibilità, le nuove righe create dal codice Step 6 continuano a usare lo spazio `#1` tramite default. Questo evita di esporre un selettore di spazio prima che ogni query sia realmente isolata. Inviti e condivisione operativa verranno attivati solo dopo lo scoping completo.

## Obiettivi architetturali

1. Il database resta **centrale**: non si condivide il file SQLite tra persone.
2. L'identità interna di una persona è separata dagli account Telegram/Google.
3. I dati condivisibili appartengono a uno **spazio** personale o condiviso.
4. Condividere e copiare sono operazioni differenti.
5. Lo storico deve sempre sapere **chi ha fatto cosa**, salvo azioni marcate
   esplicitamente come automatiche/sistema.
6. Le funzioni trasversali vanno riusate dai moduli invece di essere replicate.
7. Le nuove scelte non devono rompere case, stanze, contenitori, oggetti,
   foto e storico già esistenti.
8. Le migration di sviluppo vengono provate anche sul DB di test esistente;
   prima del go-live è ammesso un reset manuale del solo DB di sviluppo.

## Moduli influenzati dalla progettazione

### Implementazione nello Step 7

- [Alimentazione](../moduli/alimentazione/README.md)
- utenti/spazi/condivisione;
- storico/audit;
- reminder trasversali;
- integrazioni Google/email necessarie all'Alimentazione.

### Specificati ora, implementati dopo lo Step 7

- [Acquisti e prezzi](../moduli/acquisti/README.md)
- [Viaggi](../moduli/viaggi/README.md)
- [Spese](../moduli/spese/README.md)

Sono documentati adesso perché influenzano i confini del modello dati e aiutano
ad evitare scelte Step 7 che li renderebbero difficili in futuro.

## Documenti Step 7

- [Roadmap interna](roadmap.md)
- [Decisioni architetturali](decisioni-architetturali.md)
- [Modello di condivisione](modello-condivisione.md)
- [Storico e audit](storico-e-audit.md)
- [Database e migration](database-e-migrazioni.md)

## Regola di stato della documentazione

Le funzionalità usano uno dei seguenti stati:

- **PREVISTO** — approvato ma non implementato;
- **IN SVILUPPO** — implementazione in corso;
- **IMPLEMENTATO** — codice presente ma non ancora completamente verificato;
- **VERIFICATO** — test automatici e verifiche runtime previste superati;
- **RIMANDATO** — decisione conservata ma fuori dal perimetro corrente.

Nessuna funzionalità va descritta come implementata solo perché è stata
specificata in questi documenti.
