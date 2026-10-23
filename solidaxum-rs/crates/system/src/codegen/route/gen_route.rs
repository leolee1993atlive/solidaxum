use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::{app::AppState, codegen::handler::gen_handler::*};

pub fn gen_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/tool/gen/list", get(gen_list))
        .route("/tool/gen/db/list", get(db_list))
        .route("/tool/gen/column/{tableId}", get(column_list))
        .route("/tool/gen/importTable", post(import_table))
        .route("/tool/gen/preview/{tableId}", get(preview))
        .route("/tool/gen/download/{tableId}", get(download))
        .route("/tool/gen/batchGenCode", get(batch_gen_code))
        .route("/tool/gen/synchDb/{tableId}", get(synch_db))
        .route("/tool/gen/getDataNames", get(get_data_names))
        .route("/tool/gen/{tableId}", get(get_gen_table).delete(remove))
        .route("/tool/gen", put(edit_save))
}
