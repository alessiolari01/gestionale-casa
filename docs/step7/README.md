# Step 7 — Fondazioni condivise e Alimentazione

**Stato: 7.3B chiuso e verificato. Branch di lavoro `step-7-alimentazione-s9`.**

Il branch `step-7-alimentazione` contiene un 7.3B parallelo scartato il 31 agosto
e non va usato: vedi `CHANGELOG.md`, voce "Due implementazioni parallele".

## Macro-fasi

| Macro-fase | Stato | Contenuto |
|---|---|---|
| 7.0 — Specifica e organizzazione | VERIFICATO | decisioni, confini e piano migration |
| 7.1 — Fondazioni condivise | OPERATIVE | utenti, spazi, membership, ruoli, vista multi-spazio, audit |
| 7.2 — Alimentazione | VERIFICATO | alimenti, ricette, profili, porzioni e override completi |
| 7.3 — Planner | IN SVILUPPO | 7.3A e 7.3B chiusi; restano spesa, turni e reminder |
| 7.4 — Integrazioni | PREVISTO | Google Calendar, email e integrazioni residue |

## Stato 7.3

- **7.3A VERIFICATO** — fondazioni: `planner_alimentari`, `planner_pasti`,
  `planner_pasto_profili`, `planner_pasto_ingredienti_snapshot`, piu' il dominio
  degli snapshot in `planner_alimentare.rs`.
- **7.3B VERIFICATO** — planner operativo su Telegram: vista settimanale
  lunedi'-domenica, dettaglio giornaliero, aggiunta e modifica dei pasti, tipo
  pasto, scelta ricetta paginata a 5, selezione multipla dei Profili, snapshot
  delle quantita' con percentuali e override, quantita' aggregate, completamento
  con congelamento, esito "saltato", segnalazione della ricetta cambiata su
  settimana, giorno e dettaglio — limitata ai pasti di oggi o futuri — e
  `🔄 Aggiorna alla ricetta attuale` con conferma esplicita.

La settimana viene creata implicitamente alla prima apertura: non esiste una
creazione manuale del planner. E' una scelta deliberata, per non aggiungere un
concetto in piu' all'utente medio.

## Prossimo ordine

1. lista della spesa aggregata dagli snapshot dei pasti non completati;
2. turni e routine;
3. reminder ed export Alimentazione;
4. integrazioni residue.

Fuori sequenza, gia' individuati e non urgenti: aritmetica delle date in Rust al
posto delle query di calendario, decisione sui pasti liberi, riallineamento di
`main`.

## Documenti Step 7

- [Roadmap](roadmap.md)
- [Decisioni architetturali](decisioni-architetturali.md)
- [Modello di condivisione](modello-condivisione.md)
- [Storico e audit](storico-e-audit.md)
- [Database e migrazioni](database-e-migrazioni.md)
- [Chiusura 7.2H](step-7.2h-profili-spazi-inviti.md)
- [Alimentazione](../moduli/alimentazione/README.md)

## Regola di stato

Usare: **PREVISTO**, **IN SVILUPPO**, **IMPLEMENTATO**, **VERIFICATO**, **RIMANDATO**. Non dichiarare un test live eseguito se richiede un account/condizione non disponibile.
