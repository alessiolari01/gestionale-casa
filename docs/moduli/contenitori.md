# Contenitori e sotto-posizioni

## Modello

`contenitori` rappresenta sotto-posizioni annidabili.

```text
Casa principale
└── Garage
    └── Armadio attrezzi
        └── Ripiano 2
            └── Scatola rossa
                └── Trapano
```

Campi principali: `id`, `abitazione_id`, `stanza_id` opzionale, `contenitore_padre_id` opzionale, `nome`, `descrizione`, timestamp.

`item_luogo.contenitore_id` collega l'item al contenitore corrente. Il percorso completo è derivato dalle relazioni, non duplicato come sorgente dati testuale.

## Regole

- annidamento arbitrario;
- padre e figlio devono appartenere allo stesso ambito casa/stanza;
- vietati i cicli;
- spostare un contenitore sposta sottoalbero e item contenuti;
- eliminare un contenitore non elimina gli oggetti.

Se si elimina un contenitore con padre, figli e oggetti diretti vengono promossi al padre. Se si elimina un contenitore radice, restano direttamente nella stessa casa/stanza.

Se si elimina una stanza, i contenitori radice vengono promossi alla casa mantenendo la gerarchia interna; gli oggetti non vengono cancellati.

## Telegram e stato

6C.2 (`4c64798`) espone `/contenitori`, creazione, annidamento, elenco, breadcrumb, rinomina, spostamento ed eliminazione sicura.

6C.3A integra i contenitori nella navigazione generale e aggiunge `➕ Nuovo oggetto qui`.

Per le regole comuni vedere `navigazione-luoghi.md`.

Prossimi passi: completare il movimento degli oggetti tra contenitori (6C.3), poi integrare lo storico specifico dei contenitori/percorso (6C.4).

Nessuna utility persistente di reset/cancellazione globale del database.
