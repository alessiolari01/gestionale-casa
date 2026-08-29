# Modulo Alimentazione

**Stato complessivo: IN SVILUPPO — Alimenti, Ricette e Profili alimentari OPERATIVI; prossimo blocco Porzioni/override.**

Alimentazione raccoglie alimenti, prodotti commerciali, ricette, profili, turni/routine, pianificazione dei pasti e lista della spesa. È progettato per uso personale e condiviso.

## Stato delle aree

| Area | Stato |
|---|---|
| Fondazioni utenti/spazi | OPERATIVE |
| Alimenti e unità | OPERATIVI/VERIFICATI |
| Categorie e filtri | OPERATIVI/VERIFICATI |
| Prodotti commerciali, formati e nutrizione | OPERATIVI/VERIFICATI |
| Ricette e procedimento guidato | OPERATIVI/VERIFICATI |
| Profili alimentari | OPERATIVI/VERIFICATI per i flussi disponibili |
| Porzioni e override | PREVISTO — prossimo sviluppo |
| Turni/routine | PREVISTO |
| Planner pasti | PREVISTO |
| Lista della spesa | PREVISTO |
| Reminder Telegram/email | PREVISTO |
| Export PDF/immagine | PREVISTO |
| Google Calendar/email | PREVISTO — Step 7.3 |
| Dispensa/scorte | RIMANDATO |
| Sostituzioni automatiche ingredienti | RIMANDATO |

## Navigazione Telegram corrente

```text
🍽️ Alimentazione
├── 🥕 Alimenti
└── 🍳 Ricette
```

Le sezioni interne mantengono elenco/creazione/ricerca/filtri pertinenti. Le liste operative usano massimo 5 elementi per pagina e rispettano la UI a schermata singola.

## Alimenti

- catalogo base globale;
- alimenti personali/condivisi;
- proprietà separata dalla visibilità;
- categorie molti-a-molti con filtri OR;
- unità strutturate e nomi descrittivi (`grammi (g)`, `chilogrammi (kg)`, ecc.);
- ricerca anche tramite marca/nome dei prodotti commerciali, restituendo l'alimento generico;
- collaboratori tramite permessi espliciti;
- nessun ID tecnico mostrato in Telegram.

## Prodotti commerciali e formati

Gerarchia:

```text
Alimento
└── Prodotto commerciale
    ├── Formato 1
    ├── Formato 2
    └── Formato N
```

Marca, nome commerciale e nutrizione appartengono al prodotto. Quantità, unità e barcode/EAN appartengono al formato acquistabile. I formati possono essere modificati o eliminati.

La Ricetta può fissare il prodotto commerciale ma **non il formato**: la scelta della confezione appartiene alla futura Lista spesa.

## Ricette

Operative con:

- proprietà/visibilità/permessi;
- ingredienti strutturati;
- prodotto commerciale opzionale;
- unità proposta dal default alimento ma modificabile prima della quantità;
- procedimento a step con foto/video;
- vista completa e procedura guidata;
- ricerca per nome, categoria e ingredienti;
- categoria come filtro nella ricerca ingredienti;
- archiviazione ed eliminazione definitiva.

Dettagli: [Ricette](ricette.md).

## Prossima sequenza

1. porzioni e override in [Profili e porzioni](profili-e-porzioni.md);
2. [Turni e routine](turni-e-routine.md);
3. [Pianificazione e lista della spesa](pianificazione-e-spesa.md);
4. [Reminder](reminder.md) ed [Export](export.md).

## Principi

- alimento ≠ prodotto commerciale ≠ formato ≠ scorta;
- ricette con ingredienti strutturati, non testo libero;
- profilo alimentare separato e opzionalmente collegabile a un account;
- quantità base personalizzabili per profilo/persona;
- pianificazione basata su date reali e partecipanti;
- lista della spesa derivabile dalla pianificazione;
- condivisione e copia seguono le regole generali Step 7;
- ogni modifica condivisa deve poter essere attribuita all'autore nello storico.

## Documentazione

- [Alimenti e unità](alimenti-e-unita.md)
- [Ricette](ricette.md)
- [Profili e porzioni](profili-e-porzioni.md)
- [Turni e routine](turni-e-routine.md)
- [Pianificazione e lista della spesa](pianificazione-e-spesa.md)
- [Reminder](reminder.md)
- [Export](export.md)
- [Google Calendar ed email](integrazioni-google-email.md)
