const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const source = fs.readFileSync(
    path.join(__dirname, "../src/gui/components/beat_grid.rs"),
    "utf8",
);
const script = source.match(/const BEAT_GRID_JS: &str = r#"([\s\S]*?)"#;/)[1];
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

async function setup(overrides = {}, viewport = {}) {
    const strokes = [];
    let x;
    const ctx = {
        scale() {},
        clearRect() { strokes.length = 0; },
        beginPath() {},
        moveTo(value) { x = value; },
        lineTo() {},
        stroke() { strokes.push({ x, color: this.strokeStyle, width: this.lineWidth }); },
    };
    const canvas = {
        clientWidth: 1000,
        clientHeight: 112,
        getContext: () => ctx,
    };
    const window = { waveZoom: 20, waveScroll: 40 / 114, devicePixelRatio: 1, ...viewport };
    const data = {
        bpm: 127,
        tempo: 127.4,
        duration: 120,
        snap: true,
        offset: 0.137,
        shift: 0,
        beats: [0.137, 5.137],
        bars: [0.137, 2.137],
        theme: "dark",
        ...overrides,
    };
    const document = { getElementById: () => canvas };
    await new AsyncFunction("dioxus", "window", "document", script)(
        { recv: async () => data }, window, document,
    );
    return { window, canvas, strokes, data };
}

function drawnTimes(state) {
    const visible = state.data.duration / state.window.waveZoom;
    const start = state.window.waveScroll * (state.data.duration - visible);
    return state.strokes.map(({ x }) => start + x / state.canvas.clientWidth * visible);
}

function assertNear(actual, expected) {
    assert.ok(Math.abs(actual - expected) < 1e-4, `${actual} != ${expected}`);
}

test("grid lines are uniform despite gaps in the detected beat array", async () => {
    const state = await setup({ beats: [0.137, 0.607, 5.137, 5.607] });
    const times = drawnTimes(state);
    const interval = 60 / 127.4;

    for (let index = 1; index < times.length; index++) {
        assertNear(times[index] - times[index - 1], interval);
    }
    assert.ok(times.some(time => Math.abs(time - 43.0) < interval));
});

test("snapping uses the grid tempo supplied by the UI", async () => {
    const state = await setup();
    const precise = 0.137 + 91 * (60 / 127.4);
    const rounded = 0.137 + 91 * (60 / 127);

    assertNear(state.window.snapToGrid(43.2), precise);
    assert.ok(Math.abs(state.window.snapToGrid(43.2) - rounded) > 0.1);
});

test("zoomed out shows bar lines with the bar style", async () => {
    const state = await setup({}, { waveZoom: 1, waveScroll: 0 });
    const times = drawnTimes(state);

    assert.equal(state.strokes[0].width, 1.4);
    assert.equal(state.strokes[0].color, "rgba(255,255,255,0.45)");
    assertNear(times[1] - times[0], (60 / 127.4) * 4 * 2);
});

test("manual grid shift moves the drawn phase and snapping together", async () => {
    const base = await setup();
    const shifted = await setup({ shift: 0.25 });

    assertNear(drawnTimes(shifted)[0] - drawnTimes(base)[0], 0.25);
    assertNear(shifted.window.snapToGrid(43.2), 0.387 + 91 * (60 / 127.4));
});

test("missing tempo falls back to the rounded BPM", async () => {
    const state = await setup({ tempo: 0, bpm: 120, beats: [0], bars: [0] });
    assertNear(state.window.snapToGrid(1.2), 1.0);
});

test("disabled snapping leaves the requested time unchanged", async () => {
    const state = await setup({ snap: false });
    assert.equal(state.window.snapToGrid(43.123), 43.123);
});

test("theme colors and high-DPI redraws stay stable", async () => {
    const state = await setup({}, { devicePixelRatio: 2 });
    const before = state.strokes.map(line => ({ ...line }));
    state.window.drawBeatGrid();

    assert.deepEqual(state.strokes, before);
    assert.equal(state.canvas.width, 2000);
    assert.equal(state.canvas.height, 224);
    assert.equal(state.strokes[0].color, "rgba(255,255,255,0.22)");
});

test("light theme keeps beat lines visible", async () => {
    const state = await setup({ theme: "light" });
    assert.equal(state.strokes[0].color, "rgba(27,28,28,0.26)");
});
