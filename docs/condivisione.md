<!-- MODELLO_CONDIVISIONE_7_2H_CHIUSO -->
# Aggiornamento 7.2H — modello operativo

Lo **Spazio** è il contesto di collaborazione. Non coincide con un'abitazione fisica. Un account autorizzato al bot non diventa automaticamente membro di uno Spazio: la membership è esplicita e può essere acquisita tramite invito privato.

I Profili alimentari sono risorse personali/condivisibili tramite Spazi e non possono essere globali. La condivisione mantiene una sola risorsa viva; una futura duplicazione/personalizzazione deve produrre una copia indipendente con provenienza.

# Modello di condivisione

**Operativo.** Utenti, membership, spazio attivo, isolamento dei dati, membri
e inviti via deep-link sono realizzati (`identity.rs`, `spazi_membri.rs`,
`access_control.rs`, `resource_permissions.rs`).

Restano **non costruiti**, e le sezioni che li descrivono sono specifiche:
condivisione diretta risorsa → account fuori dallo spazio, `📄 Crea una mia
versione`, proposte sui contenuti globali, `🛡️ Modalità utente`. La tabella
`item_condivisioni` esiste nello schema ma **non e' riferita da nessuna riga di
codice**: prima di usarla va verificato che il modello sia ancora quello giusto.

<!-- testo storico, conservato per il modello -->
Utenti, membership, spazio attivo e isolamento dei
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

## 6. Proprietà, visibilità e posizione cross-space

Da Step 7.1B non vale più la regola assoluta "item e luogo devono appartenere allo stesso spazio". Si distinguono tre concetti:

- **proprietà**: `items.spazio_id` indica lo spazio proprietario dell'entità;
- **visibilità**: dipende dalle membership e, in futuro, dalle condivisioni esplicite;
- **posizione fisica**: per gli oggetti può appartenere a un altro spazio accessibile.

Esempio valido:

```text
Portatile
Proprietà: Personale Alessio
Posizione: Casa condivisa / Camera
```

Lo spostamento non trasferisce la proprietà. Per effettuare uno spostamento verso un altro spazio, l'attore deve avere diritto di scrittura sia sullo spazio proprietario dell'oggetto sia sullo spazio che contiene la destinazione. Un ID conosciuto ma appartenente a uno spazio senza membership resta inutilizzabile.

Case, stanze e contenitori restano invece entità strutturali dello spazio che le possiede. In particolare un contenitore non viene trasferito cross-space come effetto di un semplice spostamento: si spostano gli oggetti, oppure si modellerà esplicitamente un futuro trasferimento di proprietà.

### Vista multi-spazio

Lo `spazio_attivo_id` storico diventa semanticamente lo **spazio predefinito**. `preferenze_utente.vista_spazi` può valere:

- `predefinito`: query e liste mostrano solo lo spazio predefinito;
- `tutti`: query e liste mostrano tutti e soli gli spazi di cui l'utente è membro.

La modalità di vista è un filtro di consultazione, non una concessione di permessi.

#### Disambiguazione dei luoghi omonimi

Nomi identici sono validi in spazi differenti. Di conseguenza il nome dello spazio fa parte del **contesto visivo**, non del nome persistito dell'abitazione. In vista multi-spazio la UI rende i luoghi come `Casa principale · Spazio principale` e `Casa principale · Casa condivisa`; stanze e contenitori ereditano lo stesso contesto nel percorso. I messaggi che confermano un'assegnazione o uno spostamento mostrano sempre lo spazio della posizione. Non si rinominano automaticamente le case e non si duplica il dato solo per renderlo distinguibile.

### Condivisione della stessa entità

`item_condivisioni` associa un item a uno spazio ulteriore con permesso `lettura` o `modifica`. Questa tabella prepara la condivisione di oggetti, ricette e altri item senza duplicazione. La UI e le regole complete di modifica condivisa saranno abilitate in un blocco successivo.

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

<!-- STEP7_2H0_PROFILI_FONDAZIONI -->
## 8. Decisione 7.2H.0 — spazio, luogo, proprietà e visibilità

Da questo blocco lo **spazio** va interpretato come un contesto di collaborazione e autorizzazione, non come sinonimo di abitazione fisica.

I concetti restano distinti:

- **account esterno**: identità Telegram o futura integrazione;
- **utente interno**: identità applicativa stabile;
- **spazio**: gruppo/contesto nel quale più utenti possono collaborare;
- **abitazione, stanza, contenitore**: luoghi fisici;
- **proprietà**: chi possiede/gestisce una risorsa;
- **visibilità**: chi può scoprirla e usarla;
- **permessi**: quali modifiche sono consentite.

Per le risorse che lo supportano vengono riconosciuti tre ambiti concettuali:

1. **privata** — visibile al proprietario/gestore;
2. **condivisa** — stessa risorsa sincronizzata, visibile a uno o più account/spazi autorizzati;
3. **globale** — catalogo comune del gestionale, pubblicabile/modificabile direttamente solo dall'amministratore.

La visibilità globale **non** è prevista per abitazioni, stanze, contenitori, oggetti, profili alimentari, planner o liste della spesa. È invece adatta a cataloghi come alimenti, prodotti commerciali e ricette di riferimento.

Condividere non crea una copia: chi usa la risorsa condivisa vede la stessa entità aggiornata. Una futura azione `📄 Crea una mia versione` produrrà invece una copia indipendente mantenendo soltanto la provenienza informativa.

### Proposte sui contenuti globali

Un utente normale non modifica direttamente un contenuto globale. È prevista una coda di **proposte di modifica** amministrative, con confronto prima/dopo e approvazione/rifiuto. Questa funzione è futura e non è implementata nel 7.2H.0.

### Modalità utente per l'amministratore

È prevista una futura `🛡️ Modalità utente`: l'amministratore potrà usare il bot con interfaccia e autorizzazioni ordinarie, evitando azioni globali accidentali. L'uscita dalla modalità dovrà essere esplicita e chiaramente indicata. La modalità non modifica il ruolo persistito dell'account: è una vista operativa protetta.

<!-- STEP7_2H3_MEMBRI_SPAZI -->
## 9. Membri degli spazi — 7.2H.3

L'accesso al gestionale e l'accesso a uno spazio sono autorizzazioni separate.
L'approvazione di un account Telegram crea/abilita l'utente e il suo spazio
personale, ma **non** lo aggiunge automaticamente agli spazi di altre persone.

Per gli spazi `famiglia` e `condiviso`, proprietario e amministratori possono
usare `👥 Membri dello spazio` per aggiungere un account già autorizzato con uno
dei ruoli:

- `👤 Membro` — uso ordinario dello spazio, con i permessi previsti dalle
  singole risorse;
- `👁️ Sola lettura` — consultazione senza scrittura nello spazio;
- `🛡️ Amministratore` — gestione dello spazio e dei membri.

Il ruolo `proprietario` non viene assegnato dal picker. Il proprietario non può
essere rimosso e la schermata non consente di rimuovere se stessi. Gli spazi
`personale` non accettano membri aggiuntivi.

Rimuovere una membership non elimina l'account, il suo spazio personale o i
suoi dati posseduti. La perdita della membership rimuove invece l'accesso alle
risorse che erano visibili soltanto attraverso quello spazio; il fallback dello
spazio attivo resta gestito dai trigger già presenti.

## Step 7.2H.4A — Inviti privati agli spazi

La gestione membri usa inviti privati tramite deep-link Telegram e non espone agli utenti normali l'elenco degli account autorizzati. Proprietario/amministratore sceglie ruolo e modalità del link: monouso, riutilizzabile, numero massimo di utilizzi o scadenza.

Gli inviti attivi consentono `📋 Copia link d'invito`, modifica del ruolo futuro, aggiunta/modifica/rimozione della scadenza e revoca. Data e orario sono modificabili dall'interfaccia Telegram; il calendario è costruito con inline button.

Gli inviti monouso consumati, quelli che raggiungono il limite, quelli scaduti e quelli revocati vengono eliminati dalla tabella operativa. I token/link non vengono registrati nello Storico.

Il destinatario deve accettare esplicitamente l'invito. Se non è ancora autorizzato al gestionale, deve prima completare il normale flusso di accesso e poi riaprire lo stesso link. L'approvazione dell'account non crea automaticamente membership in spazi condivisi.


## Rifiniture UX H.4B

Gli inviti restano separati dall'accesso al gestionale. Se il creatore dell'invito o un utente già membro apre il deep-link, il backend non incrementa `utilizzi` e non elimina un invito monouso; la UI informa l'utente e propone il ritorno a `👥 I miei spazi` o al `🏠 Menù principale`.

Le notifiche operative relative agli spazi sono messaggi temporanei: possono essere lette e usate per navigare, ma vengono ripulite alla successiva interazione per non accumulare messaggi persistenti.

Le schermate del flusso inviti mantengono la navigazione standard sulla stessa riga quando esiste un livello precedente:
`⬅️ Indietro | 🏠 Menù principale | 💡 Migliora`.
