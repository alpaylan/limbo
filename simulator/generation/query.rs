use crate::generation::{Arbitrary, ArbitraryFrom, Shadow};
use crate::model::query::predicate::Predicate;
use crate::model::query::select::{
    CompoundOperator, CompoundSelect, Distinctness, FromClause, JoinTable, JoinType, JoinedTable,
    ResultColumn, SelectBody, SelectInner,
};
use crate::model::query::update::Update;
use crate::model::query::{Create, Delete, Drop, Insert, Query, Select};
use crate::model::table::{SimValue, Table};
use crate::SimulatorEnv;
use itertools::Itertools;
use rand::Rng;

use super::property::Remaining;
use super::{backtrack, frequency, pick};

impl Arbitrary for Create {
    fn arbitrary<R: Rng>(rng: &mut R) -> Self {
        Create {
            table: Table::arbitrary(rng),
        }
    }
}

impl ArbitraryFrom<&Vec<Table>> for FromClause {
    fn arbitrary_from<R: Rng>(rng: &mut R, tables: &Vec<Table>) -> Self {
        let query_size = (rng.gen_range(0.0..=15.0_f32).log2().ceil() as usize).saturating_sub(2);

        let mut tables = tables.clone();
        let mut table = pick(&tables, rng).clone();

        tables.retain(|t| t.name != table.name);

        let name = table.name.clone();

        let joins: Vec<_> = (0..query_size)
            .filter_map(|_| {
                if tables.is_empty() {
                    return None;
                }
                let join_table = pick(&tables, rng).clone();
                tables.retain(|t| t.name != join_table.name);
                table = JoinTable {
                    tables: vec![table.clone(), join_table.clone()],
                    rows: table
                        .rows
                        .iter()
                        .cartesian_product(join_table.rows.iter())
                        .map(|(t_row, j_row)| {
                            let mut row = t_row.clone();
                            row.extend(j_row.clone());
                            row
                        })
                        .collect(),
                }
                .into_table();
                for row in &mut table.rows {
                    assert_eq!(
                        row.len(),
                        table.columns.len(),
                        "Row length does not match column length after join"
                    );
                }

                let predicate = Predicate::arbitrary_from(rng, &table);
                Some(JoinedTable {
                    table: join_table.name.clone(),
                    join_type: JoinType::Inner,
                    on: predicate,
                })
            })
            .collect();
        FromClause { table: name, joins }
    }
}

impl ArbitraryFrom<&Vec<Table>> for SelectInner {
    fn arbitrary_from<R: Rng>(rng: &mut R, tables: &Vec<Table>) -> Self {
        let from = FromClause::arbitrary_from(rng, tables);
        let mut tables = tables.clone();
        // todo: this is a temporary hack because env is not separated from the tables
        let join_table = from
            .shadow(&mut tables)
            .expect("Failed to shadow FromClause")
            .into_table();

        SelectInner {
            distinctness: Distinctness::arbitrary(rng),
            columns: vec![ResultColumn::Star],
            from,
            where_clause: Predicate::arbitrary_from(rng, &join_table),
        }
    }
}

impl Arbitrary for Distinctness {
    fn arbitrary<R: Rng>(rng: &mut R) -> Self {
        match rng.gen_range(0..=5) {
            0..4 => Distinctness::All,
            _ => Distinctness::Distinct,
        }
    }
}
impl Arbitrary for CompoundOperator {
    fn arbitrary<R: Rng>(rng: &mut R) -> Self {
        match rng.gen_range(0..=1) {
            0 => CompoundOperator::Union,
            1 => CompoundOperator::UnionAll,
            _ => unreachable!(),
        }
    }
}

impl ArbitraryFrom<&Vec<Table>> for Select {
    fn arbitrary_from<R: Rng>(rng: &mut R, tables: &Vec<Table>) -> Self {
        let query_size = (rng.gen_range(0.0..=15.0_f32).log2().ceil() as usize).saturating_sub(2);

        let table = pick(tables, rng);

        let num_selects = rng.gen_range(0..=query_size);
        let first = SelectInner::arbitrary_from(rng, tables);

        let rest: Vec<SelectInner> = (0..num_selects)
            .map(|_| {
                let mut select = first.clone();
                select.where_clause = Predicate::arbitrary_from(rng, table);
                select
            })
            .collect();

        Self {
            body: SelectBody {
                select: Box::new(first),
                compounds: rest
                    .into_iter()
                    .map(|s| CompoundSelect {
                        operator: CompoundOperator::arbitrary(rng),
                        select: Box::new(s),
                    })
                    .collect(),
            },
            limit: None,
        }
    }
}

impl ArbitraryFrom<&SimulatorEnv> for Insert {
    fn arbitrary_from<R: Rng>(rng: &mut R, env: &SimulatorEnv) -> Self {
        let gen_values = |rng: &mut R| {
            let table = pick(&env.tables, rng);
            let num_rows = rng.gen_range(1..10);
            let values: Vec<Vec<SimValue>> = (0..num_rows)
                .map(|_| {
                    table
                        .columns
                        .iter()
                        .map(|c| SimValue::arbitrary_from(rng, &c.column_type))
                        .collect()
                })
                .collect();
            Some(Insert::Values {
                table: table.name.clone(),
                values,
            })
        };

        let _gen_select = |rng: &mut R| {
            // Find a non-empty table
            let select_table = env.tables.iter().find(|t| !t.rows.is_empty())?;
            let row = pick(&select_table.rows, rng);
            let predicate = Predicate::arbitrary_from(rng, (select_table, row));
            // Pick another table to insert into
            let select = Select::simple(select_table.name.clone(), predicate);
            let table = pick(&env.tables, rng);
            Some(Insert::Select {
                table: table.name.clone(),
                select: Box::new(select),
            })
        };

        // Backtrack here cannot return None
        backtrack(
            vec![
                (1, Box::new(gen_values)),
                // todo: test and enable this once `INSERT INTO <table> SELECT * FROM <table>` is supported
                // (1, Box::new(|rng| gen_select(rng))),
            ],
            rng,
        )
        .unwrap()
    }
}

impl ArbitraryFrom<&SimulatorEnv> for Delete {
    fn arbitrary_from<R: Rng>(rng: &mut R, env: &SimulatorEnv) -> Self {
        let table = pick(&env.tables, rng);
        Self {
            table: table.name.clone(),
            predicate: Predicate::arbitrary_from(rng, table),
        }
    }
}

impl ArbitraryFrom<&SimulatorEnv> for Drop {
    fn arbitrary_from<R: Rng>(rng: &mut R, env: &SimulatorEnv) -> Self {
        let table = pick(&env.tables, rng);
        Self {
            table: table.name.clone(),
        }
    }
}

impl ArbitraryFrom<(&SimulatorEnv, &Remaining)> for Query {
    fn arbitrary_from<R: Rng>(rng: &mut R, (env, remaining): (&SimulatorEnv, &Remaining)) -> Self {
        frequency(
            vec![
                (
                    remaining.create,
                    Box::new(|rng| Self::Create(Create::arbitrary(rng))),
                ),
                (
                    remaining.read,
                    Box::new(|rng| Self::Select(Select::arbitrary_from(rng, &env.tables))),
                ),
                (
                    remaining.write,
                    Box::new(|rng| Self::Insert(Insert::arbitrary_from(rng, env))),
                ),
                (
                    remaining.update,
                    Box::new(|rng| Self::Update(Update::arbitrary_from(rng, env))),
                ),
                (
                    f64::min(remaining.write, remaining.delete),
                    Box::new(|rng| Self::Delete(Delete::arbitrary_from(rng, env))),
                ),
            ],
            rng,
        )
    }
}

impl ArbitraryFrom<&SimulatorEnv> for Update {
    fn arbitrary_from<R: Rng>(rng: &mut R, env: &SimulatorEnv) -> Self {
        let table = pick(&env.tables, rng);
        let num_cols = rng.gen_range(1..=table.columns.len());
        let set_values: Vec<(String, SimValue)> = (0..num_cols)
            .map(|_| {
                let column = pick(&table.columns, rng);
                (
                    column.name.clone(),
                    SimValue::arbitrary_from(rng, &column.column_type),
                )
            })
            .collect();
        Update {
            table: table.name.clone(),
            set_values,
            predicate: Predicate::arbitrary_from(rng, table),
        }
    }
}

#[cfg(test)]
mod query_generation_tests {
    use rand::RngCore;
    use turso_core::Value;
    use turso_sqlite3_parser::to_sql_string::ToSqlString;

    use super::*;
    use crate::model::query::predicate::Predicate;
    use crate::model::query::EmptyContext;
    use crate::model::table::{Column, ColumnType};
    use crate::SimulatorEnv;

    #[test]
    fn test_select_query_generation() {
        let mut rng = rand::thread_rng();
        // CREATE TABLE users (id INTEGER, name TEXT);
        // INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob');
        // CREATE TABLE orders (order_id INTEGER, user_id INTEGER);
        // INSERT INTO orders (order_id, user_id) VALUES (1, 1), (2, 2);

        let tables = vec![
            Table {
                name: "users".to_string(),
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        column_type: ColumnType::Integer,
                        primary: false,
                        unique: false,
                    },
                    Column {
                        name: "name".to_string(),
                        column_type: ColumnType::Text,
                        primary: false,
                        unique: false,
                    },
                ],
                rows: vec![
                    vec![
                        SimValue(Value::Integer(1)),
                        SimValue(Value::Text("Alice".into())),
                    ],
                    vec![
                        SimValue(Value::Integer(2)),
                        SimValue(Value::Text("Bob".into())),
                    ],
                ],
            },
            Table {
                name: "orders".to_string(),
                columns: vec![
                    Column {
                        name: "order_id".to_string(),
                        column_type: ColumnType::Integer,
                        primary: false,
                        unique: false,
                    },
                    Column {
                        name: "user_id".to_string(),
                        column_type: ColumnType::Integer,
                        primary: false,
                        unique: false,
                    },
                ],
                rows: vec![
                    vec![SimValue(Value::Integer(1)), SimValue(Value::Integer(1))],
                    vec![SimValue(Value::Integer(2)), SimValue(Value::Integer(2))],
                ],
            },
            Table {
                name: "products".to_string(),
                columns: vec![
                    Column {
                        name: "product_id".to_string(),
                        column_type: ColumnType::Integer,
                        primary: true,
                        unique: true,
                    },
                    Column {
                        name: "product_name".to_string(),
                        column_type: ColumnType::Text,
                        primary: false,
                        unique: false,
                    },
                ],
                rows: vec![
                    vec![
                        SimValue(Value::Integer(1)),
                        SimValue(Value::Text("Widget".into())),
                    ],
                    vec![
                        SimValue(Value::Integer(2)),
                        SimValue(Value::Text("Gadget".into())),
                    ],
                ],
            },
        ];
        for _ in 0..100 {
            let query = Select::arbitrary_from(&mut rng, &tables);
            println!("{}", query.to_sql_ast().to_sql_string(&EmptyContext {}));
            // println!("{:?}", query.to_sql_ast());
        }
    }
}
