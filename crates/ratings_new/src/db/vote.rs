use super::{ClientHash, DbError, Result};
use sqlx::{types::time::OffsetDateTime, FromRow, PgConnection};
use tracing::{debug, error};

/// A Vote, as submitted by a user
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Vote {
    /// The hash of the user client
    pub client_hash: ClientHash,
    /// The ID of the snap being voted on
    pub snap_id: String,
    /// The revision of the snap being voted on
    #[sqlx(try_from = "i32")]
    pub snap_revision: u32,
    /// Whether this is a positive or negative vote
    pub vote_up: bool,
    /// The timestamp of the vote
    #[sqlx(rename = "created")]
    pub timestamp: OffsetDateTime,
}

/// Gets votes for a snap with the given ID from a given [`ClientHash`]
///
/// [`ClientHash`]: crate::features::user::entities::ClientHash
impl Vote {
    pub async fn get_all_by_client_hash_and_snap_id(
        snap_id: String,
        client_hash: String,
        conn: &mut PgConnection,
    ) -> Result<Vec<Vote>> {
        debug!("client_hash: '{}', snap_id: '{}'", &client_hash, &snap_id);
        let votes = sqlx::query_as(
            r#"
                SELECT
                    votes.id,
                    votes.created,
                    votes.snap_id,
                    votes.snap_revision,
                    votes.vote_up,
                    users.client_hash
                FROM
                    users
                INNER JOIN
                    votes
                ON
                    users.id = votes.user_id_fk
                WHERE
                    users.client_hash = $1
                AND
                    votes.snap_id = $2
        "#,
        )
        .bind(client_hash)
        .bind(snap_id)
        .fetch_all(conn)
        .await
        .map_err(|error| {
            error!("{error:?}");
            DbError::FailedToGetUserVote
        })?;

        Ok(votes)
    }

    pub async fn get_all_by_user(
        client_hash: String,
        snap_id_filter: Option<String>,
        conn: &mut PgConnection,
    ) -> Result<Vec<Vote>> {
        let votes = sqlx::query_as(
            r#"
                SELECT
                    votes.id,
                    votes.created,
                    votes.snap_id,
                    votes.snap_revision,
                    votes.vote_up
                FROM
                    users
                INNER JOIN
                    votes
                ON
                    users.id = votes.user_id_fk
                WHERE
                    users.client_hash = $1
                AND
                    ($2 IS NULL OR votes.snap_id = $2);
            "#,
        )
        .bind(client_hash)
        .bind(snap_id_filter)
        .fetch_all(conn)
        .await
        .map_err(|error| {
            error!("{error:?}");
            DbError::FailedToGetUserVote
        })?;

        Ok(votes)
    }

    /// Saves a [`Vote`] to the database, if possible.
    pub async fn save_to_db(self, conn: &mut PgConnection) -> Result<u64> {
        let result = sqlx::query(
            r#"
        INSERT INTO votes (user_id_fk, snap_id, snap_revision, vote_up)
        VALUES ((SELECT id FROM users WHERE client_hash = $1), $2, $3, $4)
        ON CONFLICT (user_id_fk, snap_id, snap_revision)
        DO UPDATE SET vote_up = EXCLUDED.vote_up;
        "#,
        )
        .bind(self.client_hash)
        .bind(self.snap_id)
        .bind(self.snap_revision as i32)
        .bind(self.vote_up)
        .execute(conn)
        .await
        .map_err(|error| {
            error!("{error:?}");
            DbError::FailedToCastVote
        })?;

        Ok(result.rows_affected())
    }
}
