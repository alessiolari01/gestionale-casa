# Storico e audit multiutente

**Stato: PREVISTO — estensione dello storico Step 6B/6C nel 7.1.**

Lo storico attuale registra già eventi, cambiamenti prima/dopo, cambi di luogo,
percorsi container-aware ed eventi padre/figlio. Lo Step 7 deve aggiungere il
contesto multiutente senza riscrivere gli eventi esistenti.

## Obiettivo

Ogni evento condiviso deve rispondere almeno a:

- **cosa** è cambiato;
- **chi** ha richiesto/eseguito l'azione;
- **quando**;
- **dove/nel quale spazio**;
- **come** è nata l'azione (utente, sistema, integrazione);
- quali effetti sono stati automatici.

## Autore umano

Per un'azione avviata da un utente vanno conservati:

- riferimento all'utente interno;
- snapshot del nome visualizzato utile allo storico;
- eventuale origine, per esempio Telegram.

Esempio UI:

```text
✏️ Oggetto modificato
👤 Alessio
🕒 23/08/2026 18:42

Nome:
Vecchio -> Nuovo
```

## Eventi automatici

Gli effetti derivati non devono sembrare azioni manuali separate.

Esempio:

```text
#101 Spostato Armadio
👤 Richiesto da Alessio

#102 Scatola spostata
⚙️ Effetto automatico di #101

#103 Trapano spostato
⚙️ Effetto automatico di #101
```

L'attuale `evento_padre_id` è il meccanismo naturale da preservare.

## Sistema e integrazioni

Sono previste origini come:

- utente/Telegram;
- sistema;
- reminder/automazione;
- integrazione Google/email.

Un evento automatico può conservare anche l'utente che ha originato il flusso,
quando applicabile.

## Snapshot

Come per i percorsi dei contenitori, gli elementi utili a interpretare il
passato non devono dipendere esclusivamente dai dati correnti.

Candidati a snapshot:

- nome autore;
- nome entità;
- contesto del luogo;
- eventuale nome dello spazio;
- valori prima/dopo già previsti dallo storico.

## Filtri

Lo storico globale dovrà poter aggiungere almeno:

- spazio;
- autore;
- origine automatica/manuale;

mantenendo i filtri esistenti per modulo, operazione, periodo, luogo ed entità.

## Moduli futuri

La stessa regola vale per Alimentazione, Acquisti, Viaggi e Spese.

Esempi:

```text
💰 Prezzo base aggiornato
👤 Laura
1,29 € -> 1,39 €
```

```text
✅ GoPro verificata al rientro
👤 Alessio
Viaggio: Corfù
```

```text
💸 Spesa modificata
👤 Marco
48,00 € -> 52,00 €
```

## Migration

La migration 7.1 non deve inventare autori per gli eventi storici già presenti.
Per i record pre-Step 7 va usata una strategia esplicita e documentata, per
esempio autore sconosciuto/legacy oppure collegamento al bootstrap solo se la
semantica è certa. La scelta verrà fissata prima di scrivere la migration.
