const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const source = fs.readFileSync(
    path.join(__dirname, "../src/gui/components/waveform_bins_canvas.rs"),
    "utf8",
);
const script = source.match(/const WAVEFORM_JS: &str = r#"([\s\S]*?)"#;/)[1];
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

function makeCanvas(width = 1000, height = 72) {
    const fills = [];
    const ctx = {
        fillStyle: "",
        scale() {},
        setTransform() {},
        clearRect() {},
        beginPath() {},
        roundRect() {},
        rect() {},
        fill() {},
        fillRect(x, y, w, h) {
            fills.push({ x, y, w, h, color: this.fillStyle });
        },
    };
    const canvas = {
        clientWidth: width,
        clientHeight: height,
        width: 0,
        height: 0,
        getContext: () => ctx,
    };

    return { canvas, ctx, fills };
}

async function setup(data, options = {}) {
    const { canvas, ctx, fills } = makeCanvas(options.width, options.height);
    const window = {
        devicePixelRatio: 1,
        waveZoom: 1,
        waveScroll: 0,
        ...options.window,
    };
    const document = {
        getElementById: (id) => (id === "waveform-canvas" ? canvas : null),
    };
    const sent = [];
    const frames = [];
    let frameId = 0;

    const requestAnimationFrame = (callback) => {
        frames.push(callback);
        return ++frameId;
    };
    const cancelAnimationFrame = () => {};

    const dioxus = {
        recv: async () => data,
        send: (value) => sent.push(value),
    };

    await new AsyncFunction(
        "dioxus",
        "window",
        "document",
        "requestAnimationFrame",
        "cancelAnimationFrame",
        script,
    )(dioxus, window, document, requestAnimationFrame, cancelAnimationFrame);

    Object.assign(window, options.window || {});

    return { window, canvas, ctx, fills, sent, frames, dioxus };
}

function encodeBins(bins) {
    return Buffer.from(Int8Array.from(bins)).toString("base64");
}

function peakData(bins, totalFrames, sampleRate = 44100, binFrames = 128) {
    return {
        duration: totalFrames / sampleRate,
        reset: true,
        overview: [],
        waveformPeaks: {
            encoded: encodeBins(bins),
            totalFrames,
            binFrames,
            sampleRate,
        },
    };
}

function constantBins(binCount, min = -50, max = 50) {
    const bins = new Array(binCount * 2);

    for (let index = 0; index < binCount; index++) {
        bins[index * 2] = min;
        bins[index * 2 + 1] = max;
    }

    return bins;
}

function coveredColumns(fills) {
    return new Set(fills.map((fill) => Math.floor(fill.x)));
}

function peakColumn(fills) {
    const heights = new Map();

    for (const fill of fills) {
        const x = Math.round(fill.x);
        heights.set(x, Math.max(heights.get(x) || 0, fill.h));
    }

    let bestX = 0;
    let bestHeight = -1;

    for (const [x, height] of heights) {
        if (height > bestHeight) {
            bestHeight = height;
            bestX = x;
        }
    }

    return bestX;
}

function render(state) {
    state.fills.length = 0;
    state.window.waveformState.appearStarted = -10000;
    state.window.waveformRenderFrame();
}

test("buildPeakLevels builds paired min/max mip levels", async () => {
    const state = await setup({ duration: 10, reset: true, overview: [] });
    const level0 = Int8Array.from([
        -127, 127, -64, 64, -32, 32, -16, 16,
        -8, 8, -4, 4, -2, 2, -1, 1,
    ]);

    const pyramid = state.window.waveformBuildPeakLevels(level0, 128);

    assert.equal(pyramid.levels.length, 3);
    assert.deepEqual(Array.from(pyramid.binFrames), [128, 256, 512]);
    assert.deepEqual(
        Array.from(pyramid.levels[1]),
        [-127, 127, -32, 32, -8, 8, -2, 2],
    );
    assert.deepEqual(Array.from(pyramid.levels[2]), [-127, 127, -8, 8]);
});

test("choosePeakLevel picks the coarsest level within one pixel", async () => {
    const state = await setup({ duration: 10, reset: true, overview: [] });
    const binFrames = [128, 256, 512];

    assert.equal(state.window.waveformChoosePeakLevel(binFrames, 100), -1);
    assert.equal(state.window.waveformChoosePeakLevel(binFrames, 128), 0);
    assert.equal(state.window.waveformChoosePeakLevel(binFrames, 300), 1);
    assert.equal(state.window.waveformChoosePeakLevel(binFrames, 1024), 2);
});

test("aggregatePeakLevel combines 1-2 bins per pixel", async () => {
    const state = await setup({ duration: 10, reset: true, overview: [] });
    const level = Int8Array.from([-100, 20, -10, 80, -40, 40, -5, 5]);

    const aggregate = state.window.waveformAggregatePeakLevel(
        level,
        256,
        0,
        512,
    );

    assert.equal(aggregate.min, -100 / 127);
    assert.equal(aggregate.max, 80 / 127);
    assert.equal(aggregate.count, 2);
});

test("peak pyramid renders one bar per canvas column", async () => {
    const totalFrames = 128_000;
    const state = await setup(peakData(constantBins(1000), totalFrames));

    render(state);

    assert.equal(coveredColumns(state.fills).size, 1000);
    assert.equal(state.window.waveformStats.framesPerPixel, 128);
    assert.equal(state.window.waveformStats.lod, 0);
    assert.equal(state.window.waveformStats.fallbackColumns, 0);
});

test("overview fills the whole viewport before the pyramid arrives", async () => {
    const state = await setup({
        duration: 120,
        reset: true,
        overview: new Array(500).fill(0.5),
        waveformPeaks: null,
    });

    render(state);

    const columns = [...coveredColumns(state.fills)].sort((a, b) => a - b);

    assert.ok(state.fills.length >= 450);
    assert.equal(columns[0], 0);
    assert.ok(columns[columns.length - 1] >= 996);
    assert.equal(state.window.waveformStats.lod, -1);
    assert.equal(state.window.waveformStats.fallbackColumns, 1000);
});

test("far zoom keeps the coarse overview before switching to mip", async () => {
    const totalFrames = 1_280_000;
    const data = peakData(constantBins(10_000), totalFrames);
    data.overview = new Array(500).fill(0.5);

    const state = await setup(data, {
        window: { waveZoom: 1, waveScroll: 0 },
    });

    render(state);

    assert.equal(state.window.waveformStats.lod, -1);
    assert.equal(state.window.waveformStats.fallbackColumns, 1000);
    assert.equal(state.window.waveformStats.detailColumns, 0);

    state.window.waveZoom = 8;
    render(state);

    assert.ok(state.window.waveformStats.lod >= 0);
    assert.equal(state.window.waveformStats.fallbackColumns, 0);
});

test("placeholder covers the full width when no waveform data exists", async () => {
    const state = await setup({
        duration: 120,
        reset: true,
        overview: [],
        waveformPeaks: null,
    });

    render(state);

    assert.equal(state.fills.length, 1);
    assert.equal(state.fills[0].x, 0);
    assert.equal(state.fills[0].w, 1000);
    assert.equal(state.window.waveformStats.fallbackColumns, 1000);
});

test("partial detail window keeps every column filled by the mip layer", async () => {
    const totalFrames = 8000;
    const state = await setup(peakData(constantBins(63, -10, 10), totalFrames));

    state.window.waveformAddDetailWindow({
        requestId: 1,
        startFrame: 0,
        endFrame: 4000,
        totalFrames,
        sampleRate: 44100,
        points: new Array(512).fill({ min: -0.8, max: 0.8 }),
    });

    render(state);

    assert.equal(coveredColumns(state.fills).size, 1000);
    assert.equal(state.window.waveformStats.detailColumns, 500);
    assert.equal(state.window.waveformStats.fallbackColumns, 0);
});

test("local window is requested only outside the loaded LRU window", async () => {
    const totalFrames = 8000;
    const state = await setup(peakData(constantBins(63), totalFrames), {
        window: { waveZoom: 100, waveScroll: 0.5 },
    });

    const request = state.window.waveformPlanLocalWindow();

    assert.ok(request);
    assert.ok(request.startFrame < request.endFrame);
    assert.equal(request.points, 4096);

    state.window.waveformAddDetailWindow({
        requestId: 1,
        startFrame: 3950,
        endFrame: 4050,
        totalFrames,
        sampleRate: 44100,
        points: new Array(4096).fill({ min: -0.5, max: 0.5 }),
    });

    assert.equal(state.window.waveformPlanLocalWindow(), null);

    state.window.waveScroll = 0;
    assert.ok(state.window.waveformPlanLocalWindow());

    for (let index = 2; index <= 5; index++) {
        state.window.waveformAddDetailWindow({
            requestId: index,
            startFrame: index * 100,
            endFrame: index * 100 + 10,
            totalFrames,
            sampleRate: 44100,
            points: new Array(64).fill({ min: -0.5, max: 0.5 }),
        });
    }

    assert.equal(state.window.waveformState.windows.length, 4);
});

test("detail window with integer rounding is still rendered", async () => {
    const totalFrames = 1_280_000;
    const state = await setup(peakData(constantBins(10_000), totalFrames), {
        window: { waveZoom: 11, waveScroll: 0.5 },
    });

    const plan = state.window.waveformPlanLocalWindow();

    assert.ok(plan, "extreme zoom should plan a local window");

    state.window.waveformAddDetailWindow({
        requestId: 1,
        startFrame: plan.startFrame,
        endFrame: plan.endFrame,
        totalFrames,
        sampleRate: 44100,
        points: new Array(plan.points).fill({ min: -0.8, max: 0.8 }),
    });

    assert.equal(state.window.waveformPlanLocalWindow(), null);

    render(state);

    assert.equal(coveredColumns(state.fills).size, 1000);
    assert.equal(state.window.waveformStats.detailColumns, 1000);
});

test("sample-level window is used when a pixel covers less than a frame", async () => {
    const totalFrames = 8000;
    const state = await setup(peakData(constantBins(63), totalFrames), {
        window: { waveZoom: 10_000, waveScroll: 0.5 },
    });

    state.window.waveformAddDetailWindow({
        requestId: 1,
        startFrame: 3990,
        endFrame: 4010,
        totalFrames,
        sampleRate: 44100,
        points: new Array(20).fill({ min: -0.8, max: 0.8 }),
    });

    assert.equal(state.window.waveformPlanLocalWindow(), null);

    render(state);

    assert.equal(coveredColumns(state.fills).size, 1000);
    assert.equal(state.window.waveformStats.detailColumns, 1000);
});

test("wheel and pan move the viewport without sending loading commands", async () => {
    const totalFrames = 8000;
    const state = await setup(peakData(constantBins(63), totalFrames));

    state.sent.length = 0;

    const zoomed = state.window.handleWaveWheel(0.5, 0, -120, false);
    const panned = state.window.handleWaveWheel(0.5, 120, 0, false);

    state.window.panBy(20);

    assert.equal(state.sent.length, 0);
    assert.ok(zoomed.zoom > 1);
    assert.equal(panned.zoom, zoomed.zoom);
    assert.notEqual(state.window.waveScroll, 0);
});

test("stale viewport echoes do not roll back wheel zoom", async () => {
    const totalFrames = 128_000;
    const state = await setup(peakData(constantBins(1000), totalFrames));

    state.window.waveZoom = 1;
    state.window.waveScroll = 0;

    state.window.handleWaveWheel(0.5, 0, -100, false);

    const afterWheel = state.window.waveZoom;

    assert.ok(afterWheel > 1);

    state.window.setWaveViewport(1, 50);

    assert.equal(state.window.waveZoom, afterWheel);

    state.window.waveformLastWheelAt = performance.now() - 1000;
    state.window.setWaveViewport(3.5, 50);

    assert.equal(state.window.waveZoom, 3.5);
});

test("known marker stays aligned across mip levels and local detail", async () => {
    const totalFrames = 1_000_000;
    const binCount = Math.ceil(totalFrames / 128);
    const bins = constantBins(binCount, -5, 5);
    const markerFrame = 500_000;

    for (let frame = markerFrame - 256; frame < markerFrame + 256; frame += 128) {
        const bin = Math.floor(frame / 128);
        bins[bin * 2] = -120;
        bins[bin * 2 + 1] = 120;
    }

    const coarse = await setup(peakData(bins, totalFrames), {
        window: { waveZoom: 1, waveScroll: 0 },
    });

    render(coarse);

    const coarseColumn = peakColumn(coarse.fills);

    const detailed = await setup(peakData(bins, totalFrames), {
        window: { waveZoom: 10, waveScroll: 0.5 },
    });

    render(detailed);

    const detailedColumn = peakColumn(detailed.fills);

    assert.ok(
        Math.abs(coarseColumn - 500) <= 2,
        `coarse marker column ${coarseColumn}`,
    );
    assert.ok(
        Math.abs(detailedColumn - 500) <= 3,
        `detailed marker column ${detailedColumn}`,
    );
});

test("first and last pixels stay within the track bounds", async () => {
    const totalFrames = 128_000;
    const state = await setup(peakData(constantBins(1000), totalFrames), {
        window: { waveZoom: 4, waveScroll: 1 },
    });

    render(state);

    const columns = [...coveredColumns(state.fills)].sort((a, b) => a - b);

    assert.equal(columns[0], 0);
    assert.equal(columns[columns.length - 1], 999);
    assert.equal(columns.length, 1000);
});
