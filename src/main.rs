mod inputfile;
mod sql_string;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use inputfile::InputFile;
use itertools::intersperse;
use lazy_static::lazy_static;
use postgres::Client;
use regex::{Regex, RegexSet};
use sql_string::SqlString;
use sqlx::postgres::PgPoolOptions;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/// Command line arguments
struct Args {
    /// Configuration file
    #[clap(short, long, display_order = 1)]
    #[clap(default_value_t = String::from("./pg_parcel.toml"))]
    file: String,

    /// Dump only columns where `column_name` is one of these values.
    ///
    /// Multiple values can be specified by using this option more than once. At
    /// least one value must be given.
    #[clap(name = "id", short, long, required = true, display_order = 2)]
    ids: Vec<String>,

    /// Override database URL in parcel config.
    #[clap(long, display_order = 3)]
    database_url: Option<String>,

    /// Insert a `TRUNCATE` command before any `COPY` commands.
    ///
    /// This will truncate every table found in the schema *except* those that
    /// are explicitly skipped, *without* cascading. If a table included in the
    /// dump is referenced by a foreign key from a skipped table, this injected
    /// `TRUNCATE` command will likely fail. Should this happen, instead of
    /// skipping a table, include it with an override query containing a `WHERE
    /// false` condition.
    #[clap(long, display_order = 4)]
    truncate: bool,

    /// Prints a report estimating row count and size of the data to be dumped
    /// for each table, and in total. Does not dump table data.
    ///
    /// Note that the figures reported may be well off the mark, especially the
    /// estimated size of the dump, but they should be off the mark by a roughly
    /// constant factor.
    #[clap(long, display_order = 10)]
    estimate_only: bool,

    /// Populate session variable `pg_parcel.features` with these strings. If
    /// set, it takes precedence over the default_features in pg_parcel.toml
    #[clap(long, value_delimiter = ',', display_order = 5)]
    features: Option<Vec<String>>,

    /// Omit this feature from `pg_parcel.features`, overriding both --features
    /// and pg_parcel.toml.
    #[clap(long = "no-feature", value_delimiter = ',', display_order = 6)]
    skipped_features: Option<Vec<String>>,
}

/// Options here is a combination of command line arguments and contents of the slicefile.
struct Options {
    column_name: String,
    column_values: Vec<String>,
    schema: String,
    database_url: String,
    accept_invalid_certs: bool,
    skip_tables: RegexSet,
    overrides: HashMap<String, String>,
    estimate_only: bool,
    truncate: bool,
    features: HashSet<String>,
}

impl Options {
    pub fn load() -> Result<Options, Box<dyn Error>> {
        let args = Args::parse();
        let file = InputFile::load(Path::new(&args.file))?;

        // Features requested at the command-line take precedence, then the
        // config file, then empty.
        let mut features: HashSet<String> = match (args.features, file.features.clone()) {
            (Some(arg), _) => {
                file.validate_features(&arg);
                arg.into_iter().collect()
            }
            (None, Some(defined)) => defined,
            (None, None) => HashSet::new(),
        };

        if let Some(remove) = args.skipped_features {
            file.validate_features(&remove);
            for feature in remove.iter() {
                features.remove(feature);
            }
        }

        let options = Options {
            column_name: file.column_name,
            column_values: args.ids,
            database_url: file
                .database_url
                .or(args.database_url)
                .unwrap_or_else(|| "postgres://localhost:5432/postgres".to_string()),
            schema: file.schema_name,
            accept_invalid_certs: file.accept_invalid_certs.unwrap_or(false),
            skip_tables: match file.skip_tables {
                Some(patterns) => RegexSet::new(patterns)?,
                None => RegexSet::empty(),
            },
            overrides: file.overrides.unwrap_or_default(),
            estimate_only: args.estimate_only,
            truncate: args.truncate,
            features,
        };
        Ok(options)
    }
}

fn pg_client(options: &Options) -> Result<Client, Box<dyn Error>> {
    mod danger {
        pub struct NoCertificateVerification {}

        impl rustls::client::ServerCertVerifier for NoCertificateVerification {
            fn verify_server_cert(
                &self,
                _end_entity: &rustls::Certificate,
                _intermediates: &[rustls::Certificate],
                _server_name: &rustls::ServerName,
                _scts: &mut dyn Iterator<Item = &[u8]>,
                _ocsp_response: &[u8],
                _now: std::time::SystemTime,
            ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
                Ok(rustls::client::ServerCertVerified::assertion())
            }
        }
    }

    let mut config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    if options.accept_invalid_certs {
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(danger::NoCertificateVerification {}));
    }
    let tls = tokio_postgres_rustls::MakeRustlsConnect::new(config);
    Ok(Client::connect(&options.database_url, tls)?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::load()?;

    let mut client = pg_client(&options)?;

    // Restrict `search_path` to just the one schema.
    client.execute(&format!("SET SCHEMA {}", options.schema.sql_value()), &[])?;
    client.execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;", &[])?;

    // Populate features settings
    client.execute(
        &format!(
            "SET pg_parcel.features = '{{{}}}'",
            &options
                .features
                .clone()
                .into_iter()
                .collect::<Vec<String>>()
                .join(",")
        ),
        &[],
    )?;
    for feature in options.features.iter() {
        client.execute(&format!("SET pg_parcel.feature.{feature} = true"), &[])?;
    }

    client.execute(
        &format!(
            "SET pg_parcel.ids = '{{{}}}'",
            &options.column_values.join(",")
        ),
        &[],
    )?;

    let tables = get_tables(&options)?;

    let pb = ProgressBar::new(tables.len() as u64);
    let pb_template = format!(
        "{{msg:>{width}.bold}} {{spinner:.blue/white}} {{wide_bar:.blue/white}} eta {{eta}}",
        width = tables
            .iter()
            .map(|table| table.name.len())
            .max()
            .unwrap_or(30)
    );
    pb.set_style(
        ProgressStyle::with_template(&pb_template)
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    pb.enable_steady_tick(Duration::from_millis(250));

    if options.estimate_only {
        let mut total_size: u64 = 0; // Estimate in kibibytes.

        pb.println("        Rows / Total |         |  Size estimate | Table name");
        for table in tables.iter() {
            let count_statement = format!(
                // The `postgres` crate does not define `FromSql for u64` (or
                // usize, or u128), so it would appear that the only safe way to
                // query a PostgreSQL `int8` is as text.
                "SELECT COUNT(*)::text FROM ({}) AS query",
                table.copy_out_query(&options)
            );
            pb.set_message(table.name.to_owned());
            let row_count_s: String = client.query_one(&count_statement, &[])?.get(0);
            let row_count: u64 = row_count_s.parse()?;
            let row_selectivity = (100f64 * row_count as f64 / table.rows as f64)
                .max(0.0) // Deal with NAN.
                .clamp(0.0, 100.0);
            let size_estimate = if table.rows > 0 {
                let size = (row_count as f64 * table.size as f64) / (table.rows as f64 * 1024f64);
                size.max(0.0) as u64 // Deal with NAN.
            } else {
                0u64
            };
            pb.println(format!(
                "{row_frac:>20} | {row_selectivity:>6.2}% | {size_estimate:10.0} kiB | {name}",
                row_frac = format!("{row_count} of {rows_total}", rows_total = table.rows),
                name = table.name
            ));
            pb.inc(1);
            total_size = total_size.saturating_add(size_estimate);
        }
        pb.finish_with_message(format!("Total size estimated at: {total_size} kiB"));
    } else {
        let mut sizes: Vec<(String, u64)> = Vec::with_capacity(tables.len());

        // Truncate tables first. There can be foreign key relationships between
        // tables so either we need to truncate all tables now or we need to
        // truncate with cascade as we go along, but we can't do the latter
        // because we might truncate tables we've only just populated.
        if options.truncate {
            writeln!(
                std::io::stdout(),
                "TRUNCATE TABLE\n  {}\n;",
                // `iter_intersperse` is an unstable feature in the standard
                // library. When it stabilises, we can remove `itertools` and
                // just chain into `Iterator.intersperse` instead.
                itertools::Itertools::intersperse(
                    tables.iter().map(Table::sql_identifier),
                    ",\n  ".to_owned(),
                )
                .collect::<String>()
            )?;
        }

        // Dump table data.
        for table in tables.iter() {
            let query = table.copy_out_query(&options);
            // let query = format!("{query} LIMIT 10"); // TESTING ONLY
            let copy_statement = format!("COPY ({}) TO stdout;", query);
            pb.set_message(table.name.to_owned());

            let mut stdout = std::io::stdout();
            writeln!(stdout, "{};", table.copy_in_query())?;
            let mut reader = client.copy_out(&copy_statement)?;
            let size = std::io::copy(&mut reader, &mut stdout)?;
            sizes.push((table.name.clone(), size));
            writeln!(stdout, "\\.")?;

            pb.inc(1);
        }

        // Summarize table sizes. Append the report to the dump as SQL comments.
        {
            let total = sizes.iter().map(|(.., size)| *size).sum::<u64>();
            if total > 0 {
                let mut stdout = std::io::stdout();
                writeln!(stdout)?;
                writeln!(stdout, "-- SUMMARY ---------------------------------")?;
                writeln!(stdout, "--        Bytes | % of total | Table name")?;
                writeln!(stdout, "-- -----------------------------------------")?;
                sizes.sort_by_key(|(.., size)| *size);
                for (name, size) in sizes.iter() {
                    let percent = ((*size as f64) * 100f64) / (total as f64);
                    writeln!(stdout, "-- {size:12} | {percent:9.1}% | {name}")?;
                }
            }
        }

        pb.finish_with_message(format!("Dumped {} tables", tables.len()));
    }

    client.query("ROLLBACK", &[])?;

    Ok(())
}

#[derive(Debug, Clone)]
struct Table {
    name: String,
    columns: Vec<Column>,
    schema: String,
    size: u64, // Bytes.
    rows: u64, // Estimate.
}

impl Table {
    fn sql_identifier(&self) -> String {
        format!(
            "{}.{}",
            self.schema.sql_identifier(),
            self.name.sql_identifier()
        )
    }
    fn copy_out_query(&self, options: &Options) -> String {
        let column_values = options.column_values.iter().map(|s| s.sql_value());
        let column_values = intersperse(column_values, ",".to_string()).collect::<String>();
        if let Some(query) = options.overrides.get(&self.name) {
            lazy_static! {
                static ref RE: Regex = Regex::new(r":ids\b").unwrap();
            }
            RE.replace_all(query, format!("({column_values})"))
                .to_string()
        } else {
            let query = format!(
                "SELECT {} FROM {}",
                &self.column_list(),
                &self.sql_identifier()
            );
            if let Some(scope_column) = self
                .columns
                .iter()
                .find(|column| column.name == options.column_name)
            {
                let column_ident = options.column_name.sql_identifier();
                if scope_column.is_nullable {
                    format!("{query} WHERE {column_ident} IN ({column_values}) OR {column_ident} IS NULL")
                } else {
                    format!("{query} WHERE {column_ident} IN ({column_values})")
                }
            } else {
                query
            }
        }
    }
    fn copy_in_query(&self) -> String {
        format!(
            "COPY {schema}.{name} ({columns}) FROM stdin",
            schema = self.schema.sql_identifier(),
            name = self.name.sql_identifier(),
            columns = self.column_list()
        )
    }
    fn column_list(&self) -> String {
        self.columns
            .iter()
            .map(|column| column.name.sql_identifier())
            .collect::<Vec<String>>()
            .join(", ")
    }
}

#[derive(Debug, Clone)]
struct Column {
    pub name: String,
    pub is_nullable: bool,
}

async fn get_tables(options: &Options) -> Result<Vec<Table>, Box<dyn Error>> {
    let pool = PgPoolOptions::new().connect(&options.database_url).await?;
    let records = sqlx::query!(
        r#"
        select
          tables.table_name as "table_name!",
          pg_total_relation_size(pg_class.oid) as "table_size!",
          max(pg_class.reltuples::int8) as "table_rows!", -- https://wiki.postgresql.org/wiki/Count_estimate
          array_agg(columns.column_name::text order by columns.ordinal_position) as "column_names!",
          array_agg(columns.is_nullable = 'YES' order by columns.ordinal_position) as "column_nullables!"
        from information_schema.tables
        join information_schema.columns on (
          columns.table_catalog = tables.table_catalog
          and columns.table_schema = tables.table_schema
          and columns.table_name = tables.table_name
          and columns.is_generated = 'NEVER'
        )
        join pg_namespace on (
          pg_namespace.nspname = tables.table_schema)
        join pg_class on (
          pg_class.relnamespace = pg_namespace.oid
          and pg_class.relname = tables.table_name)
        where tables.table_schema = $1
        and tables.table_type = 'BASE TABLE'
        group by tables.table_name, pg_class.oid
        order by tables.table_name
        "#, &options.schema
    )
    .fetch_all(&pool)
    .await?;
    println!("{:#?}", records);

    let tables: Vec<Table> = records
        .iter()
        .filter_map(|row| {
            println!("{:?}", row);
            if options.skip_tables.is_match(&row.table_name) {
                None
            } else {
                let columns = row
                    .column_names
                    .iter()
                    .zip(row.column_nullables.clone())
                    .map(|(name, is_nullable)| Column {
                        name: name.to_string(),
                        is_nullable,
                    })
                    .collect();
                Some(Table {
                    name: row.table_name.to_string(),
                    columns,
                    schema: options.schema.to_string(),
                    size: row.table_size as u64,
                    rows: row.table_rows as u64,
                })
            }
        })
        .collect();

    Ok(tables)
}
