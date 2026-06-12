use wasm_bindgen::prelude::*;

// D2Q9 lattice velocities
const EX: [i32; 9] = [0, 1, 0, -1, 0, 1, -1, -1, 1];
const EY: [i32; 9] = [0, 0, 1, 0, -1, 1, 1, -1, -1];
const W: [f32; 9] = [
    4.0 / 9.0,
    1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0,
    1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0,
];
const OPP: [usize; 9] = [0, 3, 4, 1, 2, 7, 8, 5, 6];

// Grid dimensions
const NX: usize = 200;
const NY: usize = 100;

// Flow parameters
const U_MAX: f32 = 0.1;
const L_CHAR: f32 = 25.0; // characteristic length = NY/4 in lattice units

// Substeps per JS frame (balances accuracy vs speed)
const SUBSTEPS: usize = 8;

// Smagorinsky SGS constant
const C_SMAG: f32 = 0.1;

#[inline(always)]
fn cell(x: usize, y: usize) -> usize {
    y * NX + x
}

// Free-slip wall: reflect y-component of velocity direction
#[inline(always)]
fn reflect_y(k: usize) -> usize {
    match k {
        2 => 4, 4 => 2,
        5 => 8, 8 => 5,
        6 => 7, 7 => 6,
        _ => k,
    }
}

#[inline(always)]
fn feq(rho: f32, ux: f32, uy: f32, k: usize) -> f32 {
    let eu = EX[k] as f32 * ux + EY[k] as f32 * uy;
    let u2 = ux * ux + uy * uy;
    W[k] * rho * (1.0 + 3.0 * eu + 4.5 * eu * eu - 1.5 * u2)
}

fn omega_from_re(re: f32) -> f32 {
    let nu = U_MAX * L_CHAR / re;
    1.0 / (3.0 * nu + 0.5)
}

#[wasm_bindgen]
pub struct LBMSimulation {
    f: Vec<f32>,
    f_tmp: Vec<f32>,
    obstacle: Vec<bool>,
    rho: Vec<f32>,
    ux: Vec<f32>,
    uy: Vec<f32>,
    image_data: Vec<u8>,
    noise: Vec<f32>,
    omega: f32,
    reynolds: f32,
    time: f32,
    display_mode: u32,
    colormap_scale: f32,
    colormap_origin: f32,
    paused: bool,
}

#[wasm_bindgen]
impl LBMSimulation {
    #[wasm_bindgen(constructor)]
    pub fn new(reynolds: f64) -> LBMSimulation {
        let re = reynolds as f32;
        let n = NX * NY;

        let mut f = vec![0.0f32; n * 9];
        for i in 0..n {
            for k in 0..9 {
                f[i * 9 + k] = feq(1.0, U_MAX, 0.0, k);
            }
        }

        let noise: Vec<f32> = (0..n)
            .map(|i| (((i.wrapping_mul(1103515245)).wrapping_add(12345) >> 8) & 0xFF) as f32 / 255.0)
            .collect();

        LBMSimulation {
            f_tmp: f.clone(),
            f,
            obstacle: vec![false; n],
            rho: vec![1.0; n],
            ux: vec![U_MAX; n],
            uy: vec![0.0; n],
            image_data: vec![128; n * 4],
            noise,
            omega: omega_from_re(re),
            reynolds: re,
            time: 0.0,
            display_mode: 0,
            colormap_scale: 1.0,
            colormap_origin: 0.0,
            paused: false,
        }
    }

    // --- Core simulation steps ---

    fn do_collision(&mut self) {
        let omega = self.omega;
        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    continue;
                }
                let base = c * 9;

                let mut rho = 0.0f32;
                let mut ux = 0.0f32;
                let mut uy = 0.0f32;
                for k in 0..9 {
                    let fk = self.f[base + k];
                    rho += fk;
                    ux += EX[k] as f32 * fk;
                    uy += EY[k] as f32 * fk;
                }
                if rho < 1e-6 {
                    continue;
                }
                ux /= rho;
                uy /= rho;

                let mut feq_arr = [0.0f32; 9];
                let mut pi_xx = 0.0f32;
                let mut pi_yy = 0.0f32;
                let mut pi_xy = 0.0f32;
                for k in 0..9 {
                    feq_arr[k] = feq(rho, ux, uy, k);
                    let fneq = self.f[base + k] - feq_arr[k];
                    let ex = EX[k] as f32;
                    let ey = EY[k] as f32;
                    pi_xx += ex * ex * fneq;
                    pi_yy += ey * ey * fneq;
                    pi_xy += ex * ey * fneq;
                }

                // Smagorinsky sub-grid stress → cap omega so tau ≥ 0.52
                let pi2 = 2.0 * (pi_xx * pi_xx + pi_yy * pi_yy) + 4.0 * pi_xy * pi_xy;
                let tau0 = 1.0 / omega;
                let tau_eff = 0.5
                    * (tau0
                        + (tau0 * tau0
                            + 18.0 * std::f32::consts::SQRT_2 * C_SMAG * C_SMAG * pi2.sqrt()
                                / rho.max(1e-6))
                            .sqrt());
                // Safety floor: keep tau_eff ≥ 0.52 to avoid instability near obstacles
                let om_eff = 1.0 / tau_eff.max(0.52);

                // Unrolled BGK update (avoids loop overhead in hot path)
                for k in 0..9 {
                    unsafe {
                        *self.f.get_unchecked_mut(base + k) +=
                            om_eff * (feq_arr[k] - *self.f.get_unchecked(base + k));
                    }
                }

                self.rho[c] = rho;
                self.ux[c] = ux;
                self.uy[c] = uy;
            }
        }
    }

    fn do_stream(&mut self) {
        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    continue;
                }
                for k in 0..9 {
                    let xp = x as i32 - EX[k];
                    let yp = y as i32 - EY[k];

                    let val = if xp < 0 {
                        // Left inlet placeholder — overwritten by apply_inlet()
                        self.f[c * 9 + k]
                    } else if xp >= NX as i32 {
                        // Right outlet: zero-gradient from NX-2
                        self.f[cell(NX - 2, y) * 9 + k]
                    } else if yp < 0 || yp >= NY as i32 {
                        // Top / bottom wall: free-slip (reflect y-component)
                        self.f[c * 9 + reflect_y(k)]
                    } else {
                        let src = cell(xp as usize, yp as usize);
                        if self.obstacle[src] {
                            // Halfway bounce-back from obstacle
                            self.f[c * 9 + OPP[k]]
                        } else {
                            self.f[src * 9 + k]
                        }
                    };
                    self.f_tmp[c * 9 + k] = val;
                }
            }
        }
        std::mem::swap(&mut self.f, &mut self.f_tmp);
    }

    fn apply_inlet(&mut self) {
        for y in 0..NY {
            let base = cell(0, y) * 9;
            for k in 0..9 {
                self.f[base + k] = feq(1.0, U_MAX, 0.0, k);
            }
            self.rho[cell(0, y)] = 1.0;
            self.ux[cell(0, y)] = U_MAX;
            self.uy[cell(0, y)] = 0.0;
        }
    }

    fn apply_outlet(&mut self) {
        for y in 0..NY {
            let src = cell(NX - 2, y) * 9;
            let dst = cell(NX - 1, y) * 9;
            for k in 0..9 {
                self.f[dst + k] = self.f[src + k];
            }
        }
    }

    // --- Public WASM API ---

    pub fn step(&mut self) {
        if self.paused {
            self.render();
            return;
        }
        for _ in 0..SUBSTEPS {
            self.do_collision();
            self.do_stream();
            self.apply_inlet();
            self.apply_outlet();
        }
        self.time += SUBSTEPS as f32;
        self.render();
    }

    pub fn render(&mut self) {
        match self.display_mode {
            1 => self.render_vorticity(),
            2 => self.render_pressure(),
            3 => self.compute_lic(),
            4 => self.render_bernoulli(),
            _ => self.render_velocity(),
        }
    }

    pub fn width(&self) -> usize {
        NX
    }
    pub fn height(&self) -> usize {
        NY
    }

    pub fn image_ptr(&self) -> *const u8 {
        self.image_data.as_ptr()
    }

    pub fn obstacle_ptr(&mut self) -> *mut bool {
        self.obstacle.as_mut_ptr()
    }
    pub fn obstacle_len(&self) -> usize {
        self.obstacle.len()
    }

    pub fn set_obstacle(&mut self, x: usize, y: usize, value: bool) {
        if x >= NX || y >= NY {
            return;
        }
        let c = cell(x, y);
        self.obstacle[c] = value;
        if value {
            for k in 0..9 {
                self.f[c * 9 + k] = 0.0;
                self.f_tmp[c * 9 + k] = 0.0;
            }
        } else {
            // Reinitialize freed cell with local average or inlet equilibrium
            for k in 0..9 {
                let v = feq(1.0, U_MAX, 0.0, k);
                self.f[c * 9 + k] = v;
                self.f_tmp[c * 9 + k] = v;
            }
            self.rho[c] = 1.0;
            self.ux[c] = U_MAX;
            self.uy[c] = 0.0;
        }
    }

    pub fn clear_obstacles(&mut self) {
        for c in 0..NX * NY {
            self.obstacle[c] = false;
            for k in 0..9 {
                let v = feq(1.0, U_MAX, 0.0, k);
                self.f[c * 9 + k] = v;
                self.f_tmp[c * 9 + k] = v;
            }
            self.rho[c] = 1.0;
            self.ux[c] = U_MAX;
            self.uy[c] = 0.0;
        }
        self.time = 0.0;
    }

    pub fn set_reynolds(&mut self, reynolds: f64) {
        self.reynolds = reynolds as f32;
        self.omega = omega_from_re(self.reynolds);
    }
    pub fn get_reynolds(&self) -> f64 {
        self.reynolds as f64
    }

    pub fn set_display_mode(&mut self, mode: u32) {
        self.display_mode = mode;
    }

    pub fn set_colormap_scale(&mut self, scale: f64) {
        self.colormap_scale = scale as f32;
    }
    pub fn get_colormap_scale(&self) -> f64 {
        self.colormap_scale as f64
    }

    pub fn set_colormap_origin(&mut self, origin: f64) {
        self.colormap_origin = origin as f32;
    }
    pub fn get_colormap_origin(&self) -> f64 {
        self.colormap_origin as f64
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn get_time(&self) -> f64 {
        self.time as f64
    }

    pub fn get_velocity_at(&self, norm_x: f64, norm_y: f64) -> Vec<f64> {
        let x = ((norm_x * NX as f64) as usize).min(NX - 1);
        let y = ((norm_y * NY as f64) as usize).min(NY - 1);
        let c = cell(x, y);
        vec![self.ux[c] as f64, self.uy[c] as f64]
    }

    pub fn regenerate_noise(&mut self) {
        let seed = self.time as usize;
        for i in 0..self.noise.len() {
            self.noise[i] = (((i.wrapping_add(seed).wrapping_mul(1103515245)).wrapping_add(12345)
                >> 8)
                & 0xFF) as f32
                / 255.0;
        }
    }

    // --- Rendering helpers ---

    fn value_to_color(&self, value: f32) -> (u8, u8, u8) {
        let n = (value - self.colormap_origin) / self.colormap_scale;
        let c = n.max(-1.0).min(1.0);
        if c > 0.0 {
            let i = (c * 255.0) as u8;
            (
                128 + i / 2,
                (128.0 * (1.0 - c)) as u8,
                (128.0 * (1.0 - c)) as u8,
            )
        } else {
            let i = (-c * 255.0) as u8;
            (
                (128.0 * (1.0 + c)) as u8,
                (128.0 * (1.0 + c)) as u8,
                128 + i / 2,
            )
        }
    }

    #[inline(always)]
    fn set_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        let p = ((NY - 1 - y) * NX + x) * 4;
        self.image_data[p] = r;
        self.image_data[p + 1] = g;
        self.image_data[p + 2] = b;
        self.image_data[p + 3] = 255;
    }

    fn render_velocity(&mut self) {
        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    self.set_pixel(x, y, 64, 64, 64);
                } else {
                    let spd =
                        (self.ux[c] * self.ux[c] + self.uy[c] * self.uy[c]).sqrt();
                    let (r, g, b) = self.value_to_color(spd / U_MAX - 1.0);
                    self.set_pixel(x, y, r, g, b);
                }
            }
        }
    }

    fn render_vorticity(&mut self) {
        let mut vort = vec![0.0f32; NX * NY];
        for y in 1..NY - 1 {
            for x in 1..NX - 1 {
                let duy_dx = (self.uy[cell(x + 1, y)] - self.uy[cell(x - 1, y)]) * 0.5;
                let dux_dy = (self.ux[cell(x, y + 1)] - self.ux[cell(x, y - 1)]) * 0.5;
                vort[cell(x, y)] = duy_dx - dux_dy;
            }
        }
        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    self.set_pixel(x, y, 64, 64, 64);
                } else {
                    let (r, g, b) = self.value_to_color(vort[c] / U_MAX);
                    self.set_pixel(x, y, r, g, b);
                }
            }
        }
    }

    fn render_pressure(&mut self) {
        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    self.set_pixel(x, y, 64, 64, 64);
                } else {
                    let p = (self.rho[c] - 1.0) / (3.0 * U_MAX * U_MAX);
                    let (r, g, b) = self.value_to_color(p);
                    self.set_pixel(x, y, r, g, b);
                }
            }
        }
    }

    fn render_bernoulli(&mut self) {
        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    self.set_pixel(x, y, 64, 64, 64);
                } else {
                    let u2 = self.ux[c] * self.ux[c] + self.uy[c] * self.uy[c];
                    let p = (self.rho[c] - 1.0) / 3.0;
                    let b_val = (0.5 * u2 + p) / (U_MAX * U_MAX);
                    let (r, g, b) = self.value_to_color(b_val);
                    self.set_pixel(x, y, r, g, b);
                }
            }
        }
    }

    fn compute_lic(&mut self) {
        const KL: usize = 20;
        let mut lic = vec![0.0f32; NX * NY];

        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    continue;
                }
                let mut sum = 0.0f32;
                let mut wt = 0.0f32;

                // Forward integration
                let (mut px, mut py) = (x as f32 + 0.5, y as f32 + 0.5);
                for i in 0..KL {
                    if px < 0.0 || px >= NX as f32 || py < 0.0 || py >= NY as f32 {
                        break;
                    }
                    let ix = (px as usize).min(NX - 1);
                    let iy = (py as usize).min(NY - 1);
                    let nc = cell(ix, iy);
                    let w = 1.0 - i as f32 / KL as f32;
                    sum += self.noise[nc] * w;
                    wt += w;
                    if self.obstacle[nc] {
                        break;
                    }
                    let mag =
                        (self.ux[nc] * self.ux[nc] + self.uy[nc] * self.uy[nc]).sqrt().max(1e-7);
                    px += self.ux[nc] / mag * 0.5;
                    py += self.uy[nc] / mag * 0.5;
                }

                // Backward integration
                let (mut px, mut py) = (x as f32 + 0.5, y as f32 + 0.5);
                for i in 1..KL {
                    let ix = (px as usize).min(NX - 1);
                    let iy = (py as usize).min(NY - 1);
                    let nc = cell(ix, iy);
                    if self.obstacle[nc] {
                        break;
                    }
                    let mag =
                        (self.ux[nc] * self.ux[nc] + self.uy[nc] * self.uy[nc]).sqrt().max(1e-7);
                    px -= self.ux[nc] / mag * 0.5;
                    py -= self.uy[nc] / mag * 0.5;
                    if px < 0.0 || px >= NX as f32 || py < 0.0 || py >= NY as f32 {
                        break;
                    }
                    let ix = (px as usize).min(NX - 1);
                    let iy = (py as usize).min(NY - 1);
                    let nc = cell(ix, iy);
                    let w = 1.0 - i as f32 / KL as f32;
                    sum += self.noise[nc] * w;
                    wt += w;
                }

                if wt > 0.0 {
                    lic[c] = sum / wt;
                }
            }
        }

        for y in 0..NY {
            for x in 0..NX {
                let c = cell(x, y);
                if self.obstacle[c] {
                    self.set_pixel(x, y, 64, 64, 64);
                } else {
                    let spd =
                        (self.ux[c] * self.ux[c] + self.uy[c] * self.uy[c]).sqrt();
                    let sf = (spd / U_MAX).min(2.0) / 2.0;
                    let bv = lic[c];
                    let base = (bv * 180.0 + 40.0).min(255.0) as u8;
                    let r = (base as f32 * (1.0 - sf * 0.3)) as u8;
                    let g = (base as f32 * (1.0 + sf * 0.2)).min(255.0) as u8;
                    self.set_pixel(x, y, r, g, base);
                }
            }
        }
    }
}
