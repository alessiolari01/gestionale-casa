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
## Oggetti diretti nel contenitore (6C.3B)

La scheda di ogni contenitore espone `📋 Oggetti in questo contenitore (N)`. La vista elenca solo gli oggetti il cui `item_luogo.contenitore_id` coincide direttamente con quel contenitore; gli oggetti dei sottocontenitori restano nel proprio livello.

La vista mostra anche il percorso del contenitore e `/luogo_c<ID>`, con ritorno diretto al contenitore e al menu principale.

## Convenzione icone e ritorno post-creazione (6C.3B)

`📦` resta riservato ai contenitori. Gli oggetti direttamente contenuti sono mostrati con `🏷️`, così un elenco misto distingue subito nodi contenitore e item.

Quando `Nuovo oggetto qui` parte da un contenitore, dopo il salvataggio la scheda dell'oggetto offre `↩️ Torna a <nome contenitore>` tramite callback inline; la stessa regola vale per creazioni partite da casa o stanza.

## Convenzione UI gerarchica

Nelle tastiere inline i sottocontenitori già presenti vengono mostrati prima delle azioni. Le creazioni usano etichette compatte (`➕📦 Contenitore`, `➕🏷️ Oggetto`) e l'elenco degli oggetti diretti usa `📋🏷️ Oggetti qui`. Questa convenzione è condivisa con casa e stanza.

### Ritorno dagli elenchi contestuali

Gli elenchi aperti con `Contenitori qui` mantengono il contesto del luogo:
- da una casa, `↩️ Torna alla casa` riapre quella casa;
- da una stanza, `↩️ Torna alla stanza` riapre quella stanza.

Il pulsante non deve risalire automaticamente al livello superiore della gerarchia: deve tornare al luogo dal quale l'elenco contestuale è stato aperto.
