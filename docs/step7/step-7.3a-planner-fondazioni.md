# Step 7.3A — Planner alimentare: fondazioni

## Scopo

Creare la base persistente del Planner senza introdurre ancora la UI Telegram.

## Decisioni

- il planner può essere personale oppure legato a uno Spazio;
- i partecipanti sono Profili alimentari reali;
- i pasti sono associati a una data e a un tipo di pasto;
- quando una ricetta viene assegnata vengono conservati snapshot della ricetta,
  dei profili e delle quantità calcolate;
- una modifica successiva della ricetta non modifica silenziosamente il planner;
- per i pasti pianificati il sistema potrà mostrare `🔄 Aggiorna`;
- un pasto completato resta congelato;
- l'esclusione ingrediente resta distinta da una quantità zero.

## Tabelle

- `planner_alimentari`
- `planner_pasti`
- `planner_pasto_profili`
- `planner_pasto_ingredienti_snapshot`

## Step successivo

7.3B renderà operative queste fondazioni su Telegram con vista settimanale,
aggiunta/modifica/rimozione dei pasti e scelta dei Profili partecipanti.
