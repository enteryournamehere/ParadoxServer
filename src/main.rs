use std::{convert::TryFrom, fs::File, path::PathBuf};

use assembly_data::fdb::{
    common::{Latin1Str, Latin1String, ValueType},
    file::FDBHeader,
    mem, query,
    ro::{ArcHandle, Handle},
};
use color_eyre::eyre::WrapErr;
use linked_hash_map::LinkedHashMap;
use mapr::Mmap;
use structopt::StructOpt;
use warp::{reply::Json, Filter};

#[derive(StructOpt)]
/// Starts the server that serves a JSON API to the client files
struct Options {
    /// The cdclient file
    file: PathBuf,
    /// The port for the server
    #[structopt(long, default_value = "3030")]
    port: u16,
    /// Bind to `0.0.0.0` instead of `127.0.0.1`
    #[structopt(long)]
    public: bool,
}

fn table_index(db_table: Handle<'_, FDBHeader>, lname: &Latin1Str, key: String) -> Json {
    let table = db_table.into_table_by_name(lname).unwrap();

    if let Some(table) = table.transpose() {
        let table_def = table.into_definition().unwrap();
        let table_data = table.into_data().unwrap();

        let mut cols = table_def.column_header_list().unwrap();
        let index_col = cols.next().unwrap();
        let index_type = ValueType::try_from(index_col.raw().column_data_type).unwrap();
        let index_name = index_col.column_name().unwrap().raw().decode();

        if let Ok(pk_filter) = query::pk_filter(key, index_type) {
            let bucket_index = pk_filter.hash() % table_data.raw().buckets.count;
            let mut buckets = table_data.bucket_header_list().unwrap();
            let bucket = buckets.nth(bucket_index as usize).unwrap();

            let mut rows = Vec::new();
            for row_header in bucket.row_header_iter() {
                let row_header = row_header.unwrap();

                let mut field_iter = row_header.field_data_list().unwrap();
                let index_field = field_iter.next().unwrap();
                let index_value = index_field.try_get_value().unwrap();
                let index_mem = mem::Field::try_from(index_value).unwrap();

                if !pk_filter.filter(&index_mem) {
                    continue;
                }

                let mut row = LinkedHashMap::new();
                row.insert(index_name.clone(), index_mem);
                // add the remaining fields
                #[allow(clippy::clone_on_copy)]
                for col in cols.clone() {
                    let col_name = col.column_name().unwrap().raw().decode();
                    let field = field_iter.next().unwrap();
                    let value = field.try_get_value().unwrap();
                    let mem_val = mem::Field::try_from(value).unwrap();
                    row.insert(col_name, mem_val);
                }
                rows.push(row);
            }

            return warp::reply::json(&rows);
        }
    }
    warp::reply::json(&())
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    pretty_env_logger::init();

    color_eyre::install()?;
    let opts = Options::from_args();

    // Load the database file
    let file = File::open(&opts.file)
        .wrap_err_with(|| format!("Failed to open input file '{}'", opts.file.display()))?;
    let hnd = ArcHandle::new_arc(unsafe { Mmap::map(&file)? });
    let hnd = hnd.into_tables()?;
    let hnd_state = warp::any().map(move || hnd.clone());

    let api = warp::path("api").and(warp::path("v0"));
    let tables = api.and(warp::path("tables")).and(hnd_state);
    let table = tables.clone().and(warp::path::param());

    // The `/tables` endpoint
    let tables = tables
        .and(warp::path::end())
        .map(|db: ArcHandle<_, FDBHeader>| {
            let db = db.as_bytes_handle();

            let mut list = Vec::with_capacity(db.raw().tables.count as usize);
            let header_list = db.table_header_list().unwrap();
            for tbl in header_list {
                let def = tbl.into_definition().unwrap();
                let name = *def.table_name().unwrap().raw();
                list.push(name);
            }
            warp::reply::json(&list)
        });

    // The `/tables/:name/def` endpoint
    let table_def = table.clone().and(warp::path("def")).map(
        move |db_table: ArcHandle<_, FDBHeader>, name: String| {
            let lname = Latin1String::encode(&name);
            let db_table = db_table.as_bytes_handle();
            let table = db_table.into_table_by_name(lname.as_ref()).unwrap();

            if let Some(table) = table.transpose() {
                let table_def = table.into_definition().unwrap();
                return warp::reply::json(&table_def);
            }
            warp::reply::json(&())
        },
    );

    // The `/tables/:name/:key` endpoint
    let table_get = table.clone().and(warp::path::param()).map(
        move |db_table: ArcHandle<_, FDBHeader>, name: String, key: String| {
            let lname = Latin1String::encode(&name);
            let db_table = db_table.as_bytes_handle();

            table_index(db_table, lname.as_ref(), key)
        },
    );

    /*// The `/tables/:name/content` endpoint
    let table_content = table.and(warp::path("content")).map(|_, _: String| {
        let our_ids = vec![1, 3, 7, 13];
        warp::reply::json(&our_ids)
    });*/

    let routes = warp::get().and(
        tables
            .or(table_def) /*.or(table_content)*/
            .or(table_get),
    );

    let ip = if opts.public {
        [0, 0, 0, 0]
    } else {
        [127, 0, 0, 1]
    };
    warp::serve(routes).run((ip, opts.port)).await;

    Ok(())
}
