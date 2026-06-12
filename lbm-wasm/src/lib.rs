//! 2D Lattice Boltzmann Method with Central Moments
//! 格子ボルツマン法による2次元流体計算

use wasm_bindgen::prelude::*;

// D2Q9 lattice velocities
const EX: [i32; 9] = [0, 1, 0, -1, 0, 1, -1, -1, 1];
const EY: [i32; 9] = [0, 0, 1, 0, -1, 1, 1, -1, -1];
const W: [f64; 9] = [4.0/9.0, 1.0/9.0, 1.0/9.0, 1.0/9.0, 1.0/9.0, 1.0/36.0, 1.0/36.0, 1.0/36.0, 1.0/36.0];

// Opposite direction indices for bounce-back
const OPP: [usize; 9] = [0, 3, 4, 1, 2, 7, 8, 5, 6];

// Simulation constants
const U_INF: f64 = 0.1;  // Mach 0.1
const CS2: f64 = 1.0 / 3.0;  // Speed of sound squared

// Grid dimensions
const NX_FINE: usize = 179;  // Fine grid x cells (shown region)
const NY_FINE: usize = 89;   // Fine grid y cells (shown region)
const DX_FINE: f64 = 8.0 / (NX_FINE as f64 - 1.0);  // Physical spacing

// Coarse grid has 2x spacing
const DX_COARSE: f64 = DX_FINE * 2.0;

// Physical domain for computation: -4 <= x <= 16, -8 <= y <= 12
// Shown region: 0 <= x <= 8, 0 <= y <= 4
const X_MIN: f64 = -4.0;
const X_MAX: f64 = 16.0;
const Y_MIN: f64 = -8.0;
const Y_MAX: f64 = 12.0;

// Coarse grid dimensions
const NX_COARSE: usize = ((X_MAX - X_MIN) / DX_COARSE) as usize + 1;
const NY_COARSE: usize = ((Y_MAX - Y_MIN) / DX_COARSE) as usize + 1;

// Fine grid offset in coarse coordinates
const FINE_X_OFFSET: usize = ((0.0 - X_MIN) / DX_COARSE) as usize;
const FINE_Y_OFFSET: usize = ((0.0 - Y_MIN) / DX_COARSE) as usize;

// Standalone index functions to avoid borrow checker issues
#[inline(always)]
fn idx_fine(x: usize, y: usize, k: usize) -> usize {
    (y * NX_FINE + x) * 9 + k
}

#[inline(always)]
fn idx_coarse(x: usize, y: usize, k: usize) -> usize {
    (y * NX_COARSE + x) * 9 + k
}

#[inline(always)]
fn idx_2d(x: usize, y: usize) -> usize {
    y * NX_FINE + x
}

/// LBM Simulation state
#[wasm_bindgen]
pub struct LBMSimulation {
    // Fine grid distribution functions (shown region + ghost cells)
    f_fine: Vec<f64>,
    f_fine_tmp: Vec<f64>,

    // Coarse grid distribution functions (outer region)
    f_coarse: Vec<f64>,
    f_coarse_tmp: Vec<f64>,
    f_coarse_history: Vec<Vec<f64>>,  // For temporal interpolation

    // Macroscopic quantities (fine grid for display)
    rho: Vec<f64>,
    ux: Vec<f64>,
    uy: Vec<f64>,

    // Obstacle map (fine grid)
    obstacle: Vec<bool>,

    // Visualization buffer
    image_data: Vec<u8>,

    // LIC noise texture
    noise: Vec<f64>,

    // Simulation parameters
    nu: f64,           // Kinematic viscosity
    omega: f64,        // BGK relaxation parameter
    omega_coarse: f64, // Relaxation for coarse grid
    reynolds: f64,

    // Physical time
    time: f64,

    // Display settings
    display_mode: u32,     // 0: velocity, 1: vorticity, 2: pressure, 3: streamlines, 4: Bernoulli
    colormap_scale: f64,
    colormap_origin: f64,

    // Running state
    paused: bool,
}

impl LBMSimulation {
    /// Compute equilibrium distribution
    fn feq(rho: f64, ux: f64, uy: f64, k: usize) -> f64 {
        let eu = (EX[k] as f64) * ux + (EY[k] as f64) * uy;
        let u2 = ux * ux + uy * uy;
        W[k] * rho * (1.0 + eu / CS2 + eu * eu / (2.0 * CS2 * CS2) - u2 / (2.0 * CS2))
    }

    /// Compute macroscopic quantities from distribution
    fn compute_macro(f: &[f64], base: usize) -> (f64, f64, f64) {
        let mut rho = 0.0;
        let mut ux = 0.0;
        let mut uy = 0.0;
        for k in 0..9 {
            rho += f[base + k];
            ux += (EX[k] as f64) * f[base + k];
            uy += (EY[k] as f64) * f[base + k];
        }
        if rho > 1e-10 {
            ux /= rho;
            uy /= rho;
        }
        (rho, ux, uy)
    }

    /// Central moment collision operator (cascaded LBM)
    /// Currently unused: needs more work before replacing BGK in the hot loop
    #[allow(dead_code)]
    fn collision_central_moment(&mut self, f: &mut [f64], base: usize, omega: f64) {
        // Get macroscopic values
        let (rho, ux, uy) = Self::compute_macro(f, base);

        if rho < 1e-10 {
            return;
        }

        // Transform to central moments
        // k00 = rho
        // k10 = 0, k01 = 0 (by definition of central moments)
        // k20 = sum(fi * (exi - ux)^2)
        // k02 = sum(fi * (eyi - uy)^2)
        // k11 = sum(fi * (exi - ux) * (eyi - uy))
        // k21 = sum(fi * (exi - ux)^2 * (eyi - uy))
        // k12 = sum(fi * (exi - ux) * (eyi - uy)^2)
        // k22 = sum(fi * (exi - ux)^2 * (eyi - uy)^2)

        let mut k20 = 0.0;
        let mut k02 = 0.0;
        let mut k11 = 0.0;
        let mut k21 = 0.0;
        let mut k12 = 0.0;
        let mut k22 = 0.0;

        for i in 0..9 {
            let cx = (EX[i] as f64) - ux;
            let cy = (EY[i] as f64) - uy;
            let cx2 = cx * cx;
            let cy2 = cy * cy;
            let fi = f[base + i];

            k20 += fi * cx2;
            k02 += fi * cy2;
            k11 += fi * cx * cy;
            k21 += fi * cx2 * cy;
            k12 += fi * cx * cy2;
            k22 += fi * cx2 * cy2;
        }

        // Equilibrium central moments
        let k20_eq = rho * CS2;
        let k02_eq = rho * CS2;
        let k11_eq = 0.0;
        let k21_eq = 0.0;
        let k12_eq = 0.0;
        let k22_eq = rho * CS2 * CS2;

        // Relaxation (different rates for different moments)
        let omega_3 = 1.0;  // Higher order moments relax faster
        let omega_4 = 1.0;

        k20 = (1.0 - omega) * k20 + omega * k20_eq;
        k02 = (1.0 - omega) * k02 + omega * k02_eq;
        k11 = (1.0 - omega) * k11 + omega * k11_eq;
        k21 = (1.0 - omega_3) * k21 + omega_3 * k21_eq;
        k12 = (1.0 - omega_3) * k12 + omega_3 * k12_eq;
        k22 = (1.0 - omega_4) * k22 + omega_4 * k22_eq;

        // Transform back to distribution functions
        // Using the inverse transformation matrix for D2Q9
        let trace = k20 + k02;
        let diff = k20 - k02;

        f[base + 0] = rho - trace - k22;

        f[base + 1] = rho / 9.0 + (trace + 3.0 * diff) / 18.0 + (ux * rho) / 3.0
                    - k11 / 4.0 + k21 / 4.0 - k12 / 4.0 + k22 / 4.0;
        f[base + 3] = rho / 9.0 + (trace + 3.0 * diff) / 18.0 - (ux * rho) / 3.0
                    - k11 / 4.0 - k21 / 4.0 + k12 / 4.0 + k22 / 4.0;
        f[base + 2] = rho / 9.0 + (trace - 3.0 * diff) / 18.0 + (uy * rho) / 3.0
                    - k11 / 4.0 - k21 / 4.0 - k12 / 4.0 + k22 / 4.0;
        f[base + 4] = rho / 9.0 + (trace - 3.0 * diff) / 18.0 - (uy * rho) / 3.0
                    - k11 / 4.0 + k21 / 4.0 + k12 / 4.0 + k22 / 4.0;

        f[base + 5] = rho / 36.0 + trace / 36.0 + (ux + uy) * rho / 12.0
                    + k11 / 4.0 + k21 / 4.0 + k12 / 4.0 + k22 / 4.0;
        f[base + 6] = rho / 36.0 + trace / 36.0 + (-ux + uy) * rho / 12.0
                    - k11 / 4.0 - k21 / 4.0 + k12 / 4.0 + k22 / 4.0;
        f[base + 7] = rho / 36.0 + trace / 36.0 + (-ux - uy) * rho / 12.0
                    + k11 / 4.0 - k21 / 4.0 - k12 / 4.0 + k22 / 4.0;
        f[base + 8] = rho / 36.0 + trace / 36.0 + (ux - uy) * rho / 12.0
                    - k11 / 4.0 + k21 / 4.0 - k12 / 4.0 + k22 / 4.0;

        // Ensure positivity and normalization
        let mut sum = 0.0;
        for i in 0..9 {
            if f[base + i] < 0.0 {
                f[base + i] = 1e-10;
            }
            sum += f[base + i];
        }
        if (sum - rho).abs() > 1e-6 {
            for i in 0..9 {
                f[base + i] *= rho / sum;
            }
        }
    }

    /// BGK collision with Smagorinsky subgrid model for stability at high Re
    fn collision_bgk(f: &mut [f64], base: usize, omega: f64) {
        let (rho, ux, uy) = Self::compute_macro(f, base);

        let mut feq = [0.0; 9];
        for k in 0..9 {
            feq[k] = Self::feq(rho, ux, uy, k);
        }

        // Non-equilibrium momentum flux tensor
        let mut pi_xx = 0.0;
        let mut pi_yy = 0.0;
        let mut pi_xy = 0.0;
        for k in 0..9 {
            let fneq = f[base + k] - feq[k];
            let ex = EX[k] as f64;
            let ey = EY[k] as f64;
            pi_xx += ex * ex * fneq;
            pi_yy += ey * ey * fneq;
            pi_xy += ex * ey * fneq;
        }
        let q = (pi_xx * pi_xx + pi_yy * pi_yy + 2.0 * pi_xy * pi_xy).sqrt();

        // Effective relaxation time with eddy viscosity (Hou et al.)
        const C_SMAG: f64 = 0.16;
        let tau0 = 1.0 / omega;
        let tau_eff = 0.5 * (tau0 + (tau0 * tau0
            + 18.0 * std::f64::consts::SQRT_2 * C_SMAG * C_SMAG * q / rho.max(1e-10)).sqrt());
        let omega_eff = 1.0 / tau_eff;

        for k in 0..9 {
            f[base + k] = (1.0 - omega_eff) * f[base + k] + omega_eff * feq[k];
        }
    }

    /// Streaming step for fine grid
    fn stream_fine(&mut self) {
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);
                if self.obstacle[idx] {
                    continue;
                }

                for k in 0..9 {
                    let xp = (x as i32 - EX[k]) as usize;
                    let yp = (y as i32 - EY[k]) as usize;

                    if xp < NX_FINE && yp < NY_FINE {
                        let src_idx = idx_2d(xp, yp);
                        if self.obstacle[src_idx] {
                            // Halfway bounce-back
                            self.f_fine_tmp[idx_fine(x, y, k)] =
                                self.f_fine[idx_fine(x, y, OPP[k])];
                        } else {
                            self.f_fine_tmp[idx_fine(x, y, k)] =
                                self.f_fine[idx_fine(xp, yp, k)];
                        }
                    }
                }
            }
        }
        std::mem::swap(&mut self.f_fine, &mut self.f_fine_tmp);
    }

    /// Streaming step for coarse grid
    fn stream_coarse(&mut self) {
        for y in 1..NY_COARSE-1 {
            for x in 1..NX_COARSE-1 {
                for k in 0..9 {
                    let xp = (x as i32 - EX[k]) as usize;
                    let yp = (y as i32 - EY[k]) as usize;

                    if xp < NX_COARSE && yp < NY_COARSE {
                        self.f_coarse_tmp[idx_coarse(x, y, k)] =
                            self.f_coarse[idx_coarse(xp, yp, k)];
                    }
                }
            }
        }
        std::mem::swap(&mut self.f_coarse, &mut self.f_coarse_tmp);
    }

    /// Apply inlet boundary condition (left edge) - equilibrium distribution
    fn apply_inlet(&mut self) {
        let rho_in = 1.0;

        for y in 0..NY_FINE {
            let x = 0;
            for k in 0..9 {
                self.f_fine[idx_fine(x, y, k)] = Self::feq(rho_in, U_INF, 0.0, k);
            }
        }

        for y in 0..NY_COARSE {
            let x = 0;
            for k in 0..9 {
                self.f_coarse[idx_coarse(x, y, k)] = Self::feq(rho_in, U_INF, 0.0, k);
            }
        }
    }

    /// Apply outlet boundary condition (right edge) - convective
    fn apply_outlet(&mut self) {
        let u_conv = U_INF;

        // Fine grid outlet (right edge of shown region connects to coarse)
        // Coarse grid outlet
        for y in 1..NY_COARSE-1 {
            let x = NX_COARSE - 1;
            let xm = NX_COARSE - 2;

            for k in 0..9 {
                // Simple convective: f(x,t+dt) = f(x-dx,t)
                self.f_coarse[idx_coarse(x, y, k)] =
                    self.f_coarse[idx_coarse(xm, y, k)];
            }
        }
    }

    /// Apply top/bottom boundary conditions - equilibrium
    fn apply_top_bottom(&mut self) {
        let rho_in = 1.0;

        // Coarse grid boundaries
        for x in 0..NX_COARSE {
            // Bottom
            for k in 0..9 {
                self.f_coarse[idx_coarse(x, 0, k)] = Self::feq(rho_in, U_INF, 0.0, k);
            }
            // Top
            for k in 0..9 {
                self.f_coarse[idx_coarse(x, NY_COARSE-1, k)] = Self::feq(rho_in, U_INF, 0.0, k);
            }
        }
    }

    /// Cubic spline interpolation
    fn cubic_interp(t: f64, f0: f64, f1: f64, f2: f64, f3: f64) -> f64 {
        let t2 = t * t;
        let t3 = t2 * t;

        let a0 = -0.5 * f0 + 1.5 * f1 - 1.5 * f2 + 0.5 * f3;
        let a1 = f0 - 2.5 * f1 + 2.0 * f2 - 0.5 * f3;
        let a2 = -0.5 * f0 + 0.5 * f2;
        let a3 = f1;

        a0 * t3 + a1 * t2 + a2 * t + a3
    }

    /// Transfer data from coarse to fine grid (spatial interpolation)
    fn coarse_to_fine_boundary(&mut self) {
        // Left boundary of fine grid (x=0)
        let coarse_x = FINE_X_OFFSET;

        for fy in 0..NY_FINE {
            // Physical y coordinate
            let y_phys = (fy as f64) * DX_FINE;
            // Coarse grid coordinate
            let cy_f = (y_phys - Y_MIN) / DX_COARSE + (FINE_Y_OFFSET as f64);
            let cy0 = (cy_f as usize).saturating_sub(1).min(NY_COARSE - 4);
            let t = cy_f - (cy0 + 1) as f64;

            for k in 0..9 {
                if EX[k] > 0 {  // Incoming from left
                    let f0 = self.f_coarse[idx_coarse(coarse_x, cy0, k)];
                    let f1 = self.f_coarse[idx_coarse(coarse_x, cy0 + 1, k)];
                    let f2 = self.f_coarse[idx_coarse(coarse_x, cy0 + 2, k)];
                    let f3 = self.f_coarse[idx_coarse(coarse_x, cy0 + 3, k)];

                    self.f_fine[idx_fine(0, fy, k)] = Self::cubic_interp(t, f0, f1, f2, f3);
                }
            }
        }

        // Similar for other boundaries...
        // Bottom boundary of fine grid
        let coarse_y = FINE_Y_OFFSET;

        for fx in 0..NX_FINE {
            let x_phys = (fx as f64) * DX_FINE;
            let cx_f = x_phys / DX_COARSE + (FINE_X_OFFSET as f64);
            let cx0 = (cx_f as usize).saturating_sub(1).min(NX_COARSE - 4);
            let t = cx_f - (cx0 + 1) as f64;

            for k in 0..9 {
                if EY[k] > 0 {  // Incoming from bottom
                    let f0 = self.f_coarse[idx_coarse(cx0, coarse_y, k)];
                    let f1 = self.f_coarse[idx_coarse(cx0 + 1, coarse_y, k)];
                    let f2 = self.f_coarse[idx_coarse(cx0 + 2, coarse_y, k)];
                    let f3 = self.f_coarse[idx_coarse(cx0 + 3, coarse_y, k)];

                    self.f_fine[idx_fine(fx, 0, k)] = Self::cubic_interp(t, f0, f1, f2, f3);
                }
            }
        }
    }

    /// Transfer data from fine to coarse grid
    fn fine_to_coarse_boundary(&mut self) {
        // Average fine grid values to update coarse grid at interface
        // This is simplified - full implementation would use proper restriction

        for cy in FINE_Y_OFFSET..(FINE_Y_OFFSET + NY_FINE/2) {
            let fy = (cy - FINE_Y_OFFSET) * 2;
            if fy + 1 >= NY_FINE { continue; }

            for cx in FINE_X_OFFSET..(FINE_X_OFFSET + NX_FINE/2) {
                let fx = (cx - FINE_X_OFFSET) * 2;
                if fx + 1 >= NX_FINE { continue; }

                for k in 0..9 {
                    // Average 4 fine cells
                    let avg = (self.f_fine[idx_fine(fx, fy, k)]
                             + self.f_fine[idx_fine(fx+1, fy, k)]
                             + self.f_fine[idx_fine(fx, fy+1, k)]
                             + self.f_fine[idx_fine(fx+1, fy+1, k)]) / 4.0;

                    self.f_coarse[idx_coarse(cx, cy, k)] = avg;
                }
            }
        }
    }

    /// Update macroscopic variables for visualization
    fn update_macro(&mut self) {
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);
                let base = idx * 9;

                if self.obstacle[idx] {
                    self.rho[idx] = 1.0;
                    self.ux[idx] = 0.0;
                    self.uy[idx] = 0.0;
                } else {
                    let (rho, ux, uy) = Self::compute_macro(&self.f_fine, base);
                    self.rho[idx] = rho;
                    self.ux[idx] = ux;
                    self.uy[idx] = uy;
                }
            }
        }
    }

    /// Compute vorticity at a point
    fn compute_vorticity(&self, x: usize, y: usize) -> f64 {
        if x == 0 || x >= NX_FINE - 1 || y == 0 || y >= NY_FINE - 1 {
            return 0.0;
        }

        // duy/dx - dux/dy
        let duy_dx = (self.uy[idx_2d(x+1, y)] - self.uy[idx_2d(x-1, y)]) / (2.0 * DX_FINE);
        let dux_dy = (self.ux[idx_2d(x, y+1)] - self.ux[idx_2d(x, y-1)]) / (2.0 * DX_FINE);

        duy_dx - dux_dy
    }

    /// Compute pressure (from density, since incompressible: p = rho * cs^2)
    fn compute_pressure(&self, x: usize, y: usize) -> f64 {
        let idx = idx_2d(x, y);
        (self.rho[idx] - 1.0) * CS2  // Pressure relative to reference
    }

    /// Compute Bernoulli quantity (total head)
    fn compute_bernoulli(&self, x: usize, y: usize) -> f64 {
        let idx = idx_2d(x, y);
        let u2 = self.ux[idx] * self.ux[idx] + self.uy[idx] * self.uy[idx];
        let p = (self.rho[idx] - 1.0) * CS2;

        // Dynamic pressure + static pressure (normalized)
        0.5 * u2 + p
    }

    /// Color mapping: gray at origin, red for positive, blue for negative
    fn value_to_color(&self, value: f64) -> (u8, u8, u8) {
        let normalized = (value - self.colormap_origin) / self.colormap_scale;
        let clamped = normalized.max(-1.0).min(1.0);

        if clamped > 0.0 {
            // Positive: gray to red
            let intensity = (clamped * 255.0) as u8;
            (128 + intensity / 2, (128.0 * (1.0 - clamped)) as u8, (128.0 * (1.0 - clamped)) as u8)
        } else {
            // Negative: gray to blue
            let intensity = (-clamped * 255.0) as u8;
            ((128.0 * (1.0 + clamped)) as u8, (128.0 * (1.0 + clamped)) as u8, 128 + intensity / 2)
        }
    }

    /// LIC (Line Integral Convolution) for streamline visualization
    fn compute_lic(&mut self) {
        let kernel_length = 20;
        let mut lic_result = vec![0.0; NX_FINE * NY_FINE];

        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);

                if self.obstacle[idx] {
                    lic_result[idx] = 0.0;
                    continue;
                }

                let mut sum = 0.0;
                let mut weight = 0.0;

                // Forward integration
                let mut px = x as f64;
                let mut py = y as f64;

                for i in 0..kernel_length {
                    let ix = px as usize;
                    let iy = py as usize;

                    if ix >= NX_FINE || iy >= NY_FINE {
                        break;
                    }

                    let noise_idx = iy * NX_FINE + ix;
                    let w = 1.0 - (i as f64) / (kernel_length as f64);
                    sum += self.noise[noise_idx] * w;
                    weight += w;

                    let vel_idx = idx_2d(ix, iy);
                    if self.obstacle[vel_idx] {
                        break;
                    }

                    let ux = self.ux[vel_idx];
                    let uy = self.uy[vel_idx];
                    let mag = (ux * ux + uy * uy).sqrt().max(1e-10);

                    px += ux / mag * 0.5;
                    py += uy / mag * 0.5;
                }

                // Backward integration
                px = x as f64;
                py = y as f64;

                for i in 1..kernel_length {
                    let ix = px as usize;
                    let iy = py as usize;

                    if ix >= NX_FINE || iy >= NY_FINE {
                        break;
                    }

                    let vel_idx = idx_2d(ix, iy);
                    if self.obstacle[vel_idx] {
                        break;
                    }

                    let ux = self.ux[vel_idx];
                    let uy = self.uy[vel_idx];
                    let mag = (ux * ux + uy * uy).sqrt().max(1e-10);

                    px -= ux / mag * 0.5;
                    py -= uy / mag * 0.5;

                    let ix = px as usize;
                    let iy = py as usize;

                    if ix >= NX_FINE || iy >= NY_FINE {
                        break;
                    }

                    let noise_idx = iy * NX_FINE + ix;
                    let w = 1.0 - (i as f64) / (kernel_length as f64);
                    sum += self.noise[noise_idx] * w;
                    weight += w;
                }

                if weight > 0.0 {
                    lic_result[idx] = sum / weight;
                }
            }
        }

        // Store LIC result in image
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);
                let pixel_idx = ((NY_FINE - 1 - y) * NX_FINE + x) * 4;

                if self.obstacle[idx] {
                    // Obstacle: dark gray
                    self.image_data[pixel_idx] = 64;
                    self.image_data[pixel_idx + 1] = 64;
                    self.image_data[pixel_idx + 2] = 64;
                } else {
                    // LIC value modulated by velocity magnitude
                    let vel_idx = idx_2d(x, y);
                    let speed = (self.ux[vel_idx].powi(2) + self.uy[vel_idx].powi(2)).sqrt();
                    let speed_factor = (speed / U_INF).min(2.0) / 2.0;

                    let lic_val = lic_result[idx];
                    let base = (lic_val * 180.0 + 40.0) as u8;

                    // Blend with speed-based color
                    let r = (base as f64 * (1.0 - speed_factor * 0.3)) as u8;
                    let g = (base as f64 * (1.0 + speed_factor * 0.2)) as u8;
                    let b = base;

                    self.image_data[pixel_idx] = r;
                    self.image_data[pixel_idx + 1] = g;
                    self.image_data[pixel_idx + 2] = b;
                }
                self.image_data[pixel_idx + 3] = 255;
            }
        }
    }

    /// Render velocity magnitude colormap
    fn render_velocity(&mut self) {
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);
                let pixel_idx = ((NY_FINE - 1 - y) * NX_FINE + x) * 4;

                if self.obstacle[idx] {
                    self.image_data[pixel_idx] = 64;
                    self.image_data[pixel_idx + 1] = 64;
                    self.image_data[pixel_idx + 2] = 64;
                } else {
                    let speed = (self.ux[idx].powi(2) + self.uy[idx].powi(2)).sqrt();
                    // Normalize by U_INF, value=1.0 means flow at inlet speed
                    let value = speed / U_INF - 1.0;  // 0 at U_INF, positive for faster
                    let (r, g, b) = self.value_to_color(value);

                    self.image_data[pixel_idx] = r;
                    self.image_data[pixel_idx + 1] = g;
                    self.image_data[pixel_idx + 2] = b;
                }
                self.image_data[pixel_idx + 3] = 255;
            }
        }
    }

    /// Render vorticity colormap
    fn render_vorticity(&mut self) {
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);
                let pixel_idx = ((NY_FINE - 1 - y) * NX_FINE + x) * 4;

                if self.obstacle[idx] {
                    self.image_data[pixel_idx] = 64;
                    self.image_data[pixel_idx + 1] = 64;
                    self.image_data[pixel_idx + 2] = 64;
                } else {
                    let vorticity = self.compute_vorticity(x, y);
                    let (r, g, b) = self.value_to_color(vorticity);

                    self.image_data[pixel_idx] = r;
                    self.image_data[pixel_idx + 1] = g;
                    self.image_data[pixel_idx + 2] = b;
                }
                self.image_data[pixel_idx + 3] = 255;
            }
        }
    }

    /// Render pressure colormap
    fn render_pressure(&mut self) {
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);
                let pixel_idx = ((NY_FINE - 1 - y) * NX_FINE + x) * 4;

                if self.obstacle[idx] {
                    self.image_data[pixel_idx] = 64;
                    self.image_data[pixel_idx + 1] = 64;
                    self.image_data[pixel_idx + 2] = 64;
                } else {
                    let pressure = self.compute_pressure(x, y);
                    // Normalize similar to velocity
                    let value = pressure / (0.5 * U_INF * U_INF);
                    let (r, g, b) = self.value_to_color(value);

                    self.image_data[pixel_idx] = r;
                    self.image_data[pixel_idx + 1] = g;
                    self.image_data[pixel_idx + 2] = b;
                }
                self.image_data[pixel_idx + 3] = 255;
            }
        }
    }

    /// Render Bernoulli quantity colormap
    fn render_bernoulli(&mut self) {
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let idx = idx_2d(x, y);
                let pixel_idx = ((NY_FINE - 1 - y) * NX_FINE + x) * 4;

                if self.obstacle[idx] {
                    self.image_data[pixel_idx] = 64;
                    self.image_data[pixel_idx + 1] = 64;
                    self.image_data[pixel_idx + 2] = 64;
                } else {
                    let bernoulli = self.compute_bernoulli(x, y);
                    // Reference: 0.5 * U_INF^2 (far field)
                    let ref_value = 0.5 * U_INF * U_INF;
                    let value = (bernoulli - ref_value) / ref_value;
                    let (r, g, b) = self.value_to_color(value);

                    self.image_data[pixel_idx] = r;
                    self.image_data[pixel_idx + 1] = g;
                    self.image_data[pixel_idx + 2] = b;
                }
                self.image_data[pixel_idx + 3] = 255;
            }
        }
    }
}

#[wasm_bindgen]
impl LBMSimulation {
    /// Create a new simulation
    #[wasm_bindgen(constructor)]
    pub fn new(reynolds: f64) -> LBMSimulation {
        // Calculate lattice viscosity from Reynolds number
        // Re = L * U / nu with L = 1 physical unit = 1/DX_FINE lattice cells
        let nu = U_INF / (reynolds * DX_FINE);

        // BGK relaxation parameter: omega = 1 / (3*nu + 0.5)
        let omega = 1.0 / (3.0 * nu + 0.5);

        // Coarse grid: acoustic scaling (dt ~ dx) gives nu_lattice / 2 at 2x spacing
        let nu_coarse = nu / 2.0;
        let omega_coarse = 1.0 / (3.0 * nu_coarse + 0.5);

        let n_fine = NX_FINE * NY_FINE * 9;
        let n_coarse = NX_COARSE * NY_COARSE * 9;
        let n_cells = NX_FINE * NY_FINE;

        // Initialize distribution functions with equilibrium + small perturbation
        let mut f_fine = vec![0.0; n_fine];
        let mut f_coarse = vec![0.0; n_coarse];

        // Initial equilibrium with small perturbation
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let base = (y * NX_FINE + x) * 9;
                // Small random perturbation for initial instability
                let perturb_x = 0.001 * ((x * 7 + y * 13) % 100) as f64 / 100.0 - 0.0005;
                let perturb_y = 0.001 * ((x * 11 + y * 17) % 100) as f64 / 100.0 - 0.0005;

                for k in 0..9 {
                    f_fine[base + k] = LBMSimulation::feq(1.0, U_INF + perturb_x, perturb_y, k);
                }
            }
        }

        for y in 0..NY_COARSE {
            for x in 0..NX_COARSE {
                let base = (y * NX_COARSE + x) * 9;
                for k in 0..9 {
                    f_coarse[base + k] = LBMSimulation::feq(1.0, U_INF, 0.0, k);
                }
            }
        }

        // Initialize noise texture for LIC
        let mut noise = vec![0.0; n_cells];
        for i in 0..n_cells {
            noise[i] = ((i * 1103515245 + 12345) % 256) as f64 / 255.0;
        }

        LBMSimulation {
            f_fine: f_fine.clone(),
            f_fine_tmp: f_fine,
            f_coarse: f_coarse.clone(),
            f_coarse_tmp: f_coarse.clone(),
            f_coarse_history: vec![f_coarse; 3],
            rho: vec![1.0; n_cells],
            ux: vec![U_INF; n_cells],
            uy: vec![0.0; n_cells],
            obstacle: vec![false; n_cells],
            image_data: vec![128; n_cells * 4],
            noise,
            nu,
            omega,
            omega_coarse,
            reynolds,
            time: 0.0,
            display_mode: 0,
            colormap_scale: 1.0,
            colormap_origin: 0.0,
            paused: false,
        }
    }

    /// Get image width
    pub fn width(&self) -> usize {
        NX_FINE
    }

    /// Get image height
    pub fn height(&self) -> usize {
        NY_FINE
    }

    /// Get pointer to image data
    pub fn image_ptr(&self) -> *const u8 {
        self.image_data.as_ptr()
    }

    /// Get pointer to obstacle data
    pub fn obstacle_ptr(&mut self) -> *mut bool {
        self.obstacle.as_mut_ptr()
    }

    /// Get obstacle array length
    pub fn obstacle_len(&self) -> usize {
        self.obstacle.len()
    }

    /// Set obstacle at pixel position
    pub fn set_obstacle(&mut self, x: usize, y: usize, value: bool) {
        if x < NX_FINE && y < NY_FINE {
            let idx = y * NX_FINE + x;
            self.obstacle[idx] = value;

            // Reset distribution at obstacle
            if value {
                let base = idx * 9;
                for k in 0..9 {
                    self.f_fine[base + k] = LBMSimulation::feq(1.0, 0.0, 0.0, k);
                }
            }
        }
    }

    /// Clear all obstacles
    pub fn clear_obstacles(&mut self) {
        for i in 0..self.obstacle.len() {
            self.obstacle[i] = false;
        }

        // Reinitialize flow
        for y in 0..NY_FINE {
            for x in 0..NX_FINE {
                let base = (y * NX_FINE + x) * 9;
                for k in 0..9 {
                    self.f_fine[base + k] = LBMSimulation::feq(1.0, U_INF, 0.0, k);
                }
            }
        }
    }

    /// Set Reynolds number
    pub fn set_reynolds(&mut self, reynolds: f64) {
        self.reynolds = reynolds;
        self.nu = U_INF / (reynolds * DX_FINE);
        self.omega = 1.0 / (3.0 * self.nu + 0.5);

        let nu_coarse = self.nu / 2.0;
        self.omega_coarse = 1.0 / (3.0 * nu_coarse + 0.5);
    }

    /// Get current Reynolds number
    pub fn get_reynolds(&self) -> f64 {
        self.reynolds
    }

    /// Set display mode (0: velocity, 1: vorticity, 2: pressure, 3: streamlines, 4: Bernoulli)
    pub fn set_display_mode(&mut self, mode: u32) {
        self.display_mode = mode;
    }

    /// Set colormap scale
    pub fn set_colormap_scale(&mut self, scale: f64) {
        self.colormap_scale = scale.max(0.001);
    }

    /// Get colormap scale
    pub fn get_colormap_scale(&self) -> f64 {
        self.colormap_scale
    }

    /// Set colormap origin
    pub fn set_colormap_origin(&mut self, origin: f64) {
        self.colormap_origin = origin;
    }

    /// Get colormap origin
    pub fn get_colormap_origin(&self) -> f64 {
        self.colormap_origin
    }

    /// Pause/resume simulation
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Get physical time
    pub fn get_time(&self) -> f64 {
        self.time
    }

    /// Perform one simulation step (physical time = 0.05)
    pub fn step(&mut self) {
        if self.paused {
            self.render();
            return;
        }

        // Store coarse grid history for temporal interpolation
        self.f_coarse_history.remove(0);
        self.f_coarse_history.push(self.f_coarse.clone());

        // Coarse grid: 9 substeps
        for _ in 0..9 {
            // Collision
            for y in 1..NY_COARSE-1 {
                for x in 1..NX_COARSE-1 {
                    let base = idx_coarse(x, y, 0);
                    Self::collision_bgk(&mut self.f_coarse, base, self.omega_coarse);
                }
            }

            // Streaming
            self.stream_coarse();

            // Boundary conditions
            self.apply_inlet();
            self.apply_outlet();
            self.apply_top_bottom();
        }

        // Fine grid: 18 substeps
        for substep in 0..18 {
            // Interpolate from coarse to fine at boundaries (temporal + spatial)
            if substep % 2 == 0 {
                self.coarse_to_fine_boundary();
            }

            // Collision on fine grid (BGK; central moments need more work)
            for y in 1..NY_FINE-1 {
                for x in 1..NX_FINE-1 {
                    let idx = idx_2d(x, y);
                    if !self.obstacle[idx] {
                        let base = idx * 9;
                        Self::collision_bgk(&mut self.f_fine, base, self.omega);
                    }
                }
            }

            // Streaming
            self.stream_fine();

            // Fine grid inlet (left edge of shown region gets from coarse)
            for y in 0..NY_FINE {
                let x = 0;
                for k in 0..9 {
                    if EX[k] > 0 {
                        self.f_fine[idx_fine(x, y, k)] =
                            LBMSimulation::feq(1.0, U_INF, 0.0, k);
                    }
                }
            }

            // Fine grid outlet (simple extrapolation)
            for y in 0..NY_FINE {
                let x = NX_FINE - 1;
                let xm = NX_FINE - 2;
                for k in 0..9 {
                    if EX[k] < 0 {
                        self.f_fine[idx_fine(x, y, k)] =
                            self.f_fine[idx_fine(xm, y, k)];
                    }
                }
            }

            // Top/bottom boundaries
            for x in 0..NX_FINE {
                // Bottom
                for k in 0..9 {
                    if EY[k] > 0 {
                        self.f_fine[idx_fine(x, 0, k)] =
                            LBMSimulation::feq(1.0, U_INF, 0.0, k);
                    }
                }
                // Top
                for k in 0..9 {
                    if EY[k] < 0 {
                        self.f_fine[idx_fine(x, NY_FINE-1, k)] =
                            LBMSimulation::feq(1.0, U_INF, 0.0, k);
                    }
                }
            }
        }

        // Update macro quantities for visualization
        self.update_macro();

        // Update physical time
        self.time += 0.05;

        // Render
        self.render();
    }

    /// Render current state to image buffer
    pub fn render(&mut self) {
        match self.display_mode {
            0 => self.render_velocity(),
            1 => self.render_vorticity(),
            2 => self.render_pressure(),
            3 => self.compute_lic(),
            4 => self.render_bernoulli(),
            _ => self.render_velocity(),
        }
    }

    /// Get velocity at normalized coordinates (0-1)
    pub fn get_velocity_at(&self, norm_x: f64, norm_y: f64) -> Vec<f64> {
        let x = ((norm_x * NX_FINE as f64) as usize).min(NX_FINE - 1);
        let y = ((norm_y * NY_FINE as f64) as usize).min(NY_FINE - 1);
        let idx = idx_2d(x, y);

        vec![self.ux[idx], self.uy[idx]]
    }

    /// Regenerate noise texture for LIC
    pub fn regenerate_noise(&mut self) {
        let seed = (self.time * 1000.0) as usize;
        for i in 0..self.noise.len() {
            self.noise[i] = (((i + seed) * 1103515245 + 12345) % 256) as f64 / 255.0;
        }
    }
}
