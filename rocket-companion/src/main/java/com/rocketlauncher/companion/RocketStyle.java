package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.RocketCompanion.text;

import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.Button;
import net.minecraft.network.chat.Component;

final class RocketStyle {
    static final int ACCENT = 0xFF709DEB;
    static final int TEXT = 0xFFF2F4F8;
    static final int MUTED = 0xFFB1BAC9;
    static final int WARNING = 0xFFFFB491;
    static final int PANEL = 0xFF151820;
    static final int TILE = 0xFF1E232D;
    static final int BORDER = 0xFF3E4757;
    static final int CONTROL_HEIGHT = 20;

    private static final String VERSION = FabricLoader.getInstance().getModContainer("rocket_companion")
        .map(mod -> mod.getMetadata().getVersion().getFriendlyString().split("\\+", 2)[0])
        .orElse("");
    // Pixel interpretation of the Rocket launcher mark, drawn at the native GUI scale.
    private static final String[] MARK = {
        "......####", "....######", "...###..##", "..####..##", ".f#######.",
        "ff######..", "f.#####...", "...###f...", ".t..ff....", "t...f....."
    };

    private RocketStyle() {}

    static void frame(GuiGraphics graphics, int x, int y, int width, int height, int border, int fill) {
        graphics.fill(x, y, x + width, y + height, border);
        graphics.fill(x + 1, y + 1, x + width - 1, y + height - 1, fill);
    }

    static void header(GuiGraphics graphics, Font font, Component title, int x, int y, int width, int reserved) {
        frame(graphics, x, y, width, 32, BORDER, TILE);
        graphics.fill(x + 1, y + 1, x + width - 1, y + 2, ACCENT);
        for (int row = 0; row < MARK.length; row++) {
            for (int column = 0; column < MARK[row].length(); column++) {
                char pixel = MARK[row].charAt(column);
                if (pixel == '.') continue;
                int left = x + 7 + column * 2;
                int top = y + 6 + row * 2;
                int color = pixel == 'f' ? TEXT : pixel == 't' ? 0xFFA6C4F9 : ACCENT;
                graphics.fill(left, top, left + 2, top + 2, color);
            }
        }
        int available = width - reserved - 42;
        line(graphics, font, title, x + 34, y + (VERSION.isEmpty() ? 12 : 6), available, TEXT);
        if (!VERSION.isEmpty()) {
            line(graphics, font, text("version", VERSION), x + 34, y + 18, available, MUTED);
        }
    }

    static void line(GuiGraphics graphics, Font font, Component text, int x, int y, int width, int color) {
        graphics.drawString(font, fit(font, text, width), x, y, color);
    }

    private static String fit(Font font, Component text, int width) {
        String value = text.getString();
        if (font.width(value) <= width) return value;
        String ellipsis = "…";
        if (width < font.width(ellipsis)) return "";
        return font.plainSubstrByWidth(value, width - font.width(ellipsis)) + ellipsis;
    }

    static final class Control extends Button {
        private final int left;
        private final int top;
        private final int span;
        private Component caption;
        private boolean selected;

        Control(Component label, OnPress press, int x, int y, int width) {
            super(x, y, width, CONTROL_HEIGHT, label, press, DEFAULT_NARRATION);
            left = x;
            top = y;
            span = width;
            caption = label;
        }

        void state(Component narration, Component caption, boolean selected) {
            setMessage(narration);
            this.caption = caption;
            this.selected = selected;
        }

        @Override
        public void renderWidget(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
            int border = selected ? ACCENT : BORDER;
            int face = selected ? 0xFF30496F : 0xFF282F3A;
            if (!active) {
                face = TILE;
            } else if (isHovered || isFocused()) {
                face = selected ? 0xFF3B5986 : 0xFF394455;
                border = isFocused() ? TEXT : ACCENT;
            }
            frame(graphics, left, top, span, CONTROL_HEIGHT, border, face);
            graphics.fill(left + 1, top + 1, left + span - 1, top + 2, selected ? ACCENT : 0xFF566172);
            graphics.fill(left + 1, top + CONTROL_HEIGHT - 3, left + span - 1, top + CONTROL_HEIGHT - 1, PANEL);
            Font font = Minecraft.getInstance().font;
            String label = fit(font, caption, span - 8);
            graphics.drawString(font, label, left + (span - font.width(label)) / 2, top + 6, active ? TEXT : MUTED);
        }

        void focusForNavigation(boolean focused) {
            setFocused(focused);
        }
    }
}
