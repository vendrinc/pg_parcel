mod inputfile;
mod sql_string;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use inputfile::InputFile;
use lazy_static::lazy_static;
use postgres::{Client, NoTls};
use regex::Regex;
use sql_string::SqlString;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::io::Write;
use std::path::Path;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/// Command line arguments
struct Args {
    /// Configuration file
    #[clap(short, long, display_order = 1)]
    #[clap(default_value_t = String::from("./pg_parcel.toml"))]
    file: String,

    /// Dump only columns where `column_name` is this value
    #[clap(short, long, display_order = 2)]
    id: String,

    /// Prints a report estimating row count and size of the data to be dumped
    /// for each table, and in total. Does not dump table data.
    ///
    /// Note that the figures reported may be well off the mark, especially the
    /// estimated size of the dump, but they should be off the mark by a roughly
    /// constant factor.
    #[clap(long, display_order = 10)]
    estimate_only: bool,
}

/// Options here is a combination of command line arguments and contents of the slicefile.
struct Options {
    column_name: String,
    column_value: String,
    schema: String,
    database_url: String,
    skip_tables: HashSet<String>,
    overrides: HashMap<String, String>,
    estimate_only: bool,
}

impl Options {
    pub fn load() -> Result<Options, Box<dyn Error>> {
        let args = Args::parse();
        let file = InputFile::load(Path::new(&args.file))?;
        let options = Options {
            column_name: file.column_name,
            column_value: args.id,
            database_url: file.database_url,
            schema: file.schema_name,
            skip_tables: file.skip_tables.unwrap_or_default(),
            overrides: file.overrides.unwrap_or_default(),
            estimate_only: args.estimate_only,
        };
        Ok(options)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::load()?;
    let mut client = Client::connect(&options.database_url, NoTls)?;

    // Restrict `search_path` to just the one schema.
    client.execute(&format!("SET SCHEMA {}", options.schema.sql_value()), &[])?;
    client.execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;", &[])?;

    let tables = get_tables(&options)?;

    let pb = ProgressBar::new(tables.len() as u64);
    let pb_template = format!(
        "{{msg:>{width}.bold}} {{spinner}} {{wide_bar}} eta {{eta}} ",
        width = tables
            .iter()
            .map(|table| table.name.len())
            .max()
            .unwrap_or(30)
    );
    pb.set_style(ProgressStyle::default_bar().template(&pb_template));
    pb.enable_steady_tick(250);

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
            let size_estimate = ((row_count as f64 * table.size as f64)
                / (table.rows as f64 * 1024f64))
                .max(0.0) as u64; // Deal with NAN.
            pb.println(format!(
                "{row_frac:>20} | {row_selectivity:>6.2}% | {size_estimate:10.0} kiB | {name}",
                row_frac = format!("{row_count} of {rows_total}", rows_total = table.rows),
                name = table.name
            ));
            pb.inc(1);
            total_size += size_estimate;
        }
        pb.finish_with_message(format!("Total size estimated at: {total_size} kiB"));
    } else {
        let mut sizes: Vec<(String, u64)> = Vec::with_capacity(tables.len());

        // Dump table data.
        for table in tables.iter() {
            let copy_statement = format!("COPY ({}) TO stdout;", table.copy_out_query(&options));
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
        if let Some(query) = options.overrides.get(&self.name) {
            lazy_static! {
                static ref RE: Regex = Regex::new(r":id\b").unwrap();
            }
            RE.replace_all(query, &options.column_value.sql_value())
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
                let column_value = options.column_value.sql_value();
                if scope_column.is_nullable {
                    format!(
                        "{query} WHERE {column_ident} = {column_value} OR {column_ident} IS NULL"
                    )
                } else {
                    format!("{query} WHERE {column_ident} = {column_value}")
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

fn get_tables(options: &Options) -> Result<Vec<Table>, Box<dyn Error>> {
    let mut client = Client::connect(&options.database_url, NoTls)?;
    let query = format!(
        r#"
        select
          tables.table_name,
          pg_total_relation_size(pg_class.oid)::text as table_size,
          max(pg_class.reltuples::int8)::text as table_rows, -- https://wiki.postgresql.org/wiki/Count_estimate
          array_agg(columns.column_name::text order by columns.ordinal_position) as column_names,
          array_agg(columns.is_nullable = 'YES' order by columns.ordinal_position) as column_nullables
        from information_schema.tables
        join information_schema.columns on (
          columns.table_catalog = tables.table_catalog
          and columns.table_schema = tables.table_schema
          and columns.table_name = tables.table_name)
        join pg_namespace on (
          pg_namespace.nspname = tables.table_schema)
        join pg_class on (
          pg_class.relnamespace = pg_namespace.oid
          and pg_class.relname = tables.table_name)
        where tables.table_schema = {schema}
        and tables.table_type = 'BASE TABLE'
        group by tables.table_name, pg_class.oid
        order by tables.table_name
        "#,
        schema = options.schema.sql_value(),
    );
    let mut tables: Vec<Table> = client
        .query(&query, &[])?
        .into_iter()
        .filter_map(|row| {
            let table_name: String = row.get("table_name");
            if !options.skip_tables.contains(&table_name) {
                let table_size_s: String = row.get("table_size");
                let table_size: u64 = table_size_s.parse().unwrap_or(0);
                let table_rows_s: String = row.get("table_rows");
                let table_rows: u64 = table_rows_s.parse().unwrap_or(0);
                let column_names: Vec<String> = row.get("column_names");
                let column_nullables: Vec<bool> = row.get("column_nullables");
                let columns = column_names
                    .into_iter()
                    .zip(column_nullables)
                    .map(|(name, is_nullable)| Column { name, is_nullable })
                    .collect();
                Some(Table {
                    name: table_name,
                    columns,
                    schema: options.schema.clone(),
                    size: table_size,
                    rows: table_rows,
                })
            } else {
                None
            }
        })
        .collect();

    tables.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tables)
}
