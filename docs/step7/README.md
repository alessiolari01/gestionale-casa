# Step 7 — Fondazioni condivise e Alimentazione

**Stato:** IN SVILUPPO — specifica architetturale iniziale.

**Branch di lavoro:** `step-7-alimentazione`.

**Base di partenza:** `219caba`, merge dello Step 6C in `main`.

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
| 7.0 — Specifica e organizzazione | IN SVILUPPO | documentazione, confini, decisioni e piano migration |
| 7.1 — Fondazioni condivise | PREVISTO | utenti, spazi, membri, ruoli, inviti, proprietà dati, condivisione/copia, audit con autore, reminder trasversali |
| 7.2 — Alimentazione completa | PREVISTO | alimenti, unità, ricette, profili, porzioni, turni/routine, planner, lista spesa, export |
| 7.3 — Integrazioni e condivisione operativa | PREVISTO | inviti Telegram, Google Calendar, email, account Google e rifiniture multiutente |

I checkpoint possono diventare commit intermedi sullo stesso branch; non sono
obbligatoriamente branch o Pull Request separati.

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
