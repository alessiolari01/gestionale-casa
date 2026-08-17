//! Step 6C.1 - backend contenitori e sotto-posizioni.
//! Nessuna UI Telegram in questo step.

use anyhow::{bail, ensure, Result};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ContainerRecord {
    pub id: i64,
    pub home_id: i64,
    pub room_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerBreadcrumb {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPath {
    pub home_id: i64,
    pub home_name: String,
    pub room_id: Option<i64>,
    pub room_name: Option<String>,
    pub containers: Vec<ContainerBreadcrumb>,
}

#[derive(Debug, FromRow)]
struct BreadcrumbRow {
    id: i64,
    name: String,
}

#[derive(Debug, FromRow)]
struct ScopeNames {
    home_name: String,
    room_name: Option<String>,
}

pub async fn create_container(
    pool: &SqlitePool,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
    name: &str,
    description: Option<&str>,
) -> Result<i64> {
    let name = clean_required_name(name)?;
    let description = clean_optional_text(description);
    let mut tx = pool.begin().await?;

    validate_scope(&mut tx, home_id, room_id).await?;
    validate_parent_target(&mut tx, None, home_id, room_id, parent_id).await?;

    let result = sqlx::query(
        "INSERT INTO contenitori \
         (abitazione_id, stanza_id, contenitore_padre_id, nome, descrizione) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(home_id)
    .bind(room_id)
    .bind(parent_id)
    .bind(name)
    .bind(description)
    .execute(&mut *tx)
    .await?;

    let id = result.last_insert_rowid();
    tx.commit().await?;
    Ok(id)
}

pub async fn get_container(pool: &SqlitePool, id: i64) -> Result<Option<ContainerRecord>> {
    Ok(sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list_container_children(
    pool: &SqlitePool,
    parent_id: i64,
) -> Result<Vec<ContainerRecord>> {
    Ok(sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori WHERE contenitore_padre_id = ? \
         ORDER BY nome COLLATE NOCASE, id",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await?)
}

pub async fn rename_container(pool: &SqlitePool, id: i64, name: &str) -> Result<bool> {
    let name = clean_required_name(name)?;
    let result = sqlx::query(
        "UPDATE contenitori SET nome = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(name)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn set_container_description(
    pool: &SqlitePool,
    id: i64,
    description: Option<&str>,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE contenitori SET descrizione = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(clean_optional_text(description))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn container_path(pool: &SqlitePool, id: i64) -> Result<Option<ContainerPath>> {
    let Some(container) = get_container(pool, id).await? else {
        return Ok(None);
    };

    let scope = sqlx::query_as::<_, ScopeNames>(
        "SELECT a.nome AS home_name, s.nome AS room_name \
         FROM abitazioni a LEFT JOIN stanze s ON s.id = ? WHERE a.id = ?",
    )
    .bind(container.room_id)
    .bind(container.home_id)
    .fetch_one(pool)
    .await?;

    let containers = sqlx::query_as::<_, BreadcrumbRow>(
        "WITH RECURSIVE chain(id, nome, contenitore_padre_id, depth) AS ( \
            SELECT id, nome, contenitore_padre_id, 0 FROM contenitori WHERE id = ? \
            UNION ALL \
            SELECT c.id, c.nome, c.contenitore_padre_id, chain.depth + 1 \
            FROM contenitori c JOIN chain ON chain.contenitore_padre_id = c.id \
         ) SELECT id, nome AS name FROM chain ORDER BY depth DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| ContainerBreadcrumb {
        id: row.id,
        name: row.name,
    })
    .collect();

    Ok(Some(ContainerPath {
        home_id: container.home_id,
        home_name: scope.home_name,
        room_id: container.room_id,
        room_name: scope.room_name,
        containers,
    }))
}

pub async fn assign_item_to_container(
    pool: &SqlitePool,
    item_id: i64,
    container_id: i64,
) -> Result<()> {
    let container = get_container(pool, container_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("contenitore #{container_id} inesistente"))?;

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE id = ?)")
        .bind(item_id)
        .fetch_one(pool)
        .await?;
    ensure!(exists, "item #{item_id} inesistente");

    sqlx::query(
        "INSERT INTO item_luogo (item_id, abitazione_id, stanza_id, contenitore_id) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(item_id) DO UPDATE SET \
             abitazione_id = excluded.abitazione_id, \
             stanza_id = excluded.stanza_id, \
             contenitore_id = excluded.contenitore_id",
    )
    .bind(item_id)
    .bind(container.home_id)
    .bind(container.room_id)
    .bind(container.id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn move_container(
    pool: &SqlitePool,
    id: i64,
    new_home_id: i64,
    new_room_id: Option<i64>,
    new_parent_id: Option<i64>,
) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let Some(current) = get_container_conn(&mut tx, id).await? else {
        return Ok(false);
    };

    validate_scope(&mut tx, new_home_id, new_room_id).await?;
    validate_parent_target(&mut tx, Some(id), new_home_id, new_room_id, new_parent_id).await?;

    if current.home_id == new_home_id
        && current.room_id == new_room_id
        && current.parent_id == new_parent_id
    {
        return Ok(false);
    }

    sqlx::query("UPDATE contenitori SET contenitore_padre_id = NULL WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE contenitori SET abitazione_id = ?, stanza_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id IN ( \
            WITH RECURSIVE subtree(id) AS ( \
                SELECT ? UNION ALL \
                SELECT c.id FROM contenitori c \
                JOIN subtree s ON c.contenitore_padre_id = s.id \
            ) SELECT id FROM subtree \
         )",
    )
    .bind(new_home_id)
    .bind(new_room_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE item_luogo SET abitazione_id = ?, stanza_id = ? \
         WHERE contenitore_id IN ( \
            WITH RECURSIVE subtree(id) AS ( \
                SELECT ? UNION ALL \
                SELECT c.id FROM contenitori c \
                JOIN subtree s ON c.contenitore_padre_id = s.id \
            ) SELECT id FROM subtree \
         )",
    )
    .bind(new_home_id)
    .bind(new_room_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE contenitori SET contenitore_padre_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(new_parent_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(true)
}

pub async fn delete_container(pool: &SqlitePool, id: i64) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let Some(container) = get_container_conn(&mut tx, id).await? else {
        return Ok(false);
    };

    sqlx::query(
        "UPDATE contenitori SET contenitore_padre_id = ?, \
         aggiornato_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE contenitore_padre_id = ?",
    )
    .bind(container.parent_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE item_luogo SET contenitore_id = ? WHERE contenitore_id = ?")
        .bind(container.parent_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    let result = sqlx::query("DELETE FROM contenitori WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

async fn get_container_conn(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Option<ContainerRecord>> {
    Ok(sqlx::query_as::<_, ContainerRecord>(
        "SELECT id, abitazione_id AS home_id, stanza_id AS room_id, \
                contenitore_padre_id AS parent_id, nome AS name, descrizione AS description \
         FROM contenitori WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

async fn validate_scope(
    conn: &mut SqliteConnection,
    home_id: i64,
    room_id: Option<i64>,
) -> Result<()> {
    let home_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM abitazioni WHERE id = ?)")
            .bind(home_id)
            .fetch_one(&mut *conn)
            .await?;
    ensure!(home_exists, "abitazione #{home_id} inesistente");

    if let Some(room_id) = room_id {
        let coherent: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM stanze WHERE id = ? AND abitazione_id = ?)",
        )
        .bind(room_id)
        .bind(home_id)
        .fetch_one(&mut *conn)
        .await?;
        ensure!(coherent, "stanza non appartenente all'abitazione");
    }
    Ok(())
}

async fn validate_parent_target(
    conn: &mut SqliteConnection,
    moving_id: Option<i64>,
    home_id: i64,
    room_id: Option<i64>,
    parent_id: Option<i64>,
) -> Result<()> {
    let Some(parent_id) = parent_id else {
        return Ok(());
    };

    let parent = get_container_conn(conn, parent_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("contenitore padre #{parent_id} inesistente"))?;

    ensure!(
        parent.home_id == home_id && parent.room_id == room_id,
        "il contenitore padre appartiene a un luogo differente"
    );

    if let Some(moving_id) = moving_id {
        ensure!(
            moving_id != parent_id,
            "un contenitore non può contenere se stesso"
        );

        let inside: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
                WITH RECURSIVE subtree(id) AS ( \
                    SELECT id FROM contenitori WHERE id = ? \
                    UNION ALL \
                    SELECT c.id FROM contenitori c \
                    JOIN subtree s ON c.contenitore_padre_id = s.id \
                ) SELECT 1 FROM subtree WHERE id = ? \
             )",
        )
        .bind(moving_id)
        .bind(parent_id)
        .fetch_one(conn)
        .await?;

        if inside {
            bail!("il nuovo padre è un discendente: ciclo rifiutato");
        }
    }
    Ok(())
}

fn clean_required_name(value: &str) -> Result<String> {
    let value = value.trim();
    ensure!(!value.is_empty(), "nome contenitore vuoto");
    Ok(value.to_string())
}

fn clean_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate");
        pool
    }

    async fn home(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO abitazioni (nome) VALUES (?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("home")
            .last_insert_rowid()
    }
    async fn room(pool: &SqlitePool, home: i64, name: &str) -> i64 {
        sqlx::query("INSERT INTO stanze (abitazione_id, nome) VALUES (?, ?)")
            .bind(home)
            .bind(name)
            .execute(pool)
            .await
            .expect("room")
            .last_insert_rowid()
    }
    async fn item(pool: &SqlitePool, name: &str) -> i64 {
        sqlx::query("INSERT INTO items (tipo, nome) VALUES ('oggetto', ?)")
            .bind(name)
            .execute(pool)
            .await
            .expect("item")
            .last_insert_rowid()
    }

    #[tokio::test]
    async fn annidamento_e_percorso() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let a = create_container(&pool, h, Some(r), None, "Armadio", None)
            .await
            .unwrap();
        let b = create_container(&pool, h, Some(r), Some(a), "Ripiano 2", None)
            .await
            .unwrap();
        let c = create_container(&pool, h, Some(r), Some(b), "Scatola", None)
            .await
            .unwrap();
        let p = container_path(&pool, c).await.unwrap().unwrap();
        assert_eq!(p.room_name.as_deref(), Some("Garage"));
        assert_eq!(
            p.containers
                .iter()
                .map(|x| x.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Armadio", "Ripiano 2", "Scatola"]
        );
    }

    #[tokio::test]
    async fn padre_deve_essere_nello_stesso_luogo() {
        let pool = test_pool().await;
        let h1 = home(&pool, "A").await;
        let h2 = home(&pool, "B").await;
        let r1 = room(&pool, h1, "R1").await;
        let r2 = room(&pool, h2, "R2").await;
        let p = create_container(&pool, h1, Some(r1), None, "P", None)
            .await
            .unwrap();
        assert!(create_container(&pool, h2, Some(r2), Some(p), "X", None)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn ciclo_rifiutato() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let a = create_container(&pool, h, Some(r), None, "A", None)
            .await
            .unwrap();
        let b = create_container(&pool, h, Some(r), Some(a), "B", None)
            .await
            .unwrap();
        let c = create_container(&pool, h, Some(r), Some(b), "C", None)
            .await
            .unwrap();
        assert!(move_container(&pool, a, h, Some(r), Some(c)).await.is_err());
    }

    #[tokio::test]
    async fn spostamento_muove_sottoalbero_e_item() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let g = room(&pool, h, "Garage").await;
        let c = room(&pool, h, "Camera").await;
        let a = create_container(&pool, h, Some(g), None, "Armadio", None)
            .await
            .unwrap();
        let s = create_container(&pool, h, Some(g), Some(a), "Scatola", None)
            .await
            .unwrap();
        let i = item(&pool, "Trapano").await;
        assign_item_to_container(&pool, i, s).await.unwrap();
        assert!(move_container(&pool, a, h, Some(c), None).await.unwrap());
        assert_eq!(
            get_container(&pool, s).await.unwrap().unwrap().room_id,
            Some(c)
        );
        let loc: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(loc, (h, Some(c), Some(s)));
    }

    #[tokio::test]
    async fn delete_promuove_al_padre() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let a = create_container(&pool, h, Some(r), None, "Armadio", None)
            .await
            .unwrap();
        let s = create_container(&pool, h, Some(r), Some(a), "Scatola", None)
            .await
            .unwrap();
        let child = create_container(&pool, h, Some(r), Some(s), "Punte", None)
            .await
            .unwrap();
        let i = item(&pool, "Trapano").await;
        assign_item_to_container(&pool, i, s).await.unwrap();
        assert!(delete_container(&pool, s).await.unwrap());
        assert_eq!(
            get_container(&pool, child)
                .await
                .unwrap()
                .unwrap()
                .parent_id,
            Some(a)
        );
        let target: Option<i64> =
            sqlx::query_scalar("SELECT contenitore_id FROM item_luogo WHERE item_id = ?")
                .bind(i)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(target, Some(a));
    }

    #[tokio::test]
    async fn delete_root_mantiene_casa_e_stanza() {
        let pool = test_pool().await;
        let h = home(&pool, "Casa").await;
        let r = room(&pool, h, "Garage").await;
        let root = create_container(&pool, h, Some(r), None, "Scaffale", None)
            .await
            .unwrap();
        let child = create_container(&pool, h, Some(r), Some(root), "Cassetto", None)
            .await
            .unwrap();
        let i = item(&pool, "Chiavi").await;
        assign_item_to_container(&pool, i, root).await.unwrap();
        assert!(delete_container(&pool, root).await.unwrap());
        let child = get_container(&pool, child).await.unwrap().unwrap();
        assert_eq!(
            (child.parent_id, child.home_id, child.room_id),
            (None, h, Some(r))
        );
        let loc: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT abitazione_id, stanza_id, contenitore_id FROM item_luogo WHERE item_id = ?",
        )
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(loc, (h, Some(r), None));
    }
}
