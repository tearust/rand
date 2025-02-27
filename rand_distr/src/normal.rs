// Copyright 2018 Developers of the Rand project.
// Copyright 2013 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The normal and derived distributions.

use crate::utils::ziggurat;
use num_traits::Float;
use crate::{ziggurat_tables, Distribution, Open01};
use rand::Rng;
use core::fmt;

/// Samples floating-point numbers according to the normal distribution
/// `N(0, 1)` (a.k.a. a standard normal, or Gaussian). This is equivalent to
/// `Normal::new(0.0, 1.0)` but faster.
///
/// See `Normal` for the general normal distribution.
///
/// Implemented via the ZIGNOR variant[^1] of the Ziggurat method.
///
/// [^1]: Jurgen A. Doornik (2005). [*An Improved Ziggurat Method to
///       Generate Normal Random Samples*](
///       https://www.doornik.com/research/ziggurat.pdf).
///       Nuffield College, Oxford
///
/// # Example
/// ```
/// use rand::prelude::*;
/// use rand_distr::StandardNormal;
///
/// let val: f64 = thread_rng().sample(StandardNormal);
/// println!("{}", val);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct StandardNormal;

impl Distribution<f32> for StandardNormal {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f32 {
        // TODO: use optimal 32-bit implementation
        let x: f64 = self.sample(rng);
        x as f32
    }
}

impl Distribution<f64> for StandardNormal {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        #[inline]
        fn pdf(x: f64) -> f64 {
            (-x * x / 2.0).exp()
        }
        #[inline]
        fn zero_case<R: Rng + ?Sized>(rng: &mut R, u: f64) -> f64 {
            // compute a random number in the tail by hand

            // strange initial conditions, because the loop is not
            // do-while, so the condition should be true on the first
            // run, they get overwritten anyway (0 < 1, so these are
            // good).
            let mut x = 1.0f64;
            let mut y = 0.0f64;

            while -2.0 * y < x * x {
                let x_: f64 = rng.sample(Open01);
                let y_: f64 = rng.sample(Open01);

                x = x_.ln() / ziggurat_tables::ZIG_NORM_R;
                y = y_.ln();
            }

            if u < 0.0 {
                x - ziggurat_tables::ZIG_NORM_R
            } else {
                ziggurat_tables::ZIG_NORM_R - x
            }
        }

        ziggurat(
            rng,
            true, // this is symmetric
            &ziggurat_tables::ZIG_NORM_X,
            &ziggurat_tables::ZIG_NORM_F,
            pdf,
            zero_case,
        )
    }
}

/// The normal distribution `N(mean, std_dev**2)`.
///
/// This uses the ZIGNOR variant of the Ziggurat method, see [`StandardNormal`]
/// for more details.
///
/// Note that [`StandardNormal`] is an optimised implementation for mean 0, and
/// standard deviation 1.
///
/// # Example
///
/// ```
/// use rand_distr::{Normal, Distribution};
///
/// // mean 2, standard deviation 3
/// let normal = Normal::new(2.0, 3.0).unwrap();
/// let v = normal.sample(&mut rand::thread_rng());
/// println!("{} is from a N(2, 9) distribution", v)
/// ```
///
/// [`StandardNormal`]: crate::StandardNormal
#[derive(Clone, Copy, Debug)]
pub struct Normal<F>
where F: Float, StandardNormal: Distribution<F>
{
    mean: F,
    std_dev: F,
}

/// Error type returned from `Normal::new` and `LogNormal::new`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The mean value is too small (log-normal samples must be positive)
    MeanTooSmall,
    /// The standard deviation or other dispersion parameter is not finite.
    BadVariance,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::MeanTooSmall => "mean < 0 or NaN in log-normal distribution",
            Error::BadVariance => "variation parameter is non-finite in (log)normal distribution",
        })
    }
}

#[cfg(feature = "std")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "std")))]
impl std::error::Error for Error {}

impl<F> Normal<F>
where F: Float, StandardNormal: Distribution<F>
{
    /// Construct, from mean and standard deviation
    ///
    /// Parameters:
    ///
    /// -   mean (`μ`, unrestricted)
    /// -   standard deviation (`σ`, must be finite)
    #[inline]
    pub fn new(mean: F, std_dev: F) -> Result<Normal<F>, Error> {
        if !std_dev.is_finite() {
            return Err(Error::BadVariance);
        }
        Ok(Normal { mean, std_dev })
    }

    /// Construct, from mean and coefficient of variation
    ///
    /// Parameters:
    ///
    /// -   mean (`μ`, unrestricted)
    /// -   coefficient of variation (`cv = abs(σ / μ)`)
    #[inline]
    pub fn from_mean_cv(mean: F, cv: F) -> Result<Normal<F>, Error> {
        if !cv.is_finite() || cv < F::zero() {
            return Err(Error::BadVariance);
        }
        let std_dev = cv * mean;
        Ok(Normal { mean, std_dev })
    }

    /// Sample from a z-score
    ///
    /// This may be useful for generating correlated samples `x1` and `x2`
    /// from two different distributions, as follows.
    /// ```
    /// # use rand::prelude::*;
    /// # use rand_distr::{Normal, StandardNormal};
    /// let mut rng = thread_rng();
    /// let z = StandardNormal.sample(&mut rng);
    /// let x1 = Normal::new(0.0, 1.0).unwrap().from_zscore(z);
    /// let x2 = Normal::new(2.0, -3.0).unwrap().from_zscore(z);
    /// ```
    #[inline]
    pub fn from_zscore(&self, zscore: F) -> F {
        self.mean + self.std_dev * zscore
    }
}

impl<F> Distribution<F> for Normal<F>
where F: Float, StandardNormal: Distribution<F>
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> F {
        self.from_zscore(rng.sample(StandardNormal))
    }
}


/// The log-normal distribution `ln N(mean, std_dev**2)`.
///
/// If `X` is log-normal distributed, then `ln(X)` is `N(mean, std_dev**2)`
/// distributed.
///
/// # Example
///
/// ```
/// use rand_distr::{LogNormal, Distribution};
///
/// // mean 2, standard deviation 3
/// let log_normal = LogNormal::new(2.0, 3.0).unwrap();
/// let v = log_normal.sample(&mut rand::thread_rng());
/// println!("{} is from an ln N(2, 9) distribution", v)
/// ```
#[derive(Clone, Copy, Debug)]
pub struct LogNormal<F>
where F: Float, StandardNormal: Distribution<F>
{
    norm: Normal<F>,
}

impl<F> LogNormal<F>
where F: Float, StandardNormal: Distribution<F>
{
    /// Construct, from (log-space) mean and standard deviation
    ///
    /// Parameters are the "standard" log-space measures (these are the mean
    /// and standard deviation of the logarithm of samples):
    ///
    /// -   `mu` (`μ`, unrestricted) is the mean of the underlying distribution
    /// -   `sigma` (`σ`, must be finite) is the standard deviation of the
    ///     underlying Normal distribution
    #[inline]
    pub fn new(mu: F, sigma: F) -> Result<LogNormal<F>, Error> {
        let norm = Normal::new(mu, sigma)?;
        Ok(LogNormal { norm })
    }

    /// Construct, from (linear-space) mean and coefficient of variation
    ///
    /// Parameters are linear-space measures:
    ///
    /// -   mean (`μ > 0`) is the (real) mean of the distribution
    /// -   coefficient of variation (`cv = σ / μ`, requiring `cv ≥ 0`) is a
    ///     standardized measure of dispersion
    ///
    /// As a special exception, `μ = 0, cv = 0` is allowed (samples are `-inf`).
    #[inline]
    pub fn from_mean_cv(mean: F, cv: F) -> Result<LogNormal<F>, Error> {
        if cv == F::zero() {
            let mu = mean.ln();
            let norm = Normal::new(mu, F::zero()).unwrap();
            return Ok(LogNormal { norm });
        }
        if !(mean > F::zero()) {
            return Err(Error::MeanTooSmall);
        }
        if !(cv >= F::zero()) {
            return Err(Error::BadVariance);
        }

        // Using X ~ lognormal(μ, σ), CV² = Var(X) / E(X)²
        // E(X) = exp(μ + σ² / 2) = exp(μ) × exp(σ² / 2)
        // Var(X) = exp(2μ + σ²)(exp(σ²) - 1) = E(X)² × (exp(σ²) - 1)
        // but Var(X) = (CV × E(X))² so CV² = exp(σ²) - 1
        // thus σ² = log(CV² + 1)
        // and exp(μ) = E(X) / exp(σ² / 2) = E(X) / sqrt(CV² + 1)
        let a = F::one() + cv * cv; // e
        let mu = F::from(0.5).unwrap() * (mean * mean / a).ln();
        let sigma = a.ln().sqrt();
        let norm = Normal::new(mu, sigma)?;
        Ok(LogNormal { norm })
    }

    /// Sample from a z-score
    ///
    /// This may be useful for generating correlated samples `x1` and `x2`
    /// from two different distributions, as follows.
    /// ```
    /// # use rand::prelude::*;
    /// # use rand_distr::{LogNormal, StandardNormal};
    /// let mut rng = thread_rng();
    /// let z = StandardNormal.sample(&mut rng);
    /// let x1 = LogNormal::from_mean_cv(3.0, 1.0).unwrap().from_zscore(z);
    /// let x2 = LogNormal::from_mean_cv(2.0, 4.0).unwrap().from_zscore(z);
    /// ```
    #[inline]
    pub fn from_zscore(&self, zscore: F) -> F {
        self.norm.from_zscore(zscore).exp()
    }
}

impl<F> Distribution<F> for LogNormal<F>
where F: Float, StandardNormal: Distribution<F>
{
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> F {
        self.norm.sample(rng).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal() {
        let norm = Normal::new(10.0, 10.0).unwrap();
        let mut rng = crate::test::rng(210);
        for _ in 0..1000 {
            norm.sample(&mut rng);
        }
    }
    #[test]
    fn test_normal_cv() {
        let norm = Normal::from_mean_cv(1024.0, 1.0 / 256.0).unwrap();
        assert_eq!((norm.mean, norm.std_dev), (1024.0, 4.0));
    }
    #[test]
    fn test_normal_invalid_sd() {
        assert!(Normal::from_mean_cv(10.0, -1.0).is_err());
    }

    #[test]
    fn test_log_normal() {
        let lnorm = LogNormal::new(10.0, 10.0).unwrap();
        let mut rng = crate::test::rng(211);
        for _ in 0..1000 {
            lnorm.sample(&mut rng);
        }
    }
    #[test]
    fn test_log_normal_cv() {
        let lnorm = LogNormal::from_mean_cv(0.0, 0.0).unwrap();
        assert_eq!((lnorm.norm.mean, lnorm.norm.std_dev), (-core::f64::INFINITY, 0.0));

        let lnorm = LogNormal::from_mean_cv(1.0, 0.0).unwrap();
        assert_eq!((lnorm.norm.mean, lnorm.norm.std_dev), (0.0, 0.0));

        let e = core::f64::consts::E;
        let lnorm = LogNormal::from_mean_cv(e.sqrt(), (e - 1.0).sqrt()).unwrap();
        assert_almost_eq!(lnorm.norm.mean, 0.0, 2e-16);
        assert_almost_eq!(lnorm.norm.std_dev, 1.0, 2e-16);

        let lnorm = LogNormal::from_mean_cv(e.powf(1.5), (e - 1.0).sqrt()).unwrap();
        assert_almost_eq!(lnorm.norm.mean, 1.0, 1e-15);
        assert_eq!(lnorm.norm.std_dev, 1.0);
    }
    #[test]
    fn test_log_normal_invalid_sd() {
        assert!(LogNormal::from_mean_cv(-1.0, 1.0).is_err());
        assert!(LogNormal::from_mean_cv(0.0, 1.0).is_err());
        assert!(LogNormal::from_mean_cv(1.0, -1.0).is_err());
    }
}
