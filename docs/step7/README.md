# Step 7 — Fondazioni condivise e Alimentazione

**Stato: IN SVILUPPO — Step 7.2H chiuso; prossimo blocco Porzioni e override.**
**Branch:** `step-7-alimentazione`.

## Macro-fasi

| Macro-fase | Stato | Contenuto |
|---|---|---|
| 7.0 — Specifica e organizzazione | VERIFICATO | decisioni, confini e piano migration |
| 7.1 — Fondazioni condivise | OPERATIVE | utenti, spazi, membership, ruoli, vista multi-spazio, audit, accesso DB-driven |
| 7.2 — Alimentazione | IN SVILUPPO | Alimenti/Ricette/Profili operativi; Porzioni/Turni/Planner/Spesa da completare |
| 7.3 — Integrazioni | PREVISTO | Google Calendar, email e altre integrazioni |

## Stato 7.2

Completati:

- Alimenti/unità/categorie/catalogo/compatibilità;
- prodotti commerciali, formati e nutrizione;
- Ricette con ingredienti strutturati e procedimento guidato;
- accesso approvato e amministrazione;
- workflow Miglioramenti, verifica guidata e export;
- Profili alimentari separati dagli account;
- condivisione Profili tramite Spazi;
- membri degli Spazi e inviti privati Telegram;
- export progetto sanitizzato;
- gestione non distruttiva dell'input inatteso.

## Step 7.2H — chiuso

Dettaglio: [step-7.2h-profili-spazi-inviti.md](step-7.2h-profili-spazi-inviti.md).

Le verifiche che richiedono un secondo account restano differite e documentate nell'handoff; non bloccano il prossimo sviluppo.

## Prossimo ordine

1. Porzioni personali per Profilo;
2. override quantità/esclusione ingrediente;
3. turni/routine;
4. planner versionato;
5. lista della spesa;
6. reminder/export e integrazioni residue.

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
