# Roadmap funzionale

Questo documento raccoglie i requisiti futuri già emersi senza trasformarli
automaticamente in decisioni implementative. Le scelte architetturali indicate
come **da confermare** non devono produrre migration o codice persistente prima
dell'approvazione.

## Stato corrente

- Step 1-4: chiusi.
- Step 5A — Oggetti generici: verifiche runtime completate sul Galaxy S9; lo
  step è chiuso quando questa revisione è presente su `main` con CI verde.

## Sequenza proposta

1. **Step 5B — Foto oggetti**
   - aggiunta, visualizzazione e gestione foto collegate a `items`;
   - uso della tabella core `foto`;
   - salvataggio locale dei file da definire nello step.
2. **Step 5C — Modifica ed eliminazione oggetti**
   - modifica di un oggetto già salvato;
   - conferme esplicite per eliminazioni;
   - conservazione dell'integrità delle relazioni.
3. **Step 5D — Documenti e tag**.
4. **Step 5E — Garanzie e promemoria**.
5. **Step 5F — Prestiti e storico**.
6. **Step 6 — Luoghi e multi-abitazione** — requisito trasversale da progettare
   prima dei moduli successivi che dipendono dalla posizione.

## Requisito: più case e stanze

Il gestionale dovrà supportare:

- più case/abitazioni separate;
- stanze appartenenti a una casa;
- assegnazione di un oggetto a una stanza riconosciuta dal sistema;
- spostamento dell'oggetto scegliendo la nuova stanza;
- elenco e filtri per singola casa e singola stanza;
- ricerca globale su tutte le case oppure limitata a una casa/stanza.

### Proposta da confermare

La soluzione preferita è un albero di **luoghi**:

```text
Casa A
├── Cucina
├── Camera
└── Garage
    └── Scaffale 2   (eventuale livello futuro)

Casa B
├── Soggiorno
└── Cantina
```

A livello dati, una singola tabella gerarchica con `parent_id` e tipo di luogo
permetterebbe di rappresentare case, stanze e in futuro sotto-posizioni senza
duplicare logica. Gli oggetti potrebbero riferirsi a un `luogo_id`, lasciando
un dettaglio libero solo quando serve.

**Questa architettura non è ancora approvata:** va presentata e confermata prima
dell'implementazione.
