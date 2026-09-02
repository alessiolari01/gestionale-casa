# Reminder Alimentazione

**Stato: PREVISTO.**

Il reminder va progettato come infrastruttura trasversale, non come funzione
esclusiva delle ricette.

## Canali Step 7

- Telegram;
- email.

**SMS escluso dalla specifica corrente.**

## Casi Alimentazione

- preparare un pasto in anticipo;
- ricordare un pasto pianificato;
- ricordare una lista/spesa quando esplicitamente configurato;
- future azioni come scongelamento, se verranno approvate.

## Default

L'utente può avere preferenze di default, ma una routine/pasto può fare
override.

Durante la configurazione di un turno con meal-prep il bot dovrebbe chiedere se
creare il reminder, evitando automazioni inattese.

## Audit

Creazione/modifica/cancellazione di un reminder condiviso deve avere autore.
L'invio automatico è invece un evento di sistema collegabile alla configurazione
che lo ha generato.
