use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{generation::Shadow, model::table::{SimValue, Table}, SimulatorEnv};

use super::predicate::Predicate;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Update {
    pub(crate) table: String,
    pub(crate) set_values: Vec<(String, SimValue)>, // Pair of value for set expressions => SET name=value
    pub(crate) predicate: Predicate,
}

impl Shadow for Update {
    type Result = anyhow::Result<Vec<Vec<SimValue>>>;

    fn shadow(&self, tables: &mut Vec<Table>) -> Self::Result {
        let table = tables.iter_mut().find(|t| t.name == self.table);

        let table = if let Some(table) = table {
            table
        } else {
            return Err(anyhow::anyhow!(
                "Table {} does not exist. UPDATE statement ignored.",
                self.table
            ));
        };

        let t2 = table.clone();
        for row in table
            .rows
            .iter_mut()
            .filter(|r| self.predicate.test(r, &t2))
        {
            for (column, set_value) in &self.set_values {
                table
                    .columns
                    .iter()
                    .enumerate()
                    .find(|(_, c)| &c.name == column)
                    .map(|(idx, _)| {
                        row[idx] = set_value.clone();
                    });
            }
        }

        Ok(vec![])
    }
}

impl Display for Update {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UPDATE {} SET ", self.table)?;
        for (i, (name, value)) in self.set_values.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{} = {}", name, value)?;
        }
        write!(f, " WHERE {}", self.predicate)?;
        Ok(())
    }
}
