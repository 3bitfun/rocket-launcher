package com.rocketlauncher.companion;

record CrosshairStyle(Mode mode, int parts, int size, int thickness, int gap, int opacity,
                      int color, int outline, boolean shadow, boolean dynamic, int targetColor) {
    enum Mode { VANILLA, CUSTOM, HIDDEN }

    static final int HORIZONTAL = 1;
    static final int TOP = 2;
    static final int BOTTOM = 4;
    static final int DOT = 8;
    static final int RING = 16;
    static final int CROSS = HORIZONTAL | TOP | BOTTOM;
    static final CrosshairStyle DEFAULT = new CrosshairStyle(Mode.VANILLA, CROSS, 5, 1, 2, 100, 0xFFFFFF, 1, false, false, -1);

    CrosshairStyle {
        parts &= 31;
        size = bounded(size, 1, 14);
        thickness = bounded(thickness, 1, 5);
        gap = bounded(gap, 0, 12);
        opacity = bounded(opacity, 0, 100);
        color &= 0xFFFFFF;
        outline = bounded(outline, 0, 3);
        if (targetColor != -1) targetColor &= 0xFFFFFF;
    }

    private static int bounded(int value, int min, int max) { return Math.max(min, Math.min(max, value)); }

    String encode() {
        return String.join(",", mode.name(), "" + parts, "" + size, "" + thickness, "" + gap, "" + opacity,
            "" + color, "" + outline, "" + shadow, "" + dynamic, "" + targetColor);
    }

    static CrosshairStyle decode(String encoded) {
        if (encoded == null) return DEFAULT;
        String[] v = encoded.split(",", -1);
        if (v.length != 11) return DEFAULT;
        try {
            return new CrosshairStyle(Mode.valueOf(v[0]), Integer.parseInt(v[1]), Integer.parseInt(v[2]),
                Integer.parseInt(v[3]), Integer.parseInt(v[4]), Integer.parseInt(v[5]), Integer.parseInt(v[6]),
                Integer.parseInt(v[7]), Boolean.parseBoolean(v[8]), Boolean.parseBoolean(v[9]), Integer.parseInt(v[10]));
        } catch (IllegalArgumentException malformed) {
            return DEFAULT;
        }
    }

    CrosshairStyle withMode(Mode value) { return new CrosshairStyle(value, parts, size, thickness, gap, opacity, color, outline, shadow, dynamic, targetColor); }
    CrosshairStyle withParts(int value) { return new CrosshairStyle(Mode.CUSTOM, value, size, thickness, gap, opacity, color, outline, shadow, dynamic, targetColor); }
    CrosshairStyle withColor(int value) { return new CrosshairStyle(Mode.CUSTOM, parts, size, thickness, gap, opacity, value, outline, shadow, dynamic, targetColor); }
    CrosshairStyle withTarget(int value) { return new CrosshairStyle(Mode.CUSTOM, parts, size, thickness, gap, opacity, color, outline, shadow, dynamic, value); }
    CrosshairStyle withEffects(boolean shadow, boolean dynamic) {
        return new CrosshairStyle(Mode.CUSTOM, parts, size, thickness, gap, opacity, color, outline, shadow, dynamic, targetColor);
    }

    enum Measure {
        SIZE, THICKNESS, GAP, OPACITY, OUTLINE;

        int value(CrosshairStyle style) {
            return switch (this) {
                case SIZE -> style.size;
                case THICKNESS -> style.thickness;
                case GAP -> style.gap;
                case OPACITY -> style.opacity;
                case OUTLINE -> style.outline;
            };
        }

        CrosshairStyle adjust(CrosshairStyle style, int change) {
            int value = value(style) + change * (this == OPACITY ? 5 : 1);
            return new CrosshairStyle(Mode.CUSTOM, style.parts, this == SIZE ? value : style.size,
                this == THICKNESS ? value : style.thickness, this == GAP ? value : style.gap,
                this == OPACITY ? value : style.opacity, style.color, this == OUTLINE ? value : style.outline,
                style.shadow, style.dynamic, style.targetColor);
        }
    }
}
