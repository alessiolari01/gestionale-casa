<!-- PROFILI_OPERATIVI_7_2H -->
# Stato corrente — Profili operativi, Porzioni prossimo blocco

Lo Step 7.2H ha implementato le fondazioni e la gestione Telegram dei **Profili alimentari**. Un Profilo rappresenta una persona alimentare, può esistere senza account Telegram, può essere collegato opzionalmente a un utente e può essere condiviso tramite Spazi. Non è una risorsa globale.

La parte **Porzioni/override** di questo documento resta invece il prossimo sviluppo: deve appoggiarsi ai Profili esistenti senza ricrearli.

# Profili e porzioni

**Stato: IN IMPLEMENTAZIONE — fondazione database 7.2H.0 aggiunta; UI Telegram ancora da implementare.**

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

<!-- STEP7_2H0_PROFILI_FONDAZIONI -->
## Fondazione database 7.2H.0

Il profilo alimentare è ora definito nello schema come entità autonoma tramite `profili_alimentari`.

Campi concettuali principali:

- **gestore**: account interno che amministra il profilo;
- **utente collegato** opzionale: account che rappresenta la stessa persona, se esiste;
- **nome** e nome normalizzato;
- **note** facoltative;
- stato di **archiviazione**.

Un account può essere collegato a un solo profilo alimentare attivo. Un gestore può invece amministrare più profili, ad esempio se organizza i pasti anche per una persona senza Telegram.

La tabella `profilo_alimentare_spazi` rende un profilo visibile in uno o più spazi di collaborazione. Nessuna condivisione implica profilo privato. I profili alimentari non supportano la visibilità globale.

Nel primo blocco operativo la condivisione è riservata al gestore del profilo e richiede diritto di scrittura nello spazio di destinazione. Eventuali collaboratori espliciti verranno aggiunti in un blocco successivo riusando il modello `permessi_risorsa` con controlli fail-closed dedicati.

La migration non crea profili retroattivi: il popolamento avverrà dal wizard Telegram del blocco successivo.
