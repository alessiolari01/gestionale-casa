# Roadmap funzionale


## Stato Step 6C — checkpoint corrente

- 6C.1 ✅ backend gerarchia contenitori — `cc3ba4c`.
- 6C.2 ✅ UI Telegram contenitori, verificata su S9 — `4c64798`.
- 6C.3A ✅ navigazione unificata + creazione contestuale — `413605e`.
- 6C.3B ✅ rifiniture UX, posizione completa e infrastruttura — `24944ac`.
- 6C.3C ✅ spostamento oggetti fino a contenitori/sottocontenitori, **62/62 test + runtime S9** — `658e455`.
- 6C.4 🔧 storico contenitori e cambi percorso: implementazione predisposta, nuova migration e 7 test aggiunti; verifica S9 ancora necessaria.
- 6C.5 ⏭️ verifica finale dello Step 6C, documentazione conclusiva, PR/CI/merge.

Principio approvato: navigare per **luogo corrente**, non per silos separati casa/stanza/contenitore. Lo storico deve usare snapshot immutabili dei percorsi, senza inventare cronologia retroattiva.

## Stato aggiornato dopo l'implementazione Step 6B

- Step 1 ✅ Scheletro progetto
- Step 2 ✅ Schema dati core
- Step 3 ✅ Backend Telegram + whitelist
- Step 3.1 ✅ Handoff, GitHub Actions, Dependabot e workflow Git
- Step 4 ✅ SQLite runtime + migration automatiche + `/status`
- Step 5A ✅ Oggetti generici
- Step 5B ✅ Foto degli oggetti
- Step 5C ✅ Modifica ed eliminazione oggetti
- Step 6A ✅ Case, stanze e posizione strutturata
- Step 6B ✅ Implementato e verificato su S9; PR/CI/merge ancora necessari
- Step 6C ⏭️ Contenitori e sotto-posizioni
- Step 7A ⏳ Documenti e garanzie
- Step 7B ⏳ Promemoria e scadenze
- Step 7C ⏳ Tag + ricerca globale
- Step 8 ⏳ Primo nuovo modulo applicativo: Veicoli o Vestiti

Il 6C parte solo dopo l'ingresso del 6B nella baseline ufficiale `main`.

---

Questo documento raccoglie la sequenza approvata degli sviluppi futuri e le
scelte che devono restare visibili anche quando il progetto passa a un'altra
persona o a un'altra AI.

## Stato corrente

- Step 1-4: **chiusi e verificati**.
- Step 5A — Oggetti generici: **chiuso e verificato**.
- Step 5B — Foto oggetti: **chiuso e verificato**.
- Step 5C — Modifica/eliminazione oggetti: **chiuso e verificato**, mergiato su
  `main` con CI verde.
- Step 6A — Case, stanze e posizione strutturata: **step corrente in sviluppo**.

## Sequenza approvata

### Step 6A — Case, stanze e posizione strutturata

Obiettivi:

- più abitazioni nello stesso account;
- stanze appartenenti a una casa;
- oggetti assegnabili direttamente a una casa oppure a una stanza;
- spostamento guidato dell'oggetto;
- elenchi filtrati per casa e stanza;
- ricerca oggetti anche per nome della casa/stanza;
- mantenimento del vecchio campo `oggetti.posizione` come dettaglio libero;
- base condivisa tramite `items`, riutilizzabile dai moduli futuri.

Architettura approvata: `abitazioni` + `stanze` + `item_luogo`. Dettagli in
`docs/moduli/luoghi.md`.

### Step 6B — Storico globale + storico individuale

Lo storico deve essere una funzione trasversale del gestionale, non un log
specifico del modulo Oggetti.

Due viste:

1. **storico individuale** dalla scheda di un'entità;
2. **storico globale dell'account** con filtri.

Ogni evento dovrà conservare almeno:

- data e ora;
- tipo di operazione;
- modulo/sezione;
- entità interessata e relativo ID quando ancora esiste;
- casa e stanza rilevanti, se presenti;
- valori precedenti e nuovi per le modifiche;
- una descrizione leggibile per Telegram.

Filtri desiderati:

- periodo;
- modulo (`oggetti`, `vestiti`, `veicoli`, `case`, `stanze`, ecc.);
- casa;
- stanza;
- tipo di operazione;
- elemento specifico.

Esempi di eventi: creazione, modifica, spostamento, foto aggiunta/rimossa,
archiviazione, eliminazione, tag e promemoria. Gli eventi importanti devono
restare consultabili anche dopo la cancellazione dell'entità originale.

Per i luoghi, lo storico dovrà distinguere almeno tre eventi diversi:

- prima assegnazione (`nessun luogo -> casa/stanza`);
- spostamento (`casa/stanza A -> casa/stanza B`), conservando esplicitamente
  origine e destinazione;
- rimozione del luogo (`casa/stanza -> nessun luogo`).

La UI dello Step 6A usa già questa distinzione, così il significato delle azioni
rimane coerente quando verrà introdotto lo storico.

### Step 6C — Contenitori e sotto-posizioni

Estensione futura della posizione fisica:

```text
Casa principale
→ Garage
→ Scaffale 2
→ Cassetta attrezzi
→ Chiave dinamometrica
```

Lo Step 6A non forza già questa gerarchia. Nel 6C si valuterà se usare un terzo
livello dedicato oppure una struttura gerarchica generica sotto la stanza.

### Step 7A — Documenti e garanzie

Documenti collegabili alle entità, per esempio:

- scontrini e fatture;
- manuali;
- garanzie;
- libretti e certificati;
- polizze e altri PDF/immagini.

La funzione deve essere condivisa tra moduli quando possibile, evitando tabelle
separate per Oggetti, Vestiti e Veicoli.

### Step 7B — Promemoria e scadenze

Sviluppare concretamente l'infrastruttura `promemoria` già prevista nel core:

- scadenze collegate a un'entità;
- notifiche Telegram anticipate;
- ricorrenze;
- completamento;
- esempi: garanzia, revisione, assicurazione, manutenzione.

### Step 7C — Tag + ricerca globale

- tag condivisi tra moduli;
- filtri combinabili per casa, stanza, modulo, stato e tag;
- ricerca globale che non richieda di conoscere prima il modulo dell'elemento.

Esempio futuro:

```text
/cerca casco
→ oggetto
→ vestito/accessorio
→ documento
→ manutenzione collegata
```

### Step 8 — Primo nuovo modulo applicativo

Priorità da scegliere al momento tra:

- **Veicoli**;
- **Vestiti**.

Entrambi dovranno riusare, dove sensato, luoghi, foto, documenti, tag,
promemoria e storico invece di ricreare sistemi paralleli.

## Funzioni future approvate da tenere in progettazione

### Manutenzioni e interventi

Registro di eventi reali, distinto dallo storico tecnico delle modifiche al
gestionale. Particolarmente utile per veicoli, elettrodomestici e attrezzatura.

Dati possibili: data, chilometraggio/ore, descrizione, costo, officina/persona,
documenti e foto.

### Costi e valore

Per ogni entità potranno essere aggregati:

- prezzo di acquisto;
- manutenzioni;
- accessori;
- assicurazioni/spese ricorrenti;
- valore stimato attuale.

In futuro questi dati alimenteranno statistiche e dashboard.

### Prestiti

Stato temporaneo per oggetti prestati a una persona, con:

- persona;
- data prestito;
- restituzione prevista;
- restituzione effettiva;
- promemoria opzionale;
- eventi nello storico.

### QR code e codici a barre

Possibile generazione di QR per aprire direttamente la scheda di:

- oggetto;
- stanza;
- contenitore.

Particolarmente utile dopo lo Step 6C.

### Archivio invece della sola eliminazione

Oltre alla cancellazione definitiva introdotta nello Step 5C, il progetto dovrà
valutare uno stato di archivio che conservi dati e storico per elementi non più
attivi:

- venduto;
- regalato;
- buttato;
- perso;
- dismesso.

L'archivio sarà preferibile alla cancellazione quando si vuole conservare la
storia dell'elemento.

### Registro acquisti

Flusso rapido che parte da un acquisto e crea/collega:

- elemento;
- prezzo;
- venditore;
- data;
- scontrino/fattura;
- garanzia.

In futuro si potrà valutare l'estrazione assistita dei dati da foto/documenti,
ma non è un requisito dei primi step.

### Dashboard e statistiche

Vista sintetica futura, per esempio:

- numero di oggetti/veicoli/vestiti;
- elementi per casa/stanza;
- prossime scadenze;
- elementi da riparare;
- valore stimato e spese aggregate.

## Principio architetturale trasversale

Il progetto deve evitare di creare una versione diversa della stessa funzione
per ogni modulo.

Il pattern attuale `items` va evoluto mantenendo questa idea:

```text
                    ITEM / ENTITÀ
                         |
          +--------------+--------------+
          |              |              |
       Oggetto         Veicolo        Vestito
          |              |              |
          +---- Foto / Documenti / Tag / Promemoria / Storico ----+
          +-------------------- Luogo condiviso --------------------+
```

Non significa che ogni futura entità debba per forza essere una riga `items`:
case, stanze e altri concetti di sistema possono avere tabelle proprie. Significa
che le funzionalità trasversali vanno progettate una volta e riusate quando il
dominio lo consente.

## Regola per le future decisioni

Prima di introdurre una migration strutturale importante:

1. descrivere il requisito;
2. confrontare le alternative;
3. scegliere e documentare il modello;
4. solo dopo creare migration e codice;
5. mantenere compatibilità con i dati reali già presenti quando possibile.
