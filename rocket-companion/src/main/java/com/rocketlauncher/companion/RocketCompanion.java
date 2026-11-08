package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.RocketStyle.*;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;

public final class RocketCompanion implements ClientModInitializer {
    private static final HudModule[] MODULES = HudModule.values();
    private static final int MARGIN = 7;
    static final int WIDGET_HEIGHT = 17;
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
                    client.setScreen(new RocketMenu(null, settings));
                }
            }
        });
        ClientPlatform.registerHud(this::renderHud);
        CrosshairRenderer.bind(settings);
        ClientPlatform.registerCrosshair();
        ScreenEvents.AFTER_INIT.register((client, screen, width, height) -> {
            if (screen instanceof TitleScreen || screen instanceof PauseScreen) {
                Screens.getButtons(screen).add(button(text("button"),
                    pressed -> client.setScreen(new RocketMenu(screen, settings)), 8, 8, 72));
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
                drawWidget(graphics, value, area.place(settings.position(module), widgetWidth(value)), true);
            }
        }
    }

    static int widgetWidth(Component text) {
        return Minecraft.getInstance().font.width(text) + 14;
    }

    static void drawWidget(GuiGraphics graphics, Component value, Bounds bounds, boolean enabled) {
        int x = bounds.x();
        int y = bounds.y();
        int right = x + bounds.width();
        int bottom = y + WIDGET_HEIGHT;
        graphics.fill(x, y, right, bottom, enabled ? 0xDB151820 : 0xAA202633);
        graphics.fill(x, y, x + 2, bottom, enabled ? ACCENT : BORDER);
        line(graphics, Minecraft.getInstance().font, value, x + 7, y + 5, bounds.width() - 14, enabled ? TEXT : MUTED);
    }

    record Bounds(int x, int y, int width) {
        boolean contains(double mouseX, double mouseY) {
            return mouseX >= x && mouseX < x + width && mouseY >= y && mouseY < y + WIDGET_HEIGHT;
        }
    }

    record Area(int x, int y, int width, int height) {
        Bounds place(ClientSettings.Position position, int widgetWidth) {
            return new Bounds(x + (int) Math.round(position.x() * horizontalRange(widgetWidth)),
                y + (int) Math.round(position.y() * verticalRange()), widgetWidth);
        }

        ClientSettings.Position position(double mouseX, double mouseY, int widgetWidth) {
            return new ClientSettings.Position(ClientSettings.clamp((mouseX - x) / horizontalRange(widgetWidth)),
                ClientSettings.clamp((mouseY - y) / verticalRange()));
        }

        int horizontalRange(int widgetWidth) { return Math.max(1, width - widgetWidth); }
        int verticalRange() { return Math.max(1, height - WIDGET_HEIGHT); }
    }
}
