pub trait SqlString {
    fn sql_identifier(&self) -> Self;
    fn sql_value(&self) -> Self;
}

// TODO we can actually skip quoting if it's not a reserved word and it doesn't have mixed case.
impl SqlString for String {
    fn sql_identifier(&self) -> Self {
        format!(r#""{}""#, self)
    }
    fn sql_value(&self) -> Self {
        format!(r#"'{}'"#, self)
    }
}
