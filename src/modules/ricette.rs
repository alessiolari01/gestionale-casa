//! Step 7.2C - fondazioni Ricette.
//!
//! Una ricetta è un record centrale posseduto da un utente e può essere resa
//! visibile in zero, uno o più spazi senza crearne copie. Gli ingredienti
//! referenziano gli alimenti esistenti tramite `alimento_id`.
//!
//! La ricerca per ingredienti usa semantica OR: una ricetta entra nei
//! risultati se contiene almeno uno degli alimenti richiesti. L'ordinamento
//! privilegia il numero di ingredienti richiesti effettivamente presenti.

use anyhow::{Context, Result};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

#[expect(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct RecipeIngredientMatch {
    pub recipe_id: i64,
    pub recipe_name: String,
    pub matched_ingredients: i64,
    pub total_ingredients: i64,
}

/// Cerca ricette visibili contenenti almeno uno degli alimenti richiesti.
///
/// Ranking:
/// 1. più ingredienti richiesti presenti;
/// 2. a parità, nome ricetta;
/// 3. a ulteriore parità, ID interno stabile (non mostrato in UI).
#[expect(dead_code)]
pub async fn search_by_ingredients(
    pool: &SqlitePool,
    ingredient_ids: &[i64],
    user_id: i64,
    current_space_id: i64,
    view_all_spaces: bool,
    limit: i64,
) -> Result<Vec<RecipeIngredientMatch>> {
    let mut ids = ingredient_ids
        .iter()
        .copied()
        .filter(|id| *id > 0)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();

    if ids.is_empty() || limit <= 0 {
        return Ok(Vec::new());
    }

    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT \
            r.id AS recipe_id, \
            r.nome AS recipe_name, \
            COUNT(DISTINCT ri.alimento_id) AS matched_ingredients, \
            (SELECT COUNT(*) FROM ricetta_ingredienti all_ri \
             WHERE all_ri.ricetta_id = r.id) AS total_ingredients \
         FROM ricette r \
         JOIN ricetta_ingredienti ri ON ri.ricetta_id = r.id \
         WHERE r.archiviata = 0 \
           AND ri.alimento_id IN (",
    );

    {
        let mut separated = query.separated(", ");
        for ingredient_id in &ids {
            separated.push_bind(*ingredient_id);
        }
    }
    query.push(") AND (");
    query.push("r.catalogo_globale = 1 OR r.proprietario_utente_id = ");
    query.push_bind(user_id);

    if view_all_spaces {
        query.push(
            " OR EXISTS (\
                SELECT 1 \
                FROM ricetta_spazi rs \
                JOIN membri_spazio ms ON ms.spazio_id = rs.spazio_id \
                WHERE rs.ricetta_id = r.id \
                  AND ms.utente_id = ",
        );
        query.push_bind(user_id);
        query.push(")");
    } else {
        query.push(
            " OR EXISTS (\
                SELECT 1 FROM ricetta_spazi rs \
                WHERE rs.ricetta_id = r.id \
                  AND rs.spazio_id = ",
        );
        query.push_bind(current_space_id);
        query.push(")");
    }

    query.push(
        ") \
         GROUP BY r.id, r.nome \
         HAVING COUNT(DISTINCT ri.alimento_id) > 0 \
         ORDER BY matched_ingredients DESC, r.nome COLLATE NOCASE, r.id \
         LIMIT ",
    );
    query.push_bind(limit);

    query
        .build_query_as::<RecipeIngredientMatch>()
        .fetch_all(pool)
        .await
        .context("Impossibile cercare le ricette per ingredienti")
}
