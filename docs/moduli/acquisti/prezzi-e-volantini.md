# Prezzi base e confronto volantini

**Stato: RIMANDATO.**

## Prezzo persistente

Il gestionale conserva principalmente il **prezzo normale/base** di un prodotto
presso un negozio.

Il prezzo è modificabile e conserva almeno:

- prezzo confezione;
- quantità/unità confezione;
- prezzo normalizzato calcolabile;
- data ultimo aggiornamento;
- autore dell'aggiornamento.

## Visualizzazione

Mostrare entrambi:

```text
Confezione: 500 g
Prezzo confezione: 1,29 €
Prezzo normalizzato: 2,58 €/kg
```

Per altri prodotti l'unità può essere €/l, €/pezzo, €/rotolo o altra metrica
sensata e dichiarata.

## Volantini

Le offerte temporanee non sostituiscono il prezzo base e non sono parte dello
storico principale dei prezzi.

Il volantino viene usato come input di confronto:

```text
Prezzo base negozio: 1,29 €
Prezzo visto sul volantino: 0,89 €
Risparmio: 0,40 € / 31%
```

Il confronto può anche mostrare che un'offerta di un negozio resta più cara del
prezzo base di un altro.

L'eventuale lettura assistita da PDF/foto è futura e deve richiedere conferma dei
dati estratti prima di usarli.

## Storico

Ha senso conservare lo storico delle modifiche ai **prezzi base**. Non va
riempito con ogni sconto settimanale visto su un volantino.
