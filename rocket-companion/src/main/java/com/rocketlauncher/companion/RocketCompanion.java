package com.rocketlauncher.companion;

import com.mojang.blaze3d.platform.InputConstants;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.options.VideoSettingsScreen;
import net.minecraft.client.gui.screens.options.controls.ControlsScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class RocketCompanion implements ClientModInitializer {
    private static final Logger LOGGER = LoggerFactory.getLogger("Rocket Launcher Client");
    private final Path hudConfig = FabricLoader.getInstance().getConfigDir().resolve("rocket-client-hud.txt");
    private boolean showHud;

    @Override
    public void onInitializeClient() {
        try {
            showHud = Files.exists(hudConfig) && Boolean.parseBoolean(Files.readString(hudConfig).trim());
        } catch (IOException error) {
            LOGGER.warn("Could not read Rocket HUD preference", error);
        }
        var category = KeyMapping.Category.register(Identifier.fromNamespaceAndPath("rocket_companion", "client"));
        var menuKey = KeyBindingHelper.registerKeyBinding(new KeyMapping(
            "key.rocket_companion.menu", InputConstants.Type.KEYSYM, InputConstants.UNKNOWN.getValue(), category));
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            while (menuKey.consumeClick()) {
                if (client.screen == null && client.player != null) {
                    client.setScreen(new RocketMenu(null));
                }
            }
        });
        HudElementRegistry.attachElementBefore(VanillaHudElements.CHAT,
            Identifier.fromNamespaceAndPath("rocket_companion", "hud"),
            (graphics, delta) -> renderHud(graphics));

        ScreenEvents.AFTER_INIT.register((client, screen, width, height) -> {
            if (screen instanceof TitleScreen || screen instanceof PauseScreen) {
                Screens.getButtons(screen).add(
                    Button.builder(Component.translatable("rocket_companion.button"),
                        button -> client.setScreen(new RocketMenu(screen)))
                        .bounds(8, 8, 72, 20)
                        .build()
                );
            }
        });
    }

    private void renderHud(GuiGraphics graphics) {
        var client = Minecraft.getInstance();
        if (!showHud || client.player == null || client.getDebugOverlay().showDebugScreen()) {
            return;
        }
        var position = client.player.blockPosition();
        var fps = Component.translatable("rocket_companion.hud.fps", client.getFps());
        var coords = Component.translatable("rocket_companion.hud.position", position.getX(), position.getY(), position.getZ());
        int right = 22 + Math.max(client.font.width(fps), client.font.width(coords));
        graphics.fill(8, 8, right, 36, 0xB5121722);
        graphics.fill(8, 8, 10, 36, 0xFFFF896A);
        graphics.drawString(client.font, fps, 15, 12, 0xFFFF896A);
        graphics.drawString(client.font, coords, 15, 24, 0xFFF0F4FA);
    }

    private Component hudLabel() {
        return Component.translatable(showHud ? "rocket_companion.hud.on" : "rocket_companion.hud.off");
    }

    private final class RocketMenu extends Screen {
        private final Screen parent;

        private RocketMenu(Screen parent) {
            super(Component.translatable("rocket_companion.title"));
            this.parent = parent;
        }

        @Override
        protected void init() {
            int x = width / 2 - 92;
            int y = height / 2 - 29;
            addRenderableWidget(Button.builder(hudLabel(), button -> {
                showHud = !showHud;
                button.setMessage(hudLabel());
                try {
                    Files.createDirectories(hudConfig.getParent());
                    Files.writeString(hudConfig, Boolean.toString(showHud));
                } catch (IOException error) {
                    LOGGER.warn("Could not save Rocket HUD preference", error);
                }
            }).bounds(x, y, 184, 20).build());
            addRenderableWidget(Button.builder(Component.translatable("rocket_companion.video"),
                button -> minecraft.setScreen(new VideoSettingsScreen(this, minecraft, minecraft.options)))
                .bounds(x, y + 24, 184, 20).build());
            addRenderableWidget(Button.builder(Component.translatable("rocket_companion.controls"),
                button -> minecraft.setScreen(new ControlsScreen(this, minecraft.options)))
                .bounds(x, y + 48, 184, 20).build());
            addRenderableWidget(Button.builder(Component.translatable("rocket_companion.back"),
                button -> onClose()).bounds(x, y + 76, 184, 20).build());
        }

        @Override
        public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
            super.render(graphics, mouseX, mouseY, delta);
            graphics.drawCenteredString(font, title, width / 2, height / 2 - 65, 0xFFFF896A);
            graphics.drawCenteredString(font, Component.translatable("rocket_companion.subtitle"),
                width / 2, height / 2 - 49, 0xFFE0E6EF);
        }

        @Override
        public void onClose() {
            minecraft.setScreen(parent);
        }
    }
}
