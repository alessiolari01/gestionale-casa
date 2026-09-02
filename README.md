# Gestionale Casa

Gestionale personale e condiviso per organizzare beni, luoghi e vita
quotidiana, senza installare un'app: l'interfaccia è un **bot Telegram**.

**Rust + SQLite**, in esecuzione su un Galaxy S9 che fa da server. Il vincolo
hardware non è un dettaglio: è la ragione di buona parte delle scelte
tecniche, dalla dimensione delle pagine al numero di query per schermata.

## Cosa fa oggi

Alimentazione (alimenti, ricette, profili alimentari con porzioni personali,
planner dei pasti), oggetti, case/stanze/contenitori, storico di tutto ciò che
cambia, spazi condivisi con inviti e ruoli, e un backlog di miglioramenti
gestito dal bot stesso.

Quello che **non** fa ancora è elencato in `docs/previsto/`: quei documenti
sono specifiche, non descrizioni.

## Avvio

Servono tre variabili d'ambiente. Non serve un file `.env`: se le variabili
sono già nell'ambiente hanno la precedenza, ed è il modo consigliato perché
tiene i segreti fuori dalla cartella del progetto.

```bash
export TELOXIDE_TOKEN=...          # da @BotFather
export ALLOWED_CHAT_IDS=123456789  # chat autorizzate, separate da virgola
export DATABASE_URL=sqlite://percorso/del/database.db
cargo run
```

Sul Galaxy S9 non si lancia `cargo run` a mano: si usa lo script, che aggiorna,
verifica, fa il backup e prova le migration su una copia prima di toccare il
database reale.

```bash
./scripts/aggiorna-s9.sh --ramo <nome-ramo>
```

## Dove si legge il resto

Un fatto sta in un posto solo. Se lo cerchi, è qui:

| Domanda | Documento |
|---|---|
| **Dove siamo adesso?** | `STATO.md` |
| Cosa è cambiato, e quando | `CHANGELOG.md` |
| Perché è fatto così | `docs/architettura.md` |
| Perché non è fatto in un altro modo | `docs/storico-del-progetto.md` |
| Come deve comportarsi ogni schermata | `docs/convenzioni-telegram.md` |
| Schema dati e regole delle migration | `docs/database.md` |
| Spazi, ruoli, permessi | `docs/condivisione.md` |
| Server, script, CI | `docs/infrastruttura.md` |
| Un singolo modulo che esiste | `docs/moduli/` |
| Qualcosa che non esiste ancora | `docs/previsto/` |
| Cosa viene dopo | `docs/roadmap.md` |

**Chi modifica il codice aggiorna anche i documenti**, nello stesso commit. La
regola per esteso, con la lista di cosa toccare, è in testa a `STATO.md`.
