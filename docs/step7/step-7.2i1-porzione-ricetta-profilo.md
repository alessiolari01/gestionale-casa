# Step 7.2I.1 — Porzione ricetta per Profilo alimentare

Introduce la prima UI Telegram delle porzioni per profilo.

Funzioni:
- accesso dal dettaglio del profilo gestito;
- elenco ricette visibili, massimo 5 per pagina;
- preset 80%, 100%, 120%, 150%;
- 100% equivale alla porzione standard e rimuove la personalizzazione;
- persistenza in `profilo_ricetta_porzioni`;
- controllo backend del gestore del profilo;
- rispetto della visibilità delle ricette;
- registrazione nello Storico.

Nessuna nuova migration: viene riutilizzato lo schema di 7.2I.0.
