# Modello di condivisione

**Stato: IN SVILUPPO.** Utenti, membership, spazio attivo e isolamento dei
moduli Step 6 sono implementati nel blocco corrente; inviti/gestione membri e
condivisione/copia completa restano da implementare.

## 1. Entità principali

### Utente

Identità interna stabile del gestionale.

Non deve coincidere con un Telegram chat ID o un account Google.

### Account esterno

Collega un utente a un provider, inizialmente Telegram e in seguito Google.
Le credenziali/token sensibili non devono finire nel repository.

### Spazio

Confine di proprietà/condivisione dei dati.

Esempi:

- spazio personale;
- famiglia;
- casa condivisa.


## 1.1 Spazio attivo operativo

Ogni utente ha una preferenza `spazio_attivo_id`. I moduli operativi usano
quello spazio come confine di lettura/scrittura.

Regole:

- un ID di un altro spazio non concede accesso;
- cambiare spazio invalida le sessioni temporanee;
- case e tag possono riusare lo stesso nome in spazi differenti;
- un ruolo `lettura` non può eseguire le scritture principali;
- la creazione di uno spazio non trasferisce dati dallo spazio precedente.

### Membro dello spazio

Associazione utente↔spazio con ruolo.

Ruoli iniziali proposti:

- **Proprietario** — controllo completo e gestione membri;
- **Amministratore** — gestione ordinaria dello spazio e dei contenuti;
- **Membro** — uso e modifica ordinaria secondo le regole del modulo;
- **Sola lettura** — consultazione senza modifiche.

Il dettaglio esatto delle autorizzazioni va testato durante 7.1 senza creare una
matrice di permessi eccessivamente complessa.

## 2. Proprietà dei dati

Le entità condivise dovranno appartenere a uno spazio quando ciò rappresenta il
loro reale ambito.

Esempi naturali:

- casa, stanza, contenitore;
- oggetto;
- ricetta condivisa;
- pianificazione famiglia;
- lista della spesa condivisa;
- viaggio condiviso;
- gruppo di spese.

Un utente può mantenere dati privati usando il proprio spazio personale.

## 3. Condivisione vs copia

### Condivisione

Più utenti vedono la stessa entità. Le modifiche autorizzate sono visibili a
tutti i membri che hanno accesso.

### Copia

Viene creata una nuova entità con un nuovo ID e vita indipendente.

Quando utile si può conservare una provenienza informativa:

```text
copia derivata da <entità originale>
```

La provenienza non deve trasformarsi automaticamente in sincronizzazione
bidirezionale.

### Invio di una copia

Per alcune entità dovrà essere possibile inviare una copia a un altro account
senza aggiungerlo allo spazio famiglia.

Casi candidati:

- ricetta;
- modello turno/routine;
- modello checklist viaggio;
- piano/template riutilizzabile.

Il destinatario accetta e salva una nuova copia indipendente.

## 4. Inviti a uno spazio

Flusso previsto:

1. proprietario/amministratore genera un invito;
2. il backend crea un token casuale;
3. nel DB si conserva un hash, non il token in chiaro;
4. l'invito ha scadenza, revoca e limite d'uso;
5. può essere consegnato come codice o deep-link Telegram;
6. il destinatario accetta;
7. nasce la membership con il ruolo previsto.

## 5. Profili senza account

Un profilo alimentare o un partecipante alle spese può esistere senza login.
Questo evita di obbligare bambini, ospiti o amici a registrarsi.

Quando in futuro un ospite diventa utente, l'eventuale collegamento fra profilo
e account dovrà essere esplicito e non basato solo sul nome.

## 6. Regola cross-space

Una relazione che rappresenta possesso/posizione non deve collegare entità di
spazi incompatibili.

Esempio da impedire:

```text
oggetto dello Spazio B -> casa dello Spazio A
```

I vincoli verranno implementati con foreign key, query transazionali e, dove
necessario, trigger/validazioni applicative. La scelta concreta va verificata
nella migration 7.1.

## 7. Cancellazione e uscita dallo spazio

Non va usato `ON DELETE CASCADE` indiscriminatamente sugli elementi condivisi.
Prima dell'implementazione di ogni delete vanno definiti:

- chi può cancellare;
- se l'entità va archiviata o eliminata;
- cosa succede ai riferimenti storici;
- cosa succede alle copie/provenienze;
- cosa succede se un membro lascia lo spazio.

Lo storico deve rimanere interpretabile anche dopo uscita/rimozione di un
utente.
