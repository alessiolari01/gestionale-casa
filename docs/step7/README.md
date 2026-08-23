# Step 7 — Fondazioni condivise e Alimentazione

**Stato:** IN SVILUPPO — checkpoint 7.0 chiuso, 7.1 tecnico in corso.

**Branch di lavoro:** `step-7-alimentazione`.

**Checkpoint tecnico verificato:** `a650bc8` sul branch
`step-7-alimentazione`, derivato dal 7.0 documentale `135dd33` e dal merge
Step 6C `219caba`.

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

Il checkpoint `a650bc8` ha introdotto schema condiviso, identità Telegram,
spazio bootstrap, `/profilo` e audit autore.

Il blocco corrente aggiunge:

- migration `20260823174500_spazi_operativi.sql`;
- unicità `abitazioni(spazio_id, nome)` e `tag(spazio_id, nome)`;
- `/spazi`, `/spazio_nuovo <nome>` e `/spazio_rinomina <nome>`;
- cambio spazio tramite pulsanti inline;
- spazio personale automatico per i nuovi utenti successivi al bootstrap;
- scoping di oggetti, luoghi, contenitori, foto e storico;
- blocco delle scritture principali per il ruolo `lettura`;
- invalidazione delle sessioni temporanee al cambio spazio;
- test espliciti contro letture e mutazioni cross-space tramite ID;
- coerenza membership/spazio attivo con fallback automatico e autoriparazione legacy;
- contesto `AuditActor` obbligatorio in produzione per le operazioni space-aware.

Gli eventi Step 6 già presenti restano nello spazio bootstrap con autore
sconosciuto/origine `legacy`: non viene inventato nulla retroattivamente.

### Limite corrente intenzionale

Gli spazi diventano operativi, ma **inviti e gestione completa dei membri non
sono ancora esposti**. Creare uno spazio non trasferisce dati esistenti e non
condivide automaticamente nulla. Uscita/eliminazione degli spazi verranno
aggiunte solo dopo aver definito in modo sicuro proprietà e destino dei dati.

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
