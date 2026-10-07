# 📄 Documenti

**Stato: PREVISTO, decisioni prese il 7 ottobre 2026.** Si costruisce dopo
le scadenze dei Promemoria (`docs/moduli/promemoria.md`), che usa.

## Le scelte di Alessio (7 ottobre 2026)

- **Copia digitale**: foto e PDF mandati al bot, conservati sull'S9, **e**
  un link (Drive, email…). Tutte e due, se servono.
- **Scadenze**: come i Promemoria, con la priorità che decide quante volte e
  quando avvisare (le scadenze dei Promemoria: un documento ne ha una o più).
- **Chi lo vede**: scelto documento per documento: "solo mio" (la carta
  d'identità) o "dello spazio" (il contratto d'affitto, le bollette).
- **Cartelle**: si parte con cartelle già pronte — Identità, Casa, Auto,
  Salute, Lavoro, Tasse, Garanzie e scontrini, Banca e assicurazioni — "ma
  quella è la base": si creano, rinominano, annidano ed **eliminano** anche
  quelle già pronte.

## Il documento

- titolo (l'unico obbligatorio), cartella, note;
- dov'è, fisicamente: casa → stanza → contenitore (anche annidati, come
  "sottoscaffali"), gli stessi degli Oggetti, più un dettaglio libero;
- la copia digitale: foto e PDF (più di uno: fronte e retro), e un link;
- numero, ente che l'ha rilasciato, data di rilascio: facoltativi;
- una o più scadenze (`scadenze.documento_id`), ognuna con la sua priorità;
- privato o dello spazio.

## Scelte strutturali

- **Una tabella propria, non `items`**: `items.tipo` ha un vincolo che non
  ammette "documento", e cambiarlo vuol dire ricostruire la tabella di tutti
  gli oggetti. La posizione usa le stesse tabelle (`abitazioni`, `stanze`,
  `contenitori`), con le stesse regole.
- **Le cartelle sono dello spazio** e nascono alla prima apertura della
  sezione in quello spazio: eliminarne una già pronta non la fa ricomparire.
  Una cartella non si elimina se ha dentro documenti: prima si spostano.
- **I file** stanno in `data/media/documenti/<id>/`, come le foto degli
  oggetti in `data/media/oggetti/`. Il bot li rimanda a chi li chiede, con
  i permessi del documento.
- Un documento "solo mio" non compare a nessun altro membro dello spazio, in
  nessuna schermata né ricerca.

## Schermate

```
📄 Documenti
  📁 Identità (2) · 📁 Casa (5) · … (le cartelle, poi i documenti fuori cartella)
  [🔎 Cerca] [➕ Nuovo documento] [📅 In scadenza] [📁 Cartelle]

Documento: titolo, cartella, dov'è, scadenze, 🔒 solo mio | 👥 dello spazio
  [📎 Mostra la copia] [➕ Aggiungi foto/PDF] [🔗 Link] [📍 Dov'è]
  [📅 Scadenze] [✏️ Modifica] [🗑️ Elimina]
```
