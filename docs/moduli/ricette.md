# Ricette

**Operativo.** `src/modules/ricette.rs`, `🍽️ Alimentazione → 🍳 Ricette`.

## Modello

Le Ricette seguono gli stessi principi del resto del gestionale:

- proprietario separato dalla visibilità negli spazi;
- una sola ricetta centrale, condivisibile in zero, uno o più spazi;
- nessuna copia della ricetta quando viene condivisa;
- visibilità ≠ modifica ≠ gestione permessi;
- backend fail-closed;
- nessun ID tecnico mostrato nella UI utente.

Le tabelle di base restano:

- `ricette`;
- `ricetta_spazi`;
- `ricetta_ingredienti`.

I permessi riusano `inviti_risorsa` / `permessi_risorsa` con
`tipo_risorsa = 'ricetta'`.

## Ingredienti

Ogni ingrediente mantiene sempre il riferimento all'alimento generico:

```text
ricetta_ingredienti
├── ricetta_id
├── alimento_id                 obbligatorio
├── prodotto_alimentare_id      opzionale
├── quantita
├── unita_misura_id
├── note                        opzionali
└── opzionale
```

Se l'alimento possiede prodotti commerciali, il wizard permette di scegliere:

```text
🌐 Usa alimento generico
oppure
🛒 Scegli prodotto specifico
```

Il prodotto specifico non sostituisce mai `alimento_id`. Un trigger DB verifica
che il prodotto appartenga allo stesso alimento.

### Prodotto ≠ formato

Dal Step 7.2F.0 un prodotto commerciale può avere più formati di vendita.
La Ricetta salva eventualmente il prodotto, **mai il formato acquistabile**.

Esempio:

```text
Ricetta:
Philadelphia · Original
150 g necessari

Prodotto:
Philadelphia · Original

Formati disponibili:
175 g
200 g
350 g
```

La scelta del formato verrà effettuata dalla futura Lista spesa in base alla
quantità aggregata, disponibilità, prezzo e avanzo previsto.

## Porzioni

`ricette.porzioni_base` conserva il numero di porzioni per cui sono state
inserite le quantità. Quantità e unità degli ingredienti sono strutturate e
restano indipendenti dalle confezioni dei prodotti.

## Procedimento strutturato

La migration `20260825231500_ricette_procedimento_guidato.sql` aggiunge:

- `ricetta_step`;
- `ricetta_step_media`;
- `v_ricetta_step_con_media`.

Il procedimento non viene più modellato come un unico testo libero. Ogni
ricetta ha step ordinati e numerati:

```text
Ricetta
└── Procedimento
    ├── Step 1
    │   ├── testo
    │   ├── 0..N foto
    │   └── 0..N video
    ├── Step 2
    │   ├── testo
    │   └── media opzionali
    └── Step N
```

La colonna legacy `ricette.procedimento` resta nello schema per compatibilità
storica ma non è più la fonte autorevole. Se al momento della migration esiste
un vecchio procedimento testuale, viene convertito conservativamente nello
Step 1.

Gli allegati sono salvati localmente sotto:

```text
data/media/ricette/<ricetta_id>/<step_id>/
```

Durante la creazione vengono prima salvati sotto una cartella `_draft` e
spostati nella posizione definitiva solo dopo il salvataggio della ricetta.

## Due modalità di consultazione

Gli stessi step alimentano due viste differenti.

### 📖 Procedimento completo

Mostra tutti gli step in ordine, con indicazione degli allegati disponibili.
Se il testo supera il limite di un singolo messaggio Telegram, viene suddiviso
in più messaggi senza perdere step. Gli step con foto/video espongono un
pulsante per aprire i media associati.

### 👨‍🍳 Procedura guidata

Mostra un solo step alla volta:

```text
👨‍🍳 Procedura guidata
Step 2/7

[testo dello step]

📎 Vedi foto/video dello step

⬅️ Step precedente | 2/7 | Step successivo ➡️
```

Il pulsante centrale `2/7` è informativo/no-op. All'ultimo step compare
`✅ Termina`.

## Creazione Telegram

Flusso strutturale:

```text
➕ Nuova ricetta
→ Nome
→ Porzioni base
→ Ingredienti
   → alimento
   → generico / prodotto specifico
   → quantità
   → unità
→ Procedimento
   → testo Step 1
   → foto/video opzionali
   → aggiungi Step 2 / fine procedimento
→ Visibilità
→ Riepilogo
→ Salva
```

Sono richiesti almeno un ingrediente e uno step.

## Modifica

La UI operativa permette almeno:

- modifica nome;
- modifica porzioni;
- aggiunta/rimozione ingredienti;
- aggiunta/modifica/eliminazione step;
- spostamento step su/giù con numerazione coerente;
- aggiunta/rimozione foto e video per step;
- modifica visibilità;
- gestione collaboratori;
- archiviazione da parte del proprietario.

Una ricetta deve mantenere almeno uno step.

## Elenco, dettaglio e ricerca

Menu Ricette:

```text
🍳 Ricette
├── 📋 Elenco ricette
├── ➕ Nuova ricetta
├── 🔎 Cerca
└── 🥕 Cerca per ingredienti
```

Gli elenchi usano 5 ricette per pagina e il pulsante centrale pagina/totale è
informativo.

La ricerca per nome usa `nome_normalizzato` e rispetta la visibilità corrente.

## Ricerca per ingredienti

La ricerca multi-ingrediente usa semantica **OR** e ordina per numero di
corrispondenze:

```text
Richiesti: pollo + riso + zucchine

Ricetta A → 3/3
Ricetta B → 2/3
Ricetta C → 1/3
```

A parità vengono usati nome ricetta e ID interno stabile; l'ID non viene
mostrato all'utente.

## Compatibilità alimentare

Il dettaglio può usare `v_ricetta_compatibilita_alimentare` per derivare la
compatibilità dagli ingredienti:

- almeno un ingrediente `no` → ricetta `no`;
- nessun `no` ma almeno un `verificare`/dato mancante → `da verificare`;
- tutti `si` → compatibile.

Le etichette sono un supporto gestionale e non sostituiscono la verifica delle
etichette reali in caso di allergie/intolleranze.

## Permessi e condivisione

- proprietario: modifica e gestione;
- permesso `Edit`: modifica contenuti ma non gestione dei permessi;
- permesso `Manage`: modifica + gestione permessi;
- semplice visibilità nello stesso spazio: sola lettura;
- ruolo admin di sistema: non rende automaticamente proprietario della ricetta.

## Da non confondere con le rifiniture UX

Il macro-step punta alla struttura e alle funzioni principali. Piccole
rifiniture di testi, disposizione pulsanti e scorciatoie possono essere
registrate in `💡 Miglioramenti` senza bloccare lo sviluppo strutturale.

## Rifiniture operative consolidate nel 7.2G

Dopo gli smoke test sono state integrate nel flusso principale:

- durante l'aggiunta ingrediente viene prima proposta l'unità predefinita dell'alimento e resta disponibile `📏 Cambia unità`; solo dopo viene richiesta la quantità;
- nella ricerca per ingredienti il primo alimento può essere digitato direttamente, senza il passaggio ridondante `Aggiungi ingrediente`;
- il filtro categoria restringe i risultati degli alimenti nella ricerca per ingredienti e non viene trattato come ingrediente alternativo;
- la ricerca Ricette espone separatamente ricerca per categoria e ricerca per ingredienti;
- al termine dell'ultimo step la procedura guidata dichiara esplicitamente che la ricetta è terminata;
- oltre all'archiviazione è disponibile l'eliminazione definitiva della ricetta con le necessarie conferme/autorizzazioni;
- la navigazione segue la UI Telegram a schermata singola e include `💡 Migliora` contestuale dove previsto.
