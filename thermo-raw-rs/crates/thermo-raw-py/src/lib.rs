use numpy::{IntoPyArray, PyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use ::thermo_raw::{MsLevel, RawFile as InnerRawFile};

#[pyclass]
struct RawFile {
    inner: InnerRawFile,
}

#[pymethods]
impl RawFile {
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let inner =
            InnerRawFile::open(path).map_err(|e| PyValueError::new_err(format!("{}", e)))?;
        Ok(Self { inner })
    }

    #[getter]
    fn n_scans(&self) -> u32 {
        self.inner.n_scans()
    }

    #[getter]
    fn first_scan(&self) -> u32 {
        self.inner.first_scan()
    }

    #[getter]
    fn last_scan(&self) -> u32 {
        self.inner.last_scan()
    }

    #[getter]
    fn start_time(&self) -> f64 {
        self.inner.start_time()
    }

    #[getter]
    fn end_time(&self) -> f64 {
        self.inner.end_time()
    }

    #[getter]
    fn instrument_model(&self) -> String {
        self.inner.metadata().instrument_model.clone()
    }

    #[getter]
    fn sample_name(&self) -> String {
        self.inner.metadata().sample_name.clone()
    }

    /// Return (mz_array, intensity_array) as numpy arrays.
    fn scan<'py>(
        &self,
        py: Python<'py>,
        scan_number: u32,
    ) -> PyResult<(Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>)> {
        let scan = self
            .inner
            .scan(scan_number)
            .map_err(|e| PyValueError::new_err(format!("{}", e)))?;
        let mz = scan.centroid_mz.into_pyarray(py);
        let intensity = scan.centroid_intensity.into_pyarray(py);
        Ok((mz, intensity))
    }

    /// TIC: return (rt_array, intensity_array) as numpy arrays.
    fn tic<'py>(
        &self,
        py: Python<'py>,
    ) -> (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>) {
        let chrom = self.inner.tic();
        let rt = chrom.rt.into_pyarray(py);
        let intensity = chrom.intensity.into_pyarray(py);
        (rt, intensity)
    }

    /// XIC: return (rt_array, intensity_array) as numpy arrays.
    #[pyo3(signature = (mz, ppm=None))]
    fn xic<'py>(
        &self,
        py: Python<'py>,
        mz: f64,
        ppm: Option<f64>,
    ) -> PyResult<(Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>)> {
        let ppm = ppm.unwrap_or(5.0);
        let chrom = self
            .inner
            .xic(mz, ppm)
            .map_err(|e| PyValueError::new_err(format!("{}", e)))?;
        let rt = chrom.rt.into_pyarray(py);
        let intensity = chrom.intensity.into_pyarray(py);
        Ok((rt, intensity))
    }

    /// Read all MS1 scans in parallel, return list of (mz, intensity) tuples.
    fn all_ms1_scans<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>)>> {
        let first = self.inner.first_scan();
        let last = self.inner.last_scan();
        let scans = self
            .inner
            .scans_parallel(first..last + 1)
            .map_err(|e| PyValueError::new_err(format!("{}", e)))?;
        let results: Vec<_> = scans
            .into_iter()
            .filter(|s| matches!(s.ms_level, MsLevel::Ms1))
            .map(|s| {
                let mz = s.centroid_mz.into_pyarray(py);
                let int = s.centroid_intensity.into_pyarray(py);
                (mz, int)
            })
            .collect();
        Ok(results)
    }
}

#[pymodule]
fn thermo_raw(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RawFile>()?;
    Ok(())
}
