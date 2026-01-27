/**
 * Draw in Flow - 2D Fluid Visualization
 * パーティクルベースの疑似流体シミュレーション
 */

class FluidSimulation {
    constructor(canvas) {
        this.canvas = canvas;
        this.ctx = canvas.getContext('2d');

        // キャンバスサイズ設定
        this.resize();
        window.addEventListener('resize', () => this.resize());

        // シミュレーションパラメータ
        this.flowSpeed = 2.0;
        this.particleCount = 3000;
        this.brushSize = 15;
        this.isFlowing = true;

        // パーティクル配列
        this.particles = [];

        // 障害物マップ（高速な衝突検出用）
        this.obstacleMap = null;
        this.obstacleCanvas = null;
        this.obstacleCtx = null;

        // 速度場キャッシュ（パフォーマンス向上用）
        this.velocityField = null;
        this.fieldResolution = 4; // グリッド解像度（ピクセル単位）

        // マウス描画状態
        this.isDrawing = false;
        this.lastMousePos = null;

        // 初期化
        this.initObstacleCanvas();
        this.initParticles();
        this.setupEventListeners();

        // アニメーション開始
        this.animate();
    }

    resize() {
        // ウィンドウサイズに応じてキャンバスサイズを決定
        const maxWidth = Math.min(window.innerWidth - 40, 900);
        const maxHeight = Math.min(window.innerHeight - 280, 600);

        this.canvas.width = maxWidth;
        this.canvas.height = maxHeight;

        // 障害物キャンバスも同じサイズに
        if (this.obstacleCanvas) {
            this.obstacleCanvas.width = maxWidth;
            this.obstacleCanvas.height = maxHeight;
            this.rebuildVelocityField();
        }
    }

    initObstacleCanvas() {
        // 障害物を描画するためのオフスクリーンキャンバス
        this.obstacleCanvas = document.createElement('canvas');
        this.obstacleCanvas.width = this.canvas.width;
        this.obstacleCanvas.height = this.canvas.height;
        this.obstacleCtx = this.obstacleCanvas.getContext('2d');

        // 速度場の初期化
        this.rebuildVelocityField();
    }

    initParticles() {
        this.particles = [];
        for (let i = 0; i < this.particleCount; i++) {
            this.particles.push(this.createParticle());
        }
    }

    createParticle(fromLeft = false) {
        return {
            x: fromLeft ? -Math.random() * 50 : Math.random() * this.canvas.width,
            y: Math.random() * this.canvas.height,
            vx: 0,
            vy: 0,
            age: 0,
            maxAge: 200 + Math.random() * 300,
            size: 1 + Math.random() * 1.5,
            alpha: 0.3 + Math.random() * 0.5
        };
    }

    setupEventListeners() {
        // マウスイベント
        this.canvas.addEventListener('mousedown', (e) => this.startDrawing(e));
        this.canvas.addEventListener('mousemove', (e) => this.draw(e));
        this.canvas.addEventListener('mouseup', () => this.stopDrawing());
        this.canvas.addEventListener('mouseleave', () => this.stopDrawing());

        // タッチイベント
        this.canvas.addEventListener('touchstart', (e) => {
            e.preventDefault();
            this.startDrawing(e.touches[0]);
        });
        this.canvas.addEventListener('touchmove', (e) => {
            e.preventDefault();
            this.draw(e.touches[0]);
        });
        this.canvas.addEventListener('touchend', () => this.stopDrawing());

        // コントロールボタン
        document.getElementById('clearBtn').addEventListener('click', () => this.clearObstacles());
        document.getElementById('toggleFlowBtn').addEventListener('click', () => this.toggleFlow());

        // スライダー
        document.getElementById('brushSize').addEventListener('input', (e) => {
            this.brushSize = parseInt(e.target.value);
            document.getElementById('brushSizeValue').textContent = this.brushSize;
        });

        document.getElementById('flowSpeed').addEventListener('input', (e) => {
            this.flowSpeed = parseFloat(e.target.value);
            document.getElementById('flowSpeedValue').textContent = this.flowSpeed.toFixed(1);
        });

        document.getElementById('particleCount').addEventListener('input', (e) => {
            this.particleCount = parseInt(e.target.value);
            document.getElementById('particleCountValue').textContent = this.particleCount;
            this.adjustParticleCount();
        });
    }

    getMousePos(e) {
        const rect = this.canvas.getBoundingClientRect();
        return {
            x: e.clientX - rect.left,
            y: e.clientY - rect.top
        };
    }

    startDrawing(e) {
        this.isDrawing = true;
        this.lastMousePos = this.getMousePos(e);
        this.drawObstacle(this.lastMousePos.x, this.lastMousePos.y);
    }

    draw(e) {
        if (!this.isDrawing) return;

        const pos = this.getMousePos(e);

        // 前の位置から現在の位置まで線を描画
        if (this.lastMousePos) {
            this.drawLine(this.lastMousePos.x, this.lastMousePos.y, pos.x, pos.y);
        }

        this.lastMousePos = pos;
    }

    stopDrawing() {
        this.isDrawing = false;
        this.lastMousePos = null;
        // 描画が終わったら速度場を再計算
        this.rebuildVelocityField();
    }

    drawObstacle(x, y) {
        this.obstacleCtx.fillStyle = '#ffffff';
        this.obstacleCtx.beginPath();
        this.obstacleCtx.arc(x, y, this.brushSize, 0, Math.PI * 2);
        this.obstacleCtx.fill();
    }

    drawLine(x1, y1, x2, y2) {
        const dist = Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
        const steps = Math.max(1, Math.floor(dist / (this.brushSize / 2)));

        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            const x = x1 + (x2 - x1) * t;
            const y = y1 + (y2 - y1) * t;
            this.drawObstacle(x, y);
        }
    }

    clearObstacles() {
        this.obstacleCtx.clearRect(0, 0, this.obstacleCanvas.width, this.obstacleCanvas.height);
        this.rebuildVelocityField();
    }

    toggleFlow() {
        this.isFlowing = !this.isFlowing;
        document.getElementById('toggleFlowBtn').textContent = this.isFlowing ? '流れ停止' : '流れ開始';
    }

    adjustParticleCount() {
        while (this.particles.length < this.particleCount) {
            this.particles.push(this.createParticle());
        }
        while (this.particles.length > this.particleCount) {
            this.particles.pop();
        }
    }

    rebuildVelocityField() {
        // 障害物マップを取得
        const imageData = this.obstacleCtx.getImageData(
            0, 0, this.obstacleCanvas.width, this.obstacleCanvas.height
        );
        this.obstacleData = imageData.data;

        // 速度場を計算（低解像度グリッド）
        const gridW = Math.ceil(this.canvas.width / this.fieldResolution);
        const gridH = Math.ceil(this.canvas.height / this.fieldResolution);

        this.velocityField = new Float32Array(gridW * gridH * 2);

        for (let gy = 0; gy < gridH; gy++) {
            for (let gx = 0; gx < gridW; gx++) {
                const x = gx * this.fieldResolution;
                const y = gy * this.fieldResolution;

                const vel = this.calculateVelocity(x, y);
                const idx = (gy * gridW + gx) * 2;
                this.velocityField[idx] = vel.vx;
                this.velocityField[idx + 1] = vel.vy;
            }
        }

        this.gridWidth = gridW;
        this.gridHeight = gridH;
    }

    isObstacle(x, y) {
        if (x < 0 || x >= this.canvas.width || y < 0 || y >= this.canvas.height) {
            return false;
        }
        const idx = (Math.floor(y) * this.obstacleCanvas.width + Math.floor(x)) * 4;
        return this.obstacleData[idx + 3] > 128; // アルファ値でチェック
    }

    calculateVelocity(x, y) {
        // 基本の一様流（左から右）
        let vx = this.flowSpeed;
        let vy = 0;

        // 障害物の影響を計算
        const searchRadius = 60;
        let totalForceX = 0;
        let totalForceY = 0;
        let obstacleFound = false;

        // 周囲の障害物をサンプリング
        for (let angle = 0; angle < Math.PI * 2; angle += Math.PI / 8) {
            for (let dist = 5; dist <= searchRadius; dist += 5) {
                const checkX = x + Math.cos(angle) * dist;
                const checkY = y + Math.sin(angle) * dist;

                if (this.isObstacle(checkX, checkY)) {
                    obstacleFound = true;
                    // 障害物から離れる方向への力
                    const strength = 1.0 / (dist * dist) * 500;
                    totalForceX -= Math.cos(angle) * strength;
                    totalForceY -= Math.sin(angle) * strength;
                }
            }
        }

        if (obstacleFound) {
            // 障害物周りで流れを曲げる
            // 渦効果を加えて自然な回り込みを表現
            const perpX = -totalForceY;
            const perpY = totalForceX;

            vx += totalForceX * 0.3 + perpX * 0.2;
            vy += totalForceY * 0.3 + perpY * 0.2;

            // 正規化して速度を保つ
            const mag = Math.sqrt(vx * vx + vy * vy);
            if (mag > 0) {
                vx = (vx / mag) * this.flowSpeed;
                vy = (vy / mag) * this.flowSpeed;
            }
        }

        return { vx, vy };
    }

    getVelocityAt(x, y) {
        if (!this.velocityField) {
            return { vx: this.flowSpeed, vy: 0 };
        }

        // グリッド座標
        const gx = x / this.fieldResolution;
        const gy = y / this.fieldResolution;

        // バイリニア補間
        const gx0 = Math.floor(gx);
        const gy0 = Math.floor(gy);
        const gx1 = Math.min(gx0 + 1, this.gridWidth - 1);
        const gy1 = Math.min(gy0 + 1, this.gridHeight - 1);

        const fx = gx - gx0;
        const fy = gy - gy0;

        const idx00 = (gy0 * this.gridWidth + gx0) * 2;
        const idx10 = (gy0 * this.gridWidth + gx1) * 2;
        const idx01 = (gy1 * this.gridWidth + gx0) * 2;
        const idx11 = (gy1 * this.gridWidth + gx1) * 2;

        const vx = (1 - fx) * (1 - fy) * this.velocityField[idx00] +
                   fx * (1 - fy) * this.velocityField[idx10] +
                   (1 - fx) * fy * this.velocityField[idx01] +
                   fx * fy * this.velocityField[idx11];

        const vy = (1 - fx) * (1 - fy) * this.velocityField[idx00 + 1] +
                   fx * (1 - fy) * this.velocityField[idx10 + 1] +
                   (1 - fx) * fy * this.velocityField[idx01 + 1] +
                   fx * fy * this.velocityField[idx11 + 1];

        return { vx, vy };
    }

    updateParticles() {
        if (!this.isFlowing) return;

        for (let i = 0; i < this.particles.length; i++) {
            const p = this.particles[i];

            // 速度場から速度を取得
            const vel = this.getVelocityAt(p.x, p.y);

            // 障害物内にいる場合は押し出す
            if (this.isObstacle(p.x, p.y)) {
                // 最寄りの非障害物位置を探す
                let found = false;
                for (let dist = 1; dist < 50 && !found; dist += 2) {
                    for (let angle = 0; angle < Math.PI * 2; angle += Math.PI / 4) {
                        const newX = p.x + Math.cos(angle) * dist;
                        const newY = p.y + Math.sin(angle) * dist;
                        if (!this.isObstacle(newX, newY)) {
                            p.x = newX;
                            p.y = newY;
                            found = true;
                            break;
                        }
                    }
                }
                if (!found) {
                    // どうしても見つからない場合はリセット
                    Object.assign(p, this.createParticle(true));
                    continue;
                }
            }

            // 速度を適用（少しの慣性を加える）
            p.vx = p.vx * 0.9 + vel.vx * 0.1;
            p.vy = p.vy * 0.9 + vel.vy * 0.1;

            // 位置更新
            p.x += p.vx;
            p.y += p.vy;
            p.age++;

            // 画面外または寿命が尽きたらリセット
            if (p.x > this.canvas.width + 10 ||
                p.x < -50 ||
                p.y < -10 ||
                p.y > this.canvas.height + 10 ||
                p.age > p.maxAge) {
                Object.assign(p, this.createParticle(true));
            }
        }
    }

    render() {
        const ctx = this.ctx;

        // 背景をフェードアウトで描画（軌跡効果）
        ctx.fillStyle = 'rgba(10, 10, 26, 0.15)';
        ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

        // パーティクルを描画
        for (const p of this.particles) {
            // 速度に基づいて色を決定
            const speed = Math.sqrt(p.vx * p.vx + p.vy * p.vy);
            const normalizedSpeed = Math.min(speed / (this.flowSpeed * 1.5), 1);

            // 速度に応じた色（青→シアン→白）
            const r = Math.floor(100 + normalizedSpeed * 155);
            const g = Math.floor(180 + normalizedSpeed * 75);
            const b = 255;

            ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${p.alpha})`;
            ctx.beginPath();
            ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
            ctx.fill();
        }

        // 障害物を描画（半透明で表示）
        ctx.save();
        ctx.globalAlpha = 0.8;
        ctx.drawImage(this.obstacleCanvas, 0, 0);
        ctx.restore();

        // 障害物に輪郭効果を追加
        ctx.save();
        ctx.globalCompositeOperation = 'source-over';
        ctx.shadowColor = '#00d4ff';
        ctx.shadowBlur = 15;
        ctx.globalAlpha = 0.3;
        ctx.drawImage(this.obstacleCanvas, 0, 0);
        ctx.restore();
    }

    animate() {
        this.updateParticles();
        this.render();
        requestAnimationFrame(() => this.animate());
    }
}

// アプリケーション初期化
document.addEventListener('DOMContentLoaded', () => {
    const canvas = document.getElementById('fluidCanvas');
    new FluidSimulation(canvas);
});
