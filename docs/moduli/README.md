# Documentazione dei moduli

Ogni modulo mantiene una documentazione vicina all'implementazione o alla
specifica approvata. I documenti devono distinguere chiaramente fra
**PREVISTO**, **IN SVILUPPO**, **IMPLEMENTATO**, **VERIFICATO** e **RIMANDATO**.

Per ogni area vanno descritti, quando applicabili:

- schema dati e relazioni;
- regole di dominio e casi limite;
- UI/comandi Telegram;
- proprietà, condivisione e permessi;
- storico/audit;
- comportamento di modifica/eliminazione;
- test automatici e verifiche runtime;
- sviluppi futuri già approvati.

## Moduli implementati/verificati

- [Oggetti](oggetti.md)
- [Foto](foto.md)
- [Modifica ed eliminazione](modifica-eliminazione.md)
- [Luoghi](luoghi.md)
- [Navigazione dei luoghi](navigazione-luoghi.md)
- [Contenitori](contenitori.md)
- [Storico](storico.md)

## Step 7 — in progettazione/implementazione

- [Alimentazione](alimentazione/README.md) — modulo principale dello Step 7;
- [Ricette](ricette.md) — rimando di compatibilità alla nuova documentazione
  Alimentazione.

## Moduli futuri già specificati

- [Acquisti e prezzi](acquisti/README.md) — RIMANDATO;
- [Viaggi](viaggi/README.md) — RIMANDATO;
- [Spese](spese/README.md) — RIMANDATO;
- Vestiti — futuro;
- Veicoli — futuro.

Le fondamenta condivise dello Step 7 sono documentate in
[`docs/step7/README.md`](../step7/README.md).
