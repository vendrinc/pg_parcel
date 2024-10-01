use super::*;

#[cfg(test)]
mod copy_out_query_tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use regex::RegexSet;

    // Helper function to create a simple table for testing
    fn create_test_table() -> Table {
        Table {
            name: "test_table".to_string(),
            columns: vec![
                Column { name: "id".to_string(), is_nullable: false },
                Column { name: "name".to_string(), is_nullable: true },
            ],
            schema: "public".to_string(),
            size: 0,
            rows: 0,
        }
    }

    // Helper function to create a simple options for testing
    fn create_test_options() -> Options {
        Options {
            column_name: "id".to_string(),
            column_values: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            schema: "public".to_string(),
            database_url: "postgres://localhost/test".to_string(),
            accept_invalid_certs: false,
            skip_tables: RegexSet::empty(),
            overrides: HashMap::new(),
            estimate_only: false,
            truncate: false,
            features: HashSet::new(),
        }
    }

    #[test]
    fn test_simple_query_without_override() {
        let table = create_test_table();
        let mut options = create_test_options();
        options.column_name = "id".to_string();
        options.column_values = vec!["1".to_string(), "2".to_string(), "3".to_string()];

        let result = table.copy_out_query(&options);
        assert_eq!(result, r#"SELECT "id", "name" FROM "public"."test_table" WHERE "id" IN ('1','2','3')"#);
    }

    #[test]
    fn test_nullable_column_query_without_override() {
        let table = create_test_table();
        let mut options = create_test_options();
        options.column_name = "name".to_string();
        options.column_values = vec!["'Alice'".to_string(), "'Bob'".to_string()];

        let result = table.copy_out_query(&options);
        assert_eq!(result, r#"SELECT "id", "name" FROM "public"."test_table" WHERE "name" IN (''Alice'',''Bob'') OR "name" IS NULL"#);
    }

    #[test]
    fn test_simple_override() {
        let table = create_test_table();
        let mut options = create_test_options();
        options.overrides.insert("test_table".to_string(), "SELECT * FROM test_table WHERE id IN (:ids)".to_string());

        let result = table.copy_out_query(&options);
        assert_eq!(result, r#"SELECT * FROM test_table WHERE id IN (('1','2','3'))"#);
    }

    #[test]
    fn test_complex_override() {
        let table = create_test_table();
        let mut options = create_test_options();
        options.overrides.insert(
            "test_table".to_string(),
            "SELECT t.*, o.order_date FROM test_table t JOIN orders o ON t.id = o.user_id WHERE t.id IN (:ids) AND o.order_date > '2023-01-01'".to_string()
        );

        let result = table.copy_out_query(&options);
        assert_eq!(result, "SELECT t.*, o.order_date FROM test_table t JOIN orders o ON t.id = o.user_id WHERE t.id IN (('1','2','3')) AND o.order_date > '2023-01-01'");
    }

    #[test]
    fn test_override_without_ids_placeholder() {
        let table = create_test_table();
        let mut options = create_test_options();
        options.overrides.insert(
            "test_table".to_string(),
            "SELECT * FROM test_table WHERE active = true".to_string()
        );

        let result = table.copy_out_query(&options);
        assert_eq!(result, "SELECT * FROM test_table WHERE active = true");
    }
}