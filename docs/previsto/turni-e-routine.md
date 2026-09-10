# Turni e routine

**Stato: PRIMA FETTA SCRITTA il 10 settembre 2026** (modello, pasti-modello,
assegnazione a una data per un profilo con copia dei pasti, riferimento di
sola lettura nella schermata "Giorno" del planner) — vedi
`docs/moduli/turni-e-routine.md` per come è fatta e `STATO.md`, sezione
2quinquies, per cosa è stato collaudato finora (non ancora dal vivo su
Telegram). **Il resto di questo documento resta PREVISTO**: condivisione e
copia di un modello, invio a un altro utente, reminder alla creazione (righe
sotto) non sono ancora costruiti.

## Obiettivo

Un turno/routine descrive una giornata tipo che influenza la pianificazione dei
pasti.

Esempi:

- Apertura;
- Chiusura;
- Riposo;
- Università;
- Trasferta;
- altra routine personalizzata.

Ogni modello ha un **nome personalizzato** scelto dall'utente.

## Modello vs assegnazione

Il modello contiene i default. L'assegnazione a una data crea una giornata che
può essere modificata senza cambiare il modello.

Esempio:

```text
Modello Chiusura
- lavoro 14:00-22:30
- pranzo 12:00 a casa
- cena 19:00 al lavoro, da preparare prima
```

Il 27 agosto l'utente può dichiarare `cena fuori` solo per quella giornata.

## Pasti della routine

Per ogni pasto sono previsti:

- tipo pasto;
- orario suggerito;
- situazione: casa / lavoro / fuori / saltato / altro;
- preparazione anticipata sì/no;
- anticipo o data/ora di preparazione;
- reminder opzionale (**non costruito in questa fetta**: l'infrastruttura
  reminder non esiste ancora nel progetto, arriverà in un blocco
  successivo);
- note.

## Reminder alla creazione — non costruito in questa fetta

Quando si configura un turno il bot dovrebbe chiedere se impostare un reminder
per la preparazione dei pasti che lo richiedono. Se esistono default utente,
vengono proposti ma restano modificabili.

I canali Step 7 sono Telegram ed email. Niente SMS.

## Condivisione e copia — non costruita in questa fetta

I modelli turno/routine sono candidati naturali per:

- condivisione nello stesso spazio;
- copia indipendente;
- invio di una copia ad un altro utente.

Le assegnazioni giornaliere restano riferite alla persona/profilo e non vengono
condivise automaticamente solo perché il modello è condiviso.

## Relazione con il planner

**Parzialmente costruita**: il planner mostra oggi solo un riferimento
testuale di sola lettura (vedi `docs/moduli/turni-e-routine.md`), mai una
precompilazione automatica — quella resta prevista, non ancora costruita.

Il planner può usare la routine del giorno per precompilare:

- orari;
- luogo/situazione;
- preparazione;
- reminder.

La routine suggerisce: la pianificazione della singola data può sempre fare
override.
