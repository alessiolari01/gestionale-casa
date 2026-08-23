# Alimenti e unità

**Stato: PREVISTO.**

## Alimento

Entità culinaria riutilizzabile nelle ricette.

Esempi:

- pasta;
- pollo;
- uovo;
- latte.

Gli alimenti comuni possono essere globali all'installazione. Uno spazio può
avere alimenti personalizzati quando il catalogo globale non basta.

## Cosa non è un alimento

Un alimento non è:

- una confezione specifica acquistabile;
- una scorta fisica presente in frigorifero;
- un oggetto del modulo Oggetti.

Questa separazione permette di usare `pollo` nella ricetta anche se viene
acquistato in confezioni/marche differenti.

## Alias e ricerca

Il modello dovrà poter gestire alias/sinonimi senza creare duplicati inutili.
L'implementazione esatta verrà definita nella migration 7.2.

## Unità

Le quantità sono strutturate come valore + unità.

Famiglie convertibili candidate:

- massa: g, kg;
- volume: ml, l.

Unità discrete/non convertibili universalmente:

- pezzo;
- cucchiaino;
- cucchiaio;
- `q.b.`;
- altre unità personalizzate se necessarie.

Non si devono inventare conversioni tra volume e massa senza un'informazione
specifica dell'alimento.

## Futuro collegamento alle scorte

La futura dispensa dovrà mantenere separato il concetto di scorta:

```text
ALIMENTO: Pollo
SCORTA: 800 g disponibili
RICETTA: 300 g richiesti
PIANO: martedì Pollo
SPESA: quantità mancante
```

La dispensa è RIMANDATA e non blocca Step 7.
