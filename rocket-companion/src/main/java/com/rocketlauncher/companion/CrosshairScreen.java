package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.CrosshairStyle.*;
import static com.rocketlauncher.companion.RocketCompanion.text;
import static com.rocketlauncher.companion.RocketStyle.*;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.function.BooleanSupplier;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

final class CrosshairScreen extends Screen {
    private enum Page { SHAPE, SIZE, COLORS, RGB, PRESETS }
    private static final String[] CHANNELS = { "red", "green", "blue" };
    private final Screen parent;
    private final ClientSettings settings;
    private final CrosshairRenderer renderer = new CrosshairRenderer();
    private final List<Runnable> refreshLabels = new ArrayList<>();
    private Page page = Page.SHAPE;
    private Page requestedPage;
    private boolean targetPreview;
    private boolean motionPreview;
    private int left, top, span, previewWidth, contentWidth;

    CrosshairScreen(Screen parent, ClientSettings settings) {
        super(text("crosshair.title"));
        this.parent = parent;
        this.settings = settings;
    }

    @Override
    protected void init() {
        refreshLabels.clear();
        span = Math.min(width - 16, 440);
        left = (width - span) / 2;
        top = Math.max(4, (height - 226) / 2);
        previewWidth = Math.min(140, span / 3);
        contentWidth = span - previewWidth - 23;
        button(text("back"), this::onClose, left + span - 63, top + 6, 55);
        Page[] pages = Page.values();
        int tabWidth = (span - 25) / pages.length;
        for (Page item : pages) {
            Component label = text("crosshair.page." + item.name().toLowerCase(Locale.ROOT));
            Control control = button(label, () -> requestedPage = item,
                left + 8 + item.ordinal() * (tabWidth + 3), top + 38, tabWidth);
            control.state(label, label, item == page);
        }
        switch (page) {
            case SHAPE -> shapeControls();
            case SIZE -> sizeControls();
            case COLORS -> colorControls();
            case RGB -> rgbControls();
            case PRESETS -> presetControls();
        }
        updateLabels();
    }

    private int row(int index) { return top + 64 + index * 23; }
    private int column(int index, int count) { return left + 8 + index * ((contentWidth + 3) / count); }
    private int cell(int count) { return (contentWidth + 3) / count - 3; }

    private Control button(Component label, Runnable action, int x, int y, int width) {
        return addRenderableWidget(new Control(label, pressed -> { action.run(); updateLabels(); }, x, y, width));
    }

    private void toggle(String key, BooleanSupplier state, Runnable action, int x, int y, int width) {
        Component label = text("crosshair." + key);
        Control control = button(label, action, x, y, width);
        refreshLabels.add(() -> {
            boolean selected = state.getAsBoolean();
            Component narration = text(selected ? "module.on" : "module.off", label);
            control.state(narration, label, selected);
        });
    }

    private void shapeControls() {
        for (Mode mode : Mode.values()) {
            String name = mode.name().toLowerCase(Locale.ROOT);
            toggle(name, () -> settings.crosshair().mode() == mode,
                () -> settings.setCrosshair(settings.crosshair().withMode(mode)), column(mode.ordinal(), 3), row(0), cell(3));
        }
        int[] shapes = { CROSS, DOT, RING, HORIZONTAL | BOTTOM };
        String[] names = { "cross", "dot", "ring", "t" };
        for (int i = 0; i < shapes.length; i++) {
            int mask = shapes[i];
            button(text("crosshair." + names[i]), () -> settings.setCrosshair(settings.crosshair().withParts(mask)),
                column(i % 2, 2), row(1 + i / 2), cell(2));
        }
        int[] bits = { HORIZONTAL, TOP, BOTTOM, DOT, RING };
        String[] parts = { "horizontal", "top", "bottom", "center", "ring" };
        for (int i = 0; i < bits.length; i++) {
            int bit = bits[i];
            toggle(parts[i], () -> (settings.crosshair().parts() & bit) != 0,
                () -> settings.setCrosshair(settings.crosshair().withParts(settings.crosshair().parts() ^ bit)),
                column(i % 3, 3), row(3 + i / 3), cell(3));
        }
    }

    private void sizeControls() {
        for (Measure measure : Measure.values()) {
            int y = row(measure.ordinal()), x = column(0, 1), end = x + contentWidth;
            Component label = text("crosshair." + measure.name().toLowerCase(Locale.ROOT));
            Control down = button(text("crosshair.minus"), () -> settings.setCrosshair(measure.adjust(settings.crosshair(), -1)), end - 45, y, 21);
            Control up = button(text("crosshair.plus"), () -> settings.setCrosshair(measure.adjust(settings.crosshair(), 1)), end - 21, y, 21);
            refreshLabels.add(() -> {
                int value = measure.value(settings.crosshair());
                down.state(text("crosshair.decrease", label, value), text("crosshair.minus"), false);
                up.state(text("crosshair.increase", label, value), text("crosshair.plus"), false);
            });
        }
    }

    private void colorControls() {
        addHex(false, row(0));
        addHex(true, row(1));
        toggle("target_color", () -> settings.crosshair().targetColor() >= 0, () -> {
            CrosshairStyle style = settings.crosshair();
            settings.setCrosshair(style.withTarget(style.targetColor() >= 0 ? -1 : 0xFF6464));
        }, column(0, 2), row(2), cell(2));
        toggle("test_target", () -> targetPreview, () -> targetPreview = !targetPreview, column(1, 2), row(2), cell(2));
        toggle("shadow", () -> settings.crosshair().shadow(), () -> {
            CrosshairStyle style = settings.crosshair();
            settings.setCrosshair(style.withEffects(!style.shadow(), style.dynamic()));
        }, column(0, 2), row(3), cell(2));
        toggle("dynamic", () -> settings.crosshair().dynamic(), () -> {
            CrosshairStyle style = settings.crosshair();
            settings.setCrosshair(style.withEffects(style.shadow(), !style.dynamic()));
        }, column(1, 2), row(3), cell(2));
        toggle("test_motion", () -> motionPreview, () -> motionPreview = !motionPreview, column(0, 1), row(4), cell(1));
    }

    private void addHex(boolean target, int y) {
        int color = target ? settings.crosshair().targetColor() : settings.crosshair().color();
        EditBox field = addRenderableWidget(new EditBox(font, left + 8 + contentWidth - 70, y, 70, 20,
            text(target ? "crosshair.target_hex" : "crosshair.hex")));
        field.setMaxLength(7);
        field.setValue(String.format(Locale.ROOT, "#%06X", color < 0 ? 0xFF6464 : color));
        field.setResponder(value -> {
            if (value.matches("#?[a-fA-F0-9]{6}")) {
                int rgb = Integer.parseInt(value.replace("#", ""), 16);
                CrosshairStyle current = settings.crosshair();
                settings.setCrosshair(target ? current.withTarget(rgb) : current.withColor(rgb));
                updateLabels();
            }
        });
    }

    private void rgbControls() {
        int x = left + 8 + contentWidth - 55;
        for (int i = 0; i < CHANNELS.length; i++) {
            int shift = (2 - i) * 8;
            EditBox field = addRenderableWidget(new EditBox(font, x, row(i), 55, 20, text("crosshair." + CHANNELS[i])));
            field.setMaxLength(3);
            field.setValue(Integer.toString(settings.crosshair().color() >> shift & 255));
            field.setResponder(value -> {
                if (!value.matches("[0-9]{1,3}")) return;
                int channel = Integer.parseInt(value);
                if (channel > 255) return;
                CrosshairStyle style = settings.crosshair();
                int color = style.color() & ~(255 << shift) | channel << shift;
                settings.setCrosshair(style.withColor(color));
            });
        }
    }

    private void presetControls() {
        CrosshairStyle custom = DEFAULT.withMode(Mode.CUSTOM);
        CrosshairStyle[] styles = {
            DEFAULT, new CrosshairStyle(Mode.CUSTOM, CROSS, 3, 1, 1, 90, 0xFFFFFF, 0, false, false, -1),
            new CrosshairStyle(Mode.CUSTOM, CROSS | DOT, 5, 1, 3, 100, 0x6CFFB2, 1, false, false, 0xFF6464),
            custom.withParts(DOT), custom.withEffects(false, true)
        };
        String[] names = { "default", "minimal", "pvp", "dot", "dynamic" };
        for (int i = 0; i < styles.length; i++) {
            CrosshairStyle style = styles[i];
            button(text("crosshair." + names[i]), () -> settings.setCrosshair(style), column(i % 3, 3), row(i / 3), cell(3));
        }
        for (int slot = 1; slot <= 3; slot++) {
            int index = slot;
            Control load = button(text("crosshair.load", slot), () -> settings.loadCrosshairPreset(index),
                column(0, 2), row(slot + 1), cell(2));
            refreshLabels.add(() -> load.active = settings.hasCrosshairPreset(index));
            button(text("crosshair.save", slot), () -> settings.saveCrosshairPreset(index), column(1, 2), row(slot + 1), cell(2));
        }
    }

    private void updateLabels() { refreshLabels.forEach(Runnable::run); }

    @Override
    public void tick() {
        if (requestedPage != null) {
            page = requestedPage;
            requestedPage = null;
            setFocused(null);
            clearWidgets();
            init();
        }
        updateLabels();
    }

    @Override
    public void renderBackground(GuiGraphics graphics, int mouseX, int mouseY, float delta) {}

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
        graphics.fill(0, 0, width, height, 0xB0090B10);
        frame(graphics, left, top, span, 223, BORDER, PANEL);
        header(graphics, font, title, left, top, span, 63);
        int previewX = left + span - previewWidth - 8;
        frame(graphics, previewX, row(0), previewWidth, 112, BORDER, 0xFF536276);
        line(graphics, font, text("crosshair.preview"), previewX + 5, row(0) + 6, previewWidth - 10, TEXT);
        CrosshairStyle style = settings.crosshair();
        if (style.mode() != Mode.HIDDEN) {
            CrosshairStyle displayed = style.mode() == Mode.VANILLA ? DEFAULT.withParts(CROSS | DOT) : style;
            renderer.draw(graphics, displayed, previewX + previewWidth / 2, row(0) + 58,
                motionPreview && displayed.dynamic() ? 5 : 0, targetPreview);
        }
        if (page == Page.SIZE) for (Measure measure : Measure.values()) {
            line(graphics, font, text("crosshair.measure", text("crosshair." + measure.name().toLowerCase(Locale.ROOT)),
                measure.value(style)), left + 9, row(measure.ordinal()) + 6, contentWidth - 49, TEXT);
        }
        if (page == Page.COLORS) {
            line(graphics, font, text("crosshair.hex"), left + 9, row(0) + 6, contentWidth - 73, TEXT);
            line(graphics, font, text("crosshair.target_hex"), left + 9, row(1) + 6, contentWidth - 73, TEXT);
        }
        if (page == Page.RGB) for (int i = 0; i < CHANNELS.length; i++) {
            line(graphics, font, text("crosshair." + CHANNELS[i]), left + 9, row(i) + 6, contentWidth - 59, TEXT);
        }
        String hint = settings.saveFailed() ? "save_error" : "crosshair.hint";
        line(graphics, font, text(hint), left + 8, top + 204, span - 16, settings.saveFailed() ? WARNING : MUTED);
        super.render(graphics, mouseX, mouseY, delta);
    }

    @Override
    public void onClose() { minecraft.setScreen(parent); }
}
