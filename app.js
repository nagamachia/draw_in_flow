/**
 * Draw in Flow - 2D Fluid Simulation with Lattice Boltzmann Method
 * 格子ボルツマン法による2次元流体計算
 */

import init, { LBMSimulation } from './lbm_wasm.js?v=4';

class FluidApp {
    constructor() {
        this.canvas = document.getElementById('fluidCanvas');
        this.ctx = this.canvas.getContext('2d');

        // Simulation dimensions — set from WASM after init()
        this.simWidth = 200;
        this.simHeight = 100;

        // Display scale (canvas pixels per simulation cell)
        this.scale = 5;

        // Provisional canvas size (resized after WASM reports actual dimensions)
        this.canvas.width = this.simWidth * this.scale;
        this.canvas.height = this.simHeight * this.scale;

        // Simulation state
        this.simulation = null;
        this.wasmMemory = null;
        this.imageData = null;

        // Drawing state
        this.isDrawing = false;
        this.lastMousePos = null;
        this.brushSize = 10;

        // Pause state
        this.paused = false;

        // Image obstacle state
        this.lastLoadedImage = null;
        this.imageThreshold = 128;
        this.invertObstacle = false;

        // Initialize
        this.init();
    }

    async init() {
        try {
            // Initialize WASM module
            const wasm = await init({ module_or_path: new URL('./lbm_wasm_bg.wasm?v=4', import.meta.url) });
            this.wasmMemory = wasm.memory;

            // Create simulation with default Reynolds number
            this.simulation = new LBMSimulation(1000);

            // Read actual grid dimensions from WASM and resize canvas
            this.simWidth = this.simulation.width();
            this.simHeight = this.simulation.height();
            this.canvas.width = this.simWidth * this.scale;
            this.canvas.height = this.simHeight * this.scale;

            // Create ImageData for rendering
            this.imageData = this.ctx.createImageData(this.simWidth, this.simHeight);

            // Setup event listeners
            this.setupEventListeners();

            // Start animation loop
            this.animate();

            window.__lbmStarted = true;
            console.log('LBM Simulation initialized');
        } catch (error) {
            console.error('Failed to initialize WASM:', error);
            this.showError('WebAssemblyの初期化に失敗しました');
        }
    }

    setupEventListeners() {
        // Mouse events for drawing
        this.canvas.addEventListener('mousedown', (e) => this.startDrawing(e));
        this.canvas.addEventListener('mousemove', (e) => this.draw(e));
        this.canvas.addEventListener('mouseup', () => this.stopDrawing());
        this.canvas.addEventListener('mouseleave', () => this.stopDrawing());

        // Touch events for mobile
        this.canvas.addEventListener('touchstart', (e) => {
            e.preventDefault();
            this.startDrawing(e.touches[0]);
        });
        this.canvas.addEventListener('touchmove', (e) => {
            e.preventDefault();
            this.draw(e.touches[0]);
        });
        this.canvas.addEventListener('touchend', () => this.stopDrawing());

        // Control buttons
        document.getElementById('clearBtn').addEventListener('click', () => this.clearObstacles());
        document.getElementById('toggleFlowBtn').addEventListener('click', () => this.togglePause());

        // Brush size slider
        document.getElementById('brushSize').addEventListener('input', (e) => {
            this.brushSize = parseInt(e.target.value);
            document.getElementById('brushSizeValue').textContent = this.brushSize;
        });

        // Reynolds number slider
        document.getElementById('reynolds').addEventListener('input', (e) => {
            const reynolds = parseInt(e.target.value);
            document.getElementById('reynoldsValue').textContent = reynolds;
            if (this.simulation) {
                this.simulation.set_reynolds(reynolds);
            }
        });

        // Display mode selector
        document.getElementById('displayMode').addEventListener('change', (e) => {
            const mode = parseInt(e.target.value);
            if (this.simulation) {
                this.simulation.set_display_mode(mode);
                // Regenerate noise for LIC mode
                if (mode === 3) {
                    this.simulation.regenerate_noise();
                }
            }
        });

        // Colormap scale slider (only when paused)
        const colorScaleSlider = document.getElementById('colorScale');
        colorScaleSlider.addEventListener('input', (e) => {
            if (!this.paused) return;
            const scale = parseFloat(e.target.value);
            document.getElementById('colorScaleValue').textContent = scale.toFixed(2);
            if (this.simulation) {
                this.simulation.set_colormap_scale(scale);
            }
        });

        // Colormap origin slider (only when paused)
        const colorOriginSlider = document.getElementById('colorOrigin');
        colorOriginSlider.addEventListener('input', (e) => {
            if (!this.paused) return;
            const origin = parseFloat(e.target.value);
            document.getElementById('colorOriginValue').textContent = origin.toFixed(2);
            if (this.simulation) {
                this.simulation.set_colormap_origin(origin);
            }
        });

        // Initial state for colormap controls
        this.updateColormapControls();

        // Image obstacle controls
        this.setupImageControls();
    }

    setupImageControls() {
        const imageInput = document.getElementById('imageInput');
        const thresholdSlider = document.getElementById('threshold');
        const invertBtn = document.getElementById('invertBtn');
        const canvasContainer = document.getElementById('canvasContainer');

        // File picker: label[for="imageInput"] triggers this natively on all browsers including iOS
        imageInput.addEventListener('change', (e) => {
            const file = e.target.files[0];
            if (file) this.loadImageFile(file);
            imageInput.value = '';
        });

        // Drag-and-drop on canvas container
        canvasContainer.addEventListener('dragover', (e) => {
            e.preventDefault();
            canvasContainer.classList.add('drag-over');
        });
        canvasContainer.addEventListener('dragleave', (e) => {
            if (!canvasContainer.contains(e.relatedTarget)) {
                canvasContainer.classList.remove('drag-over');
            }
        });
        canvasContainer.addEventListener('drop', (e) => {
            e.preventDefault();
            canvasContainer.classList.remove('drag-over');
            const file = e.dataTransfer.files[0];
            if (file && file.type.startsWith('image/')) {
                this.loadImageFile(file);
            }
        });

        // Paste from clipboard (Ctrl+V)
        document.addEventListener('paste', (e) => {
            for (const item of e.clipboardData.items) {
                if (item.type.startsWith('image/')) {
                    this.loadImageFile(item.getAsFile());
                    break;
                }
            }
        });

        // Threshold slider
        thresholdSlider.addEventListener('input', (e) => {
            this.imageThreshold = parseInt(e.target.value);
            document.getElementById('thresholdValue').textContent = this.imageThreshold;
            if (this.lastLoadedImage) {
                this.applyImageAsObstacles(this.lastLoadedImage);
            }
        });

        // Invert toggle
        invertBtn.addEventListener('click', () => {
            this.invertObstacle = !this.invertObstacle;
            invertBtn.textContent = this.invertObstacle ? '明→障害物' : '暗→障害物';
            invertBtn.classList.toggle('active', this.invertObstacle);
            if (this.lastLoadedImage) {
                this.applyImageAsObstacles(this.lastLoadedImage);
            }
        });
    }

    loadImageFile(file) {
        const url = URL.createObjectURL(file);
        const img = new Image();
        img.onload = () => {
            this.lastLoadedImage = img;
            this.applyImageAsObstacles(img);
            URL.revokeObjectURL(url);
            // Enable threshold and invert controls
            document.getElementById('threshold').disabled = false;
            document.getElementById('invertBtn').disabled = false;
        };
        img.onerror = () => URL.revokeObjectURL(url);
        img.src = url;
    }

    applyImageAsObstacles(img) {
        if (!this.simulation) return;

        this.simulation.clear_obstacles();

        // Scale image into sim grid with aspect-ratio-preserving letterbox
        const tempCanvas = document.createElement('canvas');
        tempCanvas.width = this.simWidth;
        tempCanvas.height = this.simHeight;
        const ctx = tempCanvas.getContext('2d');

        // Background: white (no obstacle) by default
        ctx.fillStyle = 'white';
        ctx.fillRect(0, 0, this.simWidth, this.simHeight);

        // Center image while maintaining aspect ratio
        const imgAspect = img.width / img.height;
        const simAspect = this.simWidth / this.simHeight;
        let dw, dh, dx, dy;
        if (imgAspect > simAspect) {
            dw = this.simWidth;
            dh = this.simWidth / imgAspect;
            dx = 0;
            dy = (this.simHeight - dh) / 2;
        } else {
            dh = this.simHeight;
            dw = this.simHeight * imgAspect;
            dx = (this.simWidth - dw) / 2;
            dy = 0;
        }
        ctx.drawImage(img, dx, dy, dw, dh);

        const { data } = ctx.getImageData(0, 0, this.simWidth, this.simHeight);

        // Map pixels to obstacles; skip inlet (x<2) and outlet (x>NX-3) columns
        for (let py = 0; py < this.simHeight; py++) {
            for (let px = 2; px < this.simWidth - 2; px++) {
                const i = (py * this.simWidth + px) * 4;
                // Perceptual luminance
                const lum = 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
                const isObstacle = this.invertObstacle
                    ? lum >= this.imageThreshold   // bright → obstacle
                    : lum < this.imageThreshold;   // dark  → obstacle
                if (isObstacle) {
                    // Flip y: image row 0 is at the top, sim y=0 is at the bottom
                    this.simulation.set_obstacle(px, this.simHeight - 1 - py, true);
                }
            }
        }
    }

    updateColormapControls() {
        const controls = document.getElementById('colormapControls');
        const scaleSlider = document.getElementById('colorScale');
        const originSlider = document.getElementById('colorOrigin');

        if (this.paused) {
            controls.classList.add('enabled');
            scaleSlider.disabled = false;
            originSlider.disabled = false;
        } else {
            controls.classList.remove('enabled');
            scaleSlider.disabled = true;
            originSlider.disabled = true;
        }
    }

    getMousePos(e) {
        const rect = this.canvas.getBoundingClientRect();
        // Use actual display dimensions so coordinates work regardless of CSS scaling
        return {
            x: (e.clientX - rect.left) * this.simWidth / rect.width,
            y: (e.clientY - rect.top) * this.simHeight / rect.height
        };
    }

    startDrawing(e) {
        this.isDrawing = true;
        this.lastMousePos = this.getMousePos(e);
        this.setObstacle(this.lastMousePos.x, this.lastMousePos.y);
    }

    draw(e) {
        if (!this.isDrawing) return;

        const pos = this.getMousePos(e);

        if (this.lastMousePos) {
            this.drawLine(this.lastMousePos.x, this.lastMousePos.y, pos.x, pos.y);
        }

        this.lastMousePos = pos;
    }

    stopDrawing() {
        this.isDrawing = false;
        this.lastMousePos = null;
    }

    setObstacle(x, y) {
        if (!this.simulation) return;

        // brushSize is divided by scale(5) so the slider range 3-30 maps to ~1-6 sim cells
        const radius = this.brushSize / this.scale;

        // Draw a circle of obstacles
        for (let dy = -radius; dy <= radius; dy++) {
            for (let dx = -radius; dx <= radius; dx++) {
                if (dx * dx + dy * dy <= radius * radius) {
                    const px = Math.floor(x + dx);
                    const py = Math.floor(y + dy);

                    // Convert to simulation coordinates (y is flipped)
                    const simY = this.simHeight - 1 - py;

                    if (px >= 0 && px < this.simWidth && simY >= 0 && simY < this.simHeight) {
                        this.simulation.set_obstacle(px, simY, true);
                    }
                }
            }
        }
    }

    drawLine(x1, y1, x2, y2) {
        const dist = Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
        const steps = Math.max(1, Math.floor(dist * 2));

        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            const x = x1 + (x2 - x1) * t;
            const y = y1 + (y2 - y1) * t;
            this.setObstacle(x, y);
        }
    }

    clearObstacles() {
        if (this.simulation) {
            this.simulation.clear_obstacles();
        }
    }

    togglePause() {
        this.paused = !this.paused;

        if (this.simulation) {
            this.simulation.set_paused(this.paused);
        }

        const btn = document.getElementById('toggleFlowBtn');
        if (this.paused) {
            btn.textContent = '再開';
            btn.classList.add('paused');
        } else {
            btn.textContent = '一時停止';
            btn.classList.remove('paused');
        }

        this.updateColormapControls();
    }

    render() {
        if (!this.simulation || !this.wasmMemory) return;

        // Get image data from WASM
        const ptr = this.simulation.image_ptr();
        const len = this.simWidth * this.simHeight * 4;
        const wasmData = new Uint8ClampedArray(this.wasmMemory.buffer, ptr, len);

        // Copy to ImageData
        this.imageData.data.set(wasmData);

        // Draw scaled image
        // First draw to a temp canvas at simulation resolution
        const tempCanvas = document.createElement('canvas');
        tempCanvas.width = this.simWidth;
        tempCanvas.height = this.simHeight;
        const tempCtx = tempCanvas.getContext('2d');
        tempCtx.putImageData(this.imageData, 0, 0);

        // Then scale up to display canvas
        this.ctx.imageSmoothingEnabled = false;
        this.ctx.drawImage(tempCanvas, 0, 0, this.canvas.width, this.canvas.height);

        // Update time display
        const time = this.simulation.get_time();
        document.getElementById('timeValue').textContent = time.toFixed(2);
    }

    animate() {
        if (this.simulation) {
            // Run simulation step
            this.simulation.step();

            // Render result
            this.render();
        }

        requestAnimationFrame(() => this.animate());
    }

    showError(message) {
        const container = document.querySelector('.canvas-container');
        container.innerHTML = `
            <div style="padding: 40px; text-align: center; color: #f87171;">
                <h3>エラー</h3>
                <p>${message}</p>
                <p style="font-size: 12px; margin-top: 10px;">
                    このアプリはWebAssemblyをサポートするブラウザが必要です。
                </p>
            </div>
        `;
    }
}

// Initialize app when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    new FluidApp();
});
