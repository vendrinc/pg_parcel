use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashSet;

pub trait SqlString {
    fn sql_identifier(&self) -> Self;
    fn sql_value(&self) -> Self;
}

impl SqlString for String {
    fn sql_value(&self) -> Self {
        format!(r#"'{self}'"#)
    }

    fn sql_identifier(&self) -> Self {
        if needs_quoting(self) {
            return format!(r#""{self}""#);
        }
        self.to_owned()
    }
}

fn needs_quoting(string: &str) -> bool {
    let lower = string.to_ascii_lowercase();

    // If it doesn't match the lower case equivalent, assume the column name is actually mixed cased and that we need to quote it.
    if string != lower {
        return true;
    }

    if NOT_ALPHANUMERIC.is_match(string) {
        return true;
    }

    if POSTGRESQL_RESERVED_WORDS.contains(lower.as_str()) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use crate::sql_string::*;

    #[test]
    fn it_does_not_quote_when_optional() {
        assert_eq!("hello".to_string().sql_identifier(), "hello")
    }

    #[test]
    fn it_quotes_mixed_case() {
        assert_eq!("fullName".to_string().sql_identifier(), "\"fullName\"")
    }

    #[test]
    fn it_quotes_reserved_words() {
        assert_eq!("action".to_string().sql_identifier(), "\"action\"")
    }

    #[test]
    fn it_quotes_terrible_names() {
        assert_eq!("my data".to_string().sql_identifier(), "\"my data\"")
    }
}

lazy_static! {
    static ref NOT_ALPHANUMERIC: Regex = Regex::new("[^a-zA-Z0-9]").unwrap();

    /// Reserved words according to https://www.postgresql.org/docs/14/sql-keywords-appendix.html
    static ref POSTGRESQL_RESERVED_WORDS: HashSet<&'static str> = [
        "a",
        "abort",
        "abs",
        "absent",
        "absolute",
        "access",
        "according",
        "acos",
        "action",
        "ada",
        "add",
        "admin",
        "after",
        "aggregate",
        "all",
        "allocate",
        "also",
        "alter",
        "always",
        "analyse",
        "analyze",
        "and",
        "any",
        "are",
        "array",
        "array_agg",
        "array_max_cardinality",
        "as",
        "asc",
        "asensitive",
        "asin",
        "assertion",
        "assignment",
        "asymmetric",
        "at",
        "atan",
        "atomic",
        "attach",
        "attribute",
        "attributes",
        "authorization",
        "avg",
        "backward",
        "base64",
        "before",
        "begin",
        "begin_frame",
        "begin_partition",
        "bernoulli",
        "between",
        "bigint",
        "binary",
        "bit",
        "bit_length",
        "blob",
        "blocked",
        "bom",
        "boolean",
        "both",
        "breadth",
        "by",
        "c",
        "cache",
        "call",
        "called",
        "cardinality",
        "cascade",
        "cascaded",
        "case",
        "cast",
        "catalog",
        "catalog_name",
        "ceil",
        "ceiling",
        "chain",
        "chaining",
        "char",
        "character",
        "characteristics",
        "characters",
        "character_length",
        "character_set_catalog",
        "character_set_name",
        "character_set_schema",
        "char_length",
        "check",
        "checkpoint",
        "class",
        "classifier",
        "class_origin",
        "clob",
        "close",
        "cluster",
        "coalesce",
        "cobol",
        "collate",
        "collation",
        "collation_catalog",
        "collation_name",
        "collation_schema",
        "collect",
        "column",
        "columns",
        "column_name",
        "command_function",
        "command_function_code",
        "comment",
        "comments",
        "commit",
        "committed",
        "compression",
        "concurrently",
        "condition",
        "conditional",
        "condition_number",
        "configuration",
        "conflict",
        "connect",
        "connection",
        "connection_name",
        "constraint",
        "constraints",
        "constraint_catalog",
        "constraint_name",
        "constraint_schema",
        "constructor",
        "contains",
        "content",
        "continue",
        "control",
        "conversion",
        "convert",
        "copy",
        "corr",
        "corresponding",
        "cos",
        "cosh",
        "cost",
        "count",
        "covar_pop",
        "covar_samp",
        "create",
        "cross",
        "csv",
        "cube",
        "cume_dist",
        "current",
        "current_catalog",
        "current_date",
        "current_default_transform_group",
        "current_path",
        "current_role",
        "current_row",
        "current_schema",
        "current_time",
        "current_timestamp",
        "current_transform_group_for_type",
        "current_user",
        "cursor",
        "cursor_name",
        "cycle",
        "data",
        "database",
        "datalink",
        "date",
        "datetime_interval_code",
        "datetime_interval_precision",
        "day",
        "db",
        "deallocate",
        "dec",
        "decfloat",
        "decimal",
        "declare",
        "default",
        "defaults",
        "deferrable",
        "deferred",
        "define",
        "defined",
        "definer",
        "degree",
        "delete",
        "delimiter",
        "delimiters",
        "dense_rank",
        "depends",
        "depth",
        "deref",
        "derived",
        "desc",
        "describe",
        "descriptor",
        "detach",
        "deterministic",
        "diagnostics",
        "dictionary",
        "disable",
        "discard",
        "disconnect",
        "dispatch",
        "distinct",
        "dlnewcopy",
        "dlpreviouscopy",
        "dlurlcomplete",
        "dlurlcompleteonly",
        "dlurlcompletewrite",
        "dlurlpath",
        "dlurlpathonly",
        "dlurlpathwrite",
        "dlurlscheme",
        "dlurlserver",
        "dlvalue",
        "do",
        "document",
        "domain",
        "double",
        "drop",
        "dynamic",
        "dynamic_function",
        "dynamic_function_code",
        "each",
        "element",
        "else",
        "empty",
        "enable",
        "encoding",
        "encrypted",
        "end",
        "end",
        "end_frame",
        "end_partition",
        "enforced",
        "enum",
        "equals",
        "error",
        "escape",
        "event",
        "every",
        "except",
        "exception",
        "exclude",
        "excluding",
        "exclusive",
        "exec",
        "execute",
        "exists",
        "exp",
        "explain",
        "expression",
        "extension",
        "external",
        "extract",
        "false",
        "family",
        "fetch",
        "file",
        "filter",
        "final",
        "finalize",
        "finish",
        "first",
        "first_value",
        "flag",
        "float",
        "floor",
        "following",
        "for",
        "force",
        "foreign",
        "format",
        "fortran",
        "forward",
        "found",
        "frame_row",
        "free",
        "freeze",
        "from",
        "fs",
        "fulfill",
        "full",
        "function",
        "functions",
        "fusion",
        "g",
        "general",
        "generated",
        "get",
        "global",
        "go",
        "goto",
        "grant",
        "granted",
        "greatest",
        "group",
        "grouping",
        "groups",
        "handler",
        "having",
        "header",
        "hex",
        "hierarchy",
        "hold",
        "hour",
        "id",
        "identity",
        "if",
        "ignore",
        "ilike",
        "immediate",
        "immediately",
        "immutable",
        "implementation",
        "implicit",
        "import",
        "in",
        "include",
        "including",
        "increment",
        "indent",
        "index",
        "indexes",
        "indicator",
        "inherit",
        "inherits",
        "initial",
        "initially",
        "inline",
        "inner",
        "inout",
        "input",
        "insensitive",
        "insert",
        "instance",
        "instantiable",
        "instead",
        "int",
        "integer",
        "integrity",
        "intersect",
        "intersection",
        "interval",
        "into",
        "invoker",
        "is",
        "isnull",
        "isolation",
        "join",
        "json",
        "json_array",
        "json_arrayagg",
        "json_exists",
        "json_object",
        "json_objectagg",
        "json_query",
        "json_table",
        "json_table_primitive",
        "json_value",
        "k",
        "keep",
        "key",
        "keys",
        "key_member",
        "key_type",
        "label",
        "lag",
        "language",
        "large",
        "last",
        "last_value",
        "lateral",
        "lead",
        "leading",
        "leakproof",
        "least",
        "left",
        "length",
        "level",
        "library",
        "like",
        "like_regex",
        "limit",
        "link",
        "listagg",
        "listen",
        "ln",
        "load",
        "local",
        "localtime",
        "localtimestamp",
        "location",
        "locator",
        "lock",
        "locked",
        "log",
        "log10",
        "logged",
        "lower",
        "m",
        "map",
        "mapping",
        "match",
        "matched",
        "matches",
        "match_number",
        "match_recognize",
        "materialized",
        "max",
        "maxvalue",
        "measures",
        "member",
        "merge",
        "message_length",
        "message_octet_length",
        "message_text",
        "method",
        "min",
        "minute",
        "minvalue",
        "mod",
        "mode",
        "modifies",
        "module",
        "month",
        "more",
        "move",
        "multiset",
        "mumps",
        "name",
        "names",
        "namespace",
        "national",
        "natural",
        "nchar",
        "nclob",
        "nested",
        "nesting",
        "new",
        "next",
        "nfc",
        "nfd",
        "nfkc",
        "nfkd",
        "nil",
        "no",
        "none",
        "normalize",
        "normalized",
        "not",
        "nothing",
        "notify",
        "notnull",
        "nowait",
        "nth_value",
        "ntile",
        "null",
        "nullable",
        "nullif",
        "nulls",
        "number",
        "numeric",
        "object",
        "occurrences_regex",
        "octets",
        "octet_length",
        "of",
        "off",
        "offset",
        "oids",
        "old",
        "omit",
        "on",
        "one",
        "only",
        "open",
        "operator",
        "option",
        "options",
        "or",
        "order",
        "ordering",
        "ordinality",
        "others",
        "out",
        "outer",
        "output",
        "over",
        "overflow",
        "overlaps",
        "overlay",
        "overriding",
        "owned",
        "owner",
        "p",
        "pad",
        "parallel",
        "parameter",
        "parameter_mode",
        "parameter_name",
        "parameter_ordinal_position",
        "parameter_specific_catalog",
        "parameter_specific_name",
        "parameter_specific_schema",
        "parser",
        "partial",
        "partition",
        "pascal",
        "pass",
        "passing",
        "passthrough",
        "password",
        "past",
        "path",
        "pattern",
        "per",
        "percent",
        "percentile_cont",
        "percentile_disc",
        "percent_rank",
        "period",
        "permission",
        "permute",
        "placing",
        "plan",
        "plans",
        "pli",
        "policy",
        "portion",
        "position",
        "position_regex",
        "power",
        "precedes",
        "preceding",
        "precision",
        "prepare",
        "prepared",
        "preserve",
        "primary",
        "prior",
        "private",
        "privileges",
        "procedural",
        "procedure",
        "procedures",
        "program",
        "prune",
        "ptf",
        "public",
        "publication",
        "quote",
        "quotes",
        "range",
        "rank",
        "read",
        "reads",
        "real",
        "reassign",
        "recheck",
        "recovery",
        "recursive",
        "ref",
        "references",
        "referencing",
        "refresh",
        "regr_avgx",
        "regr_avgy",
        "regr_count",
        "regr_intercept",
        "regr_r2",
        "regr_slope",
        "regr_sxx",
        "regr_sxy",
        "regr_syy",
        "reindex",
        "relative",
        "release",
        "rename",
        "repeatable",
        "replace",
        "replica",
        "requiring",
        "reset",
        "respect",
        "restart",
        "restore",
        "restrict",
        "result",
        "return",
        "returned_cardinality",
        "returned_length",
        "returned_octet_length",
        "returned_sqlstate",
        "returning",
        "returns",
        "revoke",
        "right",
        "role",
        "rollback",
        "rollup",
        "routine",
        "routines",
        "routine_catalog",
        "routine_name",
        "routine_schema",
        "row",
        "rows",
        "row_count",
        "row_number",
        "rule",
        "running",
        "savepoint",
        "scalar",
        "scale",
        "schema",
        "schemas",
        "schema_name",
        "scope",
        "scope_catalog",
        "scope_name",
        "scope_schema",
        "scroll",
        "search",
        "second",
        "section",
        "security",
        "seek",
        "select",
        "selective",
        "self",
        "sensitive",
        "sequence",
        "sequences",
        "serializable",
        "server",
        "server_name",
        "session",
        "session_user",
        "set",
        "setof",
        "sets",
        "share",
        "show",
        "similar",
        "simple",
        "sin",
        "sinh",
        "size",
        "skip",
        "smallint",
        "snapshot",
        "some",
        "source",
        "space",
        "specific",
        "specifictype",
        "specific_name",
        "sql",
        "sqlcode",
        "sqlerror",
        "sqlexception",
        "sqlstate",
        "sqlwarning",
        "sqrt",
        "stable",
        "standalone",
        "start",
        "state",
        "statement",
        "static",
        "statistics",
        "stddev_pop",
        "stddev_samp",
        "stdin",
        "stdout",
        "storage",
        "stored",
        "strict",
        "string",
        "strip",
        "structure",
        "style",
        "subclass_origin",
        "submultiset",
        "subscription",
        "subset",
        "substring",
        "substring_regex",
        "succeeds",
        "sum",
        "support",
        "symmetric",
        "sysid",
        "system",
        "system_time",
        "system_user",
        "t",
        "table",
        "tables",
        "tablesample",
        "tablespace",
        "table_name",
        "tan",
        "tanh",
        "temp",
        "template",
        "temporary",
        "text",
        "then",
        "through",
        "ties",
        "time",
        "timestamp",
        "timezone_hour",
        "timezone_minute",
        "to",
        "token",
        "top_level_count",
        "trailing",
        "transaction",
        "transactions_committed",
        "transactions_rolled_back",
        "transaction_active",
        "transform",
        "transforms",
        "translate",
        "translate_regex",
        "translation",
        "treat",
        "trigger",
        "trigger_catalog",
        "trigger_name",
        "trigger_schema",
        "trim",
        "trim_array",
        "true",
        "truncate",
        "trusted",
        "type",
        "types",
        "uescape",
        "unbounded",
        "uncommitted",
        "unconditional",
        "under",
        "unencrypted",
        "union",
        "unique",
        "unknown",
        "unlink",
        "unlisten",
        "unlogged",
        "unmatched",
        "unnamed",
        "unnest",
        "until",
        "untyped",
        "update",
        "upper",
        "uri",
        "usage",
        "user",
        "user_defined_type_catalog",
        "user_defined_type_code",
        "user_defined_type_name",
        "user_defined_type_schema",
        "using",
        "utf16",
        "utf32",
        "utf8",
        "vacuum",
        "valid",
        "validate",
        "validator",
        "value",
        "values",
        "value_of",
        "varbinary",
        "varchar",
        "variadic",
        "varying",
        "var_pop",
        "var_samp",
        "verbose",
        "version",
        "versioning",
        "view",
        "views",
        "volatile",
        "when",
        "whenever",
        "where",
        "whitespace",
        "width_bucket",
        "window",
        "with",
        "within",
        "without",
        "work",
        "wrapper",
        "write",
        "xml",
        "xmlagg",
        "xmlattributes",
        "xmlbinary",
        "xmlcast",
        "xmlcomment",
        "xmlconcat",
        "xmldeclaration",
        "xmldocument",
        "xmlelement",
        "xmlexists",
        "xmlforest",
        "xmliterate",
        "xmlnamespaces",
        "xmlparse",
        "xmlpi",
        "xmlquery",
        "xmlroot",
        "xmlschema",
        "xmlserialize",
        "xmltable",
        "xmltext",
        "xmlvalidate",
        "year",
        "yes",
        "zone"
    ]
    .into_iter()
    .collect();
}
