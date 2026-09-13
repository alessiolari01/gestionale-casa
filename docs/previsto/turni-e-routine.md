# Turni e routine

**Stato: PRIMA FETTA SCRITTA il 10 settembre 2026, TREDICI CORREZIONI
SCRITTE l'11 settembre 2026** dopo il primo collaudo dal vivo — vedi
`docs/moduli/turni-e-routine.md` per come è fatta e `STATO.md`, sezioni
2quinquies e 2sexies, per cosa è stato collaudato finora (le correzioni
dell'11 settembre non ancora dal vivo su Telegram). **La copia
indipendente di un modello per un altro profilo è stata costruita**
(punto 13 delle correzioni, `📤 Copia per un altro profilo`): **il resto
di questo documento resta PREVISTO** — condivisione di un modello nello
spazio senza copiarlo, invio a un altro utente, reminder alla creazione
(righe sotto) non sono ancora costruiti.

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

## Condivisione e copia

**Copia indipendente: costruita l'11 settembre 2026** (`📤 Copia per un
altro profilo`, `turni::copia_modello_per_profilo`): crea un nuovo
modello con gli stessi pasti, intestato a un profilo diverso scelto tra
quelli visibili nello spazio, senza nessun collegamento con l'originale
dopo la copia.

**Restano non costruite**: condivisione dello stesso modello nello spazio
(senza copiarlo — un modello oggi appartiene a un solo profilo, punto 13
delle correzioni dell'11 settembre), invio di una copia a un altro utente
al di fuori dello spazio corrente.

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

## Sincronizzazione con diritto di veto del proprietario — idea proposta, non decisa di costruirla ora

Proposta di Alessio discussa con l'agente il 12 settembre 2026, durante il
secondo giro di collaudo dal vivo di Turni e routine: un meccanismo di
sincronizzazione **opzionale**, con un "diritto di veto" del proprietario,
per le entità "copiabili" del bot — oggi solo i modelli turno
(`📤 Copia per un altro profilo`, costruita l'11 settembre 2026 come copia
indipendente, deliberatamente scollegata dopo la copia), in futuro forse
altro (una ricetta? un profilo alimentare?).

**L'idea, in sintesi**: se un'entità copiabile viene condivisa tra due
account, il proprietario decide se sincronizzarla con la copia (invece di
lasciarla indipendente per sempre come oggi); se entrambe le parti sono
d'accordo, le due copie restano sincronizzate; i permessi di modifica
restano di default solo al proprietario, delegabili con consenso esplicito
dell'altra parte.

**Deciso insieme ad Alessio: non si implementa ora.** Tre motivi, discussi
e condivisi:

1. **Si sovrappone alla condivisione già esistente negli spazi.** Il
   progetto ha già un meccanismo di condivisione — gli spazi con i loro
   ruoli membro (proprietario/amministratore/membro, vedi
   `docs/database.md` e `docs/condivisione.md`) — che risolve un
   problema simile (più persone che vedono/modificano la stessa cosa) con
   un modello diverso (appartenenza allo spazio, non copia+sincronizzazione
   punto a punto fra due entità specifiche). Costruire un secondo
   meccanismo di condivisione in parallelo, prima di aver capito bene dove
   finisce l'uno e comincia l'altro, rischia di produrre due sistemi che si
   accavallano invece di uno solo coerente.
2. **È un'infrastruttura di permessi orizzontale**, non un dettaglio del
   modulo Turni: riguarderebbe potenzialmente ogni entità copiabile
   presente e futura del bot (non solo i modelli turno), con le sue
   proprie regole di consenso, revoca, conflitto (cosa succede se le due
   copie divergono nel frattempo e poi si sincronizzano?). Un meccanismo
   così trasversale merita una sessione di progettazione dedicata, con lo
   stesso livello di attenzione già dato a spazi/membership/ruoli in
   7.0-7.1 — non va deciso di corsa dentro una sessione di correzioni di
   collaudo su un modulo specifico.
3. **Un solo caso d'uso reale oggi**: la copia dei modelli turno appena
   costruita (`copia_modello_per_profilo`), e proprio quella è stata
   progettata **deliberatamente scollegata** dall'originale dopo la copia
   (vedi sopra, "Condivisione e copia") — un secondo meccanismo che la
   ricollega introdurrebbe complessità per un bisogno non ancora sentito
   due volte. Se in futuro emergesse un secondo caso d'uso reale (un'altra
   entità copiabile con lo stesso bisogno di restare sincronizzata), sarebbe
   il momento giusto per riprendere questa idea con più contesto, non prima.

Nessun codice scritto per questa idea: resta qui come traccia della
discussione, per non doverla rifare da capo se in futuro tornerà utile.
