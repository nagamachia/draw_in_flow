export class LBMSimulation {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        LBMSimulationFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_lbmsimulation_free(ptr, 0);
    }
    clear_obstacles() {
        wasm.lbmsimulation_clear_obstacles(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    get_colormap_origin() {
        const ret = wasm.lbmsimulation_get_colormap_origin(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_colormap_scale() {
        const ret = wasm.lbmsimulation_get_colormap_scale(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_reynolds() {
        const ret = wasm.lbmsimulation_get_reynolds(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_time() {
        const ret = wasm.lbmsimulation_get_time(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} norm_x
     * @param {number} norm_y
     * @returns {Float64Array}
     */
    get_velocity_at(norm_x, norm_y) {
        const ret = wasm.lbmsimulation_get_velocity_at(this.__wbg_ptr, norm_x, norm_y);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {number}
     */
    height() {
        const ret = wasm.lbmsimulation_height(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    image_ptr() {
        const ret = wasm.lbmsimulation_image_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {boolean}
     */
    is_paused() {
        const ret = wasm.lbmsimulation_is_paused(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} reynolds
     */
    constructor(reynolds) {
        const ret = wasm.lbmsimulation_new(reynolds);
        this.__wbg_ptr = ret;
        LBMSimulationFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {number}
     */
    obstacle_len() {
        const ret = wasm.lbmsimulation_obstacle_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    obstacle_ptr() {
        const ret = wasm.lbmsimulation_obstacle_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    regenerate_noise() {
        wasm.lbmsimulation_regenerate_noise(this.__wbg_ptr);
    }
    render() {
        wasm.lbmsimulation_render(this.__wbg_ptr);
    }
    /**
     * @param {number} origin
     */
    set_colormap_origin(origin) {
        wasm.lbmsimulation_set_colormap_origin(this.__wbg_ptr, origin);
    }
    /**
     * @param {number} scale
     */
    set_colormap_scale(scale) {
        wasm.lbmsimulation_set_colormap_scale(this.__wbg_ptr, scale);
    }
    /**
     * @param {number} mode
     */
    set_display_mode(mode) {
        wasm.lbmsimulation_set_display_mode(this.__wbg_ptr, mode);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {boolean} value
     */
    set_obstacle(x, y, value) {
        wasm.lbmsimulation_set_obstacle(this.__wbg_ptr, x, y, value);
    }
    /**
     * @param {boolean} paused
     */
    set_paused(paused) {
        wasm.lbmsimulation_set_paused(this.__wbg_ptr, paused);
    }
    /**
     * @param {number} reynolds
     */
    set_reynolds(reynolds) {
        wasm.lbmsimulation_set_reynolds(this.__wbg_ptr, reynolds);
    }
    step() {
        wasm.lbmsimulation_step(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    width() {
        const ret = wasm.lbmsimulation_width(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) LBMSimulation.prototype[Symbol.dispose] = LBMSimulation.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_bbadd78c1bac3a77: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./lbm_wasm_bg.js": import0,
    };
}

const LBMSimulationFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_lbmsimulation_free(ptr, 1));

function getArrayF64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedFloat64ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('lbm_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
