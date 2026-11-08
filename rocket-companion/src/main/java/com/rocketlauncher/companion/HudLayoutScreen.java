package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.RocketCompanion.*;
import static com.rocketlauncher.companion.RocketStyle.*;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.lwjgl.glfw.GLFW;

final class HudLayoutScreen extends ClientPlatform.PointerScreen {
    private static final HudModule[] MODULES = HudModule.values();
    private static final int LEFT_MOUSE_BUTTON = 0;
    private static final int SNAP_DISTANCE = 3;
    private static final int GUIDE = 0x88709DEB;
    private static final int BACK_WIDTH = 54;
    private final Screen parent;
    private final ClientSettings settings;
    private HudModule selected;
    private Control selection;
    private ClientSettings.Position preview;
    private Component dragLabel;
    private double offsetX;
    private double offsetY;

    HudLayoutScreen(Screen parent, ClientSettings settings, HudModule selected) {
        super(text("layout.title"));
        this.parent = parent;
        this.settings = settings;
        this.selected = selected == null ? MODULES[0] : selected;
    }

    @Override
    protected void init() {
        cancelDrag();
        addRenderableWidget(new Control(text("back"), pressed -> onClose(),
            width - BACK_WIDTH - 14, 12, BACK_WIDTH));
        int footerY = height - 29;
        int sideWidth = Math.min(102, (width - 120) / 2);
        int selectWidth = Math.min(158, width - 2 * sideWidth - 30);
        addRenderableWidget(new Control(text("layout.reset"), pressed -> {
            cancelDrag();
            settings.resetPositions();
        }, 8, footerY, sideWidth));
        selection = addRenderableWidget(new Control(selected.displayName(), pressed -> {
            cancelDrag();
            selected = MODULES[(selected.ordinal() + 1) % MODULES.length];
            updateSelection();
        }, (width - selectWidth) / 2, footerY, selectWidth));
        updateSelection();
        addRenderableWidget(new Control(text("layout.done"), pressed -> onClose(),
            width - sideWidth - 8, footerY, sideWidth));
    }

    private void updateSelection() {
        Component caption = text("layout.select", selected.displayName());
        selection.state(text("layout.select_narration", selected.displayName()), caption, true);
    }

    private Area area() { return new Area(12, 69, width - 24, height - 117); }

    @Override
    public void renderBackground(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
        // Keep the world visible behind the layout canvas without vanilla menu blur.
    }

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
        graphics.fill(0, 0, width, height, 0xA6090B10);
        header(graphics, font, text("title"), 8, 6, width - 16, BACK_WIDTH + 6);
        line(graphics, font, title, 12, 42, width - 24, TEXT);
        line(graphics, font, text("layout.hint"), 12, 54, width - 24, MUTED);
        line(graphics, font, text(settings.saveFailed() ? "save_error" : "layout.keys"),
            12, height - 41, width - 24, settings.saveFailed() ? WARNING : MUTED);

        Area area = area();
        frame(graphics, area.x() - 3, area.y() - 3, area.width() + 6, area.height() + 6, BORDER, 0xE8151820);
        drawGuides(graphics, area);
        for (HudModule module : MODULES) {
            if (module != selected) drawPreview(graphics, area, module, mouseX, mouseY);
        }
        drawPreview(graphics, area, selected, mouseX, mouseY);
        super.render(graphics, mouseX, mouseY, delta);
    }

    private void drawGuides(GuiGraphics graphics, Area area) {
        int right = area.x() + area.width();
        int bottom = area.y() + area.height();
        int middleX = area.x() + area.width() / 2;
        int middleY = area.y() + area.height() / 2;
        graphics.fill(middleX, area.y(), middleX + 1, bottom, preview != null && preview.x() == 0.5 ? GUIDE : BORDER);
        graphics.fill(area.x(), middleY, right, middleY + 1, preview != null && preview.y() == 0.5 ? GUIDE : BORDER);
        if (preview == null) return;
        if (preview.x() == 0) graphics.fill(area.x(), area.y(), area.x() + 1, bottom, GUIDE);
        if (preview.x() == 1) graphics.fill(right - 1, area.y(), right, bottom, GUIDE);
        if (preview.y() == 0) graphics.fill(area.x(), area.y(), right, area.y() + 1, GUIDE);
        if (preview.y() == 1) graphics.fill(area.x(), bottom - 1, right, bottom, GUIDE);
    }

    private void drawPreview(GuiGraphics graphics, Area area, HudModule module, int mouseX, int mouseY) {
        Component label = module == selected && dragLabel != null ? dragLabel : module.value(minecraft);
        Bounds bounds = bounds(area, module, label);
        if (module == selected || bounds.contains(mouseX, mouseY)) {
            frame(graphics, bounds.x() - 1, bounds.y() - 1, bounds.width() + 2, WIDGET_HEIGHT + 2,
                module == selected ? ACCENT : MUTED, PANEL);
        }
        drawWidget(graphics, label, bounds, settings.enabled(module));
    }

    private Bounds bounds(Area area, HudModule module, Component label) {
        ClientSettings.Position position = module == selected && preview != null ? preview : settings.position(module);
        return area.place(position, Math.min(area.width(), widgetWidth(label)));
    }

    @Override
    boolean grab(double x, double y, int button) {
        if (button != LEFT_MOUSE_BUTTON) return false;
        Area area = area();
        if (grab(area, selected, x, y)) return true;
        for (int i = MODULES.length - 1; i >= 0; i--) {
            HudModule module = MODULES[i];
            if (module != selected && grab(area, module, x, y)) return true;
        }
        return false;
    }

    private boolean grab(Area area, HudModule module, double x, double y) {
        Component label = module.value(minecraft);
        Bounds bounds = bounds(area, module, label);
        if (!bounds.contains(x, y)) return false;
        selected = module;
        dragLabel = label;
        preview = settings.position(module);
        offsetX = x - bounds.x();
        offsetY = y - bounds.y();
        if (getFocused() instanceof Control focused) focused.focusForNavigation(false);
        setFocused(selection);
        selection.focusForNavigation(true);
        updateSelection();
        return true;
    }

    @Override
    boolean drag(double x, double y, int button) {
        if (preview == null || button != LEFT_MOUSE_BUTTON) return false;
        Area area = area();
        int labelWidth = Math.min(area.width(), widgetWidth(dragLabel));
        ClientSettings.Position position = area.position(x - offsetX, y - offsetY, labelWidth);
        preview = new ClientSettings.Position(snap(position.x(), area.horizontalRange(labelWidth)),
            snap(position.y(), area.verticalRange()));
        return true;
    }

    private static double snap(double position, int range) {
        double nearest = Math.round(position * 2) / 2.0;
        return Math.abs(position - nearest) * range <= SNAP_DISTANCE ? nearest : position;
    }

    @Override
    boolean drop(int button) {
        if (preview == null || button != LEFT_MOUSE_BUTTON) return false;
        if (!preview.equals(settings.position(selected))) settings.setPosition(selected, preview);
        cancelDrag();
        return true;
    }

    @Override
    boolean moveSelection(int key) {
        if (getFocused() != selection || preview != null) return false;
        int dx = key == GLFW.GLFW_KEY_LEFT ? -1 : key == GLFW.GLFW_KEY_RIGHT ? 1 : 0;
        int dy = key == GLFW.GLFW_KEY_UP ? -1 : key == GLFW.GLFW_KEY_DOWN ? 1 : 0;
        if (dx == 0 && dy == 0) return false;
        Area area = area();
        int labelWidth = Math.min(area.width(), widgetWidth(selected.value(minecraft)));
        ClientSettings.Position current = settings.position(selected);
        ClientSettings.Position moved = new ClientSettings.Position(
            ClientSettings.clamp(current.x() + (double) dx / area.horizontalRange(labelWidth)),
            ClientSettings.clamp(current.y() + (double) dy / area.verticalRange()));
        if (!moved.equals(current)) settings.setPosition(selected, moved);
        return true;
    }

    private void cancelDrag() {
        preview = null;
        dragLabel = null;
    }

    @Override
    public void onClose() {
        cancelDrag();
        minecraft.setScreen(parent);
    }
}
