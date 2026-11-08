package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.CrosshairStyle.*;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;

public final class CrosshairRenderer {
    private static final CrosshairRenderer IN_GAME = new CrosshairRenderer();
    private static ClientSettings settings;
    private record Span(int x, int y, int width, int layer) {}
    private CrosshairStyle previous;
    private final List<Span>[] spreads = createCache();

    static void bind(ClientSettings value) { settings = value; }

    /** Returns true when the platform hook should skip the vanilla crosshair. */
    public static boolean renderInGame(GuiGraphics graphics) {
        if (settings == null) return false;
        var client = Minecraft.getInstance();
        var style = settings.crosshair();
        if (style.mode() == Mode.VANILLA || client.player == null || client.player.isSpectator()
            || !client.options.getCameraType().isFirstPerson() || ClientPlatform.debugVisible(client)) return false;
        if (style.mode() == Mode.HIDDEN) return true;
        int spread = 0;
        if (style.dynamic()) {
            if (client.player.isSprinting()) spread += 2;
            if (client.options.keyUp.isDown() || client.options.keyDown.isDown()
                || client.options.keyLeft.isDown() || client.options.keyRight.isDown()) spread++;
            if (client.options.keyAttack.isDown()) spread += 2;
        }
        IN_GAME.draw(graphics, style, graphics.guiWidth() / 2, graphics.guiHeight() / 2,
            spread, client.hitResult instanceof net.minecraft.world.phys.EntityHitResult);
        return true;
    }

    @SuppressWarnings("unchecked")
    private static List<Span>[] createCache() { return (List<Span>[]) new List<?>[6]; }

    void draw(GuiGraphics graphics, CrosshairStyle style, int x, int y, int spread, boolean target) {
        if (!style.equals(previous)) {
            Arrays.fill(spreads, null);
            previous = style;
        }
        if (spreads[spread] == null) spreads[spread] = rasterize(style, spread);
        int opacity = (style.opacity() * 255 + 50) / 100;
        int color = target && style.targetColor() >= 0 ? style.targetColor() : style.color();
        for (Span span : spreads[spread]) {
            int argb = switch (span.layer) {
                case 3 -> opacity << 24 | color;
                case 2 -> opacity << 24;
                default -> (opacity / 2) << 24;
            };
            graphics.fill(x + span.x, y + span.y, x + span.x + span.width, y + span.y + 1, argb);
        }
    }

    private static List<Span> rasterize(CrosshairStyle style, int spread) {
        int extent = style.gap() + style.size() + spread + style.thickness() + style.outline() + 3;
        List<Span> spans = new ArrayList<>();
        for (int y = -extent; y <= extent; y++) {
            int last = 0, start = -extent;
            for (int x = -extent; x <= extent + 1; x++) {
                int layer = x > extent ? 0 : layer(style, x, y, spread);
                if (layer == last) continue;
                if (last != 0) spans.add(new Span(start, y, x - start, last));
                start = x;
                last = layer;
            }
        }
        return spans;
    }

    private static int layer(CrosshairStyle style, int x, int y, int spread) {
        if (contains(style, x, y, spread)) return 3;
        int outline = style.outline();
        for (int dy = -outline; dy <= outline; dy++) {
            for (int dx = -outline; dx <= outline; dx++) {
                if (contains(style, x + dx, y + dy, spread)) return 2;
            }
        }
        return style.shadow() && contains(style, x - outline - 1, y - outline - 1, spread) ? 1 : 0;
    }

    private static boolean contains(CrosshairStyle style, int x, int y, int spread) {
        int parts = style.parts();
        int start = style.gap() + spread + 1;
        int end = start + style.size();
        int low = -(style.thickness() / 2);
        int high = low + style.thickness();
        boolean thinX = x >= low && x < high;
        boolean thinY = y >= low && y < high;
        if ((parts & DOT) != 0 && thinX && thinY) return true;
        if ((parts & HORIZONTAL) != 0 && thinY && Math.abs(x) >= start && Math.abs(x) < end) return true;
        if ((parts & TOP) != 0 && thinX && -y >= start && -y < end) return true;
        if ((parts & BOTTOM) != 0 && thinX && y >= start && y < end) return true;
        int radius = style.size() + style.gap() + spread;
        int inner = Math.max(0, radius - style.thickness() + 1);
        int distance = x * x + y * y;
        return (parts & RING) != 0 && distance >= inner * inner && distance <= radius * radius;
    }
}
