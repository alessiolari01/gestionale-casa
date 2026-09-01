# Spese condivise e saldi

**Stato: RIMANDATO.**

## Spesa

Una spesa deve poter conservare:

- importo;
- valuta;
- descrizione/categoria;
- data;
- chi ha pagato;
- partecipanti;
- contesto opzionale (viaggio, spazio, acquisto, ecc.);
- note;
- autore dell'inserimento/modifica.

## Partecipanti

Possono essere:

- utenti del gestionale;
- profili/ospiti senza account.

## Divisione

Modalità previste:

- quote uguali;
- importi personalizzati;
- percentuali;
- quote/pesi.

La somma deve essere validata prima del salvataggio definitivo.

## Saldo aggregato

Non è necessario saldare ogni spesa separatamente. Il gestionale può calcolare
un saldo netto fra le persone del gruppo/viaggio.

Esempio:

```text
Laura deve ad Alessio: 135,00 €
```

Un rimborso/saldo può essere registrato come operazione separata e attribuito
all'autore.

## Relazione con Acquisti

Un acquisto reale può alimentare contemporaneamente:

- la spesa effettivamente pagata;
- l'eventuale aggiornamento del prezzo base, solo se l'utente lo conferma.

Il prezzo osservato e la spesa pagata restano concetti distinti.

## Storico

Ogni modifica significativa deve mostrare chi ha cambiato cosa, inclusi importo,
quote, partecipanti e stato di saldo.
