use crate::config::EnvironmentVariables;
use crate::graph::CsrGraphMmap;
use crate::mmap_structs::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub title_to_dense_id: Arc<TitleToDenseIdMmap>,
    pub dense_id_to_title: Arc<DenseIdToTitleMmap>,
    pub orig_to_dense_id: Arc<OrigToDenseIdMmap>,
    pub dense_id_to_orig: Arc<DenseIdToOrigMmap>,
    pub redirects_passed: Arc<RedirectsPassedMmap>,
    pub redirect_targets_dense: Arc<RedirectTargetsDenseMmap>,
    pub csr_graph: Arc<CsrGraphMmap>,
    pub redis_pool: deadpool_redis::Pool,
    pub sqlite_pool: sqlx::SqlitePool,
    pub env: EnvironmentVariables,
}
