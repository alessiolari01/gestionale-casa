# Rinominazione del progetto in "Villica"

**Stato: DECISO, non ancora applicato.** Da fare quando il progetto avrà
raggiunto la forma "finita" immaginata (vedi `docs/roadmap.md` e
`docs/previsto/`), non ora — deciso esplicitamente da Alessio l'8 settembre
2026: nessun file va rinominato, nessun riferimento va cambiato finché non
arriva quel momento.

## Il nome scelto

**Villica.** Nell'antica Roma la villica era la donna che amministrava
concretamente una casa/tenuta — dispensa, servitù, gestione quotidiana. Il
nome descrive **una funzione attiva** (chi amministra), non un contenitore
passivo (una casa, un luogo) — scelto apposta per restare coerente anche
mentre il progetto diventa più agentivo (l'automazione del ciclo di
sviluppo, un'AI che scrive/testa/distribuisce da sola).

Preferito a **"Casarium"** (italiano "casa" + latino "-arium", più
immediato da leggere ma descrittivo di un luogo, non di un'azione), l'altro
finalista della stessa ricerca.

## Perché non un nome ovvio in inglese

Prima di arrivare a "Villica" sono stati verificati via ricerca web una
quarantina di nomi, in stili diversi (parole inglesi calde tipo
Daily/Home/Hearth/Nest, radici classiche singole tipo Nexus/Vita/Oikia,
incroci di due radici). Quasi tutti erano già presi — spesso da app che
fanno *esattamente* quello che sta diventando questo progetto:

- **"Domus: Your Family Organizer"** e **"Domio — AI Family Organizer"**:
  calendario condiviso, faccende, meal plan, coordinamento familiare via AI.
- **"Majordomo"**: un servizio "AI-powered household management" a cui si
  scrive via testo (niente app) per tracciare appuntamenti, manutenzioni,
  spese — concettualmente vicino a dove sta andando questo bot.
- **"Homio"**, **"Hearthly"**, **"Promus"**, **"Nestwise"**, **"Familium"**:
  tutte app reali di organizzazione familiare con nomi quasi identici a
  quelli provati.
- **"Kasa"** (TP-Link, smart home) e **"Domus"** (storica rivista italiana
  di design/architettura, dal 1928): collisioni con marchi affermati, non
  solo con altre app.

Conclusione pratica: lo spazio dei nomi ovvi per "app di organizzazione
casa/famiglia" è saturo nel 2026. Un nome coniato da un incrocio di radici
classiche (non una parola da dizionario) si è rivelata l'unica strada
rimasta libera con un vero significato dietro.

## Alternative rimaste pulite, non scelte

Se "Villica" dovesse rivelarsi problematico in futuro (es. un controllo di
marchio reale in fase di pubblicazione, non solo una ricerca web), questi
restano verificati senza collisioni dirette al momento della ricerca:
**Casarium**, **Focarium** (da "focus"/focolare), **Larora** (dai Lari,
divinità domestiche romane), **Penaria** (dalle provviste di casa, radice
dei Penati), **Cellararius** (il monaco addetto alla dispensa in un
monastero medievale).

## Cosa comporta applicarlo, quando sarà il momento

Punti distinti, non tutti con lo stesso peso — da valutare separatamente
quando si deciderà di procedere:

- il nome del bot su Telegram (`Gestionale_personalizzato_Bot`): cambiarlo
  passa da BotFather, e se cambia anche lo username tutti i link/riferimenti
  salvati smettono di funzionare;
- i documenti del repository (`STATO.md`, `CHANGELOG.md`, tutto `docs/`):
  dicono ancora "Gestionale Casa" ovunque, servirebbe una sostituzione
  sistematica;
- il repository GitHub (`alessiolari01/gestionale-casa`): rinominabile, ma
  cambia gli URL esistenti.
