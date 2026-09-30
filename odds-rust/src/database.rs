//! Historical-rating persistence for the legacy HTTP endpoint.
//! The complete series is computed before its atomic database update.
use crate::ratings::HistoricalRating;
use mysql::{prelude::Queryable, Conn, Opts, OptsBuilder, Params, TxOpts, Value};
use std::time::Duration;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub const DEFAULT_DATABASE_URL: &str = "mysql://root@127.0.0.1:3306/GolAberto_development";
fn connection_options(url: &str) -> Result<OptsBuilder, Error> {
    // Allow the same DATABASE_URL as Rails' mysql2 adapter.
    let normalized = url
        .strip_prefix("mysql2://")
        .map(|rest| format!("mysql://{rest}"));
    let opts =
        Opts::from_url(normalized.as_deref().unwrap_or(url)).map_err(|_| "invalid DATABASE_URL")?;
    let opts = OptsBuilder::from_opts(opts)
        .read_timeout(Some(Duration::from_secs(30)))
        .write_timeout(Some(Duration::from_secs(30)))
        .tcp_connect_timeout(Some(Duration::from_secs(5)));
    Ok(opts)
}
pub fn connect() -> Result<Conn, Error> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.into());
    Ok(Conn::new(connection_options(&url)?)?)
}
fn rounded(v: f64) -> f64 {
    // Preserve the six fractional digits of Go's %f SQL strings.
    format!("{v:.6}").parse().unwrap()
}
pub fn persist_history(conn: &mut Conn, rows: &[HistoricalRating]) -> Result<(), Error> {
    let mut tx = conn.start_transaction(TxOpts::default())?;
    for chunk in rows.chunks(1000) {
        let query = format!("INSERT INTO historical_ratings (team_id,off_rating,def_rating,rating,measure_date) VALUES {} ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating)",
            vec!["(?,?,?,?,?)"; chunk.len()].join(","));
        let values: Vec<Value> = chunk
            .iter()
            .flat_map(|r| {
                [
                    r.team_id.into(),
                    rounded(r.off_rating).into(),
                    rounded(r.def_rating).into(),
                    rounded(r.rating).into(),
                    r.measure_date.clone().into(),
                ]
            })
            .collect();
        tx.exec_drop(query, Params::Positional(values))?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rails_and_native_urls_share_connection_configuration() {
        for url in [
            "mysql://root@127.0.0.1/example",
            "mysql2://root@127.0.0.1/example",
        ] {
            let opts: Opts = connection_options(url).unwrap().into();
            assert_eq!(opts.get_db_name(), Some("example"));
            assert_eq!(
                opts.get_read_timeout().copied(),
                Some(Duration::from_secs(30))
            );
            assert_eq!(
                opts.get_write_timeout().copied(),
                Some(Duration::from_secs(30))
            );
        }
    }

    #[test]
    fn invalid_url_error_does_not_repeat_connection_configuration() {
        assert_eq!(
            connection_options("invalid://private-value")
                .unwrap_err()
                .to_string(),
            "invalid DATABASE_URL"
        );
    }
}
