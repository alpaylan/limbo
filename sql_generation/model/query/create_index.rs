use serde::{Deserialize, Serialize};
use turso_parser::ast::SortOrder;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CreateIndex {
    pub index_name: String,
    pub table_name: String,
    pub columns: Vec<(String, SortOrder)>,
}

impl std::fmt::Display for CreateIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CREATE INDEX {} ON {} ({})",
            self.index_name,
            self.table_name,
            self.columns
                .iter()
                .map(|(name, order)| format!("{name} {order}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}
