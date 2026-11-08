package com.rocketlauncher.companion;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;

public final class RocketCompanion implements ClientModInitializer {
    private static final HudModule[] MODULES = HudModule.values();
    private static final int ACCENT = 0xFFFF896A;
    private static final int TEXT = 0xFFF0F4FA;
    private static final int SUBTLE = 0xFFABB8C9;
    private static final int MARGIN = 7;
    private static final int ROW_HEIGHT = 17;
    private static final int LEFT_MOUSE_BUTTON = 0;
    private final ClientSettings settings = new ClientSettings();
    private int ticks;

    @Override
    public void onInitializeClient() {
        settings.initialize();
        var menuKey = ClientPlatform.registerMenuKey();
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            if (settings.shared() && ++ticks >= 40) {
                settings.refresh();
                ticks = 0;
            }
            while (menuKey.consumeClick()) {
                if (client.screen == null && client.player != null) {
                    client.setScreen(new RocketMenu(null));
                }
            }
        });
        ClientPlatform.registerHud(this::renderHud);
        ScreenEvents.AFTER_INIT.register((client, screen, width, height) -> {
            if (screen instanceof TitleScreen || screen instanceof PauseScreen) {
                Screens.getButtons(screen).add(button(text("button"),
                    pressed -> client.setScreen(new RocketMenu(screen)), 8, 8, 72));
            }
        });
    }

    static Component text(String key, Object... arguments) {
        return Component.translatable("rocket_companion." + key, arguments);
    }

    private static Button button(Component label, Button.OnPress press, int x, int y, int width) {
        return Button.builder(label, press).bounds(x, y, width, 20).build();
    }

    private void renderHud(GuiGraphics graphics) {
        var client = Minecraft.getInstance();
        if (client.player == null || client.options.hideGui || ClientPlatform.debugVisible(client)) return;
        var area = new Area(MARGIN, MARGIN, graphics.guiWidth() - 2 * MARGIN, graphics.guiHeight() - 2 * MARGIN);
        for (HudModule module : MODULES) {
            if (settings.enabled(module)) {
                Component value = module.value(client);
                drawWidget(graphics, value, area.place(settings.position(module), widgetWidth(value)), true, false);
            }
        }
    }

    private static int widgetWidth(Component text) {
        return Minecraft.getInstance().font.width(text) + 14;
    }

    private static void drawWidget(GuiGraphics graphics, Component value, Bounds bounds, boolean enabled, boolean focused) {
        int x = bounds.x();
        int y = bounds.y();
        int right = x + bounds.width();
        int bottom = y + ROW_HEIGHT;
        graphics.fill(x, y, right, bottom, enabled ? 0xDB121722 : 0xAA202633);
        graphics.fill(x, y, x + 2, bottom, enabled || focused ? ACCENT : SUBTLE);
        if (focused) graphics.fill(x, bottom - 1, right, bottom, ACCENT);
        graphics.drawString(Minecraft.getInstance().font, value, x + 7, y + 5, enabled ? TEXT : SUBTLE, false);
    }

    private record Bounds(int x, int y, int width) {
        boolean contains(double mouseX, double mouseY) {
            return mouseX >= x && mouseX < x + width && mouseY >= y && mouseY < y + ROW_HEIGHT;
        }
    }

    private record Area(int x, int y, int width, int height) {
        Bounds place(ClientSettings.Position position, int widgetWidth) {
            return new Bounds(x + (int) Math.round(position.x() * horizontalRange(widgetWidth)),
                y + (int) Math.round(position.y() * verticalRange()), widgetWidth);
        }

        ClientSettings.Position position(double mouseX, double mouseY, int widgetWidth) {
            return new ClientSettings.Position(ClientSettings.clamp((mouseX - x) / horizontalRange(widgetWidth)),
                ClientSettings.clamp((mouseY - y) / verticalRange()));
        }

        private int horizontalRange(int widgetWidth) { return Math.max(1, width - widgetWidth); }
        private int verticalRange() { return Math.max(1, height - ROW_HEIGHT); }
    }

    private final class RocketMenu extends Screen {
        private static final int TOGGLE_ROWS = (MODULES.length + 1) / 2;
        private final Screen parent;
        private final Button[] toggles = new Button[MODULES.length];

        private RocketMenu(Screen parent) {
            super(text("title"));
            this.parent = parent;
        }

        @Override
        protected void init() {
            settings.refresh();
            int buttonWidth = Math.min(126, (width - 30) / 2);
            int y = toggleTop();
            for (int index = 0; index < MODULES.length; index++) {
                HudModule module = MODULES[index];
                int x = width / 2 - buttonWidth - 3 + (index % 2) * (buttonWidth + 6);
                toggles[index] = addRenderableWidget(button(label(module), button -> {
                    settings.setEnabled(module, !settings.enabled(module));
                    button.setMessage(label(module));
                }, x, y + (index / 2) * 24, buttonWidth));
            }
            int fullWidth = buttonWidth * 2 + 6;
            int x = (width - fullWidth) / 2;
            addRenderableWidget(button(text("layout"), pressed -> minecraft.setScreen(new LayoutEditor(this)),
                x, y + TOGGLE_ROWS * 24 + 3, fullWidth));
            addRenderableWidget(button(text("options"), pressed -> ClientPlatform.openOptions(this, minecraft),
                x, y + TOGGLE_ROWS * 24 + 31, buttonWidth));
            addRenderableWidget(button(text("back"), pressed -> onClose(),
                x + buttonWidth + 6, y + TOGGLE_ROWS * 24 + 31, buttonWidth));
        }

        private int toggleTop() { return (height - (TOGGLE_ROWS * 24 + 51)) / 2; }

        private Component label(HudModule module) {
            return text(settings.enabled(module) ? "module.on" : "module.off", module.displayName());
        }

        @Override
        public void tick() {
            for (int i = 0; i < MODULES.length; i++) toggles[i].setMessage(label(MODULES[i]));
        }

        @Override
        public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
            graphics.fill(0, 0, width, height, 0xB50E141D);
            super.render(graphics, mouseX, mouseY, delta);
            graphics.drawCenteredString(font, title, width / 2, toggleTop() - 40, ACCENT);
            graphics.drawCenteredString(font, text(settings.shared() ? "scope.shared" : "scope.local"),
                width / 2, toggleTop() - 21, SUBTLE);
            if (settings.saveFailed()) {
                graphics.drawCenteredString(font, text("save_error"), width / 2, toggleTop() + TOGGLE_ROWS * 24 + 57, ACCENT);
            }
        }

        @Override
        public void onClose() { minecraft.setScreen(parent); }
    }

    private final class LayoutEditor extends ClientPlatform.PointerScreen {
        private final Screen parent;
        private HudModule dragging;
        private ClientSettings.Position preview;
        private double offsetX;
        private double offsetY;

        private LayoutEditor(Screen parent) {
            super(text("layout"));
            this.parent = parent;
        }

        @Override
        protected void init() {
            dragging = null;
            int buttonWidth = Math.min(110, (width - 30) / 2);
            addRenderableWidget(button(text("layout.reset"), pressed -> settings.resetPositions(),
                width / 2 - buttonWidth - 3, height - 28, buttonWidth));
            addRenderableWidget(button(text("layout.done"), pressed -> onClose(),
                width / 2 + 3, height - 28, buttonWidth));
        }

        private Area area() { return new Area(MARGIN + 2, 38, width - 2 * (MARGIN + 2), height - 78); }

        private Bounds bounds(HudModule module, Component label) {
            return area().place(module == dragging ? preview : settings.position(module), widgetWidth(label));
        }

        @Override
        public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
            super.render(graphics, mouseX, mouseY, delta);
            var area = area();
            graphics.fill(area.x() - 2, area.y() - 2, area.x() + area.width() + 2, area.y() + area.height() + 2, 0xFF424D60);
            graphics.fill(area.x() - 1, area.y() - 1, area.x() + area.width() + 1, area.y() + area.height() + 1, 0xED131923);
            for (HudModule module : MODULES) {
                if (module != dragging) drawPreview(graphics, module, mouseX, mouseY);
            }
            if (dragging != null) drawPreview(graphics, dragging, mouseX, mouseY);
            graphics.drawCenteredString(font, title, width / 2, 8, ACCENT);
            graphics.drawCenteredString(font, text(settings.saveFailed() ? "save_error" : "layout.hint"),
                width / 2, 23, SUBTLE);
        }

        private void drawPreview(GuiGraphics graphics, HudModule module, int mouseX, int mouseY) {
            Component label = module.value(minecraft);
            Bounds bounds = bounds(module, label);
            drawWidget(graphics, label, bounds, settings.enabled(module),
                module == dragging || bounds.contains(mouseX, mouseY));
        }

        @Override
        boolean grab(double x, double y, int button) {
            if (button != LEFT_MOUSE_BUTTON) return false;
            for (int index = MODULES.length - 1; index >= 0; index--) {
                HudModule module = MODULES[index];
                Bounds bounds = bounds(module, module.value(minecraft));
                if (bounds.contains(x, y)) {
                    offsetX = x - bounds.x();
                    offsetY = y - bounds.y();
                    preview = settings.position(module);
                    dragging = module;
                    return true;
                }
            }
            return false;
        }

        @Override
        boolean drag(double x, double y, int button) {
            if (dragging == null || button != LEFT_MOUSE_BUTTON) return false;
            preview = area().position(x - offsetX, y - offsetY, widgetWidth(dragging.value(minecraft)));
            return true;
        }

        @Override
        boolean drop(int button) {
            if (dragging == null || button != LEFT_MOUSE_BUTTON) return false;
            settings.setPosition(dragging, preview);
            dragging = null;
            return true;
        }

        @Override
        public void onClose() {
            dragging = null;
            minecraft.setScreen(parent);
        }
    }
}
