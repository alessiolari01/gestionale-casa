# Turni e routine

**Stato: PREVISTO.**

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
- reminder opzionale;
- note.

## Reminder alla creazione

Quando si configura un turno il bot dovrebbe chiedere se impostare un reminder
per la preparazione dei pasti che lo richiedono. Se esistono default utente,
vengono proposti ma restano modificabili.

I canali Step 7 sono Telegram ed email. Niente SMS.

## Condivisione e copia

I modelli turno/routine sono candidati naturali per:

- condivisione nello stesso spazio;
- copia indipendente;
- invio di una copia ad un altro utente.

Le assegnazioni giornaliere restano riferite alla persona/profilo e non vengono
condivise automaticamente solo perché il modello è condiviso.

## Relazione con il planner

Il planner può usare la routine del giorno per precompilare:

- orari;
- luogo/situazione;
- preparazione;
- reminder.

La routine suggerisce: la pianificazione della singola data può sempre fare
override.
