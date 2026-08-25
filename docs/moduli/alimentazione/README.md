# Modulo Alimentazione

**Stato complessivo: IN SVILUPPO — Alimentazione operativa, Ricette Step 7.2F.1 da verificare su S9.**

Alimentazione raccoglie alimenti, ricette, profili, turni/routine,
pianificazione dei pasti e lista della spesa. È progettato per uso personale e
condiviso.

## Stato delle aree

| Area | Stato |
|---|---|
| Fondazioni utenti/spazi | OPERATIVE |
| Alimenti e unità | OPERATIVI |
| Ricette | STEP 7.2F.1 — implementate nel pacchetto, da verificare |
| Profili e porzioni | PREVISTO |
| Turni/routine | PREVISTO |
| Planner pasti | PREVISTO |
| Lista della spesa | PREVISTO |
| Reminder Telegram/email | PREVISTO |
| Export PDF/immagine | PREVISTO |
| Google Calendar/email | PREVISTO — Step 7.3 |
| Dispensa/scorte | RIMANDATO |
| Sostituzioni ingredienti | RIMANDATO |

## Documentazione

- [Alimenti e unità](alimenti-e-unita.md)
- [Ricette](ricette.md)
- [Profili e porzioni](profili-e-porzioni.md)
- [Turni e routine](turni-e-routine.md)
- [Pianificazione e lista della spesa](pianificazione-e-spesa.md)
- [Reminder](reminder.md)
- [Export](export.md)
- [Google Calendar ed email](integrazioni-google-email.md)

## Principi

- alimento ≠ prodotto acquistabile ≠ scorta;
- ricette con ingredienti strutturati, non solo testo libero;
- quantità base personalizzabili per profilo/persona;
- un profilo può esistere senza account;
- pianificazione basata su date reali e partecipanti;
- turni/routine influenzano orari, luogo del pasto e preparazione;
- lista della spesa derivabile dalla pianificazione;
- condivisione e copia seguono le regole generali Step 7;
- ogni modifica condivisa deve essere attribuita all'autore nello storico.

## Relazione con altri moduli

### Acquisti

Alimentazione produce bisogni (`serve 1,5 kg di pasta`). Acquisti si occupa di
prodotti, confezioni, negozi e prezzi.

### Spese

Un acquisto reale può generare una spesa personale/condivisa senza trasformare
il planner in un modulo contabile.

### Viaggi

Un viaggio può in futuro usare pasti, liste e spese ma resta un dominio
separato.

## Prodotto commerciale e formato di vendita — Step 7.2F.0

Il prodotto commerciale non coincide più con una singola confezione. La
gerarchia di riferimento è:

```text
🥛 Formaggio spalmabile
└── 🛒 Philadelphia · Original
    ├── 📦 175 g
    ├── 📦 200 g
    └── 📦 350 g
```

Marca, nome commerciale e valori nutrizionali appartengono al prodotto.
Quantità confezione, unità e barcode/EAN appartengono invece al formato.
Ricette e ingredienti specifici continuano a referenziare
`prodotto_alimentare_id`; la scelta del formato è demandata alla Lista spesa e
ai futuri prezzi/disponibilità.
