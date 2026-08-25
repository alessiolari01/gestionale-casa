//! Step 7.2D.0.3 - fondazioni Ricette con prodotto commerciale opzionale.
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

#[expect(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct RecipeFoodCompatibility {
    pub label_code: String,
    pub label_name: String,
    pub label_emoji: String,
    pub label_type: String,
    pub status: String,
    pub total_ingredients: i64,
    pub incompatible_ingredients: i64,
    pub ingredients_to_check: i64,
}

/// Restituisce le compatibilità alimentari derivate dagli ingredienti.
///
/// La vista database considera `verificare` anche una compatibilità mancante:
/// in questo modo un nuovo alimento non classificato non può far apparire una
/// ricetta come certamente compatibile.
#[expect(dead_code)]
pub async fn recipe_food_compatibility(
    pool: &SqlitePool,
    recipe_id: i64,
) -> Result<Vec<RecipeFoodCompatibility>> {
    sqlx::query_as::<_, RecipeFoodCompatibility>(
        "SELECT \
            etichetta_codice AS label_code, \
            etichetta_nome AS label_name, \
            etichetta_emoji AS label_emoji, \
            etichetta_tipo AS label_type, \
            stato AS status, \
            ingredienti_totali AS total_ingredients, \
            ingredienti_non_compatibili AS incompatible_ingredients, \
            ingredienti_da_verificare AS ingredients_to_check \
         FROM v_ricetta_compatibilita_alimentare \
         WHERE ricetta_id = ? \
         ORDER BY ordinamento, etichetta_nome COLLATE NOCASE",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await
    .context("Impossibile calcolare la compatibilità alimentare della ricetta")
}

/// Prodotto commerciale opzionalmente selezionabile per un ingrediente.
///
/// La ricetta mantiene sempre anche `alimento_id`: il prodotto specifico è
/// un livello aggiuntivo utile per prezzi, disponibilità e valori nutrizionali.
#[expect(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct RecipeProductChoice {
    pub product_id: i64,
    pub brand: String,
    pub product_name: String,
    pub package_quantity: f64,
    pub package_unit_symbol: String,
}

#[expect(dead_code)]
pub async fn product_choices_for_food(
    pool: &SqlitePool,
    food_id: i64,
) -> Result<Vec<RecipeProductChoice>> {
    sqlx::query_as::<_, RecipeProductChoice>(
        "SELECT \
            p.id AS product_id, \
            p.marca AS brand, \
            p.nome_commerciale AS product_name, \
            p.quantita_confezione AS package_quantity, \
            um.simbolo AS package_unit_symbol \
         FROM prodotti_alimentari p \
         JOIN unita_misura um ON um.id = p.unita_confezione_id \
         WHERE p.alimento_id = ? \
           AND p.attivo = 1 \
         ORDER BY p.marca COLLATE NOCASE, p.nome_commerciale COLLATE NOCASE, p.id",
    )
    .bind(food_id)
    .fetch_all(pool)
    .await
    .context("Impossibile leggere i prodotti commerciali dell'ingrediente")
}
