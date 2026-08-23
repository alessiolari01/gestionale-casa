# Profili e porzioni

**Stato: PREVISTO.**

## Profilo alimentare

Rappresenta una persona per quantità, preferenze organizzative e partecipazione
ai pasti.

Il collegamento a un utente del gestionale è opzionale.

Esempi:

- utente registrato;
- partner;
- bambino;
- ospite.

## Porzione personalizzata

La ricetta conserva una base comune. Il profilo può definire:

1. un fattore/porzione personale generale per quella ricetta;
2. override per ingredienti specifici.

Esempio:

```text
Pasta base: 100 g/persona
Alessio: 120 g
Persona B: 80 g
```

Non serve duplicare la ricetta.

## Override ingrediente

Un profilo può richiedere, quando necessario:

- quantità diversa;
- ingrediente escluso;
- alternativa futura.

Le sostituzioni automatiche sono RIMANDATE: Step 7 deve solo evitare di
chiudere il modello in modo che diventino impossibili.

## Preferenze/esclusioni

Possono esistere come dati organizzativi per aiutare il planner. Non devono
essere presentate come garanzia medica o controllo clinico degli allergeni.

## Partecipazione ai pasti

Ogni pasto pianificato può avere più profili. Il calcolo della spesa deve usare
le quantità risultanti dai partecipanti effettivi, non un numero generico di
porzioni.
