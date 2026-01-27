/**
 * Draw in Flow - 2D流体可視化アプリ
 * パーティクルベースの疑似流体シミュレーション
 */

class FluidSimulation {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');

        // キャンバスサイズ設定
        this.resizeCanvas();
        window.addEventListener('resize', () => this.resizeCanvas());

        // パラメータ
        this.params = {
            brushSize: 15,
            flowSpeed: 2.0,
            particleCount: 1500,
            particleSize: 2,
            obstacleInfluenceRadius: 80,
            trailLength: 8
        };

        // 状態
        this.isDrawing = false;
        this.isPaused = false;
        this.lastMousePos = null;

        // 障害物データ（グリッドベースで高速検索）
        this.obstacles = new Set();
        this.obstacleGrid = new Map();
        this.gridCellSize = 10;

        // パーティクル配列
        this.particles = [];

        // 初期化
        this.initParticles();
        this.setupEventListeners();
        this.setupControls();

        // アニメーション開始
        this.animate();
    }

    resizeCanvas() {
        const maxWidth = Math.min(window.innerWidth - 40, 1000);
        const maxHeight = Math.min(window.innerHeight - 300, 600);
        this.canvas.width = maxWidth;
        this.canvas.height = maxHeight;

        // パーティクルを再初期化（キャンバスサイズ変更時）
        if (this.particles.length > 0) {
            this.initParticles();
        }
    }

    initParticles() {
        this.particles = [];
        for (let i = 0; i < this.params.particleCount; i++) {
            this.particles.push(this.createParticle());
        }
    }

    createParticle(x = null) {
        return {
            x: x !== null ? x : Math.random() * this.canvas.width,
            y: Math.random() * this.canvas.height,
            vx: this.params.flowSpeed,
            vy: 0,
            trail: [],
            hue: 200 + Math.random() * 40, // 青系の色相
            alpha: 0.6 + Math.random() * 0.4
        };
    }

    // グリッドセルのキーを取得
    getGridKey(x, y) {
        const cellX = Math.floor(x / this.gridCellSize);
        const cellY = Math.floor(y / this.gridCellSize);
        return `${cellX},${cellY}`;
    }

    // 障害物点を追加
    addObstaclePoint(x, y) {
        const key = `${Math.round(x)},${Math.round(y)}`;
        if (this.obstacles.has(key)) return;

        this.obstacles.add(key);

        // グリッドに追加
        const gridKey = this.getGridKey(x, y);
        if (!this.obstacleGrid.has(gridKey)) {
            this.obstacleGrid.set(gridKey, []);
        }
        this.obstacleGrid.get(gridKey).push({ x: Math.round(x), y: Math.round(y) });
    }

    // ブラシで障害物を描画
    drawObstacle(x, y) {
        const radius = this.params.brushSize;
        const step = 3; // 点の間隔

        for (let dx = -radius; dx <= radius; dx += step) {
            for (let dy = -radius; dy <= radius; dy += step) {
                if (dx * dx + dy * dy <= radius * radius) {
                    const px = x + dx;
                    const py = y + dy;
                    if (px >= 0 && px < this.canvas.width && py >= 0 && py < this.canvas.height) {
                        this.addObstaclePoint(px, py);
                    }
                }
            }
        }
    }

    // 線上に障害物を描画（ドラッグ時）
    drawObstacleLine(x1, y1, x2, y2) {
        const dist = Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
        const steps = Math.max(1, Math.floor(dist / 3));

        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            const x = x1 + (x2 - x1) * t;
            const y = y1 + (y2 - y1) * t;
            this.drawObstacle(x, y);
        }
    }

    // 近くの障害物点を取得
    getNearbyObstacles(x, y, radius) {
        const nearby = [];
        const cellRadius = Math.ceil(radius / this.gridCellSize);
        const centerCellX = Math.floor(x / this.gridCellSize);
        const centerCellY = Math.floor(y / this.gridCellSize);

        for (let dx = -cellRadius; dx <= cellRadius; dx++) {
            for (let dy = -cellRadius; dy <= cellRadius; dy++) {
                const gridKey = `${centerCellX + dx},${centerCellY + dy}`;
                const points = this.obstacleGrid.get(gridKey);
                if (points) {
                    for (const point of points) {
                        const dist = Math.sqrt((point.x - x) ** 2 + (point.y - y) ** 2);
                        if (dist < radius && dist > 0) {
                            nearby.push({ ...point, dist });
                        }
                    }
                }
            }
        }

        return nearby;
    }

    // パーティクルの速度を計算（ポテンシャル流れの近似）
    calculateVelocity(particle) {
        // 基本の一様流
        let vx = this.params.flowSpeed;
        let vy = 0;

        // 近くの障害物からの影響を計算
        const nearby = this.getNearbyObstacles(
            particle.x,
            particle.y,
            this.params.obstacleInfluenceRadius
        );

        if (nearby.length === 0) {
            return { vx, vy };
        }

        // 各障害物点からの影響を重ね合わせる
        // ポテンシャル流れの簡易近似: 障害物周りで流線が曲がる
        let totalInfluenceX = 0;
        let totalInfluenceY = 0;
        let totalWeight = 0;

        for (const obs of nearby) {
            const dx = particle.x - obs.x;
            const dy = particle.y - obs.y;
            const distSq = dx * dx + dy * dy;
            const dist = Math.sqrt(distSq);

            // 影響の強さ（距離の2乗に反比例）
            const influence = 1 / (distSq + 1);
            const weight = influence;

            // 法線方向（障害物から離れる方向）
            const nx = dx / dist;
            const ny = dy / dist;

            // 接線方向（流れに沿う方向）
            // 一様流の方向(1, 0)と法線の外積で決定
            const tangentSign = ny >= 0 ? 1 : -1;
            const tx = -ny * tangentSign;
            const ty = nx * tangentSign;

            // 障害物に近いほど接線方向の成分を強くする
            const deflectionStrength = Math.exp(-dist / 30) * 3;

            // 法線方向の反発（障害物に入らないように）
            const repulsionStrength = Math.exp(-dist / 15) * 2;

            totalInfluenceX += (tx * deflectionStrength + nx * repulsionStrength) * weight;
            totalInfluenceY += (ty * deflectionStrength + ny * repulsionStrength) * weight;
            totalWeight += weight;
        }

        if (totalWeight > 0) {
            totalInfluenceX /= totalWeight;
            totalInfluenceY /= totalWeight;

            // 影響を速度に加える
            vx += totalInfluenceX;
            vy += totalInfluenceY;

            // 速度の大きさを保存（流体の非圧縮性の近似）
            const speed = Math.sqrt(vx * vx + vy * vy);
            const targetSpeed = this.params.flowSpeed * 1.2;
            if (speed > 0.1) {
                vx = (vx / speed) * Math.min(speed, targetSpeed);
                vy = (vy / speed) * Math.min(speed, targetSpeed);
            }
        }

        return { vx, vy };
    }

    // パーティクルが障害物内にあるかチェック
    isInsideObstacle(x, y) {
        const nearby = this.getNearbyObstacles(x, y, 10);
        return nearby.some(obs => obs.dist < 5);
    }

    // パーティクルを更新
    updateParticles() {
        for (const particle of this.particles) {
            // 軌跡を保存
            particle.trail.push({ x: particle.x, y: particle.y });
            if (particle.trail.length > this.params.trailLength) {
                particle.trail.shift();
            }

            // 速度を計算
            const { vx, vy } = this.calculateVelocity(particle);
            particle.vx = vx;
            particle.vy = vy;

            // 位置を更新
            particle.x += particle.vx;
            particle.y += particle.vy;

            // 障害物内に入った場合は押し出す
            if (this.isInsideObstacle(particle.x, particle.y)) {
                const nearby = this.getNearbyObstacles(particle.x, particle.y, 20);
                if (nearby.length > 0) {
                    // 最も近い障害物から離れる方向に押し出す
                    let avgDx = 0, avgDy = 0;
                    for (const obs of nearby) {
                        avgDx += particle.x - obs.x;
                        avgDy += particle.y - obs.y;
                    }
                    const len = Math.sqrt(avgDx * avgDx + avgDy * avgDy);
                    if (len > 0) {
                        particle.x += (avgDx / len) * 5;
                        particle.y += (avgDy / len) * 5;
                    }
                }
            }

            // 画面端の処理
            if (particle.x > this.canvas.width + 10) {
                // 右端を出たら左端に戻す
                particle.x = -5;
                particle.y = Math.random() * this.canvas.height;
                particle.trail = [];
            }
            if (particle.x < -20) {
                particle.x = -5;
                particle.y = Math.random() * this.canvas.height;
                particle.trail = [];
            }
            if (particle.y < -20 || particle.y > this.canvas.height + 20) {
                particle.x = -5;
                particle.y = Math.random() * this.canvas.height;
                particle.trail = [];
            }
        }
    }

    // 描画
    render() {
        // 背景をクリア（軌跡効果のため半透明）
        this.ctx.fillStyle = 'rgba(10, 10, 26, 0.15)';
        this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

        // 障害物を描画
        this.ctx.fillStyle = '#404060';
        for (const points of this.obstacleGrid.values()) {
            for (const point of points) {
                this.ctx.fillRect(point.x - 1, point.y - 1, 3, 3);
            }
        }

        // パーティクルと軌跡を描画
        for (const particle of this.particles) {
            // 軌跡を描画
            if (particle.trail.length > 1) {
                this.ctx.beginPath();
                this.ctx.moveTo(particle.trail[0].x, particle.trail[0].y);

                for (let i = 1; i < particle.trail.length; i++) {
                    this.ctx.lineTo(particle.trail[i].x, particle.trail[i].y);
                }
                this.ctx.lineTo(particle.x, particle.y);

                // 速度に基づいて色を変化させる
                const speed = Math.sqrt(particle.vx ** 2 + particle.vy ** 2);
                const hue = particle.hue - (speed - this.params.flowSpeed) * 20;

                this.ctx.strokeStyle = `hsla(${hue}, 80%, 60%, ${particle.alpha * 0.5})`;
                this.ctx.lineWidth = this.params.particleSize;
                this.ctx.lineCap = 'round';
                this.ctx.stroke();
            }

            // パーティクル本体
            const speed = Math.sqrt(particle.vx ** 2 + particle.vy ** 2);
            const hue = particle.hue - (speed - this.params.flowSpeed) * 20;

            this.ctx.beginPath();
            this.ctx.arc(particle.x, particle.y, this.params.particleSize, 0, Math.PI * 2);
            this.ctx.fillStyle = `hsla(${hue}, 90%, 70%, ${particle.alpha})`;
            this.ctx.fill();
        }
    }

    // アニメーションループ
    animate() {
        if (!this.isPaused) {
            this.updateParticles();
            this.render();
        }
        requestAnimationFrame(() => this.animate());
    }

    // イベントリスナー設定
    setupEventListeners() {
        // マウスイベント
        this.canvas.addEventListener('mousedown', (e) => {
            this.isDrawing = true;
            const rect = this.canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            this.drawObstacle(x, y);
            this.lastMousePos = { x, y };
        });

        this.canvas.addEventListener('mousemove', (e) => {
            if (!this.isDrawing) return;
            const rect = this.canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;

            if (this.lastMousePos) {
                this.drawObstacleLine(this.lastMousePos.x, this.lastMousePos.y, x, y);
            }
            this.lastMousePos = { x, y };
        });

        this.canvas.addEventListener('mouseup', () => {
            this.isDrawing = false;
            this.lastMousePos = null;
        });

        this.canvas.addEventListener('mouseleave', () => {
            this.isDrawing = false;
            this.lastMousePos = null;
        });

        // タッチイベント（モバイル対応）
        this.canvas.addEventListener('touchstart', (e) => {
            e.preventDefault();
            this.isDrawing = true;
            const rect = this.canvas.getBoundingClientRect();
            const touch = e.touches[0];
            const x = touch.clientX - rect.left;
            const y = touch.clientY - rect.top;
            this.drawObstacle(x, y);
            this.lastMousePos = { x, y };
        });

        this.canvas.addEventListener('touchmove', (e) => {
            e.preventDefault();
            if (!this.isDrawing) return;
            const rect = this.canvas.getBoundingClientRect();
            const touch = e.touches[0];
            const x = touch.clientX - rect.left;
            const y = touch.clientY - rect.top;

            if (this.lastMousePos) {
                this.drawObstacleLine(this.lastMousePos.x, this.lastMousePos.y, x, y);
            }
            this.lastMousePos = { x, y };
        });

        this.canvas.addEventListener('touchend', () => {
            this.isDrawing = false;
            this.lastMousePos = null;
        });
    }

    // コントロール設定
    setupControls() {
        // ブラシサイズ
        const brushSizeSlider = document.getElementById('brushSize');
        const brushSizeValue = document.getElementById('brushSizeValue');
        brushSizeSlider.addEventListener('input', (e) => {
            this.params.brushSize = parseInt(e.target.value);
            brushSizeValue.textContent = e.target.value;
        });

        // 流速
        const flowSpeedSlider = document.getElementById('flowSpeed');
        const flowSpeedValue = document.getElementById('flowSpeedValue');
        flowSpeedSlider.addEventListener('input', (e) => {
            this.params.flowSpeed = parseFloat(e.target.value);
            flowSpeedValue.textContent = e.target.value;
        });

        // パーティクル密度
        const particleDensitySlider = document.getElementById('particleDensity');
        const particleDensityValue = document.getElementById('particleDensityValue');
        particleDensitySlider.addEventListener('input', (e) => {
            const newCount = parseInt(e.target.value);
            particleDensityValue.textContent = e.target.value;

            // パーティクル数を調整
            if (newCount > this.params.particleCount) {
                for (let i = this.params.particleCount; i < newCount; i++) {
                    this.particles.push(this.createParticle(-5));
                }
            } else {
                this.particles.length = newCount;
            }
            this.params.particleCount = newCount;
        });

        // クリアボタン
        const clearBtn = document.getElementById('clearBtn');
        clearBtn.addEventListener('click', () => {
            this.obstacles.clear();
            this.obstacleGrid.clear();
            // 画面をクリア
            this.ctx.fillStyle = 'rgba(10, 10, 26, 1)';
            this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
        });

        // 一時停止ボタン
        const pauseBtn = document.getElementById('pauseBtn');
        pauseBtn.addEventListener('click', () => {
            this.isPaused = !this.isPaused;
            pauseBtn.textContent = this.isPaused ? '再開' : '一時停止';
            pauseBtn.classList.toggle('paused', this.isPaused);
        });
    }

    // 障害物をクリア
    clearObstacles() {
        this.obstacles.clear();
        this.obstacleGrid.clear();
    }
}

// アプリケーション開始
document.addEventListener('DOMContentLoaded', () => {
    new FluidSimulation('fluidCanvas');
});
