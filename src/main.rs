use postgres::{Client, NoTls};
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;

static DATABASE_URL: &str = "postgres://localhost:15432/postgres";

static SCHEMA: &str = "saasdash";

static REFLECT_QUERY: &str = r#"
    select
        table_name,
        column_name,
        case is_nullable when 'YES' then True else False end as is_nullable
    from information_schema.columns
    where table_schema = 'saasdash'
    order by table_name, ordinal_position;
"#;

static PARTITION_COLUMN: &str = "OrganizationId";

fn main() {
    try_main().unwrap();
}

fn try_main() -> Result<(), Box<dyn Error>> {
    let client = Client::connect(DATABASE_URL, NoTls)?;
    // fu, borrow checker
    let mut copy_client = Client::connect(DATABASE_URL, NoTls)?;
    for table in get_tables(client)?.iter() {
        let copy_statement = format!(r#"copy ({}) to stdout"#, table.copy_out_query("lksjd"));
        let mut reader = copy_client.copy_out(&copy_statement)?;
        let mut buf = vec![];
        reader.read_to_end(&mut buf)?;
        println!("{}", table.copy_in_query());
        println!("{}", std::str::from_utf8(&buf)?);
        println!("\\.");
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct Table {
    pub name: String,
    // pub query: String,
    pub columns: Vec<Column>,
}

impl Table {
    fn copy_out_query(&self, partition_column_value: &str) -> String {
        let mut query = format!(r#"select * from "{}""#, &self.name);
        if let Some(org_scope) = self
            .columns
            .iter()
            .find(|column| column.name == PARTITION_COLUMN)
        {
            if org_scope.is_nullable {
                query = format!(
                    r#"{query} where "{PARTITION_COLUMN}" = '{}'"#,
                    partition_column_value
                )
            }
        }
        query = format!("{query} limit 100");
        query
    }
    fn copy_in_query(&self) -> String {
        // COPY saasdash.people (id, "primaryEmail", "fullName", "profileImageKey", "GoogleUserId", "OrganizationId", "createdAt", "updatedAt", name, "onboardDate", "offboardDate", "onboardingNotes", "personType", "deprecated_googleUserLastKnownEmailAccess", "googleUserSuspended", "googleUserDeletionTime", "personalEmail", "OktaUserId", "notesForOnboardee", "subscriptionCount", "appCount", "managedSpend", "deprecated_OneLoginUserId", "deprecated_BambooEmployeeId", "manuallyCreated", "deprecated_SalesforceUserId", "deprecated_ZendeskUserId", "deprecated_ZoomUserId", "activeAccounts", "idpActive", sources, status, "archivedAt", "ManagerPersonId") FROM stdin;
        let column_list = self
            .columns
            .iter()
            .map(|column| column.quoted_name())
            .collect::<Vec<String>>()
            .join(", ");

        format!(
            r#"COPY "{SCHEMA}"."{}" ({}) FROM stdin;"#,
            self.name, column_list
        )
    }
}

#[derive(Debug, Clone)]
struct Column {
    pub name: String,
    pub is_nullable: bool,
}

impl Column {
    fn quoted_name(&self) -> String {
        format!("\"{}\"", self.name)
    }
}

fn get_tables(mut client: Client) -> Result<Vec<Table>, Box<dyn Error>> {
    let tables: Vec<Table> = client
        .query(REFLECT_QUERY, &[])?
        .into_iter()
        .fold(
            HashMap::new(),
            |mut acc: HashMap<String, Vec<Column>>, row| {
                let table_name: &str = row.get("table_name");
                let column = Column {
                    name: row.get("column_name"),
                    is_nullable: row.get("is_nullable"),
                };
                if let Some(columns) = acc.get_mut(table_name) {
                    columns.push(column)
                } else {
                    acc.insert(table_name.to_owned(), vec![column]);
                }
                return acc;
            },
        )
        .into_iter()
        .map(|(name, columns)| {
            let table = Table { name, columns };
            table
        })
        .collect();
    Ok(tables)
}
