# Multipiattaforma — bot Telegram, poi web app, poi le altre

**Stato: PREVISTO, solo una decisione di ordine — nessun codice.**

## Il desiderio di Alessio

In futuro il progetto (che diventerà "Villica", vedi
`docs/previsto/rinominazione-villica.md`) dovrebbe esistere anche come web
app, e poi come app per Play Store, App Store, Windows e Mac — non solo come
bot Telegram.

## Perché ha senso (discusso il 7-8 settembre 2026)

Un gestionale domestico che copre alimentazione/planner/porzioni/spazi
condivisi con questo livello di dettaglio non è comune sul mercato: la
maggior parte delle app simili o è generica (liste/promemoria) o si ferma
alla spesa, senza il livello di modellazione già costruito qui (profili,
porzioni per persona, planner con snapshot, spazi condivisi con ruoli). Un
fronte web/desktop/mobile aggiungerebbe soprattutto comodità d'uso (schermo
più grande, tastiera, notifiche native), non nuova logica di dominio.

## Decisione (8 settembre 2026): il bot viene finito per primo

**Non si costruisce la web app in parallelo al bot.** Si completa prima il
bot Telegram così come già pensato (i moduli restanti di `docs/roadmap.md`:
lista della spesa, poi gli altri domini previsti), e solo dopo si costruisce
la web app appoggiandosi alla stessa struttura — stesso schema dati, stessa
logica di dominio (regole di permessi, calcolo porzioni, propagazione degli
snapshot del planner, ecc.), esposta eventualmente via API invece di essere
duplicata.

**Perché non in parallelo**: ogni decisione di dominio non ancora presa
(permessi, spazi condivisi, badge/novità, liste, ecc.) andrebbe pensata e
collaudata due volte in contemporanea su due interfacce diverse, con il
rischio concreto che le due implementazioni divergano nel tempo — lo stesso
tipo di rischio già visto in questo progetto con "due implementazioni
parallele" (vedi `docs/storico-del-progetto.md`, Step 7.3B). Finendo prima
il bot, la logica di dominio arriva alla web app già matura e collaudata da
un utente vero, e il lavoro sul fronte web si concentra su interfaccia e
hosting, non anche su bug di dominio ancora aperti.

**Cosa NON è ancora deciso**: come sarà strutturata la web app (framework,
hosting, se e come esporre un'API dal codice Rust esistente o riscriverla),
l'ordine tra web/iOS/Android/Windows/Mac dopo il bot, e i tempi. Da
riprendere in un documento dedicato quando il bot sarà nella forma "finita"
di cui parla `docs/previsto/rinominazione-villica.md`.
