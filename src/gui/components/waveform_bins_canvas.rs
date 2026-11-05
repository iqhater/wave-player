use crate::PlayerContext;
use crate::gui::layout::LoopDragging;
use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::PlayerEvent;
use dioxus::core::use_drop;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const WAVEFORM_JS: &str = r#"
    let data = await dioxus.recv();

    let canvas = document.getElementById("waveform-canvas");

    if (!canvas) {
        return;
    }

    let ctx = canvas.getContext("2d");

    let width = canvas.clientWidth || 600;
    let height = canvas.clientHeight || 72;
    let dpr = window.devicePixelRatio || 1;

    if (
        canvas.width !== width * dpr ||
        canvas.height !== height * dpr
    ) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;

        // Use transform instead of calling ctx.scale() again to avoid
        // accumulating scaling on every resize.
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    if (typeof data.duration === "number") {
        window.waveDuration = data.duration;
    }

    if (typeof data.progress === "number") {
        window.waveProgress = data.progress;
    }

    if (typeof window.waveProgress !== "number") {
        window.waveProgress = 0;
    }

    // Zoom and scroll are shared by the waveform, beat grid, seek,
    // and loop markers.
    if (data.reset || window.waveZoom === undefined) {
        window.waveZoom = 1.0;
        window.waveScroll = 0.0;
    }

    if (data.reset || !window.waveformState) {
        window.waveformState = {
            overview: null,
            levels: [],
            binFrames: [],
            totalFrames: 0,
            sampleRate: 0,
            windows: [],
            windowCounter: 0,
            appearStarted: 0,
        };
    }

    const state = window.waveformState;

    if (data.reset) {
        state.overview = null;
        state.levels = [];
        state.binFrames = [];
        state.totalFrames = 0;
        state.sampleRate = 0;
        state.windows = [];
        state.appearStarted = 0;
        window.waveformLastWheelAt = 0;
    }

    if (Array.isArray(data.overview) && data.overview.length > 0) {
        const overview = new Float32Array(data.overview.length);

        for (let i = 0; i < data.overview.length; i++) {
            overview[i] = Number(data.overview[i]) || 0;
        }

        state.overview = overview;
    }

    /*
     * Builds the mip levels of the peak pyramid in one pass.
     * Level 0 contains packed min/max pairs.
     */
    function buildPeakLevels(level0, finestBinFrames) {
        const levels = [level0];
        const binFrames = [finestBinFrames];

        let current = level0;
        let stride = finestBinFrames;

        while (current.length > 4) {
            const next = new Int8Array(
                Math.ceil(current.length / 4) * 2
            );

            const binCount = next.length / 2;

            for (let i = 0; i < binCount; i++) {
                const left = i * 4;
                const right = left + 2;

                const leftMin = current[left];
                const leftMax = current[left + 1];

                const rightMin =
                    right < current.length
                        ? current[right]
                        : leftMin;

                const rightMax =
                    right + 1 < current.length
                        ? current[right + 1]
                        : leftMax;

                next[i * 2] = Math.min(leftMin, rightMin);
                next[i * 2 + 1] = Math.max(leftMax, rightMax);
            }

            stride *= 2;
            levels.push(next);
            binFrames.push(stride);
            current = next;
        }

        return { levels, binFrames };
    }

    window.waveformBuildPeakLevels = buildPeakLevels;

    if (data.waveformPeaks && data.waveformPeaks.encoded) {
        const wavePeaks = data.waveformPeaks;
        const binary = atob(wavePeaks.encoded);
        const bytes = new Uint8Array(binary.length);

        for (let i = 0; i < binary.length; i++) {
            bytes[i] = binary.charCodeAt(i);
        }

        const pyramid = buildPeakLevels(
            new Int8Array(bytes.buffer),
            Number(wavePeaks.binFrames) || 1
        );

        state.levels = pyramid.levels;
        state.binFrames = pyramid.binFrames;
        state.totalFrames = Number(wavePeaks.totalFrames) || 0;
        state.sampleRate = Number(wavePeaks.sampleRate) || 0;
        state.windows = [];
        state.appearStarted = 0;
    }

    /*
     * Converts a position inside the visible canvas area into an
     * absolute track time.
     *
     * pct is in the 0..1 range.
     */
    window.waveTimeAtViewportPct = function (pct) {
        const trackDuration = window.waveDuration || 0;
        const zoom = window.waveZoom || 1.0;
        const scroll = window.waveScroll || 0.0;

        if (trackDuration <= 0) {
            return 0;
        }

        const visibleDuration = trackDuration / zoom;
        const startTime =
            scroll * (trackDuration - visibleDuration);

        const normalizedPct = Math.max(0, Math.min(1, pct));

        return Math.max(
            0,
            Math.min(
                trackDuration,
                startTime + normalizedPct * visibleDuration
            )
        );
    };

    /*
     * Converts an absolute track time into a percentage of the
     * current visible canvas area.
     */
    window.waveViewportPctAtTime = function (seconds) {
        const trackDuration = window.waveDuration || 0;
        const zoom = window.waveZoom || 1.0;
        const scroll = window.waveScroll || 0.0;

        if (trackDuration <= 0) {
            return 0;
        }

        const visibleDuration = trackDuration / zoom;
        const startTime =
            scroll * (trackDuration - visibleDuration);

        return (
            ((seconds - startTime) / visibleDuration) * 100
        );
    };

    /*
     * Updates CSS variables consumed by HTML overlays:
     * the playback cursor and loop markers.
     */
    function updateWaveViewportVariables() {
        const waveformCanvas =
            document.getElementById("waveform-canvas");

        if (!waveformCanvas) {
            return;
        }

        const waveformContainer =
            waveformCanvas.parentElement;

        if (!waveformContainer) {
            return;
        }

        const cursorTime =
            (window.waveProgress || 0) *
            (window.waveDuration || 0);

        const cursorPct =
            window.waveViewportPctAtTime(cursorTime);

        waveformContainer.style.setProperty(
            "--wave-cursor-pct",
            `${Math.max(0, Math.min(100, cursorPct))}%`
        );
    }

    window.updateWaveViewportVariables = updateWaveViewportVariables;

    function waveformCanvasWidth() {
        const waveformCanvas =
            document.getElementById("waveform-canvas");

        return (waveformCanvas && waveformCanvas.clientWidth) || 1;
    }

    /*
     * Returns the current CSS size of the canvas and recreates the
     * backing store when the window width changes.
     */
    function waveformSize() {
        const waveformCanvas =
            document.getElementById("waveform-canvas");

        const nextWidth =
            (waveformCanvas && waveformCanvas.clientWidth) || 600;

        const nextHeight =
            (waveformCanvas && waveformCanvas.clientHeight) || 72;

        const nextDpr = window.devicePixelRatio || 1;

        if (
            waveformCanvas &&
            (waveformCanvas.width !== nextWidth * nextDpr ||
                waveformCanvas.height !== nextHeight * nextDpr)
        ) {
            waveformCanvas.width = nextWidth * nextDpr;
            waveformCanvas.height = nextHeight * nextDpr;

            const nextContext =
                waveformCanvas.getContext("2d");

            nextContext.setTransform(
                nextDpr,
                0,
                0,
                nextDpr,
                0,
                0
            );
        }

        return { width: nextWidth, height: nextHeight };
    }

    function waveformViewport() {
        const trackState = window.waveformState;

        const totalFrames = trackState
            ? trackState.totalFrames || 0
            : 0;

        const zoom = Math.max(1.0, window.waveZoom || 1.0);
        const scroll = Math.max(
            0.0,
            Math.min(1.0, window.waveScroll || 0.0)
        );

        const visibleFrames = totalFrames / zoom;
        const startFrame =
            scroll * Math.max(0, totalFrames - visibleFrames);

        return {
            totalFrames,
            zoom,
            scroll,
            visibleFrames,
            startFrame,
            endFrame: startFrame + visibleFrames,
        };
    }

    function choosePeakLevel(binFrames, framesPerPixel) {
        let selected = -1;

        for (let i = 0; i < binFrames.length; i++) {
            if (binFrames[i] <= framesPerPixel) {
                selected = i;
            } else {
                break;
            }
        }

        return selected;
    }

    window.waveformChoosePeakLevel = choosePeakLevel;

    function scheduleWaveformFrame() {
        if (window.waveformAnimId) {
            return;
        }

        window.waveformAnimId =
            requestAnimationFrame(renderWaveformFrame);
    }

    function cancelWaveformFrame() {
        if (window.waveformAnimId) {
            cancelAnimationFrame(window.waveformAnimId);
            window.waveformAnimId = null;
        }
    }

    /*
     * Changes zoom relative to the point under the cursor.
     *
     * mousePct is in the 0..1 range.
     */
    window.zoomAt = function (mousePct, factor) {
        const zoom = window.waveZoom || 1.0;
        const scroll = window.waveScroll || 0.0;
        const totalDuration = window.waveDuration || 0;

        if (totalDuration <= 0) {
            return;
        }

        const normalizedMousePct =
            Math.max(0, Math.min(1, mousePct));

        const newZoom = Math.max(
            1.0,
            Math.min(1_000_000.0, zoom * factor)
        );

        const visibleOld = 1.0 / zoom;
        const startOld = scroll * (1.0 - visibleOld);

        /*
         * Absolute time coordinate of the point under the cursor.
         */
        const anchor =
            startOld + normalizedMousePct * visibleOld;

        const visibleNew = 1.0 / newZoom;
        const startNew =
            anchor - normalizedMousePct * visibleNew;

        let newScroll;

        if (visibleNew >= 1.0) {
            newScroll = 0;
        } else {
            newScroll = startNew / (1.0 - visibleNew);
        }

        newScroll = Math.max(0, Math.min(1, newScroll));

        window.waveZoom = newZoom;
        window.waveScroll = newScroll;

        updateWaveViewportVariables();

        if (window.drawBeatGrid) {
            window.drawBeatGrid();
        }

        scheduleWaveformFrame();
    };

    window.setWaveViewport = function (
        nextZoom,
        centerPercent
    ) {
        // Rust syncs the viewport with a throttle. Applying those echoes
        // while the wheel is still moving would roll the canvas back to an
        // older zoom value and make scrolling feel slow.
        const now =
            typeof performance !== "undefined" &&
            typeof performance.now === "function"
                ? performance.now()
                : Date.now();

        if (now - (window.waveformLastWheelAt || 0) < 250) {
            return;
        }

        const parsedZoom = Number(nextZoom);
        const parsedCenter = Number(centerPercent);

        const zoom =
            Number.isFinite(parsedZoom)
                ? Math.max(1.0, parsedZoom)
                : 1.0;

        const center =
            Number.isFinite(parsedCenter)
                ? Math.max(0, Math.min(100, parsedCenter)) / 100
                : 0.5;

        const visibleFraction = 1.0 / zoom;
        const maxStart = Math.max(0, 1.0 - visibleFraction);

        const startFraction = Math.max(
            0,
            Math.min(
                maxStart,
                center - visibleFraction / 2.0
            )
        );

        window.waveZoom = zoom;

        window.waveScroll =
            maxStart > 0
                ? startFraction / maxStart
                : 0.0;

        updateWaveViewportVariables();

        if (window.drawBeatGrid) {
            window.drawBeatGrid();
        }

        scheduleWaveformFrame();
    };

    window.panBy = function (deltaPx) {
        const panCanvas =
            document.getElementById("waveform-canvas");

        if (!panCanvas) {
            return;
        }

        const panWidth = panCanvas.clientWidth || 1;
        const zoom = window.waveZoom || 1.0;
        const visibleFraction = 1.0 / zoom;
        const maxStart = 1.0 - visibleFraction;

        if (maxStart <= 0) {
            return;
        }

        const currentStart =
            (window.waveScroll || 0.0) * maxStart;

        const shift =
            (deltaPx / panWidth) * visibleFraction;

        const nextStart =
            Math.max(0, Math.min(maxStart, currentStart + shift));

        window.waveScroll =
            nextStart / maxStart;

        updateWaveViewportVariables();

        if (window.drawBeatGrid) {
            window.drawBeatGrid();
        }

        scheduleWaveformFrame();
    };

    function aggregateOverviewBetween(
        overview,
        startFraction,
        endFraction
    ) {
        const count = overview.length;

        if (count === 0) {
            return { min: 0, max: 0, count: 0 };
        }

        let first = Math.floor(startFraction * count);
        let last = Math.ceil(endFraction * count) - 1;

        first = Math.max(0, Math.min(count - 1, first));
        last = Math.max(0, Math.min(count - 1, last));

        let peak = 0;

        for (let i = first; i <= last; i++) {
            peak = Math.max(peak, Math.abs(overview[i]));
        }

        return { min: -peak, max: peak, count: last - first + 1 };
    }

    function aggregatePeakLevel(
        level,
        binFrames,
        startFrame,
        endFrame
    ) {
        const binCount = level.length / 2;

        let first = Math.floor(startFrame / binFrames);
        let last = Math.ceil(endFrame / binFrames) - 1;

        first = Math.max(0, Math.min(binCount - 1, first));
        last = Math.max(0, Math.min(binCount - 1, last));

        let min = 127;
        let max = -128;

        for (let i = first; i <= last; i++) {
            min = Math.min(min, level[i * 2]);
            max = Math.max(max, level[i * 2 + 1]);
        }

        return {
            min: min / 127,
            max: max / 127,
            count: last - first + 1,
        };
    }

    window.waveformAggregatePeakLevel = aggregatePeakLevel;

    function aggregateDetailWindow(
        detailWindow,
        startFrame,
        endFrame
    ) {
        const points = detailWindow.points;

        let first = Math.floor(
            (startFrame - detailWindow.startFrame) /
                detailWindow.framesPerPoint
        );

        let last = Math.ceil(
            (endFrame - detailWindow.startFrame) /
                detailWindow.framesPerPoint
        ) - 1;

        first = Math.max(0, Math.min(points.length - 1, first));
        last = Math.max(0, Math.min(points.length - 1, last));

        let min = 1;
        let max = -1;

        for (let i = first; i <= last; i++) {
            min = Math.min(min, points[i].min);
            max = Math.max(max, points[i].max);
        }

        return { min, max, count: last - first + 1 };
    }

    /*
     * Plans a local window for extreme zoom. Returns null when the
     * pyramid is not loaded yet, the resolution is sufficient, or the
     * visible area is already covered by a window with enough detail.
     */
    function planLocalWindow() {
        const trackState = window.waveformState;

        if (
            !trackState ||
            !trackState.levels ||
            trackState.levels.length === 0 ||
            !trackState.binFrames ||
            trackState.binFrames.length === 0 ||
            !trackState.sampleRate
        ) {
            return null;
        }

        const viewport = waveformViewport();

        if (viewport.totalFrames <= 0 || viewport.visibleFrames <= 0) {
            return null;
        }

        const canvasWidth = waveformCanvasWidth();
        const framesPerPixel =
            viewport.visibleFrames / canvasWidth;

        if (framesPerPixel >= trackState.binFrames[0]) {
            return null;
        }

        // The base layer is limited by the finest peak level. A detail
        // window is enough when it is at least as fine as the viewport
        // (or sample-level when a pixel covers less than one frame).
        // A small tolerance absorbs integer frame rounding in the
        // requested window.
        const requiredResolution =
            Math.max(framesPerPixel, 1) * 1.05;

        for (const detailWindow of trackState.windows) {
            const coversViewport =
                detailWindow.startFrame <= viewport.startFrame + 2 &&
                detailWindow.endFrame >= viewport.endFrame - 2;

            if (
                coversViewport &&
                detailWindow.framesPerPoint <= requiredResolution
            ) {
                return null;
            }
        }

        const span = viewport.visibleFrames * 3;

        const startFrame = Math.max(
            0,
            Math.floor(viewport.startFrame - span / 2)
        );

        const endFrame = Math.min(
            viewport.totalFrames,
            Math.ceil(viewport.endFrame + span / 2)
        );

        if (endFrame <= startFrame) {
            return null;
        }

        return {
            startFrame,
            endFrame,
            start: startFrame / trackState.sampleRate,
            end: endFrame / trackState.sampleRate,
            points: 4096,
        };
    }

    window.waveformPlanLocalWindow = planLocalWindow;

    window.handleWaveWheel = function (
        mousePct,
        deltaX,
        deltaY,
        shiftKey
    ) {
        window.waveformLastWheelAt =
            typeof performance !== "undefined" &&
            typeof performance.now === "function"
                ? performance.now()
                : Date.now();

        const hasHorizontalDelta =
            Math.abs(deltaX) > Math.abs(deltaY) &&
            Math.abs(deltaX) > 0.01;

        const shouldPan =
            hasHorizontalDelta ||
            (shiftKey && Math.abs(deltaY) > 0.01);

        if (shouldPan) {
            const panDelta =
                hasHorizontalDelta ? deltaX : deltaY;

            window.panBy(panDelta);
        } else {
            const factor =
                deltaY < 0.0 ? 1.3 : 1.0 / 1.3;

            window.zoomAt(mousePct, factor);
        }

        return {
            zoom: window.waveZoom || 1.0,
            scroll: window.waveScroll || 0.0,
            detail: planLocalWindow(),
        };
    };

    window.waveformAddDetailWindow = function (detail) {
        const trackState = window.waveformState;

        if (
            !trackState ||
            !detail ||
            !Array.isArray(detail.points) ||
            detail.points.length === 0
        ) {
            return;
        }

        const startFrame = Number(detail.startFrame);
        const endFrame = Number(detail.endFrame);

        if (
            !Number.isFinite(startFrame) ||
            !Number.isFinite(endFrame) ||
            endFrame <= startFrame
        ) {
            return;
        }

        const points = detail.points.map((point) => {
            let min = Number(point.min);
            let max = Number(point.max);

            if (!Number.isFinite(min)) {
                min = 0;
            }

            if (!Number.isFinite(max)) {
                max = 0;
            }

            return {
                min: Math.max(-1, Math.min(1, min)),
                max: Math.max(-1, Math.min(1, max)),
            };
        });

        trackState.windowCounter += 1;

        const entry = {
            requestId: Number(detail.requestId) || 0,
            startFrame,
            endFrame,
            points,
            framesPerPoint:
                (endFrame - startFrame) / points.length,
            lastUsed: trackState.windowCounter,
        };

        const duplicate = trackState.windows.findIndex(
            (windowEntry) =>
                windowEntry.startFrame === startFrame &&
                windowEntry.endFrame === endFrame
        );

        if (duplicate >= 0) {
            trackState.windows.splice(duplicate, 1);
        }

        trackState.windows.push(entry);
        trackState.windows.sort(
            (left, right) => left.lastUsed - right.lastUsed
        );

        while (trackState.windows.length > 4) {
            trackState.windows.shift();
        }

        scheduleWaveformFrame();
    };

    function drawPlaceholder(width, height) {
        ctx.fillStyle = "rgba(196, 200, 192, 0.22)";
        ctx.fillRect(0, Math.max(0, height / 2 - 1), width, 2);
    }

    function drawBarSet(bars, color, appear, height) {
        ctx.fillStyle = color;

        for (const bar of bars) {
            let min = bar.min;
            let max = bar.max;

            if (min > max) {
                const swap = min;
                min = max;
                max = swap;
            }

            const yTop =
                height / 2 - max * appear * height / 2;

            const yBottom =
                height / 2 - min * appear * height / 2;

            const top = Math.min(yTop, yBottom);
            const barHeight = Math.max(1, Math.abs(yBottom - yTop));

            if (bar.width > 3 && ctx.roundRect) {
                ctx.beginPath();
                ctx.roundRect(bar.x, top, bar.width, barHeight, 4);
                ctx.fill();
            } else {
                ctx.fillRect(bar.x, top, bar.width, barHeight);
            }
        }
    }

    function drawOverviewBarSet(bars, color) {
        ctx.fillStyle = color;

        for (const bar of bars) {
            if (bar.width >= 3 && ctx.roundRect) {
                ctx.beginPath();
                ctx.roundRect(
                    bar.x,
                    bar.y,
                    bar.width,
                    bar.height,
                    4
                );
                ctx.fill();
            } else {
                ctx.fillRect(bar.x, bar.y, bar.width, bar.height);
            }
        }
    }

    function renderWaveformFrame() {
        window.waveformAnimId = null;

        const size = waveformSize();
        const width = size.width;
        const height = size.height;

        const now =
            typeof performance !== "undefined" &&
            typeof performance.now === "function"
                ? performance.now()
                : Date.now();

        ctx.clearRect(0, 0, width, height);

        const trackState = window.waveformState;

        if (trackState && trackState.appearStarted === 0) {
            trackState.appearStarted = now;
        }

        const appearDuration = 220;

        const appear = trackState
            ? Math.max(
                  0,
                  Math.min(
                      1,
                      (now - trackState.appearStarted) / appearDuration
                  )
              )
            : 1;

        const overview = trackState ? trackState.overview : null;
        const hasPeaks =
            trackState &&
            trackState.levels.length > 0 &&
            trackState.totalFrames > 0;

        const viewport = waveformViewport();
        const columns = Math.max(1, Math.floor(width));

        if (
            !hasPeaks &&
            (!overview || overview.length === 0)
        ) {
            drawPlaceholder(width, height);
            updateWaveViewportVariables();

            window.waveformStats = {
                lod: -1,
                framesPerPixel: 0,
                fallbackColumns: columns,
                detailColumns: 0,
                frameTime: 0,
            };

            return;
        }

        const framesPerPixel = hasPeaks
            ? viewport.visibleFrames / columns
            : 0;

        // Keep the familiar coarse overview at far zoom. The peak pyramid
        // takes over after the user zooms past the old detail threshold.
        const useOverview =
            (!hasPeaks || viewport.zoom < 8.0) &&
            overview &&
            overview.length > 0;

        const overviewStartFraction =
            viewport.scroll *
            (1 - 1 / Math.max(1, viewport.zoom));

        const overviewVisibleFraction =
            1 / Math.max(1, viewport.zoom);

        const progress = Number(window.waveProgress) || 0;

        if (useOverview) {
            const totalBins = overview.length;

            const startPosition =
                overviewStartFraction * totalBins;

            const endPosition =
                (overviewStartFraction + overviewVisibleFraction) *
                totalBins;

            const visibleBins = Math.max(
                1,
                endPosition - startPosition
            );

            const step = width / visibleBins;

            let gap;

            if (step >= 4.0) {
                gap = 1.5;
            } else if (step >= 2.0) {
                gap = 1.0;
            } else {
                gap = 0.65;
            }

            const barWidth = Math.max(0.8, step - gap);

            const firstBin = Math.max(
                0,
                Math.floor(startPosition)
            );

            const lastBin = Math.min(
                totalBins,
                Math.ceil(endPosition)
            );

            const playedBars = [];
            const unplayedBars = [];

            for (let i = firstBin; i < lastBin; i++) {
                const peak = Number(overview[i]) || 0;

                const normalized = Math.pow(
                    Math.max(0.02, peak),
                    0.85
                );

                const barHeight = Math.max(
                    2.0,
                    normalized * height * appear
                );

                const x = (i - startPosition) * step;
                const y = (height - barHeight) / 2;

                const bar = {
                    x,
                    y,
                    width: barWidth,
                    height: barHeight,
                };

                if (i / totalBins <= progress) {
                    playedBars.push(bar);
                } else {
                    unplayedBars.push(bar);
                }
            }

            drawOverviewBarSet(
                playedBars,
                "rgba(90, 122, 122, 1)"
            );

            drawOverviewBarSet(
                unplayedBars,
                "rgba(196, 200, 192, 1)"
            );

            updateWaveViewportVariables();

            window.waveformStats = {
                lod: -1,
                framesPerPixel,
                fallbackColumns: columns,
                detailColumns: 0,
                frameTime:
                    (typeof performance !== "undefined" &&
                    typeof performance.now === "function"
                        ? performance.now()
                        : Date.now()) - now,
            };

            if (appear < 1) {
                window.waveformAnimId =
                    requestAnimationFrame(renderWaveformFrame);
            }

            return;
        }

        const lod = choosePeakLevel(
            trackState.binFrames,
            framesPerPixel
        );

        // In local-window mode a coarser mip level replaces the missing
        // detail until the precise window arrives.
        const baseLevel = lod >= 0 ? lod : 0;

        const bars = [];
        let detailColumns = 0;

        for (let x = 0; x < columns; x++) {
            const frameStart =
                viewport.startFrame + x * framesPerPixel;

            const frameEnd = frameStart + framesPerPixel;

            const played =
                frameStart / viewport.totalFrames <= progress;

            let min = 0;
            let max = 0;
            let resolution = trackState.binFrames[baseLevel];

            const aggregate = aggregatePeakLevel(
                trackState.levels[baseLevel],
                trackState.binFrames[baseLevel],
                frameStart,
                frameEnd
            );

            min = aggregate.min;
            max = aggregate.max;

            for (const detailWindow of trackState.windows) {
                if (
                    frameEnd <= detailWindow.startFrame ||
                    frameStart >= detailWindow.endFrame
                ) {
                    continue;
                }

                // Use the finest available window. Even when a pixel
                // covers less than one frame, sample-level detail is
                // the best data we can get.
                if (
                    detailWindow.framesPerPoint >= resolution
                ) {
                    continue;
                }

                const detail = aggregateDetailWindow(
                    detailWindow,
                    frameStart,
                    frameEnd
                );

                min = detail.min;
                max = detail.max;
                resolution = detailWindow.framesPerPoint;
                detailColumns += 1;
            }

            bars.push({
                x,
                width: 1,
                min,
                max,
                played,
            });
        }

        const playedBars = bars.filter((bar) => bar.played);
        const unplayedBars = bars.filter((bar) => !bar.played);

        drawBarSet(
            playedBars,
            "rgba(90, 122, 122, 1)",
            appear,
            height
        );

        drawBarSet(
            unplayedBars,
            "rgba(196, 200, 192, 1)",
            appear,
            height
        );

        updateWaveViewportVariables();

        window.waveformStats = {
            lod,
            framesPerPixel,
            fallbackColumns: 0,
            detailColumns,
            frameTime:
                (typeof performance !== "undefined" &&
                typeof performance.now === "function"
                    ? performance.now()
                    : Date.now()) - now,
        };

        if (appear < 1) {
            window.waveformAnimId =
                requestAnimationFrame(renderWaveformFrame);
        }
    }

    window.waveformRenderFrame = renderWaveformFrame;

    window.renderWaveform = function () {
        scheduleWaveformFrame();
    };

    if (window.waveformResizeObserver) {
        window.waveformResizeObserver.disconnect();
        window.waveformResizeObserver = undefined;
    }

    if (typeof ResizeObserver !== "undefined") {
        window.waveformResizeObserver = new ResizeObserver(
            () => scheduleWaveformFrame()
        );

        window.waveformResizeObserver.observe(canvas);
    }

    updateWaveViewportVariables();
    scheduleWaveformFrame();
"#;

const WAVEFORM_DETAIL_JS: &str = r#"
let data = await dioxus.recv();

if (window.waveformAddDetailWindow) {
    window.waveformAddDetailWindow(data);
}
"#;

const WAVEFORM_VIEWPORT_JS: &str = r#"
let data = await dioxus.recv();

if (window.setWaveViewport) {
    window.setWaveViewport(
        data.zoom,
        data.viewCenter
    );
}
"#;

const WAVEFORM_PROGRESS_JS: &str = r#"
let data = await dioxus.recv();

if (typeof data.progress === "number") {
    window.waveProgress = data.progress;

    if (window.updateWaveViewportVariables) {
        window.updateWaveViewportVariables();
    }

    if (window.renderWaveform) {
        window.renderWaveform();
    }
}
"#;

// Lightweight variant used while the loop region is being dragged: it only
// advances the playback cursor. The full canvas redraw is skipped so pointer
// handling keeps priority, but the cursor must never freeze.
const WAVEFORM_CURSOR_JS: &str = r#"
let data = await dioxus.recv();

if (typeof data.progress === "number") {
    window.waveProgress = data.progress;

    if (window.updateWaveViewportVariables) {
        window.updateWaveViewportVariables();
    }
}
"#;

#[derive(Clone, PartialEq)]
struct WaveformPeaksData {
    sample_rate: u32,
    total_frames: usize,
    bin_frames: usize,
    encoded: Arc<str>,
}

const VIEWPORT_SYNC_INTERVAL: Duration = Duration::from_millis(50);
const VIEWPORT_SYNC_DELAY: Duration = Duration::from_millis(60);
const DETAIL_REQUEST_DELAY: Duration = Duration::from_millis(50);

#[component]
pub fn WaveformBinsCanvas(
    total_duration: ReadSignal<f32>,
    cursor_progress: ReadSignal<f32>,
    mut zoom: Signal<f32>,
    mut view_center: Signal<f32>,
) -> Element {
    let ctx = use_context::<PlayerContext>();
    let loop_dragging = use_context::<LoopDragging>().0;
    let mut position_secs = use_signal(|| 0.0);
    let mut waveform_peaks = use_signal(Vec::<f32>::new);
    let mut peak_pyramid = use_signal(|| None::<WaveformPeaksData>);

    // Zoom reset signal for a newly loaded track.
    let mut reset_zoom = use_signal(|| false);

    // Viewport signals are synchronized at most once per 50 ms; the
    // final value is applied after the gesture stops.
    let mut viewport_synced_at = use_signal(|| None::<Instant>);
    let mut viewport_pending = use_signal(|| None::<(f32, f32)>);
    let mut viewport_generation = use_signal(|| 0_u64);

    let mut detail_generation = use_signal(|| 0_u64);
    let mut next_detail_request = use_signal(|| 0_u64);

    use_effect(move || {
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::RequestWaveformOverview);
    });

    // Background event listener for audio-core events.
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started { .. } => {
                        waveform_peaks.set(Vec::new());
                        peak_pyramid.set(None);

                        // Reset zoom when a new track starts.
                        reset_zoom.set(true);
                        zoom.set(1.0);
                        view_center.set(50.0);

                        viewport_synced_at.set(None);
                        viewport_pending.set(None);

                        // Replay cached overview and peaks if analysis for the
                        // new track has already completed.
                        let _ = ctx
                            .command_tx
                            .read()
                            .send(PlayerCommand::RequestWaveformOverview);
                    }
                    PlayerEvent::WaveformChunkReady {
                        chunk,
                        start_bin,
                        total_bins,
                    } => {
                        if waveform_peaks.read().len() != total_bins {
                            waveform_peaks.set(vec![0.0f32; total_bins]);
                        }
                        waveform_peaks.with_mut(|vec| {
                            for (i, peak_value) in chunk.into_iter().enumerate() {
                                let target_idx = start_bin + i;
                                if let Some(elem) = vec.get_mut(target_idx) {
                                    *elem = peak_value;
                                }
                            }
                        });
                    }
                    PlayerEvent::WaveformPeaksReady {
                        sample_rate,
                        total_frames,
                        bin_frames,
                        encoded,
                    } => {
                        peak_pyramid.set(Some(WaveformPeaksData {
                            sample_rate,
                            total_frames,
                            bin_frames,
                            encoded,
                        }));
                    }
                    PlayerEvent::WaveformDetailReady {
                        request_id,
                        start_frame,
                        end_frame,
                        total_frames,
                        sample_rate,
                        points,
                    } => {
                        // A stale window is not applied to the current
                        // viewport directly but stays in the JS LRU cache.
                        let detail = serde_json::json!({
                            "requestId": request_id,
                            "startFrame": start_frame,
                            "endFrame": end_frame,
                            "totalFrames": total_frames,
                            "sampleRate": sample_rate,
                            "points": points,
                        });

                        spawn(async move {
                            let eval = dioxus::document::eval(WAVEFORM_DETAIL_JS);
                            let _ = eval.send(detail);
                        });
                    }

                    _ => {}
                }
            }
        }
    });

    // Poll the real playback position from the atomic so the waveform
    // fill updates in sync with the cursor.
    use_future(move || {
        let pos = ctx.playback_progress.read().clone();

        async move {
            let initial_position = pos.load(Ordering::Relaxed);
            position_secs.set(initial_position as f32 / 1000.0);

            let mut last_position = initial_position;

            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;

                let current_position = pos.load(Ordering::Relaxed);

                if current_position != last_position {
                    last_position = current_position;
                    position_secs.set(current_position as f32 / 1000.0);
                }
            }
        }
    });

    use_effect(move || {
        if loop_dragging() {
            let _ = dioxus::document::eval(
                r#"
                if (window.waveformAnimId) {
                    cancelAnimationFrame(window.waveformAnimId);
                    window.waveformAnimId = null;
                }
                "#,
            );
        }
    });

    use_effect(move || {
        let dragging = loop_dragging();
        let current_pos = position_secs();
        let total_dur = total_duration();

        let progress = if total_dur > 0.0 {
            (current_pos / total_dur).clamp(0.0, 1.0)
        } else {
            0.0
        };

        spawn(async move {
            let script = if dragging {
                WAVEFORM_CURSOR_JS
            } else {
                WAVEFORM_PROGRESS_JS
            };
            let eval = dioxus::document::eval(script);

            let _ = eval.send(serde_json::json!({
                "progress": progress,
            }));
        });
    });

    // Redraw the canvas when the overview, peaks, or track length changes.
    use_effect(move || {
        let peaks = waveform_peaks.read().clone();
        let pyramid = peak_pyramid.read().clone();
        let total_dur = total_duration();
        let do_reset = reset_zoom();

        if do_reset {
            reset_zoom.set(false);
        }

        spawn(async move {
            let eval = dioxus::document::eval(WAVEFORM_JS);

            let _ = eval.send(serde_json::json!({
                "overview": peaks,
                "waveformPeaks": pyramid.map(|data| serde_json::json!({
                    "sampleRate": data.sample_rate,
                    "totalFrames": data.total_frames,
                    "binFrames": data.bin_frames,
                    "encoded": data.encoded.to_string(),
                })),
                "duration": total_dur,
                "reset": do_reset,
            }));
        });
    });

    use_effect(move || {
        let current_zoom = zoom();
        let current_view_center = view_center();

        spawn(async move {
            let eval = dioxus::document::eval(WAVEFORM_VIEWPORT_JS);

            let _ = eval.send(serde_json::json!({
                "zoom": current_zoom,
                "viewCenter": current_view_center,
            }));
        });
    });

    // Clean up waveform animation resources on drop
    use_drop(|| {
        let _ = dioxus::document::eval(
            r#"
            if (window.waveformAnimId !== null) {
                cancelAnimationFrame(window.waveformAnimId);
                window.waveformAnimId = null;
            }

            if (window.waveformResizeObserver) {
                window.waveformResizeObserver.disconnect();
                window.waveformResizeObserver = undefined;
            }

            window.waveformLastWheelAt = 0;

            window.waveViewportPctAtTime = undefined;
            window.waveTimeAtViewportPct = undefined;
            window.zoomAt = undefined;
            window.updateWaveViewportVariables = undefined;
            "#,
        );
    });

    rsx! {
        canvas {
            id: "waveform-canvas",
            class: "w-full h-18 flex-shrink-0 pointer-events-auto",
            style: "display: block; width: 100%; height: 72px; background: transparent;",

            // Mouse wheel handling (zoom/pan).
            onwheel: move |e| {
                let delta = e.delta().strip_units();
                let delta_x = delta.x;
                let delta_y = delta.y;
                let shift_key = e.modifiers().shift();
                let click_x = e.element_coordinates().x as f32;

                // All zoom logic lives in JS (synchronously, without races and
                // without querying the width from Rust).
                let mut eval = dioxus::document::eval(
                    r#"
                    let data = await dioxus.recv();

                    const canvas =
                        document.getElementById("waveform-canvas");

                    if (!canvas || !window.handleWaveWheel) {
                        return;
                    }

                    const width =
                        canvas.clientWidth || 1;

                    const pct =
                        Math.max(0, Math.min(1, data.x / width));

                    const result =
                        window.handleWaveWheel(
                            pct,
                            data.deltaX,
                            data.deltaY,
                            data.shiftKey
                        );

                    dioxus.send(result);
                    "#
                );
                let _ = eval.send(serde_json::json!({
                    "x": click_x,
                    "deltaX": delta_x,
                    "deltaY": delta_y,
                    "shiftKey": shift_key,
                }));

                spawn(async move {
                    if let Ok(serde_json::Value::Object(data)) = eval.recv().await {
                        let new_zoom = data
                            .get("zoom")
                            .and_then(|value| value.as_f64())
                            .unwrap_or(1.0) as f32;

                        let scroll = data
                            .get("scroll")
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0) as f32;

                        let visible_fraction = 1.0 / new_zoom.max(1.0);

                        let start_fraction =
                            scroll.clamp(0.0, 1.0) * (1.0 - visible_fraction);

                        let center_percent =
                            (start_fraction + visible_fraction / 2.0) * 100.0;

                        let center_percent = center_percent.clamp(0.0, 100.0);
                        let detail = data
                            .get("detail")
                            .filter(|value| !value.is_null())
                            .cloned();

                        let now = Instant::now();
                        let should_sync = viewport_synced_at()
                            .is_none_or(|last| now.duration_since(last) >= VIEWPORT_SYNC_INTERVAL);

                        let generation = viewport_generation.with_mut(|value| {
                            *value = value.wrapping_add(1);
                            *value
                        });

                        if should_sync {
                            viewport_synced_at.set(Some(now));
                            viewport_pending.set(None);
                            zoom.set(new_zoom);
                            view_center.set(center_percent);
                        } else {
                            viewport_pending.set(Some((new_zoom, center_percent)));
                        }

                        spawn(async move {
                            tokio::time::sleep(VIEWPORT_SYNC_DELAY).await;

                            if viewport_generation() != generation {
                                return;
                            }

                            let pending = viewport_pending();

                            viewport_pending.set(None);
                            viewport_synced_at.set(Some(Instant::now()));

                            if let Some((pending_zoom, pending_center)) = pending {
                                zoom.set(pending_zoom);
                                view_center.set(pending_center);
                            }
                        });

                        let detail_generation_value = detail_generation.with_mut(|value| {
                            *value = value.wrapping_add(1);
                            *value
                        });

                        if let Some(detail) = detail {
                            spawn(async move {
                                tokio::time::sleep(DETAIL_REQUEST_DELAY).await;

                                if detail_generation() != detail_generation_value {
                                    return;
                                }

                                let start = detail.get("start").and_then(|value| value.as_f64());
                                let end = detail.get("end").and_then(|value| value.as_f64());
                                let points = detail
                                    .get("points")
                                    .and_then(|value| value.as_u64())
                                    .unwrap_or(1024) as usize;

                                let (Some(start), Some(end)) = (start, end) else {
                                    return;
                                };

                                let request_id = next_detail_request.with_mut(|value| {
                                    *value = value.wrapping_add(1);
                                    *value
                                });

                                let _ = ctx.command_tx.read().send(
                                    PlayerCommand::RequestWaveformDetail {
                                        request_id,
                                        start: Duration::from_secs_f64(start),
                                        end: Duration::from_secs_f64(end),
                                        points,
                                    },
                                );
                            });
                        }
                    }
                });
            }
        }
    }
}
