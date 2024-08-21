use std::{fs, path::PathBuf};

use log::{debug, error, info};
use rusqlite::{params, types::Value, Connection, Rows, ToSql};

#[allow(dead_code)]
#[derive(Debug)]
struct TablePragma {
    cid: u8,
    name: String,
    type_: String,
    notnull: i8,
    dflt_value: Option<i8>,
    pk: i8,
}

#[derive(Debug)]
struct PrimaryKeyInfo {
    primary_key_index: Option<u8>,
    last_primary_key: Option<i64>,
}

fn open_connection(db_url: &PathBuf) -> rusqlite::Connection {
    rusqlite::Connection::open(db_url).expect("open sqlite connection fail.")
}

fn query_table_structures(
    conn: &Connection,
    table: &String,
    table_structures: &mut Vec<Vec<TablePragma>>,
) -> Result<(), anyhow::Error> {
    let sql = format!("PRAGMA table_info({})", table);
    debug!("query table structures sql: {}", sql);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TablePragma {
                cid: row.get(0)?,
                name: row.get(1)?,
                type_: row.get(2)?,
                notnull: row.get(3)?,
                dflt_value: if row.get::<_, i8>(4).is_ok() {
                    Some(row.get(4)?)
                } else {
                    None
                },
                pk: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<TablePragma>, _>>()?;
    table_structures.push(rows);
    Ok(())
}

fn query_tables_last_entry(
    conn: &Connection,
    table: &str,
    structures: &[TablePragma],
) -> anyhow::Result<PrimaryKeyInfo> {
    match structures.iter().find(|item| item.pk == 1) {
        Some(primary_key) => {
            debug!("table: {}, primary key is: {:?}", table, primary_key);
            let primary_key_type = primary_key.type_.as_str();
            if primary_key_type != "INTEGER" && primary_key_type != "INT" {
                return Ok(PrimaryKeyInfo {
                    primary_key_index: Some(primary_key.cid),
                    last_primary_key: None,
                });
            }
            let mut sql = format!("SELECT COUNT({}) FROM {}", primary_key.name, table);
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let last_row = rows.next()?;
            match last_row {
                Some(row) => {
                    debug!("last row: {:?}", row);
                    let row_count = row.get::<_, i64>(0)?;
                    if row_count == 0 {
                        return Ok(PrimaryKeyInfo {
                            primary_key_index: Some(primary_key.cid),
                            last_primary_key: Some(0),
                        });
                    }
                    sql = format!("SELECT * FROM {} LIMIT 1 OFFSET {}", table, row_count - 1);
                    let mut stmt = conn.prepare(&sql)?;
                    let mut rows = stmt.query([])?;
                    let last_row = rows.next()?;
                    let last_primary_key = unsafe { last_row.unwrap_unchecked() }
                        .get::<_, i64>(primary_key.cid as usize)?;
                    Ok(PrimaryKeyInfo {
                        primary_key_index: Some(primary_key.cid),
                        last_primary_key: Some(last_primary_key),
                    })
                }
                None => Ok(PrimaryKeyInfo {
                    primary_key_index: Some(primary_key.cid),
                    last_primary_key: Some(0),
                }),
            }
        }
        None => Ok(PrimaryKeyInfo {
            primary_key_index: None,
            last_primary_key: None,
        }),
    }
}

pub fn merge_tables(
    src_files: &[PathBuf],
    dst_file: &PathBuf,
    batch_size: usize,
) -> anyhow::Result<()> {
    let sql = "SELECT name FROM sqlite_master WHERE type = 'table' AND name != 'sqlite_sequence'";
    let first_db_file = &src_files[0];
    fs::copy(first_db_file, dst_file)?;
    let dst_conn = open_connection(dst_file);
    let mut stmt = dst_conn.prepare(sql)?;
    let table_names = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<String>, _>>()?;

    debug!("tables: {:?}", table_names);

    let mut tables_structures = Vec::with_capacity(table_names.len());
    for table_name in &table_names {
        query_table_structures(&dst_conn, table_name, &mut tables_structures)?;
    }

    let table_structure_zip = table_names
        .into_iter()
        .zip(tables_structures)
        .collect::<Vec<(String, Vec<TablePragma>)>>();

    debug!("table structure zip: {:?}", table_structure_zip);

    let mut table_primary_key_info = table_structure_zip
        .iter()
        .map(|(table, structures)| {
            query_tables_last_entry(&dst_conn, table, structures).unwrap_or(PrimaryKeyInfo {
                primary_key_index: None,
                last_primary_key: None,
            })
        })
        .collect::<Vec<PrimaryKeyInfo>>();

    debug!("table primary key info: {:?}", table_primary_key_info);

    for db_file in src_files.iter().skip(1) {
        let src_conn = open_connection(db_file);
        for (index, (table_name, structures)) in table_structure_zip.iter().enumerate() {
            let sql = format!("SELECT COUNT(*) FROM {}", table_name);
            let mut stmt = src_conn.prepare(&sql)?;
            let row_count = stmt
                .query_row([], |row| row.get::<_, usize>(0))
                .unwrap_or_default();
            let primary_key_info = table_primary_key_info.get_mut(index).unwrap();
            let iter_count = if row_count < batch_size {
                1
            } else {
                row_count / batch_size + 1
            };
            for i in 0..iter_count {
                let sql = format!("SELECT * FROM {} LIMIT (?1) OFFSET (?2)", table_name);
                let mut stmt = src_conn.prepare(&sql)?;
                let mut rows = stmt.query(params![batch_size, i * batch_size])?;
                match insert_batch_data(
                    &dst_conn,
                    &mut rows,
                    table_name,
                    primary_key_info,
                    structures,
                ) {
                    Ok(_) => {
                        info!(
                            "Success!!! insert table: {}, batch size: {}, offset: {}",
                            table_name,
                            batch_size,
                            i * batch_size
                        );
                    }
                    Err(err) => {
                        error!(
                            "Failure!!! insert table: {}, batch size: {}, offset: {}, err: {}",
                            table_name,
                            batch_size,
                            i * batch_size,
                            err
                        );
                    }
                };
            }
        }
    }
    Ok(())
}

fn insert_batch_data(
    dst_conn: &Connection,
    rows: &mut Rows,
    table: &str,
    primary_key_info: &mut PrimaryKeyInfo,
    structures: &[TablePragma],
) -> anyhow::Result<()> {
    match (
        primary_key_info.primary_key_index,
        primary_key_info.last_primary_key,
    ) {
        (Some(_), Some(_)) => {
            case_when_primary_key_is_integer(dst_conn, rows, table, primary_key_info, structures)?
        }
        (Some(_), None) => case_when_primary_key_is_not_integer(
            dst_conn,
            rows,
            table,
            primary_key_info,
            structures,
        )?,
        (None, _) => {
            case_when_without_primary_key(dst_conn, rows, table, primary_key_info, structures)?
        }
    }
    Ok(())
}

fn case_when_primary_key_is_integer(
    dst_conn: &Connection,
    rows: &mut Rows,
    table: &str,
    primary_key_info: &mut PrimaryKeyInfo,
    structures: &[TablePragma],
) -> anyhow::Result<()> {
    let mut sql = format!("INSERT INTO {} VALUES ", table);
    let mut col_data = Vec::new();
    let mut count = 1;
    let primary_key_index = unsafe { primary_key_info.primary_key_index.unwrap_unchecked() };
    let last_primary_key = unsafe {
        primary_key_info
            .last_primary_key
            .as_mut()
            .unwrap_unchecked()
    };
    while let Some(row) = rows.next()? {
        sql.push('(');
        for (i, _pragma) in structures.iter().enumerate() {
            if i == primary_key_index as usize {
                *last_primary_key += 1;
                col_data.push(Value::Integer(*last_primary_key));
            } else {
                col_data.push(row.get::<_, Value>(i).unwrap_or(Value::Null));
            }
            sql.push_str(&format!("?{},", count));
            count += 1;
        }
        sql.pop();
        sql.push_str("),");
    }
    sql.pop();
    let insert_data: Vec<&dyn ToSql> = col_data.iter().map(|item| item as &dyn ToSql).collect();
    dst_conn.execute(&sql, &insert_data[..])?;
    Ok(())
}

fn case_when_primary_key_is_not_integer(
    dst_conn: &Connection,
    rows: &mut Rows,
    table: &str,
    _primary_key_info: &mut PrimaryKeyInfo,
    structures: &[TablePragma],
) -> anyhow::Result<()> {
    let mut sql = format!("INSERT INTO {} VALUES ", table);
    let mut col_data = Vec::new();
    let mut count = 1;
    while let Some(row) = rows.next()? {
        sql.push('(');
        for (i, _pragma) in structures.iter().enumerate() {
            col_data.push(row.get::<_, Value>(i).unwrap_or(Value::Null));
            sql.push_str(&format!("?{},", count));
            count += 1;
        }
        sql.pop();
        sql.push_str("),");
    }
    sql.pop();
    let insert_data: Vec<&dyn ToSql> = col_data.iter().map(|item| item as &dyn ToSql).collect();
    dst_conn.execute(&sql, &insert_data[..])?;
    Ok(())
}

fn case_when_without_primary_key(
    dst_conn: &Connection,
    rows: &mut Rows,
    table: &str,
    _primary_key_info: &mut PrimaryKeyInfo,
    structures: &[TablePragma],
) -> anyhow::Result<()> {
    let mut sql = format!("INSERT INTO {} VALUES ", table);
    let mut col_data = Vec::new();
    let mut count = 1;
    while let Some(row) = rows.next()? {
        sql.push('(');
        for (i, _pragma) in structures.iter().enumerate() {
            col_data.push(row.get::<_, Value>(i).unwrap_or(Value::Null));
            sql.push_str(&format!("?{},", count));
            count += 1;
        }
        sql.pop();
        sql.push_str("),");
    }
    sql.pop();
    let insert_data: Vec<&dyn ToSql> = col_data.iter().map(|item| item as &dyn ToSql).collect();
    dst_conn.execute(&sql, &insert_data[..])?;
    Ok(())
}
