/// Simulation errors.
#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error("Newton-Raphson iteration limit exceeded ({0} iterations)")]
    IterationLimit(usize),

    #[error("singular matrix at equation {0}")]
    SingularMatrix(usize),

    #[error("DC operating point failed to converge")]
    NoConvergence,

    #[error("timestep too small ({0:.3e}s < minimum {1:.3e}s)")]
    TimestepTooSmall(f64, f64),

    #[error("device not found: {0}")]
    DeviceNotFound(String),

    #[error("{0}")]
    Other(String),
}
