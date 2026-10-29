//! Parquet import placeholder.
//!
//! Real Parquet support will use the `parquet` + `arrow` crates once the
//! environment can fetch crates.io. Until then we surface a clear error so the
//! Open dialog can accept `.parquet` without panicking.

use std::path::Path;

use crate::model::Dataset;

pub fn load_parquet_path(path: &Path) -> anyhow::Result<Dataset> {
    Err(anyhow::anyhow!(
        "Parquet support is stubbed (need parquet/arrow crates). Path: {}",
        path.display()
    ))
}
